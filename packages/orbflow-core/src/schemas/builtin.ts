import type { NodeTypeDefinition } from "../types/schema";

export const httpNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:http",
  name: "HTTP Request",
  description: "Connect to any API or web service",
  category: "builtin",
  icon: "globe",
  color: "#3B82F6",
  imageUrl: "/icons/globe.svg",
  inputs: [
    {
      key: "method",
      label: "Method",
      type: "string",
      required: true,
      default: "GET",
      description: "The HTTP method to use",
      enum: ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"],
    },
    {
      key: "url",
      label: "URL",
      type: "string",
      required: true,
      description: "The web address to send the request to",
    },
    {
      key: "body",
      label: "Body",
      type: "string",
      description: "Data to send with the request",
    },
    {
      key: "headers",
      label: "Headers",
      type: "object",
      description: "Additional request headers",
      children: [],
    },
  ],
  outputs: [
    { key: "status", label: "Status Code", type: "number" },
    { key: "status_text", label: "Status Text", type: "string" },
    { key: "body", label: "Response Body", type: "string", dynamic: true },
    {
      key: "headers",
      label: "Response Headers",
      type: "object",
      children: [],
      dynamic: true,
    },
  ],
};

export const delayNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:delay",
  name: "Wait",
  description: "Pause before continuing to the next step",
  category: "builtin",
  icon: "clock",
  color: "#F59E0B",
  imageUrl: "/icons/clock.svg",
  inputs: [
    {
      key: "duration",
      label: "Duration",
      type: "string",
      required: true,
      default: "5s",
      description: "How long to wait (e.g. 5s, 1m, 500ms)",
    },
  ],
  outputs: [{ key: "delayed", label: "Wait Time", type: "string" }],
};

export const logNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:log",
  name: "Log Output",
  description: "Record data and pass it to the next step",
  category: "builtin",
  icon: "terminal",
  color: "#22D3EE",
  imageUrl: "/icons/terminal.svg",
  inputs: [
    {
      key: "message",
      label: "Message",
      type: "string",
      description: "The message to log",
    },
  ],
  outputs: [{ key: "message", label: "Message", type: "string" }],
};

export const subWorkflowNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:sub-workflow",
  name: "Run Sub-Workflow",
  description: "Run another workflow as part of this one",
  category: "builtin",
  icon: "workflow",
  color: "#7C5CFC",
  imageUrl: "/icons/workflow.svg",
  inputs: [
    {
      key: "workflow_id",
      label: "Workflow",
      type: "string",
      required: true,
      description: "Which workflow to run",
    },
    {
      key: "input",
      label: "Input Data",
      type: "object",
      description: "Data to pass into the child workflow",
    },
  ],
  outputs: [
    { key: "child_instance_id", label: "Run ID", type: "string" },
    { key: "child_workflow_id", label: "Workflow ID", type: "string" },
    { key: "status", label: "Status", type: "string" },
    { key: "outputs", label: "Results", type: "object", dynamic: true },
  ],
};

export const transformNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:transform",
  name: "Transform",
  description: "Reshape data using a CEL expression",
  category: "builtin",
  icon: "zap",
  color: "#8B5CF6",
  imageUrl: "/icons/zap.svg",
  inputs: [
    {
      key: "expression",
      label: "Expression",
      type: "string",
      required: true,
      description: "CEL expression to evaluate (use 'input' to reference data)",
    },
    {
      key: "data",
      label: "Input Data",
      type: "object",
      description: "Data available as 'input' in the expression",
    },
  ],
  outputs: [
    { key: "result", label: "Result", type: "object" },
    { key: "type", label: "Result Type", type: "string" },
  ],
};

