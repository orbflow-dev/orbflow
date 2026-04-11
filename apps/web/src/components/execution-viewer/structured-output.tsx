"use client";

import { useState, useMemo } from "react";
import { cn } from "@/lib/cn";
import { NodeIcon } from "@/core/components/icons";
import { type DataShape, detectShape, isHttpResponse } from "./data-shape-utils";
import { RawJson } from "./json-renderer";
import { EmptyPlaceholder, InlineBadge, TextBlock, InlineList } from "./structured-output-primitives";
import { KeyValueTable } from "./key-value-table";
import {
  DataTable, CollapsibleTree, HttpResponseView,
} from "./structured-output-views";
import { analyzeOutput, formatBytes } from "@/lib/output-safety";

/* -- Types ------------------------------------------- */

interface StructuredOutputProps {
  data: Record<string, unknown> | unknown;
  pluginRef?: string;
  maxDepth?: number;
  maxArrayRows?: number;
  /** Optional content-type from HTTP response headers for smarter rendering */
  contentType?: string | null;
}

/* -- Main Component ---------------------------------- */

function StructuredOutput({ data, pluginRef, maxDepth = 3, maxArrayRows = 20, contentType }: StructuredOutputProps) {
  const [rawMode, setRawMode] = useState(false);
  const isHttp = pluginRef === "builtin:http" && isHttpResponse(data);

  // Safety analysis: binary detection, size checks, JSON-in-string parsing
  const analysis = useMemo(() => analyzeOutput(data, contentType), [data, contentType]);

  // If string contains JSON, use the parsed version for structured rendering
  const effectiveData = analysis.parsedJson !== null ? analysis.parsedJson : data;
  const shape = detectShape(effectiveData);

  if (shape === "empty" && !isHttp && !analysis.isBinary) return <EmptyPlaceholder />;

  // Binary data: show placeholder instead of trying to render
  if (analysis.isBinary && !rawMode) {
    return (
      <div className="relative">
        <div className="flex justify-end mb-2">
          <button
            onClick={() => setRawMode(true)}
            aria-label="Switch to raw view"
            className="flex items-center gap-1 px-2 py-0.5 rounded text-caption font-medium transition-all
              bg-orbflow-add-btn-bg text-orbflow-text-faint hover:text-orbflow-text-muted border border-transparent
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          >
            <NodeIcon name="code" className="w-2.5 h-2.5" />
            Raw
          </button>
        </div>
        <div className="flex flex-col items-center justify-center py-8 px-4 rounded-lg bg-orbflow-surface/30 border border-orbflow-border/40 text-center">
          <NodeIcon name="file" className="w-8 h-8 mb-3 text-orbflow-text-ghost" />
          <p className="text-body font-medium text-orbflow-text-faint">Binary Data</p>
          <p className="text-caption mt-1 text-orbflow-text-ghost">
            {analysis.contentType ?? "Unknown type"} &middot; {analysis.sizeFormatted}
          </p>
          <p className="text-caption mt-2 text-orbflow-text-ghost/60 max-w-[280px]">
            Binary content cannot be displayed inline. Use the Raw view to see the encoded data.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="relative">
      {/* Size warning */}
      {analysis.isTooLarge && (
        <div className="flex items-center gap-2 px-3 py-2 mb-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
          <NodeIcon name="alert-triangle" className="w-3.5 h-3.5 text-amber-400/70 shrink-0" />
          <p className="text-body-sm text-amber-400/60">
            Large output: {analysis.sizeFormatted}. Rendering may be slow.
          </p>
        </div>
      )}

      {shape !== "empty" && (
        <div className="flex justify-end mb-2">
          <button
            onClick={() => setRawMode((prev) => !prev)}
            aria-label={rawMode ? "Switch to structured view" : "Switch to raw JSON"}
            aria-pressed={rawMode}
            className={cn(
              "flex items-center gap-1 px-2 py-0.5 rounded text-caption font-medium transition-all",
              "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
              rawMode
                ? "bg-electric-indigo/10 text-electric-indigo border border-electric-indigo/20"
                : "bg-orbflow-add-btn-bg text-orbflow-text-faint hover:text-orbflow-text-muted border border-transparent",
            )}
          >
            <NodeIcon name="code" className="w-2.5 h-2.5" />
            Raw
          </button>
        </div>
      )}

      {rawMode ? (
        <RawJson data={data} />
      ) : (
        <StructuredContent data={effectiveData} shape={shape} isHttp={isHttp} maxDepth={maxDepth} maxArrayRows={maxArrayRows} />
      )}
    </div>
  );
}

/* -- Content router ---------------------------------- */

function StructuredContent({
  data, shape, isHttp, maxDepth, maxArrayRows,
}: {
  data: unknown; shape: DataShape; isHttp: boolean; maxDepth: number; maxArrayRows: number;
}) {
  if (isHttp) return <HttpResponseView data={data as Record<string, unknown>} />;

  switch (shape) {
    case "flat-object":
      return <KeyValueTable data={data as Record<string, unknown>} />;
    case "array-objects":
      return <DataTable data={data as Record<string, unknown>[]} maxRows={maxArrayRows} />;
    case "nested-object":
      return <CollapsibleTree data={data as Record<string, unknown>} depth={0} maxDepth={maxDepth} />;
    case "array-primitives":
      return <InlineList items={data as unknown[]} />;
    case "primitive-string":
      return <TextBlock value={data as string} />;
    case "primitive-number":
      return <InlineBadge value={data as number} />;
    case "primitive-boolean":
      return <InlineBadge value={data as boolean} />;
    case "empty":
    default:
      return <EmptyPlaceholder />;
  }
}

/* -- Exports ----------------------------------------- */

export { StructuredOutput };
