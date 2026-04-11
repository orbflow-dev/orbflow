"use client";

import { memo } from "react";
import type { NodeProps } from "@xyflow/react";
import { Handle, Position } from "@xyflow/react";
import { cn } from "@/lib/cn";

export type DiffStatus = "added" | "removed" | "modified" | "unchanged";

export interface DiffNodeData {
  label: string;
  pluginRef: string;
  kind: "trigger" | "action" | "capability";
  diffStatus: DiffStatus;
  commentCount?: number;
  [key: string]: unknown;
}

type DiffNodeProps = NodeProps & { data: DiffNodeData };

const statusStyles: Record<DiffStatus, string> = {
  added: "ring-2 ring-emerald-500 bg-emerald-500/10",
  removed: "ring-2 ring-red-500 bg-red-500/10 opacity-60",
  modified: "ring-2 ring-amber-500 bg-amber-500/10",
  unchanged: "ring-1 ring-[var(--orbflow-node-border)]",
};

const pillStyles: Record<Exclude<DiffStatus, "unchanged">, string> = {
  added: "bg-emerald-500 text-white",
  removed: "bg-red-500 text-white",
  modified: "bg-amber-500 text-white",
};

const pillLabels: Record<Exclude<DiffStatus, "unchanged">, string> = {
  added: "Added",
  removed: "Removed",
  modified: "Modified",
};

const iconColors: Record<DiffNodeData["kind"], string> = {
  trigger: "bg-red-500/20 text-red-400",
  action: "bg-blue-500/20 text-blue-400",
  capability: "bg-sky-500/20 text-sky-400",
};

const handleStyle =
  "w-2 h-2 rounded-full border-2 border-[var(--orbflow-node-border)] bg-[var(--orbflow-node-bg)]";

export const DiffNode = memo(function DiffNode({ data }: DiffNodeProps) {
  const { label, pluginRef, kind, diffStatus, commentCount } = data;
  const showPill = diffStatus !== "unchanged";
  const showComments = typeof commentCount === "number" && commentCount > 0;

  return (
    <div className="relative">
      <div
        className={cn(
          "relative w-[200px] h-16 rounded-lg bg-[var(--orbflow-node-bg)] flex items-center gap-2.5 px-3",
          statusStyles[diffStatus],
        )}
      >
        {kind !== "trigger" && (
          <Handle type="target" position={Position.Top} className={handleStyle} />
        )}
        {kind !== "capability" && (
          <Handle type="source" position={Position.Bottom} className={handleStyle} />
        )}

        <div
          className={cn(
            "w-8 h-8 rounded-full flex items-center justify-center text-xs font-semibold shrink-0",
            iconColors[kind],
          )}
        >
          {pluginRef.charAt(0).toUpperCase()}
        </div>

        <div className="min-w-0 flex-1">
          <p
            className={cn(
              "text-sm font-medium text-[var(--orbflow-text)] truncate",
              diffStatus === "removed" && "line-through",
            )}
          >
            {label}
          </p>
          <p className="text-[11px] text-[var(--orbflow-text-muted)] truncate">{pluginRef}</p>
        </div>

        {showComments && (
          <div className="absolute -top-2 -right-2 w-5 h-5 rounded-full bg-blue-500 text-white text-[10px] flex items-center justify-center">
            {commentCount}
          </div>
        )}
      </div>

      {showPill && (
        <div className="flex justify-center mt-1">
          <span
            className={cn(
              "text-[10px] px-1.5 py-0.5 rounded-full",
              pillStyles[diffStatus as Exclude<DiffStatus, "unchanged">],
            )}
          >
            {pillLabels[diffStatus as Exclude<DiffStatus, "unchanged">]}
          </span>
        </div>
      )}
    </div>
  );
});
