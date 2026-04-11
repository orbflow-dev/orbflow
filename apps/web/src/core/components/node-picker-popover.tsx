"use client";

import { useState, useMemo, useEffect, useRef, useCallback } from "react";
import { createPortal } from "react-dom";
import { useOrbflow } from "../context/orbflow-provider";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { NodeIcon } from "./icons";
import { cn } from "../utils/cn";
import type { NodeKind, NodeTypeDefinition } from "../types/schema";

type TabKind = "all" | "trigger" | "action" | "capability";

const TABS: { id: TabKind; label: string; icon: string; kind?: NodeKind }[] = [
  { id: "all", label: "All", icon: "layers" },
  { id: "trigger", label: "Triggers", icon: "zap", kind: "trigger" },
  { id: "action", label: "Actions", icon: "play", kind: "action" },
  { id: "capability", label: "Connections", icon: "database", kind: "capability" },
];

const RECENT_STORAGE_KEY = "orbflow-recent-nodes";
const MAX_RECENT = 5;

function getRecentNodes(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch (err) {
    console.error("[orbflow] Failed to parse recent nodes from localStorage:", err);
    return [];
  }
}

function addRecentNode(pluginRef: string) {
  const recent = getRecentNodes().filter((r) => r !== pluginRef);
  recent.unshift(pluginRef);
  localStorage.setItem(RECENT_STORAGE_KEY, JSON.stringify(recent.slice(0, MAX_RECENT)));
}

/** Score a schema against a search query. Higher = better match. 0 = no match. */
function scoreMatch(schema: NodeTypeDefinition, query: string): number {
  const q = query.toLowerCase();
  const name = schema.name.toLowerCase();
  const ref = schema.pluginRef.toLowerCase();
  const desc = schema.description.toLowerCase();

  if (name === q || ref === q) return 100;            // exact
  if (name.startsWith(q) || ref.startsWith(q)) return 80; // starts with
  if (name.includes(q) || ref.includes(q)) return 60; // includes in name/ref
  if (desc.includes(q)) return 30;                    // includes in description
  // Simple fuzzy: all query chars appear in order in the name
  let qi = 0;
  for (let i = 0; i < name.length && qi < q.length; i++) {
    if (name[i] === q[qi]) qi++;
  }
  if (qi === q.length) return 10;
  return 0;
}

interface NodePickerPopoverProps {
  position: { x: number; y: number };
  allowedKinds?: NodeKind[];
  onSelect: (pluginRef: string) => void;
  onClose: () => void;
}

