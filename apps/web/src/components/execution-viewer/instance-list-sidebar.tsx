"use client";

import { useState, useEffect, useRef, useMemo } from "react";
import { NodeIcon } from "@/core/components/icons";
import { EmptyState } from "@/core/components/empty-state";
import { cn } from "@/lib/cn";
import { STATUS_THEMES } from "@/lib/execution";

import type { Instance } from "@/lib/api";
import { STATUS_OPTIONS } from "./viewer-utils";
import { InstanceCard, SkeletonCard } from "./shared-components";

interface InstanceListSidebarProps {
  instances: Instance[];
  instancesLoading: boolean;
  selectedInstance: Instance | null;
  filteredInstances: Instance[];
  groupedInstances: { label: string; items: Instance[] }[];
  flatInstances: Instance[];
  statusFilter: string;
  setStatusFilter: (filter: string) => void;
  search: string;
  setSearch: (search: string) => void;
  statusCounts: Record<string, number>;
  getWorkflowName: (wfId: string) => string;
  triggerTypeMap: Record<string, { icon: string; label: string }>;
  onSelectInstance: (id: string) => void;
}

export function InstanceListSidebar({
  instances,
  instancesLoading,
  selectedInstance,
  filteredInstances,
  groupedInstances,
  flatInstances,
  statusFilter,
  setStatusFilter,
  search,
  setSearch,
  statusCounts,
  getWorkflowName,
  triggerTypeMap,
  onSelectInstance,
}: InstanceListSidebarProps) {
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const sidebarListRef = useRef<HTMLDivElement>(null);
  const focusedIndexRef = useRef(focusedIndex);

  useEffect(() => { focusedIndexRef.current = focusedIndex; }, [focusedIndex]);

  const totalCount = instances.length;

  const filterKey = useMemo(() => `${statusFilter}|${search}`, [statusFilter, search]);

  useEffect(() => {
    setFocusedIndex(-1);
  }, [filterKey]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const active = document.activeElement;
      if (active?.tagName === "INPUT" || active?.tagName === "TEXTAREA") return;
      if (!sidebarListRef.current?.contains(active) && active?.tagName !== "BODY") return;

      if (e.key === "ArrowDown" || e.key === "j") {
        e.preventDefault();
        setFocusedIndex((prev) => Math.min(prev + 1, flatInstances.length - 1));
      } else if (e.key === "ArrowUp" || e.key === "k") {
        e.preventDefault();
        setFocusedIndex((prev) => Math.max(prev - 1, 0));
      } else if (e.key === "Enter" && focusedIndexRef.current >= 0 && focusedIndexRef.current < flatInstances.length) {
        e.preventDefault();
        onSelectInstance(flatInstances[focusedIndexRef.current].id);
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [flatInstances, onSelectInstance]);

  useEffect(() => {
    if (focusedIndex < 0 || !sidebarListRef.current) return;
    const buttons = sidebarListRef.current.querySelectorAll("[data-instance-card]");
    const target = buttons[focusedIndex] as HTMLElement | undefined;
    target?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    target?.focus({ preventScroll: true });
  }, [focusedIndex]);

  const flatIndexMap = useMemo(() => {
    const map = new Map<string, number>();
    flatInstances.forEach((inst, i) => map.set(inst.id, i));
    return map;
  }, [flatInstances]);

  return (
    <aside className="activity-sidebar-panel flex max-h-[50vh] min-h-0 w-full shrink-0 flex-col overflow-hidden border-b border-orbflow-border xl:h-full xl:max-h-none xl:w-[20.5rem] xl:min-w-[20.5rem] xl:max-w-[20.5rem] xl:border-b-0 xl:border-r">
      <div className="border-b border-orbflow-border/70 px-4 py-3">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-xl border border-electric-indigo/15 bg-electric-indigo/8">
              <NodeIcon name="bar-chart" className="h-3.5 w-3.5 text-electric-indigo/80" />
            </div>
            <h2 className="text-[13px] font-semibold tracking-tight text-orbflow-text-secondary">
              Runs
            </h2>
          </div>
          <span className="rounded-full border border-orbflow-border/40 bg-orbflow-bg/50 px-2 py-0.5 text-[10px] font-medium tabular-nums text-orbflow-text-ghost">
            {filteredInstances.length}/{totalCount}
          </span>
        </div>

        <div className="relative mt-2.5">
          <NodeIcon name="search" className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-orbflow-text-ghost" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search runs…"
            className="w-full rounded-xl border border-orbflow-border/60 bg-orbflow-add-btn-bg px-9 py-2 text-[12px] text-orbflow-text-secondary placeholder:text-orbflow-text-ghost transition-all focus:border-electric-indigo/30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-electric-indigo/50"
          />
          {search && (
            <button
              onClick={() => setSearch("")}
              aria-label="Clear search"
              className="absolute right-1.5 top-1/2 flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-lg transition-colors hover:bg-orbflow-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-electric-indigo/50"
            >
              <NodeIcon name="x" className="h-3 w-3 text-orbflow-text-ghost" />
            </button>
          )}
        </div>

        <div className="-mx-1 mt-2.5 flex flex-wrap gap-1 px-1 pb-0.5">
          {STATUS_OPTIONS.map((status) => {
            const isActive = statusFilter === status;
            const theme = status !== "all" ? STATUS_THEMES[status] : null;
            const count = status === "all" ? totalCount : statusCounts[status] || 0;

            return (
                <button
                  key={status}
                  onClick={() => setStatusFilter(status)}
                  aria-pressed={isActive}
                className={cn(
                  "flex shrink-0 items-center gap-1.5 rounded-lg border px-2 py-1 text-[11px] font-medium capitalize transition-all",
                  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-electric-indigo/50",
                  isActive
                    ? "border-electric-indigo/20 bg-electric-indigo/10 text-electric-indigo"
                    : "border-transparent bg-transparent text-orbflow-text-muted hover:bg-orbflow-surface-hover/60 hover:text-orbflow-text-secondary",
                )}
              >
                {theme ? (
                  <span
                    className="h-1.5 w-1.5 rounded-full"
                    style={{ backgroundColor: theme.accent, opacity: isActive ? 0.8 : 0.35 }}
                  />
                ) : null}
                <span>{status}</span>
                {count > 0 && (
                  <span className="text-[10px] tabular-nums text-current/50">
                    {count}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>

      <div ref={sidebarListRef} className="custom-scrollbar flex-1 min-h-0 overflow-y-auto overscroll-contain px-4 py-3">
        {instancesLoading && instances.length === 0 && (
          <div className="space-y-3">
            <SkeletonCard />
            <SkeletonCard />
            <SkeletonCard />
          </div>
        )}

        {groupedInstances.map((group) => (
          <section key={group.label} className="mb-4 last:mb-0">
            <div className="activity-date-group mb-2.5 flex items-center justify-between rounded-full px-3 py-1.5">
              <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-orbflow-text-ghost">
                {group.label}
              </span>
              <span className="rounded-full border border-orbflow-border/60 bg-orbflow-bg/70 px-2 py-0.5 text-[10px] font-mono tabular-nums text-orbflow-text-faint">
                {group.items.length}
              </span>
            </div>

            <div className="space-y-2.5">
              {group.items.map((inst) => (
                <InstanceCard
                  key={inst.id}
                  inst={inst}
                  isSelected={selectedInstance?.id === inst.id}
                  isFocused={flatIndexMap.get(inst.id) === focusedIndex}
                  workflowName={getWorkflowName(inst.workflow_id)}
                  trigger={triggerTypeMap[inst.workflow_id]}
                  onSelect={onSelectInstance}
                />
              ))}
            </div>
          </section>
        ))}

        {!instancesLoading && filteredInstances.length === 0 && (
          instances.length === 0 ? (
            <EmptyState
              icon="inbox"
              title="No runs yet"
              description="Run a workflow from the Builder tab to see execution history."
            />
          ) : (
            <EmptyState
              icon="search"
              title="No matching runs"
              description="Adjust filters or search to find runs."
            />
          )
        )}
      </div>
    </aside>
  );
}
