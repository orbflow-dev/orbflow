// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Axum-based HTTP REST API for Orbflow.

pub mod errors;
pub mod handlers;
pub mod middleware;
mod router;

pub use handlers::AppState;
pub use middleware::{AuthUser, StartRateLimiter, check_permission};
pub use router::{HttpApiOptions, create_router};
