// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Budget persistence for PostgreSQL.

use chrono::{DateTime, Duration, Months, Utc};
use sqlx::{FromRow, Postgres, Transaction};

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

fn next_reset_after(
    mut reset_at: DateTime<Utc>,
    period: BudgetPeriod,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    while reset_at <= now {
        reset_at = match period {
            BudgetPeriod::Daily => reset_at + Duration::days(1),
            BudgetPeriod::Weekly => reset_at + Duration::weeks(1),
            BudgetPeriod::Monthly => reset_at
                .checked_add_months(Months::new(1))
                .unwrap_or_else(|| reset_at + Duration::days(31)),
        };
    }
    reset_at
}

async fn rollover_if_expired(
    tx: &mut Transaction<'_, Postgres>,
    row: &mut BudgetRow,
    now: DateTime<Utc>,
) -> Result<(), OrbflowError> {
    if row.reset_at > now {
        return Ok(());
    }

    let next_reset = next_reset_after(row.reset_at, parse_period(&row.period), now);
    sqlx::query("UPDATE budgets SET current_usd = 0, reset_at = $2 WHERE id = $1")
        .bind(&row.id)
        .bind(next_reset)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: roll over budget '{}': {e}", row.id))
        })?;

    row.current_usd = 0.0;
    row.reset_at = next_reset;
    Ok(())
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
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: begin budget tx: {e}")))?;

        let mut rows: Vec<BudgetRow> = sqlx::query_as(
            r#"SELECT id, workflow_id, team, period, limit_usd, current_usd, reset_at, created_at
               FROM budgets
               WHERE workflow_id = $1 OR workflow_id IS NULL
               ORDER BY CASE WHEN workflow_id = $1 THEN 0 ELSE 1 END, created_at ASC
               FOR UPDATE"#,
        )
        .bind(workflow_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: lock budgets for workflow '{workflow_id}': {e}"
            ))
        })?;

        let now = Utc::now();
        for row in &mut rows {
            rollover_if_expired(&mut tx, row, now).await?;
        }

        rows.sort_by(|a, b| {
            let a_exceeded = a.current_usd >= a.limit_usd;
            let b_exceeded = b.current_usd >= b.limit_usd;
            b_exceeded
                .cmp(&a_exceeded)
                .then_with(|| {
                    let a_specific = a.workflow_id.as_deref() == Some(workflow_id);
                    let b_specific = b.workflow_id.as_deref() == Some(workflow_id);
                    b_specific.cmp(&a_specific)
                })
                .then_with(|| a.created_at.cmp(&b.created_at))
        });
        let budget = rows.first().map(row_to_budget);

        tx.commit()
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: commit budget tx: {e}")))?;

        Ok(budget)
    }

    async fn increment_cost(&self, workflow_id: &str, cost_usd: f64) -> Result<(), OrbflowError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: begin budget tx: {e}")))?;

        let mut rows: Vec<BudgetRow> = sqlx::query_as(
            r#"SELECT id, workflow_id, team, period, limit_usd, current_usd, reset_at, created_at
               FROM budgets
               WHERE workflow_id = $1 OR workflow_id IS NULL
               ORDER BY CASE WHEN workflow_id = $1 THEN 0 ELSE 1 END, created_at ASC
               FOR UPDATE"#,
        )
        .bind(workflow_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: lock budgets for workflow '{workflow_id}': {e}"
            ))
        })?;

        if rows.is_empty() {
            tx.commit()
                .await
                .map_err(|e| OrbflowError::Database(format!("postgres: commit budget tx: {e}")))?;
            return Ok(());
        }

        let now = Utc::now();
        for row in &mut rows {
            rollover_if_expired(&mut tx, row, now).await?;
        }

        if let Some(row) = rows
            .iter()
            .find(|row| row.current_usd + cost_usd > row.limit_usd)
        {
            return Err(OrbflowError::BudgetExceeded(format!(
                "Budget exceeded for workflow {workflow_id} by budget {} (attempted to add ${cost_usd:.2}: ${:.2} / ${:.2})",
                row.id, row.current_usd, row.limit_usd
            )));
        }

        for row in &rows {
            sqlx::query("UPDATE budgets SET current_usd = $2, reset_at = $3 WHERE id = $1")
                .bind(&row.id)
                .bind(row.current_usd + cost_usd)
                .bind(row.reset_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    OrbflowError::Database(format!(
                        "postgres: increment cost for budget '{}' on workflow '{workflow_id}': {e}",
                        row.id
                    ))
                })?;
        }

        tx.commit()
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: commit budget tx: {e}")))?;

        Ok(())
    }
}
