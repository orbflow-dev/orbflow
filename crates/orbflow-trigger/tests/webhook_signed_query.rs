#![allow(clippy::type_complexity)]
// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use orbflow_core::TriggerType;
use orbflow_core::workflow::WorkflowId;
use orbflow_trigger::{TriggerCallback, WebhookHandler};
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

type HmacSha256 = Hmac<Sha256>;

fn signed_header(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    let mut signature = String::from("sha256=");
    use std::fmt::Write as _;
    for byte in digest {
        write!(&mut signature, "{byte:02x}").unwrap();
    }
    signature
}

async fn spawn_webhook_server(
    handler: WebhookHandler,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = handler.router();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    for _ in 0..50 {
        if TcpStream::connect(addr).await.is_ok() {
            return (addr, handle);
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    handle.abort();
    panic!("webhook test server did not start on {addr}");
}

async fn post_signed(addr: SocketAddr, path: &str, body: &[u8], signature: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Connection: close\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         X-Orbflow-Signature: {signature}\r\n\
         \r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let text = String::from_utf8_lossy(&response).into_owned();
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .expect("HTTP response status code");
    (status, text)
}

fn recording_callback() -> (
    TriggerCallback,
    mpsc::Receiver<(WorkflowId, TriggerType, HashMap<String, serde_json::Value>)>,
) {
    let (tx, rx) = mpsc::channel(4);
    let cb: TriggerCallback = Arc::new(move |wf, trigger_type, payload| {
        let tx = tx.clone();
        Box::pin(async move {
            let _ = tx.send((wf, trigger_type, payload)).await;
        })
    });
    (cb, rx)
}

#[tokio::test]
async fn signed_webhook_body_only_payload_still_fires() {
    let (callback, mut rx) = recording_callback();
    let handler = WebhookHandler::new(callback);
    let workflow_id = WorkflowId::new("wf-signed-body");
    handler.register_with_secret(&workflow_id, "", Some("route-secret".to_string()));
    let (addr, server) = spawn_webhook_server(handler).await;

    let body = br#"{"trusted":"body"}"#;
    let signature = signed_header("route-secret", body);
    let (status, response) = post_signed(addr, "/webhooks/wf-signed-body", body, &signature).await;
    server.abort();

    assert_eq!(status, 200, "response was: {response}");
    let (_, trigger_type, payload) =
        tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("signed body-only webhook should fire")
            .expect("callback payload");
    assert_eq!(trigger_type, TriggerType::Webhook);
    assert_eq!(payload.get("trusted"), Some(&serde_json::json!("body")));
}

#[tokio::test]
async fn signed_webhook_rejects_unsigned_query_injection() {
    let (callback, mut rx) = recording_callback();
    let handler = WebhookHandler::new(callback);
    let workflow_id = WorkflowId::new("wf-signed-query");
    handler.register_with_secret(&workflow_id, "", Some("route-secret".to_string()));
    let (addr, server) = spawn_webhook_server(handler).await;

    let body = br#"{"trusted":"body"}"#;
    let signature = signed_header("route-secret", body);
    let (status, response) = post_signed(
        addr,
        "/webhooks/wf-signed-query?unsigned=evil",
        body,
        &signature,
    )
    .await;

    assert_eq!(
        status, 400,
        "signed webhook routes must reject unsigned query data, response was: {response}"
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .is_err(),
        "rejected signed-query webhook must not fire the trigger callback"
    );
    server.abort();
}
