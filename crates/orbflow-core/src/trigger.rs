// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Trigger types — how workflows get started.

use std::fmt;

use serde::{Deserialize, Serialize};

/// How a workflow can be triggered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Manual,
    Event,
    Schedule,
    Webhook,
}

impl fmt::Display for TriggerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manual => f.write_str("manual"),
            Self::Event => f.write_str("event"),
            Self::Schedule => f.write_str("schedule"),
            Self::Webhook => f.write_str("webhook"),
        }
    }
}

/// A trigger definition (deprecated — use trigger-kind Nodes instead).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    #[serde(rename = "type")]
    pub trigger_type: TriggerType,
    #[serde(default)]
    pub config: TriggerConfig,
}

/// Configuration for a trigger.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}
