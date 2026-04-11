"use client";

import { useMemo, useState, useCallback } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { STATUS_THEMES, FALLBACK_THEME } from "@/lib/execution";
import { ExecutionGraph } from "./execution-graph";
import { ExecutionFlowGraph } from "./execution-flow-graph";
import { DurationTimeline } from "./duration-timeline";
import { NodeDetailDrawer } from "./node-detail-drawer";
import { ReadOnlyNodeConfig } from "./read-only-node-config";
import { ApprovalGate } from "./approval-gate";
import { AuditTrailPanel } from "./audit-trail-panel";
import type { Workflow, Instance } from "@/lib/api";
import type { AuditVerifyResult } from "@orbflow/core";
import { instanceStats, formatOutput } from "./viewer-utils";
import { LiveDuration, CopyButton, EmptyIllustration } from "./shared-components";
import { useOrbflow } from "@/core/context/orbflow-provider";
import { api } from "@/lib/api";

const VIEW_MODES = [
  { key: "graph", icon: "git-branch", label: "Execution Map", description: "Follow the path this run took through the workflow graph." },
  { key: "timeline", icon: "grip-vertical", label: "Step Timeline", description: "Inspect each step in execution order with outputs and errors." },
  { key: "duration", icon: "clock", label: "Duration", description: "Compare how long each step took and where time was spent." },
] as const;

const DETAIL_TIME_FORMATTER = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
});

function detailTime(value?: string) {
  return value ? DETAIL_TIME_FORMATTER.format(new Date(value)) : "In progress";
}

function MetricCard({
  label,
  value,
  hint,
}: {
  label: string;
  value: React.ReactNode;
  hint: string;
}) {
  return (
    <div className="activity-metric-tile rounded-[18px] px-3 py-2.5">
      <p className="text-[9px] font-semibold uppercase tracking-[0.18em] text-orbflow-text-ghost">
        {label}
      </p>
      <div className="mt-1 text-[0.96rem] font-semibold tracking-tight text-orbflow-text-secondary">
        {value}
      </div>
      <p className="mt-0.5 text-[11px] leading-relaxed text-orbflow-text-faint">{hint}</p>
    </div>
  );
}

function ActionButton({
  icon,
  label,
  onClick,
  tone = "neutral",
  disabled = false,
}: {
  icon: string;
  label: string;
  onClick: () => void;
  tone?: "primary" | "neutral" | "danger";
  disabled?: boolean;
}) {
  const toneClass = tone === "primary"
    ? "border-electric-indigo/20 bg-electric-indigo/10 text-electric-indigo hover:bg-electric-indigo/14"
    : tone === "danger"
      ? "border-rose-500/20 bg-rose-500/[0.06] text-rose-500 hover:bg-rose-500/[0.10]"
      : "border-orbflow-border bg-orbflow-surface-hover/60 text-orbflow-text-faint hover:border-orbflow-border-hover hover:text-orbflow-text-secondary";

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex min-h-9 items-center justify-center gap-2 rounded-xl border px-3 py-2 text-[13px] font-medium transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-electric-indigo/50",
        toneClass,
        disabled && "cursor-not-allowed opacity-60",
      )}
    >
      <NodeIcon name={icon} className="h-3 w-3" />
      {label}
    </button>
  );
}

function ApprovalBannerRow({
  instanceId,
  nodeId,
  nodeName,
  onApprove,
  onReject,
  onDetails,
}: {
  instanceId: string;
  nodeId: string;
  nodeName: string;
  onApprove: (instanceId: string, nodeId: string, approvedBy?: string) => Promise<void>;
  onReject: (instanceId: string, nodeId: string, reason?: string) => Promise<void>;
  onDetails: () => void;
}) {
  const [loading, setLoading] = useState<"approve" | "reject" | null>(null);

  const runAction = async (type: "approve" | "reject") => {
    setLoading(type);
    try {
      if (type === "approve") await onApprove(instanceId, nodeId);
      else await onReject(instanceId, nodeId);
    } catch {}
    finally {
      setLoading(null);
    }
  };

  return (
    <div className="flex flex-col gap-3 rounded-[24px] border border-amber-500/15 bg-amber-500/[0.05] px-4 py-4 lg:flex-row lg:items-center">
      <div className="flex min-w-0 flex-1 items-center gap-2.5">
        <span className="h-2.5 w-2.5 rounded-full bg-amber-400 shadow-[0_0_0_5px_rgba(251,191,36,0.14)]" />
        <p className="truncate text-sm font-medium" style={{ color: "var(--orbflow-exec-cancelled)" }}>
          <strong>{nodeName}</strong> is waiting for approval
        </p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <ActionButton icon="check" label={loading === "approve" ? "Approving..." : "Approve"} onClick={() => runAction("approve")} tone="primary" disabled={loading !== null} />
        <ActionButton icon="x" label={loading === "reject" ? "Rejecting..." : "Reject"} onClick={() => runAction("reject")} tone="danger" disabled={loading !== null} />
        <ActionButton icon="eye" label="View node" onClick={onDetails} disabled={loading !== null} />
      </div>
    </div>
  );
}

