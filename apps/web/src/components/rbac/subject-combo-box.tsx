"use client";

import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";

interface SubjectComboBoxProps {
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly suggestions: string[];
  readonly placeholder?: string;
  readonly className?: string;
  readonly autoFocus?: boolean;
  readonly id?: string;
  readonly onKeyDown?: (e: React.KeyboardEvent) => void;
}

export function SubjectComboBox({
  value,
  onChange,
  suggestions,
  placeholder,
  className,
  autoFocus,
  id,
  onKeyDown,
}: SubjectComboBoxProps) {
  const [open, setOpen] = useState(false);
  const [activeIdx, setActiveIdx] = useState(-1);
  const containerRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  const filtered = useMemo(() => {
    if (!value.trim()) return suggestions;
    const q = value.toLowerCase();
    return suggestions.filter((s) => s.toLowerCase().includes(q));
  }, [value, suggestions]);

  const showNew = value.trim() !== "" && !suggestions.some((s) => s.toLowerCase() === value.trim().toLowerCase());

  useEffect(() => {
    setActiveIdx(-1);
  }, [value]);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  useEffect(() => {
    if (activeIdx >= 0 && listRef.current) {
      const item = listRef.current.children[activeIdx] as HTMLElement | undefined;
      item?.scrollIntoView({ block: "nearest" });
    }
  }, [activeIdx]);

  const select = useCallback(
    (val: string) => {
      onChange(val);
      setOpen(false);
      setActiveIdx(-1);
    },
    [onChange],
  );

  const totalItems = filtered.length + (showNew ? 1 : 0);

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (!open) {
        setOpen(true);
      }
      setActiveIdx((prev) => (prev < totalItems - 1 ? prev + 1 : 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIdx((prev) => (prev > 0 ? prev - 1 : totalItems - 1));
    } else if (e.key === "Enter" && open && activeIdx >= 0) {
      e.preventDefault();
      e.stopPropagation();
      if (activeIdx < filtered.length) {
        select(filtered[activeIdx]);
      } else if (showNew) {
        select(value.trim());
      }
    } else if (e.key === "Escape" && open) {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      setActiveIdx(-1);
    } else {
      onKeyDown?.(e);
    }
  }

  const hasItems = filtered.length > 0 || showNew;

  return (
    <div ref={containerRef} className="relative">
      <input
        id={id}
        type="text"
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          if (!open) setOpen(true);
        }}
        onFocus={() => {
          if (hasItems) setOpen(true);
        }}
        placeholder={placeholder}
        className={className}
        autoFocus={autoFocus}
        onKeyDown={handleKeyDown}
        role="combobox"
        aria-expanded={open && hasItems}
        aria-autocomplete="list"
        aria-activedescendant={activeIdx >= 0 ? `subject-option-${activeIdx}` : undefined}
        autoComplete="off"
      />

      {open && hasItems && (
        <ul
          ref={listRef}
          role="listbox"
          className="absolute left-0 right-0 top-full mt-1 max-h-48 overflow-y-auto rounded-lg bg-orbflow-surface border border-orbflow-border shadow-lg z-10"
        >
          {filtered.map((s, i) => (
            <li
              key={s}
              id={`subject-option-${i}`}
              role="option"
              aria-selected={i === activeIdx}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => select(s)}
              onMouseEnter={() => setActiveIdx(i)}
              className={cn(
                "px-3 py-2 text-body-sm cursor-pointer transition-colors truncate",
                i === activeIdx
                  ? "bg-electric-indigo/10 text-electric-indigo"
                  : "text-orbflow-text-secondary hover:bg-orbflow-surface-hover",
              )}
            >
              {value.trim() ? highlightMatch(s, value.trim()) : s}
            </li>
          ))}
          {showNew && (
            <li
              id={`subject-option-${filtered.length}`}
              role="option"
              aria-selected={filtered.length === activeIdx}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => select(value.trim())}
              onMouseEnter={() => setActiveIdx(filtered.length)}
              className={cn(
                "px-3 py-2 text-body-sm cursor-pointer transition-colors flex items-center gap-1.5",
                filtered.length === activeIdx
                  ? "bg-electric-indigo/10 text-electric-indigo"
                  : "text-orbflow-text-faint hover:bg-orbflow-surface-hover",
              )}
            >
              <NodeIcon name="plus" className="w-3 h-3 shrink-0" />
              <span className="truncate">
                Add <span className="font-medium text-orbflow-text-secondary">{value.trim()}</span>
              </span>
            </li>
          )}
        </ul>
      )}
    </div>
  );
}

function highlightMatch(text: string, query: string): React.ReactNode {
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx < 0) return text;
  return (
    <>
      {text.slice(0, idx)}
      <span className="font-semibold text-electric-indigo">{text.slice(idx, idx + query.length)}</span>
      {text.slice(idx + query.length)}
    </>
  );
}
