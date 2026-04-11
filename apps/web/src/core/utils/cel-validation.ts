/** CEL validation result with richer diagnostics */
export interface CelValidation {
  valid: boolean;
  error?: string;
  /** Position in the expression where the error was detected */
  position?: number;
  /** Severity: error blocks execution, warning is informational */
  severity?: "error" | "warning";
}

/** Validate CEL expression: checks balanced parens/brackets/braces, quotes, and common mistakes. */
export function validateCel(expr: string): CelValidation {
  if (!expr.trim()) return { valid: true };

  const stack: { char: string; pos: number }[] = [];
  let inString = false;
  let strChar = "";

  for (let i = 0; i < expr.length; i++) {
    const c = expr[i];

    if (inString) {
      if (c === "\\" && i + 1 < expr.length) { i++; continue; }
      if (c === strChar) inString = false;
      continue;
    }

    if (c === '"' || c === "'") { inString = true; strChar = c; continue; }

    if (c === "(" || c === "[" || c === "{") {
      stack.push({ char: c, pos: i });
      continue;
    }

    const matchMap: Record<string, string> = { ")": "(", "]": "[", "}": "{" };
    if (c in matchMap) {
      if (stack.length === 0 || stack[stack.length - 1].char !== matchMap[c]) {
        return { valid: false, error: `Unmatched ${c}`, position: i, severity: "error" };
      }
      stack.pop();
    }
  }

  if (inString) {
    return { valid: false, error: "Unterminated string", position: expr.length - 1, severity: "error" };
  }

  if (stack.length > 0) {
    const unclosed = stack[stack.length - 1];
    return { valid: false, error: `Unclosed ${unclosed.char}`, position: unclosed.pos, severity: "error" };
  }

  // Check for common mistakes
  const trimmed = expr.trim();

  // Method called without a subject: .contains("hi") instead of field.contains("hi")
  if (/^\.\w+\(/.test(trimmed)) {
    return { valid: false, error: 'Method needs a field first -- e.g. field.contains("text")', position: 0, severity: "error" };
  }

  // Empty function call with no argument: size() instead of size(field)
  if (/\w+\(\s*\)/.test(trimmed) && !/\.\w+\(\s*\)/.test(trimmed)) {
    const match = trimmed.match(/(\w+)\(\s*\)/);
    if (match && !["true", "false", "null"].includes(match[1])) {
      return { valid: false, error: `${match[1]}() needs an argument`, position: trimmed.indexOf(match[0]), severity: "error" };
    }
  }

  // Double dots: nodes["id"]..field
  if (/\.\./.test(trimmed)) {
    return { valid: false, error: "Double dot -- remove the extra dot", position: trimmed.indexOf(".."), severity: "error" };
  }

  // Trailing dot
  if (/\.\s*$/.test(trimmed)) {
    return { valid: false, error: "Expression ends with a dot -- select a field or method", position: trimmed.length - 1, severity: "error" };
  }

  // Single = instead of == -- strip bracket contents and strings first to avoid false positives
  {
    const stripped = trimmed
      .replace(/\["[^"]*"\]/g, "[]")   // nodes["id"] -> nodes[]
      .replace(/"[^"]*"/g, '""')       // "string" -> ""
      .replace(/'[^']*'/g, "''");      // 'string' -> ''
    const eqMatch = stripped.match(/[^=!<>]=[^=]/);
    if (eqMatch) {
      const eqIdx = stripped.indexOf(eqMatch[0]);
      return { valid: false, error: "Use == for comparison, not =", position: eqIdx + 1, severity: "error" };
    }
  }

  // Incomplete operator at end
  if (/[=!<>]{1}\s*$/.test(trimmed) && !trimmed.endsWith(">=") && !trimmed.endsWith("<=") && !trimmed.endsWith("!=") && !trimmed.endsWith("==")) {
    return { valid: false, error: "Expression ends with an incomplete operator", position: trimmed.length - 1, severity: "error" };
  }

  // Trailing logical operator
  if (/&&\s*$/.test(trimmed) || /\|\|\s*$/.test(trimmed)) {
    return { valid: false, error: "Expression ends with a logical operator -- add right-hand side", position: trimmed.length - 1, severity: "error" };
  }

  // Unquoted string argument in contains/startsWith/endsWith/matches
  const strMethodMatch = trimmed.match(/\.(contains|startsWith|endsWith|matches)\(([^)]+)\)/);
  if (strMethodMatch) {
    const arg = strMethodMatch[2].trim();
    // If arg doesn't start with a quote and isn't a field path or number, it's likely an unquoted string
    if (arg && !arg.startsWith('"') && !arg.startsWith("'") && !/^[\w\[\]"._]+$/.test(arg) && !/^\d/.test(arg)) {
      return { valid: false, error: `Argument to .${strMethodMatch[1]}() looks like an unquoted string -- wrap in quotes`, position: trimmed.indexOf(arg), severity: "warning" };
    }
  }

  return { valid: true };
}
