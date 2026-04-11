// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Conversion functions between core domain types and gRPC JSON representations.

use std::collections::HashMap;

use orbflow_core::error::OrbflowError;
use orbflow_core::execution::Instance;
use orbflow_core::ports::{DEFAULT_PAGE_SIZE, ListOptions};
use orbflow_core::workflow::{Workflow, WorkflowId};

/// Deserializes a workflow from raw JSON bytes (the `definition` field in the gRPC request).
pub fn workflow_from_bytes(data: &[u8]) -> Result<Workflow, OrbflowError> {
    serde_json::from_slice(data)
        .map_err(|e| OrbflowError::InvalidNodeConfig(format!("invalid workflow definition: {e}")))
}

/// Serializes a workflow to JSON bytes for a gRPC response.
pub fn workflow_to_bytes(wf: &Workflow) -> Result<Vec<u8>, OrbflowError> {
    serde_json::to_vec(wf).map_err(|e| OrbflowError::Internal(format!("serialize workflow: {e}")))
}

/// Serializes an instance to JSON bytes for a gRPC response.
pub fn instance_to_bytes(inst: &Instance) -> Result<Vec<u8>, OrbflowError> {
    serde_json::to_vec(inst).map_err(|e| OrbflowError::Internal(format!("serialize instance: {e}")))
}

/// Parses optional input JSON bytes into a HashMap.
#[allow(dead_code)]
pub fn parse_input(data: &[u8]) -> Result<HashMap<String, serde_json::Value>, OrbflowError> {
    if data.is_empty() {
        return Ok(HashMap::new());
    }
    serde_json::from_slice(data)
        .map_err(|e| OrbflowError::InvalidNodeConfig(format!("invalid input JSON: {e}")))
}

/// Creates a [`WorkflowId`] from a string, validating it is not empty.
pub fn parse_workflow_id(id: &str) -> Result<WorkflowId, OrbflowError> {
    if id.is_empty() {
        return Err(OrbflowError::InvalidNodeConfig(
            "workflow_id is required".into(),
        ));
    }
    Ok(WorkflowId::new(id))
}

/// Creates [`ListOptions`] from gRPC offset/limit.
pub fn parse_list_options(offset: i32, limit: i32) -> ListOptions {
    ListOptions {
        offset: offset as i64,
        limit: if limit > 0 {
            limit as i64
        } else {
            DEFAULT_PAGE_SIZE
        },
    }
}
