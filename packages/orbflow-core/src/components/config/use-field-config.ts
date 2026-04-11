"use client";

import { useMemo, useCallback } from "react";
import { usePanelStore } from "../../stores/panel-store";
import { useCanvasStore } from "../../stores/canvas-store";
import type { FieldMapping } from "../../types/schema";

export interface FieldConfigState {
  /** Current mapping mode */
  mode: "static" | "expression";
  /** Static value (when mode is "static") */
  staticValue: unknown;
  /** CEL expression string (when mode is "expression") */
  celExpression: string;
  /** Whether field is connected via a capability edge */
  isWired: boolean;
  /** Source of wire connection if wired */
  wireSource: { sourceNodeId: string; sourceField: string } | null;
  /** Set the field mode */
  setMode: (mode: "static" | "expression") => void;
  /** Set a static value */
  setStaticValue: (value: unknown) => void;
  /** Set a CEL expression */
  setCelExpression: (expr: string) => void;
  /** Toggle between static and expression mode */
  toggleMode: () => void;
}

/**
 * Extracts field-level state management for a single input mapping.
 * Wraps panel-store operations for a given node + field combination.
 */
export function useFieldConfig(
  nodeId: string,
  fieldKey: string
): FieldConfigState {
  const getNodeMappings = usePanelStore((s) => s.getNodeMappings);
  const setInputMapping = usePanelStore((s) => s.setInputMapping);
  const capabilityEdges = useCanvasStore((s) => s.capabilityEdges);

  const mapping: FieldMapping | undefined = useMemo(
    () => getNodeMappings(nodeId)[fieldKey],
    [getNodeMappings, nodeId, fieldKey]
  );

  const mode: "static" | "expression" = useMemo(() => {
    if (!mapping) return "static";
    return mapping.mode === "expression" ? "expression" : "static";
  }, [mapping]);

  const staticValue: unknown = mapping?.staticValue;

  const celExpression: string = mapping?.celExpression ?? "";

  // Check capability edges for a wire targeting this node + field
  const wireInfo = useMemo(() => {
    const capEdge = capabilityEdges.find(
      (e) => e.targetNodeId === nodeId && e.targetPortKey === fieldKey
    );
    if (!capEdge) return null;
    return {
      sourceNodeId: capEdge.sourceNodeId,
      sourceField: capEdge.targetPortKey,
    };
  }, [capabilityEdges, nodeId, fieldKey]);

  const isWired = wireInfo !== null;
  const wireSource = wireInfo;

  const setMode = useCallback(
    (newMode: "static" | "expression") => {
      const updated: FieldMapping = {
        targetKey: fieldKey,
        mode: newMode,
        staticValue: mapping?.staticValue,
        sourceNodeId: mapping?.sourceNodeId,
        sourcePath: mapping?.sourcePath,
        celExpression: mapping?.celExpression,
      };
      setInputMapping(nodeId, fieldKey, updated);
    },
    [nodeId, fieldKey, mapping, setInputMapping]
  );

  const setStaticValue = useCallback(
    (value: unknown) => {
      setInputMapping(nodeId, fieldKey, {
        targetKey: fieldKey,
        mode: "static",
        staticValue: value,
      });
    },
    [nodeId, fieldKey, setInputMapping]
  );

  const setCelExpression = useCallback(
    (expr: string) => {
      setInputMapping(nodeId, fieldKey, {
        targetKey: fieldKey,
        mode: "expression",
        celExpression: expr,
      });
    },
    [nodeId, fieldKey, setInputMapping]
  );

  const toggleMode = useCallback(() => {
    const newMode = mode === "static" ? "expression" : "static";
    setMode(newMode);
  }, [mode, setMode]);

  return {
    mode,
    staticValue,
    celExpression,
    isWired,
    wireSource,
    setMode,
    setStaticValue,
    setCelExpression,
    toggleMode,
  };
}
