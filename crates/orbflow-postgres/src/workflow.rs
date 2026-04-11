// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! WorkflowStore implementation for PostgreSQL.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

use orbflow_core::error::OrbflowError;

use crate::store::is_unique_violation;
use orbflow_core::ports::{DEFAULT_PAGE_SIZE, ListOptions, WorkflowStore};
use orbflow_core::versioning::WorkflowVersion;
use orbflow_core::workflow::{Workflow, WorkflowId};

use crate::store::PgStore;

/// Internal row representation for the `workflows` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct WorkflowRow {
    id: String,
    name: String,
    description: String,
    version: i32,
    status: String,
    definition: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Row with a window-function total for paginated queries.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct WorkflowRowWithTotal {
    id: String,
    name: String,
    description: String,
    version: i32,
    status: String,
    definition: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    total: i64,
}

/// Row metadata to merge into a workflow definition JSON.
struct RowMetadata<'a> {
    id: &'a str,
    name: &'a str,
    description: &'a str,
    version: i32,
    status: &'a str,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Merge row-level columns into the definition JSON before deserializing.
/// This ensures workflows are reconstructed correctly even when the definition
/// JSONB only contains the graph (nodes/edges) without metadata fields.
fn merge_row_into_definition(
    mut def: serde_json::Value,
    meta: &RowMetadata<'_>,
) -> serde_json::Value {
    if let Some(obj) = def.as_object_mut() {
        obj.entry("id")
            .or_insert_with(|| serde_json::json!(meta.id));
        obj.entry("name")
            .or_insert_with(|| serde_json::json!(meta.name));
        obj.entry("description")
            .or_insert_with(|| serde_json::json!(meta.description));
        obj.entry("version")
            .or_insert_with(|| serde_json::json!(meta.version));
        obj.entry("status")
            .or_insert_with(|| serde_json::json!(meta.status));
        obj.entry("created_at")
            .or_insert_with(|| serde_json::json!(meta.created_at));
        obj.entry("updated_at")
            .or_insert_with(|| serde_json::json!(meta.updated_at));
    }
    def
}

