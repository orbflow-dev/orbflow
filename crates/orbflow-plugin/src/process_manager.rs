// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Plugin process manager — spawns and supervises gRPC plugin processes.
//!
//! When a plugin manifest declares `PluginProtocol::Grpc`, the process manager
//! can automatically:
//!
//! 1. Install dependencies (e.g. `pip install -r requirements.txt`)
//! 2. Determine the correct launch command from the manifest's `language` field
//! 3. Spawn the plugin as a child process with prefixed stdout/stderr capture
//! 4. Wait for the gRPC HealthCheck to succeed (with retries)
//! 5. Kill all managed processes on shutdown
//!
//! Plugins are spawned in parallel for fast startup. Individual plugins can be
//! started/stopped/restarted via the HTTP API.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use orbflow_core::error::OrbflowError;
use orbflow_core::validate_plugin_name;
use orbflow_registry::index::LocalIndex;
use orbflow_registry::manifest::{PluginManifest, PluginProtocol};

/// Validates that a directory name is safe for use as a Python module name.
fn validate_module_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Validates that a resolved path is contained within the plugins directory.
/// Prevents path traversal attacks via symlinks or `..` components.
fn validate_path_containment(path: &Path, plugins_dir: &Path) -> Result<(), OrbflowError> {
    let canon_path = path
        .canonicalize()
        .map_err(|e| OrbflowError::Internal(format!("cannot resolve plugin path: {e}")))?;
    let canon_root = plugins_dir
        .canonicalize()
        .map_err(|e| OrbflowError::Internal(format!("cannot resolve plugins dir: {e}")))?;
    if !canon_path.starts_with(&canon_root) {
        return Err(OrbflowError::Internal(
            "plugin path escapes plugins directory".into(),
        ));
    }
    Ok(())
}

// ─── Allowed env vars for child processes ───────────────────────────────────

/// Environment variables stripped from plugin child processes to prevent
/// credential leakage. All other variables are inherited so that language
/// runtimes (Python virtualenvs, Node, etc.) work correctly.
pub const DENIED_ENV_VARS: &[&str] = &[
    "DATABASE_URL",
    "ORBFLOW_AUTH_TOKEN",
    "ORBFLOW_BOOTSTRAP_ADMIN",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "GOOGLE_AI_API_KEY",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_ROLE_ARN",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
    "AZURE_CLIENT_ID",
    "AZURE_CLIENT_SECRET",
    "AZURE_TENANT_ID",
    "GCP_SERVICE_ACCOUNT_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GITHUB_TOKEN",
    "NATS_URL",
    "NATS_TOKEN",
    "ENCRYPTION_KEY",
    "ORBFLOW_ENCRYPTION_KEY",
    "CREDENTIAL_ENCRYPTION_KEY",
];

// ─── Types ──────────────────────────────────────────────────────────────────

/// A managed plugin process with its gRPC endpoint info.
struct ManagedPlugin {
    name: String,
    child: Child,
    port: u16,
    language: String,
    /// Handles for stdout/stderr forwarding tasks — joined on stop/shutdown
    /// to ensure all output is flushed.
    log_handles: Vec<tokio::task::JoinHandle<()>>,
}

/// Manages the lifecycle of gRPC plugin processes.
///
/// Scans the plugins directory for manifests declaring `PluginProtocol::Grpc`,
/// spawns each plugin, waits for health, and kills them all on [`shutdown`].
pub struct PluginProcessManager {
    managed: Vec<ManagedPlugin>,
    /// Plugins directory path (retained for path validation).
    plugins_dir: PathBuf,
    /// Cached manifests keyed by plugin name.
    manifests: HashMap<String, CachedManifest>,
}

#[derive(Clone)]
struct CachedManifest {
    manifest: PluginManifest,
    install_path: PathBuf,
}

/// Info about a successfully spawned and healthy plugin.
#[derive(Debug, Clone)]
pub struct SpawnedPlugin {
    pub name: String,
    pub address: String,
    pub port: u16,
}

