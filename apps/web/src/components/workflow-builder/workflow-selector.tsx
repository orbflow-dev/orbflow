"use client";

import { useState, useMemo, useEffect, useRef } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import type { Workflow } from "@/lib/api";

interface WorkflowSelectorProps {
  workflows: Workflow[];
  selectedWorkflow: Workflow | null;
  onSelect: (id: string) => void;
  onImport: () => void;
  onExport: () => void;
}

export function WorkflowSelector({
  workflows,
  selectedWorkflow,
  onSelect,
  onImport,
  onExport,
}: WorkflowSelectorProps) {
  const [search, setSearch] = useState("");
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const filteredWorkflows = useMemo(() => {
    if (!search.trim()) return workflows;
    const q = search.toLowerCase();
    return workflows.filter((wf) => wf.name.toLowerCase().includes(q));
  }, [workflows, search]);

  // Close dropdown on outside click
  useEffect(() => {
    if (!dropdownOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as HTMLElement)) {
        setDropdownOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [dropdownOpen]);

  const handleSelect = (value: string) => {
    onSelect(value);
    setDropdownOpen(false);
    setSearch("");
  };

  return (
    <div className="absolute bottom-5 left-1/2 -translate-x-1/2 z-20" ref={dropdownRef}>
      <div className="flex items-center gap-1 px-2 py-1.5 rounded-xl backdrop-blur-md shadow-xl bg-orbflow-glass-bg border border-orbflow-border">
        {/* Import/Export buttons */}
        <button
          onClick={onImport}
          className="w-7 h-7 rounded-lg flex items-center justify-center hover:bg-orbflow-surface-hover transition-all text-orbflow-text-faint
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          title="Import workflow from JSON"
          aria-label="Import workflow from JSON"
        >
          <NodeIcon name="upload" className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={onExport}
          className="w-7 h-7 rounded-lg flex items-center justify-center hover:bg-orbflow-surface-hover transition-all text-orbflow-text-faint
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          title="Export workflow as JSON"
          aria-label="Export workflow as JSON"
        >
          <NodeIcon name="download" className="w-3.5 h-3.5" />
        </button>

        <div className="w-px h-5 mx-1 bg-orbflow-border" />

        {/* Selector */}
        <button
          onClick={() => setDropdownOpen(!dropdownOpen)}
          className="flex items-center gap-2 px-2.5 py-1 rounded-lg hover:bg-orbflow-surface-hover transition-colors
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          aria-label="Select workflow"
          aria-expanded={dropdownOpen}
          aria-haspopup="true"
        >
          <NodeIcon name="layers" className="w-3.5 h-3.5 text-orbflow-text-faint" />
          <span className="text-xs font-medium max-w-[180px] truncate text-orbflow-text-secondary">
            {selectedWorkflow?.name || "New workflow"}
          </span>
          <NodeIcon name="chevron-down" className="w-3 h-3 text-orbflow-text-faint" />
        </button>
      </div>

      {/* Dropdown */}
      {dropdownOpen && (
        <div className="absolute bottom-full mb-2 left-1/2 -translate-x-1/2 w-72 rounded-xl backdrop-blur-xl shadow-2xl animate-scale-in overflow-hidden bg-orbflow-glass-bg border border-orbflow-border">
          {/* Search */}
          <div className="p-2.5 border-b border-orbflow-border">
            <div className="relative">
              <NodeIcon
                name="search"
                className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3 h-3 pointer-events-none text-orbflow-text-faint"
              />
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search workflows..."
                className="w-full rounded-lg pl-8 pr-3 py-1.5
                  text-body placeholder:text-[var(--orbflow-text-faint)] focus:border-electric-indigo/30
                  focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none transition-all
                  border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary"
                autoFocus
              />
            </div>
          </div>

          {/* Options */}
          <div className="max-h-52 overflow-y-auto custom-scrollbar py-1">
            <button
              onClick={() => handleSelect("")}
              className={cn(
                "w-full flex items-center gap-2.5 px-3.5 py-2 text-left transition-colors",
                !selectedWorkflow
                  ? "bg-electric-indigo/10 text-electric-indigo"
                  : "hover:bg-orbflow-surface-hover text-orbflow-text-muted"
              )}
            >
              <NodeIcon name="plus" className="w-3 h-3" />
              <span className="text-body font-medium">New workflow</span>
            </button>
            {filteredWorkflows.map((wf) => (
              <button
                key={wf.id}
                onClick={() => handleSelect(wf.id)}
                className={cn(
                  "w-full flex items-center gap-2.5 px-3.5 py-2 text-left transition-colors",
                  selectedWorkflow?.id === wf.id
                    ? "bg-electric-indigo/10 text-electric-indigo"
                    : "hover:bg-orbflow-surface-hover text-orbflow-text-muted"
                )}
              >
                <NodeIcon name="workflow" className="w-3 h-3 text-orbflow-text-faint" />
                <span className="text-body font-medium truncate">{wf.name}</span>
              </button>
            ))}
            {filteredWorkflows.length === 0 && search && (
              <div className="px-3.5 py-4 text-center">
                <span className="text-body-sm text-orbflow-text-faint">No workflows match &ldquo;{search}&rdquo;</span>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
