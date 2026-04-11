// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared multi-provider AI client powered by the `genai` crate.
//!
//! Provider routing is automatic based on model name (e.g. `gpt-*` → OpenAI,
//! `claude-*` → Anthropic, `gemini-*` → Google). Authentication is injected
//! per-request from the node's credential data.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use dashmap::DashMap;

use genai::chat::{ChatMessage as GenaiMessage, ChatOptions, ChatRequest, ChatResponseFormat};
use genai::resolver::{AuthData, AuthResolver, Endpoint};
use genai::{Client, ServiceTarget};
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{FieldSchema, FieldType, NodeOutput};
use orbflow_core::streaming::StreamSender;

use crate::util::{float_val, int_val, string_val};

// ─── Client cache ──────────────────────────────────────────────────────────

/// Cache key: (provider name, base_url, api_key_hash).
/// Reuses connection pools for identical provider+key+url combinations.
type ClientCacheKey = (String, String, u64);

static CLIENT_CACHE: LazyLock<DashMap<ClientCacheKey, Arc<Client>>> = LazyLock::new(DashMap::new);

fn hash_api_key(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Returns a cached or newly-built genai Client keyed by provider, base_url,
/// and API key hash. This preserves connection pools across requests to the
/// same provider endpoint, avoiding per-request TLS handshakes.
fn get_or_build_client(config: &AiConfig) -> Result<Arc<Client>, OrbflowError> {
    let key_hash = hash_api_key(&config.api_key);
    let provider_name = format!("{:?}", config.provider);
    let cache_key = (provider_name, config.base_url.clone(), key_hash);

    if let Some(client) = CLIENT_CACHE.get(&cache_key) {
        return Ok(Arc::clone(client.value()));
    }

    let client = Arc::new(build_client(config)?);
    CLIENT_CACHE.insert(cache_key, Arc::clone(&client));
    Ok(client)
}

// ─── Model pricing ──────────────────────────────────────────────────────────

/// Per-model pricing in USD per 1 million tokens.
/// TODO: Move to config file or database for runtime updates.
struct ModelPricing {
    input_per_million: f64,
    output_per_million: f64,
}

impl ModelPricing {
    const fn new(input_per_million: f64, output_per_million: f64) -> Self {
        Self {
            input_per_million,
            output_per_million,
        }
    }
}

/// Default model pricing table. Prefix-matched against model names at lookup time.
/// Each entry is `(provider, model_prefix, pricing)`.
///
/// TODO: Move to config file or database for runtime updates.
static DEFAULT_MODEL_PRICING: LazyLock<Vec<(&str, &str, ModelPricing)>> = LazyLock::new(|| {
    vec![
        // OpenAI — order matters: more specific prefixes first
        ("openai", "gpt-4o-mini", ModelPricing::new(0.15, 0.60)),
        ("openai", "gpt-4o", ModelPricing::new(2.50, 10.00)),
        ("openai", "gpt-4-turbo", ModelPricing::new(10.00, 30.00)),
        ("openai", "o1-mini", ModelPricing::new(3.00, 12.00)),
        ("openai", "o1", ModelPricing::new(15.00, 60.00)),
        // Anthropic — substring match on model name
        ("anthropic", "opus", ModelPricing::new(15.00, 75.00)),
        ("anthropic", "sonnet", ModelPricing::new(3.00, 15.00)),
        ("anthropic", "haiku", ModelPricing::new(0.25, 1.25)),
        // Google AI
        ("google_ai", "flash", ModelPricing::new(0.10, 0.40)),
        ("google_ai", "pro", ModelPricing::new(1.25, 5.00)),
    ]
});

/// Provider-level fallback pricing when no model prefix matches.
static PROVIDER_FALLBACK_PRICING: LazyLock<HashMap<&str, ModelPricing>> = LazyLock::new(|| {
    HashMap::from([
        ("openai", ModelPricing::new(1.00, 3.00)),
        ("anthropic", ModelPricing::new(3.00, 15.00)),
        ("google_ai", ModelPricing::new(0.50, 1.50)),
        ("ollama", ModelPricing::new(0.0, 0.0)),
    ])
});

// ─── Shared schema fields ────────────────────────────────────────────────────

/// Returns the standard provider/model/credential parameter fields shared by all AI nodes.
pub fn ai_common_parameters() -> Vec<FieldSchema> {
    vec![
        FieldSchema {
            key: "provider".into(),
            label: "Provider".into(),
            field_type: FieldType::String,
            required: false,
            default: Some(Value::String("openai".into())),
            description: Some("LLM provider".into()),
            r#enum: vec!["openai".into(), "anthropic".into(), "google_ai".into()],
            credential_type: None,
        },
        FieldSchema {
            key: "model".into(),
            label: "Model".into(),
            field_type: FieldType::String,
            required: false,
            default: Some(Value::String("gpt-4o-mini".into())),
            description: Some("Model name to use".into()),
            r#enum: vec![],
            credential_type: None,
        },
        FieldSchema {
            key: "credential_id".into(),
            label: "Credential".into(),
            field_type: FieldType::Credential,
            required: false,
            default: None,
            description: Some("API credential for the selected provider".into()),
            r#enum: vec![],
            credential_type: Some("openai,anthropic,google_ai".into()),
        },
    ]
}

/// Returns AI output fields for usage tracking (usage + cost_usd), shared by all AI nodes.
pub fn ai_common_outputs() -> Vec<FieldSchema> {
    vec![
        FieldSchema {
            key: "usage".into(),
            label: "Usage".into(),
            field_type: FieldType::Object,
            required: false,
            default: None,
            description: Some(
                "Token usage {prompt_tokens, completion_tokens, total_tokens}".into(),
            ),
            r#enum: vec![],
            credential_type: None,
        },
        FieldSchema {
            key: "cost_usd".into(),
            label: "Cost (USD)".into(),
            field_type: FieldType::Number,
            required: false,
            default: None,
            description: Some("Estimated cost in USD".into()),
            r#enum: vec![],
            credential_type: None,
        },
    ]
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// Supported AI providers (used for cost estimation and config defaults).
#[derive(Debug, Clone, PartialEq)]
pub enum AiProvider {
    OpenAi,
    Anthropic,
    GoogleAi,
    /// Local LLM via Ollama (no API key required, zero cost).
    Ollama,
}

/// Parsed AI configuration extracted from a node config map.
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub provider: AiProvider,
    pub api_key: String,
    pub base_url: String,
    pub organization: Option<String>,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: i64,
    /// True when `base_url` is an unmodified provider default (e.g. OpenAI, Anthropic).
    /// Skips the async DNS-based SSRF check since these URLs are known-safe.
    pub is_default_base_url: bool,
}

/// Token usage reported by the provider.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

/// Unified response from any supported provider.
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub content: String,
    pub model: String,
    pub finish_reason: String,
    pub usage: TokenUsage,
}