/// Plugin status as seen by the API.
#[derive(Debug, Clone, Serialize)]
pub struct PluginStatus {
    pub name: String,
    pub status: PluginRunState,
    pub port: Option<u16>,
    pub address: Option<String>,
    pub language: Option<String>,
}

/// Run state of a managed plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRunState {
    Running,
    Stopped,
    /// Manifest found but never started.
    Available,
}

/// Optional per-plugin overrides for spawning.
#[derive(Debug, Clone, Default)]
pub struct PluginSpawnOverride {
    pub port: Option<u16>,
}

// ─── Dependency installation ────────────────────────────────────────────────

/// Installs plugin dependencies before spawning.
///
/// For Python plugins: runs `pip install -r requirements.txt` if it exists.
/// For Node plugins: runs `npm install` if `package.json` exists.
async fn install_dependencies(language: &str, plugin_dir: &Path, name: &str) {
    match language {
        "python" => {
            let req_file = plugin_dir.join("requirements.txt");
            if req_file.exists() {
                tracing::info!(plugin = %name, "installing Python dependencies");
                let mut cmd = tokio::process::Command::new("pip");
                cmd.args(["install", "-q", "-r", "requirements.txt"])
                    .current_dir(plugin_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                for var in DENIED_ENV_VARS {
                    cmd.env_remove(var);
                }
                let result = cmd.status().await;
                match result {
                    Ok(status) if status.success() => {
                        tracing::info!(plugin = %name, "dependencies installed");
                    }
                    Ok(status) => {
                        tracing::warn!(
                            plugin = %name,
                            code = ?status.code(),
                            "pip install failed"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(plugin = %name, error = %e, "failed to run pip install");
                    }
                }
            }
        }
        "typescript" | "javascript" | "node" => {
            let pkg_json = plugin_dir.join("package.json");
            if pkg_json.exists() && !plugin_dir.join("node_modules").exists() {
                tracing::info!(plugin = %name, "installing Node dependencies");
                let mut cmd = tokio::process::Command::new("npm");
                cmd.args(["install", "--silent"])
                    .current_dir(plugin_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                for var in DENIED_ENV_VARS {
                    cmd.env_remove(var);
                }
                let result = cmd.status().await;
                match result {
                    Ok(status) if status.success() => {
                        tracing::info!(plugin = %name, "dependencies installed");
                    }
                    Ok(status) => {
                        tracing::warn!(
                            plugin = %name,
                            code = ?status.code(),
                            "npm install failed"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(plugin = %name, error = %e, "failed to run npm install");
                    }
                }
            }
        }
        _ => {}
    }
}

// ─── Stdout/stderr log forwarding ───────────────────────────────────────────

/// Spawns a background task that reads lines from a reader and logs them
/// with a `[plugin-name]` prefix.
fn forward_output(
    reader: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    plugin_name: String,
    stream: &'static str,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match stream {
                "stderr" => tracing::warn!(plugin = %plugin_name, output = %line, "plugin stderr"),
                _ => tracing::info!(plugin = %plugin_name, output = %line, "plugin stdout"),
            }
        }
    })
}

// ─── PluginProcessManager ───────────────────────────────────────────────────

