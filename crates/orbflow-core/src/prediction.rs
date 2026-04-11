// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Predictive failure detection using statistical anomaly detection.
//!
//! Tracks rolling statistics for workflow and node execution patterns, then
//! flags anomalies when metrics deviate significantly from historical norms.
//!
//! # Algorithm
//!
//! Uses a Z-score approach: when a metric exceeds `mean ± (threshold × stddev)`,
//! it is flagged as an anomaly. This is simple, interpretable, and requires no
//! ML model training.

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Rolling statistics over a sliding window of observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingStats {
    /// Maximum number of observations to keep.
    window_size: usize,
    /// Stored observations (most recent at back).
    values: VecDeque<f64>,
}

impl RollingStats {
    /// Creates a new rolling stats tracker with the given window size.
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            values: VecDeque::with_capacity(window_size),
        }
    }

    /// Records a new observation.
    pub fn push(&mut self, value: f64) {
        if self.values.len() >= self.window_size {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    /// Returns the number of observations currently stored.
    pub fn count(&self) -> usize {
        self.values.len()
    }

    /// Computes the mean of stored observations.
    pub fn mean(&self) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let sum: f64 = self.values.iter().sum();
        Some(sum / self.values.len() as f64)
    }

    /// Computes the standard deviation of stored observations.
    pub fn stddev(&self) -> Option<f64> {
        let mean = self.mean()?;
        if self.values.len() < 2 {
            return None;
        }
        let variance: f64 = self.values.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
            / (self.values.len() - 1) as f64;
        Some(variance.sqrt())
    }

    /// Computes the Z-score for a given value relative to the rolling stats.
    ///
    /// Returns `None` if there's insufficient data (< 2 observations).
    pub fn z_score(&self, value: f64) -> Option<f64> {
        let mean = self.mean()?;
        let stddev = self.stddev()?;
        if stddev < f64::EPSILON {
            return None; // All values are identical — can't compute z-score.
        }
        Some((value - mean) / stddev)
    }

    /// Checks if a value is anomalous (|z-score| > threshold).
    ///
    /// Default threshold is 3.0 (3 standard deviations).
    pub fn is_anomalous(&self, value: f64, threshold: f64) -> bool {
        self.z_score(value).is_some_and(|z| z.abs() > threshold)
    }
}

/// An anomaly detected in workflow/node execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// What was measured (e.g., "node.duration_ms", "workflow.failure_rate").
    pub metric: String,
    /// The observed value that triggered the anomaly.
    pub observed: f64,
    /// The expected mean value.
    pub expected_mean: f64,
    /// The standard deviation.
    pub stddev: f64,
    /// The computed Z-score.
    pub z_score: f64,
    /// Severity based on Z-score magnitude.
    pub severity: AnomalySeverity,
    /// When the anomaly was detected.
    pub detected_at: DateTime<Utc>,
    /// Context: which workflow/node/instance triggered the anomaly.
    pub context: AnomalyContext,
}

/// Severity level of an anomaly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    /// Z-score between 2 and 3 — possible anomaly.
    Warning,
    /// Z-score between 3 and 4 — likely anomaly.
    High,
    /// Z-score > 4 — definite anomaly.
    Critical,
}

impl AnomalySeverity {
    /// Determines severity from a Z-score.
    pub fn from_z_score(z: f64) -> Option<Self> {
        let abs_z = z.abs();
        if abs_z > 4.0 {
            Some(Self::Critical)
        } else if abs_z > 3.0 {
            Some(Self::High)
        } else if abs_z > 2.0 {
            Some(Self::Warning)
        } else {
            None
        }
    }
}

/// Context for where an anomaly was detected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

