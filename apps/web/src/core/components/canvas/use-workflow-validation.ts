import { useCallback } from "react";
import type { Node, Edge } from "@xyflow/react";
import { usePanelStore } from "@orbflow/core/stores";
import type { NodeSchemaRegistry } from "../../schemas/registry";

export function useWorkflowValidation(
  nodes: Node[],
  edges: Edge[],
  registry: NodeSchemaRegistry,
) {
  return useCallback((): string[] => {
    const errors: string[] = [];
    for (const node of nodes) {
      const pluginRef = (node.data?.pluginRef as string) || "";
      const schema = registry.get(pluginRef);
      if (!schema) continue;
      const nodeName = (node.data?.label as string) || node.id;

      for (const field of schema.inputs) {
        if (!field.required) continue;
        const mapping =
          usePanelStore.getState().getNodeMappings(node.id)[field.key];
        const hasStaticValue =
          mapping?.mode === "static" &&
          mapping.staticValue !== undefined &&
          mapping.staticValue !== "";
        const hasExpression =
          mapping?.mode === "expression" && !!mapping.celExpression;
        const isWired = edges.some(
          (e) =>
            e.target === node.id &&
            (e.data?.targetField as string) === field.key,
        );
        const hasDefault =
          field.default !== undefined && field.default !== "";

        if (!hasStaticValue && !hasExpression && !isWired && !hasDefault) {
          errors.push(
            `"${nodeName}" is missing required field "${field.label}"`,
          );
        }
      }
    }
    return errors;
  }, [nodes, edges, registry]);
}