impl PluginProcessManager {
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            managed: Vec::new(),
            plugins_dir: plugins_dir.into(),
            manifests: HashMap::new(),
        }
    }

    /// Scans the plugins directory and caches all gRPC manifests.
    pub fn scan(&mut self) -> Result<(), OrbflowError> {
        let mut index = LocalIndex::new(&self.plugins_dir);
        index
            .scan()
            .map_err(|e| OrbflowError::Internal(format!("failed to scan plugins: {e}")))?;

        self.manifests.clear();
        for installed in index.list() {
            if matches!(&installed.manifest.protocol, PluginProtocol::Grpc { .. }) {
                self.manifests.insert(
                    installed.manifest.name.clone(),
                    CachedManifest {
                        manifest: installed.manifest.clone(),
                        install_path: installed.install_path.clone(),
                    },
                );
            }
        }

        Ok(())
    }

    /// Scans and spawns all gRPC plugins **in parallel**.
    ///
    /// Automatically assigns unique ports starting from 50051, incrementing
    /// for each plugin to avoid bind conflicts. All plugins are spawned
    /// concurrently so one slow/failing plugin doesn't block the others.
    pub async fn spawn_all(
        &mut self,
        overrides: &HashMap<String, PluginSpawnOverride>,
    ) -> Vec<SpawnedPlugin> {
        if let Err(e) = self.scan() {
            tracing::warn!("failed to scan plugins for auto-start: {e}");
            return Vec::new();
        }

        let mut names: Vec<String> = self.manifests.keys().cloned().collect();
        names.sort(); // Deterministic port assignment.

        // Pre-assign ports and collect spawn configs.
        let mut spawn_configs: Vec<(String, CachedManifest, u16)> = Vec::new();
        let mut next_port: u16 = 50051;

        for name in &names {
            let port = if let Some(ovr) = overrides.get(name) {
                ovr.port.unwrap_or_else(|| {
                    let p = next_port;
                    next_port = next_port.saturating_add(1);
                    p
                })
            } else {
                let p = next_port;
                next_port = next_port.saturating_add(1);
                p
            };

            if let Some(cached) = self.manifests.get(name) {
                spawn_configs.push((name.clone(), cached.clone(), port));
            }
        }

        // Install dependencies in parallel first.
        let mut dep_handles = Vec::new();
        for (name, cached, _port) in &spawn_configs {
            let language = cached
                .manifest
                .language
                .clone()
                .unwrap_or_else(|| "python".into());
            let dir = cached.install_path.clone();
            let plugin_name = name.clone();
            dep_handles.push(tokio::spawn(async move {
                install_dependencies(&language, &dir, &plugin_name).await;
            }));
        }
        for handle in dep_handles {
            let _ = handle.await;
        }

        // Spawn all plugins in parallel.
        let plugins_dir = self.plugins_dir.clone();
        let mut spawn_handles = Vec::new();

        for (name, cached, port) in spawn_configs {
            let plugins_dir = plugins_dir.clone();
            spawn_handles.push(tokio::spawn(async move {
                spawn_single_plugin(&name, &cached, port, &plugins_dir).await
            }));
        }

        let mut spawned = Vec::new();
        for handle in spawn_handles {
            match handle.await {
                Ok(Ok((sp, managed))) => {
                    tracing::info!(
                        plugin = %sp.name,
                        address = %sp.address,
                        "plugin is healthy"
                    );
                    self.managed.push(managed);
                    spawned.push(sp);
                }
                Ok(Err((name, e))) => {
                    tracing::warn!(plugin = %name, error = %e, "failed to auto-start plugin");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "plugin spawn task panicked");
                }
            }
        }

        tracing::info!(
            started = spawned.len(),
            total = self.manifests.len(),
            "parallel plugin startup complete"
        );

        spawned
    }

    /// Start a single plugin by name. Returns its endpoint info.
    ///
    /// Installs dependencies, spawns the child process, then polls the gRPC
    /// HealthCheck endpoint. If health check fails, the child is removed from
    /// the managed list (and killed via `kill_on_drop`) so that retry is possible.
    pub async fn start_plugin(
        &mut self,
        name: &str,
        overrides: Option<&PluginSpawnOverride>,
    ) -> Result<SpawnedPlugin, OrbflowError> {
        validate_plugin_name(name)?;

        if self.managed.iter().any(|m| m.name == name) {
            return Err(OrbflowError::Conflict);
        }

        let cached = self
            .manifests
            .get(name)
            .cloned()
            .ok_or(OrbflowError::NotFound)?;

        let PluginProtocol::Grpc { default_port } = &cached.manifest.protocol else {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "plugin '{name}' does not use gRPC protocol"
            )));
        };

        let port = overrides.and_then(|o| o.port).unwrap_or(*default_port);

        let language = cached.manifest.language.as_deref().unwrap_or("python");

        // Install dependencies before spawning.
        install_dependencies(language, &cached.install_path, name).await;

        let (sp, managed) = spawn_single_plugin(name, &cached, port, &self.plugins_dir)
            .await
            .map_err(|(_name, e)| e)?;

        tracing::info!(plugin = %sp.name, address = %sp.address, "plugin is healthy");
        self.managed.push(managed);
        Ok(sp)
    }

    /// Stop a single plugin by name.
    pub async fn stop_plugin(&mut self, name: &str) -> Result<(), OrbflowError> {
        validate_plugin_name(name)?;

        let idx = self
            .managed
            .iter()
            .position(|m| m.name == name)
            .ok_or(OrbflowError::NotFound)?;

        let plugin = &mut self.managed[idx];
        tracing::info!(plugin = %name, "stopping plugin process");
        plugin
            .child
            .kill()
            .await
            .map_err(|e| OrbflowError::Internal(format!("failed to kill plugin '{name}': {e}")))?;
        // Join log forwarding handles to flush remaining output.
        for handle in plugin.log_handles.drain(..) {
            let _ = handle.await;
        }

        self.managed.remove(idx);
        Ok(())
    }

    /// Restart a plugin: stop then start.
    pub async fn restart_plugin(
        &mut self,
        name: &str,
        overrides: Option<&PluginSpawnOverride>,
    ) -> Result<SpawnedPlugin, OrbflowError> {
        match self.stop_plugin(name).await {
            Ok(()) | Err(OrbflowError::NotFound) => {}
            Err(e) => return Err(e),
        }
        self.start_plugin(name, overrides).await
    }

    /// Reload all plugins: stop everything, re-scan, and spawn all again.
    pub async fn reload_all(
        &mut self,
        overrides: &HashMap<String, PluginSpawnOverride>,
    ) -> Vec<SpawnedPlugin> {
        tracing::info!("reloading all plugins");
        self.shutdown().await;
        self.spawn_all(overrides).await
    }

    /// Returns the status of all known gRPC plugins (running + available).
    pub fn list_status(&self) -> Vec<PluginStatus> {
        let mut statuses: Vec<PluginStatus> = Vec::new();

        let running_names: HashSet<&str> = self.managed.iter().map(|m| m.name.as_str()).collect();

        for m in &self.managed {
            statuses.push(PluginStatus {
                name: m.name.clone(),
                status: PluginRunState::Running,
                port: Some(m.port),
                address: Some(format!("http://localhost:{}", m.port)),
                language: Some(m.language.clone()),
            });
        }

        for (name, cached) in &self.manifests {
            if running_names.contains(name.as_str()) {
                continue;
            }
            let port = match &cached.manifest.protocol {
                PluginProtocol::Grpc { default_port } => Some(*default_port),
                _ => None,
            };
            statuses.push(PluginStatus {
                name: name.clone(),
                status: PluginRunState::Available,
                port,
                address: None,
                language: cached.manifest.language.clone(),
            });
        }

        statuses.sort_by(|a, b| a.name.cmp(&b.name));
        statuses
    }

    /// Returns the status of a single plugin by name.
    pub fn get_status(&self, name: &str) -> Option<PluginStatus> {
        if let Some(m) = self.managed.iter().find(|m| m.name == name) {
            return Some(PluginStatus {
                name: m.name.clone(),
                status: PluginRunState::Running,
                port: Some(m.port),
                address: Some(format!("http://localhost:{}", m.port)),
                language: Some(m.language.clone()),
            });
        }

        if let Some(cached) = self.manifests.get(name) {
            let port = match &cached.manifest.protocol {
                PluginProtocol::Grpc { default_port } => Some(*default_port),
                _ => None,
            };
            return Some(PluginStatus {
                name: name.to_string(),
                status: PluginRunState::Available,
                port,
                address: None,
                language: cached.manifest.language.clone(),
            });
        }

        None
    }

    /// Shuts down all managed plugin processes gracefully.
    pub async fn shutdown(&mut self) {
        for plugin in &mut self.managed {
            tracing::info!(plugin = %plugin.name, "stopping plugin process");
            if let Err(e) = plugin.child.kill().await {
                tracing::warn!(
                    plugin = %plugin.name,
                    error = %e,
                    "failed to kill plugin process"
                );
            }
            // Join log forwarding handles to flush remaining output.
            for handle in plugin.log_handles.drain(..) {
                let _ = handle.await;
            }
        }
        self.managed.clear();
    }
}

