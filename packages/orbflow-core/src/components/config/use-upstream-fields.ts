"use client";

import { useMemo } from "react";
import { useCanvasStore } from "../../stores/canvas-store";
import { useOrbflow } from "../../context/orbflow-provider";
import {
  resolveUpstreamOutputs,
  flattenUpstreamPaths,
} from "../../utils/upstream";
import type { FieldSchema } from "../../types/schema";

export interface UpstreamNode {
  nodeId: string;
  label: string;
  pluginRef: string;
  fields: FieldSchema[];
}

export interface ContextVariable {
  label: string;
  description: string;
  prefix: string;
  celPath: string;
}

export interface UpstreamFieldsResult {
  upstreamNodes: UpstreamNode[];
  flatPaths: { label: string; celPath: string; type: string }[];
  contextVars: ContextVariable[];
}

const CONTEXT_VARIABLES: ContextVariable[] = [
  {
    label: "Workflow Variables",
    description: "Input variables",
    prefix: "vars",
    celPath: "vars",
  },
  {
    label: "Trigger Data",
    description: "Event payload",
    prefix: "trigger",
    celPath: "trigger",
  },
];

/**
 * Resolves all upstream node outputs and context variables for a given node.
 * Wraps resolveUpstreamOutputs + flattenUpstreamPaths + hardcoded context vars.
 */
export function useUpstreamFields(
  nodeId: string | null
): UpstreamFieldsResult {
  const nodes = useCanvasStore((s) => s.nodes);
  const edges = useCanvasStore((s) => s.edges);
  const { registry } = useOrbflow();

  const upstreamOutputs = useMemo(() => {
    if (!nodeId) return [];
    return resolveUpstreamOutputs(nodeId, nodes, edges, registry);
  }, [nodeId, nodes, edges, registry]);

  const upstreamNodes: UpstreamNode[] = useMemo(
    () =>
      upstreamOutputs.map((u) => ({
        nodeId: u.nodeId,
        label: u.nodeName,
        pluginRef: u.pluginRef,
        fields: u.fields,
      })),
    [upstreamOutputs]
  );

  const flatPaths = useMemo(
    () => flattenUpstreamPaths(upstreamOutputs),
    [upstreamOutputs]
  );

  return {
    upstreamNodes,
    flatPaths,
    contextVars: CONTEXT_VARIABLES,
  };
}
