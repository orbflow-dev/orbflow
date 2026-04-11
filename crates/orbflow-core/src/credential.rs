// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Credential types for encrypted secret management.

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Uniquely identifies a credential.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CredentialId(pub String);

impl<'de> serde::Deserialize<'de> for CredentialId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        CredentialId::new(s).map_err(serde::de::Error::custom)
    }
}

impl CredentialId {
    /// Creates a new `CredentialId`. Returns an error if the id is empty, too
    /// long, or contains characters outside the allowed set (alphanumeric,
    /// hyphens, underscores, and dots).
    pub fn new(id: impl Into<String>) -> Result<Self, crate::error::OrbflowError> {
        let id = id.into();
        if id.is_empty() || id.len() > 128 {
            return Err(crate::error::OrbflowError::InvalidNodeConfig(
                "CredentialId must be 1-128 characters".into(),
            ));
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(crate::error::OrbflowError::InvalidNodeConfig(
                "CredentialId must contain only alphanumeric characters, hyphens, underscores, and dots".into(),
            ));
        }
        Ok(Self(id))
    }
}

impl fmt::Display for CredentialId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CredentialId {
    type Error = crate::error::OrbflowError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl TryFrom<&str> for CredentialId {
    type Error = crate::error::OrbflowError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

/// Maximum length for credential name and type fields.
const MAX_CREDENTIAL_NAME_LEN: usize = 256;

/// Maximum serialized size of credential data (64 KB).
const MAX_CREDENTIAL_DATA_SIZE: usize = 64 * 1024;

/// An encrypted credential.
///
/// `Debug` is manually implemented to redact the `data` field (which contains
/// secrets like API keys and passwords) from logs and panic messages.
///
/// `Serialize` is intentionally omitted to prevent accidental secret leakage
/// in API responses. Use `CredentialSummary` for API serialization.
/// `Serialize` is only available in test builds for roundtrip testing.
#[derive(Clone, Deserialize)]
#[cfg_attr(test, derive(Serialize))]
pub struct Credential {
    pub id: CredentialId,
    pub name: String,
    #[serde(rename = "type")]
    pub credential_type: String,
    pub data: HashMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The user who owns this credential. Used for tenant isolation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    /// Access tier controlling how this credential is shared with plugins.
    #[serde(default)]
    pub access_tier: crate::credential_proxy::CredentialAccessTier,
    /// Policy for credential proxy usage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<crate::credential_proxy::CredentialPolicy>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credential")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("credential_type", &self.credential_type)
            .field("data", &"[redacted]")
            .field("description", &self.description)
            .field("owner_id", &self.owner_id)
            .field("access_tier", &self.access_tier)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

/// Request body for creating a credential (no id/timestamps — server generates those).
///
/// `Debug` is manually implemented to redact the `data` field which contains
/// plaintext secrets before encryption.
#[derive(Clone, Deserialize)]
pub struct CreateCredentialRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub credential_type: String,
    pub data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    /// Access tier controlling how this credential is shared with plugins.
    /// Defaults to `Proxy` (most secure).
    #[serde(default)]
    pub access_tier: Option<crate::credential_proxy::CredentialAccessTier>,
    /// Policy for credential proxy usage.
    #[serde(default)]
    pub policy: Option<crate::credential_proxy::CredentialPolicy>,
}

impl fmt::Debug for CreateCredentialRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreateCredentialRequest")
            .field("name", &self.name)
            .field("credential_type", &self.credential_type)
            .field("data", &"[redacted]")
            .field("description", &self.description)
            .finish()
    }
}

impl CreateCredentialRequest {
    /// Validates the request fields (length limits, non-empty).
    pub fn validate(&self) -> Result<(), crate::error::OrbflowError> {
        if self.name.is_empty() || self.name.len() > MAX_CREDENTIAL_NAME_LEN {
            return Err(crate::error::OrbflowError::InvalidNodeConfig(format!(
                "credential name must be 1-{MAX_CREDENTIAL_NAME_LEN} characters"
            )));
        }
        if self.credential_type.is_empty() || self.credential_type.len() > MAX_CREDENTIAL_NAME_LEN {
            return Err(crate::error::OrbflowError::InvalidNodeConfig(format!(
                "credential type must be 1-{MAX_CREDENTIAL_NAME_LEN} characters"
            )));
        }
        let data_size = serde_json::to_vec(&self.data)
            .map_err(|_| {
                crate::error::OrbflowError::InvalidNodeConfig("invalid credential data".into())
            })?
            .len();
        if data_size > MAX_CREDENTIAL_DATA_SIZE {
            return Err(crate::error::OrbflowError::InvalidNodeConfig(format!(
                "credential data exceeds maximum size of {MAX_CREDENTIAL_DATA_SIZE} bytes"
            )));
        }
        Ok(())
    }

