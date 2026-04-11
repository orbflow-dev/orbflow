// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Embedded SQL migrations for the Orbflow database schema.

use orbflow_core::error::OrbflowError;
use sqlx::PgPool;

/// All migration SQL statements in order.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial",
        r#"
CREATE TABLE IF NOT EXISTS workflows (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    version     INTEGER NOT NULL DEFAULT 1,
    status      TEXT NOT NULL DEFAULT 'active',
    definition  JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS workflow_instances (
    id          TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflows(id),
    status      TEXT NOT NULL DEFAULT 'pending',
    data        JSONB NOT NULL,
    version     BIGINT NOT NULL DEFAULT 1,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_instances_workflow_id ON workflow_instances(workflow_id);
CREATE INDEX IF NOT EXISTS idx_instances_status ON workflow_instances(status);

CREATE TABLE IF NOT EXISTS events (
    id          BIGSERIAL PRIMARY KEY,
    instance_id TEXT NOT NULL REFERENCES workflow_instances(id),
    event_type  TEXT NOT NULL,
    data        JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_events_instance_id ON events(instance_id);

CREATE TABLE IF NOT EXISTS snapshots (
    instance_id TEXT PRIMARY KEY REFERENCES workflow_instances(id),
    data        JSONB NOT NULL,
    version     BIGINT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
"#,
    ),
    (
        "002_performance",
        r#"
CREATE INDEX IF NOT EXISTS idx_events_instance_id_id ON events(instance_id, id);
CREATE INDEX IF NOT EXISTS idx_instances_status_created ON workflow_instances(status, created_at DESC);
"#,
    ),
    (
        "003_credentials",
        r#"
CREATE TABLE IF NOT EXISTS credentials (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    type        TEXT NOT NULL DEFAULT 'custom',
    data        BYTEA NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credentials_type ON credentials(type);
"#,
    ),
    (
        "004_metrics",
        r#"
CREATE TABLE IF NOT EXISTS node_metrics (
    id BIGSERIAL PRIMARY KEY,
    instance_id TEXT NOT NULL REFERENCES workflow_instances(id) ON DELETE CASCADE,
    workflow_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    plugin_ref TEXT NOT NULL,
    status TEXT NOT NULL,
    duration_ms BIGINT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL,
    attempt INT NOT NULL DEFAULT 1,
    tokens BIGINT,
    cost_usd_scaled BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS instance_metrics (
    instance_id TEXT PRIMARY KEY REFERENCES workflow_instances(id) ON DELETE CASCADE,
    workflow_id TEXT NOT NULL,
    status TEXT NOT NULL,
    duration_ms BIGINT NOT NULL,
    node_count INT NOT NULL,
    failed_node_count INT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL,
    node_durations JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_node_metrics_workflow ON node_metrics(workflow_id, created_at);
CREATE INDEX IF NOT EXISTS idx_node_metrics_instance ON node_metrics(instance_id);
CREATE INDEX IF NOT EXISTS idx_instance_metrics_workflow ON instance_metrics(workflow_id, created_at);
"#,
    ),
    (
        "005_workflow_versions",
        r#"
CREATE TABLE IF NOT EXISTS workflow_versions (
    id BIGSERIAL PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    version INTEGER NOT NULL,
    definition JSONB NOT NULL,
    author TEXT,
    message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(workflow_id, version)
);
CREATE INDEX IF NOT EXISTS idx_workflow_versions_wf ON workflow_versions(workflow_id, version);
"#,
    ),
    (
        "006_audit_chain",
        r#"
ALTER TABLE events ADD COLUMN IF NOT EXISTS event_hash TEXT;
ALTER TABLE events ADD COLUMN IF NOT EXISTS prev_hash TEXT;
CREATE INDEX IF NOT EXISTS idx_events_hash ON events(event_hash);
"#,
    ),
    (
        "007_change_requests",
        r#"
CREATE TABLE IF NOT EXISTS change_requests (
    id                  TEXT PRIMARY KEY,
    workflow_id         TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    title               TEXT NOT NULL,
    description         TEXT,
    proposed_definition JSONB NOT NULL,
    base_version        INTEGER NOT NULL,
    status              TEXT NOT NULL DEFAULT 'draft',
    author              TEXT NOT NULL,
    reviewers           JSONB NOT NULL DEFAULT '[]',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS review_comments (
    id                  TEXT PRIMARY KEY,
    change_request_id   TEXT NOT NULL REFERENCES change_requests(id) ON DELETE CASCADE,
    author              TEXT NOT NULL,
    body                TEXT NOT NULL,
    node_id             TEXT,
    edge_source         TEXT,
    edge_target         TEXT,
    resolved            BOOLEAN NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cr_workflow ON change_requests(workflow_id, status);
CREATE INDEX IF NOT EXISTS idx_cr_status ON change_requests(status);
CREATE INDEX IF NOT EXISTS idx_comments_cr ON review_comments(change_request_id);
"#,
    ),
    (
        "008_change_request_constraints",
        r#"
ALTER TABLE change_requests
    ADD CONSTRAINT chk_cr_status
    CHECK (status IN ('draft', 'open', 'approved', 'rejected', 'merged'));

ALTER TABLE change_requests
    ADD CONSTRAINT chk_cr_base_version
    CHECK (base_version >= 1);

ALTER TABLE review_comments
    ADD CONSTRAINT chk_comment_edge_pair
    CHECK ((edge_source IS NULL) = (edge_target IS NULL));

DROP INDEX IF EXISTS idx_cr_status;

CREATE INDEX IF NOT EXISTS idx_cr_active
    ON change_requests(workflow_id, created_at DESC)
    WHERE status IN ('draft', 'open', 'approved');
"#,
    ),
    (
        "009_rbac",
        r#"
CREATE TABLE IF NOT EXISTS rbac_roles (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    permissions JSONB NOT NULL DEFAULT '[]',
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS rbac_bindings (
    id       BIGSERIAL PRIMARY KEY,
    subject  TEXT NOT NULL,
    role_id  TEXT NOT NULL REFERENCES rbac_roles(id) ON DELETE CASCADE,
    scope    JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(subject, role_id, scope)
);

CREATE INDEX IF NOT EXISTS idx_rbac_bindings_subject ON rbac_bindings(subject);
CREATE INDEX IF NOT EXISTS idx_rbac_bindings_role ON rbac_bindings(role_id);
"#,
    ),
    (
        "010_audit_signatures",
        r#"
ALTER TABLE events ADD COLUMN IF NOT EXISTS signature TEXT;
"#,
    ),
    (
        "011_budgets",
        r#"
CREATE TABLE IF NOT EXISTS budgets (
    id TEXT PRIMARY KEY,
    workflow_id TEXT,
    team TEXT,
    period TEXT NOT NULL DEFAULT 'monthly',
    limit_usd DOUBLE PRECISION NOT NULL,
    current_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    reset_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_budgets_workflow ON budgets(workflow_id);
"#,
    ),
    (
        "012_alerts",
        r#"
CREATE TABLE IF NOT EXISTS alert_rules (
    id TEXT PRIMARY KEY,
    workflow_id TEXT,
    metric TEXT NOT NULL,
    operator TEXT NOT NULL,
    threshold DOUBLE PRECISION NOT NULL,
    channel JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_alerts_workflow ON alert_rules(workflow_id);
CREATE INDEX IF NOT EXISTS idx_alerts_enabled ON alert_rules(enabled) WHERE enabled = TRUE;
"#,
    ),
    (
        "013_credentials_owner_scope",
        r#"
ALTER TABLE credentials
    ADD COLUMN IF NOT EXISTS owner_id TEXT;

CREATE INDEX IF NOT EXISTS idx_credentials_owner_id ON credentials(owner_id);
"#,
    ),
    (
        "014_rbac_builtin_roles",
        r#"
ALTER TABLE rbac_roles ADD COLUMN IF NOT EXISTS builtin BOOLEAN NOT NULL DEFAULT FALSE;
UPDATE rbac_roles SET builtin = TRUE WHERE id IN ('viewer', 'editor', 'operator', 'admin');
"#,
    ),
    (
        "015_credential_tiers",
        r#"
ALTER TABLE credentials
    ADD COLUMN IF NOT EXISTS access_tier TEXT NOT NULL DEFAULT 'proxy';

ALTER TABLE credentials
    ADD COLUMN IF NOT EXISTS policy JSONB;
"#,
    ),
];

/// Runs all embedded migrations in order.
///
/// Uses an internal `_orbflow_migrations` table to track which migrations have
/// already been applied, ensuring idempotency.
pub(crate) async fn run_migrations(pool: &PgPool) -> Result<(), OrbflowError> {
    // Create the migration tracking table if it doesn't exist.
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _orbflow_migrations (
            name       TEXT PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| OrbflowError::Database(format!("postgres: create migration table: {e}")))?;

    for (name, sql) in MIGRATIONS {
        // Check if already applied.
        let applied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _orbflow_migrations WHERE name = $1)")
                .bind(name)
                .fetch_one(pool)
                .await
                .map_err(|e| {
                    OrbflowError::Database(format!("postgres: check migration {name}: {e}"))
                })?;

        if applied {
            tracing::debug!(migration = name, "migration already applied, skipping");
            continue;
        }

        tracing::info!(migration = name, "applying migration");

        // Use raw_sql to allow multiple statements in a single migration.
        // sqlx::query() uses the prepared-statement protocol which rejects multi-statement SQL.
        sqlx::raw_sql(sql).execute(pool).await.map_err(|e| {
            OrbflowError::Database(format!("postgres: apply migration {name}: {e}"))
        })?;

        sqlx::query("INSERT INTO _orbflow_migrations (name) VALUES ($1)")
            .bind(name)
            .execute(pool)
            .await
            .map_err(|e| {
                OrbflowError::Database(format!("postgres: record migration {name}: {e}"))
            })?;
    }

    Ok(())
}