export const emailNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:email",
  name: "Send Email",
  description: "Send an email via SMTP",
  category: "builtin",
  icon: "mail",
  color: "#EF4444",
  imageUrl: "/icons/mail.svg",
  inputs: [
    {
      key: "to",
      label: "To",
      type: "string",
      required: true,
      description: "Recipient email(s), comma-separated",
    },
    {
      key: "subject",
      label: "Subject",
      type: "string",
      required: true,
      description: "Email subject line",
    },
    {
      key: "body",
      label: "Body",
      type: "string",
      required: true,
      description: "Email body content",
    },
    {
      key: "from",
      label: "From",
      type: "string",
      description: "Sender address (uses credential or server default if empty)",
    },
    {
      key: "content_type",
      label: "Content Type",
      type: "string",
      default: "text/plain",
      enum: ["text/plain", "text/html"],
      description: "Email content format",
    },
  ],
  parameters: [
    {
      key: "credential_id",
      label: "SMTP Credential",
      type: "credential",
      credentialType: "smtp",
      description: "Select an SMTP credential for connection settings",
    },
  ],
  outputs: [
    { key: "sent", label: "Sent", type: "boolean" },
    { key: "message", label: "Status", type: "string" },
  ],
};

export const templateNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:template",
  name: "Template",
  description: "Render a text template with variables",
  category: "builtin",
  icon: "file-text",
  color: "#06B6D4",
  imageUrl: "/icons/file-text.svg",
  inputs: [
    {
      key: "template",
      label: "Template",
      type: "string",
      required: true,
      description: 'Go template syntax (e.g. "Hello {{.name}}")',
    },
    {
      key: "variables",
      label: "Variables",
      type: "object",
      description: "Key-value pairs accessible as .key in the template",
    },
  ],
  outputs: [{ key: "result", label: "Rendered Text", type: "string" }],
};

export const encodeNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:encode",
  name: "Encode / Hash",
  description: "Base64, URL encode/decode, or hash a string",
  category: "builtin",
  icon: "shield",
  color: "#14B8A6",
  imageUrl: "/icons/shield.svg",
  inputs: [
    {
      key: "input",
      label: "Input",
      type: "string",
      required: true,
      description: "String to process",
    },
    {
      key: "operation",
      label: "Operation",
      type: "string",
      required: true,
      enum: [
        "base64-encode",
        "base64-decode",
        "url-encode",
        "url-decode",
        "sha256",
        "md5",
      ],
      description: "Which operation to perform",
    },
  ],
  outputs: [
    { key: "result", label: "Result", type: "string" },
    { key: "operation", label: "Operation", type: "string" },
  ],
};

export const filterNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:filter",
  name: "Filter",
  description: "Filter an array using an expression",
  category: "builtin",
  icon: "filter",
  color: "#EC4899",
  imageUrl: "/icons/filter.svg",
  inputs: [
    {
      key: "items",
      label: "Items",
      type: "array",
      required: true,
      description: "The array to filter",
    },
    {
      key: "expression",
      label: "Condition",
      type: "string",
      required: true,
      description: 'CEL expression per item (e.g. item.status == "active")',
    },
  ],
  outputs: [
    { key: "result", label: "Filtered Items", type: "array" },
    { key: "count", label: "Match Count", type: "number" },
    { key: "total", label: "Original Count", type: "number" },
  ],
};

// --- Trigger Nodes ---

export const manualTriggerSchema: NodeTypeDefinition = {
  pluginRef: "builtin:trigger-manual",
  name: "Manual Trigger",
  description: "Start this workflow manually or via API",
  category: "builtin",
  nodeKind: "trigger",
  icon: "play",
  color: "#10B981",
  imageUrl: "/icons/play.svg",
  inputs: [],
  outputs: [
    {
      key: "payload",
      label: "Trigger Payload",
      type: "object",
      description: "Data passed when starting the workflow",
    },
  ],
};

export const cronTriggerSchema: NodeTypeDefinition = {
  pluginRef: "builtin:trigger-cron",
  name: "Schedule",
  description: "Run on a time-based schedule",
  category: "builtin",
  nodeKind: "trigger",
  icon: "clock",
  color: "#F59E0B",
  imageUrl: "/icons/clock.svg",
  inputs: [],
  outputs: [
    { key: "scheduled_time", label: "Scheduled Time", type: "string" },
    { key: "cron_expression", label: "Cron Expression", type: "string" },
  ],
  parameters: [
    {
      key: "cron",
      label: "Cron Expression",
      type: "string",
      required: true,
      description: "Cron schedule (e.g. '0 */5 * * *')",
    },
  ],
};

