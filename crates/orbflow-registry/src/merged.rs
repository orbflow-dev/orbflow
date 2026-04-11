// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Merged plugin index that combines local (installed) and community (remote) sources.
//!
//! Local plugins take precedence when names collide (the locally installed
//! version is authoritative). Community index failures degrade gracefully
//! with a warning log — the marketplace still shows local plugins.

use std::collections::HashSet;
use std::sync::Arc;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{PluginIndex, PluginIndexEntry, PluginInstaller};

/// A [`PluginIndex`] that queries a local index first, then enriches with
/// community plugins that are not yet installed locally.
pub struct MergedIndex {
    local: Arc<dyn PluginIndex>,
    community: Arc<dyn PluginIndex>,
    http_client: reqwest::Client,
}

impl MergedIndex {
    /// Creates a new merged index. Pass a shared `reqwest::Client` for
    /// connection pool reuse across the application.
    pub fn new(
        local: Arc<dyn PluginIndex>,
        community: Arc<dyn PluginIndex>,
        http_client: reqwest::Client,
    ) -> Self {
        Self {
            local,
            community,
            http_client,
        }
    }
}

#[async_trait::async_trait]
impl PluginIndex for MergedIndex {
    async fn list_available(&self) -> Result<Vec<PluginIndexEntry>, OrbflowError> {
        let (local_result, community_result) =
            tokio::join!(self.local.list_available(), self.community.list_available(),);

        // Local failure is a hard error — installed plugins must be visible.
        let local = local_result?;

        // Community failure degrades gracefully — log and continue with local only.
        let community = match community_result {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(error = %e, "community index unavailable, showing local only");
                vec![]
            }
        };

        let local_names: HashSet<String> = local.iter().map(|p| p.name.clone()).collect();
        let mut result = local;
        for entry in community {
            if !local_names.contains(entry.name.as_str()) {
                result.push(entry);
            }
        }
        Ok(result)
    }

    async fn get_entry(&self, name: &str) -> Result<Option<PluginIndexEntry>, OrbflowError> {
        // Local takes precedence.
        if let Some(entry) = self.local.get_entry(name).await? {
            return Ok(Some(entry));
        }
        // Fall through to community.
        self.community.get_entry(name).await
    }
}

