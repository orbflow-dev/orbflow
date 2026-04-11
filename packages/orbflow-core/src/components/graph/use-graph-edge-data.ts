/**
 * Hook that computes all data needed to render a graph edge.
 *
 * Calculates the bezier path, extracts condition label (with truncation),
 * resolves execution status, and provides derived boolean flags.
 */

import { useMemo } from "react";
import { getBezierPath, type Position } from "@xyflow/react";
import { deriveEdgeStatus, type EdgeExecutionStatus } from "./derive-edge-status";
import type { ExecutionStatus } from "../../execution/execution-status";

/* ── Output interface ─────────────────────────────────── */

export interface GraphEdgeComputedData {
  /** Edge ID */
  edgeId: string;
  /** SVG path string for the bezier curve */
  path: string;
  /** X coordinate of the label position (midpoint) */
  labelX: number;
  /** Y coordinate of the label position (midpoint) */
  labelY: number;
  /** Full condition label from edge data */
  conditionLabel: string;
  /** Condition label truncated to 30 chars with ellipsis */
  truncatedLabel: string;
  /** Computed execution status of this edge */
  executionStatus: EdgeExecutionStatus;
  /** True when execution status is "active" */
  isActive: boolean;
  /** True when execution status is "completed" */
  isCompleted: boolean;
  /** True when execution status is "failed" */
  isFailed: boolean;
}

/* ── Options interface ────────────────────────────────── */

export interface UseGraphEdgeDataOptions {
  id: string;
  sourceX: number;
  sourceY: number;
  targetX: number;
  targetY: number;
  sourcePosition: Position;
  targetPosition: Position;
  data?: Record<string, unknown>;
}

/* ── Hook ─────────────────────────────────────────────── */

export function useGraphEdgeData(props: UseGraphEdgeDataOptions): GraphEdgeComputedData {
  const {
    id,
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    data,
  } = props;

  const [path, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  const conditionLabel = (data?.conditionLabel as string) || "";

  const truncatedLabel = useMemo(
    () =>
      conditionLabel.length > 30
        ? conditionLabel.slice(0, 30) + "\u2026"
        : conditionLabel,
    [conditionLabel],
  );

  // Derive execution status from source/target node statuses stored in data
  const executionStatus = useMemo((): EdgeExecutionStatus => {
    const explicit = data?.executionStatus as EdgeExecutionStatus | undefined;
    if (explicit) return explicit;

    const sourceStatus = data?.sourceExecStatus as ExecutionStatus | undefined;
    const targetStatus = data?.targetExecStatus as ExecutionStatus | undefined;
    return deriveEdgeStatus(sourceStatus, targetStatus);
  }, [data?.executionStatus, data?.sourceExecStatus, data?.targetExecStatus]);

  const isActive = executionStatus === "active";
  const isCompleted = executionStatus === "completed";
  const isFailed = executionStatus === "failed";

  return {
    edgeId: id,
    path,
    labelX,
    labelY,
    conditionLabel,
    truncatedLabel,
    executionStatus,
    isActive,
    isCompleted,
    isFailed,
  };
}
