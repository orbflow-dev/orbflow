// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! RBAC persistence for PostgreSQL.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

use async_trait::async_trait;

use crate::store::is_unique_violation;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::RbacStore;
use orbflow_core::rbac::{Permission, PolicyBinding, PolicyScope, RbacPolicy, Role};

use crate::store::PgStore;

/// Internal row representation for the `rbac_roles` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct RoleRow {
    id: String,
    name: String,
    permissions: serde_json::Value,
    description: Option<String>,
    builtin: bool,
    created_at: DateTime<Utc>,
}

/// Internal row representation for the `rbac_bindings` table.
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct BindingRow {
    id: i64,
    subject: String,
    role_id: String,
    scope: serde_json::Value,
    created_at: DateTime<Utc>,
}

fn row_to_role(row: &RoleRow) -> Result<Role, OrbflowError> {
    let permissions: Vec<Permission> =
        serde_json::from_value(row.permissions.clone()).map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: deserialize permissions for role '{}': {e}",
                row.id
            ))
        })?;

    Ok(Role {
        id: row.id.clone(),
        name: row.name.clone(),
        permissions,
        description: row.description.clone(),
        builtin: row.builtin,
    })
}

fn row_to_binding(row: &BindingRow) -> Result<PolicyBinding, OrbflowError> {
    let scope: PolicyScope = serde_json::from_value(row.scope.clone()).map_err(|e| {
        OrbflowError::Database(format!(
            "postgres: deserialize scope for binding (subject='{}', role='{}'): {e}",
            row.subject, row.role_id
        ))
    })?;

    Ok(PolicyBinding {
        subject: row.subject.clone(),
        role_id: row.role_id.clone(),
        scope,
    })
}