fn row_to_workflow(row: WorkflowRow) -> Result<Workflow, OrbflowError> {
    let def = merge_row_into_definition(
        row.definition,
        &RowMetadata {
            id: &row.id,
            name: &row.name,
            description: &row.description,
            version: row.version,
            status: &row.status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
    );
    serde_json::from_value(def).map_err(|e| {
        OrbflowError::Database(format!("postgres: deserialize workflow {}: {e}", row.id))
    })
}

fn row_with_total_to_workflow(row: &WorkflowRowWithTotal) -> Result<Workflow, OrbflowError> {
    let def = merge_row_into_definition(
        row.definition.clone(),
        &RowMetadata {
            id: &row.id,
            name: &row.name,
            description: &row.description,
            version: row.version,
            status: &row.status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
    );
    serde_json::from_value(def).map_err(|e| {
        OrbflowError::Database(format!("postgres: deserialize workflow {}: {e}", row.id))
    })
}

#[async_trait]
impl WorkflowStore for PgStore {
    async fn create_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let definition = serde_json::to_value(wf)
            .map_err(|e| OrbflowError::Database(format!("postgres: serialize workflow: {e}")))?;

        let description = wf.description.as_deref().unwrap_or("");
        let status = serde_json::to_value(wf.status)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "draft".to_owned());

        sqlx::query(
            r#"INSERT INTO workflows (id, name, description, version, status, definition, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(wf.id.0.as_str())
        .bind(&wf.name)
        .bind(description)
        .bind(wf.version)
        .bind(&status)
        .bind(&definition)
        .bind(wf.created_at)
        .bind(wf.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                OrbflowError::AlreadyExists
            } else {
                OrbflowError::Database(format!("postgres: create workflow {}: {e}", wf.id))
            }
        })?;

        Ok(())
    }

    async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, OrbflowError> {
        let row: WorkflowRow = sqlx::query_as(
            r#"SELECT id, name, description, version, status, definition, created_at, updated_at
               FROM workflows WHERE id = $1"#,
        )
        .bind(id.0.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: get workflow {id}: {e}")))?
        .ok_or(OrbflowError::NotFound)?;

        row_to_workflow(row)
    }

    async fn update_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let definition = serde_json::to_value(wf)
            .map_err(|e| OrbflowError::Database(format!("postgres: serialize workflow: {e}")))?;

        let description = wf.description.as_deref().unwrap_or("");
        let status = serde_json::to_value(wf.status)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "draft".to_owned());

        // Optimistic locking: update only if the version matches (previous version).
        let prev_version = wf.version - 1;

        // All operations within a single transaction to prevent TOCTOU races.
        let mut tx = self.pool.begin().await.map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: begin transaction for workflow {}: {e}",
                wf.id
            ))
        })?;

        // Auto-snapshot: capture the current definition before overwriting.
        // SELECT ... FOR UPDATE locks the row for the duration of the transaction.
        let existing_row: Option<WorkflowRow> = sqlx::query_as(
            r#"SELECT id, name, description, version, status, definition, created_at, updated_at
               FROM workflows WHERE id = $1 FOR UPDATE"#,
        )
        .bind(wf.id.0.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: fetch workflow for snapshot {}: {e}",
                wf.id
            ))
        })?;

        if let Some(row) = &existing_row {
            // Insert snapshot within the same transaction.
            sqlx::query(
                r#"INSERT INTO workflow_versions (workflow_id, version, definition)
                   VALUES ($1, $2, $3)
                   ON CONFLICT (workflow_id, version) DO NOTHING"#,
            )
            .bind(&row.id)
            .bind(row.version)
            .bind(&row.definition)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!(
                    "postgres: snapshot workflow version {}: {e}",
                    wf.id
                ))
            })?;
        }

        let result = sqlx::query(
            r#"UPDATE workflows SET name=$1, description=$2, version=$3, status=$4,
               definition=$5, updated_at=$6 WHERE id=$7 AND version=$8"#,
        )
        .bind(&wf.name)
        .bind(description)
        .bind(wf.version)
        .bind(&status)
        .bind(&definition)
        .bind(wf.updated_at)
        .bind(wf.id.0.as_str())
        .bind(prev_version)
        .execute(&mut *tx)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: update workflow {}: {e}", wf.id)))?;

        if result.rows_affected() == 0 {
            // Check whether the workflow exists at all (still within the transaction).
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflows WHERE id = $1)")
                    .bind(wf.id.0.as_str())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| {
                        OrbflowError::Database(format!("postgres: check workflow {}: {e}", wf.id))
                    })?;

            // Transaction will be rolled back on drop.
            if exists {
                return Err(OrbflowError::Conflict);
            }
            return Err(OrbflowError::NotFound);
        }

        tx.commit().await.map_err(|e| {
            OrbflowError::Database(format!("postgres: commit workflow update {}: {e}", wf.id))
        })?;

        Ok(())
    }

    async fn delete_workflow(&self, id: &WorkflowId) -> Result<(), OrbflowError> {
        let result = sqlx::query("DELETE FROM workflows WHERE id = $1")
            .bind(id.0.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: delete workflow {id}: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    async fn list_workflows(
        &self,
        opts: ListOptions,
    ) -> Result<(Vec<Workflow>, i64), OrbflowError> {
        let limit = if opts.limit > 0 {
            opts.limit
        } else {
            DEFAULT_PAGE_SIZE
        };
        let offset = opts.offset.max(0);

        let rows: Vec<WorkflowRowWithTotal> = sqlx::query_as(
            r#"SELECT id, name, description, version, status, definition, created_at, updated_at,
                      COUNT(*) OVER() AS total
               FROM workflows ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: list workflows: {e}")))?;

        let total = rows.first().map(|r| r.total).unwrap_or(0);
        let workflows = rows
            .iter()
            .map(row_with_total_to_workflow)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((workflows, total))
    }

    async fn save_workflow_version(&self, version: &WorkflowVersion) -> Result<(), OrbflowError> {
        self.snapshot_workflow_version(
            version.workflow_id.0.as_str(),
            version.version,
            &version.definition,
        )
        .await
    }

    async fn list_workflow_versions(
        &self,
        id: &WorkflowId,
        opts: ListOptions,
    ) -> Result<(Vec<WorkflowVersion>, i64), OrbflowError> {
        self.list_versions(id, opts).await
    }

    async fn get_workflow_version(
        &self,
        id: &WorkflowId,
        version: i32,
    ) -> Result<WorkflowVersion, OrbflowError> {
        self.get_version(id, version).await
    }
}
