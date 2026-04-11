import type { Node, Edge } from "@xyflow/react";

// ── Default dimensions & spacing ────────────────────
const DEFAULT_NODE_WIDTH = 260;
const DEFAULT_NODE_HEIGHT = 160;
const DEFAULT_GAP_X = 100;
const DEFAULT_GAP_Y = 60;

export type LayoutDirection = "LR" | "TB";

export type LayoutOptions = {
  direction?: LayoutDirection;
  nodeSpacingX?: number;
  nodeSpacingY?: number;
  /** Measured node sizes from React Flow — keyed by node id */
  nodeDimensions?: ReadonlyMap<string, { width: number; height: number }>;
};

// ── Helpers ──────────────────────────────────────────

function getNodeSize(
  nodeId: string,
  dims: ReadonlyMap<string, { width: number; height: number }> | undefined,
): { width: number; height: number } {
  const measured = dims?.get(nodeId);
  return {
    width: measured?.width ?? DEFAULT_NODE_WIDTH,
    height: measured?.height ?? DEFAULT_NODE_HEIGHT,
  };
}

/**
 * Build layers via Kahn's algorithm (topological BFS).
 * Nodes without incoming edges start in layer 0.
 * Remaining unvisited nodes (cycles) are appended to the last layer.
 */
function buildLayers(
  nodes: Node[],
  edges: Edge[],
): { layers: string[][]; adj: Map<string, string[]>; reverseAdj: Map<string, string[]> } {
  const adj = new Map<string, string[]>();
  const reverseAdj = new Map<string, string[]>();
  const inDegree = new Map<string, number>();

  for (const node of nodes) {
    adj.set(node.id, []);
    reverseAdj.set(node.id, []);
    inDegree.set(node.id, 0);
  }

  for (const edge of edges) {
    adj.get(edge.source)?.push(edge.target);
    reverseAdj.get(edge.target)?.push(edge.source);
    inDegree.set(edge.target, (inDegree.get(edge.target) ?? 0) + 1);
  }

  const layers: string[][] = [];
  let queue = nodes
    .filter((n) => (inDegree.get(n.id) ?? 0) === 0)
    .map((n) => n.id);

  const visited = new Set<string>();

  while (queue.length > 0) {
    layers.push([...queue]);
    const nextQueue: string[] = [];

    for (const nodeId of queue) {
      visited.add(nodeId);
      for (const child of adj.get(nodeId) ?? []) {
        inDegree.set(child, (inDegree.get(child) ?? 0) - 1);
        if (inDegree.get(child) === 0 && !visited.has(child)) {
          nextQueue.push(child);
        }
      }
    }

    queue = nextQueue;
  }

  // Append cycle nodes to the last layer
  for (const node of nodes) {
    if (!visited.has(node.id)) {
      const lastLayer = layers[layers.length - 1];
      if (lastLayer) {
        lastLayer.push(node.id);
      } else {
        layers.push([node.id]);
      }
    }
  }

  return { layers, adj, reverseAdj };
}

/**
 * Barycenter heuristic: reorder nodes within each layer to minimize
 * edge crossings by sorting each node by the average position of its
 * connected nodes in the adjacent layer.
 *
 * Runs `passes` iterations alternating forward and backward sweeps.
 */
function minimizeCrossings(
  layers: string[][],
  adj: Map<string, string[]>,
  reverseAdj: Map<string, string[]>,
  passes = 4,
): string[][] {
  // Work on a mutable copy
  const result = layers.map((layer) => [...layer]);

  for (let pass = 0; pass < passes; pass++) {
    if (pass % 2 === 0) {
      // Forward sweep: use parent positions to reorder children
      for (let i = 1; i < result.length; i++) {
        const prevPositions = new Map<string, number>();
        for (let p = 0; p < result[i - 1].length; p++) {
          prevPositions.set(result[i - 1][p], p);
        }
        result[i].sort((a, b) => {
          const aParents = reverseAdj.get(a) ?? [];
          const bParents = reverseAdj.get(b) ?? [];
          const aBar = barycenter(aParents, prevPositions);
          const bBar = barycenter(bParents, prevPositions);
          return aBar - bBar;
        });
      }
    } else {
      // Backward sweep: use child positions to reorder parents
      for (let i = result.length - 2; i >= 0; i--) {
        const nextPositions = new Map<string, number>();
        for (let p = 0; p < result[i + 1].length; p++) {
          nextPositions.set(result[i + 1][p], p);
        }
        result[i].sort((a, b) => {
          const aChildren = adj.get(a) ?? [];
          const bChildren = adj.get(b) ?? [];
          const aBar = barycenter(aChildren, nextPositions);
          const bBar = barycenter(bChildren, nextPositions);
          return aBar - bBar;
        });
      }
    }
  }

  return result;
}