impl PgStore {
    /// Loads all roles from the database.
    pub(crate) async fn rbac_list_roles(&self) -> Result<Vec<Role>, OrbflowError> {
        let rows: Vec<RoleRow> = sqlx::query_as(
            r#"SELECT id, name, permissions, description, builtin, created_at
               FROM rbac_roles
               ORDER BY created_at ASC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: list rbac roles: {e}")))?;

        rows.iter().map(row_to_role).collect()
    }

    /// Loads all bindings from the database, optionally filtered by subject.
    pub(crate) async fn rbac_list_bindings(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<PolicyBinding>, OrbflowError> {
        let rows: Vec<BindingRow> = if let Some(subj) = subject {
            sqlx::query_as(
                r#"SELECT id, subject, role_id, scope, created_at
                   FROM rbac_bindings
                   WHERE subject = $1
                   ORDER BY created_at ASC"#,
            )
            .bind(subj)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!("postgres: list rbac bindings for '{subj}': {e}"))
            })?
        } else {
            sqlx::query_as(
                r#"SELECT id, subject, role_id, scope, created_at
                   FROM rbac_bindings
                   ORDER BY created_at ASC"#,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: list rbac bindings: {e}")))?
        };

        rows.iter().map(row_to_binding).collect()
    }

    /// Loads the full RBAC policy (all roles + all bindings).
    pub(crate) async fn rbac_load_policy(&self) -> Result<RbacPolicy, OrbflowError> {
        let roles = self.rbac_list_roles().await?;
        let bindings = self.rbac_list_bindings(None).await?;
        Ok(RbacPolicy { roles, bindings })
    }

    /// Replaces the entire RBAC policy within a single transaction.
    pub(crate) async fn rbac_save_policy(&self, policy: &RbacPolicy) -> Result<(), OrbflowError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            OrbflowError::Database(format!("postgres: begin rbac save_policy transaction: {e}"))
        })?;

        // Delete all existing bindings first (FK constraint), then roles.
        sqlx::query("DELETE FROM rbac_bindings")
            .execute(&mut *tx)
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: delete rbac bindings: {e}")))?;

        sqlx::query("DELETE FROM rbac_roles")
            .execute(&mut *tx)
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: delete rbac roles: {e}")))?;

        // Insert new roles.
        for role in &policy.roles {
            let permissions_val = serde_json::to_value(&role.permissions).map_err(|e| {
                OrbflowError::Database(format!(
                    "postgres: serialize permissions for role '{}': {e}",
                    role.id
                ))
            })?;

            sqlx::query(
                r#"INSERT INTO rbac_roles (id, name, permissions, description, builtin)
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(&role.id)
            .bind(&role.name)
            .bind(&permissions_val)
            .bind(&role.description)
            .bind(role.builtin)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!("postgres: insert rbac role '{}': {e}", role.id))
            })?;
        }

        // Insert new bindings.
        for binding in &policy.bindings {
            let scope_val = serde_json::to_value(&binding.scope).map_err(|e| {
                OrbflowError::Database(format!(
                    "postgres: serialize scope for binding (subject='{}', role='{}'): {e}",
                    binding.subject, binding.role_id
                ))
            })?;

            sqlx::query(
                r#"INSERT INTO rbac_bindings (subject, role_id, scope)
                   VALUES ($1, $2, $3)"#,
            )
            .bind(&binding.subject)
            .bind(&binding.role_id)
            .bind(&scope_val)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!(
                    "postgres: insert rbac binding (subject='{}', role='{}'): {e}",
                    binding.subject, binding.role_id
                ))
            })?;
        }

        tx.commit().await.map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: commit rbac save_policy transaction: {e}"
            ))
        })?;

        Ok(())
    }

    /// Creates a single role.
    pub(crate) async fn rbac_create_role(&self, role: &Role) -> Result<(), OrbflowError> {
        let permissions_val = serde_json::to_value(&role.permissions).map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: serialize permissions for role '{}': {e}",
                role.id
            ))
        })?;

        sqlx::query(
            r#"INSERT INTO rbac_roles (id, name, permissions, description, builtin)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(&role.id)
        .bind(&role.name)
        .bind(&permissions_val)
        .bind(&role.description)
        .bind(role.builtin)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                OrbflowError::AlreadyExists
            } else {
                OrbflowError::Database(format!("postgres: create rbac role '{}': {e}", role.id))
            }
        })?;

        Ok(())
    }

    /// Deletes a role by ID. Bindings referencing this role are cascade-deleted.
    pub(crate) async fn rbac_delete_role(&self, role_id: &str) -> Result<(), OrbflowError> {
        let result = sqlx::query("DELETE FROM rbac_roles WHERE id = $1 AND builtin = FALSE")
            .bind(role_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!("postgres: delete rbac role '{role_id}': {e}"))
            })?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    /// Updates a non-builtin role's name, permissions, and description.
    pub(crate) async fn rbac_update_role(&self, role: &Role) -> Result<(), OrbflowError> {
        let permissions_val = serde_json::to_value(&role.permissions).map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: serialize permissions for role '{}': {e}",
                role.id
            ))
        })?;

        let result = sqlx::query(
            r#"UPDATE rbac_roles
               SET name = $2, permissions = $3, description = $4
               WHERE id = $1 AND builtin = FALSE"#,
        )
        .bind(&role.id)
        .bind(&role.name)
        .bind(&permissions_val)
        .bind(&role.description)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: update rbac role '{}': {e}", role.id))
        })?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    /// Adds a policy binding.
    pub(crate) async fn rbac_add_binding(
        &self,
        binding: &PolicyBinding,
    ) -> Result<(), OrbflowError> {
        let scope_val = serde_json::to_value(&binding.scope).map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: serialize scope for binding (subject='{}', role='{}'): {e}",
                binding.subject, binding.role_id
            ))
        })?;

        sqlx::query(
            r#"INSERT INTO rbac_bindings (subject, role_id, scope)
               VALUES ($1, $2, $3)"#,
        )
        .bind(&binding.subject)
        .bind(&binding.role_id)
        .bind(&scope_val)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                OrbflowError::AlreadyExists
            } else {
                OrbflowError::Database(format!(
                    "postgres: add rbac binding (subject='{}', role='{}'): {e}",
                    binding.subject, binding.role_id
                ))
            }
        })?;

        Ok(())
    }

    /// Lists all distinct subjects from the `rbac_bindings` table.
    pub(crate) async fn rbac_list_subjects(&self) -> Result<Vec<String>, OrbflowError> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT subject FROM rbac_bindings ORDER BY subject ASC")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| {
                    OrbflowError::Database(format!("postgres: list rbac subjects: {e}"))
                })?;

        Ok(rows.into_iter().map(|(s,)| s).collect())
    }

    /// Removes a specific policy binding by (subject, role_id, scope).
    pub(crate) async fn rbac_remove_binding(
        &self,
        subject: &str,
        role_id: &str,
        scope: &PolicyScope,
    ) -> Result<(), OrbflowError> {
        let scope_val = serde_json::to_value(scope).map_err(|e| {
            OrbflowError::Database(format!("postgres: serialize scope for remove binding: {e}"))
        })?;

        let result = sqlx::query(
            r#"DELETE FROM rbac_bindings
               WHERE subject = $1 AND role_id = $2 AND scope = $3"#,
        )
        .bind(subject)
        .bind(role_id)
        .bind(&scope_val)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!(
                "postgres: remove rbac binding (subject='{subject}', role='{role_id}'): {e}"
            ))
        })?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }
}

#[async_trait]
impl RbacStore for PgStore {
    async fn load_policy(&self) -> Result<RbacPolicy, OrbflowError> {
        self.rbac_load_policy().await
    }

    async fn save_policy(&self, policy: &RbacPolicy) -> Result<(), OrbflowError> {
        self.rbac_save_policy(policy).await
    }

    async fn create_role(&self, role: &Role) -> Result<(), OrbflowError> {
        self.rbac_create_role(role).await
    }

    async fn delete_role(&self, role_id: &str) -> Result<(), OrbflowError> {
        self.rbac_delete_role(role_id).await
    }

    async fn update_role(&self, role: &Role) -> Result<(), OrbflowError> {
        self.rbac_update_role(role).await
    }

    async fn list_roles(&self) -> Result<Vec<Role>, OrbflowError> {
        self.rbac_list_roles().await
    }

    async fn add_binding(&self, binding: &PolicyBinding) -> Result<(), OrbflowError> {
        self.rbac_add_binding(binding).await
    }

    async fn remove_binding(
        &self,
        subject: &str,
        role_id: &str,
        scope: &PolicyScope,
    ) -> Result<(), OrbflowError> {
        self.rbac_remove_binding(subject, role_id, scope).await
    }

    async fn list_bindings(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<PolicyBinding>, OrbflowError> {
        self.rbac_list_bindings(subject).await
    }

    async fn list_subjects(&self) -> Result<Vec<String>, OrbflowError> {
        self.rbac_list_subjects().await
    }
}
