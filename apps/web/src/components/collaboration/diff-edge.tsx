"use client";

import { memo } from "react";
import type { EdgeProps } from "@xyflow/react";
import { BaseEdge, getSmoothStepPath } from "@xyflow/react";

export type EdgeDiffStatus = "added" | "removed" | "unchanged";

export interface DiffEdgeData {
  diffStatus: EdgeDiffStatus;
  [key: string]: unknown;
}

export const DiffEdge = memo(function DiffEdge({
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  markerEnd,
}: EdgeProps & { data?: DiffEdgeData }) {
  const [edgePath] = getSmoothStepPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    borderRadius: 8,
  });

  const diffStatus = data?.diffStatus ?? "unchanged";

  const style: React.CSSProperties = {
    strokeWidth: diffStatus === "unchanged" ? 1.5 : 2,
    stroke:
      diffStatus === "added"
        ? "var(--color-success, #10b981)"
        : diffStatus === "removed"
          ? "var(--color-error, #ef4444)"
          : "var(--orbflow-edge-color, #6b7280)",
    ...(diffStatus === "removed" ? { strokeDasharray: "6,4" } : {}),
    ...(diffStatus === "removed" ? { opacity: 0.6 } : {}),
  };

  return <BaseEdge path={edgePath} style={style} markerEnd={markerEnd} />;
});
