import type {
  FieldMapping,
  ConditionRule,
  ConditionGroup,
  CelOperator,
} from "../types/schema";
import { isConditionGroup } from "../types/schema";

const CEL_IDENTIFIER_RE = /^[A-Za-z_][A-Za-z0-9_]*$/;

/** Escape a JavaScript string as a CEL double-quoted string literal. */
export function escapeCelStringLiteral(value: string): string {
  return JSON.stringify(value);
}

/** Append a field accessor, using bracket notation for non-identifier keys. */
export function appendCelFieldAccess(prefix: string, key: string): string {
  return CEL_IDENTIFIER_RE.test(key)
    ? `${prefix}.${key}`
    : `${prefix}[${escapeCelStringLiteral(key)}]`;
}

/** Build a CEL path from already-tokenized path segments. */
export function buildCelPath(root: string, keys: readonly string[]): string {
  return keys.reduce((path, key) => appendCelFieldAccess(path, key), root);
}

/** Build a CEL path rooted at nodes["..."] from path segments. */
export function buildNodeCelPathFromSegments(nodeId: string, segments: readonly string[]): string {
  return buildCelPath(`nodes[${escapeCelStringLiteral(nodeId)}]`, segments);
}

/** Build a CEL path rooted at nodes["..."], escaping node IDs and field keys. */
export function buildNodeCelPath(nodeId: string, sourcePath?: string): string {
  const root = `nodes[${escapeCelStringLiteral(nodeId)}]`;
  if (!sourcePath) return root;

  return buildCelPath(
    root,
    sourcePath
    .split(".")
    .filter(Boolean)
  );
}

function formatCelLiteral(value: string | number | boolean): string {
  return typeof value === "string" ? escapeCelStringLiteral(value) : String(value);
}

/**
 * Build a value string for a node's input_mapping field.
 * Values starting with "=" are treated as CEL by the engine (orbflow-engine).
 * Plain strings are treated as literals or variable references.
 */
export function buildMappingExpression(mapping: FieldMapping): string {
  if (mapping.mode === "static") {
    if (mapping.staticValue === undefined || mapping.staticValue === null) {
      return "";
    }
    return typeof mapping.staticValue === "string"
      ? mapping.staticValue
      : JSON.stringify(mapping.staticValue);
  }

  if (mapping.celExpression) {
    return mapping.celExpression.startsWith("=")
      ? mapping.celExpression
      : `=${mapping.celExpression}`;
  }

  // Expression mode: reference upstream node output via CEL
  if (mapping.sourceNodeId && mapping.sourcePath) {
    return `=${buildNodeCelPath(mapping.sourceNodeId, mapping.sourcePath)}`;
  }

  return "";
}

/**
 * Serialize a full set of FieldMappings into the input_mapping
 * format expected by core.Node (map[string]any).
 */
export function serializeMappings(
  mappings: Record<string, FieldMapping>
): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, mapping] of Object.entries(mappings)) {
    const val = buildMappingExpression(mapping);
    if (val !== "") result[key] = val;
  }
  return result;
}

/**
 * Build a CEL boolean expression from a visual condition tree.
 * Used for edge conditions, evaluated by orbflow-cel's eval_bool.
 */
export function buildConditionExpression(
  condition: ConditionRule | ConditionGroup
): string {
  if (isConditionGroup(condition)) {
    return buildGroupExpression(condition);
  }
  return buildRuleExpression(condition);
}

function buildGroupExpression(group: ConditionGroup): string {
  if (group.rules.length === 0) return "true";
  if (group.rules.length === 1) return buildConditionExpression(group.rules[0]);

  const joiner = group.logic === "and" ? " && " : " || ";
  const parts = group.rules.map((r) => {
    const expr = buildConditionExpression(r);
    return isConditionGroup(r) ? `(${expr})` : expr;
  });
  return parts.join(joiner);
}

function buildRuleExpression(rule: ConditionRule): string {
  const { field, operator, value } = rule;
  const formattedValue = formatCelLiteral(value);

  const ops: Record<CelOperator, string> = {
    "==": `${field} == ${formattedValue}`,
    "!=": `${field} != ${formattedValue}`,
    ">": `${field} > ${formattedValue}`,
    "<": `${field} < ${formattedValue}`,
    ">=": `${field} >= ${formattedValue}`,
    "<=": `${field} <= ${formattedValue}`,
    contains: `${field}.contains(${formattedValue})`,
    startsWith: `${field}.startsWith(${formattedValue})`,
    endsWith: `${field}.endsWith(${formattedValue})`,
  };

  return ops[operator] || `${field} == ${formattedValue}`;
}
