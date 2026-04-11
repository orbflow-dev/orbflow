import type { Instance } from "@/lib/api";

/* -- Utility functions ------------------------------- */

export function relativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const s = Math.floor(diff / 1000);
  if (s < 60) return "just now";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

const MS_PER_DAY = 86_400_000;

export function getDateGroup(dateStr: string): string {
  const d = new Date(dateStr);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today.getTime() - MS_PER_DAY);
  const weekAgo = new Date(today.getTime() - 7 * MS_PER_DAY);
  if (d >= today) return "Today";
  if (d >= yesterday) return "Yesterday";
  if (d >= weekAgo) return "This Week";
  return "Earlier";
}

export function formatOutput(output: Record<string, unknown>): string {
  try {
    const parsed = JSON.parse(JSON.stringify(output), (_key, value) => {
      if (typeof value === "string") {
        try { return JSON.parse(value); } catch { return value; }
      }
      return value;
    });
    return JSON.stringify(parsed, null, 2);
  } catch {
    return JSON.stringify(output, null, 2);
  }
}

export function instanceStats(nodeStates: Record<string, { status: string }> | undefined) {
  if (!nodeStates) return { completed: 0, running: 0, failed: 0, pending: 0, cancelled: 0, skipped: 0, total: 0 };
  const counts = { completed: 0, running: 0, failed: 0, pending: 0, cancelled: 0, skipped: 0 };
  const states = Object.values(nodeStates);
  for (const s of states) {
    if (s.status in counts) counts[s.status as keyof typeof counts]++;
  }
  return { ...counts, total: states.length };
}

/* -- Constants --------------------------------------- */

export const STATUS_OPTIONS = ["all", "running", "completed", "failed", "cancelled", "pending"] as const;
