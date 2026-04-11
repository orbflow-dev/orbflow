"use client";

import type { ReactElement } from "react";
import {
  BaseEdge,
  getBezierPath,
  getSmoothStepPath,
  getStraightPath,
  Position,
  type Edge,
  type EdgeProps,
} from "@xyflow/react";

export type PathType = "auto" | "bezier" | "smoothstep" | "step" | "straight";

export type DataEdge = Edge<{
  path?: PathType;
  active?: boolean;
}>;

export function DataEdge({
  data = { path: "auto" },
  id,
  markerEnd,
  selected,
  sourcePosition,
  sourceX,
  sourceY,
  style,
  targetPosition,
  targetX,
  targetY,
}: EdgeProps<DataEdge>): ReactElement {
  const resolvedPathType = resolvePathType(data.path ?? "auto");
  const isActive = Boolean(data.active);
  const [edgePath] = getPath({
    type: resolvedPathType,
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  const edgeStyle = {
    stroke: isActive || selected
      ? "var(--orbflow-edge-selected, var(--electric-indigo))"
      : "var(--orbflow-edge-color)",
    strokeWidth: isActive || selected ? 2.6 : 2.1,
    opacity: isActive ? 1 : selected ? 0.96 : 0.7,
    strokeDasharray: isActive ? "8 6" : undefined,
    ...style,
  };

  return (
    <BaseEdge
      id={id}
      path={edgePath}
      markerEnd={markerEnd}
      style={edgeStyle}
    />
  );
}

function getPath({
  type,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
}: {
  type: "bezier" | "smoothstep" | "step" | "straight";
  sourceX: number;
  sourceY: number;
  targetX: number;
  targetY: number;
  sourcePosition: Position;
  targetPosition: Position;
}): [string, number, number, ...number[]] {
  if (type === "bezier") {
    return getBezierPath({
      sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition,
    });
  }
  if (type === "smoothstep") {
    return getSmoothStepPath({
      sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition,
    });
  }
  if (type === "step") {
    return getSmoothStepPath({
      sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition,
      borderRadius: 0,
    });
  }
  return getStraightPath({ sourceX, sourceY, targetX, targetY });
}

function resolvePathType(
  type: PathType,
): "bezier" | "smoothstep" | "step" | "straight" {
  if (type !== "auto") return type;
  return "smoothstep";
}
