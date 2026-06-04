// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bus subject naming conventions.

use std::{error::Error, fmt};

/// Root prefix for all orbflow bus subjects.
pub const SUBJECT_PREFIX: &str = "orbflow";

/// Maximum worker-pool token length accepted for subject/durable construction.
pub const MAX_POOL_NAME_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectNameError {
    EmptyPool,
    PoolTooLong { max: usize },
    InvalidPoolCharacter { ch: char, index: usize },
}

impl fmt::Display for SubjectNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPool => write!(f, "worker pool name must not be empty"),
            Self::PoolTooLong { max } => {
                write!(f, "worker pool name must be at most {max} characters")
            }
            Self::InvalidPoolCharacter { ch, index } => write!(
                f,
                "worker pool name contains invalid character '{ch}' at byte {index}; use ASCII letters, digits, '_' or '-'"
            ),
        }
    }
}

impl Error for SubjectNameError {}

/// Validates a worker-pool token before using it in a NATS subject or durable name.
///
/// This intentionally rejects dots, wildcards, whitespace, and non-ASCII
/// characters so pool names stay a single deterministic subject token.
pub fn validate_pool_name(pool: &str) -> Result<(), SubjectNameError> {
    if pool.is_empty() {
        return Err(SubjectNameError::EmptyPool);
    }
    if pool.len() > MAX_POOL_NAME_LEN {
        return Err(SubjectNameError::PoolTooLong {
            max: MAX_POOL_NAME_LEN,
        });
    }
    for (index, ch) in pool.char_indices() {
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
            return Err(SubjectNameError::InvalidPoolCharacter { ch, index });
        }
    }
    Ok(())
}

/// Deterministically encodes an arbitrary pool name into a safe subject token.
///
/// Runtime adapters may choose validation-only behavior for operator mistakes,
/// or use this helper when they need a reversible-free stable token for legacy
/// pool names that already contain dots or spaces.
pub fn encode_pool_name(pool: &str) -> String {
    if validate_pool_name(pool).is_ok() {
        return pool.to_owned();
    }
    if pool.is_empty() {
        return "pool_empty".to_owned();
    }

    let mut hash = 0xcbf29ce484222325_u64;
    for byte in pool.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("pool_{hash:016x}")
}

/// Returns the subject for dispatching tasks to a worker pool.
pub fn task_subject(pool: &str) -> String {
    format!("{SUBJECT_PREFIX}.tasks.{pool}")
}

/// Validates a worker pool and returns the task subject.
pub fn try_task_subject(pool: &str) -> Result<String, SubjectNameError> {
    validate_pool_name(pool)?;
    Ok(task_subject(pool))
}

/// Encodes a worker pool if needed and returns the task subject.
pub fn safe_task_subject(pool: &str) -> String {
    task_subject(&encode_pool_name(pool))
}

/// Returns the subject for publishing results from a worker pool.
pub fn result_subject(pool: &str) -> String {
    format!("{SUBJECT_PREFIX}.results.{pool}")
}

/// Validates a worker pool and returns the result subject.
pub fn try_result_subject(pool: &str) -> Result<String, SubjectNameError> {
    validate_pool_name(pool)?;
    Ok(result_subject(pool))
}

/// Encodes a worker pool if needed and returns the result subject.
pub fn safe_result_subject(pool: &str) -> String {
    result_subject(&encode_pool_name(pool))
}

/// Returns the subject for publishing streaming chunks for an instance/node.
pub fn stream_subject(instance_id: &str, node_id: &str) -> String {
    format!("{SUBJECT_PREFIX}.stream.{instance_id}.{node_id}")
}

/// Returns the subject for notifying workers to reload plugins from disk.
pub fn plugin_reload_subject() -> String {
    format!("{SUBJECT_PREFIX}.worker.reload-plugins")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_subject() {
        assert_eq!(task_subject("default"), "orbflow.tasks.default");
        assert_eq!(task_subject("gpu"), "orbflow.tasks.gpu");
    }

    #[test]
    fn test_result_subject() {
        assert_eq!(result_subject("default"), "orbflow.results.default");
    }

    #[test]
    fn pool_validation_accepts_single_subject_tokens() {
        assert!(validate_pool_name("default").is_ok());
        assert!(validate_pool_name("gpu_pool-1").is_ok());
        assert_eq!(
            try_task_subject("gpu_pool-1").unwrap(),
            "orbflow.tasks.gpu_pool-1"
        );
    }

    #[test]
    fn pool_validation_rejects_subject_wildcards_and_dots() {
        assert!(validate_pool_name("").is_err());
        assert!(validate_pool_name("prod.us").is_err());
        assert!(validate_pool_name("prod>").is_err());
        assert!(validate_pool_name("prod*").is_err());
        assert!(validate_pool_name("prod us").is_err());
    }

    #[test]
    fn pool_encoding_is_deterministic_and_safe() {
        let encoded = encode_pool_name("prod.us");
        assert_eq!(encoded, encode_pool_name("prod.us"));
        assert_eq!(
            safe_result_subject("prod.us"),
            format!("orbflow.results.{encoded}")
        );
        assert!(validate_pool_name(&encoded).is_ok());
        assert!(validate_pool_name(&encode_pool_name(&"x.".repeat(80))).is_ok());
    }

    #[test]
    fn test_stream_subject() {
        assert_eq!(
            stream_subject("inst-1", "node-2"),
            "orbflow.stream.inst-1.node-2"
        );
    }

    #[test]
    fn test_plugin_reload_subject() {
        assert_eq!(plugin_reload_subject(), "orbflow.worker.reload-plugins");
    }
}
