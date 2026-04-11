// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Plugin installation from a GitHub monorepo.
//!
//! Downloads the repository tarball via the GitHub Archive API, then extracts
//! only the files under the plugin's `path` prefix into the local plugins
//! directory. This is a single HTTP request regardless of plugin file count.
//!
//! Installation is **atomic**: files are extracted to a temporary directory
//! first, then renamed to the final destination. If anything fails, the
//! temporary directory is cleaned up and the existing installation is untouched.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::client::RegistryError;

/// Maximum tarball size (50 MB) to prevent abuse.
const MAX_TARBALL_BYTES: usize = 50 * 1024 * 1024;

/// Maximum number of files to extract per plugin (safety limit).
const MAX_FILES_PER_PLUGIN: usize = 500;

/// Default GitHub repository for the plugin monorepo.
const DEFAULT_REPO: &str = "orbflow-dev/orbflow-plugins";

/// Default git ref (branch or tag) to download from.
///
/// Can be overridden via the `ORBFLOW_PLUGIN_GIT_REF` environment variable.
pub const DEFAULT_REF: &str = "master";

/// Downloads and extracts a plugin from the GitHub monorepo.
///
/// # Arguments
/// * `http` — shared reqwest client
/// * `plugin_path` — path within the monorepo (e.g., `python/orbflow/orbflow-uuid-gen`)
/// * `dest` — local directory to extract into (e.g., `./plugins/orbflow-uuid-gen/`)
///
/// # How it works
/// 1. Fetches the repo tarball from `https://api.github.com/repos/{repo}/tarball/{ref}`
/// 2. Decompresses (gzip) and iterates tar entries
/// 3. Extracts only entries whose path starts with `plugin_path`
/// 4. Strips the monorepo prefix so files land directly in `dest`
///
/// Installation is atomic: extraction happens in a `.tmp-{name}-{uuid}` dir,
/// then renamed to `dest` on success. Partial extractions are cleaned up.
pub async fn download_plugin(
    http: &reqwest::Client,
    plugin_path: &str,
    dest: &Path,
) -> Result<usize, RegistryError> {
    download_plugin_from(http, DEFAULT_REPO, DEFAULT_REF, plugin_path, dest, None).await
}

