import { describe, it, expect } from "vitest";
import { inferFieldType, inferFieldsFromData } from "./upstream";

describe("inferFieldType", () => {
  it("returns string for null", () => {
    expect(inferFieldType(null)).toBe("string");
  });

  it("returns string for undefined", () => {
    expect(inferFieldType(undefined)).toBe("string");
  });

  it("returns number for numbers", () => {
    expect(inferFieldType(42)).toBe("number");
    expect(inferFieldType(3.14)).toBe("number");
    expect(inferFieldType(0)).toBe("number");
  });

  it("returns boolean for booleans", () => {
    expect(inferFieldType(true)).toBe("boolean");
    expect(inferFieldType(false)).toBe("boolean");
  });

  it("returns array for arrays", () => {
    expect(inferFieldType([1, 2, 3])).toBe("array");
    expect(inferFieldType([])).toBe("array");
  });

  it("returns object for objects", () => {
    expect(inferFieldType({ a: 1 })).toBe("object");
    expect(inferFieldType({})).toBe("object");
  });

  it("returns string for strings", () => {
    expect(inferFieldType("hello")).toBe("string");
    expect(inferFieldType("")).toBe("string");
  });
});

describe("inferFieldsFromData", () => {
  it("returns empty array for non-object input", () => {
    expect(inferFieldsFromData(null as unknown as Record<string, unknown>)).toEqual([]);
    expect(inferFieldsFromData([] as unknown as Record<string, unknown>)).toEqual([]);
  });

  it("infers fields from flat object", () => {
    const data = { name: "test", count: 42, active: true };
    const fields = inferFieldsFromData(data);
    expect(fields).toHaveLength(3);
    expect(fields[0]).toMatchObject({ key: "name", type: "string", dynamic: true });
    expect(fields[1]).toMatchObject({ key: "count", type: "number", dynamic: true });
    expect(fields[2]).toMatchObject({ key: "active", type: "boolean", dynamic: true });
  });

  it("recurses into nested objects", () => {
    const data = { response: { status: 200, body: "ok" } };
    const fields = inferFieldsFromData(data);
    expect(fields).toHaveLength(1);
    expect(fields[0].key).toBe("response");
    expect(fields[0].type).toBe("object");
    expect(fields[0].children).toHaveLength(2);
    expect(fields[0].children![0]).toMatchObject({
      key: "status",
      type: "number",
    });
  });

  it("respects maxDepth", () => {
    const data = { a: { b: { c: { d: "deep" } } } };
    const fields = inferFieldsFromData(data, 2);
    // At depth 2, recursion stops — nested object children are empty
    expect(fields[0].children![0].children).toEqual([]);
  });

  it("limits keys per level to MAX_KEYS_PER_LEVEL", () => {
    const data: Record<string, unknown> = {};
    for (let i = 0; i < 150; i++) {
      data[`key_${i}`] = i;
    }
    const fields = inferFieldsFromData(data);
    expect(fields.length).toBe(100);
  });

  it("returns empty array at max depth", () => {
    expect(inferFieldsFromData({ a: 1 }, 4, 4)).toEqual([]);
  });
});
