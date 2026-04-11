// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! CapabilityPostgres node: validates PostgreSQL connection parameters and returns DSN.

use async_trait::async_trait;
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};
use orbflow_core::workflow::NodeKind;

use crate::util::{int_val, make_output, resolve_config, string_val};

/// A capability node that validates PostgreSQL connection parameters and returns
/// a connection descriptor. It does not hold a live connection — the worker's
/// connection pool manager uses the descriptor.
pub struct CapabilityPostgres;

impl NodeSchemaProvider for CapabilityPostgres {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:capability-postgres".into(),
            name: "PostgreSQL".into(),
            description: "PostgreSQL database connection".into(),
            category: "builtin".into(),
            node_kind: Some(NodeKind::Capability),
            icon: "database".into(),
            color: "#336791".into(),
            image_url: Some("/icons/database.svg".into()),
            docs: None,
            provides_capability: Some("database".into()),
            inputs: vec![],
            outputs: vec![],
            parameters: vec![
                FieldSchema {
                    key: "credential_id".into(),
                    label: "PostgreSQL Credential".into(),
                    field_type: FieldType::Credential,
                    required: false,
                    default: None,
                    description: Some(
                        "Select a PostgreSQL credential for connection settings".into(),
                    ),
                    r#enum: vec![],
                    credential_type: Some("postgres".into()),
                },
                FieldSchema {
                    key: "host".into(),
                    label: "Host".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("localhost".into())),
                    description: Some("Database server hostname (overrides credential)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "port".into(),
                    label: "Port".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: Some(Value::Number(5432.into())),
                    description: Some("Database server port (overrides credential)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "database".into(),
                    label: "Database".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Database name (overrides credential)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "user".into(),
                    label: "User".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Database user (overrides credential)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "password".into(),
                    label: "Password".into(),
                    field_type: FieldType::Password,
                    required: false,
                    default: None,
                    description: Some("Database password (overrides credential)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "sslmode".into(),
                    label: "SSL Mode".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("require".into())),
                    description: Some("SSL mode (overrides credential)".into()),
                    r#enum: vec![
                        "disable".into(),
                        "require".into(),
                        "verify-ca".into(),
                        "verify-full".into(),
                    ],
                    credential_type: None,
                },
                FieldSchema {
                    key: "pool_size".into(),
                    label: "Pool Size".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: Some(Value::Number(10.into())),
                    description: Some("Connection pool size (overrides credential)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            capability_ports: vec![],
            settings: vec![],
        }
    }
}

#[async_trait]
impl NodeExecutor for CapabilityPostgres {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let host = string_val(&cfg, "host", "localhost");
        let port = int_val(&cfg, "port", 5432);
        let database = string_val(&cfg, "database", "");
        let user = string_val(&cfg, "user", "");
        // Password is intentionally not read — it is retrieved by the
        // connection pool manager via credential_id at connect time.
        let _password = string_val(&cfg, "password", "");
        let sslmode = string_val(&cfg, "sslmode", "require");
        let pool_size = int_val(&cfg, "pool_size", 10);

        if database.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "capability-postgres node: database name is required".into(),
            ));
        }
        if user.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "capability-postgres node: user is required".into(),
            ));
        }

        // Emit password-free connection metadata. The connection pool manager
        // retrieves the actual password via credential_id at connect time.
        // SECURITY: Never include the password in node output — it would be
        // persisted in the event log and visible via the REST API.
        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("driver", Value::String("postgres".into())),
                ("host", Value::String(host)),
                ("port", Value::Number(port.into())),
                ("database", Value::String(database)),
                ("user", Value::String(user)),
                ("sslmode", Value::String(sslmode)),
                ("pool_size", Value::Number(pool_size.into())),
            ])),
            error: None,
        })
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
            node_id: "cap-pg-1".into(),
            plugin_ref: "builtin:capability-postgres".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        }
    }

    #[tokio::test]
    async fn test_cap_postgres_returns_connection_metadata() {
        let node = CapabilityPostgres;
        let mut config = HashMap::new();
        config.insert("host".into(), serde_json::json!("db.example.com"));
        config.insert("port".into(), serde_json::json!(5432));
        config.insert("database".into(), serde_json::json!("mydb"));
        config.insert("user".into(), serde_json::json!("admin"));
        config.insert("password".into(), serde_json::json!("secret"));
        config.insert("sslmode".into(), serde_json::json!("require"));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("driver").unwrap(), "postgres");
        assert_eq!(data.get("host").unwrap(), "db.example.com");
        assert_eq!(data.get("port").unwrap(), 5432);
        assert_eq!(data.get("database").unwrap(), "mydb");
        assert_eq!(data.get("user").unwrap(), "admin");
        assert_eq!(data.get("sslmode").unwrap(), "require");
        // SECURITY: password must NOT appear in node output.
        assert!(data.get("password").is_none());
        assert!(data.get("dsn").is_none());
    }

    #[tokio::test]
    async fn test_cap_postgres_missing_database() {
        let node = CapabilityPostgres;
        let mut config = HashMap::new();
        config.insert("user".into(), serde_json::json!("admin"));

        let result = node.execute(&make_input(config)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cap_postgres_missing_user() {
        let node = CapabilityPostgres;
        let mut config = HashMap::new();
        config.insert("database".into(), serde_json::json!("mydb"));

        let result = node.execute(&make_input(config)).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_cap_postgres_schema() {
        let node = CapabilityPostgres;
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:capability-postgres");
        assert_eq!(schema.node_kind, Some(NodeKind::Capability));
        assert_eq!(schema.provides_capability.as_deref(), Some("database"));
    }
}
