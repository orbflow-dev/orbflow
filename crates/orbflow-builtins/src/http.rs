// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! HTTP request executor using reqwest.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::ssrf::SsrfSafeResolver;
use crate::util::{make_output, resolve_config, string_val};

/// MIME type prefixes that indicate binary data.
const BINARY_CONTENT_PREFIXES: &[&str] = &[
    "image/",
    "audio/",
    "video/",
    "application/octet-stream",
    "application/pdf",
    "application/zip",
    "application/gzip",
    "application/x-tar",
    "application/x-7z",
    "application/vnd.",
    "font/",
];

/// Maximum response body size: 1 MiB.
const MAX_BODY_SIZE: usize = 1 << 20;

/// Returns a shared reqwest client with sensible defaults.
fn default_client() -> Result<&'static reqwest::Client, OrbflowError> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .dns_resolver(Arc::new(SsrfSafeResolver {
            allow_localhost: false,
        }))
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .min_tls_version(reqwest::tls::Version::TLS_1_2)
        .user_agent("Orbflow/1.0")
        .build()
        .map_err(|e| {
            OrbflowError::InvalidNodeConfig(format!("failed to build HTTP client: {e}"))
        })?;
    Ok(CLIENT.get_or_init(|| client))
}

fn is_binary_content_type(ct: &str) -> bool {
    let ct = ct.to_lowercase();
    let ct = ct.split(';').next().unwrap_or("").trim();
    BINARY_CONTENT_PREFIXES
        .iter()
        .any(|prefix| ct.starts_with(prefix))
}

/// Executes an HTTP request.
pub struct HttpNode;

impl NodeSchemaProvider for HttpNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:http".into(),
            name: "HTTP Request".into(),
            description: "Send an HTTP request to an external API".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "globe".into(),
            color: "#3B82F6".into(),
            image_url: Some("/icons/globe.svg".into()),
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "method".into(),
                    label: "Method".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: Some(Value::String("GET".into())),
                    description: None,
                    r#enum: vec![
                        "GET".into(),
                        "POST".into(),
                        "PUT".into(),
                        "PATCH".into(),
                        "DELETE".into(),
                        "HEAD".into(),
                        "OPTIONS".into(),
                    ],
                    credential_type: None,
                },
                FieldSchema {
                    key: "url".into(),
                    label: "URL".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("The URL to send the request to".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "body".into(),
                    label: "Body".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Request body (string or JSON)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "headers".into(),
                    label: "Headers".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("Request headers as key-value pairs".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            outputs: vec![
                FieldSchema {
                    key: "status".into(),
                    label: "Status Code".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "status_text".into(),
                    label: "Status Text".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "body".into(),
                    label: "Response Body".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "headers".into(),
                    label: "Response Headers".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            parameters: vec![],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for HttpNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let method = string_val(&cfg, "method", "GET");
        let url = string_val(&cfg, "url", "");
        if url.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "http node: url is required".into(),
            ));
        }

        // SSRF protection: block requests to private/internal addresses.
        crate::ssrf::validate_url_not_private_async(&url, false).await?;

        let method = method.parse::<reqwest::Method>().map_err(|e| {
            OrbflowError::InvalidNodeConfig(format!("http node: invalid method: {e}"))
        })?;

        let client = default_client()?;
        let mut builder = client.request(method, &url);

        builder = builder.header("Accept", "*/*");

        // Apply user-provided headers (override defaults if specified).
        // Block security-sensitive headers that could be abused for request smuggling.
        const PROTECTED_HEADERS: &[&str] = &[
            "host",
            "content-length",
            "transfer-encoding",
            "te",
            "trailer",
            "upgrade",
        ];
        if let Some(Value::Object(headers)) = cfg.get("headers") {
            for (k, v) in headers {
                if PROTECTED_HEADERS.contains(&k.to_lowercase().as_str()) {
                    return Err(OrbflowError::InvalidNodeConfig(format!(
                        "http node: header '{k}' is reserved and cannot be overridden"
                    )));
                }
                if let Value::String(s) = v {
                    builder = builder.header(k.as_str(), s.as_str());
                }
            }
        }

        if let Some(body) = cfg.get("body") {
            match body {
                Value::String(s) => {
                    builder = builder.body(s.clone());
                }
                other => {
                    let data = serde_json::to_vec(other).map_err(|e| {
                        OrbflowError::Internal(format!("http node: marshal request body: {e}"))
                    })?;
                    builder = builder
                        .header("Content-Type", "application/json")
                        .body(data);
                }
            }
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| OrbflowError::Internal(format!("http node: request failed: {e}")))?;

        let status = resp.status();
        let status_code = status.as_u16();
        let status_text = format!(
            "{} {}",
            status_code,
            status.canonical_reason().unwrap_or("")
        );

        let resp_headers: HashMap<String, Value> = resp
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_owned(),
                    Value::String(v.to_str().unwrap_or("").to_owned()),
                )
            })
            .collect();

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();

        // Read body with size limit.
        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| OrbflowError::Internal(format!("http node: read body: {e}")))?;

        let body_bytes = if body_bytes.len() > MAX_BODY_SIZE {
            &body_bytes[..MAX_BODY_SIZE]
        } else {
            &body_bytes[..]
        };

        let body_value = if is_binary_content_type(&content_type) {
            // Binary responses: store metadata only.
            serde_json::json!({
                "_binary": true,
                "content_type": content_type,
                "size_bytes": body_bytes.len(),
            })
        } else {
            let body_str = String::from_utf8_lossy(body_bytes);
            let trimmed = body_str.trim();
            if !trimmed.is_empty() && (trimmed.starts_with('{') || trimmed.starts_with('[')) {
                // Try to parse JSON body so downstream nodes get structured data.
                match serde_json::from_slice::<Value>(body_bytes) {
                    Ok(parsed) => parsed,
                    Err(_) => Value::String(body_str.into_owned()),
                }
            } else {
                Value::String(body_str.into_owned())
            }
        };

        let data = make_output(vec![
            ("status", Value::Number(status_code.into())),
            ("status_text", Value::String(status_text)),
            ("body", body_value),
            ("headers", Value::Object(resp_headers.into_iter().collect())),
        ]);

        Ok(NodeOutput {
            data: Some(data),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_binary_content_type() {
        assert!(is_binary_content_type("image/png"));
        assert!(is_binary_content_type("Image/PNG; charset=utf-8"));
        assert!(is_binary_content_type("application/pdf"));
        assert!(is_binary_content_type("application/vnd.ms-excel"));
        assert!(!is_binary_content_type("text/html"));
        assert!(!is_binary_content_type("application/json"));
    }

    #[test]
    fn test_http_node_schema() {
        let node = HttpNode;
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:http");
        assert_eq!(schema.inputs.len(), 4);
        assert_eq!(schema.outputs.len(), 4);
    }
}
