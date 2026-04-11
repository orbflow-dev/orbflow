"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/cn";
import { api } from "@/lib/api";
import { NodeIcon } from "@/core/components/icons";
import type { WorkflowVersion, WorkflowDiff } from "@/lib/api";

/* =======================================================
   Types
   ======================================================= */

interface VersionHistoryProps {
  workflowId: string;
  currentVersion: number;
  open: boolean;
  onClose: () => void;
}

interface VersionCardProps {
  version: WorkflowVersion;
  isCurrent: boolean;
  diff: WorkflowDiff | null;
  diffLoading: boolean;
  diffExpanded: boolean;
  onToggleDiff: () => void;
}

/* =======================================================
   Helpers
   ======================================================= */

function formatTimestamp(iso: string): string {
  const date = new Date(iso);
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  }) + ", " + date.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

function diffSummaryText(diff: WorkflowDiff): string {
  const parts: string[] = [];
  if (diff.added_nodes.length > 0) {
    parts.push(`+${diff.added_nodes.length} node${diff.added_nodes.length !== 1 ? "s" : ""}`);
  }
  if (diff.removed_nodes.length > 0) {
    parts.push(`-${diff.removed_nodes.length} node${diff.removed_nodes.length !== 1 ? "s" : ""}`);
  }
  if (diff.modified_nodes.length > 0) {
    parts.push(`~${diff.modified_nodes.length} modified`);
  }
  if (diff.added_edges.length > 0 || diff.removed_edges.length > 0) {
    const edgeChanges = diff.added_edges.length + diff.removed_edges.length;
    parts.push(`${edgeChanges} edge${edgeChanges !== 1 ? "s" : ""} changed`);
  }
  return parts.length > 0 ? parts.join(", ") : "No changes";
}

/* =======================================================
   Version Card
   ======================================================= */

