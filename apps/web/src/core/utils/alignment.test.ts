import { describe, it, expect } from "vitest";
import { alignNodes, distributeNodes, type NodeRect } from "./alignment";

const makeNodes = (): NodeRect[] => [
  { id: "a", x: 10, y: 20, width: 100, height: 50 },
  { id: "b", x: 200, y: 80, width: 120, height: 60 },
  { id: "c", x: 100, y: 150, width: 80, height: 40 },
];

describe("alignNodes", () => {
  it("returns unchanged for fewer than 2 nodes", () => {
    const single = [{ id: "a", x: 10, y: 20, width: 100, height: 50 }];
    expect(alignNodes(single, "left")).toEqual(single);
    expect(alignNodes([], "left")).toEqual([]);
  });

  it("aligns left to minimum x", () => {
    const result = alignNodes(makeNodes(), "left");
    expect(result.every((n) => n.x === 10)).toBe(true);
  });

  it("aligns right to maximum right edge", () => {
    const nodes = makeNodes();
    const maxRight = Math.max(...nodes.map((n) => n.x + n.width)); // 320
    const result = alignNodes(nodes, "right");
    for (const n of result) {
      expect(n.x + n.width).toBe(maxRight);
    }
  });

  it("aligns center to average center", () => {
    const nodes = makeNodes();
    const centers = nodes.map((n) => n.x + n.width / 2);
    const avgCenter = centers.reduce((a, b) => a + b, 0) / centers.length;
    const result = alignNodes(nodes, "center");
    for (const n of result) {
      expect(n.x + n.width / 2).toBeCloseTo(avgCenter);
    }
  });

  it("aligns top to minimum y", () => {
    const result = alignNodes(makeNodes(), "top");
    expect(result.every((n) => n.y === 20)).toBe(true);
  });

  it("aligns bottom to maximum bottom edge", () => {
    const nodes = makeNodes();
    const maxBottom = Math.max(...nodes.map((n) => n.y + n.height)); // 190
    const result = alignNodes(nodes, "bottom");
    for (const n of result) {
      expect(n.y + n.height).toBe(maxBottom);
    }
  });

  it("aligns middle to average middle", () => {
    const nodes = makeNodes();
    const middles = nodes.map((n) => n.y + n.height / 2);
    const avgMiddle = middles.reduce((a, b) => a + b, 0) / middles.length;
    const result = alignNodes(nodes, "middle");
    for (const n of result) {
      expect(n.y + n.height / 2).toBeCloseTo(avgMiddle);
    }
  });

  it("does not mutate original nodes", () => {
    const nodes = makeNodes();
    const origX = nodes.map((n) => n.x);
    alignNodes(nodes, "left");
    expect(nodes.map((n) => n.x)).toEqual(origX);
  });
});

describe("distributeNodes", () => {
  it("returns unchanged for fewer than 3 nodes", () => {
    const two = makeNodes().slice(0, 2);
    expect(distributeNodes(two, "horizontal")).toEqual(two);
  });

  it("distributes horizontally with even spacing", () => {
    const nodes: NodeRect[] = [
      { id: "a", x: 0, y: 0, width: 50, height: 50 },
      { id: "b", x: 100, y: 0, width: 50, height: 50 },
      { id: "c", x: 300, y: 0, width: 50, height: 50 },
    ];
    const result = distributeNodes(nodes, "horizontal");
    // Total span: 300+50 - 0 = 350, total widths: 150, space: 200, gap: 100
    expect(result[0].x).toBe(0);
    expect(result[1].x).toBe(150); // 0 + 50 + 100
    expect(result[2].x).toBe(300); // 150 + 50 + 100
  });

  it("distributes vertically with even spacing", () => {
    const nodes: NodeRect[] = [
      { id: "a", x: 0, y: 0, width: 50, height: 40 },
      { id: "b", x: 0, y: 100, width: 50, height: 40 },
      { id: "c", x: 0, y: 200, width: 50, height: 40 },
    ];
    const result = distributeNodes(nodes, "vertical");
    // Total span: 200+40 - 0 = 240, total heights: 120, space: 120, gap: 60
    expect(result[0].y).toBe(0);
    expect(result[1].y).toBe(100); // 0 + 40 + 60
    expect(result[2].y).toBe(200); // 100 + 40 + 60
  });

  it("clamps gap to 0 for overlapping nodes", () => {
    const nodes: NodeRect[] = [
      { id: "a", x: 0, y: 0, width: 100, height: 50 },
      { id: "b", x: 20, y: 0, width: 100, height: 50 },
      { id: "c", x: 40, y: 0, width: 100, height: 50 },
    ];
    const result = distributeNodes(nodes, "horizontal");
    // Gap is 0 -- tightly packed
    expect(result[0].x).toBe(0);
    expect(result[1].x).toBe(100);
    expect(result[2].x).toBe(200);
  });

  it("does not mutate original nodes", () => {
    const nodes = makeNodes();
    const origX = nodes.map((n) => n.x);
    distributeNodes(nodes, "horizontal");
    expect(nodes.map((n) => n.x)).toEqual(origX);
  });
});
