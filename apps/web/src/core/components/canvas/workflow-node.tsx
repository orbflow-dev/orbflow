"use client";

import { memo, useMemo, useCallback } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { useOrbflow } from "../../context/orbflow-provider";
import { NodeIcon } from "../icons";
import { useCanvasStore, usePickerStore, useExecutionOverlayStore, useWorkflowStore } from "@orbflow/core/stores";
import { useNodeOutputCacheStore } from "@orbflow/core/stores";
import { cn } from "../../utils/cn";
import { NODE_SIZES, STATUS_BADGE, formatDurationMs } from "@orbflow/core/execution";
import type { NodeKind } from "../../types/schema";

/** Calculate the vertical offset for the name label below a node */
function getNameLabelOffset(opts: {
  nodeSize: number;
  hasCapPorts: boolean;
  hasDuration: boolean;
}): number {
  const { nodeSize, hasCapPorts, hasDuration } = opts;
  if (hasCapPorts) {
    return nodeSize + (hasDuration ? 32 : 20);
  }
  return nodeSize + (hasDuration ? 20 : 8);
}

// Border colors for nodes parented to sticky notes (visual grouping cue)
const STICKY_BORDERS: Record<string, string> = {
  yellow: "#FDE047", blue: "#93C5FD", green: "#86EFAC", pink: "#F9A8D4", purple: "#C4B5FD",
};

function ExecStatusBadge({ status }: { status: string | undefined }) {
  if (!status) return null;
  const badge = STATUS_BADGE[status];
  if (!badge) return null;
  return (
    <div className={cn("exec-badge", badge.cssModifier)}>
      <NodeIcon
        name={badge.icon}
        className={cn("w-2.5 h-2.5", badge.spin && "animate-spin")}
      />
    </div>
  );
}

