// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bus subject naming conventions.

/// Root prefix for all orbflow bus subjects.
pub const SUBJECT_PREFIX: &str = "orbflow";

/// Returns the subject for dispatching tasks to a worker pool.
pub fn task_subject(pool: &str) -> String {
    format!("{SUBJECT_PREFIX}.tasks.{pool}")
}

/// Returns the subject for publishing results from a worker pool.
pub fn result_subject(pool: &str) -> String {
    format!("{SUBJECT_PREFIX}.results.{pool}")
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
