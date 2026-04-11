// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! SLA/SLO monitoring: detect latency anomalies and threshold violations.

use std::collections::HashMap;
use std::sync::RwLock;

use orbflow_core::prediction::RollingStats;
use orbflow_core::workflow::WorkflowId;

/// Default Z-score threshold for anomaly detection (3 standard deviations).
const ANOMALY_THRESHOLD: f64 = 3.0;

/// Default rolling window size for per-workflow duration stats.
const ROLLING_WINDOW_SIZE: usize = 100;

/// SLA configuration for a workflow.
#[derive(Debug, Clone)]
pub struct SlaConfig {
    /// Maximum allowed execution duration in milliseconds.
    /// Violations are logged as warnings.
    pub max_duration_ms: Option<i64>,
    /// Maximum allowed failure rate (0.0 to 1.0).
    pub max_failure_rate: Option<f64>,
}

/// Result of an SLA check.
#[derive(Debug, Clone)]
pub enum SlaCheckResult {
    /// All SLA conditions met.
    Ok,
    /// Duration exceeded the configured maximum.
    DurationViolation { actual_ms: i64, max_ms: i64 },
    /// Anomalous latency detected by statistical analysis.
    LatencyAnomaly {
        actual_ms: i64,
        expected_mean_ms: f64,
        z_score: f64,
    },
    /// Anomalous failure rate detected by statistical analysis.
    FailureRateAnomaly {
        workflow_id: WorkflowId,
        failure_rate: f64,
        expected_rate: f64,
        z_score: f64,
    },
}

/// Tracks rolling statistics per workflow for anomaly detection.
pub struct SlaMonitor {
    /// Per-workflow rolling duration stats.
    stats: RwLock<HashMap<WorkflowId, RollingStats>>,
    /// Per-workflow rolling failure rate stats (1.0 = failure, 0.0 = success).
    failure_stats: RwLock<HashMap<WorkflowId, RollingStats>>,
    /// Per-workflow SLA configs (loaded from workflow metadata or config).
    configs: RwLock<HashMap<WorkflowId, SlaConfig>>,
}

impl SlaMonitor {
    /// Creates a new SLA monitor.
    pub fn new() -> Self {
        Self {
            stats: RwLock::new(HashMap::new()),
            failure_stats: RwLock::new(HashMap::new()),
            configs: RwLock::new(HashMap::new()),
        }
    }

    /// Registers an SLA configuration for a workflow.
    pub fn set_config(&self, workflow_id: WorkflowId, config: SlaConfig) {
        self.configs
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(workflow_id, config);
    }

    /// Records a workflow failure (pushes 1.0 to failure rate stats).
    pub fn record_failure(&self, workflow_id: &WorkflowId) {
        let mut stats = self
            .failure_stats
            .write()
            .unwrap_or_else(|p| p.into_inner());
        let rolling = stats
            .entry(workflow_id.clone())
            .or_insert_with(|| RollingStats::new(ROLLING_WINDOW_SIZE));
        rolling.push(1.0);
    }

    /// Records a workflow success (pushes 0.0 to failure rate stats).
    pub fn record_success(&self, workflow_id: &WorkflowId) {
        let mut stats = self
            .failure_stats
            .write()
            .unwrap_or_else(|p| p.into_inner());
        let rolling = stats
            .entry(workflow_id.clone())
            .or_insert_with(|| RollingStats::new(ROLLING_WINDOW_SIZE));
        rolling.push(0.0);
    }

    /// Checks for failure rate anomaly for a workflow.
    ///
    /// Returns `Some(SlaCheckResult::FailureRateAnomaly)` if the current
    /// failure rate is statistically anomalous.
    pub fn check_failure_rate(&self, workflow_id: &WorkflowId) -> Option<SlaCheckResult> {
        let stats = self.failure_stats.read().unwrap_or_else(|p| p.into_inner());
        let rolling = stats.get(workflow_id)?;
        let current_rate = rolling.mean()?;
        if rolling.is_anomalous(current_rate, ANOMALY_THRESHOLD) {
            let expected_rate = rolling.mean().unwrap_or(0.0);
            let z_score = rolling.z_score(current_rate).unwrap_or(0.0);
            Some(SlaCheckResult::FailureRateAnomaly {
                workflow_id: workflow_id.clone(),
                failure_rate: current_rate,
                expected_rate,
                z_score,
            })
        } else {
            None
        }
    }

