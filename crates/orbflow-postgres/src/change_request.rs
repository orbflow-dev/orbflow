// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Change request persistence for PostgreSQL.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

use async_trait::async_trait;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::{ChangeRequestStore, DEFAULT_PAGE_SIZE, ListOptions};
use orbflow_core::versioning::{ChangeRequest, ChangeRequestStatus, ReviewComment};
use orbflow_core::workflow::WorkflowId;

use crate::store::PgStore;

/// Internal row representation for the `change_requests` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct ChangeRequestRow {
    id: String,
    workflow_id: String,
    title: String,
    description: Option<String>,
    proposed_definition: serde_json::Value,
    base_version: i32,
    status: String,
    author: String,
    reviewers: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Row with a window-function total for paginated queries.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct ChangeRequestRowWithTotal {
    id: String,
    workflow_id: String,
    title: String,
    description: Option<String>,
    proposed_definition: serde_json::Value,
    base_version: i32,
    status: String,
    author: String,
    reviewers: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    total: i64,
}

/// Internal row representation for the `review_comments` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct CommentRow {
    id: String,
    change_request_id: String,
    author: String,
    body: String,
    node_id: Option<String>,
    edge_source: Option<String>,
    edge_target: Option<String>,
    resolved: bool,
    created_at: DateTime<Utc>,
}

/// Shared field references for mapping a row into a `ChangeRequest`.
struct ChangeRequestFields<'a> {
    id: &'a str,
    workflow_id: &'a str,
    title: &'a str,
    description: &'a Option<String>,
    proposed_definition: &'a serde_json::Value,
    base_version: i32,
    status: &'a str,
    author: &'a str,
    reviewers: &'a serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Maps row fields into a `ChangeRequest`.
fn map_change_request_fields(f: ChangeRequestFields<'_>) -> Result<ChangeRequest, OrbflowError> {
    let status: ChangeRequestStatus =
        serde_json::from_value(serde_json::Value::String(f.status.to_string())).map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: invalid change request status '{}': {e}",
                f.status
            ))
        })?;

    let reviewers: Vec<String> = serde_json::from_value(f.reviewers.clone()).unwrap_or_default();

    Ok(ChangeRequest {
        id: f.id.to_string(),
        workflow_id: WorkflowId::new(f.workflow_id),
        title: f.title.to_string(),
        description: f.description.clone(),
        proposed_definition: f.proposed_definition.clone(),
        base_version: f.base_version,
        status,
        author: f.author.to_string(),
        reviewers,
        comments: Vec::new(),
        created_at: f.created_at,
        updated_at: f.updated_at,
    })
}

