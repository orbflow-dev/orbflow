// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Registers all built-in node executors with the engine.

use std::sync::Arc;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{Engine, NodeExecutor, NodeSchemaProvider};

use crate::ai_chat::AiChatNode;
use crate::ai_classify::AiClassifyNode;
use crate::ai_extract::AiExtractNode;
use crate::ai_sentiment::AiSentimentNode;
use crate::ai_summarize::AiSummarizeNode;
use crate::ai_translate::AiTranslateNode;
use crate::capability_postgres::CapabilityPostgres;
use crate::delay::DelayNode;
use crate::email::EmailNode;
use crate::encode::EncodeNode;
use crate::filter::FilterNode;
use crate::http::HttpNode;
use crate::log::LogNode;
use crate::mcp_tool::McpToolNode;
use crate::sort::SortNode;
use crate::template::TemplateNode;
use crate::transform::TransformNode;
use crate::triggers::{TriggerCron, TriggerEvent, TriggerManual, TriggerWebhook};

/// Helper: extracts the schema from a node and registers both executor + schema.
fn reg(
    engine: &dyn Engine,
    name: &str,
    node: impl NodeExecutor + NodeSchemaProvider + 'static,
) -> Result<(), OrbflowError> {
    let schema = node.node_schema();
    engine.register_node_with_schema(name, Arc::new(node), schema)
}

/// Registers every standard built-in node type with the given engine.
///
/// The SubWorkflowExecutor is intentionally excluded because it requires an
/// engine reference; callers that need it should register it separately.
pub fn register_builtins(engine: &dyn Engine) -> Result<(), OrbflowError> {
    // Core action nodes.
    reg(engine, "builtin:log", LogNode)?;
    reg(engine, "builtin:http", HttpNode)?;
    reg(engine, "builtin:delay", DelayNode)?;
    reg(engine, "builtin:transform", TransformNode::new())?;
    reg(engine, "builtin:email", EmailNode::new())?;
    reg(engine, "builtin:template", TemplateNode)?;
    reg(engine, "builtin:encode", EncodeNode)?;
    reg(engine, "builtin:filter", FilterNode::new())?;
    // Trigger nodes (resolved inline by engine; schemas served via API).
    reg(engine, "builtin:trigger-manual", TriggerManual)?;
    reg(engine, "builtin:trigger-cron", TriggerCron)?;
    reg(engine, "builtin:trigger-webhook", TriggerWebhook)?;
    reg(engine, "builtin:trigger-event", TriggerEvent)?;
    // Capability nodes.
    reg(engine, "builtin:capability-postgres", CapabilityPostgres)?;
    // Data processing nodes.
    reg(engine, "builtin:sort", SortNode)?;
    // AI nodes.
    reg(engine, "builtin:ai-chat", AiChatNode)?;
    reg(engine, "builtin:ai-extract", AiExtractNode)?;
    reg(engine, "builtin:ai-classify", AiClassifyNode)?;
    reg(engine, "builtin:ai-summarize", AiSummarizeNode)?;
    reg(engine, "builtin:ai-sentiment", AiSentimentNode)?;
    reg(engine, "builtin:ai-translate", AiTranslateNode)?;
    // MCP nodes.
    reg(engine, "builtin:mcp_tool", McpToolNode)?;

    Ok(())
}

/// Helper: creates a node, extracts its schema, and calls the register callback.
fn reg_with<F>(register: &F, name: &str, node: impl NodeExecutor + NodeSchemaProvider + 'static)
where
    F: Fn(&str, Arc<dyn NodeExecutor>, orbflow_core::ports::NodeSchema),
{
    let schema = node.node_schema();
    register(name, Arc::new(node), schema);
}

/// A callback-based registration alternative that doesn't require an Engine reference.
///
/// Useful when callers don't have a full Engine (e.g., standalone worker).
/// Registers both executors AND schemas (matching `register_builtins`).
pub fn register_builtins_with<F>(register: F)
where
    F: Fn(&str, Arc<dyn NodeExecutor>, orbflow_core::ports::NodeSchema),
{
    reg_with(&register, "builtin:log", LogNode);
    reg_with(&register, "builtin:http", HttpNode);
    reg_with(&register, "builtin:delay", DelayNode);
    reg_with(&register, "builtin:transform", TransformNode::new());
    reg_with(&register, "builtin:email", EmailNode::new());
    reg_with(&register, "builtin:template", TemplateNode);
    reg_with(&register, "builtin:encode", EncodeNode);
    reg_with(&register, "builtin:filter", FilterNode::new());
    reg_with(&register, "builtin:trigger-manual", TriggerManual);
    reg_with(&register, "builtin:trigger-cron", TriggerCron);
    reg_with(&register, "builtin:trigger-webhook", TriggerWebhook);
    reg_with(&register, "builtin:trigger-event", TriggerEvent);
    reg_with(&register, "builtin:capability-postgres", CapabilityPostgres);
    reg_with(&register, "builtin:sort", SortNode);
    reg_with(&register, "builtin:ai-chat", AiChatNode);
    reg_with(&register, "builtin:ai-extract", AiExtractNode);
    reg_with(&register, "builtin:ai-classify", AiClassifyNode);
    reg_with(&register, "builtin:ai-summarize", AiSummarizeNode);
    reg_with(&register, "builtin:ai-sentiment", AiSentimentNode);
    reg_with(&register, "builtin:ai-translate", AiTranslateNode);
    reg_with(&register, "builtin:mcp_tool", McpToolNode);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_register_builtins_with_callback() {
        let registered = Arc::new(Mutex::new(Vec::new()));
        let registered_clone = Arc::clone(&registered);

        register_builtins_with(move |name, _executor, _schema| {
            registered_clone.lock().unwrap().push(name.to_owned());
        });

        let names = registered.lock().unwrap();
        assert_eq!(names.len(), 21);
        assert!(names.contains(&"builtin:log".to_owned()));
        assert!(names.contains(&"builtin:http".to_owned()));
        assert!(names.contains(&"builtin:delay".to_owned()));
        assert!(names.contains(&"builtin:transform".to_owned()));
        assert!(names.contains(&"builtin:email".to_owned()));
        assert!(names.contains(&"builtin:template".to_owned()));
        assert!(names.contains(&"builtin:encode".to_owned()));
        assert!(names.contains(&"builtin:filter".to_owned()));
        assert!(names.contains(&"builtin:trigger-manual".to_owned()));
        assert!(names.contains(&"builtin:trigger-cron".to_owned()));
        assert!(names.contains(&"builtin:trigger-webhook".to_owned()));
        assert!(names.contains(&"builtin:trigger-event".to_owned()));
        assert!(names.contains(&"builtin:capability-postgres".to_owned()));
        assert!(names.contains(&"builtin:sort".to_owned()));
    }
}
