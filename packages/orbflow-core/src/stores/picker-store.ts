import { create } from "zustand";
import type { NodeKind } from "../types/schema";

interface PickerStore {
  open: boolean;
  position: { x: number; y: number };
  sourceNodeId?: string;
  sourceEdgeId?: string;
  /** When set, only these node kinds are shown in the picker */
  allowedKinds?: NodeKind[];

  openPicker: (
    pos: { x: number; y: number },
    sourceNodeId?: string,
    sourceEdgeId?: string,
    allowedKinds?: NodeKind[]
  ) => void;
  closePicker: () => void;
}

export const usePickerStore = create<PickerStore>((set) => ({
  open: false,
  position: { x: 0, y: 0 },
  sourceNodeId: undefined,
  sourceEdgeId: undefined,
  allowedKinds: undefined,

  openPicker: (pos, sourceNodeId, sourceEdgeId, allowedKinds) =>
    set({
      open: true,
      position: pos,
      sourceNodeId,
      sourceEdgeId,
      // When inserting from node/edge, only allow actions
      allowedKinds: allowedKinds ?? (sourceNodeId || sourceEdgeId ? ["action"] : undefined),
    }),

  closePicker: () =>
    set({
      open: false,
      sourceNodeId: undefined,
      sourceEdgeId: undefined,
      allowedKinds: undefined,
    }),
}));