/// Checks a value against rolling stats and returns an anomaly if detected.
pub fn check_anomaly(
    stats: &RollingStats,
    value: f64,
    metric: &str,
    threshold: f64,
    context: AnomalyContext,
) -> Option<Anomaly> {
    let z = stats.z_score(value)?;
    let severity = AnomalySeverity::from_z_score(z)?;
    let mean = stats.mean()?;
    let stddev = stats.stddev()?;

    // Only flag if it exceeds the threshold.
    if z.abs() <= threshold {
        return None;
    }

    Some(Anomaly {
        metric: metric.to_string(),
        observed: value,
        expected_mean: mean,
        stddev,
        z_score: z,
        severity,
        detected_at: Utc::now(),
        context,
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_stats_mean() {
        let mut stats = RollingStats::new(5);
        stats.push(10.0);
        stats.push(20.0);
        stats.push(30.0);
        assert!((stats.mean().unwrap() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rolling_stats_window_eviction() {
        let mut stats = RollingStats::new(3);
        stats.push(1.0);
        stats.push(2.0);
        stats.push(3.0);
        stats.push(4.0); // evicts 1.0
        assert_eq!(stats.count(), 3);
        assert!((stats.mean().unwrap() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rolling_stats_stddev() {
        let mut stats = RollingStats::new(100);
        for v in [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
            stats.push(v);
        }
        let stddev = stats.stddev().unwrap();
        // Population stddev ≈ 2.0, sample stddev ≈ 2.138
        assert!((stddev - 2.138).abs() < 0.01);
    }

    #[test]
    fn test_z_score() {
        let mut stats = RollingStats::new(100);
        // Push 100 values with mean=100, stddev≈10
        for i in 0..100 {
            stats.push(90.0 + (i as f64 % 20.0));
        }
        let mean = stats.mean().unwrap();
        let stddev = stats.stddev().unwrap();

        // A value far from mean should have a high z-score.
        let z = stats.z_score(mean + 4.0 * stddev).unwrap();
        assert!(z > 3.5);
    }

    #[test]
    fn test_is_anomalous() {
        let mut stats = RollingStats::new(100);
        for i in 0..50 {
            stats.push(100.0 + (i as f64 % 5.0)); // tight cluster around 100-104
        }
        let mean = stats.mean().unwrap();
        let stddev = stats.stddev().unwrap();

        // Normal value — not anomalous.
        assert!(!stats.is_anomalous(mean, 3.0));

        // Extreme value — anomalous.
        assert!(stats.is_anomalous(mean + 5.0 * stddev, 3.0));
    }

    #[test]
    fn test_insufficient_data() {
        let mut stats = RollingStats::new(100);
        stats.push(100.0);
        // Need at least 2 values for stddev.
        assert!(stats.stddev().is_none());
        assert!(stats.z_score(100.0).is_none());
        assert!(!stats.is_anomalous(200.0, 3.0));
    }

    #[test]
    fn test_identical_values() {
        let mut stats = RollingStats::new(100);
        for _ in 0..10 {
            stats.push(42.0);
        }
        // Stddev is 0 — z-score is undefined.
        assert!(stats.z_score(42.0).is_none());
        assert!(!stats.is_anomalous(100.0, 3.0));
    }

    #[test]
    fn test_severity_from_z_score() {
        assert_eq!(AnomalySeverity::from_z_score(1.5), None);
        assert_eq!(
            AnomalySeverity::from_z_score(2.5),
            Some(AnomalySeverity::Warning)
        );
        assert_eq!(
            AnomalySeverity::from_z_score(3.5),
            Some(AnomalySeverity::High)
        );
        assert_eq!(
            AnomalySeverity::from_z_score(5.0),
            Some(AnomalySeverity::Critical)
        );
        // Negative z-scores should also work.
        assert_eq!(
            AnomalySeverity::from_z_score(-4.5),
            Some(AnomalySeverity::Critical)
        );
    }

    #[test]
    fn test_check_anomaly() {
        let mut stats = RollingStats::new(100);
        for i in 0..50 {
            stats.push(100.0 + (i as f64 % 3.0));
        }
        let mean = stats.mean().unwrap();
        let stddev = stats.stddev().unwrap();

        // Normal value — no anomaly.
        let ctx = AnomalyContext {
            workflow_id: Some("wf-1".into()),
            node_id: None,
            instance_id: None,
        };
        let result = check_anomaly(&stats, mean, "duration_ms", 3.0, ctx.clone());
        assert!(result.is_none());

        // Extreme value — anomaly detected.
        let extreme = mean + 5.0 * stddev;
        let anomaly = check_anomaly(&stats, extreme, "duration_ms", 3.0, ctx);
        assert!(anomaly.is_some());
        let a = anomaly.unwrap();
        assert_eq!(a.metric, "duration_ms");
        assert!(a.z_score > 3.0);
    }

    #[test]
    fn test_anomaly_serde() {
        let anomaly = Anomaly {
            metric: "node.duration_ms".into(),
            observed: 5000.0,
            expected_mean: 100.0,
            stddev: 20.0,
            z_score: 245.0,
            severity: AnomalySeverity::Critical,
            detected_at: Utc::now(),
            context: AnomalyContext {
                workflow_id: Some("wf-1".into()),
                node_id: Some("http-1".into()),
                instance_id: Some("inst-42".into()),
            },
        };
        let json = serde_json::to_string(&anomaly).unwrap();
        let a2: Anomaly = serde_json::from_str(&json).unwrap();
        assert_eq!(a2.severity, AnomalySeverity::Critical);
        assert_eq!(a2.metric, "node.duration_ms");
    }
}
