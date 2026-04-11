// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Orbflow server binary (coordinator + API).
//!
//! Wires together: config, Postgres store, NATS bus, engine, builtins,
//! plugins, triggers, HTTP API (Axum), optional gRPC, graceful shutdown.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use orbflow_builtins::register_builtins;
use orbflow_config::{Config, init_tracing_with_otel};
use orbflow_core::options::EngineOptionsBuilder;
use orbflow_core::ports::{
    AlertStore, Bus, ChangeRequestStore, CredentialStore, Engine, NodeSchema, PluginIndex,
    PluginManager, RbacStore, Store,
};
use orbflow_engine::{OrbflowEngine, SUB_WORKFLOW_PLUGIN_REF, SubWorkflowExecutor};
use orbflow_grpcapi::GrpcServer;
use orbflow_httpapi::{HttpApiOptions, create_router};
use orbflow_natsbus::NatsBus;
use orbflow_plugin::{ManagedPlugins, PluginLoader, PluginProcessManager};
use orbflow_postgres::{PgStore, PgStoreOptions};
use orbflow_registry::client::CommunityIndex;
use orbflow_registry::index::LocalIndex;
use orbflow_registry::merged::MergedIndex;
use orbflow_trigger::TriggerManager;

#[tokio::main]
async fn main() {
    // Load .env file if present (silently ignore if missing).
    let _ = dotenvy::dotenv();

    // Parse command-line args.
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "orbflow.yaml".into());

    // Load configuration.
    let cfg = match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    // Initialize tracing with optional OpenTelemetry export.
    // The guard must be held for the lifetime of the process to keep the OTel
    // pipeline alive and flush pending telemetry on shutdown.
    let _otel_guard = init_tracing_with_otel(&cfg.log, &cfg.otel);

    // --- Database ---
    let mut store_opts = PgStoreOptions::default();
    if !cfg.credentials.encryption_key.is_empty() {
        let key = decode_hex_key(&cfg.credentials.encryption_key);
        store_opts.encryption_key = Some(key);
    }
    let has_encryption = store_opts.encryption_key.is_some();

    let store = match PgStore::new(&cfg.database.dsn, store_opts).await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            tracing::error!("failed to connect to database: {e}");
            std::process::exit(1);
        }
    };

    tracing::info!("database migrations applied");

    // --- NATS ---
    let bus = match NatsBus::connect(&cfg.nats.url).await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            tracing::error!("failed to connect to NATS: {e}");
            std::process::exit(1);
        }
    };

    // --- Engine ---
    let credential_store: Option<Arc<dyn CredentialStore>> = if has_encryption {
        Some(store.clone() as Arc<dyn CredentialStore>)
    } else {
        None
    };

    let mut opts_builder = EngineOptionsBuilder::new()
        .store(store.clone() as Arc<dyn Store>)
        .bus(bus.clone() as Arc<dyn Bus>)
        .pool_name(&cfg.worker.pool)
        .enable_resume(true);

    if let Some(ref cs) = credential_store {
        opts_builder = opts_builder.credential_store(cs.clone() as Arc<dyn CredentialStore>);
    }
    opts_builder = opts_builder.metrics_store(store.clone() as Arc<dyn orbflow_core::MetricsStore>);
    opts_builder = opts_builder.budget_store(store.clone() as Arc<dyn orbflow_core::BudgetStore>);

    let opts = match opts_builder.build() {
        Ok(opts) => opts,
        Err(e) => {
            tracing::error!(error = %e, "invalid engine configuration");
            std::process::exit(1);
        }
    };
    let engine = Arc::new(OrbflowEngine::new(opts));

    // --- Register built-in nodes ---
    if let Err(e) = register_builtins(engine.as_ref()) {
        tracing::warn!("failed to register built-in nodes: {e}");
    }

    // Register sub-workflow executor.
    if let Err(e) = engine.register_node(
        SUB_WORKFLOW_PLUGIN_REF,
        Arc::new(SubWorkflowExecutor::new(engine.clone() as Arc<dyn Engine>)),
    ) {
        tracing::error!(error = %e, "failed to register sub-workflow executor");
        std::process::exit(1);
    }

    // --- Clean up partial plugin installations from previous crashes ---
    // P9 fix: wrap blocking I/O in spawn_blocking to avoid stalling the Tokio runtime.
    {
        let dir = cfg.plugins.dir.clone();
        let _ = tokio::task::spawn_blocking(move || {
            orbflow_registry::install::cleanup_partial_installs(std::path::Path::new(&dir));
        })
        .await;
    }

    // --- Load external plugins ---
    let plugin_loader = load_plugins(&engine, &cfg).await;

    // --- Start engine (AFTER all plugin/schema registration is complete) ---
    engine.set_self_ref();
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        if let Err(e) = engine_clone.start().await {
            tracing::error!("engine error: {e}");
        }
    });

    // --- Trigger manager ---
    let trigger_mgr = match TriggerManager::new(
        engine.clone() as Arc<dyn Engine>,
        store.clone() as Arc<dyn orbflow_core::ports::WorkflowStore>,
    )
    .await
    {
        Ok(mgr) => Arc::new(tokio::sync::Mutex::new(mgr)),
        Err(e) => {
            tracing::warn!("trigger manager creation failed: {e}");
            // Continue without triggers.
            start_http_only(engine.clone(), credential_store, &cfg, bus, store).await;
            return;
        }
    };

    let trigger_mgr_clone = trigger_mgr.clone();
    tokio::spawn(async move {
        if let Err(e) = trigger_mgr_clone.lock().await.start().await {
            tracing::warn!("trigger manager start failed: {e}");
        }
    });

    // --- HTTP server ---
    if cfg.server.auth_token.is_some() {
        tracing::info!("API bearer token authentication enabled");
    } else {
        tracing::warn!(
            "No auth_token configured — all API endpoints are unauthenticated. Set server.auth_token in config to enable authentication."
        );
    }
    // --- RBAC policy ---
    let rbac_store: Arc<dyn RbacStore> = store.clone() as Arc<dyn RbacStore>;
    let rbac_policy_arc = setup_rbac(&rbac_store).await;

    let (plugin_index, plugin_installer) = build_plugin_index(&cfg.plugins.dir);
    let plugin_manager = build_plugin_manager(&cfg.plugins.dir);

    let http_opts = build_http_options(
        engine.clone(),
        credential_store.clone(),
        &cfg,
        &bus,
        &store,
        rbac_policy_arc,
        Some(rbac_store),
        plugin_index,
        plugin_installer,
        Some(plugin_manager.clone()),
    );

    let app = create_router(http_opts).unwrap_or_else(|e| {
        tracing::error!("failed to create HTTP router: {e}");
        std::process::exit(1);
    });
    let app = app.merge(trigger_mgr.lock().await.webhook_router());

    let http_addr = format!("{}:{}", cfg.server.host, cfg.server.port);

    let listener = match tokio::net::TcpListener::bind(&http_addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind HTTP server to {http_addr}: {e}");
            std::process::exit(1);
        }
    };

    tracing::info!("HTTP server starting on {http_addr}");

    let http_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("HTTP server error: {e}");
        }
    });

    // --- Optional gRPC server ---
    let grpc_server = if cfg.grpc.enabled {
        if cfg.server.auth_token.is_some() {
            tracing::info!("gRPC bearer token authentication enabled");
        } else {
            tracing::warn!(
                "gRPC server is unauthenticated — set server.auth_token to secure gRPC endpoints"
            );
        }
        // Warn if gRPC binds to a non-loopback address without TLS.
        if cfg.server.host != "127.0.0.1" && cfg.server.host != "localhost" {
            tracing::warn!(
                host = %cfg.server.host,
                "gRPC server binds to a non-loopback address over plaintext TCP. \
                 Auth tokens and workflow data will be transmitted unencrypted. \
                 Consider using TLS or restricting to 127.0.0.1."
            );
        }
        let grpc = Arc::new(GrpcServer::new(
            engine.clone() as Arc<dyn Engine>,
            cfg.server.auth_token.clone(),
        ));
        let grpc_addr = format!("{}:{}", cfg.server.host, cfg.grpc.port);
        let grpc_clone = grpc.clone();
        tokio::spawn(async move {
            if let Err(e) = grpc_clone.serve(&grpc_addr).await {
                tracing::error!("gRPC server error: {e}");
            }
        });
        Some(grpc)
    } else {
        None
    };

    // --- Graceful shutdown ---
    wait_for_shutdown().await;
    tracing::info!("shutting down...");

    if let Some(grpc) = grpc_server {
        grpc.stop();
    }

    trigger_mgr.lock().await.stop().await;
    drop(http_handle);

    if let Err(e) = engine.stop().await {
        tracing::warn!("engine stop error: {e}");
    }

    if let Err(e) = bus.close().await {
        tracing::warn!("bus close error: {e}");
    }

    store.close().await;
    plugin_loader.close();

    tracing::info!("server stopped");
}