export const webhookTriggerSchema: NodeTypeDefinition = {
  pluginRef: "builtin:trigger-webhook",
  name: "Webhook",
  description: "Triggered by an HTTP request",
  category: "builtin",
  nodeKind: "trigger",
  icon: "webhook",
  color: "#8B5CF6",
  imageUrl: "/icons/webhook.svg",
  inputs: [],
  outputs: [
    { key: "method", label: "HTTP Method", type: "string" },
    { key: "headers", label: "Headers", type: "object" },
    { key: "body", label: "Body", type: "object" },
    { key: "query", label: "Query Params", type: "object" },
  ],
  parameters: [
    {
      key: "path",
      label: "Webhook Path",
      type: "string",
      required: true,
      description: "URL path for the webhook endpoint",
    },
  ],
};

export const eventTriggerSchema: NodeTypeDefinition = {
  pluginRef: "builtin:trigger-event",
  name: "Event",
  description: "Triggered by a named event",
  category: "builtin",
  nodeKind: "trigger",
  icon: "zap",
  color: "#EF4444",
  imageUrl: "/icons/zap.svg",
  inputs: [],
  outputs: [
    { key: "event_name", label: "Event Name", type: "string" },
    { key: "payload", label: "Event Payload", type: "object" },
  ],
  parameters: [
    {
      key: "event_name",
      label: "Event Name",
      type: "string",
      required: true,
      description: "Name of the event to subscribe to",
    },
  ],
};

// --- Capability Nodes ---

export const postgresCapabilitySchema: NodeTypeDefinition = {
  pluginRef: "builtin:capability-postgres",
  name: "PostgreSQL",
  description: "PostgreSQL database connection",
  category: "builtin",
  nodeKind: "capability",
  icon: "database",
  color: "#336791",
  imageUrl: "/icons/database.svg",
  providesCapability: "database",
  inputs: [],
  outputs: [],
  parameters: [
    {
      key: "credential_id",
      label: "PostgreSQL Credential",
      type: "credential",
      credentialType: "postgres",
      description: "Select a PostgreSQL credential for connection settings",
    },
    {
      key: "host",
      label: "Host",
      type: "string",
      default: "localhost",
      description: "Database server hostname (overrides credential)",
    },
    { key: "port", label: "Port", type: "number", default: 5432, description: "Database server port (overrides credential)" },
    { key: "database", label: "Database", type: "string", description: "Database name (overrides credential)" },
    { key: "user", label: "User", type: "string", description: "Database user (overrides credential)" },
    { key: "password", label: "Password", type: "string", description: "Database password (overrides credential)" },
    {
      key: "sslmode",
      label: "SSL Mode",
      type: "string",
      default: "disable",
      enum: ["disable", "require", "verify-ca", "verify-full"],
      description: "SSL mode (overrides credential)",
    },
    {
      key: "pool_size",
      label: "Pool Size",
      type: "number",
      default: 10,
      description: "Connection pool size (overrides credential)",
    },
  ],
};

// --- Data Processing Nodes ---

export const sortNodeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:sort",
  name: "Sort",
  description: "Sort an array by a field",
  category: "builtin",
  icon: "arrow-up-down",
  color: "#EC4899",
  imageUrl: "/icons/arrow-up-down.svg",
  inputs: [
    {
      key: "items",
      label: "Items",
      type: "array",
      required: true,
      description: "The array to sort",
    },
  ],
  outputs: [{ key: "items", label: "Sorted Items", type: "array" }],
  parameters: [
    {
      key: "key",
      label: "Sort Key",
      type: "string",
      required: true,
      description: "Field name to sort by",
    },
    {
      key: "direction",
      label: "Direction",
      type: "string",
      default: "asc",
      enum: ["asc", "desc"],
      description: "Sort direction",
    },
  ],
};

// ── AI Nodes ─────────────────────────────────────────────────────────────────

const AI_CREDENTIAL_TYPE = "openai,anthropic,google_ai";

const aiProviderParam = {
  key: "provider",
  label: "Provider",
  type: "string" as const,
  required: true,
  default: "openai",
  description: "AI provider to use",
  enum: ["openai", "anthropic", "google_ai"],
};

