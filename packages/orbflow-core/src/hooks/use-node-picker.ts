"use client";
import { useCallback } from "react";
import { usePickerStore } from "../stores/picker-store";
import type { NodeSchemaRegistry } from "../schemas/registry";
import type { NodeTypeDefinition, NodeKind } from "../types/schema";

export interface NodePickerState {
  open: boolean;
  position: { x: number; y: number };
  sourceNodeId?: string;
  sourceEdgeId?: string;
  allowedKinds?: NodeKind[];
  openPicker: (
    pos: { x: number; y: number },
    sourceNodeId?: string,
    sourceEdgeId?: string,
    allowedKinds?: NodeKind[],
  ) => void;
  closePicker: () => void;
  filteredSchemas: (searchQuery: string) => NodeTypeDefinition[];
}

export function useNodePicker(registry?: NodeSchemaRegistry): NodePickerState {
  const open = usePickerStore((s) => s.open);
  const position = usePickerStore((s) => s.position);
  const sourceNodeId = usePickerStore((s) => s.sourceNodeId);
  const sourceEdgeId = usePickerStore((s) => s.sourceEdgeId);
  const allowedKinds = usePickerStore((s) => s.allowedKinds);
  const openPicker = usePickerStore((s) => s.openPicker);
  const closePicker = usePickerStore((s) => s.closePicker);

  const filteredSchemas = useCallback(
    (searchQuery: string) => {
      if (!registry) return [];
      let schemas = registry.getAll();
      if (allowedKinds) {
        const kinds = allowedKinds;
        schemas = schemas.filter((s) =>
          kinds.includes(s.nodeKind || "action")
        );
      }
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase();
        schemas = schemas.filter(
          (s) =>
            s.name.toLowerCase().includes(q) ||
            (s.description || "").toLowerCase().includes(q)
        );
      }
      return schemas;
    },
    [registry, allowedKinds]
  );

  return {
    open,
    position,
    sourceNodeId,
    sourceEdgeId,
    allowedKinds,
    openPicker,
    closePicker,
    filteredSchemas,
  };
}
