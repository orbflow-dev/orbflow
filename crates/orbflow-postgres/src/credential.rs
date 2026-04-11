// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! CredentialStore implementation for PostgreSQL with AES-256-GCM encryption.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

use orbflow_core::credential::{Credential, CredentialId, CredentialSummary};
use orbflow_core::credential_proxy::{CredentialAccessTier, CredentialPolicy};
use orbflow_core::crypto;
use orbflow_core::error::OrbflowError;
use orbflow_core::ports::CredentialStore;

use crate::store::PgStore;

/// Internal row representation for the `credentials` table.
#[derive(Debug, FromRow)]
struct CredentialRow {
    id: String,
    name: String,
    r#type: String,
    data: Vec<u8>,
    description: String,
    access_tier: String,
    policy: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Summary row (no data column).
#[derive(Debug, FromRow)]
struct CredentialSummaryRow {
    id: String,
    name: String,
    r#type: String,
    description: String,
    access_tier: String,
    policy: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Tier conversion helpers
// ---------------------------------------------------------------------------

/// Parse a string access tier to the enum, defaulting to Proxy.
fn parse_access_tier(s: &str) -> CredentialAccessTier {
    match s {
        "raw" => CredentialAccessTier::Raw,
        "scoped_token" => CredentialAccessTier::ScopedToken,
        _ => CredentialAccessTier::Proxy,
    }
}

/// Convert access tier enum to its database string.
fn tier_to_str(tier: &CredentialAccessTier) -> &'static str {
    match tier {
        CredentialAccessTier::Proxy => "proxy",
        CredentialAccessTier::ScopedToken => "scoped_token",
        CredentialAccessTier::Raw => "raw",
    }
}

/// Serialize policy to JSON Value for DB storage.
fn policy_to_json(
    policy: &Option<CredentialPolicy>,
) -> Result<Option<serde_json::Value>, OrbflowError> {
    policy
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| OrbflowError::Database(format!("serialize credential policy: {e}")))
}

/// Deserialize policy from JSON Value.
fn policy_from_json(val: Option<serde_json::Value>) -> Option<CredentialPolicy> {
    val.and_then(|v| {
        serde_json::from_value(v)
            .inspect_err(|e| {
                tracing::warn!(error = %e, "credential policy deserialization failed; treating as no policy");
            })
            .ok()
    })
}

/// Convert description column (empty string = NULL convention).
fn desc_opt(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

impl PgStore {
    /// Returns the encryption key, or an error if not configured.
    fn encryption_key(&self) -> Result<&[u8], OrbflowError> {
        self.opts
            .encryption_key
            .as_deref()
            .ok_or_else(|| OrbflowError::Crypto("encryption key not configured".into()))
    }

    /// Encrypts credential data to bytes.
    fn encrypt_data(
        &self,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<u8>, OrbflowError> {
        let key = self.encryption_key()?;
        let plaintext = serde_json::to_vec(data)
            .map_err(|e| OrbflowError::Database(format!("serialize credential data: {e}")))?;
        crypto::encrypt(key, &plaintext)
    }

    /// Decrypts credential data from bytes.
    fn decrypt_data(
        &self,
        encrypted: &[u8],
    ) -> Result<HashMap<String, serde_json::Value>, OrbflowError> {
        let key = self.encryption_key()?;
        let plaintext = crypto::decrypt(key, encrypted)?;
        serde_json::from_slice(&plaintext)
            .map_err(|e| OrbflowError::Database(format!("deserialize credential data: {e}")))
    }
}

impl PgStore {
    /// Checks whether the `owner_id` column exists in the credentials table.
    /// Cached after first successful call — the column either exists or it
    /// doesn't for the lifetime of the process.
    async fn has_owner_column(&self) -> Result<bool, OrbflowError> {
        static HAS_OWNER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if let Some(&v) = HAS_OWNER.get() {
            return Ok(v);
        }
        let result: Option<(i32,)> = sqlx::query_as(
            r#"SELECT 1 FROM information_schema.columns
               WHERE table_name = 'credentials' AND column_name = 'owner_id'"#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: check owner_id column existence: {e}"))
        })?;
        let exists = result.is_some();
        let _ = HAS_OWNER.set(exists);
        Ok(exists)
    }
}

