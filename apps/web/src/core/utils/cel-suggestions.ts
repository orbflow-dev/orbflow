import type { FieldSchema } from "../types/schema";

/** Suggestion entry for the CEL expression autocomplete */
export interface Suggestion {
  /** Display label e.g. "body.title" */
  label: string;
  /** Full CEL path to insert e.g. nodes["http_request_1"].body.title */
  celPath: string;
  /** Type hint e.g. "string", "number (binary)" */
  detail: string;
  /** Node name for grouping */
  group: string;
  /** Suggestion kind for icon rendering */
  kind: "field" | "function" | "context" | "node";
  /** Argument hint shown after function is inserted, e.g. '"text"' or 'field' */
  argHint?: string;
}

/** CEL built-in functions available for autocomplete */
export const CEL_FUNCTIONS: Suggestion[] = [
  { label: "size(value)", celPath: "size(", detail: "length of string/list/map", group: "Functions", kind: "function", argHint: "field or value" },
  { label: 'field.contains("text")', celPath: ".contains(", detail: "pick a field first, then add .contains()", group: "Methods", kind: "function", argHint: '"quoted string"' },
  { label: 'field.startsWith("text")', celPath: ".startsWith(", detail: "pick a field first, then add .startsWith()", group: "Methods", kind: "function", argHint: '"quoted string"' },
  { label: 'field.endsWith("text")', celPath: ".endsWith(", detail: "pick a field first, then add .endsWith()", group: "Methods", kind: "function", argHint: '"quoted string"' },
  { label: 'field.matches("regex")', celPath: ".matches(", detail: "pick a field first, then add .matches()", group: "Methods", kind: "function", argHint: '"regex pattern"' },
  { label: "int(value)", celPath: "int(", detail: "convert to integer", group: "Functions", kind: "function", argHint: "field or value" },
  { label: "uint(value)", celPath: "uint(", detail: "convert to unsigned int", group: "Functions", kind: "function", argHint: "field or value" },
  { label: "double(value)", celPath: "double(", detail: "convert to double", group: "Functions", kind: "function", argHint: "field or value" },
  { label: "string(value)", celPath: "string(", detail: "convert to string", group: "Functions", kind: "function", argHint: "field or value" },
  { label: "bool(value)", celPath: "bool(", detail: "convert to boolean", group: "Functions", kind: "function", argHint: "field or value" },
  { label: "type(value)", celPath: "type(", detail: "get value type", group: "Functions", kind: "function", argHint: "field or value" },
  { label: "has(field)", celPath: "has(", detail: "check field existence", group: "Functions", kind: "function", argHint: "field path" },
  { label: 'duration("1h30m")', celPath: "duration(", detail: "parse duration string", group: "Functions", kind: "function", argHint: '"duration string"' },
  { label: 'timestamp("...")', celPath: "timestamp(", detail: "parse timestamp string", group: "Functions", kind: "function", argHint: '"ISO timestamp"' },
];

/** Recursively flatten fields into CEL path suggestions */
export function flattenFields(
  fields: FieldSchema[],
  nodeId: string,
  nodeName: string,
  celPrefix: string,
  labelPrefix: string,
  maxDepth = 3,
  depth = 0,
): Suggestion[] {
  if (depth >= maxDepth) return [];
  const results: Suggestion[] = [];

  for (const f of fields) {
    const celPath = `${celPrefix}.${f.key}`;
    const label = labelPrefix ? `${labelPrefix}.${f.key}` : f.key;
    const detail = f.isBinary ? `${f.type} (binary)` : f.type;

    results.push({ label, celPath, detail, group: nodeName, kind: "field" });

    if (f.children && f.children.length > 0) {
      results.push(
        ...flattenFields(f.children, nodeId, nodeName, celPath, label, maxDepth, depth + 1),
      );
    }
  }

  return results;
}
