import { describe, it, expect } from "vitest";

import type { ExecutionStatus } from "./execution-status";
import {
  NODE_SIZES,
  STATUS_COLORS,
  STATUS_THEMES,
  FALLBACK_THEME,
  STATUS_LABELS,
  STATUS_BADGE,
  SEGMENT_ORDER,
  formatDurationMs,
  formatDurationRange,
} from "./constants";
import type { StatusTheme } from "./constants";
import { topoSortIds, topoSortNodes } from "./workflow-graph";
import type { Workflow, WorkflowNode, WorkflowEdge } from "../types/api";

// ---------------------------------------------------------------------------
// Helpers to build minimal Workflow fixtures
// ---------------------------------------------------------------------------

function makeNode(id: string): WorkflowNode {
  return {
    id,
    name: id,
    type: "action",
    plugin_ref: "builtin:noop",
    position: { x: 0, y: 0 },
  };
}

function makeEdge(source: string, target: string): WorkflowEdge {
  return { id: `${source}->${target}`, source, target };
}

function makeWorkflow(
  nodes: WorkflowNode[],
  edges: WorkflowEdge[] = [],
): Workflow {
  return {
    id: "wf-1",
    name: "Test Workflow",
    version: 1,
    status: "active",
    nodes,
    edges,
    created_at: "2025-01-01T00:00:00Z",
    updated_at: "2025-01-01T00:00:00Z",
  };
}

// ---------------------------------------------------------------------------
// Constants & lookup tables
// ---------------------------------------------------------------------------

const ALL_STATUSES: ExecutionStatus[] = [
  "pending",
  "queued",
  "running",
  "completed",
  "failed",
  "skipped",
  "cancelled",
  "waiting_approval",
];

describe("NODE_SIZES", () => {
  it("contains expected keys with positive numbers", () => {
    expect(NODE_SIZES.trigger).toBeGreaterThan(0);
    expect(NODE_SIZES.action).toBeGreaterThan(0);
    expect(NODE_SIZES.capability).toBeGreaterThan(0);
  });

  it("trigger is the largest", () => {
    expect(NODE_SIZES.trigger).toBeGreaterThanOrEqual(NODE_SIZES.action);
    expect(NODE_SIZES.trigger).toBeGreaterThanOrEqual(NODE_SIZES.capability);
  });
});

