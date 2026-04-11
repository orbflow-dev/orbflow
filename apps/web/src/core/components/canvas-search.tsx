"use client";

import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { useReactFlow } from "@xyflow/react";
import { cn } from "../utils/cn";
import { NodeIcon } from "./icons";
import { useCanvasStore } from "@orbflow/core/stores";

interface CanvasSearchProps {
  onClose: () => void;
}

export function CanvasSearch({ onClose }: CanvasSearchProps) {
  const [query, setQuery] = useState("");
  const [currentIndex, setCurrentIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const { nodes, selectNode } = useCanvasStore();
  const { setCenter } = useReactFlow();

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const matches = useMemo(() => {
    if (!query.trim()) return [];
    const q = query.toLowerCase();
    return nodes.filter((n) => {
      const label = ((n.data?.label as string) || "").toLowerCase();
      const pluginRef = ((n.data?.pluginRef as string) || "").toLowerCase();
      const type = ((n.data?.type as string) || "").toLowerCase();
      return label.includes(q) || pluginRef.includes(q) || type.includes(q);
    });
  }, [query, nodes]);

  const focusMatch = useCallback(
    (index: number) => {
      if (matches.length === 0) return;
      const clamped = ((index % matches.length) + matches.length) % matches.length;
      setCurrentIndex(clamped);
      const node = matches[clamped];
      selectNode(node.id);
      const w = node.measured?.width ?? 64;
      const h = node.measured?.height ?? 64;
      setCenter(node.position.x + w / 2, node.position.y + h / 2, { zoom: 1.2, duration: 300 });
    },
    [matches, selectNode, setCenter],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        if (e.shiftKey) {
          focusMatch(currentIndex - 1);
        } else {
          focusMatch(currentIndex + 1);
        }
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        focusMatch(currentIndex + 1);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        focusMatch(currentIndex - 1);
      }
    },
    [onClose, focusMatch, currentIndex],
  );

  // Re-focus first match when query changes and there are matches (0->N or count changes)
  const prevQueryRef = useRef("");
  useEffect(() => {
    const prevQuery = prevQueryRef.current;
    prevQueryRef.current = query;
    if (matches.length > 0 && query !== prevQuery) {
      focusMatch(0);
    }
  }, [matches, query, focusMatch]);

  return (
    <div className="absolute top-4 right-4 z-20 animate-fade-in-up">
      <div className="flex items-center gap-1 rounded-xl backdrop-blur-xl shadow-lg bg-orbflow-glass-bg border border-orbflow-border px-2 py-1">
        <NodeIcon name="search" className="w-3.5 h-3.5 text-orbflow-text-ghost shrink-0" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => { setQuery(e.target.value); setCurrentIndex(0); }}
          onKeyDown={handleKeyDown}
          placeholder="Find nodes..."
          className="w-48 bg-transparent text-body-lg text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
            outline-none px-1 py-0.5"
        />

        {query.trim() && (
          <span
            role="status"
            aria-live="polite"
            className="text-body-sm font-mono text-orbflow-text-faint whitespace-nowrap px-1"
          >
            {matches.length === 0
              ? "0 results"
              : `${currentIndex + 1} / ${matches.length}`}
          </span>
        )}

        {matches.length > 1 && (
          <div className="flex items-center gap-0.5">
            <button
              onClick={() => focusMatch(currentIndex - 1)}
              className="w-5 h-5 rounded flex items-center justify-center text-orbflow-text-muted
                hover:bg-orbflow-controls-btn-hover transition-colors
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
              aria-label="Previous match"
            >
              <NodeIcon name="chevron-up" className="w-3 h-3" />
            </button>
            <button
              onClick={() => focusMatch(currentIndex + 1)}
              className="w-5 h-5 rounded flex items-center justify-center text-orbflow-text-muted
                hover:bg-orbflow-controls-btn-hover transition-colors
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
              aria-label="Next match"
            >
              <NodeIcon name="chevron-down" className="w-3 h-3" />
            </button>
          </div>
        )}

        <button
          onClick={onClose}
          className="w-5 h-5 rounded flex items-center justify-center text-orbflow-text-ghost
            hover:text-orbflow-text-muted hover:bg-orbflow-controls-btn-hover transition-colors ml-0.5
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          aria-label="Close search"
        >
          <NodeIcon name="x" className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}
