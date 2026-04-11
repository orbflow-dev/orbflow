"use client";

import { useEffect, useMemo } from "react";
import { useUpdateNodeInternals } from "@xyflow/react";

type InternalsSyncProps = {
  nodeIds: string[];
};

/**
 * Invisible component that syncs ReactFlow node internals whenever the
 * set of node IDs changes. Uses requestAnimationFrame for efficient
 * batched updates.
 */
export function InternalsSync({ nodeIds }: InternalsSyncProps): null {
  const updateNodeInternals = useUpdateNodeInternals();

  // Memoize the key to avoid unnecessary re-renders
  const key = useMemo(() => nodeIds.join(","), [nodeIds]);

  useEffect(() => {
    if (nodeIds.length === 0) return;
    const raf = requestAnimationFrame(() => {
      updateNodeInternals(nodeIds);
    });
    return () => cancelAnimationFrame(raf);
  }, [key, updateNodeInternals]); // eslint-disable-line react-hooks/exhaustive-deps

  return null;
}
