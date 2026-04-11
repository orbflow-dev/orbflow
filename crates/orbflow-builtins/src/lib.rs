// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Built-in node executors for the Orbflow workflow engine.
//!
//! This crate provides all standard node types: HTTP requests, email sending,
//! data transformation, filtering, sorting, encoding/hashing, delay, template
//! rendering, trigger schemas, and PostgreSQL capability validation.

pub mod ai_chat;
pub mod ai_classify;
pub mod ai_common;
pub mod ai_extract;
pub mod ai_sentiment;
pub mod ai_summarize;
pub mod ai_translate;
pub mod capability_postgres;
pub mod delay;
pub mod email;
pub mod encode;
pub mod filter;
pub mod http;
pub mod log;
pub mod mcp_tool;
pub mod register;
pub mod sort;
pub mod ssrf;
pub mod template;
pub mod transform;
pub mod triggers;
pub mod util;

// Re-export the main registration functions at the crate root.
pub use register::{register_builtins, register_builtins_with};

// Re-export node types for direct construction.
pub use ai_chat::AiChatNode;
pub use ai_classify::AiClassifyNode;
pub use ai_extract::AiExtractNode;
pub use ai_sentiment::AiSentimentNode;
pub use ai_summarize::AiSummarizeNode;
pub use ai_translate::AiTranslateNode;
pub use capability_postgres::CapabilityPostgres;
pub use delay::DelayNode;
pub use email::EmailNode;
pub use encode::EncodeNode;
pub use filter::FilterNode;
pub use http::HttpNode;
pub use log::LogNode;
pub use mcp_tool::McpToolNode;
pub use sort::SortNode;
pub use ssrf::{SsrfSafeResolver, validate_url_not_private, validate_url_not_private_async};
pub use template::TemplateNode;
pub use transform::TransformNode;
pub use triggers::{TriggerCron, TriggerEvent, TriggerManual, TriggerWebhook};
