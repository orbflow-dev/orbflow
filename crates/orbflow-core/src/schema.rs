// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Built-in credential schemas and validation.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::credential::{CredentialField, CredentialTypeSchema};
use crate::error::OrbflowError;

/// All built-in credential type schemas.
pub static CREDENTIAL_SCHEMAS: LazyLock<Vec<CredentialTypeSchema>> = LazyLock::new(|| {
    vec![
        CredentialTypeSchema {
            credential_type: "postgres".into(),
            name: "PostgreSQL".into(),
            description: "PostgreSQL database connection".into(),
            icon: "database".into(),
            color: "#336791".into(),
            fields: vec![
                CredentialField {
                    key: "host".into(),
                    label: "Host".into(),
                    field_type: "string".into(),
                    required: true,
                    placeholder: Some("localhost".into()),
                    description: Some("Database hostname".into()),
                    default: None,
                },
                CredentialField {
                    key: "port".into(),
                    label: "Port".into(),
                    field_type: "number".into(),
                    required: true,
                    placeholder: Some("5432".into()),
                    description: Some("Database port".into()),
                    default: Some(serde_json::json!(5432)),
                },
                CredentialField {
                    key: "database".into(),
                    label: "Database".into(),
                    field_type: "string".into(),
                    required: true,
                    placeholder: None,
                    description: Some("Database name".into()),
                    default: None,
                },
                CredentialField {
                    key: "username".into(),
                    label: "Username".into(),
                    field_type: "string".into(),
                    required: true,
                    placeholder: None,
                    description: Some("Database username".into()),
                    default: None,
                },
                CredentialField {
                    key: "password".into(),
                    label: "Password".into(),
                    field_type: "password".into(),
                    required: true,
                    placeholder: None,
                    description: Some("Database password".into()),
                    default: None,
                },
                CredentialField {
                    key: "sslmode".into(),
                    label: "SSL Mode".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: Some("disable".into()),
                    description: Some("SSL mode (disable, require, verify-ca, verify-full)".into()),
                    default: Some(serde_json::json!("disable")),
                },
            ],
        },
        CredentialTypeSchema {
            credential_type: "smtp".into(),
            name: "SMTP".into(),
            description: "SMTP email server connection".into(),
            icon: "mail".into(),
            color: "#4A90D9".into(),
            fields: vec![
                CredentialField {
                    key: "host".into(),
                    label: "Host".into(),
                    field_type: "string".into(),
                    required: true,
                    placeholder: Some("smtp.example.com".into()),
                    description: Some("SMTP server hostname".into()),
                    default: None,
                },
                CredentialField {
                    key: "port".into(),
                    label: "Port".into(),
                    field_type: "number".into(),
                    required: true,
                    placeholder: Some("587".into()),
                    description: Some("SMTP server port".into()),
                    default: Some(serde_json::json!(587)),
                },
                CredentialField {
                    key: "username".into(),
                    label: "Username".into(),
                    field_type: "string".into(),
                    required: true,
                    placeholder: None,
                    description: Some("SMTP username".into()),
                    default: None,
                },
                CredentialField {
                    key: "password".into(),
                    label: "Password".into(),
                    field_type: "password".into(),
                    required: true,
                    placeholder: None,
                    description: Some("SMTP password".into()),
                    default: None,
                },
                CredentialField {
                    key: "from".into(),
                    label: "From Address".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: Some("noreply@example.com".into()),
                    description: Some("Default sender address".into()),
                    default: None,
                },
            ],
        },
        CredentialTypeSchema {
            credential_type: "api_key".into(),
            name: "API Key".into(),
            description: "API key authentication".into(),
            icon: "key".into(),
            color: "#F5A623".into(),
            fields: vec![
                CredentialField {
                    key: "key".into(),
                    label: "API Key".into(),
                    field_type: "password".into(),
                    required: true,
                    placeholder: None,
                    description: Some("The API key".into()),
                    default: None,
                },
                CredentialField {
                    key: "header_name".into(),
                    label: "Header Name".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: Some("Authorization".into()),
                    description: Some("HTTP header name for the key".into()),
                    default: Some(serde_json::json!("Authorization")),
                },
                CredentialField {
                    key: "prefix".into(),
                    label: "Prefix".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: Some("Bearer".into()),
                    description: Some("Prefix for the header value (e.g. Bearer)".into()),
                    default: Some(serde_json::json!("Bearer")),
                },
            ],
        },
        CredentialTypeSchema {
            credential_type: "oauth2".into(),
            name: "OAuth2".into(),
            description: "OAuth2 client credentials".into(),
            icon: "shield".into(),
            color: "#7B68EE".into(),
            fields: vec![
                CredentialField {
                    key: "client_id".into(),
                    label: "Client ID".into(),
                    field_type: "string".into(),
                    required: true,
                    placeholder: None,
                    description: Some("OAuth2 client ID".into()),
                    default: None,
                },
                CredentialField {
                    key: "client_secret".into(),
                    label: "Client Secret".into(),
                    field_type: "password".into(),
                    required: true,
                    placeholder: None,
                    description: Some("OAuth2 client secret".into()),
                    default: None,
                },
                CredentialField {
                    key: "token_url".into(),
                    label: "Token URL".into(),
                    field_type: "string".into(),
                    required: true,
                    placeholder: Some("https://oauth.example.com/token".into()),
                    description: Some("OAuth2 token endpoint URL".into()),
                    default: None,
                },
                CredentialField {
                    key: "scopes".into(),
                    label: "Scopes".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: Some("read write".into()),
                    description: Some("Space-separated list of scopes".into()),
                    default: None,
                },
            ],
        },
        CredentialTypeSchema {
            credential_type: "openai".into(),
            name: "OpenAI".into(),
            description: "OpenAI API key for GPT, DALL-E, and Embeddings".into(),
            icon: "brain".into(),
            color: "#10A37F".into(),
            fields: vec![
                CredentialField {
                    key: "api_key".into(),
                    label: "API Key".into(),
                    field_type: "password".into(),
                    required: true,
                    placeholder: Some("sk-...".into()),
                    description: Some("Your OpenAI API key".into()),
                    default: None,
                },
                CredentialField {
                    key: "organization".into(),
                    label: "Organization ID".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: Some("org-...".into()),
                    description: Some("Optional organization ID".into()),
                    default: None,
                },
                CredentialField {
                    key: "base_url".into(),
                    label: "Base URL".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: None,
                    description: Some("Override for Azure OpenAI or compatible APIs".into()),
                    default: Some(serde_json::json!("https://api.openai.com/v1")),
                },
            ],
        },
        CredentialTypeSchema {
            credential_type: "anthropic".into(),
            name: "Anthropic".into(),
            description: "Anthropic API key for Claude models".into(),
            icon: "brain".into(),
            color: "#D4A574".into(),
            fields: vec![
                CredentialField {
                    key: "api_key".into(),
                    label: "API Key".into(),
                    field_type: "password".into(),
                    required: true,
                    placeholder: Some("sk-ant-...".into()),
                    description: Some("Your Anthropic API key".into()),
                    default: None,
                },
                CredentialField {
                    key: "base_url".into(),
                    label: "Base URL".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: None,
                    description: Some("Override for custom deployments".into()),
                    default: Some(serde_json::json!("https://api.anthropic.com")),
                },
            ],
        },
        CredentialTypeSchema {
            credential_type: "google_ai".into(),
            name: "Google AI".into(),
            description: "Google AI API key for Gemini models".into(),
            icon: "brain".into(),
            color: "#4285F4".into(),
            fields: vec![
                CredentialField {
                    key: "api_key".into(),
                    label: "API Key".into(),
                    field_type: "password".into(),
                    required: true,
                    placeholder: None,
                    description: Some("Your Google AI API key".into()),
                    default: None,
                },
                CredentialField {
                    key: "base_url".into(),
                    label: "Base URL".into(),
                    field_type: "string".into(),
                    required: false,
                    placeholder: None,
                    description: Some("Override for custom endpoints".into()),
                    default: Some(serde_json::json!(
                        "https://generativelanguage.googleapis.com/v1beta"
                    )),
                },
            ],
        },
        CredentialTypeSchema {
            credential_type: "custom".into(),
            name: "Custom".into(),
            description: "Custom key-value credential".into(),
            icon: "settings".into(),
            color: "#888888".into(),
            fields: vec![],
        },
    ]
});

