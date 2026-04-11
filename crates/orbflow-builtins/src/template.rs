// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Template node: template rendering using tera.
//!
//! The Go implementation uses Go's `text/template` with `{{.name}}` syntax.
//! Tera uses Jinja2-style `{{ name }}` syntax. We convert Go-style templates
//! to Tera syntax for compatibility.

use async_trait::async_trait;
use serde_json::Value;
use tera::{Context, Tera};

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::util::{make_output, resolve_config, string_val};

/// Renders a text template with variables.
pub struct TemplateNode;

impl NodeSchemaProvider for TemplateNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:template".into(),
            name: "Template".into(),
            description: "Render a text template with variables".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "file-text".into(),
            color: "#06b6d4".into(),
            image_url: Some("/icons/file-text.svg".into()),
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "template".into(),
                    label: "Template".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some(
                        "Template syntax (e.g. \"Hello {{ name }}\" or \"Hello {{.name}}\")".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "variables".into(),
                    label: "Variables".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("Key-value pairs accessible in the template".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            outputs: vec![FieldSchema {
                key: "result".into(),
                label: "Rendered Text".into(),
                field_type: FieldType::String,
                required: false,
                default: None,
                description: None,
                r#enum: vec![],
                credential_type: None,
            }],
            parameters: vec![],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

/// Converts Go `text/template` syntax to Tera/Jinja2 syntax.
///
/// - `{{.name}}` becomes `{{ name }}`
/// - `{{ .name }}` becomes `{{ name }}`
/// - Handles nested access like `{{.user.name}}` -> `{{ user.name }}`
fn convert_go_template(tmpl: &str) -> String {
    let mut result = String::with_capacity(tmpl.len());
    let chars: Vec<char> = tmpl.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' {
            // Find the closing `}}`
            if let Some(close_pos) = find_closing_braces(&chars, i + 2) {
                let inner = &tmpl[i + 2..close_pos];
                let inner = inner.trim();

                // Convert `.field` to `field` (Go template dot-prefix).
                let converted = inner.strip_prefix('.').unwrap_or(inner);

                result.push_str("{{ ");
                result.push_str(converted);
                result.push_str(" }}");
                i = close_pos + 2;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

fn find_closing_braces(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == '}' && chars[i + 1] == '}' {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[async_trait]
impl NodeExecutor for TemplateNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        const MAX_TEMPLATE_SIZE: usize = 64 * 1024;
        const MAX_VARIABLES_SIZE: usize = 256 * 1024;

        let cfg = resolve_config(input);
        let tmpl_str = string_val(&cfg, "template", "");
        if tmpl_str.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "template node: template is required".into(),
            ));
        }
        if tmpl_str.len() > MAX_TEMPLATE_SIZE {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "template node: template exceeds maximum size ({} > {MAX_TEMPLATE_SIZE} bytes)",
                tmpl_str.len()
            )));
        }

        let vars = cfg
            .get("variables")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let vars_size = serde_json::to_vec(&vars)
            .map_err(|e| OrbflowError::Internal(format!("template node: serialize vars: {e}")))?
            .len();
        if vars_size > MAX_VARIABLES_SIZE {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "template node: variables exceed maximum size ({vars_size} > {MAX_VARIABLES_SIZE} bytes)"
            )));
        }

        // Convert Go-style templates to Tera syntax.
        let tera_template = convert_go_template(&tmpl_str);

        // Render the template.
        let mut tera = Tera::default();
        tera.add_raw_template("node", &tera_template).map_err(|e| {
            OrbflowError::InvalidNodeConfig(format!("template node: invalid template: {e}"))
        })?;

        let context = Context::from_value(vars)
            .map_err(|e| OrbflowError::Internal(format!("template node: context error: {e}")))?;

        let rendered = tera
            .render("node", &context)
            .map_err(|e| OrbflowError::Internal(format!("template node: render failed: {e}")))?;

        Ok(NodeOutput {
            data: Some(make_output(vec![("result", Value::String(rendered))])),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbflow_core::execution::InstanceId;
    use std::collections::HashMap;

    fn make_input(config: HashMap<String, Value>) -> NodeInput {
        NodeInput {
            instance_id: InstanceId::new("inst-1"),
            node_id: "template-1".into(),
            plugin_ref: "builtin:template".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        }
    }

    #[test]
    fn test_convert_go_template() {
        assert_eq!(convert_go_template("Hello {{.name}}"), "Hello {{ name }}");
        assert_eq!(convert_go_template("{{.user.email}}"), "{{ user.email }}");
        assert_eq!(convert_go_template("Hello {{ .name }}"), "Hello {{ name }}");
        // Tera-native syntax should pass through unchanged.
        assert_eq!(convert_go_template("Hello {{ name }}"), "Hello {{ name }}");
    }

    #[tokio::test]
    async fn test_template_render_tera() {
        let node = TemplateNode;
        let mut config = HashMap::new();
        config.insert("template".into(), serde_json::json!("Hello {{ name }}!"));
        config.insert("variables".into(), serde_json::json!({"name": "World"}));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("result").unwrap(), "Hello World!");
    }

    #[tokio::test]
    async fn test_template_render_go_syntax() {
        let node = TemplateNode;
        let mut config = HashMap::new();
        config.insert("template".into(), serde_json::json!("Hello {{.name}}!"));
        config.insert("variables".into(), serde_json::json!({"name": "World"}));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("result").unwrap(), "Hello World!");
    }

    #[tokio::test]
    async fn test_template_missing() {
        let node = TemplateNode;
        let config = HashMap::new();
        let result = node.execute(&make_input(config)).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_template_schema() {
        let node = TemplateNode;
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:template");
    }
}