// ─── AiConfig ────────────────────────────────────────────────────────────────

impl AiConfig {
    /// Parses an `AiConfig` from a resolved node config map.
    pub fn from_config(cfg: &HashMap<String, Value>) -> Result<Self, OrbflowError> {
        let provider_str = string_val(cfg, "provider", "openai");
        let provider = parse_provider(&provider_str)?;

        let api_key = string_val(cfg, "api_key", "");
        // Ollama doesn't require an API key; all other providers do.
        if api_key.is_empty() && provider != AiProvider::Ollama {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai node: api_key is required".into(),
            ));
        }

        let default_base_url = provider_default_base_url(&provider);
        let user_base_url = string_val(cfg, "base_url", "");
        let is_default_base_url = user_base_url.is_empty();
        let base_url = {
            let url = if is_default_base_url {
                default_base_url.to_owned()
            } else {
                user_base_url
            };
            // genai uses Url::join() which requires a trailing slash to avoid
            // replacing the last path segment (e.g. "/v1" + "chat/completions"
            // becomes "/chat/completions" without the slash).
            if url.ends_with('/') {
                url
            } else {
                format!("{url}/")
            }
        };

        let organization = match cfg.get("organization") {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let model = string_val(cfg, "model", "gpt-4o-mini");
        let temperature = float_val(cfg, "temperature", 0.7).clamp(0.0, 2.0);
        let max_tokens = int_val(cfg, "max_tokens", 1024).clamp(1, 16_384);

        // SSRF protection (synchronous pre-check): block obviously private URLs.
        // The async DNS rebinding check runs in execute_ai_node/chat_completion.
        validate_ai_base_url(&base_url, &provider)?;

        Ok(Self {
            provider,
            api_key,
            base_url,
            organization,
            model,
            temperature,
            max_tokens,
            is_default_base_url,
        })
    }
}