/// Fallback: start HTTP-only (when trigger manager fails to create).
async fn start_http_only(
    engine: Arc<OrbflowEngine>,
    credential_store: Option<Arc<dyn CredentialStore>>,
    cfg: &Config,
    bus: Arc<NatsBus>,
    store: Arc<PgStore>,
) {
    let (plugin_index, plugin_installer) = build_plugin_index(&cfg.plugins.dir);
    let plugin_manager = build_plugin_manager(&cfg.plugins.dir);

    let http_opts = build_http_options(
        engine.clone(),
        credential_store,
        cfg,
        &bus,
        &store,
        None,
        Some(store.clone() as Arc<dyn RbacStore>),
        plugin_index,
        plugin_installer,
        Some(plugin_manager),
    );

    let app = create_router(http_opts).unwrap_or_else(|e| {
        tracing::error!("failed to create HTTP router: {e}");
        std::process::exit(1);
    });
    let http_addr = format!("{}:{}", cfg.server.host, cfg.server.port);

    let listener = match tokio::net::TcpListener::bind(&http_addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind HTTP: {e}");
            std::process::exit(1);
        }
    };

    tracing::info!("HTTP server starting on {http_addr}");

    let http_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("HTTP server error: {e}");
        }
    });

    wait_for_shutdown().await;
    tracing::info!("shutting down...");

    drop(http_handle);
    if let Err(e) = engine.stop().await {
        tracing::warn!("engine stop error: {e}");
    }
    if let Err(e) = bus.close().await {
        tracing::warn!("bus close error: {e}");
    }
    store.close().await;
    tracing::info!("server stopped");
}

