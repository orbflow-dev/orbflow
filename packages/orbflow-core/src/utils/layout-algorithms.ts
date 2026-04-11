import type { Node, Edge } from "@xyflow/react";
import dagre from "@dagrejs/dagre";

import { autoLayout, type LayoutDirection, type LayoutOptions } from "./auto-layout";

export type LayoutAlgorithm = "auto" | "dagre" | "compact";

export type LayoutWorkflowOptions = LayoutOptions & {
  algorithm?: LayoutAlgorithm;
};

// ── Dagre layout ─────────────────────────────────────

const DEFAULT_NODE_WIDTH = 260;
const DEFAULT_NODE_HEIGHT = 160;

function dagreLayout(
  nodes: Node[],
  edges: Edge[],
  opts: LayoutOptions = {},
): Node[] {
  if (nodes.length === 0) return nodes;

  const direction = opts.direction ?? "LR";
  const gapX = opts.nodeSpacingX ?? 100;
  const gapY = opts.nodeSpacingY ?? 60;
  const dims = opts.nodeDimensions;

  const g = new dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));

  g.setGraph({
    rankdir: direction,
    nodesep: direction === "TB" ? gapX : gapY,
    ranksep: direction === "TB" ? gapY : gapX,
    marginx: 20,
    marginy: 20,
  });

  for (const node of nodes) {
    const measured = dims?.get(node.id);
    g.setNode(node.id, {
      width: measured?.width ?? DEFAULT_NODE_WIDTH,
      height: measured?.height ?? DEFAULT_NODE_HEIGHT,
    });
  }

  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }

  dagre.layout(g);

  return nodes.map((node) => {
    const positioned = g.node(node.id);
    if (!positioned) return node;

    const measured = dims?.get(node.id);
    const w = measured?.width ?? DEFAULT_NODE_WIDTH;
    const h = measured?.height ?? DEFAULT_NODE_HEIGHT;

    return {
      ...node,
      position: {
        x: positioned.x - w / 2,
        y: positioned.y - h / 2,
      },
    };
  });
}

// ── Compact layout (tighter spacing) ─────────────────

function compactLayout(
  nodes: Node[],
  edges: Edge[],
  opts: LayoutOptions = {},
): Node[] {
  return autoLayout(nodes, edges, {
    ...opts,
    nodeSpacingX: opts.nodeSpacingX ?? 60,
    nodeSpacingY: opts.nodeSpacingY ?? 30,
  });
}

// ── Unified entry point ──────────────────────────────

/**
 * Layout workflow nodes using the selected algorithm.
 *
 * - `"auto"` — built-in topological sort with barycenter crossing minimization
 * - `"dagre"` — Dagre.js hierarchical layout (best for tree-like workflows)
 * - `"compact"` — same as auto but with tighter spacing
 *
 * Returns new node objects; never mutates the input.
 */
export function layoutWorkflow(
  nodes: Node[],
  edges: Edge[],
  options: LayoutWorkflowOptions = {},
): Node[] {
  const algorithm = options.algorithm ?? "auto";

  switch (algorithm) {
    case "dagre":
      return dagreLayout(nodes, edges, options);
    case "compact":
      return compactLayout(nodes, edges, options);
    case "auto":
    default:
      return autoLayout(nodes, edges, options);
  }
}
