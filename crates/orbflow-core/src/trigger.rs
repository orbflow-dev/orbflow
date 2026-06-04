// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Trigger types — how workflows get started.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};

/// Canonical trigger type names accepted by the backend wire contract.
pub const CANONICAL_TRIGGER_TYPES: &[&str] = &["manual", "event", "schedule", "webhook"];

/// Legacy frontend/UI alias for [`TriggerType::Schedule`].
pub const LEGACY_CRON_TRIGGER_TYPE: &str = "cron";

const ACCEPTED_TRIGGER_TYPES: &[&str] = &["manual", "event", "schedule", "webhook", "cron"];

/// How a workflow can be triggered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Manual,
    Event,
    Schedule,
    Webhook,
}

impl TriggerType {
    /// Returns the canonical backend wire value for this trigger type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Event => "event",
            Self::Schedule => "schedule",
            Self::Webhook => "webhook",
        }
    }

    /// Parses a backend wire value, accepting the legacy UI alias `cron`.
    pub fn parse_wire(value: &str) -> Option<Self> {
        match value {
            "manual" => Some(Self::Manual),
            "event" => Some(Self::Event),
            "schedule" | LEGACY_CRON_TRIGGER_TYPE => Some(Self::Schedule),
            "webhook" => Some(Self::Webhook),
            _ => None,
        }
    }

    /// Returns true when `value` is an accepted alias but not canonical output.
    pub fn is_legacy_alias(value: &str) -> bool {
        value == LEGACY_CRON_TRIGGER_TYPE
    }
}

impl fmt::Display for TriggerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerTypeParseError {
    value: String,
}

impl fmt::Display for TriggerTypeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown trigger type '{}'; expected one of {}",
            self.value,
            ACCEPTED_TRIGGER_TYPES.join(", ")
        )
    }
}

impl std::error::Error for TriggerTypeParseError {}

impl FromStr for TriggerType {
    type Err = TriggerTypeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_wire(value).ok_or_else(|| TriggerTypeParseError {
            value: value.to_owned(),
        })
    }
}

impl<'de> Deserialize<'de> for TriggerType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value)
            .map_err(|_| serde::de::Error::unknown_variant(&value, ACCEPTED_TRIGGER_TYPES))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_type_serializes_canonical_schedule() {
        let json = serde_json::to_string(&TriggerType::Schedule).unwrap();
        assert_eq!(json, r#""schedule""#);
    }

    #[test]
    fn trigger_type_accepts_legacy_cron_alias() {
        let parsed: TriggerType = serde_json::from_str(r#""cron""#).unwrap();
        assert_eq!(parsed, TriggerType::Schedule);
        assert!(TriggerType::is_legacy_alias("cron"));
    }

    #[test]
    fn trigger_type_rejects_unknown_values() {
        let err = serde_json::from_str::<TriggerType>(r#""timer""#).unwrap_err();
        assert!(err.to_string().contains("timer"));
    }
}
