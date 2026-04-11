import { describe, it, expect } from "vitest";
import {
  generateUntitledName,
  sanitizeImportedWorkflow,
  buildExportPayload,
} from "./use-workflow-io";
import type { Workflow } from "@/lib/api";

// -- generateUntitledName --------------------------

describe("generateUntitledName", () => {
  it("returns base name when no conflicts", () => {
    expect(generateUntitledName([])).toBe("Untitled Workflow");
  });

  it("returns base name when existing names don't conflict", () => {
    expect(generateUntitledName(["My Workflow", "Other"])).toBe(
      "Untitled Workflow",
    );
  });

  it("increments when base name is taken", () => {
    expect(generateUntitledName(["Untitled Workflow"])).toBe(
      "Untitled Workflow 2",
    );
  });

  it("skips taken numbers", () => {
    expect(
      generateUntitledName([
        "Untitled Workflow",
        "Untitled Workflow 2",
        "Untitled Workflow 3",
      ]),
    ).toBe("Untitled Workflow 4");
  });

  it("fills gaps in numbering", () => {
    expect(
      generateUntitledName(["Untitled Workflow", "Untitled Workflow 3"]),
    ).toBe("Untitled Workflow 2");
  });
});

// -- sanitizeImportedWorkflow ----------------------

describe("sanitizeImportedWorkflow", () => {
  it("returns null for objects without name", () => {
    expect(sanitizeImportedWorkflow({ nodes: [] })).toBeNull();
  });

  it("returns null for objects without nodes array", () => {
    expect(sanitizeImportedWorkflow({ name: "test" })).toBeNull();
  });

  it("returns null for non-string name", () => {
    expect(sanitizeImportedWorkflow({ name: 42, nodes: [] })).toBeNull();
  });

  it("sanitizes a valid workflow", () => {
    const raw = {
      name: "My Flow",
      description: "A test",
      nodes: [
        {
          id: "n1",
          name: "HTTP Request",
          type: "builtin",
          plugin_ref: "http_request",
          position: { x: 100, y: 200 },
          input_mapping: { url: "https://example.com" },
        },
      ],
      edges: [
        { id: "e1", source: "n1", target: "n2", condition: "true" },
      ],
    };

    const result = sanitizeImportedWorkflow(raw);
    expect(result).not.toBeNull();
    expect(result!.name).toBe("My Flow (imported)");
    expect(result!.description).toBe("A test");
    expect(result!.nodes).toHaveLength(1);
    expect(result!.nodes![0].id).toBe("n1");
    expect(result!.nodes![0].position).toEqual({ x: 100, y: 200 });
    expect(result!.nodes![0].input_mapping).toEqual({
      url: "https://example.com",
    });
    expect(result!.edges).toHaveLength(1);
    expect(result!.edges![0].condition).toBe("true");
  });

  it("defaults missing node fields", () => {
    const raw = { name: "Test", nodes: [{}] };
    const result = sanitizeImportedWorkflow(raw);
    expect(result!.nodes![0]).toEqual({
      id: "",
      name: "",
      type: "builtin",
      plugin_ref: "",
      position: { x: 0, y: 0 },
      input_mapping: undefined,
    });
  });

  it("defaults edges to empty array when missing", () => {
    const raw = { name: "Test", nodes: [] };
    const result = sanitizeImportedWorkflow(raw);
    expect(result!.edges).toEqual([]);
  });

  it("strips unknown fields from the raw object", () => {
    const raw = {
      name: "Test",
      nodes: [],
      edges: [],
      malicious_field: "DROP TABLE",
      __proto__: { admin: true },
    };
    const result = sanitizeImportedWorkflow(raw);
    expect(result).not.toBeNull();
    expect("malicious_field" in result!).toBe(false);
    // Only expected keys present
    const keys = Object.keys(result!);
    expect(keys.sort()).toEqual(
      ["description", "edges", "name", "nodes"].sort(),
    );
  });

  it("handles non-string description gracefully", () => {
    const raw = { name: "Test", description: 123, nodes: [] };
    const result = sanitizeImportedWorkflow(raw);
    expect(result!.description).toBeUndefined();
  });

  it("handles edge without condition", () => {
    const raw = {
      name: "Test",
      nodes: [],
      edges: [{ id: "e1", source: "a", target: "b" }],
    };
    const result = sanitizeImportedWorkflow(raw);
    expect(result!.edges![0].condition).toBeUndefined();
  });
});

// -- buildExportPayload ----------------------------

describe("buildExportPayload", () => {
  const baseWorkflow: Workflow = {
    id: "wf-1",
    name: "Test Workflow",
    version: 1,
    status: "active",
    nodes: [
      { id: "n1", name: "A", type: "builtin", plugin_ref: "http", position: { x: 0, y: 0 } },
      { id: "n2", name: "B", type: "builtin", plugin_ref: "log", position: { x: 100, y: 100 } },
    ],
    edges: [{ id: "e1", source: "n1", target: "n2" }],
    created_at: "2025-01-01",
    updated_at: "2025-01-01",
  };

  it("overlays live canvas positions onto stored nodes", () => {
    const canvasNodes = [
      { id: "n1", position: { x: 300, y: 400 } },
      { id: "n2", position: { x: 500, y: 600 } },
    ];
    const result = buildExportPayload(baseWorkflow, canvasNodes);
    expect(result.nodes[0].position).toEqual({ x: 300, y: 400 });
    expect(result.nodes[1].position).toEqual({ x: 500, y: 600 });
  });

  it("keeps stored position when node is not on canvas", () => {
    const canvasNodes = [{ id: "n1", position: { x: 300, y: 400 } }];
    const result = buildExportPayload(baseWorkflow, canvasNodes);
    expect(result.nodes[0].position).toEqual({ x: 300, y: 400 });
    expect(result.nodes[1].position).toEqual({ x: 100, y: 100 });
  });

  it("does not mutate the original workflow", () => {
    const canvasNodes = [{ id: "n1", position: { x: 999, y: 999 } }];
    buildExportPayload(baseWorkflow, canvasNodes);
    expect(baseWorkflow.nodes[0].position).toEqual({ x: 0, y: 0 });
  });

  it("preserves all non-position workflow fields", () => {
    const result = buildExportPayload(baseWorkflow, []);
    expect(result.id).toBe("wf-1");
    expect(result.name).toBe("Test Workflow");
    expect(result.edges).toEqual(baseWorkflow.edges);
  });
});