fn parse_provider(s: &str) -> Result<AiProvider, OrbflowError> {
    match s.to_lowercase().as_str() {
        "openai" => Ok(AiProvider::OpenAi),
        "anthropic" => Ok(AiProvider::Anthropic),
        "google_ai" | "googleai" | "google" => Ok(AiProvider::GoogleAi),
        "ollama" => Ok(AiProvider::Ollama),
        other => Err(OrbflowError::InvalidNodeConfig(format!(
            "ai node: unknown provider: {other}; expected openai, anthropic, google_ai, or ollama"
        ))),
    }
}

fn provider_default_base_url(provider: &AiProvider) -> &'static str {
    match provider {
        AiProvider::OpenAi => "https://api.openai.com/v1/",
        AiProvider::Anthropic => "https://api.anthropic.com/",
        AiProvider::GoogleAi => "https://generativelanguage.googleapis.com/v1beta/",
        AiProvider::Ollama => "http://localhost:11434/",
    }
}

/// Validates that a user-supplied `base_url` does not point to private or
/// internal network addresses (SSRF protection).
///
/// Ollama is exempt because it is expected to run on localhost.
fn validate_ai_base_url(url: &str, provider: &AiProvider) -> Result<(), OrbflowError> {
    // Ollama is expected to run on localhost — allow loopback.
    let allow_localhost = matches!(provider, AiProvider::Ollama);
    crate::ssrf::validate_url_not_private(url, allow_localhost)
}

/// Default timeout for AI HTTP calls (2 minutes).
const AI_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

// ─── chat_completion ─────────────────────────────────────────────────────────

/// Sends a chat completion request via `genai`, routing to the correct provider
/// based on the model name. Authentication is injected from `config.api_key`.
pub async fn chat_completion(
    config: &AiConfig,
    system_prompt: Option<&str>,
    user_messages: Vec<(String, String)>, // (role, content) pairs
    json_mode: bool,
) -> Result<AiResponse, OrbflowError> {
    // Async SSRF check with DNS rebinding protection for user-controlled base_url.
    // Skip for default provider URLs (OpenAI, Anthropic, Google, etc.) since the
    // sync check during config construction is sufficient for known-safe URLs.
    //
    // SECURITY: Two-pass validation to narrow the TOCTOU window. We resolve DNS
    // twice — once here (pre-flight reject), and the genai client resolves again
    // internally. A sophisticated DNS rebinding attack could still return a
    // public IP for our check and a private IP for genai's connection. Residual
    // risk: deploy network egress policy (iptables/firewall) blocking metadata
    // IPs (169.254.169.254/32, fd00:ec2::254/128) from worker processes.
    // TODO: When `genai` exposes `with_reqwest_client_builder`, inject
    // SsrfSafeResolver to close this gap at the DNS layer.
    if !config.is_default_base_url {
        let allow_localhost = matches!(config.provider, AiProvider::Ollama);
        crate::ssrf::validate_url_not_private_async(&config.base_url, allow_localhost).await?;
    }

    let mut messages: Vec<GenaiMessage> = Vec::new();
    for (role, content) in &user_messages {
        match role.as_str() {
            "user" => messages.push(GenaiMessage::user(content.as_str())),
            "assistant" => messages.push(GenaiMessage::assistant(content.as_str())),
            _ => messages.push(GenaiMessage::user(content.as_str())),
        }
    }

    let mut chat_req = ChatRequest::from_messages(messages);
    if let Some(sys) = system_prompt {
        chat_req = chat_req.with_system(sys);
    }

    let mut opts = ChatOptions::default()
        .with_max_tokens(config.max_tokens as u32)
        .with_temperature(config.temperature);

    if json_mode {
        opts = opts.with_response_format(ChatResponseFormat::JsonMode);
    }

    // Build a per-request Client with the API key injected via AuthResolver.
    // This avoids the race condition that arises from mutating env vars under
    // Tokio's multi-threaded runtime (concurrent AI nodes cross-routing keys).
    // Ollama uses the "ollama:" model prefix and needs no API key.
    let client = get_or_build_client(config)?;
    let model_id = resolve_model_id(config);
    let response = tokio::time::timeout(
        AI_REQUEST_TIMEOUT,
        client.exec_chat(&model_id, chat_req, Some(&opts)),
    )
    .await
    .map_err(|_| OrbflowError::Timeout)?
    .map_err(|e| {
        tracing::error!(error = %e, "AI API call failed");
        OrbflowError::Internal("AI API request failed".into())
    })?;

    let content = response.content.joined_texts().unwrap_or_default();

    // Extract usage — Usage fields are Option<i32>
    let usage = TokenUsage {
        prompt_tokens: response.usage.prompt_tokens.unwrap_or(0) as i64,
        completion_tokens: response.usage.completion_tokens.unwrap_or(0) as i64,
        total_tokens: response.usage.total_tokens.unwrap_or(0) as i64,
    };

    let model = response.model_iden.model_name.to_string();

    Ok(AiResponse {
        content,
        model,
        finish_reason: "stop".into(),
        usage,
    })
}

