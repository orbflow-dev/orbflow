/**
 * Hook that resolves all computed data for a graph node.
 *
 * Looks up the schema from the registry, determines the node kind,
 * computes display name, icon, color, handle configuration, and
 * merges in optional execution state.
 *
 * Shared logic extracted from both WorkflowNode and ExecutionNode.
 */

import { useMemo } from "react";
import { useOrbflow } from "../../context/orbflow-provider";
import type { NodeKind, NodeTypeDefinition } from "../../types/schema";
import type { ExecutionStatus } from "../../execution/execution-status";
import type { GraphCapabilityPort } from "./node-handles";

/* ── Output interface ─────────────────────────────────── */

export interface GraphNodeComputedData {
  /** The node's unique ID within the graph */
  nodeId: string;
  /** Resolved node kind: trigger, action, or capability */
  kind: NodeKind;
  /** Full schema definition (undefined if pluginRef not found) */
  schema: NodeTypeDefinition | undefined;
  /** Primary display name (schema name, label, or nodeId fallback) */
  displayName: string;
  /** Custom name set by user (empty string if same as displayName) */
  customName: string;
  /** Icon identifier from schema */
  icon: string;
  /** Hex color from schema */
  color: string;
  /** Whether this node should render an input (target) handle */
  hasInputHandle: boolean;
  /** Whether this node should render an output (source) handle */
  hasOutputHandle: boolean;
  /** Capability port definitions for action nodes */
  capabilityPorts: GraphCapabilityPort[];
  /** Whether handles accept new connections */
  isConnectable: boolean;
  /** Optional execution status (e.g. "running", "completed", "failed") */
  executionStatus?: ExecutionStatus;
  /** Optional error message from execution */
  error?: string;
  /** Optional execution duration in milliseconds */
  duration?: number;
  /** Whether the node is currently selected in the canvas */
  selected: boolean;
}

/* ── Options interface ────────────────────────────────── */

export interface UseGraphNodeDataOptions {
  /** Node ID */
  nodeId: string;
  /** Plugin reference to look up in the schema registry */
  pluginRef: string;
  /** User-defined label (may differ from schema name) */
  label?: string;
  /** Whether the node is selected */
  selected?: boolean;
  /** When true, handles are not connectable */
  readOnly?: boolean;
  /** Execution status from runtime state */
  executionStatus?: ExecutionStatus;
  /** Error message from execution */
  error?: string;
  /** Execution duration in milliseconds */
  duration?: number;
  /** Override kind (used when kind is known from data, e.g. execution viewer) */
  kind?: NodeKind;
}

/* ── Hook ─────────────────────────────────────────────── */

export function useGraphNodeData(options: UseGraphNodeDataOptions): GraphNodeComputedData {
  const {
    nodeId,
    pluginRef,
    label,
    selected = false,
    readOnly = false,
    executionStatus,
    error,
    duration,
    kind: kindOverride,
  } = options;

  const { registry } = useOrbflow();

  const schema = useMemo(
    () => registry.get(pluginRef),
    [registry, pluginRef],
  );

  return useMemo((): GraphNodeComputedData => {
    const kind: NodeKind = kindOverride
      ?? (schema?.nodeKind as NodeKind | undefined)
      ?? "action";

    const isTrigger = kind === "trigger";
    const isCapability = kind === "capability";

    const displayName = schema?.name || label || nodeId;
    const customName = label && label !== displayName ? label : "";

    const icon = schema?.icon || (isTrigger ? "zap" : isCapability ? "database" : "default");
    const color = schema?.color || "#7C5CFC";

    // Determine handle visibility based on kind and schema
    const hasInputHandle = !isTrigger && !isCapability;
    const hasOutputHandle = isTrigger || (!isCapability && (schema?.outputs?.length ?? 0) > 0);

    // Map capability ports from schema to the GraphCapabilityPort shape
    const rawPorts = schema?.capabilityPorts ?? [];
    const capabilityPorts: GraphCapabilityPort[] = rawPorts.map((port) => ({
      id: port.key,
      label: port.key,
      nodeKind: port.capabilityType,
    }));

    const isConnectable = !readOnly;

    return {
      nodeId,
      kind,
      schema,
      displayName,
      customName,
      icon,
      color,
      hasInputHandle,
      hasOutputHandle,
      capabilityPorts,
      isConnectable,
      executionStatus,
      error,
      duration,
      selected,
    };
  }, [
    nodeId,
    schema,
    label,
    selected,
    readOnly,
    executionStatus,
    error,
    duration,
    kindOverride,
  ]);
}