interface InstanceDetailPanelProps {
  selectedInstance: Instance | null;
  executionWorkflow: Workflow | null;
  executionWorkflowError: boolean;
  isActive?: boolean;
  viewMode: "graph" | "timeline" | "duration";
  setViewMode: (mode: "graph" | "timeline" | "duration") => void;
  selectedNodeId: string | null;
  setSelectedNodeId: (id: string | null) => void;
  getWorkflowName: (wfId: string) => string;
  isRunning: boolean;
  canRerun: boolean;
  onRerun: () => void;
  onCancel: () => void;
  onRetryWorkflowFetch: () => void;
  onApprove?: (instanceId: string, nodeId: string, approvedBy?: string) => Promise<void>;
  onReject?: (instanceId: string, nodeId: string, reason?: string) => Promise<void>;
}

export function InstanceDetailPanel({
  selectedInstance,
  executionWorkflow,
  executionWorkflowError,
  isActive = false,
  viewMode,
  setViewMode,
  selectedNodeId,
  setSelectedNodeId,
  getWorkflowName,
  isRunning,
  canRerun,
  onRerun,
  onCancel,
  onRetryWorkflowFetch,
  onApprove,
  onReject,
}: InstanceDetailPanelProps) {
  const { registry } = useOrbflow();

  const selectedStats = useMemo(
    () => (selectedInstance ? instanceStats(selectedInstance.node_states) : null),
    [selectedInstance],
  );

  const selectedTheme = selectedInstance
    ? STATUS_THEMES[selectedInstance.status] || FALLBACK_THEME
    : FALLBACK_THEME;

  const resolvedCount = selectedStats
    ? selectedStats.completed + selectedStats.failed + selectedStats.cancelled + selectedStats.skipped
    : 0;
  const progressPct = selectedStats?.total ? (resolvedCount / selectedStats.total) * 100 : 0;

  const [auditState, setAuditState] = useState<"idle" | "loading" | "done">("idle");
  const [auditResult, setAuditResult] = useState<AuditVerifyResult | null>(null);
  const [auditError, setAuditError] = useState<string | null>(null);
  const [showAuditTrail, setShowAuditTrail] = useState(false);

  const activeView = VIEW_MODES.find((mode) => mode.key === viewMode) || VIEW_MODES[0];

  // Pre-build lookup map for O(1) node resolution
  const workflowNodeMap = useMemo(() => {
    if (!executionWorkflow) return new Map<string, Workflow["nodes"][number]>();
    return new Map(executionWorkflow.nodes.map((n) => [n.id, n]));
  }, [executionWorkflow]);

  const waitingNodes = useMemo(() => {
    if (!selectedInstance || !executionWorkflow) return [];
    return Object.entries(selectedInstance.node_states || {})
      .filter(([, state]) => state.status === "waiting_approval")
      .map(([nodeId]) => ({
        nodeId,
        name: workflowNodeMap.get(nodeId)?.name || nodeId,
      }));
  }, [selectedInstance, executionWorkflow, workflowNodeMap]);

  const handleVerifyAudit = useCallback(async () => {
    if (!selectedInstance) return;
    setAuditState("loading");
    setAuditResult(null);
    setAuditError(null);

    try {
      setAuditResult(await api.instances.verifyAudit(selectedInstance.id));
    } catch (err) {
      setAuditError(err instanceof Error ? err.message : "Verification failed");
    } finally {
      setAuditState("done");
    }
  }, [selectedInstance]);

  if (!selectedInstance || !selectedStats) {
    return (
      <div className="flex-1 overflow-y-auto px-5 py-6 lg:px-8 lg:py-8">
        <div className="activity-surface relative mx-auto flex min-h-full max-w-4xl items-center overflow-hidden rounded-[32px] border border-orbflow-border/70 px-6 py-8 lg:px-10 lg:py-10">
          <div className="absolute left-0 top-0 h-56 w-56 rounded-full bg-electric-indigo/8 blur-3xl" />
          <div className="absolute bottom-0 right-0 h-48 w-48 rounded-full bg-neon-cyan/7 blur-3xl" />

          <div className="relative w-full text-center">
            <div className="mb-8 flex justify-center">
              <EmptyIllustration />
            </div>
            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-orbflow-text-muted">
              Activity workspace
            </p>
            <h3 className="mt-2 text-[1.5rem] font-semibold tracking-tight text-orbflow-text-secondary">
              Pick a run to inspect what happened
            </h3>
            <p className="mx-auto mt-3 max-w-xl text-sm leading-relaxed text-orbflow-text-muted">
              The selected run opens with a readable summary, execution map, timeline, audit tools, and node details in one place.
            </p>

            <div className="mt-8 flex flex-wrap items-center justify-center gap-5 text-sm text-orbflow-text-muted">
              <span><kbd className="rounded-lg border border-orbflow-border bg-orbflow-surface-hover px-2 py-1 text-xs font-mono text-orbflow-text-secondary">Click</kbd> Select run</span>
              <span><kbd className="rounded-lg border border-orbflow-border bg-orbflow-surface-hover px-2 py-1 text-xs font-mono text-orbflow-text-secondary">J / K</kbd> Browse list</span>
              <span><kbd className="rounded-lg border border-orbflow-border bg-orbflow-surface-hover px-2 py-1 text-xs font-mono text-orbflow-text-secondary">Ctrl+Enter</kbd> Run workflow</span>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <>
      <div className="activity-detail-header relative shrink-0 border-b border-orbflow-border">
        <div className="px-4 py-3 lg:px-5 lg:py-4">
          <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[18px] border border-electric-indigo/20 bg-[linear-gradient(180deg,rgba(124,92,252,0.12)_0%,rgba(34,211,238,0.05)_100%)]">
                  <NodeIcon
                    name={selectedTheme.icon}
                    className={cn("h-3.5 w-3.5", isRunning && "animate-spin")}
                    style={{ color: selectedTheme.accent, opacity: 0.92 }}
                  />
                </div>

                <span
                  className="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.16em]"
                  style={{
                    color: selectedTheme.text,
                    borderColor: `rgba(${selectedTheme.accentRgb},0.20)`,
                    backgroundColor: `rgba(${selectedTheme.accentRgb},0.08)`,
                  }}
                >
                  <span className="h-1.5 w-1.5 rounded-full" style={{ backgroundColor: selectedTheme.accent }} />
                  {selectedTheme.label}
                </span>

                <span className="text-[10px] font-medium uppercase tracking-[0.18em] text-orbflow-text-ghost">
                  Run overview
                </span>
              </div>

              <h2 className="mt-2.5 truncate text-[1.18rem] font-semibold tracking-tight text-orbflow-text-secondary lg:text-[1.35rem]">
                {getWorkflowName(selectedInstance.workflow_id)}
              </h2>
              <p className="mt-1 max-w-2xl text-xs leading-relaxed text-orbflow-text-faint">
                Use the execution map for structure, the timeline for step-by-step output, and duration view when you need to isolate slow work.
              </p>

              <div className="mt-2.5 flex flex-wrap items-center gap-1.5 text-[11px] text-orbflow-text-faint">
                <span className="rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-2.5 py-1 font-mono">
                  {selectedInstance.id}
                </span>
                <span className="rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-2.5 py-1">
                  Started {detailTime(selectedInstance.created_at)}
                </span>
                <span className="rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-2.5 py-1">
                  Updated {detailTime(selectedInstance.updated_at)}
                </span>
              </div>
            </div>

            <div className="flex w-full flex-wrap gap-1.5 xl:w-auto xl:max-w-[20rem] xl:justify-end">
              {canRerun && (
                <ActionButton icon="play" label="Re-run workflow" onClick={onRerun} tone="primary" />
              )}
              {isRunning && (
                <ActionButton icon="x" label="Cancel run" onClick={onCancel} tone="danger" />
              )}
              <ActionButton icon="shield" label="Verify audit" onClick={handleVerifyAudit} disabled={auditState === "loading"} />
              <ActionButton icon="file-text" label={showAuditTrail ? "Hide trail" : "Open trail"} onClick={() => setShowAuditTrail((prev) => !prev)} />
            </div>
          </div>

          <div className="mt-3 grid gap-1.5 md:grid-cols-2 xl:grid-cols-4">
            <MetricCard
              label="Completion"
              value={`${Math.round(progressPct)}%`}
              hint={`${resolvedCount} of ${selectedStats.total} steps settled`}
            />
            <MetricCard
              label="Runtime"
              value={<LiveDuration startTime={selectedInstance.created_at} endTime={canRerun ? selectedInstance.updated_at : undefined} />}
              hint={isRunning ? "Still updating live" : "Final elapsed duration"}
            />
            <MetricCard
              label="Throughput"
              value={
                <>
                  {selectedStats.completed}
                  <span className="ml-1 text-xs font-normal text-orbflow-text-faint">completed</span>
                </>
              }
              hint={selectedStats.running > 0 ? `${selectedStats.running} still running` : "No live nodes"}
            />
            <MetricCard
              label="Attention"
              value={
                <>
                  {selectedStats.failed + selectedStats.cancelled}
                  <span className="ml-1 text-xs font-normal text-orbflow-text-faint">issues</span>
                </>
              }
              hint={selectedStats.pending > 0 ? `${selectedStats.pending} pending` : "Nothing queued"}
            />
          </div>

          <div className="mt-3 flex flex-col gap-2.5 border-t border-orbflow-border/50 pt-2.5 xl:flex-row xl:items-center xl:justify-between">
            <div className="inline-flex w-fit flex-wrap rounded-[18px] border border-orbflow-border/60 bg-orbflow-bg/70 p-1">
              {VIEW_MODES.map((mode) => (
                <button
                  key={mode.key}
                  onClick={() => setViewMode(mode.key)}
                  aria-label={`${mode.label} view`}
                  aria-pressed={viewMode === mode.key}
                  className={cn(
                    "flex items-center gap-2 rounded-xl px-2.5 py-1.5 text-[11px] font-medium transition-all",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-electric-indigo/50",
                    viewMode === mode.key
                      ? "bg-electric-indigo/10 text-electric-indigo"
                      : "text-orbflow-text-faint hover:text-orbflow-text-secondary",
                  )}
                >
                  <NodeIcon name={mode.icon} className="h-3 w-3" />
                  {mode.label}
                </button>
              ))}
            </div>

            <div className="flex flex-wrap items-center gap-2">
              {[
                { label: "completed", count: selectedStats.completed },
                { label: "running", count: selectedStats.running },
                { label: "failed", count: selectedStats.failed },
                { label: "pending", count: selectedStats.pending },
                { label: "cancelled", count: selectedStats.cancelled },
              ]
                .filter((item) => item.count > 0)
                .map((item) => {
                  const theme = STATUS_THEMES[item.label] || FALLBACK_THEME;
                  return (
                    <span key={item.label} className="inline-flex items-center gap-2 rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-2.5 py-1 text-[11px] font-medium text-orbflow-text-faint">
                      <span className="h-1.5 w-1.5 rounded-full" style={{ backgroundColor: theme.accent }} />
                      <span className="tabular-nums text-orbflow-text-secondary">{item.count}</span>
                      <span className="capitalize">{item.label}</span>
                    </span>
                  );
                })}

              <span className="inline-flex items-center gap-2 rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-2.5 py-1 text-[11px] font-medium text-orbflow-text-faint">
                <span className="tabular-nums text-orbflow-text-secondary">{resolvedCount}</span>
                <span>/ {selectedStats.total} steps settled</span>
              </span>
            </div>
          </div>
        </div>

        <div className="h-1" style={{ background: "rgba(124,92,252,0.06)" }}>
          <div
            className="h-full transition-all duration-700 ease-out"
            style={{ width: `${progressPct}%`, backgroundColor: selectedTheme.accent, opacity: 0.55 }}
          />
        </div>
      </div>

      {auditState === "loading" && (
        <div className="shrink-0 border-b border-orbflow-border/60 bg-orbflow-surface/30 px-5 py-3 text-sm text-orbflow-text-faint lg:px-8">
          <div className="flex items-center gap-2">
            <NodeIcon name="loader" className="h-3.5 w-3.5 animate-spin" />
            Verifying audit trail...
          </div>
        </div>
      )}

      {auditState === "done" && auditResult?.valid && (
        <div
          className="shrink-0 border-b px-5 py-3 text-sm lg:px-8"
          style={{
            color: "var(--orbflow-exec-completed)",
            borderColor: "color-mix(in srgb, var(--orbflow-exec-completed) 20%, transparent)",
            backgroundColor: "color-mix(in srgb, var(--orbflow-exec-completed) 8%, transparent)",
          }}
        >
          <div className="flex items-center gap-2">
            <NodeIcon name="check" className="h-3.5 w-3.5" />
            Audit verified. {auditResult.event_count} events confirmed.
          </div>
        </div>
      )}

      {auditState === "done" && (auditError || (auditResult && !auditResult.valid)) && (
        <div
          className="shrink-0 border-b px-5 py-3 text-sm lg:px-8"
          style={{
            color: "var(--orbflow-exec-failed)",
            borderColor: "color-mix(in srgb, var(--orbflow-exec-failed) 20%, transparent)",
            backgroundColor: "color-mix(in srgb, var(--orbflow-exec-failed) 8%, transparent)",
          }}
        >
          <div className="flex items-center gap-2">
            <NodeIcon name="alert-triangle" className="h-3.5 w-3.5" />
            {auditError || auditResult?.error || "Invalid audit trail"}
          </div>
        </div>
      )}

      {onApprove && onReject && waitingNodes.length > 0 && (
        <div className="shrink-0 border-b border-amber-500/20 bg-amber-500/[0.05] px-5 py-4 lg:px-8">
          <div
            className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.18em]"
            style={{ color: "var(--orbflow-exec-cancelled)" }}
          >
            <NodeIcon name="shield" className="h-3.5 w-3.5" />
            Approval checkpoints
          </div>
          <div className="space-y-2">
            {waitingNodes.map((waitingNode) => (
              <ApprovalBannerRow
                key={waitingNode.nodeId}
                instanceId={selectedInstance.id}
                nodeId={waitingNode.nodeId}
                nodeName={waitingNode.name}
                onApprove={onApprove}
                onReject={onReject}
                onDetails={() => setSelectedNodeId(waitingNode.nodeId)}
              />
            ))}
          </div>
        </div>
      )}

      {showAuditTrail && (
        <div className="flex-1 min-h-0 overflow-y-auto px-5 py-5 lg:px-8 lg:py-6">
          <div className="activity-surface h-full overflow-hidden rounded-[30px] border border-orbflow-border/70">
            <AuditTrailPanel
              instanceId={selectedInstance.id}
              auditResult={auditResult}
              auditError={auditError}
              auditState={auditState}
              onVerify={handleVerifyAudit}
              onClose={() => setShowAuditTrail(false)}
            />
          </div>
        </div>
      )}

      <div className={cn("relative flex-1 min-h-0 overflow-y-auto custom-scrollbar", showAuditTrail && "hidden")}>
        <div className={cn("min-h-full p-4 lg:p-5", viewMode === "graph" && "flex h-full min-h-0 flex-col")}>
          <div className={cn("activity-surface overflow-hidden rounded-[30px] border border-orbflow-border/70", viewMode === "graph" && "flex min-h-[30rem] flex-1 flex-col")}>
            <div className="border-b border-orbflow-border/60 px-4 py-2.5 lg:px-5">
              <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                <div>
                  <h3 className="text-sm font-semibold tracking-tight text-orbflow-text-secondary">
                    {activeView.label}
                  </h3>
                  <p className="mt-1 text-xs text-orbflow-text-faint">
                    {activeView.description}
                  </p>
                </div>

                <div className="flex flex-wrap items-center gap-2 text-xs text-orbflow-text-faint">
                  <span className="rounded-full border border-orbflow-border/60 bg-orbflow-bg/60 px-3 py-1.5">
                    Click a node to inspect details
                  </span>
                  {selectedNodeId && (
                    <span className="rounded-full border border-electric-indigo/20 bg-electric-indigo/10 px-3 py-1.5 text-electric-indigo">
                      Inspecting {selectedNodeId}
                    </span>
                  )}
                </div>
              </div>
            </div>

            <div className={cn(viewMode === "graph" ? "flex-1 min-h-[30rem]" : "p-4 lg:p-5")}>
              {executionWorkflow ? (
                viewMode === "graph" ? (
                  <ExecutionFlowGraph
                    workflow={executionWorkflow}
                    instance={selectedInstance}
                    onNodeClick={setSelectedNodeId}
                    autoFocus={isActive && viewMode === "graph"}
                    hideProgress
                    className="h-full border-0 rounded-none"
                  />
                ) : viewMode === "timeline" ? (
                  <ExecutionGraph
                    workflow={executionWorkflow}
                    nodeStates={selectedInstance.node_states || {}}
                    onNodeClick={setSelectedNodeId}
                  />
                ) : (
                  <DurationTimeline
                    workflow={executionWorkflow}
                    instance={selectedInstance}
                    onNodeClick={setSelectedNodeId}
                  />
                )
              ) : executionWorkflowError ? (
                <div className="flex flex-col items-center justify-center gap-3 rounded-[24px] border border-orbflow-border/60 bg-orbflow-surface/30 p-12">
                  <NodeIcon name="alert-triangle" className="h-6 w-6 text-orbflow-text-ghost" />
                  <p className="text-base font-medium text-orbflow-text-faint">Workflow unavailable</p>
                  <p className="max-w-[280px] text-center text-sm text-orbflow-text-ghost">
                    The workflow definition could not be loaded. It may have been deleted.
                  </p>
                  <ActionButton icon="repeat" label="Retry loading workflow" onClick={onRetryWorkflowFetch} />
                </div>
              ) : (
                <div className="flex items-center justify-center rounded-[24px] border border-orbflow-border/60 bg-orbflow-surface/30 p-12">
                  <div className="flex items-center gap-2.5 text-sm text-orbflow-text-faint">
                    <NodeIcon name="loader" className="h-4 w-4 animate-spin" />
                    Loading graph...
                  </div>
                </div>
              )}
            </div>
          </div>

          {Object.values(selectedInstance.node_states || {})
            .filter((state) => !workflowNodeMap.has(state.node_id))
            .map((state) => {
              const theme = STATUS_THEMES[state.status] || FALLBACK_THEME;
              const outputStr = state.output ? formatOutput(state.output) : "";

              return (
                <div key={state.node_id} className="activity-surface mt-4 rounded-[28px] border border-orbflow-border/70 p-5">
                  <div className="mb-3 flex items-center justify-between">
                    <span className="text-base font-semibold text-orbflow-text-secondary">
                      {state.node_id}
                    </span>
                    <span className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.16em]" style={{ color: theme.text }}>
                      <NodeIcon name={theme.icon} className="h-3 w-3" />
                      {state.status}
                    </span>
                  </div>

                  {state.error && (
                    <div className="mb-3 rounded-xl border border-rose-500/10 bg-rose-500/5 p-3">
                      <p className="text-sm font-mono text-rose-300/90">{state.error}</p>
                    </div>
                  )}

                  {state.output && (
                    <div>
                      <div className="mb-2 flex items-center justify-between">
                        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-orbflow-text-faint">
                          Output
                        </span>
                        <CopyButton text={outputStr} />
                      </div>
                      <pre className="overflow-x-auto rounded-xl border border-orbflow-border/40 bg-orbflow-surface/30 p-4 text-sm font-mono leading-relaxed text-orbflow-text-faint">
                        {outputStr}
                      </pre>
                    </div>
                  )}
                </div>
              );
            })}
        </div>

        {selectedNodeId && selectedInstance.node_states?.[selectedNodeId] && executionWorkflow && (() => {
          const workflowNode = workflowNodeMap.get(selectedNodeId) || {
            id: selectedNodeId,
            name: selectedNodeId,
            plugin_ref: "unknown",
            type: "task",
            position: { x: 0, y: 0 },
          };
          const nodeSchema = registry.get(workflowNode.plugin_ref);
          const nodeState = selectedInstance.node_states[selectedNodeId];

          if (nodeSchema) {
            return (
              <>
                <ReadOnlyNodeConfig
                  nodeId={selectedNodeId}
                  nodeState={nodeState}
                  workflowNode={workflowNode}
                  schema={nodeSchema}
                  onClose={() => setSelectedNodeId(null)}
                />

                {nodeState.status === "waiting_approval" && onApprove && onReject && (
                  <div
                    className="fixed bottom-8 left-1/2 z-[85] w-[480px] max-w-[calc(100vw-2rem)] -translate-x-1/2 rounded-2xl border border-amber-500/20 bg-orbflow-surface p-5 shadow-2xl"
                    style={{ animation: "modalSlideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1) both" }}
                  >
                    <ApprovalGate
                      instanceId={selectedInstance.id}
                      nodeId={selectedNodeId}
                      nodeName={workflowNode.name || selectedNodeId}
                      onApprove={onApprove}
                      onReject={onReject}
                    />
                  </div>
                )}
              </>
            );
          }

          return (
            <NodeDetailDrawer
              nodeId={selectedNodeId}
              nodeState={nodeState}
              workflowNode={workflowNode}
              instance={selectedInstance}
              onClose={() => setSelectedNodeId(null)}
              onApprove={onApprove}
              onReject={onReject}
            />
          );
        })()}
      </div>
    </>
  );
}
