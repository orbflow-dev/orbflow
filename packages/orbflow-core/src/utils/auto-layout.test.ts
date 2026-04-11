import { describe, it, expect } from "vitest";
import { autoLayout } from "./auto-layout";
import { layoutWorkflow } from "./layout-algorithms";
import type { Node, Edge } from "@xyflow/react";

function makeNode(id: string, x = 0, y = 0): Node {
  return { id, type: "task", position: { x, y }, data: {} };
}

function makeEdge(source: string, target: string): Edge {
  return { id: `${source}-${target}`, source, target };
}

// ── autoLayout (core algorithm) ─────────────────────

describe("autoLayout", () => {
  it("returns empty array for no nodes", () => {
    expect(autoLayout([], [])).toEqual([]);
  });

  it("positions a single node at origin", () => {
    const result = autoLayout([makeNode("a")], []);
    expect(result.length).toBe(1);
    expect(result[0].position.x).toBe(0);
    expect(result[0].position.y).toBe(0);
  });

  it("places connected nodes in separate layers (LR default)", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];
    const result = autoLayout(nodes, edges);

    const aNode = result.find((n) => n.id === "a")!;
    const bNode = result.find((n) => n.id === "b")!;
    // LR: a should be left of b (smaller x)
    expect(aNode.position.x).toBeLessThan(bNode.position.x);
  });

  it("places connected nodes in separate layers (TB)", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];
    const result = autoLayout(nodes, edges, "TB");

    const aNode = result.find((n) => n.id === "a")!;
    const bNode = result.find((n) => n.id === "b")!;
    expect(aNode.position.y).toBeLessThan(bNode.position.y);
  });

  it("places parallel nodes in the same layer (same x in LR)", () => {
    const nodes = [makeNode("a"), makeNode("b"), makeNode("c")];
    const edges = [makeEdge("a", "b"), makeEdge("a", "c")];
    const result = autoLayout(nodes, edges);

    const bNode = result.find((n) => n.id === "b")!;
    const cNode = result.find((n) => n.id === "c")!;
    // Same layer → same x in LR
    expect(bNode.position.x).toBe(cNode.position.x);
  });

  it("does not mutate input nodes", () => {
    const nodes = [makeNode("a", 100, 200), makeNode("b", 300, 400)];
    const edges = [makeEdge("a", "b")];
    autoLayout(nodes, edges);

    expect(nodes[0].position).toEqual({ x: 100, y: 200 });
    expect(nodes[1].position).toEqual({ x: 300, y: 400 });
  });

  it("handles diamond DAG", () => {
    const nodes = [makeNode("a"), makeNode("b"), makeNode("c"), makeNode("d")];
    const edges = [
      makeEdge("a", "b"),
      makeEdge("a", "c"),
      makeEdge("b", "d"),
      makeEdge("c", "d"),
    ];
    const result = autoLayout(nodes, edges);

    const aX = result.find((n) => n.id === "a")!.position.x;
    const bX = result.find((n) => n.id === "b")!.position.x;
    const dX = result.find((n) => n.id === "d")!.position.x;

    // LR: a < b < d in x
    expect(aX).toBeLessThan(bX);
    expect(bX).toBeLessThan(dX);
  });

  it("handles disconnected nodes", () => {
    const nodes = [makeNode("a"), makeNode("b"), makeNode("disconnected")];
    const edges = [makeEdge("a", "b")];
    const result = autoLayout(nodes, edges);
    expect(result.length).toBe(3);
    for (const n of result) {
      expect(n.position).toBeDefined();
    }
  });

  // ── New tests: LayoutOptions ──────────────────────

  it("accepts LayoutOptions object", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];
    const result = autoLayout(nodes, edges, { direction: "TB" });

    const aNode = result.find((n) => n.id === "a")!;
    const bNode = result.find((n) => n.id === "b")!;
    expect(aNode.position.y).toBeLessThan(bNode.position.y);
  });

  it("uses measured node dimensions", () => {
    const nodes = [makeNode("a"), makeNode("b"), makeNode("c")];
    const edges = [makeEdge("a", "b"), makeEdge("a", "c")];

    const dims = new Map([
      ["a", { width: 300, height: 100 }],
      ["b", { width: 200, height: 80 }],
      ["c", { width: 200, height: 80 }],
    ]);

    const result = autoLayout(nodes, edges, { nodeDimensions: dims });
    // b and c should be in layer 1, spaced based on measured height
    const bNode = result.find((n) => n.id === "b")!;
    const cNode = result.find((n) => n.id === "c")!;
    // Different y positions since they are in the same layer but separate rows (LR)
    expect(bNode.position.y).not.toBe(cNode.position.y);
  });

  it("respects custom spacing", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];

    const narrow = autoLayout(nodes, edges, { nodeSpacingX: 20 });
    const wide = autoLayout(nodes, edges, { nodeSpacingX: 300 });

    const aNarrow = narrow.find((n) => n.id === "a")!;
    const bNarrow = narrow.find((n) => n.id === "b")!;
    const aWide = wide.find((n) => n.id === "a")!;
    const bWide = wide.find((n) => n.id === "b")!;

    const gapNarrow = bNarrow.position.x - aNarrow.position.x;
    const gapWide = bWide.position.x - aWide.position.x;
    expect(gapWide).toBeGreaterThan(gapNarrow);
  });

  // ── Edge crossing minimization ────────────────────

  it("reduces edge crossings with barycenter heuristic", () => {
    // Graph: a -> c, a -> d, b -> c, b -> d
    // Without barycenter, order might cause crossings
    // With it, c and d should be ordered to minimize crossings relative to a, b
    const nodes = [
      makeNode("a"),
      makeNode("b"),
      makeNode("c"),
      makeNode("d"),
    ];
    const edges = [
      makeEdge("a", "c"),
      makeEdge("b", "d"),
    ];
    const result = autoLayout(nodes, edges);

    // a connects to c, b connects to d
    // After barycenter, a's child (c) and b's child (d) should maintain relative order
    const aY = result.find((n) => n.id === "a")!.position.y;
    const bY = result.find((n) => n.id === "b")!.position.y;
    const cY = result.find((n) => n.id === "c")!.position.y;
    const dY = result.find((n) => n.id === "d")!.position.y;

    // If a is above b, then c should be above d (no crossing)
    if (aY < bY) {
      expect(cY).toBeLessThanOrEqual(dY);
    } else {
      expect(dY).toBeLessThanOrEqual(cY);
    }
  });
});

