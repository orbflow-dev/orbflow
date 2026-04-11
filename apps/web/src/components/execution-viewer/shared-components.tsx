"use client";

import { useState, useCallback, useEffect, useRef, useMemo, memo } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { STATUS_THEMES, FALLBACK_THEME, formatDurationRange } from "@/lib/execution";
import { copyToClipboard } from "@/lib/clipboard";
import { relativeTime } from "./viewer-utils";

import type { Instance } from "@/lib/api";

/* -- LiveDuration ------------------------------------ */

export function LiveDuration({ startTime, endTime }: { startTime: string; endTime?: string }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (endTime) return;
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [endTime]);

  const start = new Date(startTime).getTime();
  const end = endTime ? new Date(endTime).getTime() : now;
  const ms = Math.max(0, end - start);
  const s = Math.floor(ms / 1000);
  let display: string;
  if (s < 60) display = `${s}s`;
  else if (s < 3600) display = `${Math.floor(s / 60)}m ${(s % 60).toString().padStart(2, "0")}s`;
  else display = `${Math.floor(s / 3600)}h ${(Math.floor(s / 60) % 60).toString().padStart(2, "0")}m`;

  return <span className="font-mono text-sm font-semibold tabular-nums tracking-tight text-orbflow-text-faint">{display}</span>;
}

/* -- MiniProgress ------------------------------------ */

const DONE_STATUS_SET = new Set(["completed", "failed", "cancelled"]);

export const MiniProgress = memo(function MiniProgress({ nodeStates, status }: { nodeStates: Record<string, { status: string }> | undefined; status: string }) {
  const { total, done } = useMemo(() => {
    const entries = Object.values(nodeStates || {});
    return { total: entries.length, done: entries.filter((s) => DONE_STATUS_SET.has(s.status)).length };
  }, [nodeStates]);
  if (total === 0) return null;
  const pct = (done / total) * 100;
  const color = status === "failed" ? STATUS_THEMES.failed.accent : status === "running" ? STATUS_THEMES.running.accent : STATUS_THEMES.completed.accent;
  return (
    <div
      role="progressbar"
      aria-valuenow={Math.round(pct)}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-label={`${done} of ${total} nodes settled`}
      className="mt-2 h-[3px] rounded-full overflow-hidden bg-orbflow-surface-hover/40"
    >
      <div className="h-full rounded-full transition-all duration-700 ease-out" style={{ width: `${pct}%`, backgroundColor: color, opacity: 0.45 }} />
    </div>
  );
});

/* -- Re-export SkeletonCard -------------------------- */

export { SkeletonCard } from "@/core/components/skeleton";

/* -- Copy helpers (private hook + exported components) */

function useCopyToClipboard(text: string, resetMs = 2000) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => { if (timerRef.current) clearTimeout(timerRef.current); }, []);

  const copy = useCallback(async () => {
    try {
      await copyToClipboard(text);
      setCopied(true);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setCopied(false), resetMs);
    } catch {
      // Clipboard API unavailable or permission denied — silently no-op
    }
  }, [text, resetMs]);

  return { copied, copy };
}

export function CopyButton({ text }: { text: string }) {
  const { copied, copy } = useCopyToClipboard(text, 2000);

  return (
    <button onClick={copy} aria-label={copied ? "Copied to clipboard" : "Copy to clipboard"}
      className={cn("flex items-center gap-1 px-2 py-1 rounded-md text-caption font-medium transition-all",
        "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
        copied ? "bg-emerald-500/10 text-emerald-400" : "bg-orbflow-add-btn-bg text-orbflow-text-faint hover:text-orbflow-text-muted")}>
      <NodeIcon name={copied ? "check" : "clipboard"} className="w-3 h-3" />
      {copied ? "Copied" : "Copy"}
    </button>
  );
}

export function CopyIdButton({ instanceId }: { instanceId: string }) {
  const { copied, copy } = useCopyToClipboard(instanceId, 1500);

  const handleActivate = useCallback((e: React.SyntheticEvent) => {
    e.stopPropagation();
    copy();
  }, [copy]);

  return (
    <span role="button" tabIndex={0} onClick={handleActivate}
      onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); handleActivate(e); } }}
      className={cn("w-4 h-4 rounded flex items-center justify-center transition-all cursor-pointer",
        "opacity-60 group-hover:opacity-100 focus-visible:opacity-100",
        "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
        copied ? "text-emerald-400" : "text-orbflow-text-ghost hover:text-orbflow-text-muted hover:bg-orbflow-surface-hover")}
      title={copied ? "Copied!" : "Copy instance ID"} aria-label="Copy instance ID">
      <NodeIcon name={copied ? "check" : "clipboard"} className="w-2.5 h-2.5" />
    </span>
  );
}

/* -- InstanceCard ------------------------------------ */

interface InstanceCardProps {
  inst: Instance;
  isSelected: boolean;
  isFocused: boolean;
  workflowName: string;
  trigger?: { icon: string; label: string };
  onSelect: (id: string) => void;
}