#[async_trait]
impl CredentialStore for PgStore {
    async fn create_credential(&self, cred: &Credential) -> Result<(), OrbflowError> {
        let encrypted = self.encrypt_data(&cred.data)?;
        let description = cred.description.as_deref().unwrap_or("");

        if self.has_owner_column().await? {
            let policy_json = policy_to_json(&cred.policy)?;
            sqlx::query(
                r#"INSERT INTO credentials (id, name, type, data, description, owner_id, access_tier, policy, created_at, updated_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
            )
            .bind(cred.id.0.as_str())
            .bind(&cred.name)
            .bind(&cred.credential_type)
            .bind(&encrypted)
            .bind(description)
            .bind(cred.owner_id.as_deref())
            .bind(tier_to_str(&cred.access_tier))
            .bind(&policy_json)
            .bind(cred.created_at)
            .bind(cred.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!("postgres: create credential {}: {e}", cred.id))
            })?;
        } else {
            sqlx::query(
                r#"INSERT INTO credentials (id, name, type, data, description, created_at, updated_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(cred.id.0.as_str())
            .bind(&cred.name)
            .bind(&cred.credential_type)
            .bind(&encrypted)
            .bind(description)
            .bind(cred.created_at)
            .bind(cred.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!("postgres: create credential {}: {e}", cred.id))
            })?;
        }

        Ok(())
    }

    async fn get_credential_for_owner(
        &self,
        id: &CredentialId,
        owner_id: Option<&str>,
    ) -> Result<Credential, OrbflowError> {
        // Fail hard if the owner_id column is missing — credential isolation
        // is not available without the migration. Never fall back to an
        // unscoped query, as that bypasses tenant isolation entirely.
        if !self.has_owner_column().await? {
            tracing::error!(
                credential_id = %id,
                "owner_id column not present — credential isolation is unavailable; apply migration before starting the service"
            );
            return Err(OrbflowError::Internal(
                "credential store migration required: owner_id column missing".into(),
            ));
        }

        let row: CredentialRow = sqlx::query_as(
            r#"SELECT id, name, type, data, description, access_tier, policy, created_at, updated_at
               FROM credentials
               WHERE id = $1 AND ($2::text IS NULL OR owner_id = $2)"#,
        )
        .bind(id.0.as_str())
        .bind(owner_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: get credential {id} for owner: {e}"))
        })?
        .ok_or(OrbflowError::NotFound)?;

        let data = self.decrypt_data(&row.data)?;
        Ok(Credential {
            id: CredentialId::new(row.id)?,
            name: row.name,
            credential_type: row.r#type,
            data,
            description: desc_opt(row.description),
            owner_id: owner_id.map(str::to_owned),
            access_tier: parse_access_tier(&row.access_tier),
            policy: policy_from_json(row.policy),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn get_credential(&self, id: &CredentialId) -> Result<Credential, OrbflowError> {
        let row: CredentialRow = sqlx::query_as(
            r#"SELECT id, name, type, data, description, access_tier, policy, created_at, updated_at
               FROM credentials WHERE id = $1"#,
        )
        .bind(id.0.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: get credential {id}: {e}")))?
        .ok_or(OrbflowError::NotFound)?;

        let data = self.decrypt_data(&row.data)?;

        Ok(Credential {
            id: CredentialId::new(row.id)?,
            name: row.name,
            credential_type: row.r#type,
            data,
            description: desc_opt(row.description),
            owner_id: None,
            access_tier: parse_access_tier(&row.access_tier),
            policy: policy_from_json(row.policy),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn update_credential(&self, cred: &Credential) -> Result<(), OrbflowError> {
        let encrypted = self.encrypt_data(&cred.data)?;
        let description = cred.description.as_deref().unwrap_or("");
        let policy_json = policy_to_json(&cred.policy)?;

        // Scope UPDATE by owner_id when available to prevent TOCTOU bypass
        // where a concurrent request could overwrite another user's credential.
        let result = if self.has_owner_column().await? {
            sqlx::query(
                r#"UPDATE credentials
                   SET name=$1, type=$2, data=$3, description=$4, owner_id=$5, access_tier=$6, policy=$7, updated_at=$8
                   WHERE id=$9 AND ($5::text IS NULL OR owner_id = $5)"#,
            )
            .bind(&cred.name)
            .bind(&cred.credential_type)
            .bind(&encrypted)
            .bind(description)
            .bind(cred.owner_id.as_deref())
            .bind(tier_to_str(&cred.access_tier))
            .bind(&policy_json)
            .bind(cred.updated_at)
            .bind(cred.id.0.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!("postgres: update credential {}: {e}", cred.id))
            })?
        } else {
            sqlx::query(
                r#"UPDATE credentials SET name=$1, type=$2, data=$3, description=$4, updated_at=$5
                   WHERE id=$6"#,
            )
            .bind(&cred.name)
            .bind(&cred.credential_type)
            .bind(&encrypted)
            .bind(description)
            .bind(cred.updated_at)
            .bind(cred.id.0.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!("postgres: update credential {}: {e}", cred.id))
            })?
        };

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    async fn delete_credential(
        &self,
        id: &CredentialId,
        owner_id: Option<&str>,
    ) -> Result<(), OrbflowError> {
        let result = if self.has_owner_column().await? {
            sqlx::query(
                "DELETE FROM credentials WHERE id = $1 AND ($2::text IS NULL OR owner_id = $2)",
            )
            .bind(id.0.as_str())
            .bind(owner_id)
            .execute(&self.pool)
            .await
        } else {
            sqlx::query("DELETE FROM credentials WHERE id = $1")
                .bind(id.0.as_str())
                .execute(&self.pool)
                .await
        }
        .map_err(|e| OrbflowError::Database(format!("postgres: delete credential {id}: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(OrbflowError::NotFound);
        }

        Ok(())
    }

    async fn list_credentials(&self) -> Result<Vec<CredentialSummary>, OrbflowError> {
        let rows: Vec<CredentialSummaryRow> = sqlx::query_as(
            r#"SELECT id, name, type, description, access_tier, policy, created_at, updated_at
               FROM credentials ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Database(format!("postgres: list credentials: {e}")))?;

        let summaries: Result<Vec<_>, _> = rows
            .into_iter()
            .map(|row| {
                Ok(CredentialSummary {
                    id: CredentialId::new(row.id)?,
                    name: row.name,
                    credential_type: row.r#type,
                    description: desc_opt(row.description),
                    access_tier: Some(parse_access_tier(&row.access_tier)),
                    policy: policy_from_json(row.policy),
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                })
            })
            .collect();

        summaries
    }

    async fn list_credentials_for_owner(
        &self,
        owner_id: Option<&str>,
    ) -> Result<Vec<CredentialSummary>, OrbflowError> {
        if !self.has_owner_column().await? {
            return self.list_credentials().await;
        }

        let rows: Vec<CredentialSummaryRow> = sqlx::query_as(
            r#"SELECT id, name, type, description, access_tier, policy, created_at, updated_at
               FROM credentials
               WHERE ($1::text IS NULL OR owner_id = $1)
               ORDER BY created_at DESC"#,
        )
        .bind(owner_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            OrbflowError::Database(format!("postgres: list credentials for owner: {e}"))
        })?;

        let summaries: Result<Vec<_>, _> = rows
            .into_iter()
            .map(|row| {
                Ok(CredentialSummary {
                    id: CredentialId::new(row.id)?,
                    name: row.name,
                    credential_type: row.r#type,
                    description: desc_opt(row.description),
                    access_tier: Some(parse_access_tier(&row.access_tier)),
                    policy: policy_from_json(row.policy),
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                })
            })
            .collect();

        summaries
    }
}