function barycenter(
  neighbors: string[],
  positions: Map<string, number>,
): number {
  if (neighbors.length === 0) return Infinity;
  let sum = 0;
  let count = 0;
  for (const n of neighbors) {
    const pos = positions.get(n);
    if (pos !== undefined) {
      sum += pos;
      count++;
    }
  }
  return count === 0 ? Infinity : sum / count;
}

// ── Main layout function ─────────────────────────────

/**
 * Auto-arrange nodes in a DAG layout using topological sort with
 * barycenter edge-crossing minimization.
 *
 * Supports both left-to-right (LR) and top-to-bottom (TB) directions.
 * Returns new node objects without mutating the input.
 */
export function autoLayout(
  nodes: Node[],
  edges: Edge[],
  directionOrOptions: LayoutDirection | LayoutOptions = "LR",
): Node[] {
  if (nodes.length === 0) return nodes;

  // Normalize options
  const opts: LayoutOptions =
    typeof directionOrOptions === "string"
      ? { direction: directionOrOptions }
      : directionOrOptions;

  const direction = opts.direction ?? "LR";
  const gapX = opts.nodeSpacingX ?? DEFAULT_GAP_X;
  const gapY = opts.nodeSpacingY ?? DEFAULT_GAP_Y;
  const dims = opts.nodeDimensions;

  // 1. Build topological layers
  const { layers: rawLayers, adj, reverseAdj } = buildLayers(nodes, edges);

  // 2. Minimize edge crossings
  const layers = minimizeCrossings(rawLayers, adj, reverseAdj);

  // 3. Assign positions
  const positions = new Map<string, { x: number; y: number }>();

  if (direction === "TB") {
    // Top-to-bottom: layers are rows, nodes within a layer are columns
    // Compute max layer width for centering
    let maxTotalWidth = 0;
    for (const layer of layers) {
      let layerWidth = 0;
      for (const nodeId of layer) {
        layerWidth += getNodeSize(nodeId, dims).width;
      }
      layerWidth += (layer.length - 1) * gapX;
      maxTotalWidth = Math.max(maxTotalWidth, layerWidth);
    }

    let currentY = 0;
    for (const layer of layers) {
      let layerWidth = 0;
      for (const nodeId of layer) {
        layerWidth += getNodeSize(nodeId, dims).width;
      }
      layerWidth += (layer.length - 1) * gapX;
      const offsetX = (maxTotalWidth - layerWidth) / 2;

      let maxHeight = 0;
      let currentX = offsetX;
      for (const nodeId of layer) {
        const size = getNodeSize(nodeId, dims);
        positions.set(nodeId, { x: currentX, y: currentY });
        currentX += size.width + gapX;
        maxHeight = Math.max(maxHeight, size.height);
      }

      currentY += maxHeight + gapY;
    }
  } else {
    // Left-to-right: layers are columns, nodes within a layer are rows
    // Compute max layer height for centering
    let maxTotalHeight = 0;
    for (const layer of layers) {
      let layerHeight = 0;
      for (const nodeId of layer) {
        layerHeight += getNodeSize(nodeId, dims).height;
      }
      layerHeight += (layer.length - 1) * gapY;
      maxTotalHeight = Math.max(maxTotalHeight, layerHeight);
    }

    let currentX = 0;
    for (const layer of layers) {
      let layerHeight = 0;
      for (const nodeId of layer) {
        layerHeight += getNodeSize(nodeId, dims).height;
      }
      layerHeight += (layer.length - 1) * gapY;
      const offsetY = (maxTotalHeight - layerHeight) / 2;

      let maxWidth = 0;
      let currentY = offsetY;
      for (const nodeId of layer) {
        const size = getNodeSize(nodeId, dims);
        positions.set(nodeId, { x: currentX, y: currentY });
        currentY += size.height + gapY;
        maxWidth = Math.max(maxWidth, size.width);
      }

      currentX += maxWidth + gapX;
    }
  }

  // 4. Return new nodes with updated positions (immutable)
  return nodes.map((node) => {
    const pos = positions.get(node.id);
    if (!pos) return node;
    return { ...node, position: pos };
  });
}
