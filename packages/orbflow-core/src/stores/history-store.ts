import { create } from "zustand";
import type { Node, Edge } from "@xyflow/react";

interface Snapshot {
  nodes: Node[];
  edges: Edge[];
}

interface HistoryStore {
  past: Snapshot[];
  future: Snapshot[];
  isDirty: boolean;
  /** Take a snapshot of the current canvas state before a mutation. */
  push: (snapshot: Snapshot) => void;
  /** Undo — returns the previous snapshot, or null if nothing to undo. */
  undo: (current: Snapshot) => Snapshot | null;
  /** Redo — returns the next snapshot, or null if nothing to redo. */
  redo: (current: Snapshot) => Snapshot | null;
  canUndo: () => boolean;
  canRedo: () => boolean;
  /** Mark the canvas as clean (e.g. after a successful save). */
  markClean: () => void;
  clear: () => void;
}

const MAX_HISTORY = 50;

export const useHistoryStore = create<HistoryStore>((set, get) => ({
  past: [],
  future: [],
  isDirty: false,

  push: (snapshot) =>
    set((s) => ({
      past: [...s.past.slice(-(MAX_HISTORY - 1)), snapshot],
      future: [], // Clear redo stack on new action
      isDirty: true,
    })),

  undo: (current) => {
    const { past } = get();
    if (past.length === 0) return null;
    const previous = past[past.length - 1];
    set((s) => ({
      past: s.past.slice(0, -1),
      future: [current, ...s.future],
    }));
    return previous;
  },

  redo: (current) => {
    const { future } = get();
    if (future.length === 0) return null;
    const next = future[0];
    set((s) => ({
      past: [...s.past, current],
      future: s.future.slice(1),
    }));
    return next;
  },

  canUndo: () => get().past.length > 0,
  canRedo: () => get().future.length > 0,
  markClean: () => set({ isDirty: false }),
  clear: () => set({ past: [], future: [], isDirty: false }),
}));
