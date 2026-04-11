"use client";

import { useMemo, useCallback, useEffect, useRef } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  Controls,
  useReactFlow,
  useNodesInitialized,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { ExecutionNode, type ExecutionNodeData } from "./execution-node";
import { ExecutionEdge, type ExecutionEdgeData } from "./execution-edge";
import type { Workflow, Instance } from "@/lib/api";
import { cn } from "@/lib/cn";
import { STATUS_COLORS } from "@/lib/execution";

/* -- Custom type/edge maps (OUTSIDE component for perf) -- */

const nodeTypes = { executionNode: ExecutionNode };
const edgeTypes = { executionEdge: ExecutionEdge };

/* -- Edge status derivation -------------------------------- */

function deriveEdgeStatus(
  sourceStatus: string | undefined,
  targetStatus: string | undefined,
): ExecutionEdgeData["executionStatus"] {
  if (
    sourceStatus === "completed" &&
    (targetStatus === "running" || targetStatus === "queued")
  ) {
    return "active";
  }
  if (sourceStatus === "completed" && targetStatus === "completed") {
    return "completed";
  }
  if (targetStatus === "failed") {
    return "failed";
  }
  return "idle";
}

/* -- Props ------------------------------------------------- */

interface ExecutionFlowGraphProps {
  workflow: Workflow;
  instance: Instance;
  onNodeClick?: (nodeId: string) => void;
  className?: string;
  /** Refit the viewport when the graph becomes visible or context changes. */
  autoFocus?: boolean;
  /** Hide the progress summary bar (useful for diff views). */
  hideProgress?: boolean;
  /** Hide status badges and execution tooltips on nodes (useful for diff views). */
  hideBadges?: boolean;
  /** Unique id for the ReactFlow instance (required when multiple graphs render on the same page). */
  flowId?: string;
}

/* -- Inner component (must be inside ReactFlowProvider) --- */

