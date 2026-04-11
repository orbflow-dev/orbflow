/** CEL expression tokenizer for syntax highlighting */

export interface CelToken {
  text: string;
  type: "keyword" | "string" | "number" | "operator" | "function" | "field" | "bracket" | "default";
}

const CEL_KEYWORDS = new Set(["true", "false", "null", "in", "has"]);

/** Tokenize a CEL expression string into typed tokens for syntax coloring. */
export function tokenizeCel(expr: string): CelToken[] {
  const tokens: CelToken[] = [];
  let i = 0;

  while (i < expr.length) {
    const c = expr[i];

    // Strings
    if (c === '"' || c === "'") {
      let str = c;
      i++;
      while (i < expr.length && expr[i] !== c) {
        if (expr[i] === "\\") { str += expr[i]; i++; }
        if (i < expr.length) { str += expr[i]; i++; }
      }
      if (i < expr.length) { str += expr[i]; i++; }
      tokens.push({ text: str, type: "string" });
      continue;
    }

    // Numbers
    if (/\d/.test(c) || (c === "." && i + 1 < expr.length && /\d/.test(expr[i + 1]))) {
      let num = "";
      while (i < expr.length && /[\d.eE]/.test(expr[i])) { num += expr[i]; i++; }
      tokens.push({ text: num, type: "number" });
      continue;
    }

    // Operators
    if ("=!<>&|+-*/%".includes(c)) {
      let op = c;
      i++;
      if (i < expr.length && "=&|".includes(expr[i])) { op += expr[i]; i++; }
      tokens.push({ text: op, type: "operator" });
      continue;
    }

    // Brackets / parens
    if ("()[]{}".includes(c)) {
      tokens.push({ text: c, type: "bracket" });
      i++;
      continue;
    }

    // Identifiers (keywords, functions, fields)
    if (/[a-zA-Z_]/.test(c)) {
      let ident = "";
      while (i < expr.length && /[a-zA-Z0-9_]/.test(expr[i])) { ident += expr[i]; i++; }
      if (CEL_KEYWORDS.has(ident)) {
        tokens.push({ text: ident, type: "keyword" });
      } else if (i < expr.length && expr[i] === "(") {
        tokens.push({ text: ident, type: "function" });
      } else {
        tokens.push({ text: ident, type: "field" });
      }
      continue;
    }

    // Dots and other chars
    tokens.push({ text: c, type: "default" });
    i++;
  }

  return tokens;
}

/** Theme-aware CSS class per token type */
export const TOKEN_COLORS: Record<CelToken["type"], string> = {
  keyword: "cel-token-keyword",
  string: "cel-token-string",
  number: "cel-token-number",
  operator: "cel-token-operator",
  function: "cel-token-function",
  field: "cel-token-field",
  bracket: "text-orbflow-text-muted",
  default: "text-orbflow-text-secondary",
};
