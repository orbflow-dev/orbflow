// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Community plugin index client.
//!
//! Fetches the plugin index from a GitHub-hosted `orbflow-plugins` repository.
//! The index is a single `plugins.json` file listing all community plugins with
//! metadata and GitHub Releases download URLs. Results are cached in memory
//! with a 5-minute TTL to avoid hitting GitHub on every API request.
//!
//! # How It Works
//!
//! 1. A GitHub repo (e.g., `orbflow-dev/orbflow-plugins`) hosts `plugins.json`
//! 2. Plugin authors submit PRs to add/update their entry
//! 3. This client fetches the raw JSON via GitHub's CDN (with TTL caching)
//! 4. Users browse plugins in the Marketplace tab
//! 5. Install downloads the binary from the plugin's GitHub Releases

use std::time::{Duration, Instant};

use crate::manifest::PluginSummary;

/// Default community index URL (raw GitHub content).
pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/orbflow-dev/orbflow-plugins/master/plugins.json";

/// How long the cached index is considered fresh.
const CACHE_TTL: Duration = Duration::from_secs(300);

/// Maximum size of the plugin index response (5 MB).
const MAX_INDEX_BYTES: usize = 5 * 1024 * 1024;

/// Client for fetching the community plugin index from GitHub.
///
/// Maintains an in-memory TTL cache to avoid hitting GitHub on every request.
pub struct CommunityIndex {
    http: reqwest::Client,
    index_url: String,
    cache: tokio::sync::RwLock<Option<(Instant, Vec<CommunityPlugin>)>>,
}

/// A plugin entry in the community index (plugins.json).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommunityPlugin {
    /// Plugin name (unique identifier).
    pub name: String,
    /// Latest version tag (e.g., "1.2.0").
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// Plugin author.
    pub author: String,
    /// Category for filtering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Tags for search.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Icon name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Theme color (hex, e.g., "#8B5CF6").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Node types this plugin provides.
    #[serde(default)]
    pub node_types: Vec<String>,
    /// Plugin protocol (e.g., "grpc", "subprocess").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// Implementation language (e.g., "python", "rust").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Path within the plugins repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// GitHub repository (e.g., "username/orbflow-slack"). Optional for community plugins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// License (SPDX identifier).
    #[serde(default)]
    pub license: String,
    /// Minimum compatible Orbflow version.
    #[serde(default)]
    pub orbflow_version: String,
    /// Download count (community-reported or tracked via GitHub API).
    #[serde(default)]
    pub downloads: u64,
    /// SHA-256 checksum of the tarball (optional, verified on download).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

impl From<&CommunityPlugin> for PluginSummary {
    fn from(p: &CommunityPlugin) -> Self {
        Self {
            name: p.name.clone(),
            description: p.description.clone(),
            latest_version: p.version.clone(),
            author: p.author.clone(),
            downloads: p.downloads,
            tags: p.tags.clone(),
            icon: p.icon.clone(),
            category: p.category.clone(),
            updated_at: chrono::Utc::now(),
        }
    }
}

impl CommunityIndex {
    /// Creates a client using the default community index URL and a new HTTP client.
    pub fn new() -> Self {
        // DEFAULT_INDEX_URL is a compile-time https:// constant, always valid.
        Self::with_url(DEFAULT_INDEX_URL).expect("DEFAULT_INDEX_URL is always valid")
    }

    /// Creates a client with a custom index URL and a new HTTP client.
    ///
    /// Only `https://` URLs are accepted to prevent SSRF.
    pub fn with_url(url: impl Into<String>) -> Result<Self, RegistryError> {
        Self::with_url_and_client(url, reqwest::Client::new())
    }

    /// Creates a client with a custom index URL and a shared HTTP client.
    ///
    /// Only `https://` URLs are accepted to prevent SSRF.
    pub fn with_url_and_client(
        url: impl Into<String>,
        http: reqwest::Client,
    ) -> Result<Self, RegistryError> {
        let url = url.into();
        if !url.starts_with("https://") {
            return Err(RegistryError::InvalidUrl(
                "only https:// URLs are accepted".into(),
            ));
        }
        Ok(Self {
            http,
            index_url: url,
            cache: tokio::sync::RwLock::new(None),
        })
    }