export function NodePickerPopover({
  allowedKinds,
  onSelect,
  onClose,
}: NodePickerPopoverProps) {
  const { registry, schemasReady } = useOrbflow();
  const [search, setSearch] = useState("");
  const [activeTab, setActiveTab] = useState<TabKind>(
    () => allowedKinds?.length === 1 ? allowedKinds[0] as TabKind : "all"
  );
  const [focusIndex, setFocusIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useFocusTrap(modalRef);

  // Determine which tabs to show
  const visibleTabs = useMemo(() => {
    if (!allowedKinds || allowedKinds.length === 0) return TABS;
    return TABS.filter((t) => t.id === "all" || (t.kind && allowedKinds.includes(t.kind)));
  }, [allowedKinds]);

  // Auto-focus search input
  useEffect(() => {
    requestAnimationFrame(() => inputRef.current?.focus());
  }, []);

  const allSchemas = useMemo(() => registry.getAll(), [registry, schemasReady]);

  // Recent nodes (only shown when no search query)
  const recentSchemas = useMemo(() => {
    if (search.trim()) return [];
    const recentRefs = getRecentNodes();
    return recentRefs
      .map((ref) => allSchemas.find((s) => s.pluginRef === ref))
      .filter((s): s is NodeTypeDefinition => s != null);
  }, [allSchemas, search]);

  const filtered = useMemo(() => {
    let list = allSchemas;

    // Filter by allowed kinds
    if (allowedKinds && allowedKinds.length > 0) {
      list = list.filter((s) => {
        const kind = s.nodeKind || "action";
        return allowedKinds.includes(kind as NodeKind);
      });
    }

    // Filter by active tab
    if (activeTab !== "all") {
      list = list.filter((s) => (s.nodeKind || "action") === activeTab);
    }

    // Ranked search
    if (search.trim()) {
      const scored = list
        .map((s) => ({ schema: s, score: scoreMatch(s, search.trim()) }))
        .filter((r) => r.score > 0)
        .sort((a, b) => b.score - a.score);
      return scored.map((r) => r.schema);
    }

    return list;
  }, [allSchemas, search, allowedKinds, activeTab]);

  // Reset focus when filters change -- derived from filtered list
  const prevFilterKey = useRef(`${search}|${activeTab}`);
  const filterKey = `${search}|${activeTab}`;
  if (filterKey !== prevFilterKey.current) {
    prevFilterKey.current = filterKey;
    if (focusIndex !== 0) setFocusIndex(0);
  }

  const handleSelect = useCallback(
    (pluginRef: string) => {
      addRecentNode(pluginRef);
      onSelect(pluginRef);
      onClose();
    },
    [onSelect, onClose]
  );

  // Keyboard navigation
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }

      const cols = 4;
      const total = filtered.length;
      if (total === 0) return;

      if (e.key === "ArrowRight") {
        e.preventDefault();
        setFocusIndex((i) => (i + 1) % total);
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        setFocusIndex((i) => (i - 1 + total) % total);
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setFocusIndex((i) => Math.min(i + cols, total - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setFocusIndex((i) => Math.max(i - cols, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (filtered[focusIndex]) {
          handleSelect(filtered[focusIndex].pluginRef);
        }
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose, filtered, focusIndex, handleSelect]);

  // Scroll focused card into view
  useEffect(() => {
    if (!gridRef.current) return;
    const card = gridRef.current.children[focusIndex] as HTMLElement | undefined;
    card?.scrollIntoView({ block: "nearest" });
  }, [focusIndex]);

  return createPortal(
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-[69] bg-orbflow-backdrop animate-[modalBackdropIn_0.2s_ease-out]"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="fixed inset-0 z-[70] flex items-center justify-center pointer-events-none">
        <div
          ref={modalRef}
          role="dialog"
          aria-modal="true"
          aria-label="Add node"
          className="w-full max-w-2xl h-[62vh] rounded-2xl shadow-2xl
            flex flex-col overflow-hidden pointer-events-auto
            border border-orbflow-border bg-orbflow-surface
            animate-scale-in"
        >
          {/* Header: Search */}
          <div className="p-4 pb-0">
            <div className="relative">
              <NodeIcon
                name="search"
                className="absolute left-3.5 top-1/2 -translate-y-1/2 w-4 h-4 text-orbflow-text-ghost pointer-events-none"
              />
              <input
                ref={inputRef}
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search nodes..."
                aria-label="Search nodes"
                aria-controls="node-picker-grid"
                aria-activedescendant={filtered[focusIndex] ? `node-option-${filtered[focusIndex].pluginRef}` : undefined}
                className="w-full rounded-xl pl-10 pr-10 py-2.5
                  text-heading
                  focus:outline-none focus:border-electric-indigo/30
                  focus-visible:ring-2 focus-visible:ring-electric-indigo/50
                  transition-all duration-200
                  border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary
                  placeholder:text-orbflow-text-ghost"
              />
              <button
                onClick={onClose}
                className="absolute right-3 top-1/2 -translate-y-1/2 p-0.5 rounded-md
                  text-orbflow-text-ghost hover:text-orbflow-text-faint transition-colors
                  focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                title="Close"
                aria-label="Close node picker"
              >
                <NodeIcon name="x" className="w-4 h-4" />
              </button>
            </div>
          </div>

          {/* Tabs */}
          <div className="flex items-center gap-1 px-4 pt-3 pb-1">
            {visibleTabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body font-medium transition-all",
                  "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                  activeTab === tab.id
                    ? "bg-electric-indigo/10 text-electric-indigo"
                    : "text-orbflow-text-faint hover:text-orbflow-text-muted hover:bg-orbflow-surface-hover",
                )}
              >
                <NodeIcon name={tab.icon} className="w-3 h-3" />
                {tab.label}
              </button>
            ))}
            <span className="ml-auto text-body-sm text-orbflow-text-ghost">
              {filtered.length} node{filtered.length !== 1 ? "s" : ""}
            </span>
          </div>

          {/* Grid */}
          <div className="flex-1 overflow-y-auto custom-scrollbar p-4 pt-2">
            {!schemasReady ? (
              <div className="grid grid-cols-4 gap-2">
                {Array.from({ length: 8 }).map((_, i) => (
                  <div key={i} className="flex flex-col items-center p-3 rounded-xl animate-pulse">
                    <div className="w-10 h-10 rounded-xl bg-orbflow-surface-hover mb-2" />
                    <div className="w-14 h-3 rounded bg-orbflow-surface-hover mb-1" />
                    <div className="w-10 h-2 rounded bg-orbflow-surface-hover" />
                  </div>
                ))}
              </div>
            ) : filtered.length > 0 ? (
              <>
                {/* Recent nodes section */}
                {recentSchemas.length > 0 && !search.trim() && (
                  <div className="mb-3">
                    <div className="text-caption font-semibold text-orbflow-text-faint uppercase tracking-[0.12em] mb-2 px-1">
                      Recent
                    </div>
                    <div className="flex gap-1.5 flex-wrap">
                      {recentSchemas.map((schema) => (
                        <button
                          key={`recent-${schema.pluginRef}`}
                          onClick={() => handleSelect(schema.pluginRef)}
                          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-body-sm font-medium
                            border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary
                            hover:bg-orbflow-surface-hover hover:border-orbflow-border-hover transition-all
                            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                        >
                          <NodeIcon
                            name={schema.icon || "default"}
                            className="w-3 h-3"
                            style={{ color: schema.color || "var(--orbflow-text-faint)" }}
                          />
                          {schema.name}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
                <div ref={gridRef} id="node-picker-grid" role="listbox" aria-label="Available nodes" className="grid grid-cols-4 gap-2">
                  {filtered.map((schema, i) => (
                    <NodeCard
                      key={schema.pluginRef}
                      schema={schema}
                      focused={i === focusIndex}
                      onSelect={() => handleSelect(schema.pluginRef)}
                      onHover={() => setFocusIndex(i)}
                    />
                  ))}
                </div>
              </>
            ) : (
              <div className="flex flex-col items-center justify-center py-16">
                <NodeIcon name="search" className="w-8 h-8 text-orbflow-text-ghost mb-3" />
                <p className="text-body-lg text-orbflow-text-faint">
                  No nodes match &ldquo;{search}&rdquo;
                </p>
                <p className="text-body-sm text-orbflow-text-ghost mt-1">
                  Try searching by category or a different term
                </p>
              </div>
            )}
          </div>

          {/* Keyboard hints */}
          <div className="px-4 py-2 border-t border-orbflow-border flex items-center justify-center gap-3">
            <span className="text-caption text-orbflow-text-ghost flex items-center gap-1">
              <kbd className="px-1 py-0.5 rounded border border-orbflow-border bg-orbflow-add-btn-bg text-micro font-mono">↑↓←→</kbd>
              Navigate
            </span>
            <span className="text-caption text-orbflow-text-ghost flex items-center gap-1">
              <kbd className="px-1 py-0.5 rounded border border-orbflow-border bg-orbflow-add-btn-bg text-micro font-mono">Enter</kbd>
              Select
            </span>
            <span className="text-caption text-orbflow-text-ghost flex items-center gap-1">
              <kbd className="px-1 py-0.5 rounded border border-orbflow-border bg-orbflow-add-btn-bg text-micro font-mono">Esc</kbd>
              Close
            </span>
          </div>
        </div>
      </div>
    </>,
    document.body
  );
}

function NodeCard({
  schema,
  focused,
  onSelect,
  onHover,
}: {
  schema: NodeTypeDefinition;
  focused: boolean;
  onSelect: () => void;
  onHover: () => void;
}) {
  return (
    <button
      id={`node-option-${schema.pluginRef}`}
      role="option"
      aria-selected={focused}
      onClick={onSelect}
      onMouseEnter={onHover}
      className={cn(
        "flex flex-col items-center text-center p-3 rounded-xl transition-all duration-150",
        "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
        focused
          ? "bg-electric-indigo/10 ring-1 ring-electric-indigo/20"
          : "hover:bg-orbflow-surface-hover",
      )}
    >
      <div
        className="w-10 h-10 rounded-xl flex items-center justify-center mb-2"
        style={{ backgroundColor: schema.color ? `${schema.color}15` : "var(--orbflow-add-btn-bg)" }}
      >
        <NodeIcon
          name={schema.icon || "default"}
          className="w-4.5 h-4.5"
          style={{ color: schema.color || "var(--orbflow-text-faint)" }}
        />
      </div>
      <span className="text-body font-medium text-orbflow-text-secondary leading-tight">
        {schema.name}
      </span>
      <span className="text-caption text-orbflow-text-faint mt-0.5 line-clamp-2 leading-tight">
        {schema.description}
      </span>
    </button>
  );
}
