"use client";
import { useCallback } from "react";
import { useHistoryStore } from "../stores/history-store";
import { useCanvasStore } from "../stores/canvas-store";

export function useHistory() {
  const canUndo = useHistoryStore((s) => s.canUndo);
  const canRedo = useHistoryStore((s) => s.canRedo);
  const historyUndo = useHistoryStore((s) => s.undo);
  const historyRedo = useHistoryStore((s) => s.redo);
  const pushSnapshot = useHistoryStore((s) => s.push);
  const nodes = useCanvasStore((s) => s.nodes);
  const edges = useCanvasStore((s) => s.edges);

  const undo = useCallback(() => {
    historyUndo({ nodes, edges });
  }, [historyUndo, nodes, edges]);

  const redo = useCallback(() => {
    historyRedo({ nodes, edges });
  }, [historyRedo, nodes, edges]);

  return { canUndo, canRedo, undo, redo, pushSnapshot };
}