    /// Returns the cached index if fresh, otherwise fetches from GitHub.
    ///
    /// Uses a write lock for the entire refresh path to prevent cache stampede:
    /// only one concurrent caller fetches from GitHub while others wait.
    async fn cached_index(&self) -> Result<Vec<CommunityPlugin>, RegistryError> {
        // Fast path: read lock.
        {
            let guard = self.cache.read().await;
            if let Some((fetched_at, plugins)) = guard.as_ref()
                && fetched_at.elapsed() < CACHE_TTL
            {
                return Ok(plugins.clone());
            }
        }

        // Slow path: acquire write lock and double-check (another task may
        // have refreshed while we waited for the write lock).
        let mut guard = self.cache.write().await;
        if let Some((fetched_at, plugins)) = guard.as_ref()
            && fetched_at.elapsed() < CACHE_TTL
        {
            return Ok(plugins.clone());
        }

        let plugins = self.fetch_index().await?;
        *guard = Some((Instant::now(), plugins.clone()));
        Ok(plugins)
    }

    /// Fetches the full community plugin index from GitHub (bypasses cache).
    ///
    /// Prefer [`cached_index`] for normal use.
    pub async fn fetch_index(&self) -> Result<Vec<CommunityPlugin>, RegistryError> {
        let mut req = self
            .http
            .get(&self.index_url)
            .header("User-Agent", "orbflow-registry")
            .timeout(Duration::from_secs(15));

        // Optional GitHub token for higher rate limits (5,000/hr vs 60/hr).
        if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")) {
            req = req.header("Authorization", format!("Bearer {token}"));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| RegistryError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(RegistryError::Server(format!(
                "index returned {}",
                resp.status()
            )));
        }

        // Reject obviously oversized responses before buffering.
        if let Some(len) = resp.content_length()
            && len > MAX_INDEX_BYTES as u64
        {
            return Err(RegistryError::Parse(format!(
                "index too large: {len} bytes (max {MAX_INDEX_BYTES})",
            )));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| RegistryError::Network(e.to_string()))?;
        if bytes.len() > MAX_INDEX_BYTES {
            return Err(RegistryError::Parse(format!(
                "index too large: {} bytes (max {})",
                bytes.len(),
                MAX_INDEX_BYTES
            )));
        }

        let plugins: Vec<CommunityPlugin> =
            serde_json::from_slice(&bytes).map_err(|e| RegistryError::Parse(e.to_string()))?;

