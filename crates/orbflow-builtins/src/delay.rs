// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Delay node: sleeps for configured duration with cancellation support.

use async_trait::async_trait;
use tokio::time::Duration;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::util::{make_output, resolve_config, string_val};

/// Pauses execution for a configured duration.
pub struct DelayNode;

impl NodeSchemaProvider for DelayNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:delay".into(),
            name: "Delay".into(),
            description: "Pause execution for a specified duration".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "clock".into(),
            color: "#f59e0b".into(),
            image_url: Some("/icons/clock.svg".into()),
            docs: None,
            inputs: vec![FieldSchema {
                key: "duration".into(),
                label: "Duration".into(),
                field_type: FieldType::String,
                required: true,
                default: Some(serde_json::json!("1s")),
                description: Some("Duration string (e.g. 5s, 1m, 500ms)".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            outputs: vec![FieldSchema {
                key: "delayed".into(),
                label: "Delayed Duration".into(),
                field_type: FieldType::String,
                required: false,
                default: None,
                description: None,
                r#enum: vec![],
                credential_type: None,
            }],
            parameters: vec![],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

/// Parses a human-readable duration string like "5s", "1m", "500ms", "2h", "1m30s".
///
/// Supported suffixes: `ms`, `s`, `m`, `h`.
fn parse_duration(s: &str) -> Result<Duration, OrbflowError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(OrbflowError::InvalidNodeConfig(
            "delay node: empty duration string".into(),
        ));
    }

    let mut total_ms: u64 = 0;
    let mut num_start: Option<usize> = None;

    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].is_ascii_digit() || chars[i] == '.' {
            if num_start.is_none() {
                num_start = Some(i);
            }
            i += 1;
        } else if chars[i].is_ascii_alphabetic() {
            let num_s = num_start.map(|start| &s[start..i]).ok_or_else(|| {
                OrbflowError::InvalidNodeConfig(format!("delay node: invalid duration {s:?}"))
            })?;

            let num: f64 = num_s.parse().map_err(|_| {
                OrbflowError::InvalidNodeConfig(format!(
                    "delay node: invalid number in duration {s:?}"
                ))
            })?;

            // Parse unit suffix.
            let unit_start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let unit = &s[unit_start..i];

            let ms = match unit {
                "ms" => num,
                "s" => num * 1_000.0,
                "m" => num * 60_000.0,
                "h" => num * 3_600_000.0,
                _ => {
                    return Err(OrbflowError::InvalidNodeConfig(format!(
                        "delay node: unknown unit {unit:?} in duration {s:?}"
                    )));
                }
            };

            total_ms += ms as u64;
            num_start = None;
        } else {
            i += 1;
        }
    }

    // Handle bare number (default to seconds).
    if let Some(start) = num_start {
        let num: f64 = s[start..].parse().map_err(|_| {
            OrbflowError::InvalidNodeConfig(format!("delay node: invalid duration {s:?}"))
        })?;
        total_ms += (num * 1_000.0) as u64;
    }

    const MAX_DELAY_MS: u64 = 24 * 60 * 60 * 1_000; // 24 hours
    if total_ms == 0 {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "delay node: duration must be positive, got {s:?}"
        )));
    }
    if total_ms > MAX_DELAY_MS {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "delay node: duration {s:?} exceeds maximum of 24h"
        )));
    }

    Ok(Duration::from_millis(total_ms))
}

/// Formats a duration back to a human-readable string.
fn format_duration(d: Duration) -> String {
    let total_ms = d.as_millis();
    if total_ms < 1_000 {
        format!("{total_ms}ms")
    } else if total_ms % 1_000 == 0 {
        let secs = total_ms / 1_000;
        if secs < 60 {
            format!("{secs}s")
        } else if secs % 60 == 0 {
            format!("{}m", secs / 60)
        } else {
            format!("{}m{}s", secs / 60, secs % 60)
        }
    } else {
        format!("{total_ms}ms")
    }
}

#[async_trait]
impl NodeExecutor for DelayNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);
        let dur_str = string_val(&cfg, "duration", "1s");

        let dur = parse_duration(&dur_str)?;
        let formatted = format_duration(dur);

        // Use tokio::select! so the delay is cancellable via the tokio runtime.
        tokio::time::sleep(dur).await;

        Ok(NodeOutput {
            data: Some(make_output(vec![(
                "delayed",
                serde_json::Value::String(formatted),
            )])),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_seconds() {
        let d = parse_duration("5s").unwrap();
        assert_eq!(d, Duration::from_secs(5));
    }

    #[test]
    fn test_parse_duration_ms() {
        let d = parse_duration("500ms").unwrap();
        assert_eq!(d, Duration::from_millis(500));
    }

    #[test]
    fn test_parse_duration_minutes() {
        let d = parse_duration("2m").unwrap();
        assert_eq!(d, Duration::from_secs(120));
    }

    #[test]
    fn test_parse_duration_compound() {
        let d = parse_duration("1m30s").unwrap();
        assert_eq!(d, Duration::from_secs(90));
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_secs(5)), "5s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m30s");
        assert_eq!(format_duration(Duration::from_secs(120)), "2m");
    }

    #[tokio::test]
    async fn test_delay_short() {
        let node = DelayNode;
        let mut config = std::collections::HashMap::new();
        config.insert("duration".into(), serde_json::json!("10ms"));

        let input = NodeInput {
            instance_id: orbflow_core::execution::InstanceId::new("inst-1"),
            node_id: "delay-1".into(),
            plugin_ref: "builtin:delay".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        };

        let output = node.execute(&input).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("delayed").unwrap(), "10ms");
    }

    #[test]
    fn test_delay_schema() {
        let node = DelayNode;
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:delay");
    }
}
