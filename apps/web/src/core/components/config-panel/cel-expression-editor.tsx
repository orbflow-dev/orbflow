"use client";

import { useState, useRef, useCallback, useEffect, useMemo } from "react";
import type { UpstreamOutput } from "../../utils/upstream";
import { validateCel } from "../../utils/cel-validation";
import { tokenizeCel, TOKEN_COLORS } from "../../utils/cel-tokenizer";
import { CEL_FUNCTIONS, flattenFields, type Suggestion } from "../../utils/cel-suggestions";
import { previewCelExpression } from "../../utils/cel-preview";
import { NodeIcon } from "../icons";
import { cn } from "../../utils/cn";

interface CelExpressionEditorProps {
  value: string;
  upstream: UpstreamOutput[];
  onChange: (value: string) => void;
  onToggleBrowser: () => void;
  showBrowser: boolean;
}

/** Suggestion kind icons */
function SuggestionIcon({ kind }: { kind: Suggestion["kind"] }) {
  const shared = "w-3.5 h-3.5 shrink-0";
  switch (kind) {
    case "field":
      return <NodeIcon name="database" className={cn(shared, "text-electric-indigo/60")} />;
    case "function":
      return <NodeIcon name="code" className={cn(shared, "text-blue-400/70")} />;
    case "context":
      return <NodeIcon name="zap" className={cn(shared, "text-amber-400/70")} />;
    case "node":
      return <NodeIcon name="box" className={cn(shared, "text-emerald-400/70")} />;
  }
}

