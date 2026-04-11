import type {
  FieldMapping,
  ConditionRule,
  ConditionGroup,
  CelOperator,
} from "../types/schema";
import { isConditionGroup } from "../types/schema";

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

  // Expression mode: reference upstream node output via CEL
  if (mapping.sourceNodeId && mapping.sourcePath) {
    return `=nodes["${mapping.sourceNodeId}"].${mapping.sourcePath}`;
  }

  if (mapping.celExpression) {
    return mapping.celExpression.startsWith("=")
      ? mapping.celExpression
      : `=${mapping.celExpression}`;
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
  const formattedValue =
    typeof value === "string" ? `"${value}"` : String(value);

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