impl Default for PluginProcessManager {
    fn default() -> Self {
        Self::new("./plugins")
    }
}

// ─── spawn_single_plugin ────────────────────────────────────────────────────

/// Spawns a single plugin process, waits for health, and returns the managed
/// entry. Designed to be called from `tokio::spawn` for parallel startup.
///
/// Returns `Ok((SpawnedPlugin, ManagedPlugin))` on success, or
/// `Err((plugin_name, OrbflowError))` on failure (includes name for logging).
async fn spawn_single_plugin(
    name: &str,
    cached: &CachedManifest,
    port: u16,
    plugins_dir: &Path,
) -> Result<(SpawnedPlugin, ManagedPlugin), (String, OrbflowError)> {
    let map_err = |e: OrbflowError| (name.to_string(), e);

    let language = cached.manifest.language.as_deref().unwrap_or("python");

    let plugin_dir = &cached.install_path;

    validate_path_containment(plugin_dir, plugins_dir).map_err(map_err)?;

    let (program, args) = build_launch_command(language, plugin_dir, port)
        .ok_or_else(|| {
            OrbflowError::Internal(format!(
                "unsupported plugin language '{language}' for '{name}'"
            ))
        })
        .map_err(map_err)?;

    tracing::info!(
        plugin = %name,
        port,
        cmd = %program,
        "spawning gRPC plugin process"
    );

    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .current_dir(plugin_dir)
        .env("ORBFLOW_PLUGIN_PORT", port.to_string());

    // Strip sensitive env vars to prevent credential leakage.
    for key in DENIED_ENV_VARS {
        cmd.env_remove(key);
    }

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| OrbflowError::Internal(format!("failed to spawn plugin '{name}': {e}")))
        .map_err(map_err)?;

    // Forward stdout/stderr with plugin name prefix.
    let mut log_handles = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        log_handles.push(forward_output(stdout, name.to_string(), "stdout"));
    }
    if let Some(stderr) = child.stderr.take() {
        log_handles.push(forward_output(stderr, name.to_string(), "stderr"));
    }

    // Wait for the plugin to become healthy.
    let address = format!("http://localhost:{port}");
    wait_for_healthy(&address, name, Duration::from_secs(30))
        .await
        .map_err(map_err)?;

    let sp = SpawnedPlugin {
        name: name.to_string(),
        address,
        port,
    };
    let managed = ManagedPlugin {
        name: name.to_string(),
        child,
        port,
        language: language.to_string(),
        log_handles,
    };

    Ok((sp, managed))
}