/// Constructs [`HttpApiOptions`] from shared dependencies.
///
/// Centralises the wiring of engine, stores, and config into a single place
/// so that both the normal startup path and the fallback `start_http_only`
/// path share identical construction logic.
#[allow(clippy::too_many_arguments)]
fn build_http_options(
    engine: Arc<OrbflowEngine>,
    credential_store: Option<Arc<dyn CredentialStore>>,
    cfg: &Config,
    bus: &Arc<NatsBus>,
    store: &Arc<PgStore>,
    rbac: Option<Arc<RwLock<orbflow_core::rbac::RbacPolicy>>>,
    rbac_store: Option<Arc<dyn RbacStore>>,
    plugin_index: Option<Arc<dyn PluginIndex>>,
    plugin_installer: Option<Arc<dyn orbflow_core::PluginInstaller>>,
    plugin_manager: Option<Arc<dyn PluginManager>>,
) -> HttpApiOptions {
    let cred = match &credential_store {
        Some(cs) => {
            tracing::info!("credential store enabled");
            Some(cs.clone())
        }
        None => {
            tracing::warn!("credential store disabled: no encryption key configured");
            None
        }
    };

    HttpApiOptions {
        engine: engine as Arc<dyn Engine>,
        credential_store: cred,
        bus: Some(bus.clone() as Arc<dyn orbflow_core::Bus>),
        metrics_store: Some(store.clone() as Arc<dyn orbflow_core::MetricsStore>),
        auth_token: cfg.server.auth_token.clone(),
        rbac,
        rbac_store,
        plugin_index,
        plugin_installer,
        change_request_store: Some(store.clone() as Arc<dyn ChangeRequestStore>),
        budget_store: Some(store.clone() as Arc<dyn orbflow_core::BudgetStore>),
        analytics_store: Some(store.clone() as Arc<dyn orbflow_core::AnalyticsStore>),
        alert_store: Some(store.clone() as Arc<dyn AlertStore>),
        trust_x_user_id: false,
        bootstrap_admin: std::env::var("ORBFLOW_BOOTSTRAP_ADMIN").ok().and_then(|v| {
            if v == "anonymous" {
                tracing::error!("ORBFLOW_BOOTSTRAP_ADMIN cannot be 'anonymous' — ignoring");
                None
            } else {
                Some(v)
            }
        }),
        plugin_manager,
        plugins_dir: Some(cfg.plugins.dir.clone()),
        cors_origins: cfg.server.cors_origins.clone(),
        rate_limit: cfg.server.rate_limit.clone(),
    }
}

