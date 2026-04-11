// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! EventStore implementation for PostgreSQL.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Acquire, FromRow};

use orbflow_core::audit;
use orbflow_core::error::OrbflowError;
use orbflow_core::event::DomainEvent;
use orbflow_core::execution::{Instance, InstanceId};
use orbflow_core::ports::EventStore;

use crate::store::PgStore;

/// Internal row representation for the `events` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct EventRow {
    id: i64,
    instance_id: String,
    event_type: String,
    data: serde_json::Value,
    created_at: DateTime<Utc>,
}

/// Internal row representation for the `snapshots` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct SnapshotRow {
    instance_id: String,
    data: serde_json::Value,
    version: i64,
    created_at: DateTime<Utc>,
}

/// Row used when loading audit records with hash columns.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct AuditEventRow {
    id: i64,
    data: serde_json::Value,
    event_hash: Option<String>,
    prev_hash: Option<String>,
}

#[async_trait]
impl EventStore for PgStore {
    async fn append_event(&self, event: DomainEvent) -> Result<(), OrbflowError> {
        let data = serde_json::to_value(&event)
            .map_err(|e| OrbflowError::Database(format!("postgres: serialize event: {e}")))?;

        let event_type_str = serde_json::to_value(event.event_type())
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        let instance_id_str = event.instance_id().0.clone();
        let timestamp = event.timestamp();

        // Serialize the event data for hashing (canonical JSON bytes).
        let event_json = serde_json::to_vec(&data).map_err(|e| {
            OrbflowError::Database(format!("postgres: serialize event for hash: {e}"))
        })?;

        // Use a transaction so the prev_hash lookup and INSERT are atomic,
        // preventing gaps in the hash chain under concurrent appends.
        let mut conn =
            self.pool.acquire().await.map_err(|e| {
                OrbflowError::Database(format!("postgres: acquire connection: {e}"))
            })?;
        let mut txn = conn
            .begin()
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: begin transaction: {e}")))?;

        // Fetch the most recent event_hash for this instance (or use genesis).
        let prev_hash: String = sqlx::query_scalar::<_, Option<String>>(
            "SELECT event_hash FROM events WHERE instance_id = $1 ORDER BY id DESC LIMIT 1",
        )
        .bind(&instance_id_str)
        .fetch_optional(&mut *txn)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: fetch prev hash for instance {instance_id_str}: {e}"
            ))
        })?
        .flatten()
        .unwrap_or_else(|| audit::GENESIS_HASH.to_string());

        // Compute the chained hash.
        let event_hash = audit::compute_event_hash(&event_json, &prev_hash);

        // Optionally sign the event hash if an AuditSigner is configured.
        let signature: Option<String> = self
            .opts
            .audit_signer
            .as_ref()
            .map(|signer| signer.sign(event_hash.as_bytes()));

        sqlx::query(
            r#"INSERT INTO events (instance_id, event_type, data, created_at, event_hash, prev_hash, signature)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(&instance_id_str)
        .bind(&event_type_str)
        .bind(&data)
        .bind(timestamp)
        .bind(&event_hash)
        .bind(&prev_hash)
        .bind(&signature)
        .execute(&mut *txn)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: append event {} for instance {instance_id_str}: {e}",
                event_type_str,
            ))
        })?;

        txn.commit()
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: commit event append: {e}")))?;

        Ok(())
    }

    async fn load_events(
        &self,
        instance_id: &InstanceId,
        after_version: i64,
    ) -> Result<Vec<DomainEvent>, OrbflowError> {
        let rows: Vec<EventRow> = sqlx::query_as(
            r#"SELECT id, instance_id, event_type, data, created_at
               FROM events WHERE instance_id = $1 AND id > $2 ORDER BY id ASC"#,
        )
        .bind(instance_id.0.as_str())
        .bind(after_version)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: load events for instance {instance_id}: {e}"
            ))
        })?;

        let mut events = Vec::with_capacity(rows.len());
        for row in &rows {
            let event: DomainEvent = serde_json::from_value(row.data.clone()).map_err(|e| {
                OrbflowError::Database(format!(
                    "postgres: deserialize event type {} for instance {}: {e}",
                    row.event_type, row.instance_id
                ))
            })?;
            events.push(event);
        }

        Ok(events)
    }

    async fn save_snapshot(&self, inst: &Instance) -> Result<(), OrbflowError> {
        let data = serde_json::to_value(inst)
            .map_err(|e| OrbflowError::Database(format!("postgres: serialize snapshot: {e}")))?;

        sqlx::query(
            r#"INSERT INTO snapshots (instance_id, data, version, created_at)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (instance_id) DO UPDATE SET data=$2, version=$3, created_at=$4
               WHERE snapshots.version < $3"#,
        )
        .bind(inst.id.0.as_str())
        .bind(&data)
        .bind(inst.version)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: save snapshot for instance {}: {e}",
                inst.id
            ))
        })?;

        Ok(())
    }

    async fn load_snapshot(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<Instance>, OrbflowError> {
        let row: Option<SnapshotRow> = sqlx::query_as(
            r#"SELECT instance_id, data, version, created_at
               FROM snapshots WHERE instance_id = $1"#,
        )
        .bind(instance_id.0.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: load snapshot for instance {instance_id}: {e}"
            ))
        })?;

        match row {
            Some(r) => {
                let inst: Instance = serde_json::from_value(r.data).map_err(|e| {
                    OrbflowError::Database(format!(
                        "postgres: deserialize snapshot for instance {instance_id}: {e}"
                    ))
                })?;
                Ok(Some(inst))
            }
            None => Ok(None),
        }
    }

    async fn load_audit_records(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<audit::AuditRecord>, OrbflowError> {
        let rows: Vec<AuditEventRow> = sqlx::query_as(
            r#"SELECT id, data, event_hash, prev_hash
               FROM events WHERE instance_id = $1 ORDER BY id ASC"#,
        )
        .bind(instance_id.0.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: load audit records for instance {instance_id}: {e}"
            ))
        })?;

        let mut records = Vec::with_capacity(rows.len());
        for (seq, row) in rows.into_iter().enumerate() {
            let event_data = serde_json::to_vec(&row.data).map_err(|e| {
                OrbflowError::Database(format!("postgres: serialize audit event data: {e}"))
            })?;
            records.push(audit::AuditRecord {
                prev_hash: row
                    .prev_hash
                    .unwrap_or_else(|| audit::GENESIS_HASH.to_string()),
                event_hash: row.event_hash.unwrap_or_default(),
                event_data,
                seq: seq as u64,
            });
        }

        Ok(records)
    }
}
