"use client";

import { useMemo, useState, useCallback } from "react";
import { cn } from "@/lib/cn";
import { NodeIcon } from "@/core/components/icons";
import type { Instance, Workflow, WorkflowNode } from "@/lib/api";
import { STATUS_COLORS, STATUS_LABELS, formatDurationMs, topoSortIds } from "@/lib/execution";

/* -- Types -------------------------------------------- */

interface DurationTimelineProps {
  instance: Instance;
  workflow: Workflow;
  onNodeClick?: (nodeId: string) => void;
}

interface TooltipState {
  nodeId: string;
  x: number;
  y: number;
}

interface BarLayout {
  nodeId: string;
  name: string;
  status: string;
  leftPct: number;
  widthPct: number;
  durationMs: number;
}

/* -- Constants ---------------------------------------- */

const MIN_BAR_WIDTH_PCT = 2;

/* -- Helpers ------------------------------------------ */

function computeTickMarks(totalMs: number): number[] {
  if (totalMs <= 0) return [];

  let intervalMs: number;
  if (totalMs < 10_000) intervalMs = 1_000;
  else if (totalMs < 60_000) intervalMs = 5_000;
  else if (totalMs < 300_000) intervalMs = 30_000;
  else intervalMs = 60_000;

  const ticks: number[] = [];
  let t = 0;
  while (t <= totalMs) {
    ticks.push(t);
    t += intervalMs;
  }
  return ticks;
}

function formatTickLabel(ms: number): string {
  if (ms === 0) return "0s";
  if (ms < 60_000) return `${Math.round(ms / 1000)}s`;
  const m = Math.floor(ms / 60_000);
  const s = Math.round((ms % 60_000) / 1000);
  return s > 0 ? `${m}m${s}s` : `${m}m`;
}

/* -- Bar layout computation --------------------------- */

function computeBarLayouts(
  sortedNodeIds: string[],
  nodeStates: Instance["node_states"],
  totalDurationMs: number,
  nodeMap: Map<string, WorkflowNode>,
): BarLayout[] {
  const terminalStatuses = new Set(["completed", "failed", "cancelled"]);
  const layouts: BarLayout[] = [];

  // Separate nodes by category
  const terminalNodes: string[] = [];
  const runningNodes: string[] = [];
  const otherNodes: string[] = [];

  for (const nodeId of sortedNodeIds) {
    const state = nodeStates[nodeId];
    const status = state?.status || "pending";
    if (terminalStatuses.has(status)) {
      terminalNodes.push(nodeId);
    } else if (status === "running") {
      runningNodes.push(nodeId);
    } else {
      otherNodes.push(nodeId);
    }
  }

  const terminalCount = terminalNodes.length;
  const segmentDuration =
    terminalCount > 0 ? totalDurationMs / terminalCount : totalDurationMs;

  // Terminal nodes get equal slices
  for (let i = 0; i < terminalNodes.length; i++) {
    const nodeId = terminalNodes[i];
    const state = nodeStates[nodeId];
    const node = nodeMap.get(nodeId);
    const leftPct = (i * segmentDuration / totalDurationMs) * 100;
    const widthPct = Math.max(
      MIN_BAR_WIDTH_PCT,
      (segmentDuration / totalDurationMs) * 100,
    );

    layouts.push({
      nodeId,
      name: node?.name || nodeId,
      status: state?.status || "completed",
      leftPct,
      widthPct: Math.min(widthPct, 100 - leftPct),
      durationMs: segmentDuration,
    });
  }

  // Running nodes start at end of last terminal segment and extend to 100%
  const runningStartPct = terminalCount > 0 ? 100 : 0;
  for (const nodeId of runningNodes) {
    const node = nodeMap.get(nodeId);
    const startPct = Math.min(runningStartPct, 98);
    layouts.push({
      nodeId,
      name: node?.name || nodeId,
      status: "running",
      leftPct: startPct,
      widthPct: Math.max(MIN_BAR_WIDTH_PCT, 100 - startPct),
      durationMs: 0, // duration is live, not fixed
    });
  }

  // Pending / skipped nodes get no bar
  for (const nodeId of otherNodes) {
    const state = nodeStates[nodeId];
    const node = nodeMap.get(nodeId);
    layouts.push({
      nodeId,
      name: node?.name || nodeId,
      status: state?.status || "pending",
      leftPct: 0,
      widthPct: 0,
      durationMs: 0,
    });
  }

  return layouts;
}

