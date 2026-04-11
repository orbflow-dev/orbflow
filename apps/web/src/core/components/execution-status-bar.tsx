"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import { useExecutionOverlayStore } from "@orbflow/core/stores";
import { NodeIcon } from "./icons";
import { cn } from "@/lib/cn";
import { useOrbflow } from "../context/orbflow-provider";
import { STATUS_COLORS as SEGMENT_COLORS, SEGMENT_ORDER } from "@orbflow/core/execution";

const STATUS_DOT_STYLES: Record<string, { dot: string; text: string }> = {
  running:   { dot: "var(--orbflow-exec-active)", text: "text-cyan-400" },
  completed: { dot: "var(--orbflow-exec-completed)", text: "text-emerald-400" },
  failed:    { dot: "var(--orbflow-exec-failed)", text: "text-red-400" },
  cancelled: { dot: "var(--orbflow-exec-cancelled)", text: "text-amber-400" },
};


function truncate(text: string, max: number): string {
  return text.length > max ? text.slice(0, max) + "\u2026" : text;
}

function capitalize(text: string): string {
  if (!text) return "";
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function ExecutionStatusBar() {
  const { config } = useOrbflow();
  const isLive = useExecutionOverlayStore((s) => s.isLive);
  const instanceStatus = useExecutionOverlayStore((s) => s.instanceStatus);
  const workflowName = useExecutionOverlayStore((s) => s.workflowName);
  const progress = useExecutionOverlayStore((s) => s.progress);

  const [visible, setVisible] = useState(false);
  const [dismissing, setDismissing] = useState(false);
  const autoDismissRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Show bar when isLive becomes true
  useEffect(() => {
    if (isLive) {
      setVisible(true);
      setDismissing(false);
    }
  }, [isLive]);

  // Auto-dismiss 5s after terminal state
  const isTerminal =
    instanceStatus === "completed" ||
    instanceStatus === "failed" ||
    instanceStatus === "cancelled";

  const dismissWithAnimation = useCallback(() => {
    if (autoDismissRef.current) clearTimeout(autoDismissRef.current);
    setDismissing(true);
    setTimeout(() => {
      useExecutionOverlayStore.getState().stopLiveRun();
      setVisible(false);
    }, 300);
  }, []);

  useEffect(() => {
    if (isTerminal && isLive) {
      autoDismissRef.current = setTimeout(dismissWithAnimation, 5000);
    }
    return () => {
      if (autoDismissRef.current) clearTimeout(autoDismissRef.current);
    };
  }, [isTerminal, isLive, dismissWithAnimation]);

  // Also dismiss when polling calls stopLiveRun() while bar is still visible
  useEffect(() => {
    if (visible && !isLive && isTerminal && !dismissing) {
      autoDismissRef.current = setTimeout(dismissWithAnimation, 3000);
      return () => {
        if (autoDismissRef.current) clearTimeout(autoDismissRef.current);
      };
    }
  }, [visible, isLive, isTerminal, dismissing, dismissWithAnimation]);

  const handleCancel = useCallback(async () => {
    const id = useExecutionOverlayStore.getState().activeInstanceId;
    if (!id) return;
    try {
      await fetch(`${config.apiBaseUrl}/instances/${id}/cancel`, { method: "POST" });
    } catch {
      // Polling will pick up the status change if cancel endpoint is unreachable
    }
  }, [config.apiBaseUrl]);

  if (!visible) return null;

  const status = instanceStatus ?? "running";
  const colors = STATUS_DOT_STYLES[status] ?? STATUS_DOT_STYLES.running;
  const isRunning = status === "running";
  const completedCount = progress.completed + progress.failed;
  const totalCount = progress.total;
  const percentage =
    totalCount > 0 ? Math.round((completedCount / totalCount) * 100) : 0;

  // Build progress bar segments
  const segments = SEGMENT_ORDER.map((key) => {
    const count = progress[key as keyof typeof progress] as number;
    const pct = totalCount > 0 ? (count / totalCount) * 100 : 0;
    return { key, pct, color: SEGMENT_COLORS[key] };
  }).filter((seg) => seg.pct > 0);

  return (
    <div
      className={cn(
        "absolute top-3 left-1/2 z-50",
        "backdrop-blur-2xl bg-orbflow-bg/85",
        "border border-orbflow-border/40 rounded-2xl",
        "shadow-[0_8px_32px_rgba(0,0,0,0.4)]",
        "px-4 py-2.5 min-w-[320px] max-w-[480px]",
      )}
      style={{
        transform: dismissing
          ? "translateX(-50%) translateY(-120%) scale(0.95)"
          : "translateX(-50%) translateY(0) scale(1)",
        opacity: dismissing ? 0 : 1,
        transition: "transform 0.35s cubic-bezier(0.4, 0, 0.2, 1), opacity 0.3s ease",
      }}
    >
      {/* Single row: compact */}
      <div className="flex items-center gap-2">
        {/* Status dot */}
        <span
          className="relative inline-block w-2 h-2 rounded-full shrink-0"
          style={{ backgroundColor: colors.dot }}
        >
          {isRunning && (
            <span
              className="absolute inset-0 rounded-full animate-ping"
              style={{ backgroundColor: colors.dot, opacity: 0.35 }}
            />
          )}
        </span>

        {/* Status text */}
        <span className={cn("text-xs font-semibold", colors.text)}>
          {capitalize(status)}
        </span>

        {/* Separator */}
        <span className="w-px h-3 bg-orbflow-border/40" />

        {/* Workflow name */}
        <span className="text-xs text-orbflow-text-ghost truncate max-w-[120px]">
          {truncate(workflowName, 18)}
        </span>

        {/* Progress pill */}
        <span className="text-[10px] font-mono text-orbflow-text-secondary tabular-nums bg-orbflow-surface/60 rounded-md px-1.5 py-0.5 whitespace-nowrap">
          {progress.completed}/{totalCount}
        </span>

        {/* Spacer */}
        <span className="flex-1" />

        {/* Cancel button (only when running) */}
        {isRunning && (
          <button
            type="button"
            onClick={handleCancel}
            className={cn(
              "inline-flex items-center gap-1 px-2 py-1 rounded-lg",
              "text-[10px] font-medium text-orbflow-text-ghost",
              "hover:bg-red-500/10 hover:text-red-400",
              "transition-colors cursor-pointer",
            )}
            aria-label="Cancel workflow run"
            title="Cancel run"
          >
            <NodeIcon name="x" className="w-2.5 h-2.5" />
            <span>Cancel</span>
          </button>
        )}

        {/* Dismiss button (terminal states) */}
        {isTerminal && (
          <button
            type="button"
            onClick={dismissWithAnimation}
            className={cn(
              "inline-flex items-center justify-center w-5 h-5 rounded-md",
              "text-orbflow-text-ghost",
              "hover:bg-orbflow-border/30 hover:text-orbflow-text-secondary",
              "transition-colors cursor-pointer",
            )}
            aria-label="Dismiss"
            title="Dismiss"
          >
            <NodeIcon name="x" className="w-3 h-3" />
          </button>
        )}
      </div>

      {/* Progress bar -- slim, below content */}
      <div className="mt-2 flex items-center gap-2">
        <div className="flex-1 h-[2px] rounded-full bg-orbflow-border/30 overflow-hidden flex">
          {segments.map((seg) => (
            <div
              key={seg.key}
              className={cn(
                "h-full transition-all duration-500 ease-out",
                seg.key === "running" && "animate-pulse",
              )}
              style={{
                width: `${seg.pct}%`,
                backgroundColor: seg.color,
              }}
            />
          ))}
        </div>
        <span className="text-[10px] font-mono text-orbflow-text-ghost tabular-nums w-7 text-right">
          {percentage}%
        </span>
      </div>
    </div>
  );
}

export { ExecutionStatusBar };
