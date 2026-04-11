// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Orbflow worker binary (task executor).
//!
//! Connects to NATS, registers built-in node executors and external plugins,
//! then processes tasks dispatched by the engine.

use std::collections::HashMap;
use std::sync::Arc;

use orbflow_builtins::AiChatNode;
use orbflow_builtins::register_builtins_with;
use orbflow_config::{Config, init_tracing_with_config};
use orbflow_core::ports::{Bus, NodeExecutor};
use orbflow_core::streaming::StreamingNodeExecutor;
use orbflow_natsbus::NatsBus;
use orbflow_plugin::loader::GrpcPluginEndpoint;
use orbflow_plugin::{PluginLoader, PluginProcessManager};
use orbflow_registry::index::LocalIndex;
use orbflow_worker::{Worker, WorkerOptions};

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

    // Initialize tracing.
    init_tracing_with_config(&cfg.log);

    // --- NATS ---
    let bus = match NatsBus::connect(&cfg.nats.url).await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            tracing::error!("failed to connect to NATS: {e}");
            std::process::exit(1);
        }
    };

    // --- Worker ---
    let worker_opts = WorkerOptions::new().pool_name(&cfg.worker.pool);

    let mut worker = Worker::new(bus.clone() as Arc<dyn Bus>, worker_opts);

    // Register built-in nodes (executor + schema).
    {
        let registrations = std::cell::RefCell::new(Vec::<(String, Arc<dyn NodeExecutor>)>::new());
        register_builtins_with(|name: &str, executor: Arc<dyn NodeExecutor>, _schema| {
            registrations.borrow_mut().push((name.to_owned(), executor));
        });
        for (name, executor) in registrations.into_inner() {
            worker.register_node(&name, executor);
        }
    }

    // Register streaming executors for AI nodes (real-time token output).
    worker.register_streaming(
        "builtin:ai-chat",
        Arc::new(AiChatNode) as Arc<dyn StreamingNodeExecutor>,
    );

    // --- Load external plugins (subprocess) ---
    let mut plugin_loader = PluginLoader::new(&cfg.plugins.dir);
    if let Err(e) = plugin_loader.discover().await {
        tracing::warn!("subprocess plugin discovery failed: {e}");
    } else {
        // Scan manifests to register under plugin_ref names (e.g. "plugin:csv-reader").
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

        plugin_loader.register_all(|name, exec| {
            if let Some(m) = manifest_by_pkg.get(name) {
                for node_type in &m.node_types {
                    worker.register_node(node_type, exec.clone());
                }
            } else {
                worker.register_node(name, exec);
            }
        });
    }

    // --- Auto-start and discover gRPC plugins ---
    // 1. Spawn plugin processes from manifests declaring PluginProtocol::Grpc
    // 2. Wait for each to become healthy (HealthCheck RPC)
    // 3. Merge with explicit YAML overrides for address/timeout
    // 4. Run gRPC discovery (GetSchemas) and register executors
    let mut process_manager = PluginProcessManager::new(&cfg.plugins.dir);
    {
        use orbflow_plugin::process_manager::PluginSpawnOverride;

        // Build spawn overrides from YAML config (e.g. custom port).
        let mut overrides: HashMap<String, PluginSpawnOverride> = HashMap::new();
        for g in &cfg.plugins.grpc {
            // Extract port from address like "http://localhost:50051".
            match g
                .address
                .rsplit(':')
                .next()
                .and_then(|p| p.parse::<u16>().ok())
            {
                Some(port) => {
                    overrides.insert(g.name.clone(), PluginSpawnOverride { port: Some(port) });
                }
                None => {
                    tracing::warn!(
                        plugin = %g.name,
                        address = %g.address,
                        "could not extract port from gRPC plugin address, using manifest default"
                    );
                }
            }
        }

        let spawned = process_manager.spawn_all(&overrides).await;

        // Build gRPC endpoints: spawned plugins + explicit YAML entries.
        let mut endpoints_by_name: HashMap<String, GrpcPluginEndpoint> = HashMap::new();

        for sp in &spawned {
            endpoints_by_name.insert(
                sp.name.clone(),
                GrpcPluginEndpoint {
                    name: sp.name.clone(),
                    address: sp.address.clone(),
                    timeout_secs: 30,
                },
            );
        }

        // Explicit YAML config overrides auto-spawned entries.
        for g in &cfg.plugins.grpc {
            endpoints_by_name.insert(
                g.name.clone(),
                GrpcPluginEndpoint {
                    name: g.name.clone(),
                    address: g.address.clone(),
                    timeout_secs: g.timeout_secs,
                },
            );
        }

        let endpoints: Vec<GrpcPluginEndpoint> = endpoints_by_name.into_values().collect();

        if !endpoints.is_empty() {
            tracing::info!(
                count = endpoints.len(),
                names = ?endpoints.iter().map(|e| &e.name).collect::<Vec<_>>(),
                "discovering gRPC plugins"
            );

            match plugin_loader.discover_grpc(&endpoints).await {
                Ok(schemas) => {
                    plugin_loader.register_grpc(|name, exec| {
                        worker.register_node(name, exec);
                    });
                    tracing::info!(
                        count = schemas.len(),
                        "gRPC plugin schemas available for node-types API"
                    );
                }
                Err(e) => {
                    tracing::warn!("gRPC plugin discovery failed: {e}");
                }
            }
        }
    }

    // Wrap process_manager in Arc<Mutex> so it can be shared with the reload handler.
    let process_manager = Arc::new(tokio::sync::Mutex::new(process_manager));

    // --- Start worker ---
    let worker = Arc::new(worker);
    let worker_clone = worker.clone();
    tokio::spawn(async move {
        if let Err(e) = worker_clone.start().await {
            tracing::error!("worker error: {e}");
        }
    });

    // --- Subscribe to plugin reload notifications ---
    // When the server installs a new plugin it publishes to this subject.
    // The worker re-scans the plugins directory and registers new executors
    // for both subprocess and gRPC plugins.
    {
        let reload_subject = orbflow_core::plugin_reload_subject();
        let worker_for_reload = Arc::clone(&worker);
        let pm_for_reload = Arc::clone(&process_manager);
        let plugins_dir = cfg.plugins.dir.clone();
        let handler: orbflow_core::MsgHandler = Arc::new(move |_subject, _data| {
            let worker = Arc::clone(&worker_for_reload);
            let pm = Arc::clone(&pm_for_reload);
            let dir = plugins_dir.clone();
            Box::pin(async move {
                tracing::info!("received plugin reload signal, re-discovering plugins");
                reload_worker_plugins(&worker, &pm, &dir).await;
                Ok(())
            })
        });
        if let Err(e) = bus.subscribe(&reload_subject, handler).await {
            tracing::warn!(error = %e, "failed to subscribe to plugin reload subject");
        } else {
            tracing::info!(subject = %reload_subject, "subscribed to plugin reload notifications");
        }
    }

    // --- Graceful shutdown ---
    wait_for_shutdown().await;
    tracing::info!("shutting down worker...");

    if let Err(e) = worker.stop().await {
        tracing::warn!("worker stop error: {e}");
    }

    if let Err(e) = bus.close().await {
        tracing::warn!("bus close error: {e}");
    }

    plugin_loader.close();

    // Stop managed plugin processes.
    process_manager.lock().await.shutdown().await;

    tracing::info!("worker stopped");
}