#[async_trait::async_trait]
impl PluginInstaller for MergedIndex {
    async fn install_plugin(
        &self,
        name: &str,
        dest: &std::path::Path,
    ) -> Result<usize, OrbflowError> {
        // Look up the plugin entry from the index.
        let entry = self.get_entry(name).await?.ok_or(OrbflowError::NotFound)?;

        // If plugin is already installed locally, compare versions.
        let manifest_path = dest.join("orbflow-plugin.json");
        let local_manifest = {
            let mp = manifest_path.clone();
            tokio::task::spawn_blocking(move || {
                if mp.exists() {
                    std::fs::read_to_string(&mp)
                        .ok()
                        .and_then(|data| serde_json::from_str::<serde_json::Value>(&data).ok())
                } else {
                    None
                }
            })
            .await
            .unwrap_or(None)
        };
        if let Some(local) = local_manifest {
            let local_ver = local["version"].as_str().unwrap_or("");
            if local_ver == entry.version {
                tracing::info!(
                    plugin = %name,
                    version = %local_ver,
                    "plugin already installed at latest version, skipping download"
                );
                return Ok(0);
            }
            tracing::info!(
                plugin = %name,
                local_version = %local_ver,
                remote_version = %entry.version,
                "upgrading plugin to newer version"
            );
        }

        let plugin_path = entry.path.as_deref().unwrap_or("");
        if plugin_path.is_empty() {
            // No path — create directory only (manual setup).
            let dest_owned = dest.to_path_buf();
            tokio::task::spawn_blocking(move || std::fs::create_dir_all(&dest_owned))
                .await
                .map_err(|e| OrbflowError::Internal(format!("spawn failed: {e}")))?
                .map_err(|e| {
                    OrbflowError::InvalidNodeConfig(format!("failed to create plugin dir: {e}"))
                })?;
            return Ok(0);
        }

        // Require a checksum for all community plugin downloads — integrity
        // verification is mandatory; index entries without one are rejected.
        let checksum = entry.checksum.as_deref().ok_or_else(|| {
            OrbflowError::InvalidNodeConfig(format!(
                "plugin '{name}' cannot be installed: missing checksum in index entry; \
                 integrity verification is required for community plugins"
            ))
        })?;

        // Resolve the GitHub repo: use plugin's repo or fall back to official monorepo.
        let repo = entry
            .repository
            .as_deref()
            .and_then(|r| r.strip_prefix("https://github.com/"))
            .or(entry.repository.as_deref())
            .unwrap_or("orbflow-dev/orbflow-plugins");

        // D7 fix: use DEFAULT_REF from install module instead of hardcoded "master".
        crate::install::download_plugin_from(
            &self.http_client,
            repo,
            crate::install::DEFAULT_REF,
            plugin_path,
            dest,
            Some(checksum),
        )
        .await
        .map_err(|e| OrbflowError::Internal(format!("plugin download failed: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple in-memory PluginIndex for testing.
    struct StubIndex {
        entries: Vec<PluginIndexEntry>,
        fail: bool,
    }

    impl StubIndex {
        fn new(entries: Vec<PluginIndexEntry>) -> Self {
            Self {
                entries,
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                entries: vec![],
                fail: true,
            }
        }
    }

    #[async_trait::async_trait]
    impl PluginIndex for StubIndex {
        async fn list_available(&self) -> Result<Vec<PluginIndexEntry>, OrbflowError> {
            if self.fail {
                return Err(OrbflowError::Internal("stub failure".into()));
            }
            Ok(self.entries.clone())
        }

        async fn get_entry(&self, name: &str) -> Result<Option<PluginIndexEntry>, OrbflowError> {
            if self.fail {
                return Err(OrbflowError::Internal("stub failure".into()));
            }
            Ok(self.entries.iter().find(|e| e.name == name).cloned())
        }
    }

    fn make_entry(name: &str) -> PluginIndexEntry {
        PluginIndexEntry {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: Some("test".to_string()),
            author: Some("test".to_string()),
            tags: vec![],
            icon: None,
            category: None,
            color: None,
            license: None,
            repository: None,
            node_types: vec![],
            orbflow_version: None,
            language: None,
            readme: None,
            path: None,
            protocol: None,
            checksum: None,
        }
    }

    #[tokio::test]
    async fn both_empty_returns_empty() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![])),
            Arc::new(StubIndex::new(vec![])),
            reqwest::Client::new(),
        );
        let result = merged.list_available().await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn local_only() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![make_entry("local-plugin")])),
            Arc::new(StubIndex::new(vec![])),
            reqwest::Client::new(),
        );
        let result = merged.list_available().await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "local-plugin");
    }

    #[tokio::test]
    async fn community_only() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![])),
            Arc::new(StubIndex::new(vec![make_entry("community-plugin")])),
            reqwest::Client::new(),
        );
        let result = merged.list_available().await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "community-plugin");
    }

    #[tokio::test]
    async fn local_takes_precedence_on_collision() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![make_entry("shared")])),
            Arc::new(StubIndex::new(vec![make_entry("shared")])),
            reqwest::Client::new(),
        );
        let result = merged.list_available().await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn merges_without_duplicates() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![make_entry("local")])),
            Arc::new(StubIndex::new(vec![make_entry("community")])),
            reqwest::Client::new(),
        );
        let result = merged.list_available().await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn community_failure_degrades_gracefully() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![make_entry("local")])),
            Arc::new(StubIndex::failing()),
            reqwest::Client::new(),
        );
        let result = merged.list_available().await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "local");
    }

    #[tokio::test]
    async fn local_failure_propagates_error() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::failing()),
            Arc::new(StubIndex::new(vec![make_entry("community")])),
            reqwest::Client::new(),
        );
        assert!(merged.list_available().await.is_err());
    }

    #[tokio::test]
    async fn get_entry_prefers_local() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![make_entry("plugin")])),
            Arc::new(StubIndex::new(vec![make_entry("plugin")])),
            reqwest::Client::new(),
        );
        assert!(merged.get_entry("plugin").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn get_entry_falls_through_to_community() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![])),
            Arc::new(StubIndex::new(vec![make_entry("community")])),
            reqwest::Client::new(),
        );
        assert!(merged.get_entry("community").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn get_entry_not_found() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![])),
            Arc::new(StubIndex::new(vec![])),
            reqwest::Client::new(),
        );
        assert!(merged.get_entry("nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn install_plugin_not_found_returns_error() {
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![])),
            Arc::new(StubIndex::new(vec![])),
            reqwest::Client::new(),
        );
        let dest = std::env::temp_dir().join("orbflow-test-install-notfound");
        let result = merged.install_plugin("nonexistent", &dest).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn install_plugin_skips_if_same_version_installed() {
        let dest = std::env::temp_dir().join("orbflow-test-install-skip");
        let _ = std::fs::create_dir_all(&dest);

        // Write a local manifest with version 1.0.0
        let manifest = serde_json::json!({"version": "1.0.0", "name": "test-plugin"});
        std::fs::write(
            dest.join("orbflow-plugin.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        // Index also has version 1.0.0
        let entry = make_entry("test-plugin");
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![entry.clone()])),
            Arc::new(StubIndex::new(vec![])),
            reqwest::Client::new(),
        );

        let result = merged.install_plugin("test-plugin", &dest).await.unwrap();
        assert_eq!(result, 0); // skipped

        // Cleanup
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[tokio::test]
    async fn install_plugin_creates_dir_when_no_path() {
        let dest = std::env::temp_dir().join("orbflow-test-install-nopath");
        let _ = std::fs::remove_dir_all(&dest); // ensure clean

        // Entry with no path — should just create the directory
        let entry = make_entry("empty-plugin");
        let merged = MergedIndex::new(
            Arc::new(StubIndex::new(vec![entry])),
            Arc::new(StubIndex::new(vec![])),
            reqwest::Client::new(),
        );

        let result = merged.install_plugin("empty-plugin", &dest).await.unwrap();
        assert_eq!(result, 0);
        assert!(dest.exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&dest);
    }
}
