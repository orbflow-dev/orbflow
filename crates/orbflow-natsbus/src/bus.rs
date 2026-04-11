// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! NATS JetStream implementation of [`Bus`].

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

/// NATS JetStream implementation of [`orbflow_core::ports::Bus`].
///
/// Uses a WorkQueue retention stream with explicit ack and 5s NakDelay,
/// matching the Go `natsbus.Bus` implementation.
pub struct NatsBus {
    client: async_nats::Client,
    jetstream: jetstream::Context,
    stream: Mutex<Option<jetstream::stream::Stream>>,
    subscription_handles: tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
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
    /// uses `tls://` or targets a loopback address (`127.0.0.1` / `localhost`).
    pub async fn connect_with_options(url: &str, require_tls: bool) -> Result<Self, OrbflowError> {
        let is_loopback =
            url.starts_with("nats://127.0.0.1") || url.starts_with("nats://localhost");
        let is_tls = url.starts_with("tls://");

        if require_tls && !is_tls && !is_loopback {
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

        let subjects_pattern = format!("{SUBJECT_PREFIX}.>");

        let stream = jetstream
            .get_or_create_stream(jetstream::stream::Config {
                name: STREAM_NAME.to_owned(),
                subjects: vec![subjects_pattern],
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
        })
    }
}

#[async_trait]
impl Bus for NatsBus {
    async fn publish(&self, subject: &str, data: &[u8]) -> Result<(), OrbflowError> {
        use async_nats::jetstream::context::PublishErrorKind;
        use bytes::Bytes;

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
        let stream_guard = self.stream.lock().await;
        let stream = stream_guard
            .as_ref()
            .ok_or_else(|| OrbflowError::Bus("natsbus: stream not available".into()))?;

        // Durable name derived from subject (replace "." with "_") so multiple
        // workers share the same consumer (competing consumers pattern).
        let durable = subject.replace('.', "_");

        let consumer: PullConsumer = stream
            .get_or_create_consumer(
                &durable,
                jetstream::consumer::pull::Config {
                    durable_name: Some(durable.clone()),
                    filter_subject: subject.to_owned(),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
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
                    let subject = msg.subject.to_string();
                    let payload = msg.payload.to_vec();

                    match handler(subject, payload).await {
                        Ok(()) => {
                            if let Err(e) = msg.ack().await {
                                tracing::warn!("natsbus: ack failed: {e}");
                            }
                        }
                        Err(e) => {
                            // If the handler returns a "stream closed" error,
                            // the consumer (e.g. SSE client) has disconnected.
                            // Break out of the loop to avoid leaking this task.
                            let err_str = e.to_string();
                            if err_str.contains("stream closed") {
                                tracing::info!(
                                    "natsbus: consumer disconnected, stopping subscription loop for {durable}"
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