/// Loads the RBAC policy from the database and spawns a background reload task.
///
/// Returns `Some(policy)` when roles or bindings exist, `None` otherwise.
/// The background task refreshes the policy every 30 seconds so that changes
/// made by other server instances are picked up without a restart.
async fn setup_rbac(
    rbac_store: &Arc<dyn RbacStore>,
) -> Option<Arc<RwLock<orbflow_core::rbac::RbacPolicy>>> {
    let rbac_policy_arc = match rbac_store.load_policy().await {
        Ok(policy) if !policy.roles.is_empty() || !policy.bindings.is_empty() => {
            tracing::info!(
                roles = policy.roles.len(),
                bindings = policy.bindings.len(),
                "RBAC policy loaded from database"
            );
            Some(Arc::new(RwLock::new(policy)))
        }
        Ok(_) => {
            tracing::info!(
                "RBAC not configured (no roles or bindings in database) — all requests allowed"
            );
            None
        }
        Err(e) => {
            tracing::warn!("failed to load RBAC policy, RBAC disabled: {e}");
            None
        }
    };

    if let Some(ref rbac_policy) = rbac_policy_arc {
        let policy = rbac_policy.clone();
        let reload_store = rbac_store.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                match reload_store.load_policy().await {
                    Ok(new_policy) => {
                        if let Ok(mut guard) = policy.write() {
                            *guard = new_policy;
                            tracing::debug!("RBAC policy reloaded from store");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("failed to reload RBAC policy: {e}");
                    }
                }
            }
        });
    }

    rbac_policy_arc
}

/// Discovers and registers external plugins with the engine.
///
/// Scans the plugin directory for subprocess executors and manifest files,
/// registers node executors and schemas, and also registers schemas for
/// gRPC-only plugins that have manifests but no local executable.
async fn load_plugins(engine: &Arc<OrbflowEngine>, cfg: &Config) -> PluginLoader {
    let mut plugin_loader = PluginLoader::new(&cfg.plugins.dir);
    if let Err(e) = plugin_loader.discover().await {
        tracing::warn!("plugin discovery failed: {e}");
    } else {
        let mut plugin_manifest_index = LocalIndex::new(&cfg.plugins.dir);
        if let Err(e) = plugin_manifest_index.scan() {
            tracing::warn!("failed to scan plugin manifests: {e}");
        }

        let manifests = plugin_manifest_index.list();
        let mut manifest_by_pkg: HashMap<String, &orbflow_registry::manifest::PluginManifest> =
            HashMap::new();
        for plugin in &manifests {
            manifest_by_pkg.insert(plugin.manifest.name.clone(), &plugin.manifest);
        }
        tracing::info!(
            count = manifest_by_pkg.len(),
            "plugin schemas loaded from manifests"
        );

        let engine_ref = engine.clone();
        plugin_loader.register_all(|name, exec| {
            if let Some(m) = manifest_by_pkg.get(name) {
                for node_type in &m.node_types {
                    let schema = schema_from_manifest(node_type, m);
                    let _ = engine_ref.register_node_with_schema(node_type, exec.clone(), schema);
                }
            } else {
                let _ = engine_ref.register_node(name, exec);
            }
        });

        let registered_refs: std::collections::HashSet<String> =
            engine.node_schema_refs().into_iter().collect();

        for plugin in &manifests {
            let m = &plugin.manifest;
            for node_type in &m.node_types {
                if registered_refs.contains(node_type) {
                    continue;
                }
                let schema = schema_from_manifest(node_type, m);
                engine.register_schema(node_type, schema);
                tracing::info!(plugin_ref = %node_type, "registered gRPC plugin schema from manifest");
            }
        }
    }

    plugin_loader
}

/// Scans the plugin directory and returns a merged index (local + community)
/// for the marketplace API.
///
/// The community index URL defaults to the orbflow-dev GitHub repository but
/// can be overridden via the `ORBFLOW_PLUGIN_INDEX_URL` environment variable
/// for air-gapped or enterprise deployments.
#[allow(clippy::type_complexity)]
fn build_plugin_index(
    dir: &str,
) -> (
    Option<Arc<dyn PluginIndex>>,
    Option<Arc<dyn orbflow_core::PluginInstaller>>,
) {
    // D8 fix: construct one shared HTTP client for all registry operations.
    let http_client = orbflow_registry::HttpClient::new();

    let mut local = LocalIndex::new(dir);
    if let Err(e) = local.scan() {
        tracing::warn!("failed to scan local plugin index: {e}");
    }
    let local: Arc<dyn PluginIndex> = Arc::new(local);

    let community: Arc<dyn PluginIndex> = match std::env::var("ORBFLOW_PLUGIN_INDEX_URL") {
        Ok(url) => match CommunityIndex::with_url_and_client(&url, http_client.clone()) {
            Ok(c) => {
                tracing::info!(url = %url, "using custom plugin index URL");
                Arc::new(c)
            }
            Err(e) => {
                tracing::warn!(url = %url, error = %e, "invalid custom plugin index URL, using default");
                Arc::new(CommunityIndex::new())
            }
        },
        Err(_) => Arc::new(CommunityIndex::new()),
    };
    let merged = Arc::new(MergedIndex::new(local, community, http_client));
    (
        Some(merged.clone() as Arc<dyn PluginIndex>),
        Some(merged as Arc<dyn orbflow_core::PluginInstaller>),
    )
}

