// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Plugin registry client and manifest types for the Orbflow marketplace.

pub mod client;
pub mod index;
pub mod install;
pub mod manifest;
pub mod merged;

/// Re-export reqwest::Client so consumers can construct a shared HTTP client
/// without adding reqwest as a direct dependency.
pub use reqwest::Client as HttpClient;
