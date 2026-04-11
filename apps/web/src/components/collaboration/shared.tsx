"use client";

import { memo } from "react";
import type { ChangeRequestStatus } from "@orbflow/core/types";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";

/** Formats a date string as a relative time (e.g., "2h ago", "3d ago"). */
export function timeAgo(dateStr: string): string {
  const seconds = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

/** Returns first letter of a name, uppercase. */
export function initials(name: string): string {
  return (name.trim()[0] ?? "?").toUpperCase();
}

/** Color classes for each change request status. */
export const STATUS_COLORS: Record<ChangeRequestStatus, string> = {
  draft: "bg-orbflow-surface-hover text-orbflow-text-muted",
  open: "bg-blue-500/20 text-blue-400",
  approved: "bg-emerald-500/20 text-emerald-400",
  rejected: "bg-red-500/20 text-red-400",
  merged: "bg-purple-500/20 text-purple-400",
};

const STATUS_ICONS: Record<ChangeRequestStatus, string> = {
  draft: "file-text",
  open: "git-pull-request",
  approved: "check",
  rejected: "x",
  merged: "git-merge",
};

/** Inline status badge for change requests with icon. */
export const StatusBadge = memo(function StatusBadge({ status }: { status: ChangeRequestStatus }) {
  return (
    <span className={cn("inline-flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-full font-medium capitalize", STATUS_COLORS[status])}>
      <NodeIcon name={STATUS_ICONS[status]} className="w-2.5 h-2.5" />
      {status}
    </span>
  );
});
