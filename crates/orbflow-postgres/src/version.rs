// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! WorkflowVersion persistence for PostgreSQL.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::{DEFAULT_PAGE_SIZE, ListOptions};
use orbflow_core::versioning::WorkflowVersion;
use orbflow_core::workflow::WorkflowId;

use crate::store::PgStore;

/// Internal row representation for the `workflow_versions` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct VersionRow {
    id: i64,
    workflow_id: String,
    version: i32,
    definition: serde_json::Value,
    author: Option<String>,
    message: Option<String>,
    created_at: DateTime<Utc>,
}

/// Row with a window-function total for paginated queries.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct VersionRowWithTotal {
    id: i64,
    workflow_id: String,
    version: i32,
    definition: serde_json::Value,
    author: Option<String>,
    message: Option<String>,
    created_at: DateTime<Utc>,
    total: i64,
}

fn row_to_version(row: &VersionRowWithTotal) -> WorkflowVersion {
    WorkflowVersion {
        version: row.version,
        workflow_id: WorkflowId::new(&row.workflow_id),
        definition: row.definition.clone(),
        author: row.author.clone(),
        message: row.message.clone(),
        created_at: row.created_at,
    }
}

fn single_row_to_version(row: VersionRow) -> WorkflowVersion {
    WorkflowVersion {
        version: row.version,
        workflow_id: WorkflowId::new(&row.workflow_id),
        definition: row.definition,
        author: row.author,
        message: row.message,
        created_at: row.created_at,
    }
}

impl PgStore {
    /// Snapshots the current workflow definition into the `workflow_versions` table.
    ///
    /// Called automatically before each workflow update to preserve history.
    pub(crate) async fn snapshot_workflow_version(
        &self,
        workflow_id: &str,
        version: i32,
        definition: &serde_json::Value,
    ) -> Result<(), OrbflowError> {
        sqlx::query(
            r#"INSERT INTO workflow_versions (workflow_id, version, definition)
               VALUES ($1, $2, $3)
               ON CONFLICT (workflow_id, version) DO NOTHING"#,
        )
        .bind(workflow_id)
        .bind(version)
        .bind(definition)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: snapshot workflow version {workflow_id}@v{version}: {e}"
            ))
        })?;

        Ok(())
    }

    /// Lists version history for a workflow, ordered by version descending.
    pub(crate) async fn list_versions(
        &self,
        id: &WorkflowId,
        opts: ListOptions,
    ) -> Result<(Vec<WorkflowVersion>, i64), OrbflowError> {
        let limit = if opts.limit > 0 {
            opts.limit
        } else {
            DEFAULT_PAGE_SIZE
        };
        let offset = opts.offset.max(0);

        let rows: Vec<VersionRowWithTotal> = sqlx::query_as(
            r#"SELECT id, workflow_id, version, definition, author, message, created_at,
                      COUNT(*) OVER() AS total
               FROM workflow_versions
               WHERE workflow_id = $1
               ORDER BY version DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(id.0.as_str())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: list workflow versions {id}: {e}"))
        })?;

        let total = rows.first().map(|r| r.total).unwrap_or(0);
        let versions: Vec<WorkflowVersion> = rows.iter().map(row_to_version).collect();

        Ok((versions, total))
    }

    /// Gets a specific version snapshot of a workflow.
    pub(crate) async fn get_version(
        &self,
        id: &WorkflowId,
        version: i32,
    ) -> Result<WorkflowVersion, OrbflowError> {
        let row: VersionRow = sqlx::query_as(
            r#"SELECT id, workflow_id, version, definition, author, message, created_at
               FROM workflow_versions
               WHERE workflow_id = $1 AND version = $2"#,
        )
        .bind(id.0.as_str())
        .bind(version)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: get workflow version {id}@v{version}: {e}"
            ))
        })?
        .ok_or(OrbflowError::NotFound)?;

        Ok(single_row_to_version(row))
    }
}
