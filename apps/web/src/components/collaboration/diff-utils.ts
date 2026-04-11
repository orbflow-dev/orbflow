import type { WorkflowDiff } from "@orbflow/core/types";

/**
 * Computes a structured diff between two workflow definitions (client-side).
 *
 * Mirrors the Rust `compute_diff()` algorithm in `orbflow-core/src/versioning.rs`:
 * extracts node IDs and edges from both definitions, then computes added/removed/modified sets.
 */
export function computeDiffFromDefinitions(
  baseDef: Record<string, unknown>,
  proposedDef: Record<string, unknown>,
  baseVersion: number,
): WorkflowDiff {
  const baseNodes = extractNodeMap(baseDef);
  const proposedNodes = extractNodeMap(proposedDef);
  const baseEdges = extractEdges(baseDef);
  const proposedEdges = extractEdges(proposedDef);

  const added_nodes = Object.keys(proposedNodes).filter((id) => !(id in baseNodes));
  const removed_nodes = Object.keys(baseNodes).filter((id) => !(id in proposedNodes));
  const modified_nodes = Object.keys(baseNodes).filter((id) => {
    if (!(id in proposedNodes)) return false;
    return JSON.stringify(baseNodes[id]) !== JSON.stringify(proposedNodes[id]);
  });

  const baseEdgeSet = new Set(baseEdges.map(([s, t]) => `${s}->${t}`));
  const proposedEdgeSet = new Set(proposedEdges.map(([s, t]) => `${s}->${t}`));

  const added_edges = proposedEdges
    .filter(([s, t]) => !baseEdgeSet.has(`${s}->${t}`))
    .map(([s, t]) => `${s}->${t}`);
  const removed_edges = baseEdges
    .filter(([s, t]) => !proposedEdgeSet.has(`${s}->${t}`))
    .map(([s, t]) => `${s}->${t}`);

  return {
    from_version: baseVersion,
    to_version: baseVersion + 1,
    added_nodes,
    removed_nodes,
    modified_nodes,
    added_edges,
    removed_edges,
  };
}

function extractNodeMap(def: Record<string, unknown>): Record<string, unknown> {
  const nodes = (def.nodes as Array<Record<string, unknown>>) ?? [];
  const map: Record<string, unknown> = {};
  for (const node of nodes) {
    const id = String(node.id ?? "");
    if (id) map[id] = node;
  }
  return map;
}

function extractEdges(def: Record<string, unknown>): [string, string][] {
  const edges = (def.edges as Array<Record<string, unknown>>) ?? [];
  const result: [string, string][] = [];
  for (const edge of edges) {
    const source = edge.source as string | undefined;
    const target = edge.target as string | undefined;
    if (source && target) result.push([source, target]);
  }
  return result;
}