export function CelExpressionEditor({
  value,
  upstream,
  onChange,
  onToggleBrowser,
  showBrowser,
}: CelExpressionEditorProps) {
  const [expanded, setExpanded] = useState(false);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [selectedSuggestion, setSelectedSuggestion] = useState(0);
  const [isFocused, setIsFocused] = useState(false);
  const [showFnRef, setShowFnRef] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement | HTMLInputElement | null>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);
  const blurTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Callback ref to handle textarea | input union without `as never`
  const setInputRef = useCallback((el: HTMLTextAreaElement | HTMLInputElement | null) => {
    inputRef.current = el;
  }, []);

  const validation = useMemo(() => validateCel(value), [value]);

  // Live preview -- resolves field paths and infers result types
  const preview = useMemo(() => previewCelExpression(value, upstream), [value, upstream]);

  // Syntax-highlighted tokens for the overlay
  const highlightedTokens = useMemo(() => tokenizeCel(value), [value]);

  // Build a flat list of ALL available field paths for proactive suggestions
  const allPaths = useMemo((): Suggestion[] => {
    const paths: Suggestion[] = [];
    for (const node of upstream) {
      paths.push(
        ...flattenFields(
          node.fields,
          node.nodeId,
          node.nodeName,
          `nodes["${node.nodeId}"]`,
          "",
        ),
      );
    }
    // Add context suggestions
    paths.push(
      { label: "vars", celPath: "vars", detail: "workflow variables", group: "Context", kind: "context" },
      { label: "trigger", celPath: "trigger", detail: "trigger data", group: "Context", kind: "context" },
    );
    return paths;
  }, [upstream]);

  // Pre-built node summaries -- breaks double-invalidation of upstream in suggestions
  const nodeSummaries = useMemo((): Suggestion[] =>
    upstream.map((n) => ({
      label: n.nodeName,
      celPath: `nodes["${n.nodeId}"]`,
      detail: n.pluginRef,
      group: "Nodes",
      kind: "node" as const,
    })),
  [upstream]);

  // Extract the "active token" -- the part the user is currently typing.
  // When inside function parens like `size(no`, the active token is `no`.
  // When at top level like `nodes["id"].bo`, the active token is the full value.
  const activeToken = useMemo((): string => {
    const trimmed = value.trim();
    if (!trimmed) return "";
    // Find last unmatched open paren -- everything after it is the active argument
    let depth = 0;
    let lastOpenIdx = -1;
    for (let i = trimmed.length - 1; i >= 0; i--) {
      if (trimmed[i] === ")") depth++;
      else if (trimmed[i] === "(") {
        if (depth > 0) depth--;
        else { lastOpenIdx = i; break; }
      }
    }
    if (lastOpenIdx >= 0) {
      // Inside a function call -- use text after the `(`
      return trimmed.slice(lastOpenIdx + 1).trim();
    }
    // Also check after last comma (multiple arguments)
    const lastComma = trimmed.lastIndexOf(",");
    if (lastComma >= 0) {
      return trimmed.slice(lastComma + 1).trim();
    }
    return trimmed;
  }, [value]);

  // Whether we're inside a function call (suggesting arguments, not top-level expressions)
  const insideFunctionCall = useMemo((): boolean => {
    const trimmed = value.trim();
    let depth = 0;
    for (let i = trimmed.length - 1; i >= 0; i--) {
      if (trimmed[i] === ")") depth++;
      else if (trimmed[i] === "(") {
        if (depth > 0) depth--;
        else return true;
      }
    }
    return false;
  }, [value]);

  // Filter suggestions based on current input value
  const suggestions = useMemo((): Suggestion[] => {
    // When empty and focused: show all top-level paths (not deeply nested ones for cleanliness)
    if (!value.trim()) {
      const topLevel = allPaths.filter((p) => !p.label.includes(".")).slice(0, 15);
      return [...topLevel, ...CEL_FUNCTIONS.slice(0, 5)];
    }

    // Use the active token for searching (handles inside-parens context)
    const query = activeToken.toLowerCase();

    // Inside a function call with empty argument -- show all top-level fields to pick from
    if (insideFunctionCall && !activeToken) {
      return allPaths.filter((p) => !p.label.includes(".")).slice(0, 20);
    }

    // Check if the active token is a partial path after nodes["nodeId"].
    const fieldMatch = activeToken.match(/nodes\["([^"]+)"\]\.(.*)$/);
    if (fieldMatch) {
      const nodeId = fieldMatch[1];
      const partial = fieldMatch[2].toLowerCase();
      const nodePrefix = `nodes["${nodeId}"].`;

      return allPaths
        .filter((p) => {
          if (!p.celPath.startsWith(nodePrefix)) return false;
          const remainder = p.celPath.slice(nodePrefix.length).toLowerCase();
          return remainder.startsWith(partial) && remainder !== partial;
        })
        .slice(0, 15)
        .map((p) => ({
          ...p,
          // Show only the part after the prefix the user already typed
          label: p.celPath.slice(nodePrefix.length),
        }));
    }

    // Check if user is typing 'nodes.' or 'nodes["'
    if (activeToken.endsWith("nodes.") || activeToken.endsWith('nodes["')) {
      return nodeSummaries;
    }

    // Dot-trigger: when active token ends with `.` after a resolved path,
    // show sub-fields for that path AND method suggestions (.contains, etc.)
    if (activeToken.endsWith(".")) {
      const pathBeforeDot = activeToken.slice(0, -1);
      const nodeFieldMatch = pathBeforeDot.match(/^nodes\["([^"]+)"\]\.?(.*)$/);
      if (nodeFieldMatch) {
        const nodeId = nodeFieldMatch[1];
        const subPath = nodeFieldMatch[2];
        const prefix = `nodes["${nodeId}"]${subPath ? "." + subPath : ""}`;

        // Find sub-fields at this path
        const subFields = allPaths
          .filter((p) => {
            if (!p.celPath.startsWith(prefix + ".")) return false;
            // Only immediate children (one level deeper)
            const remainder = p.celPath.slice(prefix.length + 1);
            return !remainder.includes(".");
          })
          .slice(0, 15);

        // Also offer method suggestions (e.g. .contains, .startsWith)
        const methods = CEL_FUNCTIONS.filter((f) => f.celPath.startsWith("."));

        return [...subFields, ...methods];
      }
    }

    // General fuzzy search on the active token -- include functions only at top level
    const fieldMatches = allPaths
      .filter((p) =>
        p.label.toLowerCase().includes(query) ||
        p.celPath.toLowerCase().includes(query) ||
        p.group.toLowerCase().includes(query),
      )
      .slice(0, 12);

    // Only show function suggestions when not already inside a function call
    if (insideFunctionCall) {
      return fieldMatches;
    }

    const fnMatches = CEL_FUNCTIONS
      .filter((f) => f.label.toLowerCase().includes(query) || f.detail.toLowerCase().includes(query))
      .slice(0, 5);

    return [...fieldMatches, ...fnMatches];
  }, [value, activeToken, insideFunctionCall, allPaths, nodeSummaries]);

  // Build O(1) index map for suggestion lookup (fixes O(n²) indexOf in render loop)
  const suggestionIndexMap = useMemo(() => {
    const map = new Map<Suggestion, number>();
    suggestions.forEach((s, i) => map.set(s, i));
    return map;
  }, [suggestions]);

  // Show suggestions on focus or when there are matches
  useEffect(() => {
    if (isFocused && suggestions.length > 0) {
      setShowSuggestions(true);
    } else {
      setShowSuggestions(false);
    }
    setSelectedSuggestion(0);
  }, [suggestions, isFocused]);

  // Scroll selected suggestion into view
  useEffect(() => {
    if (!showSuggestions || !suggestionsRef.current) return;
    const items = suggestionsRef.current.querySelectorAll("[data-suggestion]");
    const item = items[selectedSuggestion];
    if (item) {
      item.scrollIntoView({ block: "nearest" });
    }
  }, [selectedSuggestion, showSuggestions]);

  const applySuggestion = useCallback((suggestion: Suggestion) => {
    if (suggestion.kind === "function") {
      // For method-style functions (.contains, .startsWith, etc.)
      if (suggestion.celPath.startsWith(".")) {
        onChange(value + suggestion.celPath);
      } else {
        // For standalone functions (size(), has(), int(), etc.)
        onChange(suggestion.celPath);
      }
    } else if (insideFunctionCall) {
      // Inside a function call -- replace only the active token (text after last open paren)
      const trimmed = value.trim();
      let depth = 0;
      let lastOpenIdx = -1;
      for (let i = trimmed.length - 1; i >= 0; i--) {
        if (trimmed[i] === ")") depth++;
        else if (trimmed[i] === "(") {
          if (depth > 0) depth--;
          else { lastOpenIdx = i; break; }
        }
      }
      const prefix = lastOpenIdx >= 0 ? trimmed.slice(0, lastOpenIdx + 1) : trimmed;
      const isNodePick = suggestion.kind === "node";
      onChange(prefix + suggestion.celPath + (isNodePick ? "." : ""));
    } else {
      // Top-level field / context / node suggestions
      const fieldMatch = activeToken.match(/^(nodes\["[^"]+"\]\.)(.*)$/);
      if (fieldMatch) {
        // Replace just the partial after the node prefix
        const prefixEnd = value.lastIndexOf(fieldMatch[1]) + fieldMatch[1].length;
        onChange(value.slice(0, prefixEnd) + suggestion.label);
      } else if (activeToken.endsWith("nodes.") || activeToken.endsWith('nodes["')) {
        onChange(suggestion.celPath + ".");
      } else {
        onChange(suggestion.celPath);
      }
    }
    setShowSuggestions(false);
    requestAnimationFrame(() => inputRef.current?.focus());
  }, [value, activeToken, insideFunctionCall, onChange]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!showSuggestions || suggestions.length === 0) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedSuggestion((i) => (i + 1) % suggestions.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedSuggestion((i) => (i - 1 + suggestions.length) % suggestions.length);
    } else if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      applySuggestion(suggestions[selectedSuggestion]);
    } else if (e.key === "Escape") {
      setShowSuggestions(false);
    }
  }, [showSuggestions, suggestions, selectedSuggestion, applySuggestion]);

  const handleFocus = useCallback(() => {
    setIsFocused(true);
  }, []);

  const handleBlur = useCallback(() => {
    // Delay to allow click on suggestion
    blurTimerRef.current = setTimeout(() => {
      setIsFocused(false);
      setShowSuggestions(false);
    }, 200);
  }, []);

  // Cleanup blur timer on unmount
  useEffect(() => {
    return () => {
      if (blurTimerRef.current !== null) clearTimeout(blurTimerRef.current);
    };
  }, []);

  const InputComponent = expanded ? "textarea" : "input";

  // Group suggestions for display
  const groupedSuggestions = useMemo(() => {
    const groups: { group: string; items: Suggestion[] }[] = [];
    const seen = new Map<string, Suggestion[]>();
    for (const s of suggestions) {
      const existing = seen.get(s.group);
      if (existing) {
        existing.push(s);
      } else {
        const items = [s];
        seen.set(s.group, items);
        groups.push({ group: s.group, items });
      }
    }
    return groups;
  }, [suggestions]);

  return (
    <div className="space-y-1.5">
      {/* Accessible label */}
      <div className="sr-only" id="cel-editor-label">CEL expression editor</div>

      {/* Wrapper for input + dropdown -- relative so dropdown escapes the overflow-hidden inner box */}
      <div className="relative">
        <div className={cn(
          "flex items-stretch rounded-lg border transition-colors duration-200 bg-orbflow-surface",
          isFocused
            ? "border-electric-indigo/40 shadow-[0_0_0_1px_rgba(124,92,252,0.15)]"
            : validation.valid || !value.trim()
              ? "border-electric-indigo/20"
              : "border-rose-400/30",
          expanded && "flex-col"
        )}>
          {/* fx badge */}
          <div className={cn(
            "flex items-center justify-center shrink-0 border-electric-indigo/15 transition-colors duration-200 rounded-l-lg",
            isFocused ? "bg-electric-indigo/15" : "bg-electric-indigo/10",
            expanded ? "h-8 border-b rounded-l-none rounded-t-lg" : "w-9 h-10 border-r"
          )}>
            <span className={cn(
              "text-body font-bold font-mono italic transition-colors duration-200",
              isFocused ? "text-electric-indigo" : "text-electric-indigo/70"
            )}>fx</span>
          </div>

          {/* Input area with syntax highlighting overlay */}
          <div className="relative flex-1 overflow-hidden">
            {/* Syntax highlight overlay (non-expanded only) */}
            {!expanded && value && (
              <div
                className="absolute inset-0 px-3 py-2.5 font-mono text-body-lg pointer-events-none whitespace-nowrap overflow-hidden"
                aria-hidden="true"
              >
                {highlightedTokens.map((token, i) => (
                  <span key={i} className={TOKEN_COLORS[token.type]}>
                    {token.text}
                  </span>
                ))}
              </div>
            )}

            <InputComponent
              ref={setInputRef}
              type={expanded ? undefined : "text"}
              value={value}
              onChange={(e: React.ChangeEvent<HTMLTextAreaElement | HTMLInputElement>) => onChange(e.target.value)}
              onKeyDown={handleKeyDown}
              onFocus={handleFocus}
              onBlur={handleBlur}
              placeholder="Type expression or pick a field..."
              aria-labelledby="cel-editor-label"
              aria-autocomplete="list"
              aria-expanded={showSuggestions}
              aria-controls="cel-suggestions"
              role="combobox"
              spellCheck={false}
              className={cn(
                "w-full bg-transparent text-body-lg font-mono placeholder:text-electric-indigo/20 focus-visible:ring-0 focus-visible:outline-none transition-all",
                expanded
                  ? "px-3 py-2 min-h-[80px] resize-y text-orbflow-text-secondary"
                  : "px-3 py-2.5 text-transparent caret-electric-indigo/60",
              )}
            />
          </div>

          {/* Action buttons */}
          <div className={cn(
            "flex shrink-0",
            expanded ? "border-t border-electric-indigo/15 justify-end px-1 py-0.5" : "items-center"
          )}>
            {/* Validation indicator */}
            {value.trim() && (
              <div
                className={cn(
                  "flex items-center justify-center w-7 h-7 transition-colors duration-200",
                  expanded ? "" : "h-10"
                )}
                title={validation.valid ? "Expression looks valid" : validation.error}
              >
                {validation.valid ? (
                  <div className="w-4 h-4 rounded-full bg-emerald-400/15 flex items-center justify-center">
                    <NodeIcon name="check" className="w-2.5 h-2.5 text-emerald-400" />
                  </div>
                ) : (
                  <div className="w-4 h-4 rounded-full bg-rose-400/15 flex items-center justify-center animate-pulse-soft">
                    <NodeIcon name="x" className="w-2.5 h-2.5 text-rose-400" />
                  </div>
                )}
              </div>
            )}

            {/* Function reference toggle */}
            <button
              onClick={() => setShowFnRef(!showFnRef)}
              className={cn(
                "w-7 h-7 flex items-center justify-center transition-colors rounded cursor-pointer",
                "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                showFnRef ? "text-blue-400/70" : "text-orbflow-text-ghost hover:text-blue-400/50"
              )}
              title="Function reference"
              aria-label="Function reference"
              aria-pressed={showFnRef}
            >
              <NodeIcon name="book-open" className="w-3 h-3" />
            </button>

            {/* Expand/collapse toggle */}
            <button
              onClick={() => setExpanded(!expanded)}
              className="w-7 h-7 flex items-center justify-center text-orbflow-text-ghost hover:text-electric-indigo/60 transition-colors
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none rounded cursor-pointer"
              title={expanded ? "Collapse to single line" : "Expand to multi-line"}
              aria-label={expanded ? "Collapse editor" : "Expand editor"}
            >
              <NodeIcon name={expanded ? "minimize" : "maximize"} className="w-3 h-3" />
            </button>

            {/* Browse fields button */}
            <button
              onClick={onToggleBrowser}
              className={cn(
                "w-7 h-7 flex items-center justify-center transition-colors rounded cursor-pointer",
                "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                showBrowser ? "text-electric-indigo/60" : "text-orbflow-text-ghost hover:text-electric-indigo/60"
              )}
              title="Browse fields"
              aria-label="Browse fields"
              aria-pressed={showBrowser}
            >
              <NodeIcon name="database" className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>

        {/* Contextual hint when inside a function call */}
        {isFocused && insideFunctionCall && !activeToken && (
          <div className="px-3 py-1.5 text-caption text-electric-indigo/50 flex items-center gap-1.5">
            <NodeIcon name="info" className="w-3 h-3 shrink-0" />
            {(() => {
              // Find which function we're inside to show its specific arg hint
              const trimmed = value.trim();
              const fnMatch = trimmed.match(/\.?(\w+)\($/);
              if (fnMatch) {
                const fnName = fnMatch[1];
                const fn = CEL_FUNCTIONS.find((f) => f.celPath.endsWith(fnName + "("));
                if (fn?.argHint) {
                  const needsQuotes = fn.argHint.startsWith('"');
                  return needsQuotes
                    ? <>Type {fn.argHint} -- <span className="font-mono text-electric-indigo/70">strings need quotes</span></>
                    : <>Pick a field below, or type {fn.argHint}</>;
                }
              }
              return "Pick a field from the suggestions, or type a value";
            })()}
          </div>
        )}

        {/* Autocomplete dropdown -- positioned outside the overflow-hidden input box */}
        {showSuggestions && suggestions.length > 0 && (
          <div
            id="cel-suggestions"
            ref={suggestionsRef}
            role="listbox"
            className="absolute left-0 right-0 top-full z-20 mt-1 max-h-60 overflow-y-auto
              rounded-lg border border-orbflow-border bg-orbflow-surface shadow-xl shadow-black/30
              animate-scale-in origin-top"
          >
            {groupedSuggestions.map(({ group, items }) => (
              <div key={group}>
                <div className="px-3 py-1.5 text-micro font-bold uppercase tracking-wider text-orbflow-text-ghost bg-orbflow-bg/80 backdrop-blur-sm sticky top-0 flex items-center gap-1.5">
                  {(group === "Functions" || group === "Methods") && <NodeIcon name="code" className="w-2.5 h-2.5" />}
                  {group === "Context" && <NodeIcon name="zap" className="w-2.5 h-2.5" />}
                  {group === "Nodes" && <NodeIcon name="box" className="w-2.5 h-2.5" />}
                  {group !== "Functions" && group !== "Methods" && group !== "Context" && group !== "Nodes" && (
                    <NodeIcon name="database" className="w-2.5 h-2.5" />
                  )}
                  {group}
                  {group === "Methods" && (
                    <span className="text-orbflow-text-ghost font-normal normal-case tracking-normal ml-1">-- pick a field first</span>
                  )}
                </div>
                {items.map((s) => {
                  const globalIdx = suggestionIndexMap.get(s) ?? 0;
                  return (
                    <button
                      key={`${s.celPath}-${globalIdx}`}
                      data-suggestion
                      role="option"
                      aria-selected={globalIdx === selectedSuggestion}
                      onMouseDown={(e) => { e.preventDefault(); applySuggestion(s); }}
                      className={cn(
                        "w-full flex items-center gap-2 px-3 py-1.5 text-left text-body transition-colors cursor-pointer",
                        globalIdx === selectedSuggestion
                          ? "bg-electric-indigo/10 text-electric-indigo"
                          : "text-orbflow-text-secondary hover:bg-orbflow-surface-hover"
                      )}
                    >
                      <SuggestionIcon kind={s.kind} />
                      <span className="font-mono font-medium truncate">{s.label}</span>
                      <span className="ml-auto text-caption text-orbflow-text-ghost shrink-0">{s.detail}</span>
                    </button>
                  );
                })}
              </div>
            ))}

            {/* Keyboard hints */}
            <div className="px-3 py-1.5 border-t border-orbflow-border flex items-center gap-3 text-micro text-orbflow-text-ghost">
              <span><kbd className="px-1 py-0.5 rounded bg-orbflow-bg border border-orbflow-border font-mono">↑↓</kbd> navigate</span>
              <span><kbd className="px-1 py-0.5 rounded bg-orbflow-bg border border-orbflow-border font-mono">Tab</kbd> select</span>
              <span><kbd className="px-1 py-0.5 rounded bg-orbflow-bg border border-orbflow-border font-mono">Esc</kbd> close</span>
            </div>
          </div>
        )}
      </div>

      {/* Live preview + validation feedback */}
      <div aria-live="polite" aria-atomic="true">
        {value.trim() ? (
          !validation.valid ? (
            /* Validation error */
            <div
              className={cn(
                "text-caption flex items-start gap-1.5 rounded-md px-2.5 py-1.5",
                validation.severity === "warning"
                  ? "text-amber-400/80 bg-amber-400/[0.06] border border-amber-400/10"
                  : "text-rose-400/80 bg-rose-400/[0.06] border border-rose-400/10",
              )}
            >
              <NodeIcon name="alert-triangle" className="w-3 h-3 shrink-0 mt-px" />
              <span className="font-medium">{validation.error}</span>
            </div>
          ) : (
            /* Live preview -- shown when expression is valid */
            <div className="flex items-center gap-2 px-2.5 py-1.5 rounded-md bg-orbflow-bg/50 border border-orbflow-border/50">
              <NodeIcon name="zap" className="w-3 h-3 text-orbflow-text-ghost shrink-0" />
              <span className="text-caption text-orbflow-text-ghost">Result:</span>
              <span className={cn(
                "text-caption font-mono font-medium",
                preview.status === "resolved" ? "text-emerald-400/80" :
                preview.status === "partial" ? "text-amber-400/70" :
                "text-orbflow-text-muted",
              )}>
                {preview.preview}
              </span>
              {preview.type && (
                <span className={cn(
                  "text-micro px-1.5 py-px rounded-full border ml-auto shrink-0",
                  preview.type === "string" ? "text-emerald-400/60 border-emerald-400/15 bg-emerald-400/[0.06]" :
                  preview.type === "number" ? "text-amber-400/60 border-amber-400/15 bg-amber-400/[0.06]" :
                  preview.type === "boolean" ? "text-purple-400/60 border-purple-400/15 bg-purple-400/[0.06]" :
                  preview.type === "object" ? "text-blue-400/60 border-blue-400/15 bg-blue-400/[0.06]" :
                  preview.type === "array" ? "text-pink-400/60 border-pink-400/15 bg-pink-400/[0.06]" :
                  "text-orbflow-text-ghost border-orbflow-border bg-orbflow-bg",
                )}>
                  {preview.type}
                </span>
              )}
            </div>
          )
        ) : null}
      </div>

      {/* Function reference panel */}
      {showFnRef && (
        <div className="rounded-lg border border-orbflow-border bg-orbflow-bg/90 backdrop-blur-sm overflow-hidden animate-scale-in origin-top">
          <div className="px-3 py-2 border-b border-orbflow-border flex items-center justify-between">
            <span className="text-body-sm font-medium text-orbflow-text-muted flex items-center gap-1.5">
              <NodeIcon name="code" className="w-3 h-3 text-blue-400/60" />
              CEL Functions
            </span>
            <button
              onClick={() => setShowFnRef(false)}
              className="w-5 h-5 flex items-center justify-center rounded text-orbflow-text-ghost hover:text-orbflow-text-muted transition-colors cursor-pointer
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
              aria-label="Close function reference"
            >
              <NodeIcon name="x" className="w-3 h-3" />
            </button>
          </div>
          <div className="max-h-48 overflow-y-auto custom-scrollbar p-1">
            {CEL_FUNCTIONS.map((fn) => (
              <button
                key={fn.celPath}
                onMouseDown={(e) => {
                  e.preventDefault();
                  // Insert function at cursor -- append to current value
                  const insertion = fn.celPath.startsWith(".")
                    ? value + fn.celPath
                    : value ? value + " " + fn.celPath : fn.celPath;
                  onChange(insertion);
                  setShowFnRef(false);
                  requestAnimationFrame(() => inputRef.current?.focus());
                }}
                className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-md text-left transition-colors cursor-pointer
                  hover:bg-orbflow-surface-hover group
                  focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
              >
                <NodeIcon name="code" className="w-3 h-3 text-blue-400/50 shrink-0" />
                <span className="font-mono text-body-sm text-blue-400/80 group-hover:text-blue-400">{fn.label}</span>
                <span className="ml-auto text-caption text-orbflow-text-ghost">{fn.detail}</span>
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