// ─── chat_completion_streaming ───────────────────────────────────────────────

/// Sends a chat completion request and streams the response tokens via a
/// [`StreamSender`].
///
/// This uses a simulated streaming approach: the full completion is fetched
/// via [`chat_completion()`], then the response text is broken into
/// word-sized chunks and sent incrementally through the channel. This
/// guarantees the streaming pipeline works end-to-end regardless of whether
/// the underlying LLM SDK supports native streaming.
///
/// The final `StreamChunk::Done` carries the complete [`NodeOutput`] with
/// usage metadata so downstream consumers can render cost / token counts.
pub async fn chat_completion_streaming(
    config: &AiConfig,
    system_prompt: Option<&str>,
    user_messages: Vec<(String, String)>,
    json_mode: bool,
    sender: StreamSender,
) -> Result<NodeOutput, OrbflowError> {
    // 1. Run the full (non-streaming) completion.
    let response = chat_completion(config, system_prompt, user_messages, json_mode).await?;

    // 2. Break the response into word-boundary chunks and stream them.
    let full_text = &response.content;
    let mut cursor = 0usize;
    for word in full_text.split_inclusive(char::is_whitespace) {
        cursor += word.len();
        let is_last = cursor >= full_text.len();
        if let Err(e) = sender
            .send_data(serde_json::json!({ "token": word, "done": is_last }))
            .await
        {
            // Receiver dropped — stop streaming but still return the output.
            tracing::warn!("stream receiver dropped during AI streaming: {e}");
            break;
        }
    }

    // 3. Build the final NodeOutput.
    let usage_json = usage_to_json(&response.usage);
    let cost = estimate_cost(&config.provider, &response.model, &response.usage);
    let data = std::collections::HashMap::from([
        ("content".into(), serde_json::json!(response.content)),
        ("model".into(), serde_json::json!(response.model)),
        (
            "finish_reason".into(),
            serde_json::json!(response.finish_reason),
        ),
        ("usage".into(), usage_json),
        ("cost".into(), serde_json::json!(cost)),
    ]);

    let output = NodeOutput {
        data: Some(data),
        error: None,
    };

    // 4. Send the terminal Done chunk.
    let _ = sender.send_done(output.clone()).await;

    Ok(output)
}

// ─── estimate_cost ───────────────────────────────────────────────────────────

/// Estimates the USD cost of a request using static per-model pricing.
///
/// Prices are in USD per 1 million tokens (input / output).
pub fn estimate_cost(provider: &AiProvider, model: &str, usage: &TokenUsage) -> f64 {
    let (input_price, output_price) = model_pricing(provider, model);
    let input_cost = (usage.prompt_tokens as f64 / 1_000_000.0) * input_price;
    let output_cost = (usage.completion_tokens as f64 / 1_000_000.0) * output_price;
    input_cost + output_cost
}

fn model_pricing(provider: &AiProvider, model: &str) -> (f64, f64) {
    let provider_key = match provider {
        AiProvider::OpenAi => "openai",
        AiProvider::Anthropic => "anthropic",
        AiProvider::GoogleAi => "google_ai",
        AiProvider::Ollama => "ollama",
    };

    // For OpenAI, use prefix matching; for others, use substring (contains) matching.
    let use_prefix = matches!(provider, AiProvider::OpenAi);

    for (p, pattern, pricing) in DEFAULT_MODEL_PRICING.iter() {
        if *p != provider_key {
            continue;
        }
        let matched = if use_prefix {
            model.starts_with(pattern)
        } else {
            model.contains(pattern)
        };
        if matched {
            return (pricing.input_per_million, pricing.output_per_million);
        }
    }

    // Fall back to provider-level default pricing.
    PROVIDER_FALLBACK_PRICING
        .get(provider_key)
        .map(|p| (p.input_per_million, p.output_per_million))
        .unwrap_or((1.00, 3.00))
}

