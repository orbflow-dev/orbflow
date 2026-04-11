"use client";
import { useMemo } from "react";
import { useCanvasStore } from "../stores/canvas-store";
import { usePanelStore } from "../stores/panel-store";
import { resolveUpstreamOutputs, flattenUpstreamPaths } from "../utils/upstream";
import type { NodeSchemaRegistry } from "../schemas/registry";

export function useNodeConfig(nodeId: string | null, registry: NodeSchemaRegistry) {
  const { nodes, edges } = useCanvasStore();
  const panel = usePanelStore();

  const node = useMemo(() => nodes.find((n) => n.id === nodeId), [nodes, nodeId]);
  const pluginRef = (node?.data as Record<string, unknown>)?.pluginRef as string | undefined;
  const schema = pluginRef ? registry.get(pluginRef) : undefined;

  const upstreamOutputs = useMemo(() => {
    if (!nodeId) return [];
    return resolveUpstreamOutputs(nodeId, nodes, edges, registry);
  }, [nodeId, nodes, edges, registry]);

  const upstreamPaths = useMemo(
    () => flattenUpstreamPaths(upstreamOutputs),
    [upstreamOutputs]
  );

  return { node, schema, upstreamOutputs, upstreamPaths, panel };
}
