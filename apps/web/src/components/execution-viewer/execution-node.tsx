"use client";

import { memo, useMemo, useCallback, useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { useOrbflow } from "@/core/context/orbflow-provider";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { NODE_SIZES, STATUS_BADGE, formatDurationMs, STATUS_LABELS } from "@/lib/execution";

// -- Types ---------------------------------------
export interface ExecutionNodeData {
  pluginRef: string;
  label: string;
  kind: "trigger" | "action" | "capability";
  executionStatus?: string;
  error?: string;
  duration?: number;
  attempt?: number;
  hasOutput?: boolean;
  outputPreview?: string;
  onNodeClick?: (nodeId: string) => void;
  /** Hide status badges and execution tooltips (used in diff views). */
  hideBadge?: boolean;
  [key: string]: unknown;
}

// -- Hover tooltip ------------------------------
function NodeTooltip({
  name,
  status,
  duration,
  error,
  outputPreview,
}: {
  name: string;
  status?: string;
  duration?: number;
  error?: string;
  outputPreview?: string;
}) {
  return (
    <div
      className={cn(
        "absolute z-30 left-1/2 -translate-x-1/2 pointer-events-none",
        "rounded-lg border px-3 py-2 shadow-xl min-w-[160px] max-w-[260px]",
        "border-orbflow-border bg-orbflow-surface animate-fade-in",
      )}
      style={{ bottom: "calc(100% + 10px)" }}
    >
      <p className="text-body font-semibold text-orbflow-text-secondary truncate mb-1">
        {name}
      </p>
      <div className="flex items-center gap-2 text-body-sm">
        {status && (
          <span
            className="font-medium capitalize"
            style={{ color: `var(--exec-text-${status})` }}
          >
            {STATUS_LABELS[status] || status}
          </span>
        )}
        {duration != null && (
          <span className="text-orbflow-text-faint font-mono tabular-nums">
            {formatDurationMs(duration)}
          </span>
        )}
      </div>
      {status === "failed" && error && (
        <p className="mt-1.5 text-body-sm text-rose-400/80 font-mono leading-tight line-clamp-3 break-words">
          {error.length > 100 ? error.slice(0, 100) + "..." : error}
        </p>
      )}
      {status === "completed" && outputPreview && (
        <p className="mt-1.5 text-body-sm text-orbflow-text-faint font-mono leading-tight line-clamp-3 break-words">
          {outputPreview.length > 100
            ? outputPreview.slice(0, 100) + "..."
            : outputPreview}
        </p>
      )}
    </div>
  );
}

// -- Status badge component ----------------------
function StatusBadge({ status }: { status: string | undefined }) {
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

// -- Main component ------------------------------
function ExecutionNodeInner({ id, data }: NodeProps) {
  const { registry } = useOrbflow();

  const nodeData = data as unknown as ExecutionNodeData;
  const {
    pluginRef,
    label,
    kind,
    executionStatus,
    error,
    duration,
    outputPreview,
    onNodeClick,
    hideBadge,
  } = nodeData;

  const schema = useMemo(() => registry.get(pluginRef), [registry, pluginRef]);

  const displayName = schema?.name || label || id;
  const nodeColor = schema?.color || "#7C5CFC";
  const statusClass = executionStatus
    ? `exec-status-${executionStatus}`
    : undefined;

  const [hovered, setHovered] = useState(false);

  const handleClick = useCallback(() => {
    onNodeClick?.(id);
  }, [onNodeClick, id]);

  const handleMouseEnter = useCallback(() => setHovered(true), []);
  const handleMouseLeave = useCallback(() => setHovered(false), []);

  // -- Trigger Node ----------------------------
  if (kind === "trigger") {
    return (
      <div
        className="orbflow-node orbflow-node--trigger"
        style={{ width: NODE_SIZES.trigger, height: NODE_SIZES.trigger }}
        onClick={handleClick}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        {hovered && (
          <NodeTooltip
            name={displayName}
            status={hideBadge ? undefined : executionStatus}
            duration={hideBadge ? undefined : duration}
            error={hideBadge ? undefined : error}
            outputPreview={hideBadge ? undefined : outputPreview}
          />
        )}
        {/* Status wrapper */}
        <div className={cn("absolute inset-0 rounded-[20px]", statusClass)}>
          {/* Background box */}
          <div
            className="absolute inset-0 rounded-[20px] transition-all duration-200 border-[1.5px] bg-orbflow-node-bg border-orbflow-node-border"
            style={{
              boxShadow: `0 0 12px ${nodeColor}08`,
              background: `radial-gradient(circle at center, ${nodeColor}06 0%, transparent 70%)`,
            }}
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

          {/* Status badge (top-right) */}
          {!hideBadge && <StatusBadge status={executionStatus} />}

          {/* Error tooltip */}
          {executionStatus === "failed" && error && (
            <div className="exec-error-tooltip">{error}</div>
          )}
        </div>

        {/* Output handle */}
        <Handle
          type="source"
          position={Position.Right}
          id="out"
          className="orbflow-handle orbflow-handle--out"
          isConnectable={false}
        />

        {/* Duration label */}
        {duration != null && executionStatus !== "pending" && (
          <div
            className="exec-duration"
            style={{ top: NODE_SIZES.trigger + 4 }}
          >
            {formatDurationMs(duration)}
          </div>
        )}

        {/* Name below node */}
        <div
          className="orbflow-node-label"
          style={{ top: NODE_SIZES.trigger + (duration != null && executionStatus !== "pending" ? 20 : 8) }}
        >
          <span className="orbflow-node-label__name">{displayName}</span>
        </div>
      </div>
    );
  }

  // -- Capability Node (circular) --------------
  if (kind === "capability") {
    return (
      <div
        className="orbflow-node orbflow-node--capability"
        style={{ width: NODE_SIZES.capability, height: NODE_SIZES.capability }}
        onClick={handleClick}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        {hovered && (
          <NodeTooltip
            name={displayName}
            status={hideBadge ? undefined : executionStatus}
            duration={hideBadge ? undefined : duration}
            error={hideBadge ? undefined : error}
            outputPreview={hideBadge ? undefined : outputPreview}
          />
        )}
        {/* Status wrapper */}
        <div className={cn("absolute inset-0 rounded-full", statusClass)}>
          {/* Circular background */}
          <div
            className="absolute inset-0 rounded-full border transition-all duration-200 bg-orbflow-node-bg border-orbflow-node-border"
            style={{
              boxShadow: `0 0 12px ${nodeColor}08`,
              background: `radial-gradient(circle at center, ${nodeColor}06 0%, transparent 70%)`,
            }}
          />

          {/* Icon */}
          <div className="absolute inset-0 flex items-center justify-center">
            <NodeIcon
              name={schema?.icon || "database"}
              className="w-6 h-6"
              style={{ color: schema?.color || "var(--orbflow-text-secondary)" }}
            />
          </div>

          {/* Status badge (top-right) */}
          {!hideBadge && <StatusBadge status={executionStatus} />}

          {/* Error tooltip */}
          {executionStatus === "failed" && error && (
            <div className="exec-error-tooltip">{error}</div>
          )}
        </div>

        {/* Top handle */}
        <Handle
          type="target"
          position={Position.Top}
          id="in"
          className="orbflow-handle orbflow-handle--cap-in"
          isConnectable={false}
        />

        {/* Duration label */}
        {duration != null && executionStatus !== "pending" && (
          <div
            className="exec-duration"
            style={{ top: NODE_SIZES.capability + 4 }}
          >
            {formatDurationMs(duration)}
          </div>
        )}

        {/* Name below node */}
        <div
          className="orbflow-node-label"
          style={{ top: NODE_SIZES.capability + (duration != null && executionStatus !== "pending" ? 20 : 8) }}
        >
          <span className="orbflow-node-label__name orbflow-node-label__name--cap">
            {displayName}
          </span>
        </div>
      </div>
    );
  }

  // -- Action Node (rounded square) ------------
  return (
    <div
      className="orbflow-node orbflow-node--action"
      style={{ width: NODE_SIZES.action, height: NODE_SIZES.action }}
      onClick={handleClick}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {hovered && (
        <NodeTooltip
          name={displayName}
          status={hideBadge ? undefined : executionStatus}
          duration={hideBadge ? undefined : duration}
          error={hideBadge ? undefined : error}
          outputPreview={hideBadge ? undefined : outputPreview}
        />
      )}
      {/* Status wrapper */}
      <div className={cn("absolute inset-0 rounded-2xl", statusClass)}>
        {/* Box background */}
        <div
          className="absolute inset-0 rounded-2xl border transition-all duration-200 bg-orbflow-node-bg border-orbflow-node-border"
          style={{
            boxShadow: `0 0 12px ${nodeColor}08`,
            background: `radial-gradient(circle at center, ${nodeColor}06 0%, transparent 70%)`,
          }}
        />

        {/* Icon */}
        <div className="absolute inset-0 flex items-center justify-center">
          <NodeIcon
            name={schema?.icon || "default"}
            className="w-7 h-7"
            style={{ color: schema?.color || "var(--orbflow-text-secondary)" }}
          />
        </div>

        {/* Status badge (top-right) */}
        {!hideBadge && <StatusBadge status={executionStatus} />}

        {/* Error tooltip */}
        {executionStatus === "failed" && error && (
          <div className="exec-error-tooltip">{error}</div>
        )}
      </div>

      {/* Input handle */}
      <Handle
        type="target"
        position={Position.Left}
        id="in"
        className="orbflow-handle orbflow-handle--in"
        isConnectable={false}
      />

      {/* Output handle */}
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        className="orbflow-handle orbflow-handle--out"
        isConnectable={false}
      />

      {/* Duration label */}
      {duration != null && executionStatus !== "pending" && (
        <div
          className="exec-duration"
          style={{ top: NODE_SIZES.action + 4 }}
        >
          {formatDurationMs(duration)}
        </div>
      )}

      {/* Name below node */}
      <div
        className="orbflow-node-label"
        style={{ top: NODE_SIZES.action + (duration != null && executionStatus !== "pending" ? 20 : 8) }}
      >
        <span className="orbflow-node-label__name">{displayName}</span>
      </div>
    </div>
  );
}

export const ExecutionNode = memo(ExecutionNodeInner);