/* -- Tooltip component -------------------------------- */

function Tooltip({
  bar,
  x,
  y,
}: {
  bar: BarLayout;
  x: number;
  y: number;
}) {
  const color = STATUS_COLORS[bar.status] || STATUS_COLORS.pending;
  const label = STATUS_LABELS[bar.status] || bar.status;
  const durationText =
    bar.status === "running"
      ? "running"
      : bar.status === "pending" || bar.status === "skipped"
        ? "--"
        : formatDurationMs(bar.durationMs);

  return (
    <div
      className="fixed z-[100] pointer-events-none"
      style={{ left: x, top: y - 8, transform: "translate(-50%, -100%)" }}
    >
      <div
        className="rounded-lg px-3 py-2 border border-orbflow-border shadow-lg bg-orbflow-elevated backdrop-blur-md"
      >
        <p className="text-body-sm font-medium text-orbflow-text-secondary truncate max-w-[180px]">
          {bar.name}
        </p>
        <div className="flex items-center gap-2 mt-1">
          <div
            className="w-1.5 h-1.5 rounded-full"
            style={{ backgroundColor: color }}
          />
          <span className="text-caption text-orbflow-text-faint">{label}</span>
          <span className="text-caption font-mono tabular-nums text-orbflow-text-ghost">
            {durationText}
          </span>
        </div>
      </div>
    </div>
  );
}

/* -- Main component ----------------------------------- */