/// Re-scans the plugins directory and registers any newly discovered plugins
/// with the worker's executor registry at runtime.
///
/// Called when the server publishes a reload signal on NATS after a marketplace
/// install. Handles both subprocess and gRPC plugins. Only registers plugins
/// whose node_types are not already registered, so existing executors are
/// unaffected.
async fn reload_worker_plugins(
    worker: &Worker,
    process_manager: &tokio::sync::Mutex<PluginProcessManager>,
    plugins_dir: &str,
) {
    let mut loader = PluginLoader::new(plugins_dir);
    if let Err(e) = loader.discover().await {
        tracing::warn!(error = %e, "plugin re-discovery failed during reload");
        return;
    }

    let mut index = LocalIndex::new(plugins_dir);
    if let Err(e) = index.scan() {
        tracing::warn!(error = %e, "plugin manifest scan failed during reload");
        return;
    }

    let manifests = index.list();
    let mut manifest_by_pkg: HashMap<String, &orbflow_registry::manifest::PluginManifest> =
        HashMap::new();
    for plugin in &manifests {
        manifest_by_pkg.insert(plugin.manifest.name.clone(), &plugin.manifest);
    }

    let mut registered = 0usize;

    // --- Subprocess plugins ---
    loader.register_all(|name, exec| {
        if let Some(m) = manifest_by_pkg.get(name) {
            for node_type in &m.node_types {
                if !worker.has_executor(node_type) {
                    worker.register_node_dynamic(node_type, exec.clone());
                    tracing::info!(plugin_ref = %node_type, "hot-registered subprocess plugin executor");
                    registered += 1;
                }
            }
        } else if !worker.has_executor(name) {
            worker.register_node_dynamic(name, exec);
            tracing::info!(plugin_ref = %name, "hot-registered subprocess plugin executor");
            registered += 1;
        }
    });

    // --- gRPC plugins ---
    // Spawn any new gRPC plugin processes that aren't already running,
    // then discover their node types and register executors.
    {
        let mut pm = process_manager.lock().await;
        let spawned = pm.reload_all(&HashMap::new()).await;

        if !spawned.is_empty() {
            let endpoints: Vec<GrpcPluginEndpoint> = spawned
                .iter()
                .map(|sp| GrpcPluginEndpoint {
                    name: sp.name.clone(),
                    address: sp.address.clone(),
                    timeout_secs: 30,
                })
                .collect();

            tracing::info!(
                count = endpoints.len(),
                names = ?endpoints.iter().map(|e| &e.name).collect::<Vec<_>>(),
                "discovering newly spawned gRPC plugins"
            );

            match loader.discover_grpc(&endpoints).await {
                Ok(schemas) => {
                    loader.register_grpc(|name, exec| {
                        if !worker.has_executor(name) {
                            worker.register_node_dynamic(name, exec);
                            tracing::info!(plugin_ref = %name, "hot-registered gRPC plugin executor");
                            registered += 1;
                        }
                    });
                    tracing::info!(
                        count = schemas.len(),
                        "gRPC plugin schemas discovered during reload"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "gRPC plugin discovery failed during reload");
                }
            }
        }
    }

    if registered > 0 {
        tracing::info!(
            count = registered,
            "plugin reload complete — new executors registered"
        );
    } else {
        tracing::info!("plugin reload complete — no new executors needed");
    }
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
