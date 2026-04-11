"use client";

import { memo } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
  type EdgeProps,
} from "@xyflow/react";
import { cn } from "@/lib/cn";
import { STATUS_COLORS } from "@/lib/execution";

/* -- Types -------------------------------------------- */

export interface ExecutionEdgeData {
  executionStatus: "idle" | "active" | "completed" | "failed";
  conditionLabel?: string;
  [key: string]: unknown;
}

/* -- Status style map --------------------------------- */

const STATUS_STYLES: Record<
  ExecutionEdgeData["executionStatus"],
  { stroke: string; strokeWidth: number; strokeDasharray?: string }
> = {
  idle: {
    stroke: "var(--orbflow-edge-color)",
    strokeWidth: 1.5,
  },
  active: {
    stroke: STATUS_COLORS.running,
    strokeWidth: 2.5,
  },
  completed: {
    stroke: STATUS_COLORS.completed,
    strokeWidth: 2,
  },
  failed: {
    stroke: STATUS_COLORS.failed,
    strokeWidth: 1.5,
    strokeDasharray: "5,5",
  },
};

const LABEL_TINTS: Record<
  ExecutionEdgeData["executionStatus"],
  { bg: string; border: string; text: string }
> = {
  idle: {
    bg: "bg-orbflow-glass-bg",
    border: "border-orbflow-border",
    text: "text-orbflow-text-muted",
  },
  active: {
    bg: "",
    border: "",
    text: "",
  },
  completed: {
    bg: "",
    border: "",
    text: "",
  },
  failed: {
    bg: "",
    border: "",
    text: "",
  },
};

const LABEL_INLINE_STYLES: Record<
  ExecutionEdgeData["executionStatus"],
  React.CSSProperties
> = {
  idle: {},
  active: {
    backgroundColor: "rgba(74, 154, 175, 0.12)",
    borderColor: "rgba(74, 154, 175, 0.25)",
    color: "rgba(74, 154, 175, 0.9)",
  },
  completed: {
    backgroundColor: "rgba(16, 185, 129, 0.12)",
    borderColor: "rgba(16, 185, 129, 0.25)",
    color: "rgba(16, 185, 129, 0.9)",
  },
  failed: {
    backgroundColor: "rgba(217, 69, 79, 0.12)",
    borderColor: "rgba(217, 69, 79, 0.25)",
    color: "rgba(217, 69, 79, 0.9)",
  },
};

/* -- Component ---------------------------------------- */

function ExecutionEdgeInner({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
}: EdgeProps) {
  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  const status =
    (data?.executionStatus as ExecutionEdgeData["executionStatus"]) || "idle";
  const conditionLabel = (data?.conditionLabel as string) || "";

  const style = STATUS_STYLES[status];
  const labelTint = LABEL_TINTS[status];
  const labelInline = LABEL_INLINE_STYLES[status];

  const truncatedLabel =
    conditionLabel.length > 30
      ? conditionLabel.slice(0, 30) + "\u2026"
      : conditionLabel;

  return (
    <>
      <BaseEdge
        id={id}
        path={edgePath}
        style={{
          stroke: style.stroke,
          strokeWidth: style.strokeWidth,
          strokeDasharray: style.strokeDasharray,
          transition: "stroke 0.3s, stroke-width 0.3s",
        }}
      />

      {/* Animated particles for active edges */}
      {status === "active" && (
        <>
          <circle r="4" fill={STATUS_COLORS.running} opacity="0.9">
            <animateMotion
              dur="1.5s"
              repeatCount="indefinite"
              path={edgePath}
            />
          </circle>
          <circle r="3" fill={STATUS_COLORS.running} opacity="0.5">
            <animateMotion
              dur="1.5s"
              begin="0.75s"
              repeatCount="indefinite"
              path={edgePath}
            />
          </circle>
        </>
      )}

      {/* Condition label at edge midpoint */}
      {conditionLabel && (
        <EdgeLabelRenderer>
          <div
            className={cn(
              "nodrag nopan absolute pointer-events-none px-2 py-0.5 rounded-md text-micro font-mono border backdrop-blur-md transition-all duration-200",
              labelTint.bg,
              labelTint.border,
              labelTint.text
            )}
            style={{
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              ...labelInline,
            }}
            title={conditionLabel}
          >
            {truncatedLabel}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const ExecutionEdge = memo(ExecutionEdgeInner);
