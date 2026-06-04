// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! NATS JetStream implementation of [`Bus`].

use std::fmt::Write as _;
use std::net::IpAddr;
use std::time::Duration;

use async_nats::jetstream;
use async_nats::jetstream::consumer::PullConsumer;
use async_nats::jetstream::stream::RetentionPolicy;
use async_trait::async_trait;
use tokio::sync::Mutex;

use orbflow_core::SUBJECT_PREFIX;
use orbflow_core::error::OrbflowError;
use orbflow_core::ports::{Bus, MsgHandler};

const STREAM_NAME: &str = "ORBFLOW";
const STREAM_SUBJECT_PREFIX: &str = "orbflow.stream.";
const PLUGIN_RELOAD_SUBJECT: &str = "orbflow.worker.reload-plugins";
const DEFAULT_CONSUMER_ACK_WAIT_SECS: u64 = 330;
const DEFAULT_MAX_ACK_PENDING: i64 = 1024;

/// NATS JetStream implementation of [`orbflow_core::ports::Bus`].
///
/// Uses a WorkQueue retention stream for task/result delivery with explicit
/// ack and 5s NakDelay, matching the Go `natsbus.Bus` implementation. Stream
/// chunk and plugin-reload subjects use transient core NATS pub/sub so each
/// SSE client/worker receives its own copy instead of competing for WorkQueue
/// messages.
pub struct NatsBus {
    client: async_nats::Client,
    jetstream: jetstream::Context,
    stream: Mutex<Option<jetstream::stream::Stream>>,
    subscription_handles: tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
    consumer_ack_wait: Duration,
    consumer_max_ack_pending: i64,
}

impl NatsBus {
    /// Connects to NATS and creates/updates the JetStream stream.
    ///
    /// **Security note**: This connection uses no credentials or TLS.
    /// In production, configure NATS with authentication and TLS to prevent
    /// unauthorized clients from publishing fabricated task results.
    pub async fn connect(url: &str) -> Result<Self, OrbflowError> {
        Self::connect_with_options(url, false).await
    }

    /// Connects to NATS with explicit TLS enforcement option.
    ///
    /// When `require_tls` is `true`, the connection is rejected unless the URL
    /// uses `tls://`.
    pub async fn connect_with_options(url: &str, require_tls: bool) -> Result<Self, OrbflowError> {
        Self::connect_with_consumer_options(
            url,
            require_tls,
            Duration::from_secs(DEFAULT_CONSUMER_ACK_WAIT_SECS),
            DEFAULT_MAX_ACK_PENDING,
        )
        .await
    }

    /// Connects to NATS with explicit consumer delivery safety settings.
    pub async fn connect_with_consumer_options(
        url: &str,
        require_tls: bool,
        consumer_ack_wait: Duration,
        consumer_max_ack_pending: i64,
    ) -> Result<Self, OrbflowError> {
        let consumer_ack_wait = if consumer_ack_wait.is_zero() {
            Duration::from_secs(DEFAULT_CONSUMER_ACK_WAIT_SECS)
        } else {
            consumer_ack_wait
        };
        let consumer_max_ack_pending = consumer_max_ack_pending.max(1);

        let parsed = parse_nats_url(url);
        let is_loopback = parsed.as_ref().is_some_and(|p| p.is_loopback());
        let is_tls = parsed.as_ref().is_some_and(|p| p.scheme == "tls");

        if require_tls && !is_tls {
            return Err(OrbflowError::Bus(
                "NATS require_tls is enabled but URL does not use TLS".into(),
            ));
        }

        if !is_tls && !is_loopback {
            tracing::warn!(
                url = %url,
                "NATS connection uses no authentication or TLS on a non-loopback address. \
                 Any network-reachable client can inject or intercept workflow messages."
            );
        }

        let client = async_nats::connect(url)
            .await
            .map_err(|e| OrbflowError::Bus(format!("natsbus: connect to {url}: {e}")))?;

        let jetstream = jetstream::new(client.clone());

        let stream = jetstream
            .get_or_create_stream(jetstream::stream::Config {
                name: STREAM_NAME.to_owned(),
                subjects: vec![
                    format!("{SUBJECT_PREFIX}.tasks.*"),
                    format!("{SUBJECT_PREFIX}.results.*"),
                ],
                retention: RetentionPolicy::WorkQueue,
                max_age: Duration::from_secs(24 * 60 * 60), // 24h
                ..Default::default()
            })
            .await
            .map_err(|e| OrbflowError::Bus(format!("natsbus: create stream: {e}")))?;

        Ok(Self {
            client,
            jetstream,
            stream: Mutex::new(Some(stream)),
            subscription_handles: tokio::sync::Mutex::new(Vec::new()),
            consumer_ack_wait,
            consumer_max_ack_pending,
        })
    }
}

#[async_trait]
impl Bus for NatsBus {
    async fn publish(&self, subject: &str, data: &[u8]) -> Result<(), OrbflowError> {
        use bytes::Bytes;

        if is_fanout_subject(subject) {
            self.client
                .publish(subject.to_owned(), Bytes::copy_from_slice(data))
                .await
                .map_err(|e| OrbflowError::Bus(format!("natsbus: publish to {subject}: {e}")))?;
            return Ok(());
        }

        use async_nats::jetstream::context::PublishErrorKind;

        self.jetstream
            .publish(subject.to_owned(), Bytes::copy_from_slice(data))
            .await
            .map_err(|e| OrbflowError::Bus(format!("natsbus: publish to {subject}: {e}")))?
            .await
            .map_err(|e| match e.kind() {
                PublishErrorKind::StreamNotFound => {
                    OrbflowError::Bus(format!("natsbus: stream not found for {subject}"))
                }
                _ => OrbflowError::Bus(format!("natsbus: ack for {subject}: {e}")),
            })?;

        Ok(())
    }