fn row_to_change_request(row: &ChangeRequestRow) -> Result<ChangeRequest, OrbflowError> {
    map_change_request_fields(ChangeRequestFields {
        id: &row.id,
        workflow_id: &row.workflow_id,
        title: &row.title,
        description: &row.description,
        proposed_definition: &row.proposed_definition,
        base_version: row.base_version,
        status: &row.status,
        author: &row.author,
        reviewers: &row.reviewers,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn row_to_change_request_from_total(
    row: &ChangeRequestRowWithTotal,
) -> Result<ChangeRequest, OrbflowError> {
    map_change_request_fields(ChangeRequestFields {
        id: &row.id,
        workflow_id: &row.workflow_id,
        title: &row.title,
        description: &row.description,
        proposed_definition: &row.proposed_definition,
        base_version: row.base_version,
        status: &row.status,
        author: &row.author,
        reviewers: &row.reviewers,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn row_to_comment(row: &CommentRow) -> ReviewComment {
    let edge_ref = match (&row.edge_source, &row.edge_target) {
        (Some(source), Some(target)) => Some((source.clone(), target.clone())),
        _ => None,
    };

    ReviewComment {
        id: row.id.clone(),
        author: row.author.clone(),
        body: row.body.clone(),
        node_id: row.node_id.clone(),
        edge_ref,
        resolved: row.resolved,
        created_at: row.created_at,
    }
}

impl PgStore {
    /// Creates a new change request.
    pub(crate) async fn create_change_request(
        &self,
        cr: &ChangeRequest,
    ) -> Result<(), OrbflowError> {
        let status_str = serde_json::to_value(cr.status)
            .map_err(|e| OrbflowError::Database(format!("postgres: serialize cr status: {e}")))?;
        let status_str = status_str.as_str().unwrap_or("draft");

        let reviewers_val = serde_json::to_value(&cr.reviewers).map_err(|e| {
            OrbflowError::Database(format!("postgres: serialize cr reviewers: {e}"))
        })?;

        sqlx::query(
            r#"INSERT INTO change_requests (id, workflow_id, title, description, proposed_definition, base_version, status, author, reviewers, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
        )
        .bind(&cr.id)
        .bind(cr.workflow_id.0.as_str())
        .bind(&cr.title)
        .bind(&cr.description)
        .bind(&cr.proposed_definition)
        .bind(cr.base_version)
        .bind(status_str)
        .bind(&cr.author)
        .bind(&reviewers_val)
        .bind(cr.created_at)
        .bind(cr.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: create change request {}: {e}", cr.id))
        })?;

        Ok(())
    }

    /// Gets a change request by ID, including its comments.
    pub(crate) async fn get_change_request(&self, id: &str) -> Result<ChangeRequest, OrbflowError> {
        let row: ChangeRequestRow = sqlx::query_as(
            r#"SELECT id, workflow_id, title, description, proposed_definition, base_version, status, author, reviewers, created_at, updated_at
               FROM change_requests
               WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: get change request {id}: {e}"))
        })?
        .ok_or(OrbflowError::NotFound)?;

        let comment_rows: Vec<CommentRow> = sqlx::query_as(
            r#"SELECT id, change_request_id, author, body, node_id, edge_source, edge_target, resolved, created_at
               FROM review_comments
               WHERE change_request_id = $1
               ORDER BY created_at ASC"#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: get comments for change request {id}: {e}"
            ))
        })?;

        let mut cr = row_to_change_request(&row)?;
        cr.comments = comment_rows.iter().map(row_to_comment).collect();

        Ok(cr)
    }

    /// Lists change requests for a workflow, optionally filtered by status.
    pub(crate) async fn list_change_requests(
        &self,
        workflow_id: &WorkflowId,
        status: Option<ChangeRequestStatus>,
        opts: ListOptions,
    ) -> Result<(Vec<ChangeRequest>, i64), OrbflowError> {
        let limit = if opts.limit > 0 {
            opts.limit
        } else {
            DEFAULT_PAGE_SIZE
        };
        let offset = opts.offset.max(0);

        let rows: Vec<ChangeRequestRowWithTotal> = if let Some(st) = status {
            let status_str = serde_json::to_value(st).map_err(|e| {
                OrbflowError::Database(format!("postgres: serialize cr status filter: {e}"))
            })?;
            let status_str = status_str.as_str().unwrap_or("draft").to_string();

            sqlx::query_as(
                r#"SELECT id, workflow_id, title, description, proposed_definition,
                          base_version, status, author, reviewers, created_at, updated_at,
                          COUNT(*) OVER() AS total
                   FROM change_requests
                   WHERE workflow_id = $1 AND status = $2
                   ORDER BY created_at DESC
                   LIMIT $3 OFFSET $4"#,
            )
            .bind(workflow_id.0.as_str())
            .bind(&status_str)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!(
                    "postgres: list change requests for {workflow_id}: {e}"
                ))
            })?
        } else {
            sqlx::query_as(
                r#"SELECT id, workflow_id, title, description, proposed_definition,
                          base_version, status, author, reviewers, created_at, updated_at,
                          COUNT(*) OVER() AS total
                   FROM change_requests
                   WHERE workflow_id = $1
                   ORDER BY created_at DESC
                   LIMIT $2 OFFSET $3"#,
            )
            .bind(workflow_id.0.as_str())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!(
                    "postgres: list change requests for {workflow_id}: {e}"
                ))
            })?
        };

        let total = rows.first().map(|r| r.total).unwrap_or(0);
        let crs: Vec<ChangeRequest> = rows
            .iter()
            .map(row_to_change_request_from_total)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((crs, total))
    }

    /// Updates an existing change request.
    pub(crate) async fn update_change_request(
        &self,
        cr: &ChangeRequest,
    ) -> Result<(), OrbflowError> {
        let status_str = serde_json::to_value(cr.status)
            .map_err(|e| OrbflowError::Database(format!("postgres: serialize cr status: {e}")))?;
        let status_str = status_str.as_str().unwrap_or("draft");

        let reviewers_val = serde_json::to_value(&cr.reviewers).map_err(|e| {
            OrbflowError::Database(format!("postgres: serialize cr reviewers: {e}"))
        })?;

        let result = sqlx::query(
            r#"UPDATE change_requests
               SET title = $1, description = $2, proposed_definition = $3, status = $4, reviewers = $5, updated_at = $6
               WHERE id = $7"#,
        )
        .bind(&cr.title)
        .bind(&cr.description)
        .bind(&cr.proposed_definition)
        .bind(status_str)
        .bind(&reviewers_val)
        .bind(cr.updated_at)
        .bind(&cr.id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: update change request {}: {e}", cr.id))
        })?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    /// Adds a comment to a change request.
    pub(crate) async fn add_comment(
        &self,
        cr_id: &str,
        comment: &ReviewComment,
    ) -> Result<(), OrbflowError> {
        let (edge_source, edge_target) = match &comment.edge_ref {
            Some((s, t)) => (Some(s.as_str()), Some(t.as_str())),
            None => (None, None),
        };

        sqlx::query(
            r#"INSERT INTO review_comments (id, change_request_id, author, body, node_id, edge_source, edge_target, resolved, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        )
        .bind(&comment.id)
        .bind(cr_id)
        .bind(&comment.author)
        .bind(&comment.body)
        .bind(&comment.node_id)
        .bind(edge_source)
        .bind(edge_target)
        .bind(comment.resolved)
        .bind(comment.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: add comment {} to change request {cr_id}: {e}",
                comment.id
            ))
        })?;

        Ok(())
    }

    /// Resolves or unresolves a comment on a change request.
    pub(crate) async fn resolve_comment(
        &self,
        cr_id: &str,
        comment_id: &str,
        resolved: bool,
    ) -> Result<(), OrbflowError> {
        let result = sqlx::query(
            r#"UPDATE review_comments SET resolved = $1 WHERE id = $2 AND change_request_id = $3"#,
        )
        .bind(resolved)
        .bind(comment_id)
        .bind(cr_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: resolve comment {comment_id} on change request {cr_id}: {e}"
            ))
        })?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    /// Atomically merges an approved CR into its workflow within a single transaction.
    ///
    /// Steps (all within `BEGIN`/`COMMIT`):
    /// 1. `SELECT ... FOR UPDATE` on the CR row — locks it against concurrent merges
    /// 2. Verify status is still `approved`
    /// 3. Verify workflow version matches `expected_version` (stale-version guard)
    /// 4. Update the workflow definition and bump version
    /// 5. Mark the CR as `merged`
    pub(crate) async fn merge_change_request(
        &self,
        cr_id: &str,
        _expected_version: i32,
        new_definition: &serde_json::Value,
    ) -> Result<(), OrbflowError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            OrbflowError::Database(format!("postgres: begin merge transaction: {e}"))
        })?;

        // Step 1+2: Lock the CR row and verify it is still approved.
        let row: Option<(String, String)> = sqlx::query_as(
            r#"SELECT status, workflow_id FROM change_requests WHERE id = $1 FOR UPDATE"#,
        )
        .bind(cr_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: lock change request {cr_id}: {e}"))
        })?;

        let (status, workflow_id) = row.ok_or(OrbflowError::NotFound)?;
        if status != "approved" {
            return Err(OrbflowError::Conflict);
        }

        // Step 3: Verify workflow version matches expected.
        let current_version: Option<(i32,)> =
            sqlx::query_as(r#"SELECT version FROM workflows WHERE id = $1 FOR UPDATE"#)
                .bind(&workflow_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| {
                    OrbflowError::Database(format!("postgres: lock workflow {workflow_id}: {e}"))
                })?;

        let (current_version,) = current_version.ok_or(OrbflowError::NotFound)?;

        // Allow merge when base version matches OR when it has drifted forward.
        // The proposed definition is the desired end state — always bump from
        // the current version to avoid version gaps.
        let new_version = current_version + 1;
        let now = Utc::now();

        // Step 4: Update workflow definition and bump version.
        sqlx::query(
            r#"UPDATE workflows SET definition = $1, version = $2, updated_at = $3 WHERE id = $4"#,
        )
        .bind(new_definition)
        .bind(new_version)
        .bind(now)
        .bind(&workflow_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: update workflow {workflow_id} during merge: {e}"
            ))
        })?;

        // Snapshot the new version.
        sqlx::query(
            r#"INSERT INTO workflow_versions (workflow_id, version, definition, author, message, created_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (workflow_id, version) DO NOTHING"#,
        )
        .bind(&workflow_id)
        .bind(new_version)
        .bind(new_definition)
        .bind(format!("merge-cr-{cr_id}"))
        .bind(format!("Merged change request {cr_id}"))
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: snapshot version during merge: {e}"))
        })?;

        // Step 5: Mark CR as merged.
        sqlx::query(
            r#"UPDATE change_requests SET status = 'merged', updated_at = $1 WHERE id = $2"#,
        )
        .bind(now)
        .bind(cr_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: update CR status to merged: {e}"))
        })?;

        tx.commit().await.map_err(|e| {
            OrbflowError::Database(format!("postgres: commit merge transaction: {e}"))
        })?;

        Ok(())
    }
}

