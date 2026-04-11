// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Tracing / logging initialization with optional OpenTelemetry export.

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::{LogConfig, OtelConfig};

/// Handle returned by [`init_tracing_with_otel`] that keeps OTel providers alive.
///
/// Drop this to flush and shut down the OTel pipeline gracefully.
pub struct OtelGuard {
    _tracer_provider: Option<SdkTracerProvider>,
    _meter_provider: Option<SdkMeterProvider>,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(ref tp) = self._tracer_provider
            && let Err(e) = tp.shutdown()
        {
            eprintln!("otel tracer shutdown error: {e}");
        }
        if let Some(ref mp) = self._meter_provider
            && let Err(e) = mp.shutdown()
        {
            eprintln!("otel meter shutdown error: {e}");
        }
    }
}

/// Initializes the tracing subscriber with sensible defaults (no OTel).
pub fn init_tracing() {
    init_tracing_with_config(&LogConfig::default());
}

/// Initializes the tracing subscriber from the given [`LogConfig`].
pub fn init_tracing_with_config(cfg: &LogConfig) {
    let filter = build_env_filter(cfg);

    match cfg.format.as_str() {
        "console" => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(true)
                .init();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .json()
                .init();
        }
    }
}

/// Initializes tracing with optional OpenTelemetry export.
///
/// When `otel.enabled` is true, this sets up:
/// - OTLP trace exporter (gRPC) connected to `otel.endpoint`
/// - OTLP metrics exporter (gRPC) connected to `otel.endpoint`
/// - `tracing-opentelemetry` layer so all `tracing` spans become OTel spans
///
/// Returns an [`OtelGuard`] that must be held alive for the lifetime of the
/// application. Dropping it flushes pending telemetry.
pub fn init_tracing_with_otel(log: &LogConfig, otel: &OtelConfig) -> OtelGuard {
    if !otel.enabled {
        init_tracing_with_config(log);
        return OtelGuard {
            _tracer_provider: None,
            _meter_provider: None,
        };
    }

    let filter = build_env_filter(log);

    // --- Trace exporter ---
    let trace_exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&otel.endpoint)
        .build()
    {
        Ok(exporter) => exporter,
        Err(e) => {
            eprintln!("failed to create OTLP trace exporter: {e} — falling back to logging-only");
            init_tracing_with_config(log);
            return OtelGuard {
                _tracer_provider: None,
                _meter_provider: None,
            };
        }
    };

    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(trace_exporter)
        .with_resource(otel_resource(&otel.service_name))
        .build();

    let tracer = tracer_provider.tracer("orbflow");

    // --- Metrics exporter ---
    let metrics_exporter = match opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(&otel.endpoint)
        .build()
    {
        Ok(exporter) => exporter,
        Err(e) => {
            eprintln!("failed to create OTLP metrics exporter: {e} — metrics export disabled");
            // Still set up tracing with the tracer provider (traces work, metrics don't).
            setup_tracing_subscriber(log, filter, tracer);
            return OtelGuard {
                _tracer_provider: Some(tracer_provider),
                _meter_provider: None,
            };
        }
    };

    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(metrics_exporter).build();

    let meter_provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(otel_resource(&otel.service_name))
        .build();

    // Register global meter provider so any crate can call
    // `opentelemetry::global::meter("orbflow")`.
    opentelemetry::global::set_meter_provider(meter_provider.clone());

    // --- Compose tracing subscriber ---
    setup_tracing_subscriber(log, filter, tracer);

    tracing::info!(
        endpoint = %otel.endpoint,
        service = %otel.service_name,
        "OpenTelemetry export enabled"
    );

    OtelGuard {
        _tracer_provider: Some(tracer_provider),
        _meter_provider: Some(meter_provider),
    }
}

/// Sets up the tracing subscriber with an OTel layer and a fmt layer.
///
/// The OTel layer is added first (closest to Registry) so it is generic
/// over the subscriber type and works regardless of the fmt layer variant.
fn setup_tracing_subscriber(
    log: &LogConfig,
    filter: EnvFilter,
    tracer: opentelemetry_sdk::trace::Tracer,
) {
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    match log.format.as_str() {
        "console" => {
            tracing_subscriber::registry()
                .with(filter)
                .with(otel_layer)
                .with(tracing_subscriber::fmt::layer().with_target(true))
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(filter)
                .with(otel_layer)
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
    }
}

/// Builds the `EnvFilter` from RUST_LOG or the config level.
///
/// When the configured level is `debug` or `trace`, noisy third-party crates
/// (e.g. `async_nats`, `hyper`, `h2`, `tower`) are capped at `warn` so that
/// orbflow's own debug output remains readable.
fn build_env_filter(cfg: &LogConfig) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = match cfg.level.as_str() {
            "debug" => "debug",
            "warn" => "warn",
            "error" => "error",
            "trace" => "trace",
            _ => "info",
        };

        if matches!(level, "debug" | "trace") {
            // Keep orbflow crates at the requested level, silence noisy deps.
            EnvFilter::new(format!(
                "{level},async_nats=warn,hyper=warn,h2=warn,tower=warn,tonic=warn,rustls=warn"
            ))
        } else {
            EnvFilter::new(level)
        }
    })
}

/// Creates the OTel resource descriptor with service name.
fn otel_resource(service_name: &str) -> Resource {
    Resource::builder()
        .with_attribute(KeyValue::new("service.name", service_name.to_string()))
        .build()
}
