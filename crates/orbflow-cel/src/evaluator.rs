// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! CEL evaluator with program caching.

use std::collections::HashMap;
use std::sync::Arc;

use cel_interpreter::{Context, Program};
use parking_lot::RwLock;
use serde_json::Value;

/// CEL expression evaluator with bounded program cache.
///
/// When the cache exceeds `max_cache_size`, half the entries are evicted
/// (arbitrary order — not LRU). This is sufficient for workflow expressions
/// which tend to be long-lived and reused across runs.
/// Maximum time a single CEL expression is allowed to execute before being
/// killed. Prevents DoS via pathological expressions.
const CEL_EXECUTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub struct CelEvaluator {
    cache: RwLock<HashMap<String, Arc<Program>>>,
    max_cache_size: usize,
}

impl CelEvaluator {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            max_cache_size: 10_000,
        }
    }

    fn get_or_compile(&self, expr: &str) -> Result<Arc<Program>, orbflow_core::OrbflowError> {
        const MAX_CEL_EXPR_LEN: usize = 4096;
        if expr.len() > MAX_CEL_EXPR_LEN {
            return Err(orbflow_core::OrbflowError::InvalidNodeConfig(format!(
                "CEL expression exceeds maximum length ({} > {MAX_CEL_EXPR_LEN})",
                expr.len()
            )));
        }

        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(program) = cache.get(expr) {
                return Ok(Arc::clone(program));
            }
        }

        // Compile and cache
        let program = Program::compile(expr)
            .map_err(|e| orbflow_core::OrbflowError::Internal(format!("CEL compile error: {e}")))?;
        let program = Arc::new(program);

        {
            let mut cache = self.cache.write();
            // Evict if over capacity (arbitrary-order half-clear, not LRU)
            if cache.len() >= self.max_cache_size {
                let keys: Vec<String> = cache
                    .keys()
                    .take(self.max_cache_size / 2)
                    .cloned()
                    .collect();
                for k in keys {
                    cache.remove(&k);
                }
            }
            cache.insert(expr.to_owned(), Arc::clone(&program));
        }

        Ok(program)
    }

    /// Evaluates a CEL expression to a boolean result (synchronous).
    ///
    /// Prefer [`eval_bool_async`] from async contexts to avoid blocking the
    /// Tokio runtime on pathological expressions.
    pub fn eval_bool(
        &self,
        expr: &str,
        context: &HashMap<String, Value>,
    ) -> Result<bool, orbflow_core::OrbflowError> {
        let result = self.eval_any(expr, context)?;
        match result {
            Value::Bool(b) => Ok(b),
            _ => Ok(false),
        }
    }

    /// Async variant of [`eval_bool`] that runs on a blocking thread with a
    /// hard timeout. Use this from async engine code.
    pub async fn eval_bool_async(
        &self,
        expr: &str,
        context: &HashMap<String, Value>,
    ) -> Result<bool, orbflow_core::OrbflowError> {
        match self.eval_any_async(expr, context).await? {
            Value::Bool(b) => Ok(b),
            _ => Ok(false),
        }
    }

    /// Evaluates a CEL expression to an arbitrary JSON value.
    ///
    /// Execution uses a wall-clock check after completion. For true
    /// preemptive timeout protection in async contexts, use
    /// [`eval_any_async`] which runs the evaluation on a blocking thread
    /// with a `tokio::time::timeout` guard.
    pub fn eval_any(
        &self,
        expr: &str,
        context: &HashMap<String, Value>,
    ) -> Result<Value, orbflow_core::OrbflowError> {
        let program = self.get_or_compile(expr)?;
        let cel_context = self.build_cel_context(context)?;

        let start = std::time::Instant::now();
        let result = program.execute(&cel_context).map_err(|e| {
            orbflow_core::OrbflowError::Internal(format!("CEL execution error: {e}"))
        })?;

        // Post-execution timeout check — logs slow expressions and rejects
        // those that exceed the budget. Not preemptive, but catches abuse
        // for expressions that do eventually terminate.
        let elapsed = start.elapsed();
        if elapsed > CEL_EXECUTION_TIMEOUT {
            tracing::warn!(
                elapsed_ms = elapsed.as_millis(),
                "CEL expression exceeded execution timeout"
            );
            return Err(orbflow_core::OrbflowError::Timeout);
        }

        Ok(cel_to_json(&result))
    }

    /// Async variant of [`eval_any`] that runs CEL evaluation on a blocking
    /// thread with a hard timeout. Use this from async engine code to prevent
    /// a pathological CEL expression from blocking the Tokio runtime.
    pub async fn eval_any_async(
        &self,
        expr: &str,
        context: &HashMap<String, Value>,
    ) -> Result<Value, orbflow_core::OrbflowError> {
        let program = self.get_or_compile(expr)?;
        let cel_context = self.build_cel_context(context)?;

        tokio::time::timeout(
            CEL_EXECUTION_TIMEOUT,
            tokio::task::spawn_blocking(move || {
                program
                    .execute(&cel_context)
                    .map(|v| cel_to_json(&v))
                    .map_err(|e| {
                        orbflow_core::OrbflowError::Internal(format!("CEL execution error: {e}"))
                    })
            }),
        )
        .await
        .map_err(|_| orbflow_core::OrbflowError::Timeout)?
        .map_err(|e| orbflow_core::OrbflowError::Internal(format!("CEL task panicked: {e}")))?
    }

    /// Builds a CEL context from a JSON value map.
    fn build_cel_context(
        &self,
        context: &HashMap<String, Value>,
    ) -> Result<Context<'static>, orbflow_core::OrbflowError> {
        let mut cel_context = Context::default();
        for (key, val) in context {
            let cel_val = json_to_cel(val);
            cel_context.add_variable(key, cel_val).map_err(|e| {
                orbflow_core::OrbflowError::Internal(format!("CEL context error: {e}"))
            })?;
        }
        Ok(cel_context)
    }
}