const aiModelParam = {
  key: "model",
  label: "Model",
  type: "string" as const,
  required: true,
  default: "gpt-4o-mini",
  description: "Model name (e.g. gpt-4o-mini, claude-sonnet-4-20250514, gemini-2.0-flash)",
};

const aiCredentialParam = {
  key: "credential_id",
  label: "AI Credential",
  type: "credential" as const,
  required: true,
  credentialType: AI_CREDENTIAL_TYPE,
  description: "API key credential for the selected provider",
};

const aiUsageOutput = {
  key: "usage",
  label: "Token Usage",
  type: "object" as const,
  description: "Token counts: prompt_tokens, completion_tokens, total_tokens",
};

const aiCostOutput = {
  key: "cost_usd",
  label: "Estimated Cost",
  type: "number" as const,
  description: "Approximate cost in USD",
};

export const aiChatSchema: NodeTypeDefinition = {
  pluginRef: "builtin:ai-chat",
  name: "AI Chat",
  description: "Generate text using an LLM (OpenAI, Anthropic, Google AI)",
  category: "builtin",
  icon: "message-circle",
  color: "#A855F7",
  inputs: [
    { key: "prompt", label: "Prompt", type: "string", required: true, description: "The user message or prompt" },
    { key: "system_prompt", label: "System Prompt", type: "string", description: "Instructions that define the AI's behavior" },
    { key: "context", label: "Context Data", type: "object", description: "Additional data appended as JSON context" },
  ],
  outputs: [
    { key: "content", label: "Response", type: "string", description: "AI-generated text" },
    { key: "parsed_json", label: "Parsed JSON", type: "object", dynamic: true, description: "Parsed object when response_format is json" },
    { key: "model", label: "Model Used", type: "string" },
    aiUsageOutput,
    aiCostOutput,
    { key: "finish_reason", label: "Finish Reason", type: "string" },
  ],
  parameters: [
    aiCredentialParam,
    aiProviderParam,
    aiModelParam,
    { key: "temperature", label: "Temperature", type: "number", default: 0.7, description: "Randomness (0=deterministic, 2=very random)" },
    { key: "max_tokens", label: "Max Tokens", type: "number", default: 1024, description: "Maximum response length in tokens" },
    { key: "response_format", label: "Response Format", type: "string", default: "text", enum: ["text", "json"], description: "Output format (json forces valid JSON)" },
  ],
};

export const aiExtractSchema: NodeTypeDefinition = {
  pluginRef: "builtin:ai-extract",
  name: "AI Extract",
  description: "Extract structured data from text using AI",
  category: "builtin",
  icon: "sparkles",
  color: "#C084FC",
  inputs: [
    { key: "text", label: "Input Text", type: "string", required: true, description: "Text to extract data from" },
    { key: "schema", label: "Output Schema", type: "string", required: true, description: "JSON schema defining fields to extract (e.g. {\"name\": \"string\", \"email\": \"string\"})" },
    { key: "instructions", label: "Instructions", type: "string", description: "Additional guidance for the extraction" },
  ],
  outputs: [
    { key: "extracted", label: "Extracted Data", type: "object", dynamic: true, description: "Structured data matching the schema" },
    { key: "raw_response", label: "Raw Response", type: "string", description: "Raw AI response text" },
    aiUsageOutput,
    aiCostOutput,
  ],
  parameters: [
    aiCredentialParam,
    aiProviderParam,
    aiModelParam,
  ],
};

export const aiClassifySchema: NodeTypeDefinition = {
  pluginRef: "builtin:ai-classify",
  name: "AI Classify",
  description: "Classify text into categories using AI",
  category: "builtin",
  icon: "brain",
  color: "#9333EA",
  inputs: [
    { key: "text", label: "Text", type: "string", required: true, description: "Text to classify" },
    { key: "categories", label: "Categories", type: "string", required: true, description: "Comma-separated categories (e.g. \"spam, support, sales, other\")" },
    { key: "instructions", label: "Instructions", type: "string", description: "Additional classification guidance" },
  ],
  outputs: [
    { key: "category", label: "Category", type: "string", description: "Best matching category" },
    { key: "categories", label: "All Categories", type: "array", description: "All matched categories with confidence scores" },
    { key: "confidence", label: "Confidence", type: "number", description: "Confidence score (0-1)" },
    { key: "reasoning", label: "Reasoning", type: "string", description: "Explanation of the classification" },
    aiUsageOutput,
    aiCostOutput,
  ],
  parameters: [
    aiCredentialParam,
    aiProviderParam,
    aiModelParam,
    { key: "multi_label", label: "Allow Multiple Labels", type: "boolean", default: false, description: "Allow classifying into multiple categories" },
  ],
};