// ── layoutWorkflow (unified interface) ───────────────

describe("layoutWorkflow", () => {
  it("defaults to auto algorithm", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];

    const autoResult = autoLayout(nodes, edges);
    const unifiedResult = layoutWorkflow(nodes, edges);

    expect(unifiedResult[0].position).toEqual(autoResult[0].position);
    expect(unifiedResult[1].position).toEqual(autoResult[1].position);
  });

  it("uses dagre algorithm", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];
    const result = layoutWorkflow(nodes, edges, { algorithm: "dagre" });

    expect(result.length).toBe(2);
    const aNode = result.find((n) => n.id === "a")!;
    const bNode = result.find((n) => n.id === "b")!;
    // LR default: a should be left of b
    expect(aNode.position.x).toBeLessThan(bNode.position.x);
  });

  it("uses compact algorithm with tighter spacing", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];

    const autoResult = layoutWorkflow(nodes, edges, { algorithm: "auto" });
    const compactResult = layoutWorkflow(nodes, edges, { algorithm: "compact" });

    const autoGap =
      autoResult.find((n) => n.id === "b")!.position.x -
      autoResult.find((n) => n.id === "a")!.position.x;
    const compactGap =
      compactResult.find((n) => n.id === "b")!.position.x -
      compactResult.find((n) => n.id === "a")!.position.x;

    expect(compactGap).toBeLessThan(autoGap);
  });

  it("dagre handles diamond DAG", () => {
    const nodes = [makeNode("a"), makeNode("b"), makeNode("c"), makeNode("d")];
    const edges = [
      makeEdge("a", "b"),
      makeEdge("a", "c"),
      makeEdge("b", "d"),
      makeEdge("c", "d"),
    ];
    const result = layoutWorkflow(nodes, edges, { algorithm: "dagre" });

    const aX = result.find((n) => n.id === "a")!.position.x;
    const bX = result.find((n) => n.id === "b")!.position.x;
    const dX = result.find((n) => n.id === "d")!.position.x;

    expect(aX).toBeLessThan(bX);
    expect(bX).toBeLessThan(dX);
  });

  it("dagre handles TB direction", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];
    const result = layoutWorkflow(nodes, edges, {
      algorithm: "dagre",
      direction: "TB",
    });

    const aNode = result.find((n) => n.id === "a")!;
    const bNode = result.find((n) => n.id === "b")!;
    expect(aNode.position.y).toBeLessThan(bNode.position.y);
  });

  it("does not mutate input nodes (all algorithms)", () => {
    const nodes = [makeNode("a", 100, 200), makeNode("b", 300, 400)];
    const edges = [makeEdge("a", "b")];

    for (const algorithm of ["auto", "dagre", "compact"] as const) {
      layoutWorkflow(nodes, edges, { algorithm });
      expect(nodes[0].position).toEqual({ x: 100, y: 200 });
      expect(nodes[1].position).toEqual({ x: 300, y: 400 });
    }
  });

  it("returns empty array for no nodes (all algorithms)", () => {
    for (const algorithm of ["auto", "dagre", "compact"] as const) {
      expect(layoutWorkflow([], [], { algorithm })).toEqual([]);
    }
  });

  it("dagre respects measured dimensions", () => {
    const nodes = [makeNode("a"), makeNode("b")];
    const edges = [makeEdge("a", "b")];
    const dims = new Map([
      ["a", { width: 400, height: 200 }],
      ["b", { width: 400, height: 200 }],
    ]);

    const withDims = layoutWorkflow(nodes, edges, {
      algorithm: "dagre",
      nodeDimensions: dims,
    });
    const withoutDims = layoutWorkflow(nodes, edges, { algorithm: "dagre" });

    // Larger measured dimensions should push nodes further apart
    const gapWithDims =
      withDims.find((n) => n.id === "b")!.position.x -
      withDims.find((n) => n.id === "a")!.position.x;
    const gapWithout =
      withoutDims.find((n) => n.id === "b")!.position.x -
      withoutDims.find((n) => n.id === "a")!.position.x;

    expect(gapWithDims).toBeGreaterThan(gapWithout);
  });
});
