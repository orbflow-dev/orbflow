// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Local plugin index -- tracks installed plugins and resolves versions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::manifest::PluginManifest;

/// Manages locally installed plugins.
pub struct LocalIndex {
    /// Directory where plugins are installed.
    plugins_dir: PathBuf,
    /// Cached index of installed plugins.
    installed: HashMap<String, InstalledPlugin>,
}

/// A locally installed plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub install_path: PathBuf,
    pub installed_at: chrono::DateTime<chrono::Utc>,
}

impl LocalIndex {
    /// Creates a new local index for the given plugins directory.
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            installed: HashMap::new(),
        }
    }

    /// Scans the plugins directory and loads all manifests.
    ///
    /// Recurses up to two levels deep so that plugins can be grouped in
    /// subdirectories (e.g. `plugins/unsloth/unsloth-ai-codegen/`).
    ///
    /// NOTE: Uses blocking I/O (`std::fs`). Call only from sync context or
    /// wrap in `tokio::task::spawn_blocking` if invoked from async code.
    pub fn scan(&mut self) -> Result<(), std::io::Error> {
        self.installed.clear();
        let dir = self.plugins_dir.clone();
        if !dir.exists() {
            return Ok(());
        }

        self.scan_dir(&dir, 0)
    }

    /// Recursively scans a directory for plugin manifests up to `MAX_SCAN_DEPTH`
    /// levels below the plugins root.
    fn scan_dir(&mut self, dir: &Path, depth: u8) -> Result<(), std::io::Error> {
        const MAX_SCAN_DEPTH: u8 = 2;

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("orbflow-plugin.json");
            if manifest_path.exists()
                && let Ok(data) = std::fs::read_to_string(&manifest_path)
                && let Ok(manifest) = serde_json::from_str::<PluginManifest>(&data)
            {
                self.installed.insert(
                    manifest.name.clone(),
                    InstalledPlugin {
                        manifest,
                        install_path: path,
                        installed_at: chrono::Utc::now(),
                    },
                );
            } else if depth < MAX_SCAN_DEPTH {
                self.scan_dir(&path, depth + 1)?;
            }
        }
        Ok(())
    }

    /// Returns all installed plugins.
    pub fn list(&self) -> Vec<&InstalledPlugin> {
        self.installed.values().collect()
    }

    /// Gets an installed plugin by name.
    pub fn get(&self, name: &str) -> Option<&InstalledPlugin> {
        self.installed.get(name)
    }

    /// Checks if a plugin is installed.
    pub fn is_installed(&self, name: &str) -> bool {
        self.installed.contains_key(name)
    }

    /// Returns the plugins directory path.
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }
}

/// Converts an `InstalledPlugin` to a `PluginIndexEntry` carrying all manifest
/// fields the frontend needs for both summary and detail views.
fn to_index_entry(p: &InstalledPlugin) -> orbflow_core::ports::PluginIndexEntry {
    orbflow_core::ports::PluginIndexEntry {
        name: p.manifest.name.clone(),
        version: p.manifest.version.clone(),
        description: Some(p.manifest.description.clone()),
        author: Some(p.manifest.author.clone()),
        tags: p.manifest.tags.clone(),
        icon: p.manifest.display.icon.clone(),
        category: p.manifest.display.category.clone(),
        color: p.manifest.display.color.clone(),
        license: Some(p.manifest.license.clone()),
        repository: p.manifest.repository.clone(),
        node_types: p.manifest.node_types.clone(),
        orbflow_version: Some(p.manifest.orbflow_version.clone()),
        language: p.manifest.language.clone(),
        readme: p.manifest.readme.clone(),
        path: None,
        protocol: None,
        checksum: None,
    }
}

#[async_trait::async_trait]
impl orbflow_core::ports::PluginIndex for LocalIndex {
    async fn list_available(
        &self,
    ) -> Result<Vec<orbflow_core::ports::PluginIndexEntry>, orbflow_core::OrbflowError> {
        Ok(self.list().iter().map(|p| to_index_entry(p)).collect())
    }

    async fn get_entry(
        &self,
        name: &str,
    ) -> Result<Option<orbflow_core::ports::PluginIndexEntry>, orbflow_core::OrbflowError> {
        Ok(self.get(name).map(to_index_entry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PluginProtocol;

    #[test]
    fn test_new_index_is_empty() {
        let index = LocalIndex::new("/tmp/orbflow-test-plugins");
        assert!(index.list().is_empty());
        assert!(!index.is_installed("anything"));
        assert!(index.get("anything").is_none());
    }

    #[test]
    fn test_scan_nonexistent_dir() {
        let mut index = LocalIndex::new("/tmp/orbflow-test-nonexistent-dir-12345");
        let result = index.scan();
        assert!(result.is_ok());
        assert!(index.list().is_empty());
    }

    #[test]
    fn test_scan_loads_manifest() {
        let dir = std::env::temp_dir().join("orbflow-test-scan-manifest");
        let plugin_dir = dir.join("my-plugin");

        // Clean up from previous runs.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = PluginManifest {
            name: "my-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            repository: None,
            node_types: vec!["plugin:test".to_string()],
            orbflow_version: "0.1.0".to_string(),
            protocol: PluginProtocol::Subprocess {
                binary_name: "my-plugin".to_string(),
            },
            tags: vec![],
            display: crate::manifest::PluginDisplayHints::default(),
            language: None,
            readme: None,
            inputs: vec![],
            outputs: vec![],
            parameters: vec![],
        };

        let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
        std::fs::write(plugin_dir.join("orbflow-plugin.json"), manifest_json).unwrap();

        let mut index = LocalIndex::new(&dir);
        index.scan().unwrap();

        assert_eq!(index.list().len(), 1);
        assert!(index.is_installed("my-plugin"));

        let installed = index.get("my-plugin").unwrap();
        assert_eq!(installed.manifest.version, "0.1.0");
        assert_eq!(installed.install_path, plugin_dir);

        // Clean up.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_nested_plugin_dir() {
        let dir = std::env::temp_dir().join("orbflow-test-scan-nested");
        let nested_dir = dir.join("vendor").join("nested-plugin");

        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&nested_dir).unwrap();

        let manifest = PluginManifest {
            name: "nested-plugin".to_string(),
            version: "0.2.0".to_string(),
            description: "Nested test plugin".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            repository: None,
            node_types: vec!["plugin:nested".to_string()],
            orbflow_version: "0.1.0".to_string(),
            protocol: PluginProtocol::Subprocess {
                binary_name: "nested-plugin".to_string(),
            },
            tags: vec![],
            display: crate::manifest::PluginDisplayHints::default(),
            language: None,
            readme: None,
            inputs: vec![],
            outputs: vec![],
            parameters: vec![],
        };

        let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
        std::fs::write(nested_dir.join("orbflow-plugin.json"), manifest_json).unwrap();

        let mut index = LocalIndex::new(&dir);
        index.scan().unwrap();

        assert_eq!(index.list().len(), 1);
        assert!(index.is_installed("nested-plugin"));

        let installed = index.get("nested-plugin").unwrap();
        assert_eq!(installed.manifest.version, "0.2.0");
        assert_eq!(installed.install_path, nested_dir);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_plugins_dir_accessor() {
        let index = LocalIndex::new("/some/path");
        assert_eq!(index.plugins_dir(), Path::new("/some/path"));
    }
}