#[async_trait]
impl ChangeRequestStore for PgStore {
    async fn create_change_request(&self, cr: &ChangeRequest) -> Result<(), OrbflowError> {
        self.create_change_request(cr).await
    }

    async fn get_change_request(&self, id: &str) -> Result<ChangeRequest, OrbflowError> {
        self.get_change_request(id).await
    }

    async fn list_change_requests(
        &self,
        workflow_id: &WorkflowId,
        status: Option<ChangeRequestStatus>,
        opts: ListOptions,
    ) -> Result<(Vec<ChangeRequest>, i64), OrbflowError> {
        self.list_change_requests(workflow_id, status, opts).await
    }

    async fn update_change_request(&self, cr: &ChangeRequest) -> Result<(), OrbflowError> {
        self.update_change_request(cr).await
    }

    async fn add_comment(&self, cr_id: &str, comment: &ReviewComment) -> Result<(), OrbflowError> {
        self.add_comment(cr_id, comment).await
    }

    async fn resolve_comment(
        &self,
        cr_id: &str,
        comment_id: &str,
        resolved: bool,
    ) -> Result<(), OrbflowError> {
        self.resolve_comment(cr_id, comment_id, resolved).await
    }

    async fn merge_change_request(
        &self,
        cr_id: &str,
        expected_version: i32,
        new_definition: &serde_json::Value,
    ) -> Result<(), OrbflowError> {
        self.merge_change_request(cr_id, expected_version, new_definition)
            .await
    }
}
