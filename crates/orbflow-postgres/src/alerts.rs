// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alert rule persistence for PostgreSQL.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

use async_trait::async_trait;

use crate::store::is_unique_violation;

use orbflow_core::alerts::{AlertChannel, AlertMetric, AlertOperator, AlertRule};
use orbflow_core::error::OrbflowError;
use orbflow_core::ports::AlertStore;

use crate::store::PgStore;

/// Internal row representation for the `alert_rules` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct AlertRow {
    id: String,
    workflow_id: Option<String>,
    metric: String,
    operator: String,
    threshold: f64,
    channel: serde_json::Value,
    enabled: bool,
    created_at: DateTime<Utc>,
}

fn metric_to_str(m: AlertMetric) -> &'static str {
    match m {
        AlertMetric::FailureRate => "failure_rate",
        AlertMetric::P95Duration => "p95_duration",
        AlertMetric::ExecutionCount => "execution_count",
    }
}

fn parse_metric(s: &str) -> Result<AlertMetric, OrbflowError> {
    match s {
        "failure_rate" => Ok(AlertMetric::FailureRate),
        "p95_duration" => Ok(AlertMetric::P95Duration),
        "execution_count" => Ok(AlertMetric::ExecutionCount),
        other => Err(OrbflowError::InvalidNodeConfig(format!(
            "unknown alert metric: {other}"
        ))),
    }
}

fn operator_to_str(op: AlertOperator) -> &'static str {
    match op {
        AlertOperator::GreaterThan => "greater_than",
        AlertOperator::LessThan => "less_than",
        AlertOperator::Equals => "equals",
    }
}

fn parse_operator(s: &str) -> Result<AlertOperator, OrbflowError> {
    match s {
        "greater_than" => Ok(AlertOperator::GreaterThan),
        "less_than" => Ok(AlertOperator::LessThan),
        "equals" => Ok(AlertOperator::Equals),
        other => Err(OrbflowError::InvalidNodeConfig(format!(
            "unknown alert operator: {other}"
        ))),
    }
}

fn row_to_alert(row: &AlertRow) -> Result<AlertRule, OrbflowError> {
    let metric = parse_metric(&row.metric)?;
    let operator = parse_operator(&row.operator)?;
    let channel: AlertChannel = serde_json::from_value(row.channel.clone())
        .map_err(|e| OrbflowError::Database(format!("postgres: deserialize alert channel: {e}")))?;

    Ok(AlertRule {
        id: row.id.clone(),
        workflow_id: row.workflow_id.clone(),
        metric,
        operator,
        threshold: row.threshold,
        channel,
        enabled: row.enabled,
        created_at: row.created_at,
    })
}

#[async_trait]
impl AlertStore for PgStore {
    async fn create_alert(&self, rule: &AlertRule) -> Result<(), OrbflowError> {
        let channel_json = serde_json::to_value(&rule.channel).map_err(|e| {
            OrbflowError::Database(format!("postgres: serialize alert channel: {e}"))
        })?;

        sqlx::query(
            r#"INSERT INTO alert_rules (id, workflow_id, metric, operator, threshold, channel, enabled, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(&rule.id)
        .bind(&rule.workflow_id)
        .bind(metric_to_str(rule.metric))
        .bind(operator_to_str(rule.operator))
        .bind(rule.threshold)
        .bind(&channel_json)
        .bind(rule.enabled)
        .bind(rule.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                OrbflowError::AlreadyExists
            } else {
                OrbflowError::Database(format!("postgres: create alert '{}': {e}", rule.id))
            }
        })?;

        Ok(())
    }

    async fn get_alert(&self, id: &str) -> Result<AlertRule, OrbflowError> {
        let row: AlertRow = sqlx::query_as(
            r#"SELECT id, workflow_id, metric, operator, threshold, channel, enabled, created_at
               FROM alert_rules WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: get alert '{id}': {e}")))?
        .ok_or(OrbflowError::NotFound)?;

        row_to_alert(&row)
    }

    async fn list_alerts(&self) -> Result<Vec<AlertRule>, OrbflowError> {
        let rows: Vec<AlertRow> = sqlx::query_as(
            r#"SELECT id, workflow_id, metric, operator, threshold, channel, enabled, created_at
               FROM alert_rules ORDER BY created_at ASC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: list alerts: {e}")))?;

        rows.iter().map(row_to_alert).collect()
    }

    async fn update_alert(&self, rule: &AlertRule) -> Result<(), OrbflowError> {
        let channel_json = serde_json::to_value(&rule.channel).map_err(|e| {
            OrbflowError::Database(format!("postgres: serialize alert channel: {e}"))
        })?;

        let result = sqlx::query(
            r#"UPDATE alert_rules
               SET workflow_id = $2, metric = $3, operator = $4, threshold = $5,
                   channel = $6, enabled = $7
               WHERE id = $1"#,
        )
        .bind(&rule.id)
        .bind(&rule.workflow_id)
        .bind(metric_to_str(rule.metric))
        .bind(operator_to_str(rule.operator))
        .bind(rule.threshold)
        .bind(&channel_json)
        .bind(rule.enabled)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: update alert '{}': {e}", rule.id))
        })?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    async fn delete_alert(&self, id: &str) -> Result<(), OrbflowError> {
        let result = sqlx::query("DELETE FROM alert_rules WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: delete alert '{id}': {e}")))?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }
}
