import { useState, useCallback, useEffect, type KeyboardEvent } from "react";

export interface UseGridKeyboardNavOptions {
  /** Total number of items in the grid */
  itemCount: number;
  /** Number of columns in the grid layout */
  columns: number;
  /** Called when Enter is pressed on focused item */
  onSelect: (index: number) => void;
  /** Called when Escape is pressed */
  onEscape?: () => void;
  /** Whether keyboard navigation is active */
  enabled?: boolean;
}

export interface GridKeyboardNavResult {
  /** Currently focused item index (-1 for none) */
  focusedIndex: number;
  /** Manually set focused index */
  setFocusedIndex: (index: number) => void;
  /** Key down handler to attach to the container element */
  handleKeyDown: (e: KeyboardEvent) => void;
}

export function useGridKeyboardNav(
  options: UseGridKeyboardNavOptions,
): GridKeyboardNavResult {
  const { itemCount, columns, onSelect, onEscape, enabled = true } = options;
  const [focusedIndex, setFocusedIndex] = useState(-1);

  // Reset focusedIndex when disabled or itemCount changes
  useEffect(() => {
    if (!enabled) {
      setFocusedIndex(-1);
    }
  }, [enabled]);

  useEffect(() => {
    setFocusedIndex(-1);
  }, [itemCount]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!enabled || itemCount === 0) return;

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          setFocusedIndex((prev) => Math.min(prev + columns, itemCount - 1));
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          setFocusedIndex((prev) => Math.max(prev - columns, 0));
          break;
        }
        case "ArrowRight": {
          e.preventDefault();
          setFocusedIndex((prev) => Math.min(prev + 1, itemCount - 1));
          break;
        }
        case "ArrowLeft": {
          e.preventDefault();
          setFocusedIndex((prev) => Math.max(prev - 1, 0));
          break;
        }
        case "Enter": {
          e.preventDefault();
          if (focusedIndex >= 0) {
            onSelect(focusedIndex);
          }
          break;
        }
        case "Escape": {
          e.preventDefault();
          onEscape?.();
          break;
        }
        default:
          break;
      }
    },
    [enabled, itemCount, columns, focusedIndex, onSelect, onEscape],
  );

  return {
    focusedIndex,
    setFocusedIndex,
    handleKeyDown,
  };
}
