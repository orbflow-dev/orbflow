"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { cn } from "../utils/cn";
import { NodeIcon } from "./icons";

export interface ContextMenuItem {
  label: string;
  icon: string;
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  onClick: () => void;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [focusIndex, setFocusIndex] = useState(-1);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const enabledIndices = items
    .map((item, i) => (item.disabled ? -1 : i))
    .filter((i) => i !== -1);

  const focusItem = useCallback(
    (index: number) => {
      setFocusIndex(index);
      itemRefs.current[index]?.focus();
    },
    []
  );

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as HTMLElement)) {
        onClose();
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }

      if (e.key === "ArrowDown" || e.key === "Tab" && !e.shiftKey) {
        e.preventDefault();
        const currentPos = enabledIndices.indexOf(focusIndex);
        const next = currentPos < enabledIndices.length - 1 ? currentPos + 1 : 0;
        focusItem(enabledIndices[next]);
        return;
      }

      if (e.key === "ArrowUp" || e.key === "Tab" && e.shiftKey) {
        e.preventDefault();
        const currentPos = enabledIndices.indexOf(focusIndex);
        const prev = currentPos > 0 ? currentPos - 1 : enabledIndices.length - 1;
        focusItem(enabledIndices[prev]);
        return;
      }

      if (e.key === "Home") {
        e.preventDefault();
        if (enabledIndices.length > 0) focusItem(enabledIndices[0]);
        return;
      }

      if (e.key === "End") {
        e.preventDefault();
        if (enabledIndices.length > 0) focusItem(enabledIndices[enabledIndices.length - 1]);
        return;
      }

      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        if (focusIndex >= 0 && !items[focusIndex]?.disabled) {
          items[focusIndex].onClick();
          onClose();
        }
        return;
      }
    };

    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onClose, focusIndex, enabledIndices, items, focusItem]);

  // Auto-focus first enabled item
  useEffect(() => {
    if (enabledIndices.length > 0) {
      focusItem(enabledIndices[0]);
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Adjust position to stay within viewport
  const adjustedX = Math.min(x, window.innerWidth - 220);
  const adjustedY = Math.min(y, window.innerHeight - items.length * 40 - 20);

  return (
    <div
      ref={ref}
      className="fixed z-[60] animate-scale-in"
      style={{ left: adjustedX, top: adjustedY }}
      role="menu"
      aria-label="Context menu"
    >
      <div className="min-w-[180px] rounded-xl backdrop-blur-xl shadow-2xl py-1.5 overflow-hidden border border-orbflow-border bg-orbflow-glass-bg">
        {items.map((item, i) => (
          <button
            key={i}
            ref={(el) => { itemRefs.current[i] = el; }}
            role="menuitem"
            tabIndex={focusIndex === i ? 0 : -1}
            onClick={() => {
              if (!item.disabled) {
                item.onClick();
                onClose();
              }
            }}
            onMouseEnter={() => setFocusIndex(i)}
            disabled={item.disabled}
            className={cn(
              "w-full flex items-center gap-2.5 px-3.5 py-2 text-left transition-colors duration-100 outline-none",
              item.disabled
                ? "opacity-30 cursor-not-allowed"
                : item.danger
                  ? "text-red-400 hover:bg-red-500/10 focus-visible:bg-red-500/10"
                  : "hover:bg-orbflow-surface-hover focus-visible:bg-orbflow-surface-hover",
            )}
          >
            <NodeIcon
              name={item.icon}
              className={cn("w-3.5 h-3.5 shrink-0", !item.danger && "text-orbflow-text-faint")}
            />
            <span className={cn("text-body-lg font-medium flex-1", !item.danger && "text-orbflow-text-secondary")}>{item.label}</span>
            {item.shortcut && (
              <span className="text-body-sm font-mono ml-2 text-orbflow-text-faint">
                {item.shortcut}
              </span>
            )}
          </button>
        ))}
      </div>
    </div>
  );
}
