// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Centralized error types for the Orbflow workflow engine.

use thiserror::Error;

/// All possible errors in the Orbflow system.
#[derive(Debug, Error)]
pub enum OrbflowError {
    #[error("orbflow: not found")]
    NotFound,

    #[error("orbflow: already exists")]
    AlreadyExists,

    #[error("orbflow: cycle detected in workflow DAG")]
    CycleDetected,

    #[error("orbflow: duplicate node ID")]
    DuplicateNode,

    #[error("orbflow: duplicate edge between same source and target")]
    DuplicateEdge,

    #[error("orbflow: invalid edge (unknown source or target)")]
    InvalidEdge,

    #[error("orbflow: workflow has no entry nodes")]
    NoEntryNodes,

    #[error("orbflow: workflow graph is disconnected")]
    Disconnected,

    #[error("orbflow: invalid status transition")]
    InvalidStatus,

    #[error("orbflow: instance cancelled")]
    Cancelled,

    #[error("orbflow: execution timed out")]
    Timeout,

    #[error("orbflow: engine is stopped")]
    EngineStopped,

    #[error("orbflow: node executor not registered")]
    NodeNotFound,

    #[error("orbflow: version conflict")]
    Conflict,

    #[error("orbflow: invalid node kind")]
    InvalidNodeKind,

    #[error("orbflow: required capability not connected")]
    MissingCapability,

    #[error("orbflow: invalid capability edge")]
    InvalidCapabilityEdge,

    #[error("orbflow: invalid node configuration: {0}")]
    InvalidNodeConfig(String),

    #[error("orbflow: node kind (name) must not be empty")]
    EmptyNodeKind,

    #[error("orbflow: node executor must not be nil")]
    NilExecutor,

    #[error("orbflow: node kind already registered: {0}")]
    DuplicateNodeKind(String),

    #[error("orbflow: internal error: {0}")]
    Internal(String),

    #[error("orbflow: crypto error: {0}")]
    Crypto(String),

    #[error("orbflow: database error: {0}")]
    Database(String),

    #[error("orbflow: bus error: {0}")]
    Bus(String),

    #[error("orbflow: forbidden: {0}")]
    Forbidden(String),

    #[error("orbflow: budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("orbflow: invalid policy: {0}")]
    InvalidPolicy(String),
}

impl OrbflowError {
    /// Returns `true` if this error indicates a client-side mistake (bad input)
    /// rather than a server-side failure.
    pub fn is_validation_error(&self) -> bool {
        matches!(
            self,
            Self::CycleDetected
                | Self::DuplicateNode
                | Self::DuplicateEdge
                | Self::InvalidEdge
                | Self::NoEntryNodes
                | Self::Disconnected
                | Self::InvalidCapabilityEdge
                | Self::MissingCapability
                | Self::InvalidNodeConfig(_)
                | Self::InvalidNodeKind
                | Self::InvalidPolicy(_)
        )
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }

    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict)
    }

    pub fn is_budget_exceeded(&self) -> bool {
        matches!(self, Self::BudgetExceeded(_))
    }
}

