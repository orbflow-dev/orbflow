"use client";

import { useMemo, useState, useRef, useEffect, useCallback, memo } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import type { Workflow, NodeState } from "@/lib/api";
import { StructuredOutput } from "./structured-output";
import { STATUS_THEMES, FALLBACK_THEME, topoSortNodes } from "@/lib/execution";
import { copyToClipboard } from "@/lib/clipboard";

/* -- JSON syntax highlighter ----------------------- */

type TokenKind = "key" | "string" | "number" | "bool" | "plain";

interface Token {
  kind: TokenKind;
  text: string;
}

function tokenizeJson(json: string): Token[] {
  const tokens: Token[] = [];
  const regex = /("(?:[^"\\]|\\.)*")\s*:|:\s*("(?:[^"\\]|\\.)*")|:\s*(\d+\.?\d*)|:\s*(true|false|null)|([^":\d]+)/g;
  let match: RegExpExecArray | null;
  let lastIndex = 0;

  while ((match = regex.exec(json)) !== null) {
    if (match.index > lastIndex) {
      tokens.push({ kind: "plain", text: json.slice(lastIndex, match.index) });
    }
    if (match[1] !== undefined) {
      tokens.push({ kind: "key", text: match[1] });
      tokens.push({ kind: "plain", text: ":" });
    } else if (match[2] !== undefined) {
      tokens.push({ kind: "plain", text: ": " });
      tokens.push({ kind: "string", text: match[2] });
    } else if (match[3] !== undefined) {
      tokens.push({ kind: "plain", text: ": " });
      tokens.push({ kind: "number", text: match[3] });
    } else if (match[4] !== undefined) {
      tokens.push({ kind: "plain", text: ": " });
      tokens.push({ kind: "bool", text: match[4] });
    } else if (match[5] !== undefined) {
      tokens.push({ kind: "plain", text: match[5] });
    }
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < json.length) {
    tokens.push({ kind: "plain", text: json.slice(lastIndex) });
  }
  return tokens;
}

const TOKEN_CLASS: Record<TokenKind, string> = {
  key: "exec-json-key",
  string: "exec-json-string",
  number: "exec-json-number",
  bool: "exec-json-bool",
  plain: "",
};

function JsonHighlight({ json }: { json: string }) {
  const tokens = useMemo(() => tokenizeJson(json), [json]);

  return (
    <pre className="overflow-x-auto whitespace-pre-wrap break-words font-mono text-xs leading-relaxed text-orbflow-text-faint">
      {tokens.map((t, i) =>
        t.kind === "plain" ? (
          t.text
        ) : (
          <span key={i} className={TOKEN_CLASS[t.kind]}>
            {t.text}
          </span>
        )
      )}
    </pre>
  );
}

/* -- Copy button ----------------------------------- */

function CopyBtn({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    []
  );

  const copy = useCallback(async () => {
    await copyToClipboard(text);
    setCopied(true);
    timer.current = setTimeout(() => setCopied(false), 1500);
  }, [text]);

  return (
    <button
      onClick={copy}
      aria-label={copied ? "Copied" : "Copy to clipboard"}
      className={cn(
        "flex items-center gap-1 px-2 py-0.5 rounded text-caption font-medium transition-colors",
        "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
        copied
          ? "text-emerald-400/70"
          : "text-orbflow-text-ghost hover:text-orbflow-text-faint"
      )}
    >
      <NodeIcon
        name={copied ? "check" : "clipboard"}
        className="w-2.5 h-2.5"
      />
      {copied ? "Copied" : "Copy"}
    </button>
  );
}

/* -- Step item in timeline ------------------------- */

const StepItem = memo(function StepItem({
  node,
  nodeState,
  isLast,
  isExpanded,
  onToggle,
  onViewDetails,
}: {
  node: Workflow["nodes"][number];
  nodeState: NodeState | undefined;
  isLast: boolean;
  isExpanded: boolean;
  onToggle: () => void;
  onViewDetails?: () => void;
}) {
  const status = nodeState?.status || "pending";
  const theme = STATUS_THEMES[status] || FALLBACK_THEME;
  const isRunning = status === "running";

  const hasDetail =
    nodeState && (nodeState.output || nodeState.input || nodeState.error);

  return (
    <div className="relative flex gap-0">
      {/* Timeline track */}
      <div className="flex flex-col items-center w-8 shrink-0">
        {/* Dot */}
        <div className="relative mt-2.5">
          <div
            className={cn(
              "w-[9px] h-[9px] rounded-full transition-colors",
              isRunning && "animate-exec-step-pulse"
            )}
            style={{
              backgroundColor:
                status === "pending" ? "transparent" : theme.accent,
              border:
                status === "pending"
                  ? `2px solid ${theme.accent}`
                  : `2px solid ${theme.accent}`,
              opacity: status === "pending" ? 0.4 : 0.8,
            }}
          />
        </div>
        {/* Line */}
        {!isLast && (
          <div
            className="flex-1 w-px mt-1"
            style={{
              backgroundColor: "rgba(100,116,139,0.12)",
            }}
          />
        )}
      </div>

      {/* Content */}
      <div className={cn("flex-1 min-w-0", isLast ? "pb-0" : "pb-1")}>
        <button
          onClick={onToggle}
          disabled={!hasDetail}
          className={cn(
            "w-full flex items-center gap-3 rounded-xl px-3.5 py-3 text-left transition-colors",
            "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
            hasDetail
              ? "hover:bg-orbflow-surface-hover/40 cursor-pointer"
              : "cursor-not-allowed opacity-50",
            isExpanded && "bg-orbflow-surface-hover/25"
          )}
        >
          {/* Node name + ID */}
          <div className="flex-1 min-w-0">
            <p className="truncate text-sm font-medium leading-tight text-orbflow-text-secondary">
              {node.name || node.id}
            </p>
            <p className="mt-0.5 truncate font-mono text-xs text-orbflow-text-ghost/60">
              {node.id}
            </p>
          </div>

          {/* Status label */}
          <span
            className="shrink-0 rounded-full px-2.5 py-1 text-[11px] font-medium"
            style={{
              color: theme.text,
              backgroundColor: theme.bg,
            }}
          >
            {theme.label}
          </span>

          {/* Expand indicator */}
          {hasDetail && (
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              className={cn(
                "shrink-0 transition-transform text-orbflow-text-ghost",
                isExpanded && "rotate-180"
              )}
            >
              <path
                d="M2 3.5 L5 6.5 L8 3.5"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          )}
        </button>

        {/* Expanded detail */}
        {isExpanded && nodeState && (
          <div className="mt-1 ml-3 mr-1 space-y-2.5 pb-2 animate-exec-detail-slide">
            {/* Error */}
            {nodeState.error && (
              <div className="rounded-lg border border-orbflow-border/60 p-3 bg-orbflow-surface/30">
                <div className="flex items-center gap-1.5 mb-1.5">
                  <NodeIcon
                    name="x"
                    className="w-3 h-3"
                    style={{ color: STATUS_THEMES.failed.accent, opacity: 0.8 }}
                  />
                  <span
                    className="text-[11px] uppercase tracking-[0.16em] font-semibold"
                    style={{ color: STATUS_THEMES.failed.text }}
                  >
                    Error
                  </span>
                </div>
                <p
                  className="text-sm font-mono leading-relaxed"
                  style={{ color: STATUS_THEMES.failed.text, opacity: 0.8 }}
                >
                  {nodeState.error}
                </p>
              </div>
            )}

            {/* Output */}
            {nodeState.output && (
              <div>
                <span className="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.16em] text-orbflow-text-ghost">
                  Output
                </span>
                <div className="rounded-lg p-3 bg-orbflow-surface/20 border border-orbflow-border/40 max-h-64 overflow-y-auto custom-scrollbar">
                  <StructuredOutput data={nodeState.output} pluginRef={node.plugin_ref} />
                </div>
              </div>
            )}

            {/* Input */}
            {nodeState.input && (
              <div>
                <span className="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.16em] text-orbflow-text-ghost">
                  Input
                </span>
                <div className="rounded-lg p-3 bg-orbflow-surface/20 border border-orbflow-border/40 max-h-48 overflow-y-auto custom-scrollbar">
                  <StructuredOutput data={nodeState.input} pluginRef={node.plugin_ref} />
                </div>
              </div>
            )}

            {/* Attempt */}
            {nodeState.attempt > 1 && (
              <p className="text-xs text-orbflow-text-ghost">
                Attempt {nodeState.attempt}
              </p>
            )}

            {/* View Details */}
            {onViewDetails && (
              <button
                onClick={onViewDetails}
                className="mt-1 flex items-center gap-1.5 rounded text-xs font-medium text-electric-indigo transition-colors hover:text-electric-indigo/80
                  focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
              >
                <NodeIcon name="eye" className="w-3 h-3" />
                View Details
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
});

/* -- Main execution graph (vertical timeline) ---- */

interface ExecutionGraphProps {
  workflow: Workflow;
  nodeStates: Record<string, NodeState>;
  onNodeClick?: (nodeId: string) => void;
}

export function ExecutionGraph({
  workflow,
  nodeStates,
  onNodeClick,
}: ExecutionGraphProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const toggleExpanded = useCallback((nodeId: string) => {
    setExpandedId((prev) => (prev === nodeId ? null : nodeId));
  }, []);

  const orderedNodes = useMemo(() => topoSortNodes(workflow), [workflow]);

  // Stats — single pass O(n)
  const stats = useMemo(() => {
    const counts = { completed: 0, failed: 0, running: 0, cancelled: 0, pending: 0 };
    for (const ns of Object.values(nodeStates)) {
      if (ns.status in counts) counts[ns.status as keyof typeof counts]++;
    }
    return { total: workflow.nodes.length, ...counts };
  }, [nodeStates, workflow.nodes.length]);

  const done = stats.completed + stats.failed + stats.cancelled;

  return (
    <div>
      {/* Compact stats bar */}
      <div className="flex items-center gap-4 mb-4">
        <div className="flex items-center gap-1.5">
          <span className="text-base font-semibold tabular-nums text-orbflow-text-secondary">
            {done}/{stats.total}
          </span>
          <span className="text-sm text-orbflow-text-ghost">steps</span>
        </div>

        {/* Segmented progress bar */}
        <div className="flex-1 h-1.5 rounded-full bg-orbflow-surface overflow-hidden max-w-[220px]">
          {stats.total > 0 && (
            <div className="flex h-full">
              {stats.completed > 0 && (
                <div
                  className="h-full transition-all duration-500"
                  style={{
                    width: `${(stats.completed / stats.total) * 100}%`,
                    backgroundColor: STATUS_THEMES.completed.accent,
                    opacity: 0.5,
                  }}
                />
              )}
              {stats.failed > 0 && (
                <div
                  className="h-full transition-all duration-500"
                  style={{
                    width: `${(stats.failed / stats.total) * 100}%`,
                    backgroundColor: STATUS_THEMES.failed.accent,
                    opacity: 0.5,
                  }}
                />
              )}
              {stats.running > 0 && (
                <div
                  className="h-full transition-all duration-500"
                  style={{
                    width: `${(stats.running / stats.total) * 100}%`,
                    backgroundColor: STATUS_THEMES.running.accent,
                    opacity: 0.5,
                  }}
                />
              )}
            </div>
          )}
        </div>

        {/* Inline status counters */}
        <div className="flex items-center gap-3">
          {stats.failed > 0 && (
            <span
              className="flex items-center gap-1.5 text-xs"
              style={{ color: STATUS_THEMES.failed.text }}
            >
              <div
                className="w-1.5 h-1.5 rounded-full"
                style={{ backgroundColor: STATUS_THEMES.failed.accent, opacity: 0.7 }}
              />
              {stats.failed} failed
            </span>
          )}
          {stats.running > 0 && (
            <span
              className="flex items-center gap-1.5 text-xs"
              style={{ color: STATUS_THEMES.running.text }}
            >
              <div
                className="w-1.5 h-1.5 rounded-full animate-exec-step-pulse"
                style={{ backgroundColor: STATUS_THEMES.running.accent }}
              />
              {stats.running} running
            </span>
          )}
        </div>
      </div>

      {/* Step timeline */}
      <div className="py-1">
        {orderedNodes.map((node, i) => (
          <StepItem
            key={node.id}
            node={node}
            nodeState={nodeStates[node.id]}
            isLast={i === orderedNodes.length - 1}
            isExpanded={expandedId === node.id}
            onToggle={() => toggleExpanded(node.id)}
            onViewDetails={onNodeClick ? () => onNodeClick(node.id) : undefined}
          />
        ))}

        {orderedNodes.length === 0 && (
          <p className="text-body text-orbflow-text-ghost italic py-4">
            No steps in this workflow
          </p>
        )}
      </div>
    </div>
  );
}