// ─── build_client ───────────────────────────────────────────────────────────

/// Builds a `genai::Client` configured for the given provider.
///
/// For Ollama, uses the "ollama:" model prefix with no API key.
/// For cloud providers, injects the API key via `AuthResolver` and
/// overrides the endpoint when `base_url` differs from the provider default.
///
/// # SSRF Limitation
///
/// Unlike the HTTP builtin node (which uses [`SsrfSafeResolver`] to validate
/// resolved IPs at DNS resolution time), the `genai::Client` uses its own
/// internal `reqwest::Client` with no custom DNS resolver. This creates a
/// TOCTOU window: a DNS rebinding attack could serve a public IP during the
/// pre-flight SSRF check in [`chat_completion`], then rotate to a private IP
/// (e.g. `169.254.169.254`) for the actual connection.
///
/// Mitigations in place:
/// 1. Pre-flight sync check in `AiConfig::from_config` blocks obvious private IPs
/// 2. Pre-flight async DNS check in `chat_completion` validates resolved IPs
/// 3. Default provider URLs (OpenAI, Anthropic, Google) bypass both checks as known-safe
///
/// The residual risk applies only to custom `base_url` overrides. Deploy a
/// network egress policy (firewall/iptables) blocking metadata IP ranges
/// (`169.254.169.254/32`, `fd00:ec2::254/128`) from worker processes.
///
/// TODO: When `genai` exposes `with_reqwest_client_builder` or similar API,
/// inject `SsrfSafeResolver` to close this gap at the DNS resolution layer.
fn build_client(config: &AiConfig) -> Result<Client, OrbflowError> {
    let default_url = provider_default_base_url(&config.provider);
    let needs_custom_endpoint = config.base_url != default_url;

    if config.provider == AiProvider::Ollama {
        if needs_custom_endpoint {
            let base_url = config.base_url.clone();
            Ok(Client::builder()
                .with_service_target_resolver_fn(move |mut st: ServiceTarget| {
                    st.endpoint = Endpoint::from_owned(base_url.clone());
                    Ok(st)
                })
                .build())
        } else {
            Ok(Client::default())
        }
    } else {
        let api_key = config.api_key.clone();
        let auth_resolver = AuthResolver::from_resolver_fn(
            move |_model_iden| -> Result<Option<AuthData>, genai::resolver::Error> {
                Ok(Some(AuthData::from_single(api_key.clone())))
            },
        );

        let mut builder = Client::builder().with_auth_resolver(auth_resolver);

        if needs_custom_endpoint {
            let base_url = config.base_url.clone();
            builder = builder.with_service_target_resolver_fn(move |mut st: ServiceTarget| {
                st.endpoint = Endpoint::from_owned(base_url.clone());
                Ok(st)
            });
        }

        Ok(builder.build())
    }
}

/// Returns the model identifier to pass to genai, prefixing with the provider
/// namespace to force correct adapter routing.
///
/// The `genai` crate auto-detects the provider from known model-name patterns
/// (e.g. `gpt-*` → OpenAI). For unrecognised names it falls back to Ollama,
/// which breaks models like "Qwen/…" when the user selected OpenAI. Using the
/// `provider::model` namespace prefix forces the right adapter every time.
pub fn resolve_model_id(config: &AiConfig) -> String {
    let model = &config.model;

    // If the model already contains a namespace prefix, trust it.
    if model.contains("::") {
        return model.clone();
    }

    let prefix = match config.provider {
        AiProvider::OpenAi => "openai",
        AiProvider::Anthropic => "anthropic",
        AiProvider::GoogleAi => "gemini",
        AiProvider::Ollama => "ollama",
    };
    format!("{prefix}::{model}")
}

// ─── usage_to_json ───────────────────────────────────────────────────────────

/// Converts a `TokenUsage` into a JSON object.
pub fn usage_to_json(usage: &TokenUsage) -> Value {
    serde_json::json!({
        "prompt_tokens": usage.prompt_tokens,
        "completion_tokens": usage.completion_tokens,
        "total_tokens": usage.total_tokens,
    })
}

// ─── execute_ai_node ─────────────────────────────────────────────────────────

