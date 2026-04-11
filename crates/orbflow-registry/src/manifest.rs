// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Plugin manifest -- the package.json equivalent for Orbflow plugins.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A plugin manifest describing a Orbflow plugin package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name (unique identifier, e.g., "orbflow-slack").
    pub name: String,
    /// Semantic version (e.g., "1.2.3").
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// Author name or organization.
    pub author: String,
    /// License (SPDX identifier, e.g., "MIT").
    pub license: String,
    /// Repository URL (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Plugin refs this plugin provides (e.g., ["plugin:slack-send", "plugin:slack-channel-list"]).
    pub node_types: Vec<String>,
    /// Minimum compatible Orbflow version.
    pub orbflow_version: String,
    /// Plugin protocol (how to communicate with the plugin).
    pub protocol: PluginProtocol,
    /// Tags for search/categorization.
    #[serde(default)]
    pub tags: Vec<String>,
    /// UI presentation hints (icon, category, color). Flattened into the
    /// manifest JSON for ergonomics but logically separate from the domain
    /// contract (protocol, node_types, version).
    #[serde(flatten, default)]
    pub display: PluginDisplayHints,
    /// Plugin implementation language (e.g., "python", "typescript").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// README content (markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    /// Input field definitions (parsed as FieldSchema in the server).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<serde_json::Value>,
    /// Output field definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<serde_json::Value>,
    /// Parameter field definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<serde_json::Value>,
}

/// How to run the plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginProtocol {
    /// Subprocess plugin (JSON-RPC over stdin/stdout).
    Subprocess { binary_name: String },
    /// gRPC plugin (persistent connection).
    Grpc { default_port: u16 },
}

/// UI presentation hints for marketplace and node picker display.
///
/// These are logically separate from the plugin's domain contract
/// (protocol, node_types, version) but stored in the same manifest
/// file for convenience. Flattened via `#[serde(flatten)]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginDisplayHints {
    /// Icon name or URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Category (e.g., "communication", "database", "ai", "utility").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Color for UI display (hex, e.g., "#6366F1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// A published version of a plugin in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginVersion {
    pub name: String,
    pub version: String,
    pub manifest: PluginManifest,
    /// SHA-256 hash of the plugin binary/archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    /// Download URL for the plugin archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// Download count.
    #[serde(default)]
    pub downloads: u64,
    pub published_at: DateTime<Utc>,
}

