import type { FieldSchema } from "../types/schema";
import type { UpstreamOutput } from "./upstream";

/** Result of a client-side CEL expression preview */
export interface CelPreviewResult {
  /** Preview status */
  status: "resolved" | "partial" | "unknown" | "error";
  /** Human-readable preview text */
  preview: string;
  /** Resolved type if known */
  type?: string;
}

/** Type-aware default values for preview */
function defaultForType(type: string): string {
  switch (type) {
    case "string": return '"..."';
    case "number": return "0";
    case "boolean": return "true";
    case "array": return "[...]";
    case "object": return "{...}";
    default: return "?";
  }
}

/**
 * Resolve a field path like `nodes["id"].body.title` against the upstream schema
 * to produce a type-aware preview.
 */
function resolveFieldPath(
  path: string,
  upstream: UpstreamOutput[],
): CelPreviewResult {
  // Match nodes["nodeId"].rest.of.path
  const nodeMatch = path.match(/^nodes\["([^"]+)"\]\.?(.*)$/);
  if (nodeMatch) {
    const nodeId = nodeMatch[1];
    const fieldPath = nodeMatch[2];
    const node = upstream.find((n) => n.nodeId === nodeId);
    if (!node) return { status: "unknown", preview: "unknown node" };
    if (!fieldPath) return { status: "resolved", preview: "{...}", type: "object" };

    const parts = fieldPath.split(".");
    let fields: FieldSchema[] = node.fields;
    let resolved: FieldSchema | undefined;

    for (const part of parts) {
      resolved = fields.find((f) => f.key === part);
      if (!resolved) return { status: "partial", preview: `${node.nodeName}.${fieldPath}`, type: "unknown" };
      fields = resolved.children ?? [];
    }

    // resolved is guaranteed defined here -- loop returns early on undefined
    const finalField = resolved!;
    return {
      status: "resolved",
      preview: defaultForType(finalField.type),
      type: finalField.type,
    };
  }

  // Context variables
  if (path === "vars" || path.startsWith("vars.")) {
    return { status: "resolved", preview: path === "vars" ? "{...}" : '"..."', type: path === "vars" ? "object" : "string" };
  }
  if (path === "trigger" || path.startsWith("trigger.")) {
    return { status: "resolved", preview: path === "trigger" ? "{...}" : '"..."', type: path === "trigger" ? "object" : "string" };
  }

  return { status: "unknown", preview: path };
}

/**
 * Generate a client-side preview for a CEL expression.
 * Resolves field paths against the upstream schema and infers result types
 * from known function signatures.
 */
export function previewCelExpression(
  expr: string,
  upstream: UpstreamOutput[],
): CelPreviewResult {
  const trimmed = expr.trim();
  if (!trimmed) return { status: "unknown", preview: "" };

  // Literals -- check before field paths
  if (/^"[^"]*"$/.test(trimmed) || /^'[^']*'$/.test(trimmed)) {
    return { status: "resolved", preview: trimmed, type: "string" };
  }
  if (/^\d+(\.\d+)?$/.test(trimmed)) {
    return { status: "resolved", preview: trimmed, type: "number" };
  }
  if (trimmed === "true" || trimmed === "false") {
    return { status: "resolved", preview: trimmed, type: "boolean" };
  }

  // Simple field path (no operators, no function calls)
  if (/^[\w\[\]"._]+$/.test(trimmed) && !trimmed.includes("(")) {
    return resolveFieldPath(trimmed, upstream);
  }

  // size() function -- returns number
  const sizeMatch = trimmed.match(/^size\((.+)\)$/);
  if (sizeMatch) {
    const inner = resolveFieldPath(sizeMatch[1].trim(), upstream);
    return { status: inner.status, preview: `number (length of ${inner.type ?? "value"})`, type: "number" };
  }

  // .contains(), .startsWith(), .endsWith(), .matches() -- return boolean
  const methodBoolMatch = trimmed.match(/^(.+)\.(contains|startsWith|endsWith|matches)\(.+\)$/);
  if (methodBoolMatch) {
    const subject = resolveFieldPath(methodBoolMatch[1].trim(), upstream);
    return { status: subject.status, preview: "true / false", type: "boolean" };
  }

  // int(), uint(), double() -- return number
  const castNumMatch = trimmed.match(/^(int|uint|double)\((.+)\)$/);
  if (castNumMatch) {
    return { status: "resolved", preview: "number", type: "number" };
  }

  // string() -- returns string
  const castStrMatch = trimmed.match(/^string\((.+)\)$/);
  if (castStrMatch) {
    return { status: "resolved", preview: '"..."', type: "string" };
  }

  // bool() -- returns boolean
  if (/^bool\(.+\)$/.test(trimmed)) {
    return { status: "resolved", preview: "true / false", type: "boolean" };
  }

  // has() -- returns boolean
  if (/^has\(.+\)$/.test(trimmed)) {
    return { status: "resolved", preview: "true / false", type: "boolean" };
  }

  // Ternary / conditional -- check before comparison (ternary contains > or < too)
  if (trimmed.includes("?") && trimmed.includes(":")) {
    return { status: "partial", preview: "conditional result", type: "unknown" };
  }

  // Comparison operators -- return boolean
  if (/[=!<>]=?/.test(trimmed) && (trimmed.includes("==") || trimmed.includes("!=") || trimmed.includes(">") || trimmed.includes("<"))) {
    return { status: "partial", preview: "true / false", type: "boolean" };
  }

  // Logical operators -- return boolean
  if (trimmed.includes("&&") || trimmed.includes("||")) {
    return { status: "partial", preview: "true / false", type: "boolean" };
  }

  // Can't determine
  return { status: "partial", preview: "expression result", type: "unknown" };
}