/// Shared executor for simple AI nodes that follow the pattern:
/// 1. Validate a required text input
/// 2. Build a system prompt
/// 3. Call `chat_completion` with JSON mode
/// 4. Parse the JSON response and extract fields via a caller-provided closure
///
/// The `extract_fields` closure receives the parsed JSON and returns
/// `(key, value)` pairs for the output. Usage and cost_usd are appended
/// automatically.
pub async fn execute_ai_node(
    input: &orbflow_core::ports::NodeInput,
    node_name: &str,
    text_key: &str,
    system_prompt: &str,
    extract_fields: impl FnOnce(&Value) -> Vec<(&'static str, Value)>,
) -> Result<NodeOutput, OrbflowError> {
    let cfg = crate::util::resolve_config(input);

    let text = string_val(&cfg, text_key, "");
    if text.is_empty() {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "{node_name} node: {text_key} is required"
        )));
    }

    let config = AiConfig::from_config(&cfg)?;

    let response = chat_completion(
        &config,
        Some(system_prompt),
        vec![("user".into(), text)],
        true,
    )
    .await?;

    let cost = estimate_cost(&config.provider, &config.model, &response.usage);
    let usage_val = usage_to_json(&response.usage);

    let parsed = serde_json::from_str::<Value>(&response.content).map_err(|e| {
        OrbflowError::Internal(format!("AI node: model returned invalid JSON — {e}"))
    })?;
    let mut fields = extract_fields(&parsed);
    fields.push(("usage", usage_val));
    let cost_num = serde_json::Number::from_f64(cost)
        .ok_or_else(|| OrbflowError::Internal(format!("AI node: non-finite cost value: {cost}")))?;
    fields.push(("cost_usd", Value::Number(cost_num)));

    Ok(NodeOutput {
        data: Some(crate::util::make_output(fields)),
        error: None,
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cfg(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn test_from_config_openai() {
        let cfg = make_cfg(&[
            ("provider", Value::String("openai".into())),
            ("api_key", Value::String("sk-test".into())),
            ("model", Value::String("gpt-4o".into())),
            ("temperature", serde_json::json!(0.5)),
            ("max_tokens", serde_json::json!(512)),
        ]);
        let ai = AiConfig::from_config(&cfg).unwrap();
        assert_eq!(ai.provider, AiProvider::OpenAi);
        assert_eq!(ai.api_key, "sk-test");
        assert_eq!(ai.model, "gpt-4o");
        assert!((ai.temperature - 0.5).abs() < f64::EPSILON);
        assert_eq!(ai.max_tokens, 512);
        assert_eq!(ai.base_url, "https://api.openai.com/v1/");
    }

    #[test]
    fn test_from_config_missing_api_key() {
        let cfg = make_cfg(&[("provider", Value::String("openai".into()))]);
        assert!(AiConfig::from_config(&cfg).is_err());
    }

    #[test]
    fn test_estimate_cost() {
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };
        // gpt-4o-mini: $0.15 + $0.60 = $0.75
        let cost = estimate_cost(&AiProvider::OpenAi, "gpt-4o-mini", &usage);
        assert!((cost - 0.75).abs() < 1e-6);
        // claude sonnet: $3.00 + $15.00 = $18.00
        let cost = estimate_cost(&AiProvider::Anthropic, "claude-sonnet-4-20250514", &usage);
        assert!((cost - 18.00).abs() < 1e-6);
        // gemini flash: $0.10 + $0.40 = $0.50
        let cost = estimate_cost(&AiProvider::GoogleAi, "gemini-2.0-flash", &usage);
        assert!((cost - 0.50).abs() < 1e-6);
    }

    #[test]
    fn test_parse_provider() {
        assert_eq!(parse_provider("openai").unwrap(), AiProvider::OpenAi);
        assert_eq!(parse_provider("anthropic").unwrap(), AiProvider::Anthropic);
        assert_eq!(parse_provider("google_ai").unwrap(), AiProvider::GoogleAi);
        assert_eq!(parse_provider("ollama").unwrap(), AiProvider::Ollama);
        assert!(parse_provider("unknown").is_err());
    }

    #[test]
    fn test_ollama_no_api_key_required() {
        let cfg = make_cfg(&[
            ("provider", Value::String("ollama".into())),
            ("model", Value::String("llama3.2".into())),
        ]);
        let ai = AiConfig::from_config(&cfg).unwrap();
        assert_eq!(ai.provider, AiProvider::Ollama);
        assert_eq!(ai.model, "llama3.2");
        assert_eq!(ai.base_url, "http://localhost:11434/");
        assert!(ai.api_key.is_empty());
    }

    #[test]
    fn test_ollama_zero_cost() {
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };
        let cost = estimate_cost(&AiProvider::Ollama, "llama3.2", &usage);
        assert!((cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resolve_model_id_ollama() {
        let cfg = make_cfg(&[
            ("provider", Value::String("ollama".into())),
            ("model", Value::String("llama3.2".into())),
        ]);
        let ai = AiConfig::from_config(&cfg).unwrap();
        assert_eq!(resolve_model_id(&ai), "ollama::llama3.2");
    }

    #[test]
    fn test_resolve_model_id_openai() {
        let cfg = make_cfg(&[
            ("provider", Value::String("openai".into())),
            ("api_key", Value::String("sk-test".into())),
            ("model", Value::String("gpt-4o".into())),
        ]);
        let ai = AiConfig::from_config(&cfg).unwrap();
        assert_eq!(resolve_model_id(&ai), "openai::gpt-4o");
    }

    #[test]
    fn test_resolve_model_id_custom_model_forced_openai() {
        let cfg = make_cfg(&[
            ("provider", Value::String("openai".into())),
            ("api_key", Value::String("sk-test".into())),
            ("model", Value::String("Qwen/Qwen3-Next-80B".into())),
        ]);
        let ai = AiConfig::from_config(&cfg).unwrap();
        assert_eq!(resolve_model_id(&ai), "openai::Qwen/Qwen3-Next-80B");
    }

    #[test]
    fn test_resolve_model_id_explicit_prefix_preserved() {
        let cfg = make_cfg(&[
            ("provider", Value::String("openai".into())),
            ("api_key", Value::String("sk-test".into())),
            (
                "model",
                Value::String("anthropic::claude-sonnet-4-20250514".into()),
            ),
        ]);
        let ai = AiConfig::from_config(&cfg).unwrap();
        assert_eq!(resolve_model_id(&ai), "anthropic::claude-sonnet-4-20250514");
    }

    #[test]
    fn test_ssrf_blocks_private_ip() {
        let cfg = make_cfg(&[
            ("provider", Value::String("openai".into())),
            ("api_key", Value::String("sk-test".into())),
            (
                "base_url",
                Value::String("http://169.254.169.254/latest/".into()),
            ),
        ]);
        let err = AiConfig::from_config(&cfg).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("link-local") || msg.contains("cloud metadata") || msg.contains("private"),
            "expected SSRF error, got: {msg}"
        );
    }

    #[test]
    fn test_ssrf_blocks_localhost_for_openai() {
        let cfg = make_cfg(&[
            ("provider", Value::String("openai".into())),
            ("api_key", Value::String("sk-test".into())),
            ("base_url", Value::String("http://localhost:8080/".into())),
        ]);
        assert!(AiConfig::from_config(&cfg).is_err());
    }

    #[test]
    fn test_ssrf_allows_localhost_for_ollama() {
        let cfg = make_cfg(&[
            ("provider", Value::String("ollama".into())),
            ("base_url", Value::String("http://localhost:11434/".into())),
        ]);
        assert!(AiConfig::from_config(&cfg).is_ok());
    }

    #[test]
    fn test_ssrf_blocks_rfc1918() {
        for url in &[
            "http://10.0.0.1/v1/",
            "http://192.168.1.1/v1/",
            "http://172.16.0.1/v1/",
            "http://172.31.255.255/v1/",
        ] {
            let cfg = make_cfg(&[
                ("provider", Value::String("openai".into())),
                ("api_key", Value::String("sk-test".into())),
                ("base_url", Value::String(url.to_string())),
            ]);
            assert!(AiConfig::from_config(&cfg).is_err(), "should block {url}");
        }
    }

    #[test]
    fn test_ssrf_allows_public_url() {
        let cfg = make_cfg(&[
            ("provider", Value::String("openai".into())),
            ("api_key", Value::String("sk-test".into())),
            (
                "base_url",
                Value::String("https://api.openai.com/v1/".into()),
            ),
        ]);
        assert!(AiConfig::from_config(&cfg).is_ok());
    }

    #[test]
    fn test_usage_to_json() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 200,
            total_tokens: 300,
        };
        let v = usage_to_json(&usage);
        assert_eq!(v["prompt_tokens"], 100);
        assert_eq!(v["completion_tokens"], 200);
        assert_eq!(v["total_tokens"], 300);
    }
}
