// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Email node: SMTP sending using lettre.

use async_trait::async_trait;
use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::util::{int_val, make_output, resolve_config, string_val};

/// Sends an email via SMTP.
pub struct EmailNode {
    pub default_from: Option<String>,
    pub default_host: Option<String>,
    pub default_port: u16,
}

impl EmailNode {
    pub fn new() -> Self {
        Self {
            default_from: None,
            default_host: None,
            default_port: 587,
        }
    }
}

impl Default for EmailNode {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeSchemaProvider for EmailNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:email".into(),
            name: "Send Email".into(),
            description: "Send an email via SMTP".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "mail".into(),
            color: "#ef4444".into(),
            image_url: Some("/icons/mail.svg".into()),
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "to".into(),
                    label: "To".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("Recipient email(s), comma-separated".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "subject".into(),
                    label: "Subject".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("Email subject line".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "body".into(),
                    label: "Body".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("Email body content".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "from".into(),
                    label: "From".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some(
                        "Sender address (uses credential or server default if empty)".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "content_type".into(),
                    label: "Content Type".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("text/plain".into())),
                    description: Some("Email content format".into()),
                    r#enum: vec!["text/plain".into(), "text/html".into()],
                    credential_type: None,
                },
            ],
            outputs: vec![
                FieldSchema {
                    key: "sent".into(),
                    label: "Sent".into(),
                    field_type: FieldType::Boolean,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "message".into(),
                    label: "Status".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            parameters: vec![FieldSchema {
                key: "credential_id".into(),
                label: "SMTP Credential".into(),
                field_type: FieldType::Credential,
                required: false,
                default: None,
                description: Some("Select an SMTP credential for connection settings".into()),
                r#enum: vec![],
                credential_type: Some("smtp".into()),
            }],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for EmailNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let to = string_val(&cfg, "to", "");
        let subject = string_val(&cfg, "subject", "");
        let body = string_val(&cfg, "body", "");
        if to.is_empty() || subject.is_empty() || body.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "email node: to, subject, and body are required".into(),
            ));
        }

        // Resolve from: input > credential (from_address) > struct default > env fallback.
        let mut from = string_val(&cfg, "from", "");
        if from.is_empty() {
            from = string_val(&cfg, "from_address", "");
        }
        if from.is_empty()
            && let Some(ref default_from) = self.default_from
        {
            from = default_from.clone();
        }
        if from.is_empty() {
            from = std::env::var("ORBFLOW_SMTP_FROM").unwrap_or_default();
        }
        if from.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "email node: from address is required (set via credential, input, or ORBFLOW_SMTP_FROM)"
                    .into(),
            ));
        }

        // Resolve host: input > credential (host / smtp_host) > struct default > env fallback.
        let mut host = string_val(&cfg, "host", "");
        if host.is_empty() {
            host = string_val(&cfg, "smtp_host", "");
        }
        if host.is_empty()
            && let Some(ref default_host) = self.default_host
        {
            host = default_host.clone();
        }
        if host.is_empty() {
            host = std::env::var("ORBFLOW_SMTP_HOST").unwrap_or_default();
        }
        if host.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "email node: SMTP host is required (set via credential, input, or ORBFLOW_SMTP_HOST)"
                    .into(),
            ));
        }

        let mut port = int_val(&cfg, "port", 0);
        if port <= 0 {
            port = int_val(&cfg, "smtp_port", self.default_port as i64);
        }
        if port <= 0 {
            port = 587;
        }
        if port > 65535 {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "email node: invalid SMTP port {port} (must be 1–65535)"
            )));
        }

        // SSRF protection: block SMTP connections to private/internal addresses.
        // Validate host does not contain URL-special characters that could cause
        // the synthetic URL to parse differently than the actual SMTP destination.
        if host.contains('@')
            || host.contains('#')
            || host.contains('?')
            || (host.contains('[') && !host.starts_with('['))
        {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "email node: SMTP host '{host}' contains invalid characters"
            )));
        }
        let check_url = format!("http://{host}:{port}/");
        crate::ssrf::validate_url_not_private_async(&check_url, false)
            .await
            .map_err(|e| {
                OrbflowError::InvalidNodeConfig(format!("email node: SMTP host '{host}': {e}"))
            })?;

        let content_type_str = string_val(&cfg, "content_type", "text/plain");
        let content_type = if content_type_str == "text/html" {
            ContentType::TEXT_HTML
        } else {
            ContentType::TEXT_PLAIN
        };

        let from_mailbox: Mailbox = from.parse().map_err(|e| {
            OrbflowError::InvalidNodeConfig(format!("email node: invalid from address: {e}"))
        })?;

        let recipients: Vec<&str> = to.split(',').map(|s| s.trim()).collect();

        let mut message_builder = Message::builder().from(from_mailbox).subject(subject);

        for recipient in &recipients {
            if recipient.is_empty() {
                continue;
            }
            let mailbox: Mailbox = recipient.parse().map_err(|e| {
                OrbflowError::InvalidNodeConfig(format!(
                    "email node: invalid recipient address {recipient:?}: {e}"
                ))
            })?;
            message_builder = message_builder.to(mailbox);
        }

        let message = message_builder
            .header(content_type)
            .body(body)
            .map_err(|e| OrbflowError::Internal(format!("email node: build message: {e}")))?;

        // Resolve SMTP credentials for authentication (optional).
        let username = string_val(&cfg, "username", "");
        let password = string_val(&cfg, "password", "");

        // Build SMTP transport.
        let transport = if port == 465 {
            // Port 465 uses implicit TLS.
            let mut builder = AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
                .map_err(|e| OrbflowError::Internal(format!("email node: SMTP relay: {e}")))?
                .port(port as u16);

            if !username.is_empty() {
                builder = builder.credentials(Credentials::new(username, password));
            }
            builder.build()
        } else {
            // Port 587 uses STARTTLS.
            let mut builder = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
                .map_err(|e| {
                    OrbflowError::Internal(format!("email node: SMTP starttls relay: {e}"))
                })?
                .port(port as u16);

            if !username.is_empty() {
                builder = builder.credentials(Credentials::new(username, password));
            }
            builder.build()
        };

        tokio::time::timeout(std::time::Duration::from_secs(30), transport.send(message))
            .await
            .map_err(|_| OrbflowError::Timeout)?
            .map_err(|e| OrbflowError::Internal(format!("email node: smtp send failed: {e}")))?;

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("sent", Value::Bool(true)),
                ("message", Value::String("Email sent successfully".into())),
            ])),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_schema() {
        let node = EmailNode::new();
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:email");
        assert_eq!(schema.inputs.len(), 5);
        assert_eq!(schema.outputs.len(), 2);
        assert_eq!(schema.parameters.len(), 1);
    }

    #[tokio::test]
    async fn test_email_missing_required_fields() {
        let node = EmailNode::new();
        let config = std::collections::HashMap::new();
        let input = NodeInput {
            instance_id: orbflow_core::execution::InstanceId::new("inst-1"),
            node_id: "email-1".into(),
            plugin_ref: "builtin:email".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        };

        let result = node.execute(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_email_missing_from() {
        let node = EmailNode::new();
        let mut config = std::collections::HashMap::new();
        config.insert("to".into(), serde_json::json!("test@example.com"));
        config.insert("subject".into(), serde_json::json!("Test"));
        config.insert("body".into(), serde_json::json!("Hello"));

        let input = NodeInput {
            instance_id: orbflow_core::execution::InstanceId::new("inst-1"),
            node_id: "email-1".into(),
            plugin_ref: "builtin:email".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        };

        let result = node.execute(&input).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("from address is required"));
    }
}
