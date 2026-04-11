/**
 * Orchestration hook that transforms a Workflow (+ optional Instance)
 * into ReactFlow-compatible nodes, edges, nodeTypes, and edgeTypes.
 *
 * Consumers pass their own graph node/edge components so this hook
 * remains headless and CSS-free.
 */

import { useMemo } from "react";
import type { Node, Edge } from "@xyflow/react";
import type { Workflow, Instance } from "../../types/api";
import { deriveEdgeStatus } from "./derive-edge-status";

/* ── Options ──────────────────────────────────────────── */

export interface UseWorkflowGraphOptions {
  /** Workflow definition containing nodes and edges */
  workflow: Workflow;
  /** Optional runtime instance with node execution states */
  instance?: Instance;
  /** When true, handles will not be connectable */
  readOnly?: boolean;
  /** Callback when a node is clicked (passed in node data) */
  onNodeClick?: (nodeId: string) => void;
  /** ReactFlow component for rendering graph nodes */
  graphNodeComponent: React.ComponentType;
  /** ReactFlow component for rendering graph edges */
  graphEdgeComponent: React.ComponentType;
}

/* ── Return type ──────────────────────────────────────── */

export interface UseWorkflowGraphResult {
  /** ReactFlow node array */
  nodes: Node[];
  /** ReactFlow edge array */
  edges: Edge[];
  /** Map of custom node type name to component */
  nodeTypes: Record<string, React.ComponentType>;
  /** Map of custom edge type name to component */
  edgeTypes: Record<string, React.ComponentType>;
}

/* ── Constants ────────────────────────────────────────── */

const GRAPH_NODE_TYPE = "graphNode";
const GRAPH_EDGE_TYPE = "graphEdge";

/* ── Hook ─────────────────────────────────────────────── */

export function useWorkflowGraph(opts: UseWorkflowGraphOptions): UseWorkflowGraphResult {
  const {
    workflow,
    instance,
    readOnly = false,
    onNodeClick,
    graphNodeComponent,
    graphEdgeComponent,
  } = opts;

  // Stable type maps — only recreated when the component references change
  const nodeTypes = useMemo(
    () => ({ [GRAPH_NODE_TYPE]: graphNodeComponent }),
    [graphNodeComponent],
  );

  const edgeTypes = useMemo(
    () => ({ [GRAPH_EDGE_TYPE]: graphEdgeComponent }),
    [graphEdgeComponent],
  );

  // Transform workflow nodes to ReactFlow nodes
  const nodes = useMemo((): Node[] => {
    return workflow.nodes.map((node) => {
      const nodeState = instance?.node_states?.[node.id];

      const data: Record<string, unknown> = {
        pluginRef: node.plugin_ref,
        label: node.name,
        kind: node.kind || undefined,
        readOnly,
        onNodeClick,
        // Execution state (undefined when no instance)
        executionStatus: nodeState?.status,
        error: nodeState?.error,
        duration: undefined,
        attempt: nodeState?.attempt,
        hasOutput: !!nodeState?.output,
      };

      return {
        id: node.id,
        type: GRAPH_NODE_TYPE,
        position: { x: node.position.x, y: node.position.y },
        data,
      };
    });
  }, [workflow, instance, readOnly, onNodeClick]);

  // Transform workflow edges to ReactFlow edges
  const edges = useMemo((): Edge[] => {
    return workflow.edges.map((edge) => {
      const sourceStatus = instance?.node_states?.[edge.source]?.status;
      const targetStatus = instance?.node_states?.[edge.target]?.status;

      const executionStatus = instance
        ? deriveEdgeStatus(sourceStatus, targetStatus)
        : "idle";

      const data: Record<string, unknown> = {
        conditionLabel: edge.condition || "",
        executionStatus,
        sourceExecStatus: sourceStatus,
        targetExecStatus: targetStatus,
      };

      return {
        id: edge.id,
        source: edge.source,
        target: edge.target,
        type: GRAPH_EDGE_TYPE,
        data,
      };
    });
  }, [workflow, instance]);

  return { nodes, edges, nodeTypes, edgeTypes };
}