        Ok(plugins)
    }

    /// Fetches the index (cached) and converts to PluginSummary list.
    pub async fn list_plugins(&self) -> Result<Vec<PluginSummary>, RegistryError> {
        let plugins = self.cached_index().await?;
        Ok(plugins.iter().map(PluginSummary::from).collect())
    }

    /// Search plugins by query (client-side filtering).
    pub fn filter(
        plugins: &[CommunityPlugin],
        query: &str,
        category: Option<&str>,
    ) -> Vec<CommunityPlugin> {
        let query_lower = query.to_lowercase();
        plugins
            .iter()
            .filter(|p| {
                // Category filter
                if let Some(cat) = category
                    && p.category.as_deref() != Some(cat)
                {
                    return false;
                }
                // Text search
                if query_lower.is_empty() {
                    return true;
                }
                p.name.to_lowercase().contains(&query_lower)
                    || p.description.to_lowercase().contains(&query_lower)
                    || p.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
                    || p.author.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    /// Returns the GitHub Releases download URL for a plugin binary.
    ///
    /// Format: `https://github.com/{repo}/releases/download/v{version}/{binary_name}`
    ///
    /// Returns `None` if `repo` or `binary_name` contain invalid characters.
    /// Returns the GitHub Releases download URL for a plugin binary.
    ///
    /// Requires the plugin to have a `repo` field set (e.g., "username/orbflow-slack").
    /// Returns `None` if `repo` is absent or any field contains invalid characters.
    pub fn download_url(plugin: &CommunityPlugin, binary_name: &str) -> Option<String> {
        let repo = plugin.repo.as_deref()?;

        // Validate repo format: "owner/repo" with safe characters only.
        let valid_repo = repo.split('/').count() == 2
            && repo
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/');
        let valid_binary = binary_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
        let valid_version = plugin
            .version
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-');

        if !valid_repo || !valid_binary || !valid_version {
            return None;
        }

        Some(format!(
            "https://github.com/{repo}/releases/download/v{}/{binary_name}",
            plugin.version
        ))
    }
}

impl Default for CommunityIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts a [`CommunityPlugin`] into a [`PluginIndexEntry`] for the
/// marketplace API surface.
fn to_index_entry(p: &CommunityPlugin) -> orbflow_core::ports::PluginIndexEntry {
    orbflow_core::ports::PluginIndexEntry {
        name: p.name.clone(),
        version: p.version.clone(),
        description: Some(p.description.clone()),
        author: Some(p.author.clone()),
        tags: p.tags.clone(),
        icon: p.icon.clone(),
        category: p.category.clone(),
        color: p.color.clone(),
        license: if p.license.is_empty() {
            None
        } else {
            Some(p.license.clone())
        },
        repository: p.repo.clone(),
        node_types: p.node_types.clone(),
        orbflow_version: if p.orbflow_version.is_empty() {
            None
        } else {
            Some(p.orbflow_version.clone())
        },
        language: p.language.clone(),
        readme: None,
        path: p.path.clone(),
        protocol: p.protocol.clone(),
        checksum: p.checksum.clone(),
    }
}

#[async_trait::async_trait]
impl orbflow_core::ports::PluginIndex for CommunityIndex {
    async fn list_available(
        &self,
    ) -> Result<Vec<orbflow_core::ports::PluginIndexEntry>, orbflow_core::OrbflowError> {
        let plugins = self.cached_index().await.map_err(|e| {
            orbflow_core::OrbflowError::Internal(format!("community index fetch failed: {e}"))
        })?;
        Ok(plugins.iter().map(to_index_entry).collect())
    }

    async fn get_entry(
        &self,
        name: &str,
    ) -> Result<Option<orbflow_core::ports::PluginIndexEntry>, orbflow_core::OrbflowError> {
        let plugins = self.cached_index().await.map_err(|e| {
            orbflow_core::OrbflowError::Internal(format!("community index fetch failed: {e}"))
        })?;
        Ok(plugins.iter().find(|p| p.name == name).map(to_index_entry))
    }
}

/// Errors from community index operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("network error: {0}")]
    Network(String),
    #[error("server error: {0}")]
    Server(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    /// Local filesystem I/O error (disk full, permission denied, etc.).
    #[error("I/O error: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a test plugin with sensible defaults.
    fn test_plugin(name: &str) -> CommunityPlugin {
        CommunityPlugin {
            name: name.into(),
            version: "1.0.0".into(),
            description: "Test plugin".into(),
            author: "tester".into(),
            category: None,
            tags: vec![],
            icon: None,
            color: None,
            node_types: vec![],
            protocol: None,
            language: None,
            path: None,
            repo: None,
            license: String::new(),
            orbflow_version: String::new(),
            downloads: 0,
            checksum: None,
        }
    }

    #[test]
    fn test_community_plugin_serde_monorepo_format() {
        // Real format from orbflow-dev/orbflow-plugins
        let json = r##"{
            "name": "orbflow-uuid-gen",
            "version": "0.2.0",
            "description": "Generate UUID v4 identifiers",
            "author": "Orbflow",
            "category": "utility",
            "tags": ["uuid", "id"],
            "icon": "hash",
            "color": "#8B5CF6",
            "node_types": ["plugin:uuid-gen"],
            "protocol": "grpc",
            "language": "python",
            "path": "python/orbflow/orbflow-uuid-gen"
        }"##;
        let plugin: CommunityPlugin = serde_json::from_str(json).unwrap();
        assert_eq!(plugin.name, "orbflow-uuid-gen");
        assert_eq!(plugin.language.as_deref(), Some("python"));
        assert_eq!(plugin.color.as_deref(), Some("#8B5CF6"));
        assert!(plugin.repo.is_none());
        assert_eq!(plugin.node_types.len(), 1);
    }

    #[test]
    fn test_community_plugin_serde_with_repo() {
        // Community format with repo field
        let json = r#"{
            "name": "orbflow-slack",
            "version": "1.2.0",
            "description": "Send Slack messages",
            "author": "johndoe",
            "repo": "johndoe/orbflow-slack",
            "license": "MIT",
            "node_types": ["plugin:slack-send"],
            "category": "communication",
            "tags": ["slack"]
        }"#;
        let plugin: CommunityPlugin = serde_json::from_str(json).unwrap();
        assert_eq!(plugin.name, "orbflow-slack");
        assert_eq!(plugin.repo.as_deref(), Some("johndoe/orbflow-slack"));
    }

    #[test]
    fn test_filter_by_query() {
        let mut slack = test_plugin("orbflow-slack");
        slack.description = "Send Slack messages".into();
        slack.author = "alice".into();
        slack.category = Some("communication".into());
        slack.tags = vec!["slack".into()];

        let mut pg = test_plugin("orbflow-postgres");
        pg.description = "Advanced PostgreSQL operations".into();
        pg.author = "bob".into();
        pg.category = Some("database".into());
        pg.tags = vec!["sql".into()];

        let plugins = vec![slack, pg];

        let results = CommunityIndex::filter(&plugins, "slack", None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "orbflow-slack");

        let results = CommunityIndex::filter(&plugins, "", Some("database"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "orbflow-postgres");

        let results = CommunityIndex::filter(&plugins, "", None);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_download_url() {
        let mut plugin = test_plugin("orbflow-slack");
        plugin.repo = Some("johndoe/orbflow-slack".into());
        plugin.version = "1.2.0".into();

        let url = CommunityIndex::download_url(&plugin, "orbflow-slack-x86_64-linux").unwrap();
        assert_eq!(
            url,
            "https://github.com/johndoe/orbflow-slack/releases/download/v1.2.0/orbflow-slack-x86_64-linux"
        );

        // No repo returns None
        let no_repo = test_plugin("no-repo");
        assert!(CommunityIndex::download_url(&no_repo, "binary").is_none());

        // Invalid repo returns None
        let mut bad = plugin.clone();
        bad.repo = Some("../../etc/passwd".into());
        assert!(CommunityIndex::download_url(&bad, "binary").is_none());
    }

    #[test]
    fn test_to_plugin_summary() {
        let mut plugin = test_plugin("test-plugin");
        plugin.version = "0.5.0".into();
        plugin.downloads = 42;

        let summary = PluginSummary::from(&plugin);
        assert_eq!(summary.name, "test-plugin");
        assert_eq!(summary.latest_version, "0.5.0");
        assert_eq!(summary.downloads, 42);
    }

    #[test]
    fn test_with_url_rejects_non_https() {
        let result = CommunityIndex::with_url("http://evil.com/plugins.json");
        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            RegistryError::InvalidUrl(_)
        ));
    }

    #[test]
    fn test_with_url_accepts_https() {
        assert!(CommunityIndex::with_url("https://example.com/plugins.json").is_ok());
    }

    #[test]
    fn test_to_index_entry_empty_strings_become_none() {
        let plugin = test_plugin("test");
        let entry = to_index_entry(&plugin);
        assert!(entry.license.is_none());
        assert!(entry.orbflow_version.is_none());
    }

    #[test]
    fn test_to_index_entry_maps_new_fields() {
        let mut plugin = test_plugin("mapped");
        plugin.color = Some("#FF0000".into());
        plugin.language = Some("python".into());

        let entry = to_index_entry(&plugin);
        assert_eq!(entry.color.as_deref(), Some("#FF0000"));
        assert_eq!(entry.language.as_deref(), Some("python"));
    }
}
