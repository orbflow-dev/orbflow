import type { Workflow } from "../types/api";

/** Return node IDs in topological order (BFS). Disconnected nodes appended at end. */
export function topoSortIds(workflow: Workflow): string[] {
  const adj: Record<string, string[]> = {};
  const inDeg: Record<string, number> = {};

  for (const n of workflow.nodes) {
    adj[n.id] = [];
    inDeg[n.id] = 0;
  }

  for (const e of workflow.edges) {
    if (adj[e.source]) {
      adj[e.source].push(e.target);
      inDeg[e.target] = (inDeg[e.target] || 0) + 1;
    }
  }

  const queue = workflow.nodes
    .filter((n) => (inDeg[n.id] || 0) === 0)
    .map((n) => n.id);
  const order: string[] = [];

  while (queue.length > 0) {
    const id = queue.shift()!;
    order.push(id);
    for (const next of adj[id] || []) {
      inDeg[next]--;
      if (inDeg[next] === 0) queue.push(next);
    }
  }

  for (const n of workflow.nodes) {
    if (!order.includes(n.id)) order.push(n.id);
  }

  return order;
}

/** Return workflow nodes in topological order (full objects). */
export function topoSortNodes(workflow: Workflow): Workflow["nodes"] {
  const ids = topoSortIds(workflow);
  const map = new Map(workflow.nodes.map((n) => [n.id, n]));
  return ids.map((id) => map.get(id)!).filter(Boolean);
}