    async fn subscribe(&self, subject: &str, handler: MsgHandler) -> Result<(), OrbflowError> {
        if is_fanout_subject(subject) {
            let mut subscriber = self
                .client
                .subscribe(subject.to_owned())
                .await
                .map_err(|e| OrbflowError::Bus(format!("natsbus: subscribe {subject}: {e}")))?;

            let subject_name = subject.to_owned();
            let handle = tokio::spawn(async move {
                use tokio_stream::StreamExt;

                while let Some(msg) = subscriber.next().await {
                    let subject = msg.subject.to_string();
                    let payload = msg.payload.to_vec();

                    if let Err(e) = handler(subject, payload).await {
                        let err_str = e.to_string();
                        if err_str.contains("stream closed") {
                            tracing::info!(
                                "natsbus: stream subscriber disconnected for {subject_name}"
                            );
                            return;
                        }
                        tracing::warn!("natsbus: stream handler error for {subject_name}: {e}");
                    }
                }
            });
            self.subscription_handles.lock().await.push(handle);
            return Ok(());
        }

        let stream_guard = self.stream.lock().await;
        let stream = stream_guard
            .as_ref()
            .ok_or_else(|| OrbflowError::Bus("natsbus: stream not available".into()))?;

        // Durable name derived from subject (replace "." with "_") so multiple
        // workers share the same consumer (competing consumers pattern).
        let durable = durable_name_for_subject(subject);

        let consumer: PullConsumer = stream
            .get_or_create_consumer(
                &durable,
                jetstream::consumer::pull::Config {
                    durable_name: Some(durable.clone()),
                    filter_subject: subject.to_owned(),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    ack_wait: self.consumer_ack_wait,
                    max_ack_pending: self.consumer_max_ack_pending,
                    deliver_policy: jetstream::consumer::DeliverPolicy::All,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| OrbflowError::Bus(format!("natsbus: create consumer {durable}: {e}")))?;

        // Spawn a background task that pulls messages from the consumer.
        let handler = handler.clone();
        let handle = tokio::spawn(async move {
            loop {
                let mut messages = match consumer.fetch().max_messages(64).messages().await {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!("natsbus: fetch messages for {durable}: {e}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                use tokio_stream::StreamExt;
                let mut count = 0u32;
                while let Some(Ok(msg)) = messages.next().await {
                    count += 1;
                    let handler = handler.clone();
                    let durable = durable.clone();
                    let subject = msg.subject.to_string();
                    let payload = msg.payload.to_vec();

                    tokio::spawn(async move {
                        match handler(subject, payload).await {
                            Ok(()) => {
                                if let Err(e) = msg.ack().await {
                                    tracing::warn!("natsbus: ack failed: {e}");
                                }
                            }
                            Err(e) => {
                                let err_str = e.to_string();
                                if err_str.contains("stream closed") {
                                    tracing::info!(
                                        "natsbus: consumer disconnected, acking message for {durable}"
                                    );
                                    if let Err(ne) = msg.ack().await {
                                        tracing::warn!("natsbus: ack on close failed: {ne}");
                                    }
                                    return;
                                }
                                tracing::warn!("natsbus: handler error: {e}, nak with delay");
                                if let Err(ne) = msg
                                    .ack_with(async_nats::jetstream::AckKind::Nak(Some(
                                        Duration::from_secs(5),
                                    )))
                                    .await
                                {
                                    tracing::warn!("natsbus: nak failed: {ne}");
                                }
                            }
                        }
                    });
                }

                // Only back off when idle to avoid throughput ceiling.
                if count == 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        });
        self.subscription_handles.lock().await.push(handle);

        Ok(())
    }

    async fn close(&self) -> Result<(), OrbflowError> {
        // Abort all subscription tasks.
        {
            let mut handles = self.subscription_handles.lock().await;
            for handle in handles.drain(..) {
                handle.abort();
            }
        }

        // Drop the stream reference.
        let mut stream_guard = self.stream.lock().await;
        *stream_guard = None;

        // Drain and close the NATS connection.
        self.client
            .drain()
            .await
            .map_err(|e| OrbflowError::Bus(format!("natsbus: drain: {e}")))?;

        Ok(())
    }
}

fn durable_name_for_subject(subject: &str) -> String {
    let mut durable = String::with_capacity(subject.len());
    for b in subject.bytes() {
        match b {
            b'.' => durable.push('_'),
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' => durable.push(b as char),
            other => {
                let _ = write!(&mut durable, "_x{other:02X}");
            }
        }
    }
    durable
}

fn is_stream_subject(subject: &str) -> bool {
    subject.starts_with(STREAM_SUBJECT_PREFIX)
}

fn is_fanout_subject(subject: &str) -> bool {
    is_stream_subject(subject) || subject == PLUGIN_RELOAD_SUBJECT
}

struct ParsedNatsUrl {
    scheme: String,
    host: String,
}

impl ParsedNatsUrl {
    fn is_loopback(&self) -> bool {
        self.host.eq_ignore_ascii_case("localhost")
            || self.host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
    }
}

fn parse_nats_url(url: &str) -> Option<ParsedNatsUrl> {
    let (scheme, rest) = url.split_once("://")?;
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if let Some(bracketed) = host_port.strip_prefix('[') {
        bracketed.split_once(']')?.0.to_owned()
    } else {
        host_port
            .split_once(':')
            .map_or(host_port, |(host, _)| host)
            .to_owned()
    };

    if scheme.is_empty() || host.is_empty() {
        return None;
    }

    Some(ParsedNatsUrl {
        scheme: scheme.to_ascii_lowercase(),
        host,
    })
}
