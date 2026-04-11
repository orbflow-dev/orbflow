"use client";

import { useState, useEffect, useRef, useId, useMemo } from "react";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { NodeIcon } from "./icons";

const IS_MAC =
  typeof navigator !== "undefined" && /Mac/.test(navigator.userAgent);

function platformKey(key: string): string {
  if (!IS_MAC) return key;
  switch (key) {
    case "Ctrl": return "\u2318";
    case "Alt": return "\u2325";
    case "Shift": return "\u21E7";
    case "Del": return "\u232B";
    default: return key;
  }
}

const SHORTCUT_GROUPS = [
  {
    label: "General",
    shortcuts: [
      { keys: ["Ctrl", "S"], description: "Save workflow" },
      { keys: ["Ctrl", "Enter"], description: "Run workflow" },
      { keys: ["Escape"], description: "Deselect / Close panel" },
      { keys: ["?"], description: "Show this help" },
    ],
  },
  {
    label: "Editing",
    shortcuts: [
      { keys: ["Ctrl", "Z"], description: "Undo" },
      { keys: ["Ctrl", "Shift", "Z"], description: "Redo" },
      { keys: ["Ctrl", "D"], description: "Duplicate selected" },
      { keys: ["Ctrl", "C"], description: "Copy selected" },
      { keys: ["Ctrl", "V"], description: "Paste" },
      { keys: ["Ctrl", "X"], description: "Cut selected" },
      { keys: ["Ctrl", "A"], description: "Select all" },
      { keys: ["Del"], description: "Delete selected" },
    ],
  },
  {
    label: "Canvas",
    shortcuts: [
      { keys: ["Scroll"], description: "Zoom in / out" },
      { keys: ["Drag"], description: "Pan canvas" },
      { keys: ["Shift", "Drag"], description: "Area select" },
      { keys: ["Shift", "Click"], description: "Add to selection" },
      { keys: ["Ctrl", "F"], description: "Search nodes" },
      { keys: ["Ctrl", "G"], description: "Toggle snap to grid" },
      { keys: ["Right Click"], description: "Context menu" },
    ],
  },
];

interface ShortcutHelpProps {
  onClose: () => void;
}

export function ShortcutHelp({ onClose }: ShortcutHelpProps) {
  const ref = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const [search, setSearch] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);

  useFocusTrap(ref);

  useEffect(() => {
    searchRef.current?.focus();
  }, []);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || (e.key === "?" && document.activeElement !== searchRef.current)) {
        e.preventDefault();
        onClose();
      }
    };
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as HTMLElement)) {
        onClose();
      }
    };
    document.addEventListener("keydown", handleKey);
    document.addEventListener("mousedown", handleClick);
    return () => {
      document.removeEventListener("keydown", handleKey);
      document.removeEventListener("mousedown", handleClick);
    };
  }, [onClose]);

  const filteredGroups = useMemo(() => {
    if (!search.trim()) return SHORTCUT_GROUPS;
    const q = search.toLowerCase();
    return SHORTCUT_GROUPS.map((group) => ({
      ...group,
      shortcuts: group.shortcuts.filter(
        (s) =>
          s.description.toLowerCase().includes(q) ||
          s.keys.some((k) => k.toLowerCase().includes(q))
      ),
    })).filter((group) => group.shortcuts.length > 0);
  }, [search]);

  return (
    <div className="fixed inset-0 z-[80] flex items-center justify-center bg-orbflow-backdrop backdrop-blur-sm animate-fade-in">
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="w-full max-w-md rounded-2xl border border-orbflow-border bg-orbflow-surface/95 backdrop-blur-xl shadow-2xl animate-scale-in overflow-hidden"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-orbflow-border">
          <div className="flex items-center gap-2.5">
            <NodeIcon name="help-circle" className="w-4 h-4 text-electric-indigo" />
            <h2 id={titleId} className="text-sm font-semibold text-orbflow-text-secondary">Keyboard Shortcuts</h2>
          </div>
          <button
            onClick={onClose}
            className="w-7 h-7 rounded-lg flex items-center justify-center text-orbflow-text-ghost hover:text-orbflow-text-muted hover:bg-orbflow-surface-hover transition-all
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            aria-label="Close shortcuts"
          >
            <NodeIcon name="x" className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Search */}
        <div className="px-6 pt-4 pb-2">
          <div className="relative">
            <NodeIcon name="search" className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3 h-3 pointer-events-none text-orbflow-text-ghost" />
            <input
              ref={searchRef}
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Filter shortcuts..."
              className="w-full rounded-lg pl-8 pr-3 py-1.5 text-body placeholder:text-orbflow-text-ghost
                focus:outline-none focus:border-electric-indigo/30
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50
                transition-all border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary"
            />
          </div>
        </div>

        {/* Content */}
        <div className="px-6 py-3 space-y-5 max-h-[60vh] overflow-y-auto custom-scrollbar">
          {filteredGroups.length === 0 ? (
            <p className="text-body text-orbflow-text-faint text-center py-6">No shortcuts match &ldquo;{search}&rdquo;</p>
          ) : (
            filteredGroups.map((group) => (
              <div key={group.label}>
                <h3 className="text-caption font-semibold text-orbflow-text-faint uppercase tracking-[0.15em] mb-2.5">
                  {group.label}
                </h3>
                <div className="space-y-1.5">
                  {group.shortcuts.map((shortcut) => (
                    <div
                      key={shortcut.description}
                      className="flex items-center justify-between py-1"
                    >
                      <span className="text-body-lg text-orbflow-text-muted">{shortcut.description}</span>
                      <div className="flex items-center gap-1">
                        {shortcut.keys.map((key) => (
                          <kbd
                            key={key}
                            className="inline-flex items-center justify-center min-w-[24px] h-6 px-1.5 rounded-md
                              border border-orbflow-border bg-orbflow-add-btn-bg text-body-sm font-mono text-orbflow-text-muted"
                          >
                            {platformKey(key)}
                          </kbd>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-3 border-t border-orbflow-border text-center">
          <span className="text-body-sm text-orbflow-text-ghost">
            Press <kbd className="px-1 py-0.5 rounded border border-orbflow-border bg-orbflow-add-btn-bg text-caption font-mono text-orbflow-text-muted">?</kbd> or <kbd className="px-1 py-0.5 rounded border border-orbflow-border bg-orbflow-add-btn-bg text-caption font-mono text-orbflow-text-muted">Esc</kbd> to close
          </span>
        </div>
      </div>
    </div>
  );
}
