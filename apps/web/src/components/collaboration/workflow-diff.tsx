"use client";

import { memo, useMemo, useState } from "react";
import type {
  WorkflowDiff as WorkflowDiffType,
  ReviewComment,
  Workflow,
  Instance,
  NodeState,
} from "@orbflow/core/types";
import { ExecutionFlowGraph } from "@/components/execution-viewer/execution-flow-graph";
import { cn } from "@/lib/cn";

/* =======================================================
   Props
   ======================================================= */

export type DiffStatus = "added" | "removed" | "modified" | "unchanged";

export interface WorkflowDiffProps {
  baseDefinition: Record<string, unknown>;
  proposedDefinition: Record<string, unknown>;
  diff: WorkflowDiffType;
  comments?: ReviewComment[];
  onNodeClick?: (nodeId: string, side: "base" | "proposed") => void;
}

/* =======================================================
   Helpers -- convert raw definitions to Workflow + Instance
   for reuse with ExecutionFlowGraph
   ======================================================= */

/** Map diff status to an execution status so ExecutionNode renders correct colors. */
const DIFF_TO_EXEC: Record<DiffStatus, string> = {
  added: "completed",    // green
  removed: "failed",     // red
  modified: "running",   // cyan -- progress bar is hidden via hideProgress prop
  unchanged: "pending",  // gray/neutral
};

function buildWorkflow(def: Record<string, unknown>): Workflow {
  const rawNodes = (def.nodes as Array<Record<string, unknown>>) ?? [];
  const rawEdges = (def.edges as Array<Record<string, unknown>>) ?? [];

  return {
    id: "diff-preview",
    name: "Diff Preview",
    version: 1,
    status: "active",
    nodes: rawNodes.map((n) => {
      // Support both ReactFlow format (data nested) and wire format (flat).
      // ReactFlow: { id, type: "task", data: { pluginRef, label, kind, ... }, position }
      // Wire:     { id, plugin_ref, name, kind, position }
      const data = (n.data as Record<string, unknown>) ?? {};
      const pluginRef = String(data.pluginRef ?? n.plugin_ref ?? n.type ?? "");
      const name = String(data.label ?? n.name ?? n.id ?? "");
      const kind = (data.kind ?? n.kind ?? "action") as "trigger" | "action" | "capability";
      const pos = (n.position as { x: number; y: number }) ?? { x: 0, y: 0 };

      return {
        id: String(n.id ?? ""),
        name,
        type: pluginRef,
        plugin_ref: pluginRef,
        kind,
        position: pos,
      };
    }),
    edges: rawEdges.map((e) => ({
      id: String(e.id ?? `${e.source}-${e.target}`),
      source: String(e.source ?? ""),
      target: String(e.target ?? ""),
      condition: e.condition as string | undefined,
    })),
    created_at: "",
    updated_at: "",
  };
}

function buildInstance(
  workflow: Workflow,
  diff: WorkflowDiffType,
  side: "base" | "proposed",
): Instance {
  const nodeStates: Record<string, NodeState> = {};

  for (const node of workflow.nodes) {
    let diffStatus: DiffStatus = "unchanged";

    if (side === "proposed" && diff.added_nodes.includes(node.id)) {
      diffStatus = "added";
    } else if (side === "base" && diff.removed_nodes.includes(node.id)) {
      diffStatus = "removed";
    } else if (diff.modified_nodes.includes(node.id)) {
      diffStatus = "modified";
    }

    nodeStates[node.id] = {
      node_id: node.id,
      status: DIFF_TO_EXEC[diffStatus] as NodeState["status"],
      attempt: 1,
    };
  }

  return {
    id: `diff-${side}`,
    workflow_id: "diff-preview",
    status: "completed" as Instance["status"],
    node_states: nodeStates,
    created_at: "",
    updated_at: "",
  };
}

/* =======================================================
   WorkflowDiff -- side-by-side view using ExecutionFlowGraph
   ======================================================= */

