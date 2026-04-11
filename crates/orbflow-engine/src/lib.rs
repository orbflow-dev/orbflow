// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! DAG workflow engine with saga compensation and crash recovery.
//!
//! This crate provides [`OrbflowEngine`], the single-process implementation of
//! [`orbflow_core::ports::Engine`] that coordinates workflow execution, node
//! dispatching, result handling, saga compensation, and crash recovery.
//!
//! # Architecture
//!
//! - **dag** — Multi-pass DAG evaluator with OR-join skip semantics
//! - **topo** — BFS topological sort (Kahn's algorithm) for compensation ordering
//! - **dedup** — Bounded result dedup set with LRU eviction
//! - **engine** — Core engine struct implementing the Engine trait
//! - **saga** — Saga compensation: reverse topological walk of completed nodes
//! - **resume** — Crash recovery: reset and re-dispatch in-flight nodes
//! - **subworkflow** — Sub-workflow executor: start child, poll until done
//! - **testnode** — Inline node execution without persistence

pub mod alerts;
pub mod budget;
mod dag;
mod dedup;
mod engine;
mod resume;
mod saga;
pub mod sla;
mod subworkflow;
mod testnode;
mod topo;

pub use alerts::AlertEvaluator;
pub use engine::OrbflowEngine;
pub use sla::{SlaCheckResult, SlaConfig, SlaMonitor};
pub use subworkflow::{SUB_WORKFLOW_PLUGIN_REF, SubWorkflowExecutor};