function VersionCard({
  version,
  isCurrent,
  diff,
  diffLoading,
  diffExpanded,
  onToggleDiff,
}: VersionCardProps) {
  return (
    <div
      className={cn(
        "rounded-xl border p-3.5 transition-colors",
        isCurrent
          ? "border-electric-indigo/30 bg-electric-indigo/[0.04]"
          : "border-orbflow-border bg-orbflow-surface hover:border-orbflow-border-hover"
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <span
            className={cn(
              "text-body font-semibold tabular-nums",
              isCurrent ? "text-electric-indigo" : "text-orbflow-text-secondary"
            )}
          >
            v{version.version}
          </span>
          <span className="text-body-sm text-orbflow-text-faint">
            {formatTimestamp(version.created_at)}
          </span>
        </div>
        {isCurrent && (
          <span className="text-body-sm font-medium text-electric-indigo bg-electric-indigo/10 px-2 py-0.5 rounded-md whitespace-nowrap">
            Current
          </span>
        )}
      </div>

      {/* Author */}
      {version.author && (
        <p className="text-body-sm text-orbflow-text-muted mt-1 truncate">
          by {version.author}
        </p>
      )}

      {/* Message */}
      {version.message && (
        <p className="text-body-sm text-orbflow-text-muted mt-1 line-clamp-2">
          {version.message}
        </p>
      )}

      {/* Diff summary for non-current versions */}
      {!isCurrent && (
        <div className="mt-2.5">
          {diffLoading ? (
            <div className="flex items-center gap-1.5 text-body-sm text-orbflow-text-faint">
              <NodeIcon name="loader" className="w-3 h-3 animate-spin" />
              Loading diff...
            </div>
          ) : diff ? (
            <>
              {/* Diff summary badges */}
              <div className="flex flex-wrap gap-1.5">
                {diff.added_nodes.length > 0 && (
                  <span className="text-body-sm font-medium text-emerald-400 bg-emerald-500/10 px-1.5 py-0.5 rounded">
                    +{diff.added_nodes.length} added
                  </span>
                )}
                {diff.removed_nodes.length > 0 && (
                  <span className="text-body-sm font-medium text-red-400 bg-red-500/10 px-1.5 py-0.5 rounded">
                    -{diff.removed_nodes.length} removed
                  </span>
                )}
                {diff.modified_nodes.length > 0 && (
                  <span className="text-body-sm font-medium text-amber-400 bg-amber-500/10 px-1.5 py-0.5 rounded">
                    ~{diff.modified_nodes.length} modified
                  </span>
                )}
                {(diff.added_edges.length > 0 || diff.removed_edges.length > 0) && (
                  <span className="text-body-sm font-medium text-orbflow-text-muted bg-orbflow-surface-hover px-1.5 py-0.5 rounded">
                    {diff.added_edges.length + diff.removed_edges.length} edge changes
                  </span>
                )}
              </div>

              {/* View Diff button */}
              <button
                onClick={onToggleDiff}
                className="flex items-center gap-1 mt-2 text-body-sm font-medium text-orbflow-text-muted
                  hover:text-electric-indigo transition-colors"
              >
                <NodeIcon
                  name={diffExpanded ? "chevron-down" : "chevron-right"}
                  className="w-3 h-3"
                />
                {diffExpanded ? "Hide Details" : "View Diff"}
              </button>

              {/* Expanded diff detail */}
              {diffExpanded && (
                <div className="mt-2 space-y-2 pl-1 border-l-2 border-orbflow-border ml-1">
                  {diff.added_nodes.length > 0 && (
                    <DiffSection
                      label="Added Nodes"
                      items={diff.added_nodes}
                      color="text-emerald-400"
                      icon="plus"
                    />
                  )}
                  {diff.removed_nodes.length > 0 && (
                    <DiffSection
                      label="Removed Nodes"
                      items={diff.removed_nodes}
                      color="text-red-400"
                      icon="minus"
                    />
                  )}
                  {diff.modified_nodes.length > 0 && (
                    <DiffSection
                      label="Modified Nodes"
                      items={diff.modified_nodes}
                      color="text-amber-400"
                      icon="edit"
                    />
                  )}
                  {diff.added_edges.length > 0 && (
                    <DiffSection
                      label="Added Edges"
                      items={diff.added_edges}
                      color="text-emerald-400"
                      icon="plus"
                    />
                  )}
                  {diff.removed_edges.length > 0 && (
                    <DiffSection
                      label="Removed Edges"
                      items={diff.removed_edges}
                      color="text-red-400"
                      icon="minus"
                    />
                  )}
                </div>
              )}
            </>
          ) : (
            <span className="text-body-sm text-orbflow-text-faint">
              Initial version
            </span>
          )}
        </div>
      )}
    </div>
  );
}

/* =======================================================
   Diff Section (expanded detail)
   ======================================================= */

function DiffSection({
  label,
  items,
  color,
  icon,
}: {
  label: string;
  items: string[];
  color: string;
  icon: string;
}) {
  return (
    <div className="pl-2">
      <p className={cn("text-body-sm font-medium mb-0.5", color)}>{label}</p>
      <ul className="space-y-0.5">
        {items.map((item) => (
          <li key={item} className="flex items-center gap-1.5 text-body-sm text-orbflow-text-muted">
            <NodeIcon name={icon} className={cn("w-2.5 h-2.5 shrink-0", color)} />
            <span className="font-mono text-[11px] truncate">{item}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/* =======================================================
   Version History Panel
   ======================================================= */

export function VersionHistory({
  workflowId,
  currentVersion,
  open,
  onClose,
}: VersionHistoryProps) {
  const [versions, setVersions] = useState<WorkflowVersion[]>([]);
  const [diffs, setDiffs] = useState<Record<number, WorkflowDiff>>({});
  const [loadingDiffs, setLoadingDiffs] = useState<Set<number>>(new Set());
  const [expandedDiff, setExpandedDiff] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  // Fetch versions when panel opens
  useEffect(() => {
    if (!open || !workflowId) return;

    let cancelled = false;
    setLoading(true);
    setError(null);

    api.versions
      .list(workflowId, { limit: 50 })
      .then((result) => {
        if (cancelled) return;
        // Sort versions descending (newest first)
        const sorted = [...result.items].sort((a, b) => b.version - a.version);
        setVersions(sorted);
      })
      .catch((err) => {
        if (cancelled) return;
        console.error("[orbflow] Failed to fetch versions:", err);
        setError(err instanceof Error ? err.message : "Failed to load versions");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [open, workflowId]);

  // Fetch diffs for non-current versions once the version list loads
  useEffect(() => {
    if (versions.length < 2) return;

    let cancelled = false;

    for (const version of versions) {
      // Skip the latest (current) version and version 1 (initial, no previous to diff from)
      if (version.version === currentVersion || version.version <= 1) continue;
      // Skip if already loaded
      if (diffs[version.version] !== undefined) continue;

      setLoadingDiffs((prev) => new Set([...prev, version.version]));

      api.versions
        .diff(workflowId, version.version, currentVersion)
        .then((diff) => {
          if (cancelled) return;
          setDiffs((prev) => ({ ...prev, [version.version]: diff }));
        })
        .catch((err) => {
          if (cancelled) return;
          console.error(`[orbflow] Failed to fetch diff for v${version.version}:`, err);
        })
        .finally(() => {
          if (!cancelled) {
            setLoadingDiffs((prev) => {
              const next = new Set(prev);
              next.delete(version.version);
              return next;
            });
          }
        });
    }

    return () => { cancelled = true; };
  }, [versions, workflowId, currentVersion, diffs]);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [open, onClose]);

  // Close on click outside
  useEffect(() => {
    if (!open) return;
    const handleClick = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    // Use setTimeout to avoid immediately closing from the click that opened the panel
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClick);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClick);
    };
  }, [open, onClose]);

  const handleToggleDiff = useCallback((version: number) => {
    setExpandedDiff((prev) => (prev === version ? null : version));
  }, []);

  if (!open) return null;

  const panel = (
    <div className="fixed inset-0 z-[70] pointer-events-none">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/30 backdrop-blur-[2px] pointer-events-auto animate-fade-in"
        onClick={onClose}
      />

      {/* Slide-in panel */}
      <div
        ref={panelRef}
        role="dialog"
        aria-label="Version History"
        className={cn(
          "absolute top-0 right-0 h-full w-[380px] max-w-[90vw]",
          "bg-orbflow-surface border-l border-orbflow-border shadow-2xl",
          "pointer-events-auto flex flex-col",
          "animate-slide-in-right"
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-orbflow-border shrink-0">
          <div className="flex items-center gap-2">
            <NodeIcon name="clock" className="w-4 h-4 text-orbflow-text-muted" />
            <h2 className="text-body-lg font-semibold text-orbflow-text-secondary">
              Version History
            </h2>
          </div>
          <button
            onClick={onClose}
            className="flex items-center justify-center w-7 h-7 rounded-lg
              text-orbflow-text-muted hover:bg-orbflow-controls-btn-hover
              hover:text-orbflow-text-secondary transition-colors"
            aria-label="Close version history"
          >
            <NodeIcon name="x" className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-4 py-3 space-y-2.5">
          {loading ? (
            <div className="flex flex-col items-center justify-center py-12 gap-2">
              <NodeIcon name="loader" className="w-5 h-5 text-orbflow-text-faint animate-spin" />
              <span className="text-body-sm text-orbflow-text-faint">Loading versions...</span>
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center py-12 gap-2">
              <NodeIcon name="alert-triangle" className="w-5 h-5 text-red-400" />
              <span className="text-body-sm text-red-400">{error}</span>
            </div>
          ) : versions.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 gap-2">
              <NodeIcon name="clock" className="w-5 h-5 text-orbflow-text-faint" />
              <span className="text-body-sm text-orbflow-text-faint">No versions yet</span>
              <span className="text-body-sm text-orbflow-text-ghost">
                Save your workflow to create the first version.
              </span>
            </div>
          ) : (
            versions.map((version) => {
              const isCurrent = version.version === currentVersion;
              const diff = diffs[version.version] ?? null;
              const isDiffLoading = loadingDiffs.has(version.version);
              const isExpanded = expandedDiff === version.version;

              return (
                <VersionCard
                  key={version.id}
                  version={version}
                  isCurrent={isCurrent}
                  diff={diff}
                  diffLoading={isDiffLoading}
                  diffExpanded={isExpanded}
                  onToggleDiff={() => handleToggleDiff(version.version)}
                />
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="px-4 py-2.5 border-t border-orbflow-border shrink-0">
          <p className="text-body-sm text-orbflow-text-ghost text-center">
            {versions.length > 0
              ? `${versions.length} version${versions.length !== 1 ? "s" : ""}`
              : "No history available"}
          </p>
        </div>
      </div>
    </div>
  );

  return createPortal(panel, document.body);
}