    /// Records a completed workflow duration and checks for violations.
    ///
    /// Returns the SLA check result. The caller should log/alert on violations.
    pub fn check_and_record(&self, workflow_id: &WorkflowId, duration_ms: i64) -> SlaCheckResult {
        // Update rolling stats and extract anomaly values while holding the
        // write lock. Release it before acquiring any other locks to prevent
        // lock-order inversion deadlocks.
        let (is_anomalous, expected_mean_ms, z_score) = {
            let mut stats = self.stats.write().unwrap_or_else(|p| p.into_inner());
            let rolling = stats
                .entry(workflow_id.clone())
                .or_insert_with(|| RollingStats::new(ROLLING_WINDOW_SIZE));
            rolling.push(duration_ms as f64);
            let anom = rolling.is_anomalous(duration_ms as f64, ANOMALY_THRESHOLD);
            let mean = rolling.mean().unwrap_or(0.0);
            let z = rolling.z_score(duration_ms as f64).unwrap_or(0.0);
            (anom, mean, z)
        };
        // stats lock released here — safe to acquire configs and failure_stats.

        // Check explicit duration threshold.
        let configs = self.configs.read().unwrap_or_else(|p| p.into_inner());
        if let Some(cfg) = configs.get(workflow_id)
            && let Some(max_ms) = cfg.max_duration_ms
            && duration_ms > max_ms
        {
            return SlaCheckResult::DurationViolation {
                actual_ms: duration_ms,
                max_ms,
            };
        }
        drop(configs);

        // Check for statistical anomaly.
        if is_anomalous {
            return SlaCheckResult::LatencyAnomaly {
                actual_ms: duration_ms,
                expected_mean_ms,
                z_score,
            };
        }

        // Check for failure rate anomaly.
        if let Some(result) = self.check_failure_rate(workflow_id) {
            return result;
        }

        SlaCheckResult::Ok
    }
}

impl Default for SlaMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sla_ok_when_within_threshold() {
        let monitor = SlaMonitor::new();
        let wf_id = WorkflowId("wf-1".into());
        monitor.set_config(
            wf_id.clone(),
            SlaConfig {
                max_duration_ms: Some(5000),
                max_failure_rate: None,
            },
        );

        let result = monitor.check_and_record(&wf_id, 3000);
        assert!(matches!(result, SlaCheckResult::Ok));
    }

    #[test]
    fn test_sla_duration_violation() {
        let monitor = SlaMonitor::new();
        let wf_id = WorkflowId("wf-2".into());
        monitor.set_config(
            wf_id.clone(),
            SlaConfig {
                max_duration_ms: Some(1000),
                max_failure_rate: None,
            },
        );

        let result = monitor.check_and_record(&wf_id, 2000);
        assert!(matches!(
            result,
            SlaCheckResult::DurationViolation {
                actual_ms: 2000,
                max_ms: 1000
            }
        ));
    }

    #[test]
    fn test_sla_no_config_still_tracks_stats() {
        let monitor = SlaMonitor::new();
        let wf_id = WorkflowId("wf-3".into());

        // Without config, should track stats and return Ok (no anomaly with few samples).
        for _ in 0..10 {
            let result = monitor.check_and_record(&wf_id, 100);
            assert!(matches!(result, SlaCheckResult::Ok));
        }
    }

    #[test]
    fn test_sla_anomaly_detection() {
        let monitor = SlaMonitor::new();
        let wf_id = WorkflowId("wf-4".into());

        // Build up normal baseline (100ms durations).
        for _ in 0..50 {
            monitor.check_and_record(&wf_id, 100);
        }

        // Sudden spike should trigger anomaly.
        let result = monitor.check_and_record(&wf_id, 10000);
        assert!(matches!(result, SlaCheckResult::LatencyAnomaly { .. }));
    }

    #[test]
    fn test_sla_default_trait() {
        let monitor = SlaMonitor::default();
        let wf_id = WorkflowId("wf-default".into());
        let result = monitor.check_and_record(&wf_id, 500);
        assert!(matches!(result, SlaCheckResult::Ok));
    }

    #[test]
    fn test_sla_multiple_workflows_independent() {
        let monitor = SlaMonitor::new();
        let wf_a = WorkflowId("wf-a".into());
        let wf_b = WorkflowId("wf-b".into());

        monitor.set_config(
            wf_a.clone(),
            SlaConfig {
                max_duration_ms: Some(100),
                max_failure_rate: None,
            },
        );
        monitor.set_config(
            wf_b.clone(),
            SlaConfig {
                max_duration_ms: Some(5000),
                max_failure_rate: None,
            },
        );

        // wf_a violates, wf_b does not.
        let result_a = monitor.check_and_record(&wf_a, 200);
        let result_b = monitor.check_and_record(&wf_b, 200);

        assert!(matches!(result_a, SlaCheckResult::DurationViolation { .. }));
        assert!(matches!(result_b, SlaCheckResult::Ok));
    }
}
