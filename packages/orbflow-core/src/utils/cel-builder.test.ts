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

function fieldMapping(mapping: Omit<FieldMapping, "targetKey">): FieldMapping {
  return { targetKey: "target", ...mapping };
}

function conditionRule(rule: Omit<ConditionRule, "id">): ConditionRule {
  return { id: "rule", ...rule };
}

function conditionGroup(group: Omit<ConditionGroup, "id">): ConditionGroup {
  return { id: "group", ...group };
}

describe("buildMappingExpression", () => {
  it("returns static string value as-is", () => {
    const mapping = fieldMapping({
      mode: "static",
      staticValue: "hello",
    });
    expect(buildMappingExpression(mapping)).toBe("hello");
  });

  it("returns JSON for non-string static values", () => {
    const mapping = fieldMapping({
      mode: "static",
      staticValue: 42,
    });
    expect(buildMappingExpression(mapping)).toBe("42");
  });

  it("returns empty string for undefined static value", () => {
    const mapping = fieldMapping({
      mode: "static",
      staticValue: undefined,
    });
    expect(buildMappingExpression(mapping)).toBe("");
  });

  it("builds CEL reference from sourceNodeId and sourcePath", () => {
    const mapping = fieldMapping({
      mode: "expression",
      sourceNodeId: "http-1",
      sourcePath: "body.data",
    });
    expect(buildMappingExpression(mapping)).toBe(
      '=nodes["http-1"].body.data',
    );
  });

  it("escapes node ids in generated CEL references", () => {
    const mapping = fieldMapping({
      mode: "expression",
      sourceNodeId: 'http"node\\prod',
      sourcePath: "body.data",
    });
    expect(buildMappingExpression(mapping)).toBe(
      '=nodes["http\\"node\\\\prod"].body.data',
    );
  });

  it("uses bracket access for source path segments that are not CEL identifiers", () => {
    const mapping = fieldMapping({
      mode: "expression",
      sourceNodeId: "http-1",
      sourcePath: 'body.error-message."quoted key"',
    });
    expect(buildMappingExpression(mapping)).toBe(
      '=nodes["http-1"].body["error-message"]["\\"quoted key\\""]',
    );
  });

  it("prepends = to celExpression if missing", () => {
    const mapping = fieldMapping({
      mode: "expression",
      celExpression: 'nodes["http-1"].status',
    });
    expect(buildMappingExpression(mapping)).toBe(
      '=nodes["http-1"].status',
    );
  });

  it("preserves = prefix in celExpression", () => {
    const mapping = fieldMapping({
      mode: "expression",
      celExpression: '=nodes["http-1"].status',
    });
    expect(buildMappingExpression(mapping)).toBe(
      '=nodes["http-1"].status',
    );
  });

  it("returns empty string for expression with no data", () => {
    const mapping = fieldMapping({ mode: "expression" });
    expect(buildMappingExpression(mapping)).toBe("");
  });
});

describe("serializeMappings", () => {
  it("serializes multiple mappings, omitting empty values", () => {
    const mappings: Record<string, FieldMapping> = {
      url: fieldMapping({ mode: "static", staticValue: "https://api.example.com" }),
      body: fieldMapping({
        mode: "expression",
        sourceNodeId: "transform-1",
        sourcePath: "result",
      }),
      empty: fieldMapping({ mode: "expression" }),
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
    const rule = conditionRule({
      field: 'nodes["http-1"].status',
      operator: "==",
      value: 200,
    });
    expect(buildConditionExpression(rule)).toBe(
      'nodes["http-1"].status == 200',
    );
  });

  it("quotes string values", () => {
    const rule = conditionRule({
      field: 'nodes["http-1"].method',
      operator: "==",
      value: "GET",
    });
    expect(buildConditionExpression(rule)).toBe(
      'nodes["http-1"].method == "GET"',
    );
  });

  it("escapes string literals in comparison rules", () => {
    const rule = conditionRule({
      field: 'nodes["http-1"].body.message',
      operator: "==",
      value: 'line 1\nline 2 says "ok" in C:\\tmp',
    });
    expect(buildConditionExpression(rule)).toBe(
      'nodes["http-1"].body.message == "line 1\\nline 2 says \\"ok\\" in C:\\\\tmp"',
    );
  });

  it("builds contains operator", () => {
    const rule = conditionRule({
      field: 'nodes["http-1"].body',
      operator: "contains",
      value: "error",
    });
    expect(buildConditionExpression(rule)).toBe(
      'nodes["http-1"].body.contains("error")',
    );
  });

  it("escapes string literals in string operator calls", () => {
    const rule = conditionRule({
      field: 'nodes["http-1"].body',
      operator: "contains",
      value: 'bad "token"\\suffix',
    });
    expect(buildConditionExpression(rule)).toBe(
      'nodes["http-1"].body.contains("bad \\"token\\"\\\\suffix")',
    );
  });

  it("joins AND group with &&", () => {
    const group = conditionGroup({
      logic: "and",
      rules: [
        conditionRule({ field: "a", operator: "==", value: 1 }),
        conditionRule({ field: "b", operator: ">", value: 10 }),
      ],
    });
    expect(buildConditionExpression(group)).toBe("a == 1 && b > 10");
  });

  it("joins OR group with ||", () => {
    const group = conditionGroup({
      logic: "or",
      rules: [
        conditionRule({ field: "x", operator: "==", value: "yes" }),
        conditionRule({ field: "y", operator: "!=", value: "no" }),
      ],
    });
    expect(buildConditionExpression(group)).toBe(
      'x == "yes" || y != "no"',
    );
  });

  it("returns true for empty group", () => {
    const group = conditionGroup({ logic: "and", rules: [] });
    expect(buildConditionExpression(group)).toBe("true");
  });

  it("unwraps single-rule group", () => {
    const group = conditionGroup({
      logic: "and",
      rules: [conditionRule({ field: "x", operator: ">=", value: 5 })],
    });
    expect(buildConditionExpression(group)).toBe("x >= 5");
  });

  it("wraps nested groups in parens", () => {
    const group = conditionGroup({
      logic: "and",
      rules: [
        conditionRule({ field: "a", operator: "==", value: 1 }),
        conditionGroup({
          logic: "or",
          rules: [
            conditionRule({ field: "b", operator: "==", value: 2 }),
            conditionRule({ field: "c", operator: "==", value: 3 }),
          ],
        }),
      ],
    });
    expect(buildConditionExpression(group)).toBe(
      "a == 1 && (b == 2 || c == 3)",
    );
  });
});