function DurationTimeline({
  instance,
  workflow,
  onNodeClick,
}: DurationTimelineProps) {
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  const nodeStates = instance.node_states;

  // Build node lookup map
  const nodeMap = useMemo(
    () => new Map(workflow.nodes.map((n) => [n.id, n])),
    [workflow.nodes],
  );

  // Topological order
  const sortedNodeIds = useMemo(() => topoSortIds(workflow), [workflow]);

  // Total duration (minimum 1000ms)
  const totalDurationMs = useMemo(() => {
    const start = new Date(instance.created_at).getTime();
    const end = new Date(instance.updated_at).getTime();
    return Math.max(1000, end - start);
  }, [instance.created_at, instance.updated_at]);

  // Compute bar layouts
  const barLayouts = useMemo(
    () => computeBarLayouts(sortedNodeIds, nodeStates, totalDurationMs, nodeMap),
    [sortedNodeIds, nodeStates, totalDurationMs, nodeMap],
  );

  // Time axis ticks
  const ticks = useMemo(
    () => computeTickMarks(totalDurationMs),
    [totalDurationMs],
  );

  // Check if all nodes are pending (no bars to show)
  const allPending = useMemo(
    () => barLayouts.every((b) => b.status === "pending" || b.status === "skipped"),
    [barLayouts],
  );

  // Interaction handlers
  const handleMouseEnter = useCallback(
    (nodeId: string, e: React.MouseEvent) => {
      setTooltip({ nodeId, x: e.clientX, y: e.clientY });
    },
    [],
  );

  const handleMouseMove = useCallback(
    (nodeId: string, e: React.MouseEvent) => {
      setTooltip({ nodeId, x: e.clientX, y: e.clientY });
    },
    [],
  );

  const handleMouseLeave = useCallback(() => {
    setTooltip(null);
  }, []);

  const handleClick = useCallback(
    (nodeId: string) => {
      onNodeClick?.(nodeId);
    },
    [onNodeClick],
  );

  // Edge case: no node_states at all
  if (!nodeStates || Object.keys(nodeStates).length === 0) {
    return (
      <div className="flex items-center justify-center py-8">
        <div className="flex items-center gap-2 text-orbflow-text-ghost">
          <NodeIcon name="clock" className="w-4 h-4" />
          <span className="text-body">No timing data available</span>
        </div>
      </div>
    );
  }

  const tooltipBar = tooltip
    ? barLayouts.find((b) => b.nodeId === tooltip.nodeId)
    : null;

  return (
    <div className="select-none">
      {/* Rows */}
      <div className="flex flex-col" style={{ gap: "2px" }}>
        {barLayouts.map((bar) => {
          const color = STATUS_COLORS[bar.status] || STATUS_COLORS.pending;
          const hasBar = bar.widthPct > 0;
          const isRunning = bar.status === "running";
          const isPendingLike =
            bar.status === "pending" || bar.status === "skipped";
          const durationText = isRunning
            ? "running"
            : isPendingLike
              ? "pending"
              : formatDurationMs(bar.durationMs);

          return (
            <div
              key={bar.nodeId}
              className="flex items-center"
              style={{ height: 28 }}
            >
              {/* Node name column */}
              <button
                className={cn(
                  "shrink-0 text-left truncate text-body font-mono text-orbflow-text-secondary rounded",
                  "hover:text-orbflow-text-primary transition-colors",
                  "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                  onNodeClick ? "cursor-pointer" : "cursor-default",
                )}
                style={{ width: 120 }}
                onClick={() => handleClick(bar.nodeId)}
                title={bar.name}
                aria-label={`View details for ${bar.name}`}
              >
                {bar.name}
              </button>

              {/* Bar area */}
              <div className="flex-1 relative flex items-center h-full ml-2">
                {hasBar && (
                  <div
                    className={cn(
                      "absolute rounded-full",
                      isRunning && "animate-pulse",
                      onNodeClick ? "cursor-pointer" : "cursor-default",
                    )}
                    style={{
                      left: `${bar.leftPct}%`,
                      width: `${bar.widthPct}%`,
                      height: 6,
                      backgroundColor: color,
                      opacity: 0.75,
                    }}
                    onClick={() => handleClick(bar.nodeId)}
                    onMouseEnter={(e) => handleMouseEnter(bar.nodeId, e)}
                    onMouseMove={(e) => handleMouseMove(bar.nodeId, e)}
                    onMouseLeave={handleMouseLeave}
                  />
                )}

                {/* Duration label to the right of bar */}
                <span
                  className="absolute text-caption tabular-nums text-orbflow-text-faint whitespace-nowrap"
                  style={{
                    left: hasBar
                      ? `${Math.min(bar.leftPct + bar.widthPct + 1, 92)}%`
                      : "0%",
                  }}
                >
                  {durationText}
                </span>
              </div>
            </div>
          );
        })}
      </div>

      {/* Time axis */}
      {!allPending && ticks.length > 1 && (
        <div className="flex items-start mt-2" style={{ paddingLeft: 128 }}>
          <div className="flex-1 relative" style={{ height: 16 }}>
            {/* Axis line */}
            <div
              className="absolute top-0 left-0 right-0"
              style={{
                height: 1,
                backgroundColor: "rgba(100, 116, 139, 0.12)",
              }}
            />

            {/* Tick marks + labels */}
            {ticks.map((ms) => {
              const pct = (ms / totalDurationMs) * 100;
              return (
                <div
                  key={ms}
                  className="absolute"
                  style={{ left: `${pct}%`, top: 0 }}
                >
                  <div
                    style={{
                      width: 1,
                      height: 4,
                      backgroundColor: "rgba(100, 116, 139, 0.2)",
                    }}
                  />
                  <span
                    className="absolute text-micro text-orbflow-text-faint whitespace-nowrap"
                    style={{
                      top: 5,
                      left: "50%",
                      transform: "translateX(-50%)",
                    }}
                  >
                    {formatTickLabel(ms)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Tooltip */}
      {tooltip && tooltipBar && (
        <Tooltip bar={tooltipBar} x={tooltip.x} y={tooltip.y} />
      )}
    </div>
  );
}

export { DurationTimeline };
