import type { FieldSchema, FieldType } from "../types/schema";
import type { NodeSchemaRegistry } from "../schemas/registry";
import type { Node, Edge } from "@xyflow/react";

export interface UpstreamOutput {
  nodeId: string;
  nodeName: string;
  pluginRef: string;
  fields: FieldSchema[];
}

/** Maximum keys per level to prevent DoS from objects with thousands of keys. */
const MAX_KEYS_PER_LEVEL = 100;

/**
 * Infer FieldSchema[] from runtime output data.
 * Walks the object tree and creates field entries for each key,
 * recursing into nested objects. Max depth prevents infinite loops.
 */
export function inferFieldsFromData(
  data: Record<string, unknown>,
  maxDepth = 4,
  depth = 0,
): FieldSchema[] {
  if (depth >= maxDepth) return [];
  if (!data || typeof data !== "object" || Array.isArray(data)) return [];

  const fields: FieldSchema[] = [];

  try {
    const entries = Object.entries(data);
    const limit = Math.min(entries.length, MAX_KEYS_PER_LEVEL);

    for (let i = 0; i < limit; i++) {
      const [key, value] = entries[i];
      const fieldType = inferFieldType(value);
      const field: FieldSchema = {
        key,
        label: key,
        type: fieldType,
        dynamic: true,
      };

      // Recurse into plain objects to expose nested fields
      if (
        fieldType === "object" &&
        value !== null &&
        typeof value === "object" &&
        !Array.isArray(value)
      ) {
        field.children = inferFieldsFromData(
          value as Record<string, unknown>,
          maxDepth,
          depth + 1,
        );
      }

      fields.push(field);
    }
  } catch {
    // Malformed data — return what we have
    return fields;
  }

  return fields;
}

export function inferFieldType(value: unknown): FieldType {
  if (value === null || value === undefined) return "string";
  if (typeof value === "number") return "number";
  if (typeof value === "boolean") return "boolean";
  if (Array.isArray(value)) return "array";
  if (typeof value === "object") return "object";
  return "string";
}

/**
 * Merge static schema fields with runtime output data.
 * For fields marked dynamic, if runtime data has a richer structure,
 * the runtime fields replace the schema children.
 * Any runtime keys not in the schema are added as new dynamic fields.
 */
export function mergeSchemaWithRuntime(
  schemaFields: FieldSchema[],
  runtimeData: Record<string, unknown>,
): FieldSchema[] {
  const schemaKeys = new Set(schemaFields.map((f) => f.key));
  const merged: FieldSchema[] = [];

  for (const field of schemaFields) {
    const runtimeValue = runtimeData[field.key];

    // Detect backend binary marker: { _binary: true, content_type, size_bytes }
    if (
      runtimeValue !== null &&
      runtimeValue !== undefined &&
      typeof runtimeValue === "object" &&
      !Array.isArray(runtimeValue) &&
      (runtimeValue as Record<string, unknown>)["_binary"] === true
    ) {
      merged.push({
        ...field,
        isBinary: true,
        children: [
          { key: "content_type", label: "Content Type", type: "string", dynamic: true },
          { key: "size_bytes", label: "Size (bytes)", type: "number", dynamic: true },
        ],
      });
      continue;
    }

    if (runtimeValue !== null && runtimeValue !== undefined) {
      const actualType = inferFieldType(runtimeValue);
      const typeChanged = actualType !== field.type;

      // Expand when the field is marked dynamic OR when runtime type differs
      // from schema type (e.g., schema says "string" but runtime is an object).
      if (field.dynamic || typeChanged) {
        if (
          typeof runtimeValue === "object" &&
          !Array.isArray(runtimeValue)
        ) {
          // Object: expose its actual keys as children
          merged.push({
            ...field,
            type: "object",
            children: inferFieldsFromData(runtimeValue as Record<string, unknown>),
          });
        } else if (Array.isArray(runtimeValue)) {
          merged.push({ ...field, type: "array" });
        } else {
          merged.push({ ...field, type: actualType });
        }
      } else {
        merged.push(field);
      }
    } else {
      merged.push(field);
    }
  }

  // Add runtime-only keys not in the schema
  for (const [key, value] of Object.entries(runtimeData)) {
    if (schemaKeys.has(key)) continue;
    const fieldType = inferFieldType(value);
    const extra: FieldSchema = { key, label: key, type: fieldType, dynamic: true };
    if (
      fieldType === "object" &&
      value !== null &&
      typeof value === "object" &&
      !Array.isArray(value)
    ) {
      extra.children = inferFieldsFromData(value as Record<string, unknown>);
    }
    merged.push(extra);
  }

  return merged;
}

