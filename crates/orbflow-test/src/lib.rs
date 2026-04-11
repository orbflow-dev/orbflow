// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Workflow testing framework for Orbflow.
//!
//! Provides test primitives for:
//! - **Node tests**: Execute a single node with mock inputs and assert outputs.
//! - **Workflow tests**: Execute a full workflow with mock node executors.
//! - **Assertions**: Declarative output matching (exact, contains, type checks).
//!
//! # Example
//!
//! ```ignore
//! let result = NodeTestCase::new("builtin:http")
//!     .with_config("url", json!("https://httpbin.org/get"))
//!     .with_config("method", json!("GET"))
//!     .expect_output("status", json!(200))
//!     .run(&executor)
//!     .await;
//! assert!(result.passed);
//! ```

pub mod assertions;
pub mod runner;
pub mod types;

pub use assertions::{Assertion, AssertionResult, evaluate_with_error, get_nested};
pub use runner::TestRunner;
pub use types::{NodeTestCase, TestOutcome, TestReport, TestSuiteConfig};
