import { describe, it, expect } from "vitest";
import { validateCel } from "./cel-validation";

describe("validateCel", () => {
  it("accepts empty/whitespace expressions", () => {
    expect(validateCel("")).toEqual({ valid: true });
    expect(validateCel("   ")).toEqual({ valid: true });
  });

  it("accepts simple expressions", () => {
    expect(validateCel("1 + 2").valid).toBe(true);
    expect(validateCel("true").valid).toBe(true);
    expect(validateCel('nodes["http-1"].body').valid).toBe(true);
  });

  it("accepts balanced parentheses", () => {
    expect(validateCel("(1 + 2) * 3").valid).toBe(true);
    expect(validateCel("((a + b))").valid).toBe(true);
  });

  it("accepts balanced brackets", () => {
    expect(validateCel('nodes["id"].field').valid).toBe(true);
    expect(validateCel("list[0]").valid).toBe(true);
  });

  it("accepts strings with parens/brackets inside", () => {
    expect(validateCel('"hello (world)"').valid).toBe(true);
    expect(validateCel("'array[0]'").valid).toBe(true);
  });

  it("accepts escaped quotes in strings", () => {
    expect(validateCel('"she said \\"hi\\""').valid).toBe(true);
    expect(validateCel("'it\\'s fine'").valid).toBe(true);
  });

  it("rejects unmatched closing paren", () => {
    const result = validateCel("1 + 2)");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Unmatched )");
    expect(result.position).toBe(5);
  });

  it("rejects unmatched closing bracket", () => {
    const result = validateCel("a]");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Unmatched ]");
    expect(result.position).toBe(1);
  });

  it("rejects unclosed paren", () => {
    const result = validateCel("(1 + 2");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Unclosed (");
    expect(result.position).toBe(0);
  });

  it("rejects unclosed bracket", () => {
    const result = validateCel("list[0");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Unclosed [");
    expect(result.position).toBe(4);
  });

  it("rejects unterminated double-quoted string", () => {
    const result = validateCel('"hello');
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Unterminated string");
  });

  it("rejects unterminated single-quoted string", () => {
    const result = validateCel("'hello");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Unterminated string");
  });

  it("rejects mismatched paren/bracket", () => {
    const result = validateCel("(a + b]");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Unmatched ]");
  });

  it("handles complex nested expressions", () => {
    expect(validateCel('nodes["a"].list[0].map(x, x + 1)').valid).toBe(true);
    expect(validateCel("size(nodes[0].items.filter(x, x > 0))").valid).toBe(true);
  });

  it("detects trailing dot", () => {
    const result = validateCel("nodes.");
    expect(result.valid).toBe(false);
    expect(result.error).toContain("ends with a dot");
  });

  it("detects trailing logical operator", () => {
    const result = validateCel("a > 1 &&");
    expect(result.valid).toBe(false);
    expect(result.error).toContain("logical operator");
  });

  it("accepts complete expressions with operators", () => {
    expect(validateCel("a == 1").valid).toBe(true);
    expect(validateCel("a != 1").valid).toBe(true);
    expect(validateCel("a >= 1 && b <= 2").valid).toBe(true);
  });

  it("detects method without a field", () => {
    const result = validateCel('.contains("hi")');
    expect(result.valid).toBe(false);
    expect(result.error).toContain("Method needs a field first");
  });

  it("accepts method on a field", () => {
    expect(validateCel('nodes["a"].body.contains("hi")').valid).toBe(true);
    expect(validateCel('vars.name.startsWith("foo")').valid).toBe(true);
  });

  it("handles curly braces", () => {
    expect(validateCel("{a: 1}").valid).toBe(true);
    const result = validateCel("{a: 1");
    expect(result.valid).toBe(false);
    expect(result.error).toBe("Unclosed {");
  });

  it("detects double dots", () => {
    const result = validateCel('nodes["a"]..body');
    expect(result.valid).toBe(false);
    expect(result.error).toContain("Double dot");
  });

  it("detects single = instead of ==", () => {
    const result = validateCel("a = 1");
    expect(result.valid).toBe(false);
    expect(result.error).toContain("== for comparison");
  });

  it("allows = inside bracket syntax", () => {
    expect(validateCel('nodes["id"].field').valid).toBe(true);
  });

  it("detects empty function call", () => {
    const result = validateCel("size()");
    expect(result.valid).toBe(false);
    expect(result.error).toContain("needs an argument");
  });

  it("accepts function call with argument", () => {
    expect(validateCel('size(nodes["a"].items)').valid).toBe(true);
  });

  it("returns severity for warnings", () => {
    const result = validateCel('.contains("hi")');
    expect(result.severity).toBe("error");
  });
});
