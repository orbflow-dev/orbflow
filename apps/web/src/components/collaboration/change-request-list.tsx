"use client";

import { memo, useEffect, useState, useMemo } from "react";
import { useChangeRequestStore } from "@orbflow/core/stores";
import type { ChangeRequestStatus } from "@orbflow/core/types";
import { Button } from "@/core/components/button";
import { EmptyState } from "@/core/components/empty-state";
import { SkeletonRow } from "@/core/components/skeleton";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { timeAgo, StatusBadge, initials } from "./shared";

/* =======================================================
   Filter tabs
   ======================================================= */

const FILTER_TABS: readonly { label: string; value: ChangeRequestStatus | undefined }[] = [
  { label: "All", value: undefined },
  { label: "Open", value: "open" },
  { label: "Draft", value: "draft" },
  { label: "Approved", value: "approved" },
  { label: "Rejected", value: "rejected" },
  { label: "Merged", value: "merged" },
] as const;

/* =======================================================
   Main component
   ======================================================= */

interface ChangeRequestListProps {
  workflowId: string;
  onSelect: (crId: string) => void;
  onCreate: () => void;
}

export const ChangeRequestList = memo(function ChangeRequestList({
  workflowId,
  onSelect,
  onCreate,
}: ChangeRequestListProps) {
  const { changeRequests, loading, fetchChangeRequests } = useChangeRequestStore();
  const [filter, setFilter] = useState<ChangeRequestStatus | undefined>(undefined);
  const [search, setSearch] = useState("");

  useEffect(() => {
    fetchChangeRequests(workflowId, filter).catch(() => {
      /* error is handled by the store via toast */
    });
  }, [workflowId, filter, fetchChangeRequests]);

  const visibleCRs = useMemo(
    () =>
      search.trim()
        ? changeRequests.filter((cr) =>
            cr.title.toLowerCase().includes(search.trim().toLowerCase()) ||
            cr.author.toLowerCase().includes(search.trim().toLowerCase())
          )
        : changeRequests,
    [changeRequests, search]
  );

  return (
    <div className="flex flex-col h-full bg-orbflow-bg">
      {/* Header */}
      <div className="p-4 border-b border-orbflow-border">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <h2 className="text-heading font-semibold text-orbflow-text-secondary">
              Change Requests
            </h2>
            {changeRequests.length > 0 && (
              <span className="text-micro font-medium px-1.5 py-0.5 rounded-md bg-orbflow-surface-hover text-orbflow-text-ghost">
                {changeRequests.length}
              </span>
            )}
          </div>
          <Button variant="primary" size="sm" onClick={onCreate}>
            <NodeIcon name="plus" className="w-3 h-3" />
            New CR
          </Button>
        </div>

        {/* Search input */}
        {changeRequests.length > 3 && (
          <div className="relative mb-2.5">
            <NodeIcon
              name="search"
              className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-orbflow-text-ghost pointer-events-none"
            />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search by title or author..."
              className="w-full rounded-lg border border-orbflow-border bg-orbflow-surface pl-8.5 pr-3 py-2
                text-body text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
                focus:outline-none focus:border-electric-indigo/30
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
            />
            {search && (
              <button
                onClick={() => setSearch("")}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 p-0.5 rounded text-orbflow-text-ghost
                  hover:text-orbflow-text-muted transition-colors"
                aria-label="Clear search"
              >
                <NodeIcon name="x" className="w-3 h-3" />
              </button>
            )}
          </div>
        )}

        {/* Filter tabs */}
        <div className="flex flex-wrap gap-1">
          {FILTER_TABS.map((tab) => (
            <button
              key={tab.label}
              onClick={() => setFilter(tab.value)}
              className={cn(
                "px-2 py-1 rounded-md text-caption font-medium transition-colors",
                filter === tab.value
                  ? "bg-electric-indigo/15 text-electric-indigo"
                  : "text-orbflow-text-ghost hover:bg-orbflow-surface-hover",
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto custom-scrollbar">
        {loading && changeRequests.length === 0 ? (
          <div className="p-3 space-y-2">
            <SkeletonRow />
            <SkeletonRow widths={["w-20", "w-12"]} />
            <SkeletonRow widths={["w-28", "w-14"]} />
          </div>
        ) : visibleCRs.length === 0 ? (
          search.trim() ? (
            <div className="flex flex-col items-center justify-center px-6 py-12 text-center">
              <NodeIcon name="search" className="w-5 h-5 text-orbflow-text-ghost mb-2" />
              <p className="text-body text-orbflow-text-faint">
                No change requests matching &ldquo;{search.trim()}&rdquo;
              </p>
            </div>
          ) : (
            <EmptyState
              icon="file-text"
              title="No change requests"
              description="Create a change request to propose workflow changes"
              action={{ label: "New Change Request", onClick: onCreate }}
            />
          )
        ) : (
          <div aria-label="Change requests" role="list" className="p-2 space-y-1.5">
            {visibleCRs.map((cr, index) => {
              const commentCount = cr.comments?.length ?? 0;
              const unresolvedCount = cr.comments?.filter((c) => !c.resolved).length ?? 0;
              return (
                <div
                  key={cr.id}
                  role="listitem"
                  tabIndex={0}
                  aria-label={`View change request: ${cr.title}`}
                  onClick={() => onSelect(cr.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onSelect(cr.id);
                    }
                  }}
                  className={cn(
                    "px-4 py-3 rounded-xl border border-orbflow-border bg-orbflow-surface",
                    "hover:bg-orbflow-surface-hover cursor-pointer transition-all duration-150",
                    "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                    "animate-fade-in-up",
                  )}
                  style={{ animationDelay: `${Math.min(index * 30, 150)}ms` }}
                >
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-body-lg font-medium text-orbflow-text-secondary truncate">
                      {cr.title}
                    </span>
                    <StatusBadge status={cr.status} />
                  </div>
                  <div className="flex items-center gap-3 text-body-sm text-orbflow-text-ghost">
                    <span className="flex items-center gap-1.5 truncate">
                      <span className="w-5 h-5 rounded-full bg-electric-indigo/10 text-electric-indigo text-[9px] font-bold flex items-center justify-center shrink-0">
                        {initials(cr.author)}
                      </span>
                      {cr.author}
                    </span>
                    <span className="flex items-center gap-1">
                      <NodeIcon name="message-circle" className="w-3 h-3" />
                      {commentCount}
                      {unresolvedCount > 0 && (
                        <span className="text-amber-400">({unresolvedCount})</span>
                      )}
                    </span>
                    <span className="text-orbflow-text-ghost/60">v{cr.base_version}</span>
                    <span className="ml-auto shrink-0">{timeAgo(cr.created_at)}</span>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
});
