import { describe, it, expect } from "vitest";
import { detectShape, isUrl, isHttpResponse } from "./data-shape-utils";

describe("detectShape", () => {
  it("returns 'empty' for null/undefined", () => {
    expect(detectShape(null)).toBe("empty");
    expect(detectShape(undefined)).toBe("empty");
  });

  it("returns 'primitive-string' for strings", () => {
    expect(detectShape("hello")).toBe("primitive-string");
    expect(detectShape("")).toBe("primitive-string");
  });

  it("returns 'primitive-number' for numbers", () => {
    expect(detectShape(42)).toBe("primitive-number");
    expect(detectShape(0)).toBe("primitive-number");
    expect(detectShape(-3.14)).toBe("primitive-number");
  });

  it("returns 'primitive-boolean' for booleans", () => {
    expect(detectShape(true)).toBe("primitive-boolean");
    expect(detectShape(false)).toBe("primitive-boolean");
  });

  it("returns 'empty' for empty arrays", () => {
    expect(detectShape([])).toBe("empty");
  });

  it("returns 'array-objects' for arrays of objects", () => {
    expect(detectShape([{ a: 1 }, { b: 2 }])).toBe("array-objects");
  });

  it("returns 'array-primitives' for arrays of primitives", () => {
    expect(detectShape([1, 2, 3])).toBe("array-primitives");
    expect(detectShape(["a", "b"])).toBe("array-primitives");
  });

  it("returns 'array-primitives' for mixed arrays", () => {
    expect(detectShape([1, "two", { three: 3 }])).toBe("array-primitives");
  });

  it("returns 'empty' for empty objects", () => {
    expect(detectShape({})).toBe("empty");
  });

  it("returns 'flat-object' for objects with only primitive values", () => {
    expect(detectShape({ name: "test", count: 5, active: true })).toBe("flat-object");
  });

  it("returns 'flat-object' for objects with null values", () => {
    expect(detectShape({ a: 1, b: null })).toBe("flat-object");
  });

  it("returns 'nested-object' for objects with nested objects", () => {
    expect(detectShape({ a: 1, b: { c: 2 } })).toBe("nested-object");
  });

  it("returns 'nested-object' for objects with array values", () => {
    expect(detectShape({ items: [1, 2, 3] })).toBe("nested-object");
  });
});

describe("isUrl", () => {
  it("returns true for http URLs", () => {
    expect(isUrl("http://example.com")).toBe(true);
  });

  it("returns true for https URLs", () => {
    expect(isUrl("https://example.com/path?q=1")).toBe(true);
  });

  it("returns false for non-URL strings", () => {
    expect(isUrl("hello world")).toBe(false);
    expect(isUrl("ftp://files.example.com")).toBe(false);
  });

  it("returns false for non-string values", () => {
    expect(isUrl(123)).toBe(false);
    expect(isUrl(null)).toBe(false);
    expect(isUrl(undefined)).toBe(false);
  });
});

describe("isHttpResponse", () => {
  it("detects statusCode + body", () => {
    expect(isHttpResponse({ statusCode: 200, body: "ok" })).toBe(true);
  });

  it("detects status_code + headers", () => {
    expect(isHttpResponse({ status_code: 404, headers: {} })).toBe(true);
  });

  it("detects status + body", () => {
    expect(isHttpResponse({ status: 200, body: "{}" })).toBe(true);
  });

  it("returns false for plain objects", () => {
    expect(isHttpResponse({ name: "test" })).toBe(false);
  });

  it("returns false for non-objects", () => {
    expect(isHttpResponse(null)).toBe(false);
    expect(isHttpResponse("string")).toBe(false);
    expect(isHttpResponse(42)).toBe(false);
  });
});