/// Type alias for Results using OrbflowError.
pub type Result<T> = std::result::Result<T, OrbflowError>;

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper: all variants ────────────────────────────────────────

    fn all_variants() -> Vec<(&'static str, OrbflowError)> {
        vec![
            ("NotFound", OrbflowError::NotFound),
            ("AlreadyExists", OrbflowError::AlreadyExists),
            ("CycleDetected", OrbflowError::CycleDetected),
            ("DuplicateNode", OrbflowError::DuplicateNode),
            ("DuplicateEdge", OrbflowError::DuplicateEdge),
            ("InvalidEdge", OrbflowError::InvalidEdge),
            ("NoEntryNodes", OrbflowError::NoEntryNodes),
            ("Disconnected", OrbflowError::Disconnected),
            ("InvalidStatus", OrbflowError::InvalidStatus),
            ("Cancelled", OrbflowError::Cancelled),
            ("Timeout", OrbflowError::Timeout),
            ("EngineStopped", OrbflowError::EngineStopped),
            ("NodeNotFound", OrbflowError::NodeNotFound),
            ("Conflict", OrbflowError::Conflict),
            ("InvalidNodeKind", OrbflowError::InvalidNodeKind),
            ("MissingCapability", OrbflowError::MissingCapability),
            ("InvalidCapabilityEdge", OrbflowError::InvalidCapabilityEdge),
            (
                "InvalidNodeConfig",
                OrbflowError::InvalidNodeConfig("bad field".into()),
            ),
            ("EmptyNodeKind", OrbflowError::EmptyNodeKind),
            ("NilExecutor", OrbflowError::NilExecutor),
            (
                "DuplicateNodeKind",
                OrbflowError::DuplicateNodeKind("http".into()),
            ),
            ("Internal", OrbflowError::Internal("oops".into())),
            ("Crypto", OrbflowError::Crypto("decrypt failed".into())),
            ("Database", OrbflowError::Database("connection lost".into())),
            ("Bus", OrbflowError::Bus("publish failed".into())),
            ("Forbidden", OrbflowError::Forbidden("no access".into())),
            (
                "BudgetExceeded",
                OrbflowError::BudgetExceeded("limit reached".into()),
            ),
            (
                "InvalidPolicy",
                OrbflowError::InvalidPolicy("bad policy".into()),
            ),
        ]
    }

    // ── is_validation_error ─────────────────────────────────────────

    const VALIDATION_VARIANTS: &[&str] = &[
        "CycleDetected",
        "DuplicateNode",
        "DuplicateEdge",
        "InvalidEdge",
        "NoEntryNodes",
        "Disconnected",
        "InvalidCapabilityEdge",
        "MissingCapability",
        "InvalidNodeConfig",
        "InvalidNodeKind",
        "InvalidPolicy",
    ];

    #[test]
    fn is_validation_error_returns_true_for_validation_variants() {
        for (name, err) in all_variants() {
            if VALIDATION_VARIANTS.contains(&name) {
                assert!(
                    err.is_validation_error(),
                    "{name} should be a validation error"
                );
            }
        }
    }

    #[test]
    fn is_validation_error_returns_false_for_non_validation_variants() {
        for (name, err) in all_variants() {
            if !VALIDATION_VARIANTS.contains(&name) {
                assert!(
                    !err.is_validation_error(),
                    "{name} should NOT be a validation error"
                );
            }
        }
    }

    // ── is_not_found ────────────────────────────────────────────────

    #[test]
    fn is_not_found_returns_true_only_for_not_found() {
        for (name, err) in all_variants() {
            if name == "NotFound" {
                assert!(err.is_not_found(), "NotFound should return true");
            } else {
                assert!(!err.is_not_found(), "{name} should NOT be not_found");
            }
        }
    }

    // ── is_conflict ─────────────────────────────────────────────────

    #[test]
    fn is_conflict_returns_true_only_for_conflict() {
        for (name, err) in all_variants() {
            if name == "Conflict" {
                assert!(err.is_conflict(), "Conflict should return true");
            } else {
                assert!(!err.is_conflict(), "{name} should NOT be conflict");
            }
        }
    }

    // ── is_budget_exceeded ──────────────────────────────────────────

    #[test]
    fn is_budget_exceeded_returns_true_only_for_budget_exceeded() {
        for (name, err) in all_variants() {
            if name == "BudgetExceeded" {
                assert!(
                    err.is_budget_exceeded(),
                    "BudgetExceeded should return true"
                );
            } else {
                assert!(
                    !err.is_budget_exceeded(),
                    "{name} should NOT be budget_exceeded"
                );
            }
        }
    }

    // ── Display impl ────────────────────────────────────────────────

    #[test]
    fn display_not_found() {
        assert_eq!(OrbflowError::NotFound.to_string(), "orbflow: not found");
    }

    #[test]
    fn display_already_exists() {
        assert_eq!(
            OrbflowError::AlreadyExists.to_string(),
            "orbflow: already exists"
        );
    }

    #[test]
    fn display_cycle_detected() {
        assert_eq!(
            OrbflowError::CycleDetected.to_string(),
            "orbflow: cycle detected in workflow DAG"
        );
    }

    #[test]
    fn display_duplicate_node() {
        assert_eq!(
            OrbflowError::DuplicateNode.to_string(),
            "orbflow: duplicate node ID"
        );
    }

    #[test]
    fn display_duplicate_edge() {
        assert_eq!(
            OrbflowError::DuplicateEdge.to_string(),
            "orbflow: duplicate edge between same source and target"
        );
    }

    #[test]
    fn display_invalid_edge() {
        assert_eq!(
            OrbflowError::InvalidEdge.to_string(),
            "orbflow: invalid edge (unknown source or target)"
        );
    }

    #[test]
    fn display_no_entry_nodes() {
        assert_eq!(
            OrbflowError::NoEntryNodes.to_string(),
            "orbflow: workflow has no entry nodes"
        );
    }

    #[test]
    fn display_disconnected() {
        assert_eq!(
            OrbflowError::Disconnected.to_string(),
            "orbflow: workflow graph is disconnected"
        );
    }

    #[test]
    fn display_invalid_status() {
        assert_eq!(
            OrbflowError::InvalidStatus.to_string(),
            "orbflow: invalid status transition"
        );
    }

    #[test]
    fn display_cancelled() {
        assert_eq!(
            OrbflowError::Cancelled.to_string(),
            "orbflow: instance cancelled"
        );
    }

    #[test]
    fn display_timeout() {
        assert_eq!(
            OrbflowError::Timeout.to_string(),
            "orbflow: execution timed out"
        );
    }

    #[test]
    fn display_engine_stopped() {
        assert_eq!(
            OrbflowError::EngineStopped.to_string(),
            "orbflow: engine is stopped"
        );
    }

    #[test]
    fn display_node_not_found() {
        assert_eq!(
            OrbflowError::NodeNotFound.to_string(),
            "orbflow: node executor not registered"
        );
    }

    #[test]
    fn display_conflict() {
        assert_eq!(
            OrbflowError::Conflict.to_string(),
            "orbflow: version conflict"
        );
    }

    #[test]
    fn display_invalid_node_kind() {
        assert_eq!(
            OrbflowError::InvalidNodeKind.to_string(),
            "orbflow: invalid node kind"
        );
    }

    #[test]
    fn display_missing_capability() {
        assert_eq!(
            OrbflowError::MissingCapability.to_string(),
            "orbflow: required capability not connected"
        );
    }

    #[test]
    fn display_invalid_capability_edge() {
        assert_eq!(
            OrbflowError::InvalidCapabilityEdge.to_string(),
            "orbflow: invalid capability edge"
        );
    }

    #[test]
    fn display_invalid_node_config_includes_message() {
        let err = OrbflowError::InvalidNodeConfig("missing url".into());
        assert_eq!(
            err.to_string(),
            "orbflow: invalid node configuration: missing url"
        );
    }

    #[test]
    fn display_empty_node_kind() {
        assert_eq!(
            OrbflowError::EmptyNodeKind.to_string(),
            "orbflow: node kind (name) must not be empty"
        );
    }

    #[test]
    fn display_nil_executor() {
        assert_eq!(
            OrbflowError::NilExecutor.to_string(),
            "orbflow: node executor must not be nil"
        );
    }

    #[test]
    fn display_duplicate_node_kind_includes_name() {
        let err = OrbflowError::DuplicateNodeKind("http".into());
        assert_eq!(
            err.to_string(),
            "orbflow: node kind already registered: http"
        );
    }

    #[test]
    fn display_internal_includes_message() {
        let err = OrbflowError::Internal("unexpected state".into());
        assert_eq!(err.to_string(), "orbflow: internal error: unexpected state");
    }

    #[test]
    fn display_crypto_includes_message() {
        let err = OrbflowError::Crypto("bad key".into());
        assert_eq!(err.to_string(), "orbflow: crypto error: bad key");
    }

    #[test]
    fn display_database_includes_message() {
        let err = OrbflowError::Database("timeout".into());
        assert_eq!(err.to_string(), "orbflow: database error: timeout");
    }

    #[test]
    fn display_bus_includes_message() {
        let err = OrbflowError::Bus("disconnected".into());
        assert_eq!(err.to_string(), "orbflow: bus error: disconnected");
    }

    #[test]
    fn display_forbidden_includes_message() {
        let err = OrbflowError::Forbidden("admin only".into());
        assert_eq!(err.to_string(), "orbflow: forbidden: admin only");
    }

    #[test]
    fn display_budget_exceeded_includes_message() {
        let err = OrbflowError::BudgetExceeded("100 runs used".into());
        assert_eq!(err.to_string(), "orbflow: budget exceeded: 100 runs used");
    }

    // ── Debug impl ──────────────────────────────────────────────────

    #[test]
    fn debug_impl_contains_variant_name() {
        let err = OrbflowError::NotFound;
        let debug = format!("{err:?}");
        assert!(
            debug.contains("NotFound"),
            "Debug should contain variant name"
        );
    }

    #[test]
    fn debug_impl_contains_inner_value_for_string_variants() {
        let err = OrbflowError::Internal("detail".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("detail"), "Debug should contain inner value");
    }

    // ── Error trait (source) ────────────────────────────────────────

    #[test]
    fn error_source_is_none_for_all_variants() {
        use std::error::Error;
        for (name, err) in all_variants() {
            assert!(err.source().is_none(), "{name} should have no error source");
        }
    }

    // ── Result type alias ───────────────────────────────────────────

    #[test]
    fn result_alias_works_with_ok() {
        let result: Result<i32> = Ok(42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn result_alias_works_with_err() {
        let result: Result<i32> = Err(OrbflowError::NotFound);
        assert!(result.is_err());
    }
}