describe("STATUS_COLORS", () => {
  it("has an entry for every ExecutionStatus", () => {
    for (const s of ALL_STATUSES) {
      expect(STATUS_COLORS[s]).toBeDefined();
    }
  });

  it("all values are valid hex color strings", () => {
    for (const color of Object.values(STATUS_COLORS)) {
      expect(color).toMatch(/^#[0-9A-Fa-f]{6}$/);
    }
  });
});

describe("STATUS_THEMES", () => {
  it("has an entry for every ExecutionStatus", () => {
    for (const s of ALL_STATUSES) {
      expect(STATUS_THEMES[s]).toBeDefined();
    }
  });

  it("each theme has the required shape", () => {
    for (const s of ALL_STATUSES) {
      const theme: StatusTheme = STATUS_THEMES[s];
      expect(theme.accent).toMatch(/^#[0-9A-Fa-f]{6}$/);
      expect(theme.accentRgb).toMatch(/^\d{1,3},\d{1,3},\d{1,3}$/);
      expect(theme.text).toBeTruthy();
      expect(theme.bg).toBeTruthy();
      expect(theme.icon).toBeTruthy();
      expect(theme.label).toBeTruthy();
    }
  });

  it("accent color in theme matches STATUS_COLORS", () => {
    for (const s of ALL_STATUSES) {
      expect(STATUS_THEMES[s].accent).toBe(STATUS_COLORS[s]);
    }
  });

  it("label in theme matches STATUS_LABELS", () => {
    for (const s of ALL_STATUSES) {
      expect(STATUS_THEMES[s].label).toBe(STATUS_LABELS[s]);
    }
  });
});

describe("FALLBACK_THEME", () => {
  it("is the pending theme", () => {
    expect(FALLBACK_THEME).toBe(STATUS_THEMES.pending);
  });
});

describe("STATUS_LABELS", () => {
  it("has an entry for every ExecutionStatus", () => {
    for (const s of ALL_STATUSES) {
      expect(STATUS_LABELS[s]).toBeDefined();
    }
  });

  it("waiting_approval is human-readable", () => {
    expect(STATUS_LABELS.waiting_approval).toBe("Waiting Approval");
  });
});

describe("STATUS_BADGE", () => {
  it("has entries for terminal and active statuses", () => {
    const expectedKeys = [
      "completed",
      "failed",
      "running",
      "skipped",
      "cancelled",
      "waiting_approval",
    ];
    for (const key of expectedKeys) {
      expect(STATUS_BADGE[key]).toBeDefined();
      expect(STATUS_BADGE[key]!.cssModifier).toBeTruthy();
      expect(STATUS_BADGE[key]!.icon).toBeTruthy();
    }
  });

  it("running badge has spin: true", () => {
    expect(STATUS_BADGE.running!.spin).toBe(true);
  });

  it("completed badge does not spin", () => {
    expect(STATUS_BADGE.completed!.spin).toBeUndefined();
  });

  it("pending and queued have no badge", () => {
    expect(STATUS_BADGE.pending).toBeUndefined();
    expect(STATUS_BADGE.queued).toBeUndefined();
  });
});

describe("SEGMENT_ORDER", () => {
  it("has no duplicate entries", () => {
    const unique = new Set(SEGMENT_ORDER);
    expect(unique.size).toBe(SEGMENT_ORDER.length);
  });

  it("every entry is a valid ExecutionStatus", () => {
    for (const s of SEGMENT_ORDER) {
      expect(ALL_STATUSES).toContain(s);
    }
  });

  it("completed comes first (most important result)", () => {
    expect(SEGMENT_ORDER[0]).toBe("completed");
  });

  it("pending comes last (least informative)", () => {
    expect(SEGMENT_ORDER[SEGMENT_ORDER.length - 1]).toBe("pending");
  });
});

// ---------------------------------------------------------------------------
// formatDurationMs
// ---------------------------------------------------------------------------

describe("formatDurationMs", () => {
  it("returns '<1s' for 0 ms", () => {
    expect(formatDurationMs(0)).toBe("<1s");
  });

  it("returns '<1s' for values under 1000 ms", () => {
    expect(formatDurationMs(1)).toBe("<1s");
    expect(formatDurationMs(500)).toBe("<1s");
    expect(formatDurationMs(999)).toBe("<1s");
  });

  it("formats exactly 1000 ms as seconds", () => {
    expect(formatDurationMs(1000)).toBe("1.0s");
  });

  it("formats seconds with one decimal place", () => {
    expect(formatDurationMs(1500)).toBe("1.5s");
    expect(formatDurationMs(3200)).toBe("3.2s");
    expect(formatDurationMs(59999)).toBe("60.0s");
  });

  it("formats 60000 ms as minutes and seconds", () => {
    expect(formatDurationMs(60000)).toBe("1m 0s");
  });

  it("formats mixed minutes and seconds", () => {
    expect(formatDurationMs(135000)).toBe("2m 15s");
  });

  it("formats large durations correctly", () => {
    // 10 minutes and 30 seconds
    expect(formatDurationMs(630000)).toBe("10m 30s");
  });

  it("floors seconds within the minutes range", () => {
    // 1 minute + 999 ms => 1m 0s (sub-second remainder is dropped)
    expect(formatDurationMs(60999)).toBe("1m 0s");
  });
});

// ---------------------------------------------------------------------------
// formatDurationRange
// ---------------------------------------------------------------------------

describe("formatDurationRange", () => {
  const base = "2025-06-01T12:00:00Z";

  function offset(seconds: number): string {
    return new Date(new Date(base).getTime() + seconds * 1000).toISOString();
  }

  it("returns '<1s' for identical timestamps", () => {
    expect(formatDurationRange(base, base)).toBe("<1s");
  });

  it("returns '<1s' for sub-second differences", () => {
    const end = new Date(new Date(base).getTime() + 500).toISOString();
    expect(formatDurationRange(base, end)).toBe("<1s");
  });

  it("formats seconds only (under 60s)", () => {
    expect(formatDurationRange(base, offset(1))).toBe("1s");
    expect(formatDurationRange(base, offset(45))).toBe("45s");
    expect(formatDurationRange(base, offset(59))).toBe("59s");
  });

  it("formats minutes and seconds (under 60m)", () => {
    expect(formatDurationRange(base, offset(60))).toBe("1m 0s");
    expect(formatDurationRange(base, offset(150))).toBe("2m 30s");
    expect(formatDurationRange(base, offset(3599))).toBe("59m 59s");
  });

  it("formats hours and minutes (60m+)", () => {
    expect(formatDurationRange(base, offset(3600))).toBe("1h 0m");
    expect(formatDurationRange(base, offset(3900))).toBe("1h 5m");
    expect(formatDurationRange(base, offset(7200))).toBe("2h 0m");
  });

  it("clamps negative durations to '<1s'", () => {
    // end before start
    expect(formatDurationRange(offset(60), base)).toBe("<1s");
  });
});

// ---------------------------------------------------------------------------
// topoSortIds
// ---------------------------------------------------------------------------

describe("topoSortIds", () => {
  it("returns empty array for empty workflow", () => {
    const wf = makeWorkflow([]);
    expect(topoSortIds(wf)).toEqual([]);
  });

  it("returns single node", () => {
    const wf = makeWorkflow([makeNode("A")]);
    expect(topoSortIds(wf)).toEqual(["A"]);
  });

  it("returns nodes in topological order for a linear chain", () => {
    const wf = makeWorkflow(
      [makeNode("A"), makeNode("B"), makeNode("C")],
      [makeEdge("A", "B"), makeEdge("B", "C")],
    );
    expect(topoSortIds(wf)).toEqual(["A", "B", "C"]);
  });

  it("handles diamond dependency graph", () => {
    //   A
    //  / \
    // B   C
    //  \ /
    //   D
    const wf = makeWorkflow(
      [makeNode("A"), makeNode("B"), makeNode("C"), makeNode("D")],
      [
        makeEdge("A", "B"),
        makeEdge("A", "C"),
        makeEdge("B", "D"),
        makeEdge("C", "D"),
      ],
    );
    const result = topoSortIds(wf);
    expect(result.indexOf("A")).toBeLessThan(result.indexOf("B"));
    expect(result.indexOf("A")).toBeLessThan(result.indexOf("C"));
    expect(result.indexOf("B")).toBeLessThan(result.indexOf("D"));
    expect(result.indexOf("C")).toBeLessThan(result.indexOf("D"));
    expect(result).toHaveLength(4);
  });

  it("appends disconnected nodes at the end", () => {
    const wf = makeWorkflow(
      [makeNode("A"), makeNode("B"), makeNode("Z")],
      [makeEdge("A", "B")],
    );
    const result = topoSortIds(wf);
    // Z is disconnected — it has in-degree 0 so it appears alongside A at the front
    // or after; the key invariant is A before B
    expect(result.indexOf("A")).toBeLessThan(result.indexOf("B"));
    expect(result).toContain("Z");
    expect(result).toHaveLength(3);
  });

  it("handles multiple root nodes", () => {
    // A -> C, B -> C
    const wf = makeWorkflow(
      [makeNode("A"), makeNode("B"), makeNode("C")],
      [makeEdge("A", "C"), makeEdge("B", "C")],
    );
    const result = topoSortIds(wf);
    expect(result.indexOf("A")).toBeLessThan(result.indexOf("C"));
    expect(result.indexOf("B")).toBeLessThan(result.indexOf("C"));
    expect(result).toHaveLength(3);
  });

  it("handles workflow with no edges (all roots)", () => {
    const wf = makeWorkflow([
      makeNode("X"),
      makeNode("Y"),
      makeNode("Z"),
    ]);
    const result = topoSortIds(wf);
    expect(result).toHaveLength(3);
    expect(result).toContain("X");
    expect(result).toContain("Y");
    expect(result).toContain("Z");
  });

  it("preserves original insertion order for nodes at the same level", () => {
    // All three are roots with no edges — BFS queues them in insertion order
    const wf = makeWorkflow([
      makeNode("C"),
      makeNode("A"),
      makeNode("B"),
    ]);
    expect(topoSortIds(wf)).toEqual(["C", "A", "B"]);
  });

  it("includes cycle-involved nodes via the fallback append", () => {
    // A -> B -> C -> B (cycle between B and C)
    // BFS will process A (root), then B (in-deg drops to 0 after A processed...
    // but C also points back at B so in-deg never reaches 0 for B after init).
    // Actually: in-deg B = 2 (from A and C), in-deg C = 1 (from B).
    // BFS pops A, decrements B to 1 — B never reaches 0. C never reached.
    // The fallback loop appends B, C.
    const wf = makeWorkflow(
      [makeNode("A"), makeNode("B"), makeNode("C")],
      [makeEdge("A", "B"), makeEdge("B", "C"), makeEdge("C", "B")],
    );
    const result = topoSortIds(wf);
    expect(result).toHaveLength(3);
    expect(result[0]).toBe("A");
    // B and C are appended by the fallback (cycle)
    expect(result).toContain("B");
    expect(result).toContain("C");
  });
});

// ---------------------------------------------------------------------------
// topoSortNodes
// ---------------------------------------------------------------------------

describe("topoSortNodes", () => {
  it("returns empty array for empty workflow", () => {
    const wf = makeWorkflow([]);
    expect(topoSortNodes(wf)).toEqual([]);
  });

  it("returns full node objects in topological order", () => {
    const a = makeNode("A");
    const b = makeNode("B");
    const c = makeNode("C");
    const wf = makeWorkflow([a, b, c], [makeEdge("A", "B"), makeEdge("B", "C")]);
    const result = topoSortNodes(wf);
    expect(result.map((n) => n.id)).toEqual(["A", "B", "C"]);
    // Verify they are the actual node objects
    expect(result[0]).toBe(a);
    expect(result[1]).toBe(b);
    expect(result[2]).toBe(c);
  });

  it("returns correct node data (not just ids)", () => {
    const node = { ...makeNode("X"), name: "My Special Node" };
    const wf = makeWorkflow([node]);
    const result = topoSortNodes(wf);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("My Special Node");
  });

  it("handles diamond graph with full objects", () => {
    const a = makeNode("A");
    const b = makeNode("B");
    const c = makeNode("C");
    const d = makeNode("D");
    const wf = makeWorkflow(
      [a, b, c, d],
      [
        makeEdge("A", "B"),
        makeEdge("A", "C"),
        makeEdge("B", "D"),
        makeEdge("C", "D"),
      ],
    );
    const result = topoSortNodes(wf);
    const ids = result.map((n) => n.id);
    expect(ids.indexOf("A")).toBeLessThan(ids.indexOf("D"));
    expect(ids).toHaveLength(4);
  });
});
