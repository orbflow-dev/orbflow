import type { ExecutionStatus } from "./execution-status";

export const NODE_SIZES = {
  trigger: 68,
  action: 64,
  capability: 52,
} as const;

export type { ExecutionStatus } from "./execution-status";

export const STATUS_COLORS: Record<string, string> = {
  pending: "#64748B",
  queued: "#8B8FA3",
  running: "#4A9AAF",
  completed: "#10B981",
  failed: "#D9454F",
  skipped: "#94A3B8",
  cancelled: "#C49030",
  waiting_approval: "#E8A317",
};

export interface StatusTheme {
  accent: string;
  accentRgb: string;
  text: string;
  bg: string;
  icon: string;
  label: string;
}

export const STATUS_THEMES: Record<string, StatusTheme> = {
  pending: {
    accent: "#64748B",
    accentRgb: "100,116,139",
    text: "var(--exec-text-pending)",
    bg: "rgba(100,116,139,0.06)",
    icon: "clock",
    label: "Pending",
  },
  queued: {
    accent: "#8B8FA3",
    accentRgb: "139,143,163",
    text: "var(--exec-text-pending)",
    bg: "rgba(139,143,163,0.06)",
    icon: "clock",
    label: "Queued",
  },
  running: {
    accent: "#4A9AAF",
    accentRgb: "74,154,175",
    text: "var(--exec-text-running)",
    bg: "rgba(74,154,175,0.08)",
    icon: "loader",
    label: "Running",
  },
  completed: {
    accent: "#10B981",
    accentRgb: "16,185,129",
    text: "var(--exec-text-completed)",
    bg: "rgba(16,185,129,0.08)",
    icon: "check",
    label: "Completed",
  },
  failed: {
    accent: "#D9454F",
    accentRgb: "217,69,79",
    text: "var(--exec-text-failed)",
    bg: "rgba(217,69,79,0.08)",
    icon: "x",
    label: "Failed",
  },
  skipped: {
    accent: "#94A3B8",
    accentRgb: "148,163,184",
    text: "var(--exec-text-pending)",
    bg: "rgba(148,163,184,0.06)",
    icon: "x",
    label: "Skipped",
  },
  cancelled: {
    accent: "#C49030",
    accentRgb: "196,144,48",
    text: "var(--exec-text-cancelled)",
    bg: "rgba(196,144,48,0.08)",
    icon: "x",
    label: "Cancelled",
  },
  waiting_approval: {
    accent: "#E8A317",
    accentRgb: "232,163,23",
    text: "var(--exec-text-cancelled)",
    bg: "rgba(232,163,23,0.10)",
    icon: "pause",
    label: "Waiting Approval",
  },
};

export const FALLBACK_THEME = STATUS_THEMES.pending;

export const STATUS_LABELS: Record<string, string> = {
  pending: "Pending",
  queued: "Queued",
  running: "Running",
  completed: "Completed",
  failed: "Failed",
  skipped: "Skipped",
  cancelled: "Cancelled",
  waiting_approval: "Waiting Approval",
};

export const STATUS_BADGE: Record<
  string,
  { cssModifier: string; icon: string; spin?: boolean } | undefined
> = {
  completed: { cssModifier: "exec-badge--completed", icon: "check" },
  failed: { cssModifier: "exec-badge--failed", icon: "x" },
  running: { cssModifier: "exec-badge--running", icon: "loader", spin: true },
  skipped: { cssModifier: "exec-badge--skipped", icon: "skip-forward" },
  cancelled: { cssModifier: "exec-badge--cancelled", icon: "x" },
  waiting_approval: { cssModifier: "exec-badge--waiting-approval", icon: "pause" },
};

export const SEGMENT_ORDER = [
  "completed",
  "failed",
  "running",
  "waiting_approval",
  "cancelled",
  "skipped",
  "pending",
] as const;

/** Format milliseconds to human-readable (e.g. "<1s", "3.2s", "2m 15s") */
export function formatDurationMs(ms: number): string {
  if (ms < 1000) return "<1s";
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const m = Math.floor(ms / 60000);
  const s = Math.floor((ms % 60000) / 1000);
  return `${m}m ${s}s`;
}

/** Format a date range to duration string (e.g. "<1s", "45s", "2m 30s", "1h 5m") */
export function formatDurationRange(startStr: string, endStr: string): string {
  const ms = Math.max(0, new Date(endStr).getTime() - new Date(startStr).getTime());
  const s = Math.floor(ms / 1000);
  if (s < 1) return "<1s";
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}
