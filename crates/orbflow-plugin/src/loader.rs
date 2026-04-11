// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Plugin loader: discovers plugin binaries from a directory and spawns them
//! as subprocesses communicating via stdin/stdout JSON protocol, and connects
//! to gRPC plugin servers for persistent-connection execution.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::{NodeExecutor, NodeInput, NodeOutput, NodeSchema};

use orbflow_core::validate_plugin_name;

use crate::grpc_client::GrpcPluginExecutor;
use crate::process_manager::DENIED_ENV_VARS;
use crate::protocol::{ExecuteRequest, ExecuteResponse};

/// Timeout for subprocess plugin execution (30 seconds).
const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(30);

/// Discovers and manages external node plugins from a directory.
///
/// Each plugin is an executable file that implements the JSON protocol:
/// read `ExecuteRequest` from stdin, write `ExecuteResponse` to stdout.
pub struct PluginLoader {
    dir: PathBuf,
    plugins: HashMap<String, Arc<dyn NodeExecutor>>,
    /// Tracks which plugin_ref keys were added by `discover_grpc`.
    grpc_keys: HashSet<String>,
}

impl PluginLoader {
    /// Creates a new plugin loader that scans the given directory.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            plugins: HashMap::new(),
            grpc_keys: HashSet::new(),
        }
    }

    /// Discovers plugins by scanning the plugin directory for executables.
    ///
    /// Each file in the directory (excluding hidden files) is treated as a
    /// plugin. The plugin name is the filename without extension.
    pub async fn discover(&mut self) -> Result<(), OrbflowError> {
        if !self.dir.exists() {
            tracing::info!(dir = %self.dir.display(), "plugin directory not found, skipping");
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.dir).map_err(|e| {
            OrbflowError::Internal(format!("plugin: read dir {}: {e}", self.dir.display()))
        })?;

        for entry in entries {
            let entry =
                entry.map_err(|e| OrbflowError::Internal(format!("plugin: read entry: {e}")))?;

            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            // Skip hidden files and non-executable metadata files.
            if name.starts_with('.')
                || name.ends_with(".json")
                || name.ends_with(".toml")
                || name.ends_with(".yaml")
                || name.ends_with(".yml")
                || name.ends_with(".md")
                || name.ends_with(".txt")
            {
                continue;
            }

            let plugin_name = Path::new(&*name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| name.to_string());

            // Validate plugin name to prevent injection via crafted filenames.
            if validate_plugin_name(&plugin_name).is_err() {
                tracing::warn!(
                    plugin = %plugin_name,
                    path = %path.display(),
                    "skipping plugin with invalid name"
                );
                continue;
            }

            tracing::info!(plugin = %plugin_name, path = %path.display(), "discovered plugin");

            let executor = PluginExecutor {
                name: plugin_name.clone(),
                path: path.clone(),
                child: Arc::new(Mutex::new(None)),
            };

            self.plugins.insert(plugin_name, Arc::new(executor));
        }

        tracing::info!(count = self.plugins.len(), "plugins discovered");
        Ok(())
    }

    /// Returns the executor for a given plugin name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn NodeExecutor>> {
        self.plugins.get(name).cloned()
    }

    /// Returns all discovered plugin names and their executors.
    pub fn all(&self) -> &HashMap<String, Arc<dyn NodeExecutor>> {
        &self.plugins
    }

    /// Registers all subprocess plugins with the given callback.
    ///
    /// This only registers plugins found via `discover()`, not gRPC plugins.
    /// Use [`register_grpc`](Self::register_grpc) for gRPC plugins.
    pub fn register_all<F>(&self, mut register: F)
    where
        F: FnMut(&str, Arc<dyn NodeExecutor>),
    {
        for (name, executor) in &self.plugins {
            if !self.grpc_keys.contains(name) {
                register(name, executor.clone());
            }
        }
    }

    /// Registers only gRPC-discovered plugins with the given callback.
    pub fn register_grpc<F>(&self, mut register: F)
    where
        F: FnMut(&str, Arc<dyn NodeExecutor>),
    {
        for key in &self.grpc_keys {
            if let Some(executor) = self.plugins.get(key) {
                register(key, executor.clone());
            }
        }
    }

    /// Discovers gRPC plugins from a list of endpoint configurations.
    ///
    /// Each endpoint is contacted via `GetSchemas` to learn what node types
    /// it provides. A shared [`GrpcPluginExecutor`] is registered for each
    /// `plugin_ref` returned by the server.
    ///
    /// Returns the collected schemas so they can be registered with the engine.
    pub async fn discover_grpc(
        &mut self,
        endpoints: &[GrpcPluginEndpoint],
    ) -> Result<Vec<NodeSchema>, OrbflowError> {
        let mut all_schemas = Vec::new();

        for ep in endpoints {
            let executor = match GrpcPluginExecutor::with_timeout(
                &ep.name,
                &ep.address,
                Duration::from_secs(ep.timeout_secs),
            ) {
                Ok(exec) => Arc::new(exec),
                Err(e) => {
                    tracing::warn!(
                        plugin = %ep.name,
                        address = %ep.address,
                        error = %e,
                        "failed to create gRPC plugin channel (skipping)"
                    );
                    continue;
                }
            };

            match executor.fetch_schemas().await {
                Ok(schemas) => {
                    for schema in &schemas {
                        tracing::info!(
                            plugin = %ep.name,
                            plugin_ref = %schema.plugin_ref,
                            node_name = %schema.name,
                            "registered gRPC plugin node"
                        );
                        self.grpc_keys.insert(schema.plugin_ref.clone());
                        self.plugins
                            .insert(schema.plugin_ref.clone(), executor.clone());
                    }
                    all_schemas.extend(schemas);
                }
                Err(e) => {
                    tracing::warn!(
                        plugin = %ep.name,
                        address = %ep.address,
                        error = %e,
                        "failed to fetch schemas from gRPC plugin (skipping)"
                    );
                }
            }
        }

        tracing::info!(
            grpc_nodes = all_schemas.len(),
            "gRPC plugin discovery complete"
        );

        Ok(all_schemas)
    }

    /// Closes all plugin processes (no-op — processes are spawned per-request).
    pub fn close(&self) {
        tracing::debug!("plugin loader closed");
    }
}

