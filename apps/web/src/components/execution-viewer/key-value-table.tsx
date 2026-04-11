"use client";

import { cn } from "@/lib/cn";
import { CopyBtn } from "./copy-button";
import { PrimitiveValue } from "./structured-output-primitives";

export function KeyValueTable({ data }: { data: Record<string, unknown> }) {
  return (
    <div className="rounded-lg border border-orbflow-border/40 overflow-hidden">
      {Object.entries(data).map(([key, value], i) => (
        <div
          key={key}
          className={cn("flex items-center gap-3 py-1.5 px-3 group transition-colors",
            i % 2 === 1 && "bg-orbflow-surface/20", "hover:bg-orbflow-surface-hover/40")}
        >
          <span className="shrink-0 w-[140px] text-body font-mono text-orbflow-text-ghost truncate" title={key}>{key}</span>
          <div className="flex-1 min-w-0 flex items-center"><PrimitiveValue value={value} /></div>
          <div className="opacity-0 group-hover:opacity-100 transition-opacity">
            <CopyBtn text={value === null || value === undefined ? "null" : String(value)} />
          </div>
        </div>
      ))}
    </div>
  );
}
