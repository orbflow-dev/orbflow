// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PostgreSQL store wrapping `sqlx::PgPool`.

use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use orbflow_core::audit::AuditSigner;
use orbflow_core::error::OrbflowError;
use orbflow_core::ports::Store;

/// Options for constructing a [`PgStore`].
#[derive(Clone, Default)]
pub struct PgStoreOptions {
    /// AES-256-GCM key (32 bytes) for encrypting credential data.
    pub encryption_key: Option<Vec<u8>>,
    /// Optional audit signer for digitally signing event hashes.
    pub audit_signer: Option<Arc<dyn AuditSigner>>,
}

/// PostgreSQL-backed implementation of the Orbflow Store traits.
///
/// Implements [`WorkflowStore`], [`InstanceStore`], [`EventStore`],
/// [`CredentialStore`], and [`AtomicInstanceCreator`].
pub struct PgStore {
    pub(crate) pool: PgPool,
    pub(crate) opts: PgStoreOptions,
}

impl PgStore {
    /// Creates a new `PgStore` by connecting to the database and running migrations.
    pub async fn new(dsn: &str, opts: PgStoreOptions) -> Result<Self, OrbflowError> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .min_connections(2)
            .connect(dsn)
            .await
            .map_err(|e| OrbflowError::Database(format!("postgres: connect: {e}")))?;

        let store = Self { pool, opts };

        // Run migrations on connect.
        store.migrate().await?;

        Ok(store)
    }

    /// Creates a `PgStore` from an existing pool (useful for testing).
    pub fn from_pool(pool: PgPool, opts: PgStoreOptions) -> Self {
        Self { pool, opts }
    }

    /// Runs database migrations.
    pub async fn migrate(&self) -> Result<(), OrbflowError> {
        crate::migrate::run_migrations(&self.pool).await
    }

    /// Closes the connection pool.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// PgStore implements the composite Store trait.
impl Store for PgStore {}

/// Checks if a sqlx error is a unique constraint violation (PostgreSQL error code 23505).
pub(crate) fn is_unique_violation(e: &sqlx::Error) -> bool {
    match e {
        sqlx::Error::Database(db_err) => db_err.code().as_deref() == Some("23505"),
        _ => false,
    }
}