/// Creates a [`ManagedPlugins`] adapter for start/stop/restart via the API.
fn build_plugin_manager(dir: &str) -> Arc<dyn PluginManager> {
    let mut pm = PluginProcessManager::new(dir);
    if let Err(e) = pm.scan() {
        tracing::warn!("failed to scan plugins for process manager: {e}");
    }
    Arc::new(ManagedPlugins::new(pm)) as Arc<dyn PluginManager>
}

/// Builds a [`NodeSchema`] from a plugin manifest for a given node type.
///
/// Extracts display name, field schemas, and presentation hints from the
/// manifest. Logs warnings for malformed field entries instead of silently
/// dropping them.
fn schema_from_manifest(
    node_type: &str,
    m: &orbflow_registry::manifest::PluginManifest,
) -> NodeSchema {
    let display_name = node_type
        .strip_prefix("plugin:")
        .unwrap_or(node_type)
        .split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let parse_fields =
        |values: &[serde_json::Value], kind: &str| -> Vec<orbflow_core::ports::FieldSchema> {
            values
                .iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    serde_json::from_value(v.clone())
                        .map_err(|e| {
                            tracing::warn!(
                                plugin = %m.name,
                                node_type = %node_type,
                                field_index = i,
                                kind = kind,
                                error = %e,
                                "malformed field schema in manifest — skipping"
                            );
                            e
                        })
                        .ok()
                })
                .collect()
        };

    // Validate color: only allow hex colors matching #RGB, #RRGGBB, #RRGGBBAA.
    let color = m
        .display
        .color
        .as_deref()
        .filter(|c| {
            let valid = c.len() >= 4
                && c.len() <= 9
                && c.starts_with('#')
                && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
            if !valid {
                tracing::warn!(
                    plugin = %m.name,
                    color = %c,
                    "invalid color in manifest — using default"
                );
            }
            valid
        })
        .unwrap_or("#6366f1")
        .to_string();

    NodeSchema {
        plugin_ref: node_type.to_string(),
        name: display_name,
        description: m.description.clone(),
        category: m
            .display
            .category
            .clone()
            .unwrap_or_else(|| "plugin".to_string()),
        node_kind: None,
        icon: m
            .display
            .icon
            .clone()
            .unwrap_or_else(|| "puzzle".to_string()),
        color,
        docs: None,
        image_url: None,
        inputs: parse_fields(&m.inputs, "input"),
        outputs: parse_fields(&m.outputs, "output"),
        parameters: parse_fields(&m.parameters, "parameter"),
        capability_ports: vec![],
        settings: vec![],
        provides_capability: None,
    }
}

/// Decodes a hex string to a 32-byte key, exiting on failure.
fn decode_hex_key(hex_str: &str) -> Vec<u8> {
    // Simple hex decode without extra dependency.
    let hex_str = hex_str.trim();
    if hex_str.len() != 64 {
        tracing::error!(
            "invalid credentials encryption key: must be 64 hex chars (32 bytes), got {} chars",
            hex_str.len()
        );
        std::process::exit(1);
    }

    let mut key = Vec::with_capacity(32);
    for i in (0..64).step_by(2) {
        let byte = u8::from_str_radix(&hex_str[i..i + 2], 16).unwrap_or_else(|_| {
            tracing::error!("invalid hex character in encryption key");
            std::process::exit(1);
        });
        key.push(byte);
    }

    key
}

/// Waits for SIGINT or SIGTERM.
async fn wait_for_shutdown() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = ctrl_c => {}
                    _ = sigterm.recv() => {}
                }
            }
            Err(e) => {
                tracing::warn!("failed to register SIGTERM handler: {e} — waiting for SIGINT only");
                ctrl_c.await.ok();
            }
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}