/// Downloads and extracts a plugin from a specific GitHub repo and ref.
///
/// If `expected_checksum` is provided, the downloaded tarball's SHA-256 hash
/// is compared against it. A mismatch produces a `RegistryError::Parse`.
pub async fn download_plugin_from(
    http: &reqwest::Client,
    repo: &str,
    git_ref: &str,
    plugin_path: &str,
    dest: &Path,
    expected_checksum: Option<&str>,
) -> Result<usize, RegistryError> {
    // Validate plugin_path: no path traversal.
    if plugin_path.contains("..") || plugin_path.starts_with('/') {
        return Err(RegistryError::Parse(
            "invalid plugin path: must not contain '..' or start with '/'".into(),
        ));
    }

    // Optional GitHub token for authenticated requests (5,000 req/hr vs 60/hr).
    let github_token = std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .ok();

    // Try the direct codeload URL first (avoids a 302 redirect hop),
    // then fall back to the GitHub API tarball endpoint.
    let direct_url =
        format!("https://codeload.github.com/{repo}/legacy.tar.gz/refs/heads/{git_ref}");
    let api_url = format!("https://api.github.com/repos/{repo}/tarball/{git_ref}");

    let direct_result = {
        let mut req = http
            .get(&direct_url)
            .header("User-Agent", "orbflow-registry")
            .timeout(Duration::from_secs(120));
        if let Some(ref token) = github_token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        req.send().await
    };
    let resp = match direct_result {
        Ok(r) if r.status().is_success() => {
            tracing::debug!(url = %direct_url, "downloading tarball via codeload");
            r
        }
        _ => {
            // Fallback to API endpoint (follows 302 redirect).
            tracing::debug!(url = %api_url, "codeload unavailable, falling back to API");
            let mut req = http
                .get(&api_url)
                .header("User-Agent", "orbflow-registry")
                .header("Accept", "application/vnd.github+json")
                .timeout(Duration::from_secs(120));
            if let Some(ref token) = github_token {
                req = req.header("Authorization", format!("Bearer {token}"));
            }
            req.send()
                .await
                .map_err(|e| RegistryError::Network(e.to_string()))?
        }
    };

    if !resp.status().is_success() {
        return Err(RegistryError::Server(format!(
            "tarball download returned {}",
            resp.status()
        )));
    }

    // Reject oversized responses early.
    if let Some(len) = resp.content_length()
        && len > MAX_TARBALL_BYTES as u64
    {
        return Err(RegistryError::Parse(format!(
            "tarball too large: {len} bytes (max {MAX_TARBALL_BYTES})",
        )));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| RegistryError::Network(e.to_string()))?;

    if bytes.len() > MAX_TARBALL_BYTES {
        return Err(RegistryError::Parse(format!(
            "tarball too large: {} bytes (max {MAX_TARBALL_BYTES})",
            bytes.len()
        )));
    }

    // Checksum verification.
    if let Some(expected) = expected_checksum {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = format!("{:x}", hasher.finalize());
        if actual != expected {
            return Err(RegistryError::Parse(format!(
                "checksum mismatch: expected {expected}, got {actual}"
            )));
        }
        tracing::debug!(checksum = %actual, "tarball checksum verified");
    }

    // Extract in a blocking task (tar + flate2 are synchronous).
    let plugin_path = plugin_path.to_string();
    let dest = dest.to_path_buf();
    // D6 fix: consume Bytes into Vec<u8> directly instead of copying.
    let bytes_vec: Vec<u8> = bytes.into();

    tokio::task::spawn_blocking(move || extract_plugin_atomic(&bytes_vec, &plugin_path, &dest))
        .await
        .map_err(|e| RegistryError::Network(format!("extraction task failed: {e}")))?
}

/// Cleans up partial installations left behind by crashed installs.
///
/// Scans `plugins_dir` for directories starting with `.tmp-` and removes them.
/// Call this once at server startup.
pub fn cleanup_partial_installs(plugins_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(plugins_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(".tmp-") && entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            tracing::info!(dir = %name_str, "removing partial plugin installation");
            if let Err(e) = std::fs::remove_dir_all(entry.path()) {
                tracing::warn!(dir = %name_str, error = %e, "failed to remove partial install dir");
            }
        }
    }
}

/// Atomic extraction: extract to a temp dir, then rename to final dest.
fn extract_plugin_atomic(
    tarball: &[u8],
    plugin_path: &str,
    dest: &Path,
) -> Result<usize, RegistryError> {
    // Create a temp directory next to the final destination.
    let parent = dest
        .parent()
        .ok_or_else(|| RegistryError::Parse("destination has no parent directory".into()))?;

    let dest_name = dest
        .file_name()
        .ok_or_else(|| RegistryError::Parse("destination has no file name".into()))?
        .to_string_lossy();

    let tmp_name = format!(".tmp-{}-{}", dest_name, uuid::Uuid::new_v4().as_simple());
    let tmp_dir = parent.join(&tmp_name);

    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| RegistryError::Io(format!("failed to create temp dir: {e}")))?;

    // Extract into the temp directory.
    match extract_plugin(tarball, plugin_path, &tmp_dir) {
        Ok(count) => {
            // Remove the existing destination if it exists.
            if dest.exists() {
                std::fs::remove_dir_all(dest).map_err(|e| {
                    // Clean up temp dir on failure.
                    let _ = std::fs::remove_dir_all(&tmp_dir);
                    RegistryError::Io(format!("failed to remove existing plugin dir: {e}"))
                })?;
            }

            // Rename temp dir to final destination.
            std::fs::rename(&tmp_dir, dest).map_err(|e| {
                // Clean up temp dir on failure.
                let _ = std::fs::remove_dir_all(&tmp_dir);
                RegistryError::Io(format!("failed to rename temp dir to dest: {e}"))
            })?;

            Ok(count)
        }
        Err(e) => {
            // Clean up temp dir on extraction failure.
            let _ = std::fs::remove_dir_all(&tmp_dir);
            Err(e)
        }
    }
}