function WorkflowNodeInner({ id, data, selected }: NodeProps) {
  const { registry } = useOrbflow();
  const { openPicker } = usePickerStore();

  // Color-tinted left border when parented to a sticky note
  const parentStickyColor = useCanvasStore((s) => {
    const self = s.nodes.find((n) => n.id === id);
    if (!self?.parentId) return null;
    const parent = s.nodes.find((n) => n.id === self.parentId);
    if (!parent || parent.type !== "stickyNote") return null;
    return (parent.data?.color as string) || "yellow";
  });

  const pluginRef = (data?.pluginRef as string) || "";
  const label = (data?.label as string) || "";

  const handleAddClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      e.stopPropagation();
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      openPicker({ x: rect.right + 8, y: rect.top }, id);
    },
    [openPicker, id]
  );

  // -- Execution overlay state --------------
  const execStatus = useExecutionOverlayStore(
    (s) => s.isLive ? s.nodeStatuses[id]?.status : undefined
  );
  const execTransition = useExecutionOverlayStore(
    (s) => s.isLive ? s.transitions[id] : undefined
  );
  const clearTransition = useExecutionOverlayStore((s) => s.clearTransition);

  // Duration for completed/failed nodes during live execution
  const execDuration = useExecutionOverlayStore((s) => {
    if (!s.isLive) return undefined;
    const ns = s.nodeStatuses[id];
    if (!ns?.startedAt || !ns?.endedAt) return undefined;
    if (ns.status !== "completed" && ns.status !== "failed") return undefined;
    return Math.max(0, new Date(ns.endedAt).getTime() - new Date(ns.startedAt).getTime());
  });

  // Check if this node has cached output (from test runs or prior executions)
  // Both hooks must be called unconditionally to satisfy Rules of Hooks
  const hasOverlayOutput = useExecutionOverlayStore(
    (s) => !s.isLive && !!s.nodeStatuses[id]?.output
  );
  const currentWorkflowId = useWorkflowStore((s) => s.selectedWorkflow?.id);
  const hasPersistentCache = useNodeOutputCacheStore(
    (s) => currentWorkflowId ? !!(s.cache[currentWorkflowId]?.[id]) : false
  );
  const hasCachedOutput = hasOverlayOutput || hasPersistentCache;

  const execStatusClass = execStatus ? `exec-status-${execStatus}` : "";
  const execFlashClass = execTransition === "completed" ? "exec-status-completed--flash" : "";
  const execShakeClass = execTransition === "failed" ? "exec-status-failed--shake" : "";

  const handleAnimationEnd = useCallback(() => {
    if (execTransition) {
      clearTransition(id);
    }
  }, [execTransition, clearTransition, id]);

  const suppressInlineShadow = !!execStatus;

  const schema = useMemo(() => registry.get(pluginRef), [registry, pluginRef]);

  const nodeKind: NodeKind = (schema?.nodeKind as NodeKind) || "action";
  const isTrigger = nodeKind === "trigger";
  const isCapability = nodeKind === "capability";
  const hasInputs = !isTrigger && !isCapability && (schema?.inputs?.length ?? 0) > 0;
  const hasOutputs = (schema?.outputs?.length ?? 0) > 0;
  const capabilityPorts = schema?.capabilityPorts || [];

  const displayName = schema?.name || label || id;
  // Show custom name only if user renamed it from the default
  const customName = label && label !== displayName ? label : "";
  const nodeColor = schema?.color || "#7C5CFC";

  // -- Trigger Node --------------------------
  if (isTrigger) {
    return (
      <div
        className="orbflow-node orbflow-node--trigger"
        style={{ width: NODE_SIZES.trigger, height: NODE_SIZES.trigger }}
      >
        {/* Background box */}
        <div
          className={cn(
            "absolute inset-0 rounded-[20px] transition-all duration-200 border-[1.5px]",
            selected
              ? "bg-orbflow-node-bg-hover border-orbflow-node-border-selected"
              : "bg-orbflow-node-bg border-orbflow-node-border",
            execStatusClass,
            execFlashClass,
            execShakeClass
          )}
          style={{
            boxShadow: suppressInlineShadow
              ? undefined
              : selected
                ? `0 0 24px ${nodeColor}1A, 0 0 8px ${nodeColor}10`
                : `0 0 12px ${nodeColor}18`,
            background: (selected || suppressInlineShadow) ? undefined : `radial-gradient(circle at center, ${nodeColor}14 0%, transparent 70%)`,
            ...(parentStickyColor ? { borderLeftWidth: 3, borderLeftColor: STICKY_BORDERS[parentStickyColor] } : {}),
          }}
          onAnimationEnd={handleAnimationEnd}
        />

        {/* Lightning badge */}
        <div
          className="absolute -top-2 -left-2 z-10 w-[18px] h-[18px] rounded-full flex items-center justify-center shadow-md"
          style={{ backgroundColor: "var(--orbflow-trigger-badge)" }}
        >
          <NodeIcon name="zap" className="w-[10px] h-[10px] text-white" />
        </div>

        {/* Icon */}
        <div className="absolute inset-0 flex items-center justify-center">
          <NodeIcon
            name={schema?.icon || "zap"}
            className="w-8 h-8"
            style={{ color: schema?.color || "var(--orbflow-text-secondary)" }}
          />
        </div>

        {/* Execution status badge */}
        <ExecStatusBadge status={execStatus} />

        {/* Output handle */}
        {hasOutputs && (
          <Handle
            type="source"
            position={Position.Right}
            id="out"
            className="orbflow-handle orbflow-handle--out"
          />
        )}

        {/* "+" add button on hover */}
        <div className="orbflow-add-btn nodrag nopan" onClick={handleAddClick} title="Add next step">
          <NodeIcon name="plus" className="w-2.5 h-2.5" />
        </div>

        {/* Execution duration label */}
        {execDuration != null && (
          <div className="exec-duration" style={{ top: NODE_SIZES.trigger + 4 }}>
            {formatDurationMs(execDuration)}
          </div>
        )}

        {/* Name below node */}
        <div className="orbflow-node-label" style={{ top: getNameLabelOffset({ nodeSize: NODE_SIZES.trigger, hasCapPorts: false, hasDuration: execDuration != null }) }}>
          <span className="orbflow-node-label__name" title={displayName}>{displayName}</span>
          {customName && (
            <span className="orbflow-node-label__alias">{customName}</span>
          )}
        </div>
      </div>
    );
  }

  // -- Capability Node (circular) ------------
  if (isCapability) {
    return (
      <div
        className="orbflow-node orbflow-node--capability"
        style={{ width: NODE_SIZES.capability, height: NODE_SIZES.capability }}
      >
        {/* Circular background */}
        <div
          className={cn(
            "absolute inset-0 rounded-full border transition-all duration-200",
            selected
              ? "bg-orbflow-node-bg-hover"
              : "bg-orbflow-node-bg border-orbflow-node-border",
            execStatusClass,
            execFlashClass,
            execShakeClass
          )}
          style={{
            borderColor: selected ? `${nodeColor}4D` : undefined,
            boxShadow: suppressInlineShadow
              ? undefined
              : selected
                ? `0 0 20px ${nodeColor}1F, 0 0 8px ${nodeColor}10`
                : `0 0 12px ${nodeColor}18`,
            background: (selected || suppressInlineShadow) ? undefined : `radial-gradient(circle at center, ${nodeColor}14 0%, transparent 70%)`,
            ...(parentStickyColor ? { borderLeftWidth: 3, borderLeftColor: STICKY_BORDERS[parentStickyColor] } : {}),
          }}
          onAnimationEnd={handleAnimationEnd}
        />

        {/* Icon */}
        <div className="absolute inset-0 flex items-center justify-center">
          <NodeIcon
            name={schema?.icon || "database"}
            className="w-6 h-6"
            style={{ color: schema?.color || "var(--orbflow-text-secondary)" }}
          />
        </div>

        {/* Execution status badge */}
        <ExecStatusBadge status={execStatus} />

        {/* Top handle (for cap-edge connections from action nodes) */}
        <Handle
          type="target"
          position={Position.Top}
          id="in"
          className="orbflow-handle orbflow-handle--cap-in"
        />

        {/* Execution duration label */}
        {execDuration != null && (
          <div className="exec-duration" style={{ top: NODE_SIZES.capability + 4 }}>
            {formatDurationMs(execDuration)}
          </div>
        )}

        {/* Name below node */}
        <div className="orbflow-node-label" style={{ top: NODE_SIZES.capability + (execDuration != null ? 20 : 8) }}>
          <span className="orbflow-node-label__name orbflow-node-label__name--cap">
            {displayName}
          </span>
          {customName && (
            <span className="orbflow-node-label__alias">{customName}</span>
          )}
        </div>
      </div>
    );
  }

  // -- Action Node (rounded square) ----------
  return (
    <div
      className="orbflow-node orbflow-node--action"
      style={{ width: NODE_SIZES.action, height: NODE_SIZES.action }}
    >
      {/* Box background */}
      <div
        className={cn(
          "absolute inset-0 rounded-2xl border transition-all duration-200",
          selected
            ? "bg-orbflow-node-bg-hover border-orbflow-node-border-selected"
            : "bg-orbflow-node-bg border-orbflow-node-border",
          execStatusClass,
          execFlashClass,
          execShakeClass
        )}
        style={{
          boxShadow: suppressInlineShadow
            ? undefined
            : selected
              ? `0 0 24px ${nodeColor}1F, 0 0 8px ${nodeColor}10`
              : `0 0 12px ${nodeColor}18`,
          background: (selected || suppressInlineShadow) ? undefined : `radial-gradient(circle at center, ${nodeColor}14 0%, transparent 70%)`,
          ...(parentStickyColor ? { borderLeftWidth: 3, borderLeftColor: STICKY_BORDERS[parentStickyColor] } : {}),
        }}
        onAnimationEnd={handleAnimationEnd}
      />

      {/* Icon */}
      <div className="absolute inset-0 flex items-center justify-center">
        <NodeIcon
          name={schema?.icon || "default"}
          className="w-7 h-7"
          style={{ color: schema?.color || "var(--orbflow-text-secondary)" }}
        />
      </div>

      {/* Execution status badge */}
      <ExecStatusBadge status={execStatus} />

      {/* Cached output indicator -- shows when node has test/run output */}
      {!execStatus && hasCachedOutput && (
        <div className="absolute -bottom-1.5 -left-1.5 z-10 group/cache">
          {/* Ping ring */}
          <div className="absolute inset-0 rounded-full bg-emerald-400/40 animate-ping" style={{ animationDuration: "2.5s" }} />
          {/* Dot */}
          <div className="relative w-[11px] h-[11px] rounded-full bg-emerald-400 border border-emerald-300/50 shadow-[0_0_6px_rgba(52,211,153,0.4)]" />
          {/* Tooltip -- appears below the dot on hover */}
          <div className="absolute left-1/2 -translate-x-1/2 top-full mt-2 opacity-0 scale-95 pointer-events-none
            group-hover/cache:opacity-100 group-hover/cache:scale-100 transition-all duration-150 ease-out origin-top">
            <div className="relative px-2 py-1.5 rounded-md whitespace-nowrap
              bg-[var(--orbflow-surface)] border border-[var(--orbflow-border)]/50 shadow-[0_4px_24px_rgba(0,0,0,0.35)] backdrop-blur-xl">
              {/* Arrow */}
              <div className="absolute left-1/2 -translate-x-1/2 -top-[4px] w-2 h-2 rotate-45
                bg-[var(--orbflow-surface)] border-l border-t border-[var(--orbflow-border)]/50" />
              <div className="flex items-center gap-1">
                <div className="w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0" />
                <span className="text-[10px] font-semibold tracking-wide text-emerald-300">
                  Output cached
                </span>
              </div>
              <p className="text-[9px] text-[var(--orbflow-text-secondary)] mt-0.5 leading-snug">
                From test run -- used by downstream nodes
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Approval gate badge (shield icon when requires_approval is enabled) -- hidden during live execution since status glow already communicates state */}
      {!!(data?.requiresApproval) && !execStatus && (
        <div
          className="absolute -top-2 -right-2 z-10 w-[18px] h-[18px] rounded-full flex items-center justify-center shadow-md bg-amber-500 border border-amber-400/50"
          title="Requires approval before execution"
        >
          <NodeIcon name="shield" className="w-[10px] h-[10px] text-white" />
        </div>
      )}

      {/* Input handle */}
      {hasInputs && (
        <Handle
          type="target"
          position={Position.Left}
          id="in"
          className="orbflow-handle orbflow-handle--in"
        />
      )}

      {/* Output handle */}
      {hasOutputs && (
        <Handle
          type="source"
          position={Position.Right}
          id="out"
          className="orbflow-handle orbflow-handle--out"
        />
      )}

      {/* Capability port handles (bottom) */}
      {capabilityPorts.map((port, i) => (
        <Handle
          key={port.key}
          type="target"
          position={Position.Bottom}
          id={`cap:${port.key}`}
          className="orbflow-handle orbflow-handle--cap"
          style={{
            left: `${((i + 1) / (capabilityPorts.length + 1)) * 100}%`,
          }}
        />
      ))}

      {/* "+" add button on hover */}
      <div className="orbflow-add-btn nodrag nopan" onClick={handleAddClick} title="Add next step">
        <NodeIcon name="plus" className="w-2.5 h-2.5" />
      </div>

      {/* Capability port labels (below node, above name) */}
      {capabilityPorts.length > 0 && (
        <div
          className="absolute left-1/2 -translate-x-1/2 flex gap-3 whitespace-nowrap"
          style={{ top: NODE_SIZES.action + 4 }}
        >
          {capabilityPorts.map((port) => (
            <span
              key={port.key}
              className="text-micro font-medium text-orbflow-text-faint"
            >
              {port.key}
              {port.required && (
                <span className="text-rose-400/40">*</span>
              )}
            </span>
          ))}
        </div>
      )}

      {/* Execution duration label */}
      {execDuration != null && (
        <div className="exec-duration" style={{ top: NODE_SIZES.action + (capabilityPorts.length > 0 ? 16 : 4) }}>
          {formatDurationMs(execDuration)}
        </div>
      )}

      {/* Name below node */}
      <div
        className="orbflow-node-label"
        style={{ top: getNameLabelOffset({ nodeSize: NODE_SIZES.action, hasCapPorts: capabilityPorts.length > 0, hasDuration: execDuration != null }) }}
      >
        <span className="orbflow-node-label__name" title={displayName}>{displayName}</span>
        {customName && (
          <span className="orbflow-node-label__alias">{customName}</span>
        )}
      </div>
    </div>
  );
}

export const WorkflowNode = memo(WorkflowNodeInner);