export const WorkflowDiff = memo(function WorkflowDiff({
  baseDefinition,
  proposedDefinition,
  diff,
  onNodeClick,
}: WorkflowDiffProps) {
  const baseWorkflow = useMemo(() => buildWorkflow(baseDefinition), [baseDefinition]);
  const proposedWorkflow = useMemo(() => buildWorkflow(proposedDefinition), [proposedDefinition]);

  const baseInstance = useMemo(
    () => buildInstance(baseWorkflow, diff, "base"),
    [baseWorkflow, diff],
  );
  const proposedInstance = useMemo(
    () => buildInstance(proposedWorkflow, diff, "proposed"),
    [proposedWorkflow, diff],
  );

  const stats = useMemo(
    () => ({
      added: diff.added_nodes.length,
      removed: diff.removed_nodes.length,
      modified: diff.modified_nodes.length,
    }),
    [diff],
  );

  const hasBase = baseWorkflow.nodes.length > 0;
  const hasProposed = proposedWorkflow.nodes.length > 0;

  return (
    <div className="flex flex-col h-full gap-2">
      {/* Side-by-side panels */}
      <div className="flex-1 flex gap-2 min-h-0">
        {/* Base version */}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="px-3 py-1.5 text-xs font-medium text-[var(--orbflow-text-muted)] border border-[var(--orbflow-border)] border-b-0 rounded-t-lg bg-[var(--orbflow-surface)]">
            Base Version
            <span className="ml-1.5 text-[var(--orbflow-text-faint)]">
              ({baseWorkflow.nodes.length} nodes)
            </span>
          </div>
          {hasBase ? (
            <ExecutionFlowGraph
              key={`base-${baseWorkflow.nodes.length}`}
              workflow={baseWorkflow}
              instance={baseInstance}
              onNodeClick={onNodeClick ? (id) => onNodeClick(id, "base") : undefined}
              className="flex-1 rounded-t-none"
              hideProgress
              hideBadges
              flowId="diff-base"
            />
          ) : (
            <div className="flex-1 flex items-center justify-center border border-[var(--orbflow-border)] rounded-b-lg text-sm text-[var(--orbflow-text-faint)]">
              No base version available
            </div>
          )}
        </div>

        {/* Proposed version */}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="px-3 py-1.5 text-xs font-medium text-[var(--orbflow-text-muted)] border border-[var(--orbflow-border)] border-b-0 rounded-t-lg bg-[var(--orbflow-surface)]">
            Proposed Changes
            <span className="ml-1.5 text-[var(--orbflow-text-faint)]">
              ({proposedWorkflow.nodes.length} nodes)
            </span>
          </div>
          {hasProposed ? (
            <ExecutionFlowGraph
              key={`proposed-${proposedWorkflow.nodes.length}`}
              workflow={proposedWorkflow}
              instance={proposedInstance}
              onNodeClick={onNodeClick ? (id) => onNodeClick(id, "proposed") : undefined}
              className="flex-1 rounded-t-none"
              hideProgress
              hideBadges
              flowId="diff-proposed"
            />
          ) : (
            <div className="flex-1 flex items-center justify-center border border-[var(--orbflow-border)] rounded-b-lg text-sm text-[var(--orbflow-text-faint)]">
              No proposed definition
            </div>
          )}
        </div>
      </div>

      {/* Legend bar */}
      <div className={cn(
        "flex items-center gap-4 px-3 py-2 text-xs text-[var(--orbflow-text-muted)]",
        "border border-[var(--orbflow-border)] rounded-lg bg-[var(--orbflow-surface)]",
      )}>
        <span className="font-medium">Changes:</span>
        {stats.added > 0 && (
          <span className="flex items-center gap-1">
            <span className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: "var(--exec-dot-completed, #10b981)" }} />
            {stats.added} added
          </span>
        )}
        {stats.removed > 0 && (
          <span className="flex items-center gap-1">
            <span className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: "var(--exec-dot-failed, #ef4444)" }} />
            {stats.removed} removed
          </span>
        )}
        {stats.modified > 0 && (
          <span className="flex items-center gap-1">
            <span className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: "var(--exec-dot-running, #22d3ee)" }} />
            {stats.modified} modified
          </span>
        )}
        {stats.added === 0 && stats.removed === 0 && stats.modified === 0 && (
          <span>No changes detected</span>
        )}
      </div>
    </div>
  );
});
