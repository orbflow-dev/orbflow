/**
 * Headless graph node — render-prop component that composes
 * useGraphNodeData + NodeHandles and passes all computed data
 * (including pre-rendered handles) to the children render prop.
 *
 * ZERO CSS — consumer provides all visual rendering.
 */

import {
  useGraphNodeData,
  type GraphNodeComputedData,
  type UseGraphNodeDataOptions,
} from "./use-graph-node-data";
import { NodeHandles } from "./node-handles";
import type { NodeKind } from "../../types/schema";
import type { ExecutionStatus } from "../../execution/execution-status";

/* ── Render data ──────────────────────────────────────── */

export interface GraphNodeRenderData extends GraphNodeComputedData {
  /** Pre-rendered ReactFlow Handle elements based on node kind */
  handles: React.ReactNode;
}

/* ── Props ────────────────────────────────────────────── */

export interface GraphNodeProps {
  /** ReactFlow node ID */
  id: string;
  /** ReactFlow node data bag */
  data: Record<string, unknown>;
  /** Whether the node is currently selected */
  selected?: boolean;
  /** Whether the node is in read-only mode */
  readOnly?: boolean;
  /** Render prop receiving all computed node data + handles */
  children: (data: GraphNodeRenderData) => React.ReactNode;
}

/* ── Component ────────────────────────────────────────── */

export const GraphNode: React.FC<GraphNodeProps> = ({
  id,
  data,
  selected = false,
  readOnly = false,
  children,
}) => {
  const pluginRef = (data.pluginRef as string) || "";
  const label = (data.label as string) || "";
  const executionStatus = data.executionStatus as ExecutionStatus | undefined;
  const error = data.error as string | undefined;
  const duration = data.duration as number | undefined;
  const kindOverride = data.kind as NodeKind | undefined;

  const hookOptions: UseGraphNodeDataOptions = {
    nodeId: id,
    pluginRef,
    label,
    selected,
    readOnly,
    executionStatus,
    error,
    duration,
    kind: kindOverride,
  };

  const computed = useGraphNodeData(hookOptions);

  const handles = (
    <NodeHandles
      kind={computed.kind}
      capabilityPorts={computed.capabilityPorts}
      isConnectable={computed.isConnectable}
    />
  );

  const renderData: GraphNodeRenderData = {
    ...computed,
    handles,
  };

  return <>{children(renderData)}</>;
};

GraphNode.displayName = "GraphNode";