export const aiSummarizeSchema: NodeTypeDefinition = {
  pluginRef: "builtin:ai-summarize",
  name: "AI Summarize",
  description: "Summarize text using AI",
  category: "builtin",
  icon: "file-text",
  color: "#A855F7",
  inputs: [
    { key: "text", label: "Text", type: "string", required: true, description: "Text to summarize" },
  ],
  outputs: [
    { key: "summary", label: "Summary", type: "string" },
    { key: "key_points", label: "Key Points", type: "array", description: "Main takeaways as a list" },
    aiUsageOutput,
    aiCostOutput,
  ],
  parameters: [
    aiCredentialParam,
    aiProviderParam,
    aiModelParam,
    { key: "style", label: "Summary Style", type: "string", default: "brief", enum: ["brief", "detailed", "bullet_points", "key_takeaways"] },
    { key: "max_length", label: "Max Length", type: "string", default: "1 paragraph", enum: ["1-2 sentences", "1 paragraph", "3 paragraphs", "unlimited"] },
  ],
};

export const aiSentimentSchema: NodeTypeDefinition = {
  pluginRef: "builtin:ai-sentiment",
  name: "AI Sentiment",
  description: "Analyze sentiment and emotion in text",
  category: "builtin",
  icon: "brain",
  color: "#C084FC",
  inputs: [
    { key: "text", label: "Text", type: "string", required: true, description: "Text to analyze" },
  ],
  outputs: [
    { key: "sentiment", label: "Sentiment", type: "string", description: "positive, negative, neutral, or mixed" },
    { key: "score", label: "Score", type: "number", description: "Sentiment score (-1.0 to 1.0)" },
    { key: "emotions", label: "Emotions", type: "object", dynamic: true, description: "Detected emotions with scores" },
    { key: "reasoning", label: "Reasoning", type: "string" },
    aiUsageOutput,
    aiCostOutput,
  ],
  parameters: [
    aiCredentialParam,
    aiProviderParam,
    aiModelParam,
  ],
};

export const aiTranslateSchema: NodeTypeDefinition = {
  pluginRef: "builtin:ai-translate",
  name: "AI Translate",
  description: "Translate text between languages using AI",
  category: "builtin",
  icon: "globe",
  color: "#9333EA",
  inputs: [
    { key: "text", label: "Text", type: "string", required: true, description: "Text to translate" },
  ],
  outputs: [
    { key: "translated", label: "Translated Text", type: "string" },
    { key: "detected_language", label: "Detected Source Language", type: "string" },
    aiUsageOutput,
    aiCostOutput,
  ],
  parameters: [
    aiCredentialParam,
    aiProviderParam,
    aiModelParam,
    { key: "target_language", label: "Target Language", type: "string", required: true, default: "English" },
    { key: "source_language", label: "Source Language", type: "string", default: "auto", description: "Auto-detect if empty" },
    { key: "tone", label: "Tone", type: "string", default: "formal", enum: ["formal", "informal", "technical", "casual"] },
  ],
};

export const builtinSchemas: NodeTypeDefinition[] = [
  // Triggers
  manualTriggerSchema,
  cronTriggerSchema,
  webhookTriggerSchema,
  eventTriggerSchema,
  // Actions
  httpNodeSchema,
  delayNodeSchema,
  logNodeSchema,
  subWorkflowNodeSchema,
  transformNodeSchema,
  emailNodeSchema,
  templateNodeSchema,
  encodeNodeSchema,
  filterNodeSchema,
  sortNodeSchema,
  // AI
  aiChatSchema,
  aiExtractSchema,
  aiClassifySchema,
  aiSummarizeSchema,
  aiSentimentSchema,
  aiTranslateSchema,
  // Capabilities
  postgresCapabilitySchema,
];