/// Synchronous tarball extraction — runs inside `spawn_blocking`.
fn extract_plugin(tarball: &[u8], plugin_path: &str, dest: &Path) -> Result<usize, RegistryError> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let decoder = GzDecoder::new(tarball);
    let mut archive = Archive::new(decoder);

    // Normalize the plugin path for prefix matching.
    let prefix = if plugin_path.ends_with('/') {
        plugin_path.to_string()
    } else {
        format!("{plugin_path}/")
    };

    std::fs::create_dir_all(dest)
        .map_err(|e| RegistryError::Io(format!("failed to create dest dir: {e}")))?;

    // Pre-compute canonical dest once (instead of per entry).
    // dest was created by create_dir_all above, so canonicalize must succeed.
    let canonical_dest = std::fs::canonicalize(dest)
        .map_err(|e| RegistryError::Parse(format!("cannot canonicalize dest: {e}")))?;

    let mut extracted = 0usize;

    for entry in archive
        .entries()
        .map_err(|e| RegistryError::Parse(format!("failed to read tarball entries: {e}")))?
    {
        let mut entry =
            entry.map_err(|e| RegistryError::Parse(format!("failed to read entry: {e}")))?;

        let entry_path = entry
            .path()
            .map_err(|e| RegistryError::Parse(format!("invalid entry path: {e}")))?
            .to_path_buf();

        // GitHub tarball structure: first component is `{owner}-{repo}-{short_sha}/`
        // Strip it to get the repo-relative path.
        let components: Vec<_> = entry_path.components().collect();
        if components.len() < 2 {
            continue;
        }
        let relative: PathBuf = components[1..].iter().collect();
        let relative_str = relative.to_string_lossy().replace('\\', "/");

        // Check if this entry is under the plugin path.
        if !relative_str.starts_with(&prefix) && relative_str != plugin_path {
            continue;
        }

        // Strip the plugin path prefix to get the file's position within the plugin dir.
        let inner_str = relative_str
            .strip_prefix(&prefix)
            .or_else(|| relative_str.strip_prefix(plugin_path))
            .unwrap_or("");

        if inner_str.is_empty() && entry.header().entry_type().is_dir() {
            // The plugin root directory itself — already created above.
            continue;
        }

        let target = dest.join(inner_str);

        // Security: ensure target stays within dest (no path traversal).
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let canonical_target = if target.exists() {
            std::fs::canonicalize(&target).map_err(|e| {
                RegistryError::Parse(format!(
                    "cannot canonicalize target {}: {e}",
                    target.display()
                ))
            })?
        } else if let Some(parent) = target.parent() {
            // Parent was created by create_dir_all above, so canonicalize must succeed.
            let cp = std::fs::canonicalize(parent).map_err(|e| {
                RegistryError::Parse(format!(
                    "cannot canonicalize parent {}: {e}",
                    parent.display()
                ))
            })?;
            cp.join(
                target
                    .file_name()
                    .ok_or_else(|| RegistryError::Parse("entry has no filename".into()))?,
            )
        } else {
            return Err(RegistryError::Parse(format!(
                "cannot resolve target path: {}",
                target.display()
            )));
        };

        if !canonical_target.starts_with(&canonical_dest) {
            tracing::warn!(
                path = %relative_str,
                "skipping entry outside plugin directory"
            );
            continue;
        }

        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&target)
                .map_err(|e| RegistryError::Io(format!("failed to create dir: {e}")))?;
        } else if entry.header().entry_type().is_file() {
            entry
                .unpack(&target)
                .map_err(|e| RegistryError::Io(format!("failed to extract file: {e}")))?;
            extracted += 1;

            if extracted > MAX_FILES_PER_PLUGIN {
                return Err(RegistryError::Parse(format!(
                    "too many files in plugin (max {MAX_FILES_PER_PLUGIN})"
                )));
            }
        }
        // Skip symlinks and other entry types for security.
    }

    Ok(extracted)
}
