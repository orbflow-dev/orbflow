import { describe, it, expect } from "vitest";
import { topoSortIds, topoSortNodes } from "./workflow-graph";
import type { Workflow } from "../types/api";

function makeWorkflow(
  nodes: { id: string }[],
  edges: { id: string; source: string; target: string }[],
): Workflow {
  return {
    id: "w1",
    name: "test",
    version: 1,
    status: "active",
    nodes: nodes.map((n) => ({
      id: n.id,
      type: "action" as const,
      position: { x: 0, y: 0 },
      plugin_ref: "test",
      label: n.id,
      parameters: {},
      input_mapping: {},
    })),
    edges: edges.map((e) => ({
      id: e.id,
      source: e.source,
      target: e.target,
    })),
    created_at: "",
    updated_at: "",
  } as unknown as Workflow;
}

describe("topoSortIds", () => {
  it("returns single node", () => {
    const wf = makeWorkflow([{ id: "a" }], []);
    expect(topoSortIds(wf)).toEqual(["a"]);
  });

  it("sorts a linear chain", () => {
    const wf = makeWorkflow(
      [{ id: "a" }, { id: "b" }, { id: "c" }],
      [
        { id: "e1", source: "a", target: "b" },
        { id: "e2", source: "b", target: "c" },
      ],
    );
    expect(topoSortIds(wf)).toEqual(["a", "b", "c"]);
  });

  it("handles diamond DAG", () => {
    const wf = makeWorkflow(
      [{ id: "a" }, { id: "b" }, { id: "c" }, { id: "d" }],
      [
        { id: "e1", source: "a", target: "b" },
        { id: "e2", source: "a", target: "c" },
        { id: "e3", source: "b", target: "d" },
        { id: "e4", source: "c", target: "d" },
      ],
    );
    const order = topoSortIds(wf);
    expect(order[0]).toBe("a");
    expect(order[order.length - 1]).toBe("d");
    expect(order.indexOf("b")).toBeLessThan(order.indexOf("d"));
    expect(order.indexOf("c")).toBeLessThan(order.indexOf("d"));
  });

  it("appends disconnected nodes at end", () => {
    const wf = makeWorkflow(
      [{ id: "a" }, { id: "b" }, { id: "disconnected" }],
      [{ id: "e1", source: "a", target: "b" }],
    );
    const order = topoSortIds(wf);
    expect(order).toContain("disconnected");
    expect(order.length).toBe(3);
  });

  it("handles empty workflow", () => {
    const wf = makeWorkflow([], []);
    expect(topoSortIds(wf)).toEqual([]);
  });
});

describe("topoSortNodes", () => {
  it("returns full node objects in topological order", () => {
    const wf = makeWorkflow(
      [{ id: "a" }, { id: "b" }],
      [{ id: "e1", source: "a", target: "b" }],
    );
    const nodes = topoSortNodes(wf);
    expect(nodes.length).toBe(2);
    expect(nodes[0].id).toBe("a");
    expect(nodes[1].id).toBe("b");
  });
});
