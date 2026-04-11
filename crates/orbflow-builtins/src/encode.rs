// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Encode node: base64 encode/decode, URL encode/decode, SHA-256, MD5 hashing.

use async_trait::async_trait;
use base64::Engine as Base64Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use md5::Md5;
use serde_json::Value;
use sha2::{Digest, Sha256};

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::util::{make_output, resolve_config, string_val};

/// Performs encoding, decoding, and hashing operations.
pub struct EncodeNode;

impl NodeSchemaProvider for EncodeNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:encode".into(),
            name: "Encode / Hash".into(),
            description: "Base64, URL encode/decode, or hash a string".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "shield".into(),
            color: "#14b8a6".into(),
            image_url: Some("/icons/shield.svg".into()),
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "input".into(),
                    label: "Input".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("String to process".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "operation".into(),
                    label: "Operation".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("Which operation to perform".into()),
                    r#enum: vec![
                        "base64-encode".into(),
                        "base64-decode".into(),
                        "url-encode".into(),
                        "url-decode".into(),
                        "sha256".into(),
                        "md5".into(),
                    ],
                    credential_type: None,
                },
            ],
            outputs: vec![
                FieldSchema {
                    key: "result".into(),
                    label: "Result".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "operation".into(),
                    label: "Operation".into(),
                    field_type: FieldType::String,
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
impl NodeExecutor for EncodeNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);
        let raw = string_val(&cfg, "input", "");
        let op = string_val(&cfg, "operation", "");

        if raw.is_empty() || op.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "encode node: input and operation are required".into(),
            ));
        }

        const MAX_ENCODE_INPUT_BYTES: usize = 1024 * 1024; // 1 MB
        if raw.len() > MAX_ENCODE_INPUT_BYTES {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "encode node: input exceeds maximum size of {MAX_ENCODE_INPUT_BYTES} bytes"
            )));
        }

        let result = match op.as_str() {
            "base64-encode" => BASE64_STANDARD.encode(raw.as_bytes()),

            "base64-decode" => {
                let decoded = BASE64_STANDARD.decode(raw.as_bytes()).map_err(|e| {
                    OrbflowError::Internal(format!("encode node: base64 decode failed: {e}"))
                })?;
                String::from_utf8(decoded).map_err(|e| {
                    OrbflowError::Internal(format!(
                        "encode node: base64 decode produced invalid UTF-8: {e}"
                    ))
                })?
            }

            "url-encode" => url::form_urlencoded::byte_serialize(raw.as_bytes()).collect(),

            "url-decode" => url::form_urlencoded::parse(raw.as_bytes())
                .map(|(k, v)| {
                    if v.is_empty() {
                        k.into_owned()
                    } else {
                        format!("{k}={v}")
                    }
                })
                .collect::<Vec<_>>()
                .join("&"),

            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(raw.as_bytes());
                hex::encode(hasher.finalize())
            }

            "md5" => {
                let mut hasher = Md5::new();
                hasher.update(raw.as_bytes());
                hex::encode(hasher.finalize())
            }

            _ => {
                return Err(OrbflowError::InvalidNodeConfig(format!(
                    "encode node: unknown operation {op:?}"
                )));
            }
        };

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("result", Value::String(result)),
                ("operation", Value::String(op)),
            ])),
            error: None,
        })
    }
}

/// Encode bytes as a lowercase hex string.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbflow_core::execution::InstanceId;
    use std::collections::HashMap;

    fn make_input(config: HashMap<String, Value>) -> NodeInput {
        NodeInput {
            instance_id: InstanceId::new("inst-1"),
            node_id: "encode-1".into(),
            plugin_ref: "builtin:encode".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        }
    }

    #[tokio::test]
    async fn test_base64_encode() {
        let node = EncodeNode;
        let mut config = HashMap::new();
        config.insert("input".into(), serde_json::json!("Hello, World!"));
        config.insert("operation".into(), serde_json::json!("base64-encode"));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("result").unwrap(), "SGVsbG8sIFdvcmxkIQ==");
    }

    #[tokio::test]
    async fn test_base64_decode() {
        let node = EncodeNode;
        let mut config = HashMap::new();
        config.insert("input".into(), serde_json::json!("SGVsbG8sIFdvcmxkIQ=="));
        config.insert("operation".into(), serde_json::json!("base64-decode"));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("result").unwrap(), "Hello, World!");
    }

    #[tokio::test]
    async fn test_url_encode() {
        let node = EncodeNode;
        let mut config = HashMap::new();
        config.insert("input".into(), serde_json::json!("hello world&foo=bar"));
        config.insert("operation".into(), serde_json::json!("url-encode"));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        let result = data.get("result").unwrap().as_str().unwrap();
        assert!(result.contains("%26") || result.contains("hello+world"));
    }

    #[tokio::test]
    async fn test_sha256() {
        let node = EncodeNode;
        let mut config = HashMap::new();
        config.insert("input".into(), serde_json::json!("hello"));
        config.insert("operation".into(), serde_json::json!("sha256"));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(
            data.get("result").unwrap(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[tokio::test]
    async fn test_md5() {
        let node = EncodeNode;
        let mut config = HashMap::new();
        config.insert("input".into(), serde_json::json!("hello"));
        config.insert("operation".into(), serde_json::json!("md5"));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(
            data.get("result").unwrap(),
            "5d41402abc4b2a76b9719d911017c592"
        );
    }

    #[tokio::test]
    async fn test_unknown_operation() {
        let node = EncodeNode;
        let mut config = HashMap::new();
        config.insert("input".into(), serde_json::json!("hello"));
        config.insert("operation".into(), serde_json::json!("unknown"));

        let result = node.execute(&make_input(config)).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_schema() {
        let node = EncodeNode;
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:encode");
    }
}