export const InstanceCard = memo(function InstanceCard({ inst, isSelected, isFocused, workflowName, trigger, onSelect }: InstanceCardProps) {
  const theme = STATUS_THEMES[inst.status] || FALLBACK_THEME;
  const isRunning = inst.status === "running";
  const isDone = DONE_STATUS_SET.has(inst.status);

  const { progressDone, progressTotal } = useMemo(() => {
    const entries = Object.values(inst.node_states || {});
    return {
      progressTotal: entries.length,
      progressDone: entries.filter((s) => DONE_STATUS_SET.has(s.status)).length,
    };
  }, [inst.node_states]);

  return (
    <button
      data-instance-card
      onClick={() => onSelect(inst.id)}
      aria-current={isSelected ? "true" : undefined}
      className={cn(
        "activity-list-card group relative w-full overflow-hidden rounded-[20px] border px-3 py-3 text-left transition-all duration-200",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-electric-indigo/50",
        isSelected
          ? "activity-card--selected border-electric-indigo/30 ring-1 ring-electric-indigo/15"
          : isFocused
            ? "border-electric-indigo/20 ring-1 ring-electric-indigo/10"
            : "border-orbflow-border/60 hover:border-orbflow-border-hover",
      )}
      style={isSelected ? undefined : {
        background: "color-mix(in srgb, var(--orbflow-bg) 74%, var(--orbflow-surface) 26%)",
      }}
    >
      <div
        className="absolute inset-y-4 left-0 w-[2.5px] rounded-full"
        style={{ backgroundColor: theme.accent, opacity: isSelected || isFocused ? 0.7 : 0.15 }}
      />

      <div className="flex items-start justify-between gap-2 pl-2">
        <div className="flex items-center gap-1 min-w-0">
          <span
            className="inline-flex shrink-0 items-center gap-1 rounded-full border px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.08em]"
            style={{
              color: theme.text,
              borderColor: `rgba(${theme.accentRgb},0.15)`,
              backgroundColor: `rgba(${theme.accentRgb},0.06)`,
            }}
          >
            <span className="h-1.5 w-1.5 rounded-full" style={{ backgroundColor: theme.accent, opacity: 0.85, boxShadow: isRunning ? `0 0 0 4px rgba(${theme.accentRgb},0.12)` : undefined }} />
            {inst.status}
          </span>
          {trigger && (
            <span className="inline-flex shrink-0 items-center gap-1 rounded-full border border-orbflow-border/40 bg-orbflow-bg/50 px-2 py-1 text-[11px] font-medium text-orbflow-text-ghost">
              <NodeIcon name={trigger.icon} className="h-3 w-3" />
              {trigger.label}
            </span>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <span className="whitespace-nowrap text-[11px] text-orbflow-text-ghost">
            {relativeTime(inst.created_at)}
          </span>
          <CopyIdButton instanceId={inst.id} />
        </div>
      </div>

      <p className="mt-2 truncate pl-2 text-[14px] font-semibold tracking-tight text-orbflow-text-secondary">
        {workflowName}
      </p>

      <div className="mt-2.5 flex items-center justify-between gap-3 pl-2 text-[11px] text-orbflow-text-faint">
        <span className="font-mono text-orbflow-text-ghost">#{inst.id.slice(-8)}</span>
        {isDone && inst.updated_at ? (
          <span className="flex items-center gap-1.5 tabular-nums">
            <NodeIcon name="clock" className="h-3 w-3" />
            {formatDurationRange(inst.created_at, inst.updated_at)}
          </span>
        ) : isRunning ? (
          <span className="flex items-center gap-1.5 tabular-nums" style={{ color: STATUS_THEMES.running.text }}>
            <NodeIcon name="loader" className="h-3 w-3 animate-spin" />
            Live
          </span>
        ) : (
          <span>Awaiting completion</span>
        )}
      </div>

      <div className="mt-3 pl-2">
        <div className="mb-1.5 flex items-center justify-between gap-2 text-[11px] font-medium text-orbflow-text-faint">
          <span>
            {progressTotal > 0 ? `${progressDone}/${progressTotal} steps settled` : "Waiting for node state"}
          </span>
          <span>
            {isRunning ? "Streaming" : isDone ? "Closed run" : "Queued"}
          </span>
        </div>
        <MiniProgress nodeStates={inst.node_states} status={inst.status} />
      </div>
    </button>
  );
});

/* -- EmptyIllustration ------------------------------- */

export function EmptyIllustration() {
  return (
    <svg width="240" height="72" viewBox="0 0 240 72" fill="none" className="opacity-30">
      <rect x="8" y="18" width="56" height="36" rx="10" stroke="rgba(124,92,252,0.25)" strokeWidth="1.5" fill="rgba(124,92,252,0.04)" />
      <circle cx="36" cy="36" r="8" fill="rgba(124,92,252,0.1)" />
      <polygon points="33,32 33,40 39,36" fill="rgba(124,92,252,0.4)" />
      <line x1="64" y1="36" x2="88" y2="36" stroke="rgba(124,92,252,0.15)" strokeWidth="1.5" strokeDasharray="4 3" />
      <circle cx="76" cy="36" r="2" fill="rgba(124,92,252,0.3)"><animate attributeName="opacity" values="0.15;0.5;0.15" dur="2s" repeatCount="indefinite" /></circle>
      <rect x="88" y="18" width="56" height="36" rx="10" stroke="rgba(6,182,212,0.25)" strokeWidth="1.5" fill="rgba(6,182,212,0.04)" />
      <circle cx="116" cy="36" r="8" fill="rgba(6,182,212,0.1)" />
      <rect x="112" y="33" width="8" height="6" rx="1" stroke="rgba(6,182,212,0.4)" strokeWidth="1.2" fill="none" />
      <line x1="144" y1="36" x2="168" y2="36" stroke="rgba(6,182,212,0.15)" strokeWidth="1.5" strokeDasharray="4 3" />
      <circle cx="156" cy="36" r="2" fill="rgba(6,182,212,0.3)"><animate attributeName="opacity" values="0.15;0.5;0.15" dur="2s" begin="0.6s" repeatCount="indefinite" /></circle>
      <rect x="168" y="18" width="56" height="36" rx="10" stroke="rgba(16,185,129,0.25)" strokeWidth="1.5" fill="rgba(16,185,129,0.04)" />
      <circle cx="196" cy="36" r="8" fill="rgba(16,185,129,0.1)" />
      <polyline points="191,36 194,39 201,32" stroke="rgba(16,185,129,0.4)" strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