/// Summary for listing plugins (without full manifest).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSummary {
    pub name: String,
    pub description: String,
    pub latest_version: String,
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// A workflow template that can be shared via the marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateManifest {
    pub name: String,
    pub description: String,
    /// Full workflow definition as JSON.
    pub workflow: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> PluginManifest {
        PluginManifest {
            name: "orbflow-slack".to_string(),
            version: "1.0.0".to_string(),
            description: "Slack integration for Orbflow".to_string(),
            author: "Orbflow Authors".to_string(),
            license: "MIT".to_string(),
            repository: Some("https://github.com/orbflow-dev/orbflow-slack".to_string()),
            node_types: vec![
                "plugin:slack-send".to_string(),
                "plugin:slack-channel-list".to_string(),
            ],
            orbflow_version: "0.1.0".to_string(),
            protocol: PluginProtocol::Subprocess {
                binary_name: "orbflow-slack".to_string(),
            },
            tags: vec!["communication".to_string(), "messaging".to_string()],
            display: PluginDisplayHints {
                icon: Some("slack".to_string()),
                category: Some("communication".to_string()),
                color: None,
            },
            language: None,
            readme: Some("# Orbflow Slack Plugin\nSend messages to Slack channels.".to_string()),
            inputs: vec![],
            outputs: vec![],
            parameters: vec![],
        }
    }

    #[test]
    fn test_manifest_serde_roundtrip() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).expect("serialize");
        let deserialized: PluginManifest = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.name, "orbflow-slack");
        assert_eq!(deserialized.version, "1.0.0");
        assert_eq!(deserialized.node_types.len(), 2);
        assert_eq!(deserialized.tags.len(), 2);
        assert_eq!(
            deserialized.display.category.as_deref(),
            Some("communication")
        );
    }

    #[test]
    fn test_manifest_subprocess_protocol() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).expect("serialize");
        let deserialized: PluginManifest = serde_json::from_str(&json).expect("deserialize");

        match &deserialized.protocol {
            PluginProtocol::Subprocess { binary_name } => {
                assert_eq!(binary_name, "orbflow-slack");
            }
            PluginProtocol::Grpc { .. } => panic!("expected Subprocess protocol"),
        }
    }

    #[test]
    fn test_manifest_grpc_protocol() {
        let manifest = PluginManifest {
            protocol: PluginProtocol::Grpc { default_port: 9090 },
            ..sample_manifest()
        };

        let json = serde_json::to_string(&manifest).expect("serialize");
        let deserialized: PluginManifest = serde_json::from_str(&json).expect("deserialize");

        match &deserialized.protocol {
            PluginProtocol::Grpc { default_port } => {
                assert_eq!(*default_port, 9090);
            }
            PluginProtocol::Subprocess { .. } => panic!("expected Grpc protocol"),
        }
    }

    #[test]
    fn test_manifest_optional_fields_absent() {
        let json = r#"{
            "name": "orbflow-minimal",
            "version": "0.1.0",
            "description": "Minimal plugin",
            "author": "Test",
            "license": "MIT",
            "node_types": ["plugin:test"],
            "orbflow_version": "0.1.0",
            "protocol": {"Subprocess": {"binary_name": "minimal"}}
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).expect("deserialize");

        assert_eq!(manifest.name, "orbflow-minimal");
        assert!(manifest.repository.is_none());
        assert!(manifest.display.icon.is_none());
        assert!(manifest.display.category.is_none());
        assert!(manifest.readme.is_none());
        assert!(manifest.tags.is_empty());
    }

    #[test]
    fn test_manifest_optional_fields_skipped_in_serialization() {
        let manifest = PluginManifest {
            repository: None,
            display: PluginDisplayHints {
                icon: None,
                category: None,
                color: None,
            },
            readme: None,
            ..sample_manifest()
        };

        let json = serde_json::to_string(&manifest).expect("serialize");
        assert!(!json.contains("repository"));
        assert!(!json.contains("icon"));
        assert!(!json.contains("category"));
        assert!(!json.contains("readme"));
    }

    #[test]
    fn test_plugin_version_serde_roundtrip() {
        let version = PluginVersion {
            name: "orbflow-slack".to_string(),
            version: "1.0.0".to_string(),
            manifest: sample_manifest(),
            checksum: Some("abc123def456".to_string()),
            download_url: Some(
                "https://registry.orbflow.dev/plugins/orbflow-slack/1.0.0".to_string(),
            ),
            downloads: 42,
            published_at: Utc::now(),
        };

        let json = serde_json::to_string(&version).expect("serialize");
        let deserialized: PluginVersion = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.name, "orbflow-slack");
        assert_eq!(deserialized.version, "1.0.0");
        assert_eq!(deserialized.downloads, 42);
        assert!(deserialized.checksum.is_some());
        assert!(deserialized.download_url.is_some());
    }

    #[test]
    fn test_plugin_summary_serde_roundtrip() {
        let summary = PluginSummary {
            name: "orbflow-slack".to_string(),
            description: "Slack integration".to_string(),
            latest_version: "1.0.0".to_string(),
            author: "Orbflow Authors".to_string(),
            downloads: 100,
            tags: vec!["communication".to_string()],
            icon: Some("slack".to_string()),
            category: Some("communication".to_string()),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&summary).expect("serialize");
        let deserialized: PluginSummary = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.name, "orbflow-slack");
        assert_eq!(deserialized.latest_version, "1.0.0");
        assert_eq!(deserialized.downloads, 100);
    }

    #[test]
    fn test_template_manifest_serde_roundtrip() {
        let template = TemplateManifest {
            name: "slack-notification".to_string(),
            description: "Send a Slack notification on webhook trigger".to_string(),
            workflow: serde_json::json!({
                "nodes": [
                    {"id": "trigger", "type": "trigger:webhook"},
                    {"id": "notify", "type": "plugin:slack-send"}
                ],
                "edges": [{"from": "trigger", "to": "notify"}]
            }),
            tags: vec!["slack".to_string(), "notification".to_string()],
            author: "Orbflow Authors".to_string(),
            category: Some("communication".to_string()),
        };

        let json = serde_json::to_string(&template).expect("serialize");
        let deserialized: TemplateManifest = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.name, "slack-notification");
        assert_eq!(deserialized.tags.len(), 2);
        assert!(deserialized.workflow.get("nodes").is_some());
    }
}
