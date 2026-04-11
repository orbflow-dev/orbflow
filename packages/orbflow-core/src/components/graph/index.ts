/**
 * Headless graph primitives for rendering workflow graphs.
 *
 * All components use render-prop pattern for visual customization.
 * ZERO CSS — consumers apply their own styling.
 */

// ── Pure functions ──────────────────────────────────────
export {
  deriveEdgeStatus,
  type EdgeExecutionStatus,
} from "./derive-edge-status";

// ── Hooks ───────────────────────────────────────────────
export {
  useStatusBadge,
  type StatusBadgeData,
} from "./status-badge";

export {
  useGraphNodeData,
  type GraphNodeComputedData,
  type UseGraphNodeDataOptions,
} from "./use-graph-node-data";

export {
  useGraphEdgeData,
  type GraphEdgeComputedData,
  type UseGraphEdgeDataOptions,
} from "./use-graph-edge-data";

export {
  useWorkflowGraph,
  type UseWorkflowGraphOptions,
  type UseWorkflowGraphResult,
} from "./use-workflow-graph";

// ── Render-prop components ──────────────────────────────
export {
  StatusBadge,
  type StatusBadgeProps,
} from "./status-badge";

export {
  EdgeParticles,
  type EdgeParticlesProps,
} from "./edge-particles";

export {
  EdgeConditionLabel,
  type EdgeConditionLabelProps,
  type EdgeConditionLabelRenderData,
} from "./edge-condition-label";

export {
  NodeHandles,
  type GraphCapabilityPort,
  type NodeHandlesProps,
} from "./node-handles";

export {
  GraphNode,
  type GraphNodeProps,
  type GraphNodeRenderData,
} from "./graph-node";

export {
  GraphEdge,
  type GraphEdgeProps,
  type GraphEdgeRenderData,
} from "./graph-edge";
