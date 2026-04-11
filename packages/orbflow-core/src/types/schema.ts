// Core schema types for the visual workflow builder.
// These types drive the node catalog, config panel, data mapper, and condition builder.

export type FieldType = "string" | "number" | "boolean" | "object" | "array" | "credential";

export type NodeKind = "trigger" | "action" | "capability";
export type ParameterMode = "static" | "expression";

export interface FieldSchema {
  key: string;
  label: string;
  type: FieldType;
  required?: boolean;
  default?: unknown;
  description?: string;
  children?: FieldSchema[];
  enum?: string[];
  /** For type "credential": which credential type to filter by (e.g. "smtp", "postgres") */
  credentialType?: string;
  /** True if this field's actual structure is only known after execution.
   *  The static schema defines the expected type, but the runtime value
   *  may have a richer structure (e.g., HTTP body could be JSON, text, or binary). */
  dynamic?: boolean;
  /** True if this field contains binary data (detected from runtime `_binary` marker). */
  isBinary?: boolean;
}

export interface CapabilityPortDef {
  key: string;
  capabilityType: string;
  required?: boolean;
  description?: string;
}

export interface NodeTypeDefinition {
  pluginRef: string;
  name: string;
  description: string;
  category: "builtin" | "plugin" | "custom";
  nodeKind?: NodeKind;
  icon: string;
  color: string;
  docs?: string;
  imageUrl?: string;
  inputs: FieldSchema[];
  outputs: FieldSchema[];
  configFields?: FieldSchema[];
  parameters?: FieldSchema[];
  capabilityPorts?: CapabilityPortDef[];
  settings?: FieldSchema[];
  providesCapability?: string;
}

export interface ParameterValue {
  key: string;
  mode: ParameterMode;
  value?: unknown;
  expression?: string;
}

export interface CapabilityEdge {
  id: string;
  sourceNodeId: string;
  targetNodeId: string;
  targetPortKey: string;
}

export interface Annotation {
  id: string;
  type: "sticky_note" | "text" | "markdown";
  content: string;
  position: { x: number; y: number };
  style?: Record<string, unknown>;
}

export interface TriggerNodeConfig {
  triggerType: string;
  cron?: string;
  eventName?: string;
  path?: string;
}

export interface NodeMetadata {
  description?: string;
  docs?: string;
  imageUrl?: string;
}

// Data mapping: how a node input gets its value.
export interface FieldMapping {
  targetKey: string;
  mode: "static" | "expression";
  staticValue?: unknown;
  sourceNodeId?: string;
  sourcePath?: string;
  celExpression?: string;
}

// Condition types for edge conditions.
export type CelOperator =
  | "=="
  | "!="
  | ">"
  | "<"
  | ">="
  | "<="
  | "contains"
  | "startsWith"
  | "endsWith";

export interface ConditionRule {
  id: string;
  field: string;
  operator: CelOperator;
  value: string | number | boolean;
}

export interface ConditionGroup {
  id: string;
  logic: "and" | "or";
  rules: (ConditionRule | ConditionGroup)[];
}

export function isConditionGroup(
  rule: ConditionRule | ConditionGroup
): rule is ConditionGroup {
  return "logic" in rule;
}