impl Default for CelEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds the context map for edge condition evaluation.
pub fn build_edge_context(
    node_outputs: &HashMap<String, HashMap<String, Value>>,
) -> HashMap<String, Value> {
    let mut ctx = HashMap::new();
    ctx.insert(
        "nodes".into(),
        Value::Object(
            node_outputs
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        Value::Object(v.iter().map(|(k2, v2)| (k2.clone(), v2.clone())).collect()),
                    )
                })
                .collect(),
        ),
    );
    ctx
}

/// Builds the context map for input mapping evaluation.
pub fn build_mapping_context(
    node_outputs: &HashMap<String, HashMap<String, Value>>,
    variables: &HashMap<String, Value>,
) -> HashMap<String, Value> {
    let mut ctx = build_edge_context(node_outputs);
    ctx.insert(
        "vars".into(),
        Value::Object(
            variables
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        ),
    );
    ctx
}

fn json_to_cel(val: &Value) -> cel_interpreter::Value {
    match val {
        Value::Null => cel_interpreter::Value::Null,
        Value::Bool(b) => cel_interpreter::Value::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                cel_interpreter::Value::Int(i)
            } else if let Some(u) = n.as_u64() {
                cel_interpreter::Value::UInt(u)
            } else if let Some(f) = n.as_f64() {
                cel_interpreter::Value::Float(f)
            } else {
                cel_interpreter::Value::Null
            }
        }
        Value::String(s) => cel_interpreter::Value::String(s.clone().into()),
        Value::Array(arr) => {
            cel_interpreter::Value::List(arr.iter().map(json_to_cel).collect::<Vec<_>>().into())
        }
        Value::Object(obj) => {
            let map: HashMap<cel_interpreter::objects::Key, cel_interpreter::Value> = obj
                .iter()
                .map(|(k, v)| {
                    (
                        cel_interpreter::objects::Key::String(k.clone().into()),
                        json_to_cel(v),
                    )
                })
                .collect();
            cel_interpreter::Value::Map(map.into())
        }
    }
}

fn cel_to_json(val: &cel_interpreter::Value) -> Value {
    match val {
        cel_interpreter::Value::Null => Value::Null,
        cel_interpreter::Value::Bool(b) => Value::Bool(*b),
        cel_interpreter::Value::Int(i) => Value::Number((*i).into()),
        cel_interpreter::Value::UInt(u) => Value::Number((*u).into()),
        cel_interpreter::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        cel_interpreter::Value::String(s) => Value::String(s.to_string()),
        cel_interpreter::Value::List(list) => Value::Array(list.iter().map(cel_to_json).collect()),
        cel_interpreter::Value::Map(map) => {
            let obj: serde_json::Map<String, Value> = map
                .map
                .iter()
                .map(|(k, v)| {
                    let key_str = match k {
                        cel_interpreter::objects::Key::Int(i) => i.to_string(),
                        cel_interpreter::objects::Key::Uint(u) => u.to_string(),
                        cel_interpreter::objects::Key::Bool(b) => b.to_string(),
                        cel_interpreter::objects::Key::String(s) => s.to_string(),
                    };
                    (key_str, cel_to_json(v))
                })
                .collect();
            Value::Object(obj)
        }
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_bool_true() {
        let eval = CelEvaluator::new();
        let ctx = HashMap::new();
        assert!(eval.eval_bool("true", &ctx).unwrap());
    }

    #[test]
    fn test_eval_bool_false() {
        let eval = CelEvaluator::new();
        let ctx = HashMap::new();
        assert!(!eval.eval_bool("false", &ctx).unwrap());
    }

    #[test]
    fn test_eval_with_variable() {
        let eval = CelEvaluator::new();
        let mut ctx = HashMap::new();
        ctx.insert("x".into(), Value::Number(42.into()));
        let result = eval.eval_bool("x > 10", &ctx).unwrap();
        assert!(result);
    }

    #[test]
    fn test_eval_string_equality() {
        let eval = CelEvaluator::new();
        let mut ctx = HashMap::new();
        ctx.insert("status".into(), Value::String("success".into()));
        assert!(eval.eval_bool("status == \"success\"", &ctx).unwrap());
    }

    #[test]
    fn test_eval_any_arithmetic() {
        let eval = CelEvaluator::new();
        let mut ctx = HashMap::new();
        ctx.insert("a".into(), Value::Number(10.into()));
        ctx.insert("b".into(), Value::Number(20.into()));
        let result = eval.eval_any("a + b", &ctx).unwrap();
        assert_eq!(result, Value::Number(30.into()));
    }

    #[test]
    fn test_cache_reuse() {
        let eval = CelEvaluator::new();
        let ctx = HashMap::new();
        // Compile same expression twice — second should use cache
        eval.eval_bool("true", &ctx).unwrap();
        eval.eval_bool("true", &ctx).unwrap();
        assert_eq!(eval.cache.read().len(), 1);
    }

    #[test]
    fn test_build_edge_context() {
        let mut outputs = HashMap::new();
        let mut node_out = HashMap::new();
        node_out.insert("status".into(), Value::Number(200.into()));
        outputs.insert("http_1".into(), node_out);

        let ctx = build_edge_context(&outputs);
        assert!(ctx.contains_key("nodes"));
    }

    #[test]
    fn test_build_mapping_context() {
        let outputs = HashMap::new();
        let mut vars = HashMap::new();
        vars.insert("input".into(), Value::String("hello".into()));

        let ctx = build_mapping_context(&outputs, &vars);
        assert!(ctx.contains_key("nodes"));
        assert!(ctx.contains_key("vars"));
    }
}
