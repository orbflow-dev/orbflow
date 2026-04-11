import { describe, it, expect } from "vitest";
import { previewCelExpression, type CelPreviewResult } from "./cel-preview";
import type { UpstreamOutput } from "./upstream";

const mockUpstream: UpstreamOutput[] = [
  {
    nodeId: "http_1",
    nodeName: "Get Users",
    pluginRef: "builtin:http_request",
    fields: [
      { key: "status", label: "Status", type: "number", required: false },
      { key: "status_text", label: "Status Text", type: "string", required: false },
      {
        key: "body",
        label: "Body",
        type: "object",
        required: false,
        children: [
          { key: "name", label: "Name", type: "string", required: false },
          { key: "age", label: "Age", type: "number", required: false },
          { key: "active", label: "Active", type: "boolean", required: false },
          { key: "tags", label: "Tags", type: "array", required: false },
        ],
      },
      { key: "headers", label: "Headers", type: "object", required: false },
    ],
  },
];

describe("previewCelExpression", () => {
  // Empty / whitespace
  it("returns unknown for empty expression", () => {
    expect(previewCelExpression("", mockUpstream).status).toBe("unknown");
  });

  it("returns unknown for whitespace-only", () => {
    expect(previewCelExpression("   ", mockUpstream).status).toBe("unknown");
  });

  // Field paths
  it("resolves a simple field path", () => {
    const result = previewCelExpression('nodes["http_1"].status', mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("number");
    expect(result.preview).toBe("0");
  });

  it("resolves nested field path", () => {
    const result = previewCelExpression('nodes["http_1"].body.name', mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("string");
    expect(result.preview).toBe('"..."');
  });

  it("resolves boolean field", () => {
    const result = previewCelExpression('nodes["http_1"].body.active', mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("boolean");
    expect(result.preview).toBe("true");
  });

  it("resolves array field", () => {
    const result = previewCelExpression('nodes["http_1"].body.tags', mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("array");
    expect(result.preview).toBe("[...]");
  });

  it("resolves object field", () => {
    const result = previewCelExpression('nodes["http_1"].body', mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("object");
    expect(result.preview).toBe("{...}");
  });

  it("resolves node root without field path", () => {
    const result = previewCelExpression('nodes["http_1"]', mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("object");
  });

  it("returns partial for unknown field on known node", () => {
    const result = previewCelExpression('nodes["http_1"].nonexistent', mockUpstream);
    expect(result.status).toBe("partial");
  });

  it("returns unknown for unknown node", () => {
    const result = previewCelExpression('nodes["unknown_node"].field', mockUpstream);
    expect(result.status).toBe("unknown");
    expect(result.preview).toBe("unknown node");
  });

  // Context variables
  it("resolves vars", () => {
    const result = previewCelExpression("vars", mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("object");
  });

  it("resolves vars.something", () => {
    const result = previewCelExpression("vars.myVar", mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("string");
  });

  it("resolves trigger", () => {
    const result = previewCelExpression("trigger", mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("object");
  });

  // Literals
  it("resolves string literal", () => {
    const result = previewCelExpression('"hello"', mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("string");
    expect(result.preview).toBe('"hello"');
  });

  it("resolves single-quoted string literal", () => {
    const result = previewCelExpression("'world'", mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("string");
  });

  it("resolves number literal", () => {
    const result = previewCelExpression("42", mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("number");
    expect(result.preview).toBe("42");
  });

  it("resolves float literal", () => {
    const result = previewCelExpression("3.14", mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("number");
  });

  it("resolves boolean literals", () => {
    expect(previewCelExpression("true", mockUpstream).type).toBe("boolean");
    expect(previewCelExpression("false", mockUpstream).type).toBe("boolean");
  });

  // Functions
  it("resolves size() as number", () => {
    const result = previewCelExpression('size(nodes["http_1"].body.tags)', mockUpstream);
    expect(result.type).toBe("number");
    expect(result.preview).toContain("length");
  });

  it("resolves .contains() as boolean", () => {
    const result = previewCelExpression('nodes["http_1"].status_text.contains("OK")', mockUpstream);
    expect(result.type).toBe("boolean");
    expect(result.preview).toBe("true / false");
  });

  it("resolves .startsWith() as boolean", () => {
    const result = previewCelExpression('nodes["http_1"].body.name.startsWith("J")', mockUpstream);
    expect(result.type).toBe("boolean");
  });

  it("resolves .endsWith() as boolean", () => {
    const result = previewCelExpression('nodes["http_1"].body.name.endsWith("n")', mockUpstream);
    expect(result.type).toBe("boolean");
  });

  it("resolves .matches() as boolean", () => {
    const result = previewCelExpression('nodes["http_1"].body.name.matches("^[A-Z]")', mockUpstream);
    expect(result.type).toBe("boolean");
  });

  it("resolves int() as number", () => {
    const result = previewCelExpression('int("42")', mockUpstream);
    expect(result.type).toBe("number");
  });

  it("resolves string() as string", () => {
    const result = previewCelExpression("string(42)", mockUpstream);
    expect(result.type).toBe("string");
  });

  it("resolves bool() as boolean", () => {
    const result = previewCelExpression("bool(1)", mockUpstream);
    expect(result.type).toBe("boolean");
  });

  it("resolves has() as boolean", () => {
    const result = previewCelExpression('has(nodes["http_1"].body.name)', mockUpstream);
    expect(result.type).toBe("boolean");
  });

  // Operators
  it("resolves comparison operators as boolean", () => {
    const result = previewCelExpression('nodes["http_1"].status == 200', mockUpstream);
    expect(result.type).toBe("boolean");
  });

  it("resolves logical operators as boolean", () => {
    const result = previewCelExpression("a > 1 && b < 2", mockUpstream);
    expect(result.type).toBe("boolean");
  });

  // Ternary
  it("resolves ternary as partial", () => {
    const result = previewCelExpression("x > 0 ? 1 : 0", mockUpstream);
    expect(result.status).toBe("partial");
    expect(result.preview).toContain("conditional");
  });

  // Unknown expressions
  it("returns partial for complex unrecognized expressions", () => {
    const result = previewCelExpression("something + other", mockUpstream);
    expect(result.status).toBe("partial");
  });

  // Edge cases
  it("handles expression with spaces", () => {
    const result = previewCelExpression('  nodes["http_1"].status  ', mockUpstream);
    expect(result.status).toBe("resolved");
    expect(result.type).toBe("number");
  });

  it("handles empty upstream", () => {
    const result = previewCelExpression('nodes["http_1"].status', []);
    expect(result.status).toBe("unknown");
  });
});