function ExecutionFlowGraphInner({
  workflow,
  instance,
  onNodeClick,
  className,
  autoFocus = true,
  hideProgress,
  hideBadges,
  flowId,
}: ExecutionFlowGraphProps) {
  const nodeCount = workflow.nodes.length;
  const fitPadding = useMemo(() => nodeCount <= 2 ? 0.04 : nodeCount <= 4 ? 0.08 : nodeCount <= 8 ? 0.12 : 0.16, [nodeCount]);
  const fitMaxZoom = useMemo(() => nodeCount <= 2 ? 2.2 : nodeCount <= 4 ? 1.75 : nodeCount <= 8 ? 1.25 : 1, [nodeCount]);

  // Stabilise dependency: only recompute when node_states actually change
  const nodeStates = instance.node_states;

  /* Convert workflow nodes -> React Flow nodes */
  const rfNodes = useMemo(() => {
    return workflow.nodes.map((node) => {
      const nodeState = nodeStates?.[node.id];

      // Compute duration if both timestamps exist
      let duration: number | undefined;
      if (nodeState?.started_at && nodeState?.ended_at) {
        duration = Math.max(
          0,
          new Date(nodeState.ended_at).getTime() -
            new Date(nodeState.started_at).getTime(),
        );
      }

      // Build a short output preview for tooltip (truncate to prevent huge strings)
      let outputPreview: string | undefined;
      if (nodeState?.output) {
        try {
          const raw = JSON.stringify(nodeState.output);
          outputPreview = raw.length > 200 ? raw.slice(0, 200) + "…" : raw;
        } catch {
          outputPreview = undefined;
        }
      }

      const data: ExecutionNodeData = {
        pluginRef: node.plugin_ref,
        label: node.name,
        kind: (node.kind || "action") as "trigger" | "action" | "capability",
        executionStatus: nodeState?.status,
        error: nodeState?.error,
        duration,
        attempt: nodeState?.attempt,
        hasOutput: !!nodeState?.output,
        outputPreview,
        onNodeClick,
        hideBadge: hideBadges,
      };

      return {
        id: node.id,
        type: "executionNode",
        position: { x: node.position.x, y: node.position.y },
        data,
      };
    });
  }, [workflow, nodeStates, onNodeClick, hideBadges]);

  /* Convert workflow edges -> React Flow edges */
  const rfEdges = useMemo(() => {
    return workflow.edges.map((edge) => {
      const sourceStatus = nodeStates?.[edge.source]?.status;
      const targetStatus = nodeStates?.[edge.target]?.status;

      const data: ExecutionEdgeData = {
        conditionLabel: edge.condition || "",
        executionStatus: deriveEdgeStatus(sourceStatus, targetStatus),
      };

      return {
        id: edge.id,
        source: edge.source,
        target: edge.target,
        type: "executionEdge",
        data,
      };
    });
  }, [workflow, nodeStates]);

  /* Compute progress stats — single pass O(n) */
  const stats = useMemo(() => {
    const counts = { completed: 0, running: 0, failed: 0, pending: 0, cancelled: 0, skipped: 0 };
    for (const s of Object.values(nodeStates || {})) {
      if (s.status in counts) counts[s.status as keyof typeof counts]++;
    }
    return { total: workflow.nodes.length, ...counts };
  }, [nodeStates, workflow.nodes.length]);

  const done = stats.completed + stats.failed + stats.cancelled + stats.skipped;

  /* Build progress bar segments */
  const segments = useMemo(() => {
    if (stats.total === 0) return [];

    const order: { key: string; count: number }[] = [
      { key: "completed", count: stats.completed },
      { key: "failed", count: stats.failed },
      { key: "running", count: stats.running },
      { key: "cancelled", count: stats.cancelled },
      { key: "skipped", count: stats.skipped },
      { key: "pending", count: stats.pending },
    ];

    return order
      .filter((s) => s.count > 0)
      .map((s) => ({
        key: s.key,
        pct: (s.count / stats.total) * 100,
        color: STATUS_COLORS[s.key] || STATUS_COLORS.pending,
      }));
  }, [stats]);

  const containerRef = useRef<HTMLDivElement>(null);
  const nodesInitialized = useNodesInitialized();
  const { fitView: reFitView } = useReactFlow();

  const scheduleFitView = useCallback(() => {
    if (!autoFocus) return;
    const fit = () => reFitView({ padding: fitPadding, maxZoom: fitMaxZoom, duration: 0 });

    requestAnimationFrame(() => {
      requestAnimationFrame(fit);
    });
  }, [autoFocus, reFitView, fitPadding, fitMaxZoom]);

  useEffect(() => {
    if (!autoFocus || !nodesInitialized) return;

    scheduleFitView();

    if (typeof ResizeObserver === "undefined" || !containerRef.current) return;

    let rafId = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(scheduleFitView);
    });

    observer.observe(containerRef.current);
    window.addEventListener("resize", scheduleFitView);

    return () => {
      cancelAnimationFrame(rafId);
      observer.disconnect();
      window.removeEventListener("resize", scheduleFitView);
    };
  }, [autoFocus, nodesInitialized, scheduleFitView, workflow.id, instance.id]);

  return (
    <div
      ref={containerRef}
      className={cn(
        "flex flex-col rounded-xl border border-orbflow-border overflow-hidden",
        className,
      )}
    >
      {/* -- Progress summary bar ------------------------ */}
      {!hideProgress && <div className="flex flex-wrap items-center gap-4 border-b border-orbflow-border/50 bg-orbflow-surface/20 px-4 py-3.5 lg:px-5">
        <div className="flex items-end gap-2">
          <span className="text-base font-semibold tabular-nums tracking-tight text-orbflow-text-secondary">
            {done}/{stats.total}
          </span>
          <span className="pb-0.5 text-sm text-orbflow-text-ghost">
            nodes settled
          </span>
        </div>

        {/* Segmented progress bar */}
        <div className="h-1.5 max-w-[280px] flex-1 overflow-hidden rounded-full bg-orbflow-surface-hover/70">
          {segments.length > 0 && (
            <div className="flex h-full">
              {segments.map((seg) => (
                <div
                  key={seg.key}
                  className="h-full transition-all duration-500"
                  style={{
                    width: `${seg.pct}%`,
                    backgroundColor: seg.color,
                    opacity: 0.6,
                  }}
                />
              ))}
            </div>
          )}
        </div>

        {/* Inline status counters */}
        <div className="ml-auto flex flex-wrap items-center gap-2">
          {stats.completed > 0 && (
            <span className="flex items-center gap-1.5 rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-3 py-1.5 text-xs text-orbflow-text-faint">
              <div
                className="h-1.5 w-1.5 rounded-full"
                style={{ backgroundColor: STATUS_COLORS.completed, opacity: 0.8 }}
              />
              {stats.completed} completed
            </span>
          )}
          {stats.failed > 0 && (
            <span className="flex items-center gap-1.5 rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-3 py-1.5 text-xs" style={{ color: "var(--exec-text-failed)" }}>
              <div
                className="h-1.5 w-1.5 rounded-full"
                style={{ backgroundColor: STATUS_COLORS.failed, opacity: 0.7 }}
              />
              {stats.failed} failed
            </span>
          )}
          {stats.running > 0 && (
            <span className="flex items-center gap-1.5 rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-3 py-1.5 text-xs" style={{ color: "var(--exec-text-running)" }}>
              <div
                className="h-1.5 w-1.5 rounded-full animate-exec-step-pulse"
                style={{ backgroundColor: STATUS_COLORS.running }}
              />
              {stats.running} running
            </span>
          )}
        </div>
      </div>}

      {/* -- React Flow canvas --------------------------- */}
      <div className="flex-1 min-h-[400px]">
        <ReactFlow
          id={flowId}
          nodes={rfNodes}
          edges={rfEdges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable={true}
          panOnDrag={true}
          zoomOnScroll={true}
          fitView={autoFocus}
          fitViewOptions={{ padding: fitPadding, maxZoom: fitMaxZoom }}
          onInit={scheduleFitView}
          minZoom={0.45}
          maxZoom={2.25}
          proOptions={{ hideAttribution: true }}
        >
          <Controls showInteractive={false} />
          <Background gap={24} size={1} color="color-mix(in srgb, var(--orbflow-border) 80%, transparent)" />
        </ReactFlow>
      </div>
    </div>
  );
}

/* -- Exported wrapper with ReactFlowProvider --------------- */

export function ExecutionFlowGraph(props: ExecutionFlowGraphProps) {
  return (
    <ReactFlowProvider>
      <ExecutionFlowGraphInner {...props} />
    </ReactFlowProvider>
  );
}