/**
 * Given a target node, returns the output schemas of ALL reachable ancestor
 * nodes via BFS traversal. Direct parents appear first, then grandparents, etc.
 *
 * If runtimeOutputs is provided, dynamic schema fields are enriched with
 * the actual output structure from execution, making individual JSON keys
 * available as draggable items in the field browser.
 */
export function resolveUpstreamOutputs(
  targetNodeId: string,
  nodes: Node[],
  edges: Edge[],
  registry: NodeSchemaRegistry,
  runtimeOutputs?: Record<string, Record<string, unknown>>,
): UpstreamOutput[] {
  // Build reverse adjacency: target -> [sources]
  const predecessors = new Map<string, string[]>();
  for (const edge of edges) {
    const existing = predecessors.get(edge.target) || [];
    existing.push(edge.source);
    predecessors.set(edge.target, existing);
  }

  // BFS: collect ALL reachable ancestors (direct parents first, then deeper)
  const visited = new Set<string>();
  const queue: string[] = [...(predecessors.get(targetNodeId) || [])];
  const ancestorIds: string[] = [];

  while (queue.length > 0) {
    const current = queue.shift()!;
    if (visited.has(current)) continue;
    visited.add(current);
    ancestorIds.push(current);
    const parents = predecessors.get(current) || [];
    for (const parentId of parents) {
      if (!visited.has(parentId)) {
        queue.push(parentId);
      }
    }
  }

  const upstream: UpstreamOutput[] = [];

  for (const ancestorId of ancestorIds) {
    const node = nodes.find((n) => n.id === ancestorId);
    if (!node) continue;

    const pluginRef = (node.data?.pluginRef as string) || "";
    const schema = registry.get(pluginRef);
    if (schema) {
      const label = (node.data?.label as string) || "";
      const nodeRuntime = runtimeOutputs?.[ancestorId];
      const fields = nodeRuntime
        ? mergeSchemaWithRuntime(schema.outputs, nodeRuntime)
        : schema.outputs;

      upstream.push({
        nodeId: ancestorId,
        nodeName: label && label !== schema.name ? label : ancestorId,
        pluginRef,
        fields,
      });
    }
  }

  return upstream;
}

/**
 * Build a flat list of selectable field paths for condition builder dropdowns.
 * Returns paths like: nodes["http-1"].status, nodes["http-1"].body, vars.someVar
 */
export function flattenUpstreamPaths(
  upstream: UpstreamOutput[]
): { label: string; celPath: string; type: FieldSchema["type"] }[] {
  const paths: { label: string; celPath: string; type: FieldSchema["type"] }[] =
    [];

  for (const node of upstream) {
    const flatten = (fields: FieldSchema[], prefix: string, labelPrefix: string) => {
      for (const f of fields) {
        const celPath = `${prefix}.${f.key}`;
        const label = `${labelPrefix}.${f.key}`;
        paths.push({ label, celPath, type: f.type });
        if (f.children && f.children.length > 0) {
          flatten(f.children, celPath, label);
        }
      }
    };
    flatten(
      node.fields,
      `nodes["${node.nodeId}"]`,
      node.nodeName
    );
  }

  return paths;
}