    /// Convert into a full `Credential` with a generated ID and current timestamps.
    /// Validates the request before conversion.
    pub fn into_credential(self) -> Result<Credential, crate::error::OrbflowError> {
        self.validate()?;
        self.into_credential_unchecked()
    }

    /// Convert without validation (for internal use after separate validation).
    fn into_credential_unchecked(self) -> Result<Credential, crate::error::OrbflowError> {
        let now = Utc::now();
        Ok(Credential {
            id: CredentialId::new(uuid::Uuid::new_v4().to_string())?,
            name: self.name,
            credential_type: self.credential_type,
            data: self.data,
            description: self.description,
            owner_id: None,
            access_tier: self.access_tier.unwrap_or_default(),
            policy: self.policy,
            created_at: now,
            updated_at: now,
        })
    }
}

/// Summary view of a credential (excludes secret data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: CredentialId,
    pub name: String,
    #[serde(rename = "type")]
    pub credential_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_tier: Option<crate::credential_proxy::CredentialAccessTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<crate::credential_proxy::CredentialPolicy>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&Credential> for CredentialSummary {
    fn from(c: &Credential) -> Self {
        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            credential_type: c.credential_type.clone(),
            description: c.description.clone(),
            access_tier: Some(c.access_tier),
            policy: c.policy.clone(),
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

/// Schema definition for a credential type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialTypeSchema {
    #[serde(rename = "type")]
    pub credential_type: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub fields: Vec<CredentialField>,
}

/// A single field in a credential type schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialField {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -----------------------------------------------------------------------
    // CredentialId
    // -----------------------------------------------------------------------

    #[test]
    fn credential_id_new_from_str() {
        let id = CredentialId::new("abc-123").unwrap();
        assert_eq!(id.0, "abc-123");
    }

    #[test]
    fn credential_id_new_from_string() {
        let id = CredentialId::new(String::from("owned")).unwrap();
        assert_eq!(id.0, "owned");
    }

    #[test]
    fn credential_id_from_string() {
        let id: CredentialId = String::from("from-string").try_into().unwrap();
        assert_eq!(id.0, "from-string");
    }

    #[test]
    fn credential_id_from_str_ref() {
        let id: CredentialId = "from-ref".try_into().unwrap();
        assert_eq!(id.0, "from-ref");
    }

    #[test]
    fn credential_id_display() {
        let id = CredentialId::new("display-test").unwrap();
        assert_eq!(format!("{id}"), "display-test");
    }

    #[test]
    fn credential_id_empty_string_panics() {
        let result = CredentialId::new("");
        assert!(result.is_err());
    }

    #[test]
    fn credential_id_special_characters_rejected() {
        let result = CredentialId::new("cred/with spaces & symbols!@#$");
        assert!(result.is_err());
    }

    #[test]
    fn credential_id_valid_characters() {
        let id = CredentialId::new("cred-with_dots.123").unwrap();
        assert_eq!(id.0, "cred-with_dots.123");
    }

    #[test]
    fn credential_id_equality() {
        let a = CredentialId::new("same").unwrap();
        let b = CredentialId::new("same").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn credential_id_inequality() {
        let a = CredentialId::new("one").unwrap();
        let b = CredentialId::new("two").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn credential_id_clone() {
        let original = CredentialId::new("clone-me").unwrap();
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn credential_id_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CredentialId::new("unique").unwrap());
        set.insert(CredentialId::new("unique").unwrap());
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn credential_id_serde_roundtrip() {
        let id = CredentialId::new("serde-test").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        // transparent means it serializes as a plain string
        assert_eq!(json, r#""serde-test""#);
        let back: CredentialId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn credential_id_deserialize_from_plain_string() {
        let back: CredentialId = serde_json::from_str(r#""plain""#).unwrap();
        assert_eq!(back.0, "plain");
    }

    // -----------------------------------------------------------------------
    // Credential
    // -----------------------------------------------------------------------

    fn sample_credential() -> Credential {
        let now = Utc::now();
        Credential {
            id: CredentialId::new("cred-1").unwrap(),
            name: "My API Key".into(),
            credential_type: "api_key".into(),
            data: HashMap::from([("key".into(), json!("sk-secret-123"))]),
            description: Some("Test credential".into()),
            access_tier: crate::credential_proxy::CredentialAccessTier::Proxy,
            policy: None,
            owner_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn credential_debug_redacts_data() {
        let cred = sample_credential();
        let debug_str = format!("{cred:?}");
        assert!(debug_str.contains("[redacted]"));
        assert!(!debug_str.contains("sk-secret-123"));
        assert!(debug_str.contains("cred-1"));
        assert!(debug_str.contains("My API Key"));
    }

    #[test]
    fn credential_serde_roundtrip() {
        let cred = sample_credential();
        let json = serde_json::to_string(&cred).unwrap();
        let back: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, cred.id);
        assert_eq!(back.name, cred.name);
        assert_eq!(back.credential_type, cred.credential_type);
        assert_eq!(back.data, cred.data);
        assert_eq!(back.description, cred.description);
    }

    #[test]
    fn credential_serde_type_field_rename() {
        let cred = sample_credential();
        let val: serde_json::Value = serde_json::to_value(&cred).unwrap();
        // The field should be serialized as "type", not "credential_type"
        assert!(val.get("type").is_some());
        assert!(val.get("credential_type").is_none());
    }

    #[test]
    fn credential_description_none_omitted() {
        let mut cred = sample_credential();
        cred.description = None;
        let val: serde_json::Value = serde_json::to_value(&cred).unwrap();
        assert!(val.get("description").is_none());
    }

    #[test]
    fn credential_policy_none_omitted() {
        let cred = sample_credential();
        let val: serde_json::Value = serde_json::to_value(&cred).unwrap();
        assert!(val.get("policy").is_none());
    }

    #[test]
    fn credential_clone() {
        let cred = sample_credential();
        let cloned = cred.clone();
        assert_eq!(cloned.id, cred.id);
        assert_eq!(cloned.data, cred.data);
    }

    #[test]
    fn credential_empty_data_map() {
        let now = Utc::now();
        let cred = Credential {
            id: CredentialId::new("empty-data").unwrap(),
            name: "Empty".into(),
            credential_type: "none".into(),
            data: HashMap::new(),
            description: None,
            access_tier: crate::credential_proxy::CredentialAccessTier::default(),
            policy: None,
            owner_id: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&cred).unwrap();
        let back: Credential = serde_json::from_str(&json).unwrap();
        assert!(back.data.is_empty());
    }

    #[test]
    fn credential_deserialize_with_default_access_tier() {
        // When access_tier is missing from JSON, it should default
        let json = json!({
            "id": "cred-default",
            "name": "Test",
            "type": "oauth",
            "data": {},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        let cred: Credential = serde_json::from_value(json).unwrap();
        assert_eq!(
            cred.access_tier,
            crate::credential_proxy::CredentialAccessTier::Proxy
        );
    }

    // -----------------------------------------------------------------------
    // CreateCredentialRequest
    // -----------------------------------------------------------------------

    fn sample_create_request() -> CreateCredentialRequest {
        CreateCredentialRequest {
            name: "New Cred".into(),
            credential_type: "oauth".into(),
            data: HashMap::from([("token".into(), json!("tok-abc"))]),
            description: Some("A new credential".into()),
            access_tier: None,
            policy: None,
        }
    }

    #[test]
    fn create_request_debug_redacts_data() {
        let req = sample_create_request();
        let debug_str = format!("{req:?}");
        assert!(debug_str.contains("[redacted]"));
        assert!(!debug_str.contains("tok-abc"));
        assert!(debug_str.contains("New Cred"));
    }

    #[test]
    fn create_request_into_credential_generates_id() {
        let req = sample_create_request();
        let cred = req.into_credential().unwrap();
        assert!(!cred.id.0.is_empty());
        // UUID v4 format: 8-4-4-4-12 hex chars
        assert_eq!(cred.id.0.len(), 36);
    }

    #[test]
    fn create_request_into_credential_preserves_fields() {
        let req = sample_create_request();
        let cred = req.into_credential().unwrap();
        assert_eq!(cred.name, "New Cred");
        assert_eq!(cred.credential_type, "oauth");
        assert_eq!(cred.data.get("token"), Some(&json!("tok-abc")));
        assert_eq!(cred.description, Some("A new credential".into()));
    }

    #[test]
    fn create_request_into_credential_sets_timestamps() {
        let before = Utc::now();
        let req = sample_create_request();
        let cred = req.into_credential().unwrap();
        let after = Utc::now();
        assert!(cred.created_at >= before && cred.created_at <= after);
        assert_eq!(cred.created_at, cred.updated_at);
    }

    #[test]
    fn create_request_into_credential_defaults_access_tier() {
        let req = sample_create_request();
        let cred = req.into_credential().unwrap();
        assert_eq!(
            cred.access_tier,
            crate::credential_proxy::CredentialAccessTier::Proxy
        );
    }

    #[test]
    fn create_request_into_credential_respects_access_tier() {
        let mut req = sample_create_request();
        req.access_tier = Some(crate::credential_proxy::CredentialAccessTier::Raw);
        let cred = req.into_credential().unwrap();
        assert_eq!(
            cred.access_tier,
            crate::credential_proxy::CredentialAccessTier::Raw
        );
    }

    #[test]
    fn create_request_into_credential_preserves_policy() {
        let mut req = sample_create_request();
        req.policy = Some(crate::credential_proxy::CredentialPolicy {
            allowed_domains: vec!["example.com".into()],
            ..Default::default()
        });
        let cred = req.into_credential().unwrap();
        assert!(cred.policy.is_some());
        assert_eq!(
            cred.policy.unwrap().allowed_domains,
            vec!["example.com".to_string()]
        );
    }

    #[test]
    fn create_request_deserialize_json() {
        let json = json!({
            "name": "Deserialized",
            "type": "api_key",
            "data": {"secret": "value"},
            "description": "from json"
        });
        let req: CreateCredentialRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.name, "Deserialized");
        assert_eq!(req.credential_type, "api_key");
        assert_eq!(req.description, Some("from json".into()));
    }

    #[test]
    fn create_request_deserialize_minimal() {
        let json = json!({
            "name": "Minimal",
            "type": "token",
            "data": {}
        });
        let req: CreateCredentialRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.name, "Minimal");
        assert!(req.description.is_none());
        assert!(req.access_tier.is_none());
        assert!(req.policy.is_none());
    }

    // -----------------------------------------------------------------------
    // CredentialSummary
    // -----------------------------------------------------------------------

    #[test]
    fn credential_summary_from_credential() {
        let cred = sample_credential();
        let summary = CredentialSummary::from(&cred);
        assert_eq!(summary.id, cred.id);
        assert_eq!(summary.name, cred.name);
        assert_eq!(summary.credential_type, cred.credential_type);
        assert_eq!(summary.description, cred.description);
        assert_eq!(summary.access_tier, Some(cred.access_tier));
        assert!(summary.policy.is_none());
        assert_eq!(summary.created_at, cred.created_at);
        assert_eq!(summary.updated_at, cred.updated_at);
    }

    #[test]
    fn credential_summary_excludes_secret_data() {
        let cred = sample_credential();
        let summary = CredentialSummary::from(&cred);
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("sk-secret-123"));
    }

    #[test]
    fn credential_summary_serde_roundtrip() {
        let cred = sample_credential();
        let summary = CredentialSummary::from(&cred);
        let json = serde_json::to_string(&summary).unwrap();
        let back: CredentialSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, summary.id);
        assert_eq!(back.name, summary.name);
        assert_eq!(back.credential_type, summary.credential_type);
    }

    #[test]
    fn credential_summary_type_field_rename() {
        let cred = sample_credential();
        let summary = CredentialSummary::from(&cred);
        let val: serde_json::Value = serde_json::to_value(&summary).unwrap();
        assert!(val.get("type").is_some());
        assert!(val.get("credential_type").is_none());
    }

    #[test]
    fn credential_summary_description_none_omitted() {
        let mut cred = sample_credential();
        cred.description = None;
        let summary = CredentialSummary::from(&cred);
        let val: serde_json::Value = serde_json::to_value(&summary).unwrap();
        assert!(val.get("description").is_none());
    }

    // -----------------------------------------------------------------------
    // CredentialTypeSchema & CredentialField
    // -----------------------------------------------------------------------

    #[test]
    fn credential_type_schema_serde_roundtrip() {
        let schema = CredentialTypeSchema {
            credential_type: "oauth2".into(),
            name: "OAuth 2.0".into(),
            description: "OAuth 2.0 credentials".into(),
            icon: "key".into(),
            color: "#3b82f6".into(),
            fields: vec![
                CredentialField {
                    key: "client_id".into(),
                    label: "Client ID".into(),
                    field_type: "text".into(),
                    required: true,
                    placeholder: Some("Enter client ID".into()),
                    description: Some("OAuth client identifier".into()),
                    default: None,
                },
                CredentialField {
                    key: "client_secret".into(),
                    label: "Client Secret".into(),
                    field_type: "password".into(),
                    required: true,
                    placeholder: None,
                    description: None,
                    default: None,
                },
            ],
        };

        let json = serde_json::to_string(&schema).unwrap();
        let back: CredentialTypeSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(back.credential_type, "oauth2");
        assert_eq!(back.name, "OAuth 2.0");
        assert_eq!(back.fields.len(), 2);
        assert_eq!(back.fields[0].key, "client_id");
        assert!(back.fields[0].required);
        assert!(!back.fields[1].placeholder.is_some()); // None was serialized away
    }

    #[test]
    fn credential_type_schema_type_field_rename() {
        let schema = CredentialTypeSchema {
            credential_type: "api_key".into(),
            name: "API Key".into(),
            description: "Simple API key".into(),
            icon: "lock".into(),
            color: "#000".into(),
            fields: vec![],
        };
        let val: serde_json::Value = serde_json::to_value(&schema).unwrap();
        assert!(val.get("type").is_some());
        assert!(val.get("credential_type").is_none());
    }

    #[test]
    fn credential_field_defaults() {
        // Deserialize with minimal fields — booleans and optionals should default
        let json = json!({
            "key": "token",
            "label": "Token",
            "type": "text"
        });
        let field: CredentialField = serde_json::from_value(json).unwrap();
        assert!(!field.required);
        assert!(field.placeholder.is_none());
        assert!(field.description.is_none());
        assert!(field.default.is_none());
    }

    #[test]
    fn credential_field_with_default_value() {
        let field = CredentialField {
            key: "timeout".into(),
            label: "Timeout".into(),
            field_type: "number".into(),
            required: false,
            placeholder: None,
            description: None,
            default: Some(json!(30)),
        };
        let json = serde_json::to_string(&field).unwrap();
        let back: CredentialField = serde_json::from_str(&json).unwrap();
        assert_eq!(back.default, Some(json!(30)));
    }

    #[test]
    fn credential_field_type_rename() {
        let field = CredentialField {
            key: "k".into(),
            label: "K".into(),
            field_type: "password".into(),
            required: false,
            placeholder: None,
            description: None,
            default: None,
        };
        let val: serde_json::Value = serde_json::to_value(&field).unwrap();
        assert!(val.get("type").is_some());
        assert!(val.get("field_type").is_none());
    }

    #[test]
    fn credential_field_optional_fields_omitted_when_none() {
        let field = CredentialField {
            key: "k".into(),
            label: "K".into(),
            field_type: "text".into(),
            required: false,
            placeholder: None,
            description: None,
            default: None,
        };
        let val: serde_json::Value = serde_json::to_value(&field).unwrap();
        assert!(val.get("placeholder").is_none());
        assert!(val.get("description").is_none());
        assert!(val.get("default").is_none());
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn credential_with_unicode_name() {
        let now = Utc::now();
        let cred = Credential {
            id: CredentialId::new("unicode-cred").unwrap(),
            name: "日本語の資格情報".into(),
            credential_type: "api_key".into(),
            data: HashMap::from([("キー".into(), json!("値"))]),
            description: Some("テスト用".into()),
            access_tier: crate::credential_proxy::CredentialAccessTier::default(),
            policy: None,
            owner_id: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&cred).unwrap();
        let back: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "日本語の資格情報");
        assert_eq!(back.data.get("キー"), Some(&json!("値")));
    }

    #[test]
    fn credential_with_complex_data_values() {
        let now = Utc::now();
        let cred = Credential {
            id: CredentialId::new("complex").unwrap(),
            name: "Complex".into(),
            credential_type: "custom".into(),
            data: HashMap::from([
                ("nested".into(), json!({"a": {"b": [1, 2, 3]}})),
                ("null_val".into(), json!(null)),
                ("bool_val".into(), json!(true)),
                ("number".into(), json!(42.5)),
            ]),
            description: None,
            access_tier: crate::credential_proxy::CredentialAccessTier::default(),
            policy: None,
            owner_id: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&cred).unwrap();
        let back: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(back.data.len(), 4);
        assert_eq!(back.data["number"], json!(42.5));
    }

    #[test]
    fn multiple_into_credentials_generate_unique_ids() {
        let req1 = sample_create_request();
        let req2 = sample_create_request();
        let cred1 = req1.into_credential().unwrap();
        let cred2 = req2.into_credential().unwrap();
        assert_ne!(cred1.id, cred2.id);
    }
}