/// Configuration for a single gRPC plugin endpoint.
#[derive(Debug, Clone)]
pub struct GrpcPluginEndpoint {
    /// Human-readable name for logging.
    pub name: String,
    /// gRPC address (e.g. `http://localhost:50051`).
    pub address: String,
    /// RPC timeout in seconds.
    pub timeout_secs: u64,
}

/// Inner state for a persistent subprocess: stdin writer + stdout reader.
///
/// Guarded by a Mutex so concurrent `execute()` calls are serialised per plugin.
struct PersistentChild {
    stdin: tokio::process::ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
}

/// A NodeExecutor that keeps a long-running subprocess alive.
///
/// On first `execute()` the plugin binary is spawned. Subsequent calls reuse
/// the same process, sending newline-delimited JSON requests on stdin and
/// reading one JSON response line from stdout per request. If the process
/// dies, it is automatically respawned on the next call.
struct PluginExecutor {
    name: String,
    path: PathBuf,
    child: Arc<Mutex<Option<PersistentChild>>>,
}

impl PluginExecutor {
    /// Ensure the subprocess is running, spawning it if needed.
    async fn ensure_running(&self) -> Result<(), OrbflowError> {
        let mut guard = self.child.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        tracing::info!(plugin = %self.name, path = %self.path.display(), "spawning persistent subprocess");

        let mut cmd = Command::new(&self.path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Strip sensitive env vars to prevent credential leakage.
        for key in DENIED_ENV_VARS {
            cmd.env_remove(key);
        }

        let mut child = cmd.spawn().map_err(|e| {
            OrbflowError::Internal(format!(
                "plugin {}: spawn {}: {e}",
                self.name,
                self.path.display()
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            OrbflowError::Internal(format!("plugin {}: stdin not available", self.name))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            OrbflowError::Internal(format!("plugin {}: stdout not available", self.name))
        })?;

        // Forward stderr to tracing in the background.
        if let Some(stderr) = child.stderr.take() {
            let name = self.name.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::warn!(plugin = %name, output = %line, "plugin stderr");
                }
            });
        }

        *guard = Some(PersistentChild {
            stdin,
            reader: BufReader::new(stdout),
        });

        Ok(())
    }
}

#[async_trait]
impl NodeExecutor for PluginExecutor {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let request = ExecuteRequest::from(input);
        let mut request_json = serde_json::to_string(&request).map_err(|e| {
            OrbflowError::Internal(format!("plugin {}: serialize request: {e}", self.name))
        })?;
        request_json.push('\n');

        // Retry once if the persistent process died.
        for attempt in 0..2 {
            self.ensure_running().await?;

            let result = tokio::time::timeout(SUBPROCESS_TIMEOUT, async {
                let mut guard = self.child.lock().await;
                let pc = guard.as_mut().ok_or_else(|| {
                    OrbflowError::Internal(format!("plugin {}: process not available", self.name))
                })?;

                pc.stdin
                    .write_all(request_json.as_bytes())
                    .await
                    .map_err(|e| {
                        OrbflowError::Internal(format!("plugin {}: write stdin: {e}", self.name))
                    })?;
                pc.stdin.flush().await.map_err(|e| {
                    OrbflowError::Internal(format!("plugin {}: flush stdin: {e}", self.name))
                })?;

                let mut response_line = String::new();
                pc.reader.read_line(&mut response_line).await.map_err(|e| {
                    OrbflowError::Internal(format!("plugin {}: read stdout: {e}", self.name))
                })?;

                if response_line.trim().is_empty() {
                    return Err(OrbflowError::Internal(format!(
                        "plugin {}: process returned empty response (likely crashed)",
                        self.name
                    )));
                }

                Ok(response_line)
            })
            .await;

            match result {
                Ok(Ok(response_line)) => {
                    let response: ExecuteResponse = serde_json::from_str(response_line.trim())
                        .map_err(|e| {
                            OrbflowError::Internal(format!(
                                "plugin {}: parse response: {e} (raw: {})",
                                self.name,
                                response_line.trim()
                            ))
                        })?;

                    if let Some(err) = &response.error {
                        return Err(OrbflowError::Internal(format!(
                            "plugin {}: {err}",
                            self.name
                        )));
                    }

                    return Ok(NodeOutput::from(response));
                }
                Ok(Err(e)) if attempt == 0 => {
                    tracing::warn!(
                        plugin = %self.name,
                        error = %e,
                        "subprocess died, respawning"
                    );
                    // Clear the dead child so ensure_running respawns.
                    *self.child.lock().await = None;
                    continue;
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    // Timeout — kill the child so it can be respawned.
                    *self.child.lock().await = None;
                    return Err(OrbflowError::Timeout);
                }
            }
        }

        Err(OrbflowError::Internal(format!(
            "plugin {}: exhausted retries",
            self.name
        )))
    }
}
