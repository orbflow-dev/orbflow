import { useEffect, useRef } from "react";

interface KeyboardShortcutHandlers {
  onDelete: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
  onRun: () => void;
  onDuplicate: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onCut: () => void;
  onSelectAll: () => void;
  onSearch: () => void;
  onToggleGrid: () => void;
  onEscape: () => void;
  onToggleShortcuts: () => void;
}

export function useKeyboardShortcuts(handlers: KeyboardShortcutHandlers) {
  // Ref pattern: always use latest handlers without re-registering the listener
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        tag === "SELECT" ||
        target.isContentEditable
      ) return;

      const h = handlersRef.current;

      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        h.onDelete();
        return;
      }
      // Ctrl+C -- Copy
      if ((e.ctrlKey || e.metaKey) && e.key === "c" && !e.shiftKey) {
        e.preventDefault();
        h.onCopy();
        return;
      }
      // Ctrl+V -- Paste
      if ((e.ctrlKey || e.metaKey) && e.key === "v" && !e.shiftKey) {
        e.preventDefault();
        h.onPaste();
        return;
      }
      // Ctrl+X -- Cut
      if ((e.ctrlKey || e.metaKey) && e.key === "x" && !e.shiftKey) {
        e.preventDefault();
        h.onCut();
        return;
      }
      // Ctrl+A -- Select All
      if ((e.ctrlKey || e.metaKey) && e.key === "a") {
        e.preventDefault();
        h.onSelectAll();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "z" && !e.shiftKey) {
        e.preventDefault();
        h.onUndo();
        return;
      }
      if (
        ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "Z") ||
        ((e.ctrlKey || e.metaKey) && e.key === "y")
      ) {
        e.preventDefault();
        h.onRedo();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        h.onSave();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        h.onRun();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "d") {
        e.preventDefault();
        h.onDuplicate();
        return;
      }
      // Ctrl+F -- Search
      if ((e.ctrlKey || e.metaKey) && e.key === "f") {
        e.preventDefault();
        h.onSearch();
        return;
      }
      // Ctrl+G -- Toggle snap-to-grid
      if ((e.ctrlKey || e.metaKey) && e.key === "g") {
        e.preventDefault();
        h.onToggleGrid();
        return;
      }
      if (e.key === "Escape") {
        h.onEscape();
      }
      if (e.key === "?" && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        h.onToggleShortcuts();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []); // Single registration -- handlersRef always has latest
}
