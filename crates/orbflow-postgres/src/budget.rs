// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Budget persistence for PostgreSQL.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

use async_trait::async_trait;

use crate::store::is_unique_violation;

use orbflow_core::error::OrbflowError;
use orbflow_core::metering::{AccountBudget, BudgetPeriod};
use orbflow_core::ports::BudgetStore;

use crate::store::PgStore;

/// Internal row representation for the `budgets` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct BudgetRow {
    id: String,
    workflow_id: Option<String>,
    team: Option<String>,
    period: String,
    limit_usd: f64,
    current_usd: f64,
    reset_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

fn parse_period(s: &str) -> BudgetPeriod {
    match s {
        "daily" => BudgetPeriod::Daily,
        "weekly" => BudgetPeriod::Weekly,
        _ => BudgetPeriod::Monthly,
    }
}

fn period_to_str(p: BudgetPeriod) -> &'static str {
    match p {
        BudgetPeriod::Daily => "daily",
        BudgetPeriod::Weekly => "weekly",
        BudgetPeriod::Monthly => "monthly",
    }
}

fn row_to_budget(row: &BudgetRow) -> AccountBudget {
    AccountBudget {
        id: row.id.clone(),
        workflow_id: row.workflow_id.clone(),
        team: row.team.clone(),
        period: parse_period(&row.period),
        limit_usd: row.limit_usd,
        current_usd: row.current_usd,
        reset_at: row.reset_at,
        created_at: row.created_at,
    }
}

#[async_trait]
impl BudgetStore for PgStore {
    async fn create_budget(&self, budget: &AccountBudget) -> Result<(), OrbflowError> {
        sqlx::query(
            r#"INSERT INTO budgets (id, workflow_id, team, period, limit_usd, current_usd, reset_at, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(&budget.id)
        .bind(&budget.workflow_id)
        .bind(&budget.team)
        .bind(period_to_str(budget.period))
        .bind(budget.limit_usd)
        .bind(budget.current_usd)
        .bind(budget.reset_at)
        .bind(budget.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                OrbflowError::AlreadyExists
            } else {
                OrbflowError::Database(format!("postgres: create budget '{}': {e}", budget.id))
            }
        })?;

        Ok(())
    }

    async fn get_budget(&self, id: &str) -> Result<AccountBudget, OrbflowError> {
        let row: BudgetRow = sqlx::query_as(
            r#"SELECT id, workflow_id, team, period, limit_usd, current_usd, reset_at, created_at
               FROM budgets WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: get budget '{id}': {e}")))?
        .ok_or(OrbflowError::NotFound)?;

        Ok(row_to_budget(&row))
    }

    async fn list_budgets(&self) -> Result<Vec<AccountBudget>, OrbflowError> {
        let rows: Vec<BudgetRow> = sqlx::query_as(
            r#"SELECT id, workflow_id, team, period, limit_usd, current_usd, reset_at, created_at
               FROM budgets ORDER BY created_at ASC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: list budgets: {e}")))?;

        Ok(rows.iter().map(row_to_budget).collect())
    }

    async fn update_budget(&self, budget: &AccountBudget) -> Result<(), OrbflowError> {
        let result = sqlx::query(
            r#"UPDATE budgets
               SET workflow_id = $2, team = $3, period = $4, limit_usd = $5,
                   current_usd = $6, reset_at = $7
               WHERE id = $1"#,
        )
        .bind(&budget.id)
        .bind(&budget.workflow_id)
        .bind(&budget.team)
        .bind(period_to_str(budget.period))
        .bind(budget.limit_usd)
        .bind(budget.current_usd)
        .bind(budget.reset_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: update budget '{}': {e}", budget.id))
        })?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    async fn delete_budget(&self, id: &str) -> Result<(), OrbflowError> {
        let result = sqlx::query("DELETE FROM budgets WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: delete budget '{id}': {e}")))?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    async fn check_budget(&self, workflow_id: &str) -> Result<Option<AccountBudget>, OrbflowError> {
        let row: Option<BudgetRow> = sqlx::query_as(
            r#"SELECT id, workflow_id, team, period, limit_usd, current_usd, reset_at, created_at
               FROM budgets
               WHERE workflow_id = $1
               ORDER BY created_at ASC
               LIMIT 1"#,
        )
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: check budget for workflow '{workflow_id}': {e}"
            ))
        })?;

        Ok(row.as_ref().map(row_to_budget))
    }

    async fn increment_cost(&self, workflow_id: &str, cost_usd: f64) -> Result<(), OrbflowError> {
        let result = sqlx::query(
            r#"UPDATE budgets
               SET current_usd = current_usd + $2
               WHERE workflow_id = $1
                 AND current_usd + $2 <= limit_usd
               RETURNING id"#,
        )
        .bind(workflow_id)
        .bind(cost_usd)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: increment cost for workflow '{workflow_id}': {e}"
            ))
        })?;

        if result.is_none() {
            return Err(OrbflowError::BudgetExceeded(format!(
                "Budget exceeded for workflow {workflow_id} (attempted to add ${cost_usd:.2})"
            )));
        }

        Ok(())
    }
}
