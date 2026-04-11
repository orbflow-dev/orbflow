"use client";

import { useState, useMemo } from "react";
import { cn } from "@/lib/cn";
import { NodeIcon } from "@/core/components/icons";
import { CopyBtn } from "./copy-button";
import { highlightJson } from "./json-renderer";
import { EmptyPlaceholder, PrimitiveValue } from "./structured-output-primitives";
import { KeyValueTable } from "./key-value-table";
import {
  extractContentType, isBinaryContentType, looksLikeBinary,
  tryParseJson, estimateByteSize, formatBytes,
  OUTPUT_SIZE_WARNING_BYTES,
} from "@/lib/output-safety";

/* -- DataTable --------------------------------------- */

export function DataTable({ data, maxRows }: { data: Record<string, unknown>[]; maxRows: number }) {
  const [showAll, setShowAll] = useState(false);
  if (data.length === 0) return <EmptyPlaceholder />;

  const allKeys = Array.from(new Set(data.flatMap((row) => Object.keys(row))));
  const columns = allKeys.slice(0, 8);
  const hiddenCount = allKeys.length - columns.length;
  const visibleRows = showAll ? data : data.slice(0, maxRows);

  return (
    <div className="rounded-lg border border-orbflow-border/40 overflow-hidden">
      <div className="overflow-x-auto">
        <div className="min-w-max">
          <div className="flex items-center bg-orbflow-surface/40 backdrop-blur-sm border-b border-orbflow-border/30 sticky top-0 z-10">
            <div className="shrink-0 w-10 px-2 py-1.5 text-caption font-medium text-orbflow-text-ghost text-center">#</div>
            {columns.map((col) => (
              <div key={col} className="shrink-0 w-[120px] px-2 py-1.5 text-caption font-mono font-medium text-orbflow-text-ghost truncate">{col}</div>
            ))}
            {hiddenCount > 0 && (
              <div className="shrink-0 px-2 py-1.5 text-caption text-orbflow-text-ghost italic">+{hiddenCount} more</div>
            )}
          </div>
          {visibleRows.map((row, i) => (
            <div key={i} className={cn("flex items-center transition-colors", i % 2 === 1 && "bg-orbflow-surface/20", "hover:bg-orbflow-surface-hover/40")}>
              <div className="shrink-0 w-10 px-2 py-1 text-caption font-mono tabular-nums text-orbflow-text-ghost text-center">{i + 1}</div>
              {columns.map((col) => {
                const val = row[col];
                const str = val === null || val === undefined ? "" : String(val);
                return (
                  <div key={col} className="shrink-0 w-[120px] px-2 py-1 text-body-sm text-orbflow-text-secondary truncate" title={str.length > 40 ? str : undefined}>
                    {str.length > 40 ? str.slice(0, 40) + "..." : str}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>
      {!showAll && data.length > maxRows && (
        <button
          onClick={() => setShowAll(true)}
          className="w-full py-1.5 text-caption font-medium text-orbflow-text-faint hover:text-orbflow-text-secondary bg-orbflow-surface/20 hover:bg-orbflow-surface/40 transition-colors border-t border-orbflow-border/30
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
        >
          Show all {data.length} rows ({data.length - maxRows} hidden)
        </button>
      )}
    </div>
  );
}

/* -- CollapsibleTree --------------------------------- */

export function CollapsibleTree({ data, depth, maxDepth }: { data: Record<string, unknown>; depth: number; maxDepth: number }) {
  return (
    <div style={{ paddingLeft: depth > 0 ? 16 : 0 }}>
      {Object.entries(data).map(([key, value]) => (
        <TreeRow key={key} label={key} value={value} depth={depth} maxDepth={maxDepth} />
      ))}
    </div>
  );
}

function TreeRow({ label, value, depth, maxDepth }: { label: string; value: unknown; depth: number; maxDepth: number }) {
  const [expanded, setExpanded] = useState(depth < 1);
  const isComplex = typeof value === "object" && value !== null;

  if (!isComplex) {
    return (
      <div className="flex items-center gap-2 py-1 px-1">
        <span className="text-body font-mono text-orbflow-text-ghost shrink-0">{label}:</span>
        <PrimitiveValue value={value} />
      </div>
    );
  }

  if (depth >= maxDepth) {
    return (
      <div className="py-1 px-1">
        <span className="text-body font-mono text-orbflow-text-ghost">{label}:</span>
        <pre className="mt-1 text-caption font-mono text-orbflow-text-faint bg-orbflow-surface/20 rounded p-2 overflow-x-auto">
          {JSON.stringify(value, null, 2)}
        </pre>
      </div>
    );
  }

  const isArray = Array.isArray(value);
  const summary = isArray ? `[${(value as unknown[]).length}]` : `{${Object.keys(value as object).length}}`;

  return (
    <div className="py-0.5">
      <button
        onClick={() => setExpanded((prev) => !prev)}
        className="flex items-center gap-1.5 py-1 px-1 w-full text-left rounded hover:bg-orbflow-surface-hover/40 transition-colors
          focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
      >
        <NodeIcon name={expanded ? "chevron-down" : "chevron-right"} className="w-3 h-3 text-orbflow-text-ghost shrink-0" />
        <span className="text-body font-mono text-orbflow-text-ghost">{label}</span>
        <span className="text-caption text-orbflow-text-ghost/60">{summary}</span>
      </button>
      {expanded && (
        <div className="pl-2">
          {isArray ? (
            <div>
              {(value as unknown[]).map((item, i) => (
                <TreeRow key={i} label={String(i)} value={item} depth={depth + 1} maxDepth={maxDepth} />
              ))}
            </div>
          ) : (
            <CollapsibleTree data={value as Record<string, unknown>} depth={depth + 1} maxDepth={maxDepth} />
          )}
        </div>
      )}
    </div>
  );
}

/* -- HttpResponseView -------------------------------- */

export function HttpResponseView({ data }: { data: Record<string, unknown> }) {
  const [headersExpanded, setHeadersExpanded] = useState(false);

  const statusCode = (data.statusCode ?? data.status_code ?? data.status ?? 0) as number;
  const body = data.body;
  const headers = data.headers as Record<string, unknown> | undefined;

  const statusColor =
    statusCode >= 500 ? "bg-rose-500/15 text-rose-400 border-rose-500/20"
    : statusCode >= 400 ? "bg-amber-500/15 text-amber-400 border-amber-500/20"
    : statusCode >= 300 ? "bg-blue-500/15 text-blue-400 border-blue-500/20"
    : "bg-emerald-500/15 text-emerald-400 border-emerald-500/20";

  // Analyze body content type and data
  const bodyAnalysis = useMemo(() => {
    const contentType = headers ? extractContentType(headers) : null;
    const isBinary = (contentType && isBinaryContentType(contentType))
      || (typeof body === "string" && looksLikeBinary(body));
    const sizeBytes = estimateByteSize(body);
    const isTooLarge = sizeBytes > OUTPUT_SIZE_WARNING_BYTES;

    let parsedBody: unknown = body;
    let bodyJson = "";
    if (!isBinary && typeof body === "string") {
      const parsed = tryParseJson(body);
      if (parsed !== null) parsedBody = parsed;
    }
    if (typeof parsedBody === "object" && parsedBody !== null) {
      bodyJson = JSON.stringify(parsedBody, null, 2);
    } else {
      bodyJson = typeof body === "string" ? body : JSON.stringify(body, null, 2) ?? "";
    }

    return { contentType, isBinary, sizeBytes, isTooLarge, parsedBody, bodyJson };
  }, [body, headers]);

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <span className={cn("inline-flex items-center px-2 py-1 rounded text-body font-mono font-bold border tabular-nums", statusColor)}>
          {statusCode}
        </span>
        <span className="text-body-sm text-orbflow-text-ghost">
          {statusCode >= 200 && statusCode < 300 ? "OK" : statusCode >= 400 ? "Error" : "Redirect"}
        </span>
        {bodyAnalysis.contentType && (
          <span className="text-caption font-mono text-orbflow-text-ghost/60 ml-auto truncate">
            {bodyAnalysis.contentType}
          </span>
        )}
      </div>

      {/* Size warning */}
      {bodyAnalysis.isTooLarge && (
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
          <NodeIcon name="alert-triangle" className="w-3.5 h-3.5 text-amber-400/70 shrink-0" />
          <p className="text-body-sm text-amber-400/60">
            Large response: {formatBytes(bodyAnalysis.sizeBytes)}. Rendering may be slow.
          </p>
        </div>
      )}

      {/* Body content */}
      {body !== undefined && body !== null && (
        bodyAnalysis.isBinary ? (
          <BinaryPlaceholder
            contentType={bodyAnalysis.contentType}
            sizeBytes={bodyAnalysis.sizeBytes}
          />
        ) : (
          <div className="relative">
            <div className="absolute top-2 right-2 z-10"><CopyBtn text={bodyAnalysis.bodyJson} /></div>
            <pre className="text-body-sm font-mono leading-relaxed p-3 pr-16 overflow-x-auto rounded-lg bg-orbflow-surface/30 border border-orbflow-border/40 max-h-[400px]">
              {highlightJson(bodyAnalysis.bodyJson)}
            </pre>
          </div>
        )
      )}

      {headers && typeof headers === "object" && Object.keys(headers).length > 0 && (
        <div>
          <button
            onClick={() => setHeadersExpanded((prev) => !prev)}
            className="flex items-center gap-1.5 text-body-sm font-medium text-orbflow-text-faint hover:text-orbflow-text-secondary transition-colors rounded
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          >
            <NodeIcon name={headersExpanded ? "chevron-down" : "chevron-right"} className="w-3 h-3" />
            Headers
            <span className="text-orbflow-text-ghost/60">({Object.keys(headers).length})</span>
          </button>
          {headersExpanded && (
            <div className="mt-2"><KeyValueTable data={headers as Record<string, unknown>} /></div>
          )}
        </div>
      )}
    </div>
  );
}

/* -- Binary data placeholder ------------------------ */

function BinaryPlaceholder({ contentType, sizeBytes }: { contentType: string | null; sizeBytes: number }) {
  return (
    <div className="flex flex-col items-center justify-center py-8 px-4 rounded-lg bg-orbflow-surface/30 border border-orbflow-border/40 text-center">
      <NodeIcon name="file" className="w-8 h-8 mb-3 text-orbflow-text-ghost" />
      <p className="text-body font-medium text-orbflow-text-faint">Binary Data</p>
      <p className="text-caption mt-1 text-orbflow-text-ghost">
        {contentType ?? "Unknown type"} &middot; {formatBytes(sizeBytes)}
      </p>
      <p className="text-caption mt-2 text-orbflow-text-ghost/60 max-w-[280px]">
        Binary content cannot be displayed inline. Use the Raw view to see the encoded data.
      </p>
    </div>
  );
}
