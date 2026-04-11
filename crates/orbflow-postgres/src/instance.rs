// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! InstanceStore implementation for PostgreSQL.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

use orbflow_core::error::OrbflowError;
use orbflow_core::event::DomainEvent;
use orbflow_core::execution::{Instance, InstanceId};
use orbflow_core::ports::{AtomicInstanceCreator, DEFAULT_PAGE_SIZE, InstanceStore, ListOptions};

use crate::store::PgStore;

/// Internal row representation for the `workflow_instances` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct InstanceRow {
    id: String,
    workflow_id: String,
    status: String,
    data: serde_json::Value,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Row with a window-function total for paginated queries.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct InstanceRowWithTotal {
    id: String,
    workflow_id: String,
    status: String,
    data: serde_json::Value,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    total: i64,
}

fn row_to_instance(row: &InstanceRow) -> Result<Instance, OrbflowError> {
    serde_json::from_value(row.data.clone()).map_err(|e| {
        OrbflowError::Database(format!("postgres: deserialize instance {}: {e}", row.id))
    })
}

fn row_with_total_to_instance(row: &InstanceRowWithTotal) -> Result<Instance, OrbflowError> {
    serde_json::from_value(row.data.clone()).map_err(|e| {
        OrbflowError::Database(format!("postgres: deserialize instance {}: {e}", row.id))
    })
}

/// Serializes an instance into a JSONB-ready value.
fn instance_to_json(inst: &Instance) -> Result<serde_json::Value, OrbflowError> {
    serde_json::to_value(inst)
        .map_err(|e| OrbflowError::Database(format!("postgres: serialize instance: {e}")))
}

/// Returns the status as a string for the status column.
fn status_str(inst: &Instance) -> Result<String, OrbflowError> {
    serde_json::to_value(inst.status)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .ok_or_else(|| {
            OrbflowError::Internal(format!(
                "cannot serialize instance status: {:?}",
                inst.status
            ))
        })
}

#[async_trait]
impl InstanceStore for PgStore {
    async fn create_instance(&self, inst: &Instance) -> Result<(), OrbflowError> {
        let data = instance_to_json(inst)?;
        let status = status_str(inst)?;

        sqlx::query(
            r#"INSERT INTO workflow_instances (id, workflow_id, status, data, version, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(inst.id.0.as_str())
        .bind(inst.workflow_id.0.as_str())
        .bind(&status)
        .bind(&data)
        .bind(inst.version)
        .bind(inst.created_at)
        .bind(inst.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: create instance {}: {e}", inst.id)))?;

        Ok(())
    }

    async fn get_instance(&self, id: &InstanceId) -> Result<Instance, OrbflowError> {
        let row: InstanceRow = sqlx::query_as(
            r#"SELECT id, workflow_id, status, data, version, created_at, updated_at
               FROM workflow_instances WHERE id = $1"#,
        )
        .bind(id.0.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: get instance {id}: {e}")))?
        .ok_or(OrbflowError::NotFound)?;

        row_to_instance(&row)
    }

    async fn update_instance(&self, inst: &Instance) -> Result<(), OrbflowError> {
        let data = instance_to_json(inst)?;
        let status = status_str(inst)?;

        // Optimistic locking: only update if version matches (current version - 1,
        // since the caller increments version before calling).
        let prev_version = inst.version - 1;

        let result: Option<(String,)> = sqlx::query_as(
            r#"UPDATE workflow_instances SET status=$1, data=$2, version=$3, updated_at=$4
               WHERE id=$5 AND version=$6
               RETURNING id"#,
        )
        .bind(&status)
        .bind(&data)
        .bind(inst.version)
        .bind(inst.updated_at)
        .bind(inst.id.0.as_str())
        .bind(prev_version)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: update instance {}: {e}", inst.id))
        })?;

        if result.is_none() {
            // Zero rows updated: either the instance doesn't exist or the
            // version didn't match. The engine's retry loop (max 3 attempts)
            // handles both cases identically — return Conflict to trigger a
            // retry with a fresh read. This avoids an extra SELECT round-trip
            // under high concurrency when conflicts are most frequent.
            return Err(OrbflowError::Conflict);
        }

        Ok(())
    }

    async fn list_instances(
        &self,
        opts: ListOptions,
    ) -> Result<(Vec<Instance>, i64), OrbflowError> {
        let limit = if opts.limit > 0 {
            opts.limit
        } else {
            DEFAULT_PAGE_SIZE
        };
        let offset = opts.offset.max(0);

        let rows: Vec<InstanceRowWithTotal> = sqlx::query_as(
            r#"SELECT id, workflow_id, status, data, version, created_at, updated_at,
                      COUNT(*) OVER() AS total
               FROM workflow_instances ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: list instances: {e}")))?;

        let total = rows.first().map(|r| r.total).unwrap_or(0);
        let instances = rows
            .iter()
            .map(row_with_total_to_instance)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((instances, total))
    }

    async fn list_running_instances(&self) -> Result<Vec<Instance>, OrbflowError> {
        let rows: Vec<InstanceRow> = sqlx::query_as(
            r#"SELECT id, workflow_id, status, data, version, created_at, updated_at
               FROM workflow_instances WHERE status = 'running'"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: list running instances: {e}")))?;

        rows.iter().map(row_to_instance).collect()
    }
}

#[async_trait]
impl AtomicInstanceCreator for PgStore {
    async fn create_instance_tx(
        &self,
        inst: &Instance,
        event: DomainEvent,
    ) -> Result<(), OrbflowError> {
        let data = instance_to_json(inst)?;
        let status = status_str(inst)?;

        let event_data = serde_json::to_value(&event)
            .map_err(|e| OrbflowError::Database(format!("postgres: serialize event: {e}")))?;

        let event_type_str = serde_json::to_value(event.event_type())
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: begin tx: {e}")))?;

        sqlx::query(
            r#"INSERT INTO workflow_instances (id, workflow_id, status, data, version, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(inst.id.0.as_str())
        .bind(inst.workflow_id.0.as_str())
        .bind(&status)
        .bind(&data)
        .bind(inst.version)
        .bind(inst.created_at)
        .bind(inst.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: create instance tx {}: {e}", inst.id))
        })?;

        // Compute audit hash chain for the initial event (genesis).
        let event_bytes = serde_json::to_vec(&event_data).map_err(|e| {
            OrbflowError::Database(format!("postgres: serialize event for audit hash: {e}"))
        })?;
        let prev_hash = orbflow_core::audit::GENESIS_HASH.to_string();
        let event_hash = orbflow_core::audit::compute_event_hash(&event_bytes, &prev_hash);

        sqlx::query(
            r#"INSERT INTO events (instance_id, event_type, data, created_at, event_hash, prev_hash)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(inst.id.0.as_str())
        .bind(&event_type_str)
        .bind(&event_data)
        .bind(event.timestamp())
        .bind(&event_hash)
        .bind(&prev_hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: insert event tx {}: {e}", inst.id))
        })?;

        tx.commit()
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: commit tx: {e}")))?;

        Ok(())
    }
}