// ─── build_launch_command ───────────────────────────────────────────────────

/// Determines the launch command for a plugin based on its language.
///
/// Returns `(program, args)` or `None` if unsupported.
fn build_launch_command(
    language: &str,
    plugin_dir: &Path,
    port: u16,
) -> Option<(String, Vec<String>)> {
    match language {
        "python" => {
            let main_py = plugin_dir.join("main.py");
            if main_py.exists() {
                Some(("python".into(), vec!["main.py".into()]))
            } else {
                // [P3] Validate module name before passing to `python -m`.
                let mod_name = plugin_dir.file_name()?.to_str()?;
                if !validate_module_name(mod_name) {
                    return None;
                }
                Some((
                    "python".into(),
                    vec!["-m".into(), mod_name.into(), "--grpc".into()],
                ))
            }
        }
        "typescript" | "javascript" | "node" => {
            let main_ts = plugin_dir.join("main.ts");
            let main_js = plugin_dir.join("main.js");
            if main_ts.exists() {
                Some(("npx".into(), vec!["tsx".into(), "main.ts".into()]))
            } else if main_js.exists() {
                Some(("node".into(), vec!["main.js".into()]))
            } else {
                None
            }
        }
        _ => {
            let bin_name = plugin_dir.file_name()?.to_str()?;
            if !validate_module_name(bin_name) {
                return None;
            }
            let bin_path = plugin_dir.join(bin_name);
            if bin_path.exists() {
                Some((
                    bin_path.to_str()?.into(),
                    vec!["--port".into(), port.to_string()],
                ))
            } else {
                None
            }
        }
    }
}

