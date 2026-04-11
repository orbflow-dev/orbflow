/**
 * Headless ReactFlow Handle layout for workflow graph nodes.
 *
 * Renders the appropriate Handle elements based on the node kind:
 * - trigger:    source (right) only
 * - action:     target (left) + source (right) + capability ports (bottom)
 * - capability: target (top) only
 *
 * Handle elements have NO className — consumers style via CSS
 * targeting `.react-flow__handle` or the data attributes.
 */

import { Handle, Position } from "@xyflow/react";

/* ── Types ────────────────────────────────────────────── */

export interface GraphCapabilityPort {
  /** Unique identifier for this capability port */
  id: string;
  /** Display label for the port */
  label: string;
  /** The node kind that can connect to this port */
  nodeKind: string;
}

export interface NodeHandlesProps {
  /** Node kind determines which handles are rendered */
  kind: "trigger" | "action" | "capability";
  /** Capability port definitions (only used for "action" kind) */
  capabilityPorts?: GraphCapabilityPort[];
  /** Whether handles accept new connections (passed to Handle) */
  isConnectable?: boolean;
}

/* ── Component ────────────────────────────────────────── */

/**
 * Renders ReactFlow Handle elements based on node kind.
 * No visual styling — only structural Handle placement.
 */
export function NodeHandles({
  kind,
  capabilityPorts = [],
  isConnectable,
}: NodeHandlesProps): React.ReactNode {
  if (kind === "trigger") {
    return (
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        isConnectable={isConnectable}
      />
    );
  }

  if (kind === "capability") {
    return (
      <Handle
        type="target"
        position={Position.Top}
        id="in"
        isConnectable={isConnectable}
      />
    );
  }

  // kind === "action"
  return (
    <>
      <Handle
        type="target"
        position={Position.Left}
        id="in"
        isConnectable={isConnectable}
      />
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        isConnectable={isConnectable}
      />
      {capabilityPorts.map((port, i) => (
        <Handle
          key={port.id}
          type="target"
          position={Position.Bottom}
          id={`cap:${port.id}`}
          isConnectable={isConnectable}
          style={{
            left: `${((i + 1) / (capabilityPorts.length + 1)) * 100}%`,
          }}
        />
      ))}
    </>
  );
}
