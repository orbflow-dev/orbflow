/**
 * Headless graph edge — render-prop component that composes
 * useGraphEdgeData + a BaseEdge and passes all computed data
 * (including a pre-rendered base edge) to the children render prop.
 *
 * ZERO CSS — consumer provides all visual rendering.
 */

import { BaseEdge, type Position } from "@xyflow/react";
import {
  useGraphEdgeData,
  type GraphEdgeComputedData,
} from "./use-graph-edge-data";

/* ── Render data ──────────────────────────────────────── */

export interface GraphEdgeRenderData extends GraphEdgeComputedData {
  /** Pre-rendered BaseEdge element with the computed path */
  baseEdge: React.ReactNode;
}

/* ── Props ────────────────────────────────────────────── */

export interface GraphEdgeProps {
  id: string;
  sourceX: number;
  sourceY: number;
  targetX: number;
  targetY: number;
  sourcePosition: Position;
  targetPosition: Position;
  data?: Record<string, unknown>;
  selected?: boolean;
  /** Render prop receiving all computed edge data + base edge element */
  children: (data: GraphEdgeRenderData) => React.ReactNode;
}

/* ── Component ────────────────────────────────────────── */

export const GraphEdge: React.FC<GraphEdgeProps> = ({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  children,
}) => {
  const computed = useGraphEdgeData({
    id,
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    data,
  });

  const baseEdge = (
    <BaseEdge
      id={id}
      path={computed.path}
    />
  );

  const renderData: GraphEdgeRenderData = {
    ...computed,
    baseEdge,
  };

  return <>{children(renderData)}</>;
};

GraphEdge.displayName = "GraphEdge";