/// Trait to provide credential schema lookup.
pub struct CredentialSchemas;

impl CredentialSchemas {
    /// Returns the schema for the given credential type, or `None`.
    pub fn get(cred_type: &str) -> Option<&'static CredentialTypeSchema> {
        CREDENTIAL_SCHEMAS
            .iter()
            .find(|s| s.credential_type == cred_type)
    }

    /// Validates credential data against its type schema.
    pub fn validate(
        cred_type: &str,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<(), OrbflowError> {
        let Some(schema) = Self::get(cred_type) else {
            // Unknown type — custom credentials have no required fields
            return Ok(());
        };

        let mut errors = Vec::new();
        for field in &schema.fields {
            if field.required {
                match data.get(&field.key) {
                    None => {
                        errors.push(format!("missing required field: {}", field.key));
                    }
                    Some(v) if v.is_null() => {
                        errors.push(format!("missing required field: {}", field.key));
                    }
                    Some(serde_json::Value::String(s)) if s.is_empty() => {
                        errors.push(format!("field {} must not be empty", field.key));
                    }
                    _ => {}
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(OrbflowError::InvalidNodeConfig(errors.join("; ")))
        }
    }

    /// Returns the set of field keys marked as `password` type for the given
    /// credential type.  Fields not in this set are safe to display in the UI.
    /// Returns an empty set for unknown / custom credential types (conservative:
    /// the caller should fall back to redacting all keys).
    pub fn secret_keys(cred_type: &str) -> std::collections::HashSet<&'static str> {
        match Self::get(cred_type) {
            Some(schema) => schema
                .fields
                .iter()
                .filter(|f| f.field_type == "password")
                .map(|f| f.key.as_str())
                .collect(),
            None => std::collections::HashSet::new(),
        }
    }

    /// Returns all built-in credential type schemas.
    pub fn all() -> &'static [CredentialTypeSchema] {
        &CREDENTIAL_SCHEMAS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_schema() {
        assert!(CredentialSchemas::get("postgres").is_some());
        assert!(CredentialSchemas::get("smtp").is_some());
        assert!(CredentialSchemas::get("nonexistent").is_none());
    }

    #[test]
    fn test_validate_valid_postgres() {
        let data = HashMap::from([
            ("host".into(), serde_json::json!("localhost")),
            ("port".into(), serde_json::json!(5432)),
            ("database".into(), serde_json::json!("test")),
            ("username".into(), serde_json::json!("user")),
            ("password".into(), serde_json::json!("pass")),
        ]);
        assert!(CredentialSchemas::validate("postgres", &data).is_ok());
    }

    #[test]
    fn test_validate_missing_required() {
        let data = HashMap::from([("host".into(), serde_json::json!("localhost"))]);
        assert!(CredentialSchemas::validate("postgres", &data).is_err());
    }
}
