"use client";
import { useCallback } from "react";
import { useCanvasStore } from "../stores/canvas-store";
import { generateNodeSlug } from "../utils/node-slug";
import { autoLayout } from "../utils/auto-layout";
import type { NodeSchemaRegistry } from "../schemas/registry";

export function useCanvas(registry?: NodeSchemaRegistry) {
  const nodes = useCanvasStore((s) => s.nodes);
  const edges = useCanvasStore((s) => s.edges);
  const addNode = useCanvasStore((s) => s.addNode);
  const setNodes = useCanvasStore((s) => s.setNodes);

  const addNodeFromSchema = useCallback(
    (pluginRef: string, position: { x: number; y: number }) => {
      if (!registry) return null;
      const schema = registry.get(pluginRef);
      if (!schema) return null;
      const slug = generateNodeSlug(schema.name, nodes.map((n) => n.id));
      const node = {
        id: slug,
        type: "task",
        position,
        data: { label: schema.name, pluginRef, type: schema.category },
      };
      addNode(node);
      return node;
    },
    [registry, nodes, addNode]
  );

  const layoutNodes = useCallback(() => {
    const laid = autoLayout(nodes, edges);
    setNodes(laid);
  }, [nodes, edges, setNodes]);

  return { nodes, edges, addNode, setNodes, addNodeFromSchema, layoutNodes };
}