// ─── wait_for_healthy ───────────────────────────────────────────────────────

/// Polls the gRPC HealthCheck endpoint until it succeeds or times out.
///
/// Uses `connect_lazy` to create a single channel that reconnects
/// automatically, rather than creating a new TCP connection per poll tick.
async fn wait_for_healthy(
    address: &str,
    name: &str,
    timeout: Duration,
) -> Result<(), OrbflowError> {
    use crate::grpc_proto::orbflow_plugin_client::OrbflowPluginClient;
    use tonic::transport::Channel;

    let endpoint = Channel::from_shared(address.to_string())
        .map_err(|e| OrbflowError::Internal(format!("invalid plugin address: {e}")))?
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(5));

    let channel = endpoint.connect_lazy();
    let mut client = OrbflowPluginClient::new(channel);

    let deadline = tokio::time::Instant::now() + timeout;
    let mut last_err = String::new();

    loop {
        if tokio::time::Instant::now() > deadline {
            return Err(OrbflowError::Internal(format!(
                "plugin {name}: health check timed out after {timeout:?} (last error: {last_err})"
            )));
        }

        match client
            .health_check(tonic::Request::new(
                crate::grpc_proto::HealthCheckRequest {},
            ))
            .await
        {
            Ok(resp) => {
                if resp.into_inner().healthy {
                    return Ok(());
                }
                last_err = "reported unhealthy".into();
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

// ─── ManagedPlugins — PluginManager port adapter ────────────────────────────

/// Newtype wrapper that implements the [`orbflow_core::ports::PluginManager`]
/// port trait for [`PluginProcessManager`].
///
/// The port trait requires `&self` while the concrete manager requires
/// `&mut self` for state-mutating operations. This wrapper internalises a
/// `Mutex` so callers interact with a uniform `&self` interface.
pub struct ManagedPlugins(tokio::sync::Mutex<PluginProcessManager>);

impl ManagedPlugins {
    /// Wraps a [`PluginProcessManager`] in a `ManagedPlugins` adapter.
    pub fn new(pm: PluginProcessManager) -> Self {
        Self(tokio::sync::Mutex::new(pm))
    }
}

#[async_trait::async_trait]
impl orbflow_core::ports::PluginManager for ManagedPlugins {
    async fn list_plugins(
        &self,
    ) -> Result<Vec<orbflow_core::ports::PluginInfo>, orbflow_core::OrbflowError> {
        let guard = self.0.lock().await;
        Ok(guard
            .list_status()
            .into_iter()
            .map(|s| orbflow_core::ports::PluginInfo {
                name: s.name,
                version: None,
                status: format!("{:?}", s.status),
                address: s.address,
            })
            .collect())
    }

    async fn get_plugin(
        &self,
        name: &str,
    ) -> Result<orbflow_core::ports::PluginInfo, orbflow_core::OrbflowError> {
        let guard = self.0.lock().await;
        guard
            .get_status(name)
            .map(|s| orbflow_core::ports::PluginInfo {
                name: s.name,
                version: None,
                status: format!("{:?}", s.status),
                address: s.address,
            })
            .ok_or(orbflow_core::OrbflowError::NotFound)
    }

    async fn start_plugin(
        &self,
        name: &str,
    ) -> Result<orbflow_core::ports::PluginInfo, orbflow_core::OrbflowError> {
        let mut guard = self.0.lock().await;
        let sp = guard.start_plugin(name, None).await?;
        Ok(orbflow_core::ports::PluginInfo {
            name: sp.name,
            version: None,
            status: "running".into(),
            address: Some(sp.address),
        })
    }

    async fn stop_plugin(&self, name: &str) -> Result<(), orbflow_core::OrbflowError> {
        let mut guard = self.0.lock().await;
        guard.stop_plugin(name).await
    }

    async fn reload_all(
        &self,
    ) -> Result<Vec<orbflow_core::ports::PluginInfo>, orbflow_core::OrbflowError> {
        let mut guard = self.0.lock().await;
        let spawned = guard.reload_all(&std::collections::HashMap::new()).await;
        Ok(spawned
            .into_iter()
            .map(|sp| orbflow_core::ports::PluginInfo {
                name: sp.name,
                version: None,
                status: "running".into(),
                address: Some(sp.address),
            })
            .collect())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plugin_name_valid() {
        assert!(validate_plugin_name("orbflow-ai-codegen").is_ok());
        assert!(validate_plugin_name("my_plugin_123").is_ok());
        assert!(validate_plugin_name("a").is_ok());
    }

    #[test]
    fn test_validate_plugin_name_invalid() {
        assert!(validate_plugin_name("").is_err());
        assert!(validate_plugin_name("../evil").is_err());
        assert!(validate_plugin_name("a b").is_err());
        assert!(validate_plugin_name("foo;bar").is_err());
        let long_name = "a".repeat(65);
        assert!(validate_plugin_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_module_name() {
        assert!(validate_module_name("orbflow_ai_codegen"));
        assert!(validate_module_name("my-plugin"));
        assert!(validate_module_name("_private"));
        assert!(!validate_module_name(""));
        assert!(!validate_module_name("-starts-with-dash"));
        assert!(!validate_module_name("has space"));
        assert!(!validate_module_name(".."));
    }

    #[test]
    fn test_build_launch_command_python() {
        let dir = std::env::temp_dir().join("orbflow-test-launch-py");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("main.py"), "# test").unwrap();

        let (prog, args) = build_launch_command("python", &dir, 50051).unwrap();
        assert_eq!(prog, "python");
        assert_eq!(args, vec!["main.py"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_build_launch_command_node_js() {
        let dir = std::env::temp_dir().join("orbflow-test-launch-js");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("main.js"), "// test").unwrap();

        let (prog, args) = build_launch_command("javascript", &dir, 50051).unwrap();
        assert_eq!(prog, "node");
        assert_eq!(args, vec!["main.js"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_build_launch_command_node_ts() {
        let dir = std::env::temp_dir().join("orbflow-test-launch-ts");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("main.ts"), "// test").unwrap();

        let (prog, args) = build_launch_command("typescript", &dir, 50051).unwrap();
        assert_eq!(prog, "npx");
        assert_eq!(args, vec!["tsx", "main.ts"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_build_launch_command_unsupported() {
        let dir = std::env::temp_dir().join("orbflow-test-launch-unsupported");
        let _ = std::fs::create_dir_all(&dir);

        let result = build_launch_command("ruby", &dir, 50051);
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_process_manager_default() {
        let pm = PluginProcessManager::default();
        assert!(pm.managed.is_empty());
        assert!(pm.manifests.is_empty());
    }

    #[test]
    fn test_list_status_empty() {
        let pm = PluginProcessManager::new("./plugins");
        let statuses = pm.list_status();
        assert!(statuses.is_empty());
    }
}
