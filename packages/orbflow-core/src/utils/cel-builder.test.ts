import { describe, it, expect } from "vitest";
import {
  buildMappingExpression,
  serializeMappings,
  buildConditionExpression,
} from "./cel-builder";
import type {
  FieldMapping,
  ConditionRule,
  ConditionGroup,
} from "../types/schema";

describe("buildMappingExpression", () => {
  it("returns static string value as-is", () => {
    const mapping: FieldMapping = {
      mode: "static",
      staticValue: "hello",
    };
    expect(buildMappingExpression(mapping)).toBe("hello");
  });

  it("returns JSON for non-string static values", () => {
    const mapping: FieldMapping = {
      mode: "static",
      staticValue: 42,
    };
    expect(buildMappingExpression(mapping)).toBe("42");
  });

  it("returns empty string for undefined static value", () => {
    const mapping: FieldMapping = {
      mode: "static",
      staticValue: undefined,
    };
    expect(buildMappingExpression(mapping)).toBe("");
  });

  it("builds CEL reference from sourceNodeId and sourcePath", () => {
    const mapping: FieldMapping = {
      mode: "expression",
      sourceNodeId: "http-1",
      sourcePath: "body.data",
    };
    expect(buildMappingExpression(mapping)).toBe(
      '=nodes["http-1"].body.data',
    );
  });

  it("prepends = to celExpression if missing", () => {
    const mapping: FieldMapping = {
      mode: "expression",
      celExpression: 'nodes["http-1"].status',
    };
    expect(buildMappingExpression(mapping)).toBe(
      '=nodes["http-1"].status',
    );
  });

  it("preserves = prefix in celExpression", () => {
    const mapping: FieldMapping = {
      mode: "expression",
      celExpression: '=nodes["http-1"].status',
    };
    expect(buildMappingExpression(mapping)).toBe(
      '=nodes["http-1"].status',
    );
  });

  it("returns empty string for expression with no data", () => {
    const mapping: FieldMapping = { mode: "expression" };
    expect(buildMappingExpression(mapping)).toBe("");
  });
});

describe("serializeMappings", () => {
  it("serializes multiple mappings, omitting empty values", () => {
    const mappings: Record<string, FieldMapping> = {
      url: { mode: "static", staticValue: "https://api.example.com" },
      body: {
        mode: "expression",
        sourceNodeId: "transform-1",
        sourcePath: "result",
      },
      empty: { mode: "expression" },
    };
    expect(serializeMappings(mappings)).toEqual({
      url: "https://api.example.com",
      body: '=nodes["transform-1"].result',
    });
  });

  it("returns empty object for empty mappings", () => {
    expect(serializeMappings({})).toEqual({});
  });
});

describe("buildConditionExpression", () => {
  it("builds a simple equality rule", () => {
    const rule: ConditionRule = {
      field: 'nodes["http-1"].status',
      operator: "==",
      value: 200,
    };
    expect(buildConditionExpression(rule)).toBe(
      'nodes["http-1"].status == 200',
    );
  });

  it("quotes string values", () => {
    const rule: ConditionRule = {
      field: 'nodes["http-1"].method',
      operator: "==",
      value: "GET",
    };
    expect(buildConditionExpression(rule)).toBe(
      'nodes["http-1"].method == "GET"',
    );
  });

  it("builds contains operator", () => {
    const rule: ConditionRule = {
      field: 'nodes["http-1"].body',
      operator: "contains",
      value: "error",
    };
    expect(buildConditionExpression(rule)).toBe(
      'nodes["http-1"].body.contains("error")',
    );
  });

  it("joins AND group with &&", () => {
    const group: ConditionGroup = {
      logic: "and",
      rules: [
        { field: "a", operator: "==", value: 1 },
        { field: "b", operator: ">", value: 10 },
      ],
    };
    expect(buildConditionExpression(group)).toBe("a == 1 && b > 10");
  });

  it("joins OR group with ||", () => {
    const group: ConditionGroup = {
      logic: "or",
      rules: [
        { field: "x", operator: "==", value: "yes" },
        { field: "y", operator: "!=", value: "no" },
      ],
    };
    expect(buildConditionExpression(group)).toBe(
      'x == "yes" || y != "no"',
    );
  });

  it("returns true for empty group", () => {
    const group: ConditionGroup = { logic: "and", rules: [] };
    expect(buildConditionExpression(group)).toBe("true");
  });

  it("unwraps single-rule group", () => {
    const group: ConditionGroup = {
      logic: "and",
      rules: [{ field: "x", operator: ">=", value: 5 }],
    };
    expect(buildConditionExpression(group)).toBe("x >= 5");
  });

  it("wraps nested groups in parens", () => {
    const group: ConditionGroup = {
      logic: "and",
      rules: [
        { field: "a", operator: "==", value: 1 },
        {
          logic: "or",
          rules: [
            { field: "b", operator: "==", value: 2 },
            { field: "c", operator: "==", value: 3 },
          ],
        },
      ],
    };
    expect(buildConditionExpression(group)).toBe(
      "a == 1 && (b == 2 || c == 3)",
    );
  });
});
