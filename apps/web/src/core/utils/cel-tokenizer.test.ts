import { describe, it, expect } from "vitest";
import { tokenizeCel, type CelToken } from "./cel-tokenizer";

function types(tokens: CelToken[]): string[] {
  return tokens.map((t) => t.type);
}

function texts(tokens: CelToken[]): string[] {
  return tokens.map((t) => t.text);
}

describe("tokenizeCel", () => {
  it("returns empty array for empty string", () => {
    expect(tokenizeCel("")).toEqual([]);
  });

  it("tokenizes a simple field path", () => {
    const tokens = tokenizeCel("nodes");
    expect(texts(tokens)).toEqual(["nodes"]);
    expect(types(tokens)).toEqual(["field"]);
  });

  it("tokenizes string literals with double quotes", () => {
    const tokens = tokenizeCel('"hello world"');
    expect(texts(tokens)).toEqual(['"hello world"']);
    expect(types(tokens)).toEqual(["string"]);
  });

  it("tokenizes string literals with single quotes", () => {
    const tokens = tokenizeCel("'foo'");
    expect(texts(tokens)).toEqual(["'foo'"]);
    expect(types(tokens)).toEqual(["string"]);
  });

  it("handles escaped quotes in strings", () => {
    const tokens = tokenizeCel('"say \\"hi\\"" ');
    expect(tokens[0].type).toBe("string");
    expect(tokens[0].text).toBe('"say \\"hi\\""');
  });

  it("tokenizes number literals", () => {
    expect(tokenizeCel("42")[0]).toEqual({ text: "42", type: "number" });
    expect(tokenizeCel("3.14")[0]).toEqual({ text: "3.14", type: "number" });
    expect(tokenizeCel(".5")[0]).toEqual({ text: ".5", type: "number" });
  });

  it("tokenizes keywords", () => {
    for (const kw of ["true", "false", "null", "in", "has"]) {
      expect(tokenizeCel(kw)[0].type).toBe("keyword");
    }
  });

  it("tokenizes operators", () => {
    const tokens = tokenizeCel("a == b != c >= d");
    const ops = tokens.filter((t) => t.type === "operator");
    expect(ops.map((t) => t.text)).toEqual(["==", "!=", ">="]);
  });

  it("tokenizes multi-char logical operators", () => {
    const tokens = tokenizeCel("a && b || c");
    const ops = tokens.filter((t) => t.type === "operator");
    expect(ops.map((t) => t.text)).toEqual(["&&", "||"]);
  });

  it("tokenizes brackets and parens", () => {
    const tokens = tokenizeCel('nodes["id"]');
    const brackets = tokens.filter((t) => t.type === "bracket");
    expect(brackets.map((t) => t.text)).toEqual(["[", "]"]);
  });

  it("identifies function calls (identifier followed by open paren)", () => {
    const tokens = tokenizeCel("size(items)");
    expect(tokens[0]).toEqual({ text: "size", type: "function" });
    expect(tokens[1]).toEqual({ text: "(", type: "bracket" });
    expect(tokens[2]).toEqual({ text: "items", type: "field" });
  });

  it("distinguishes function vs field by trailing paren", () => {
    const tokens = tokenizeCel("size(x) + length");
    expect(tokens.find((t) => t.text === "size")?.type).toBe("function");
    expect(tokens.find((t) => t.text === "length")?.type).toBe("field");
  });

  it("tokenizes a complex expression", () => {
    const tokens = tokenizeCel('nodes["http_1"].body.contains("error") && status >= 400');
    expect(tokens.length).toBeGreaterThan(5);
    // "error" should be a string token
    const strToken = tokens.find((t) => t.text === '"error"');
    expect(strToken?.type).toBe("string");
    // 400 should be a number
    const numToken = tokens.find((t) => t.text === "400");
    expect(numToken?.type).toBe("number");
    // && should be operator
    const andToken = tokens.find((t) => t.text === "&&");
    expect(andToken?.type).toBe("operator");
    // contains should be function (followed by paren)
    const containsToken = tokens.find((t) => t.text === "contains");
    expect(containsToken?.type).toBe("function");
  });

  it("handles unterminated string gracefully (no crash)", () => {
    const tokens = tokenizeCel('"unclosed');
    expect(tokens.length).toBe(1);
    expect(tokens[0].type).toBe("string");
    expect(tokens[0].text).toBe('"unclosed');
  });

  it("handles dots as default tokens", () => {
    const tokens = tokenizeCel("a.b");
    expect(texts(tokens)).toEqual(["a", ".", "b"]);
    expect(types(tokens)).toEqual(["field", "default", "field"]);
  });

  it("handles whitespace as default tokens", () => {
    const tokens = tokenizeCel("a b");
    expect(texts(tokens)).toEqual(["a", " ", "b"]);
  });

  it("handles empty parens", () => {
    const tokens = tokenizeCel("()");
    expect(texts(tokens)).toEqual(["(", ")"]);
    expect(types(tokens)).toEqual(["bracket", "bracket"]);
  });

  it("handles nested function calls", () => {
    const tokens = tokenizeCel("size(filter(items))");
    const fns = tokens.filter((t) => t.type === "function");
    expect(fns.map((t) => t.text)).toEqual(["size", "filter"]);
  });

  it("handles all bracket types", () => {
    const tokens = tokenizeCel("()[]{}");
    expect(texts(tokens)).toEqual(["(", ")", "[", "]", "{", "}"]);
    expect(types(tokens)).toEqual(Array(6).fill("bracket"));
  });

  it("handles scientific notation partially", () => {
    // tokenizer grabs digits+dots+e/E characters
    const tokens = tokenizeCel("1e5");
    expect(tokens[0].text).toBe("1e5");
    expect(tokens[0].type).toBe("number");
  });
});
