"use client";

import { cn } from "@/lib/cn";
import { isSafeUrl } from "@/lib/output-safety";
import { isUrl } from "./data-shape-utils";
import { CopyBtn } from "./copy-button";

/* -- Small renderers -------------------------------- */

export function EmptyPlaceholder() {
  return <span className="text-body italic text-orbflow-text-ghost">No data</span>;
}

export function InlineBadge({ value }: { value: number | boolean }) {
  if (typeof value === "boolean") {
    return (
      <span className={cn("inline-flex items-center px-1.5 py-0.5 rounded text-body-sm font-medium",
        value ? "bg-emerald-500/10 text-emerald-400" : "bg-rose-500/10 text-rose-400")}>
        {String(value)}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center px-2 py-0.5 rounded bg-orbflow-surface/40 text-body font-mono tabular-nums text-orbflow-text-secondary">
      {value}
    </span>
  );
}

export function TextBlock({ value }: { value: string }) {
  const isSafeLink = isUrl(value) && isSafeUrl(value);
  return (
    <div className="relative group">
      <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
        <CopyBtn text={value} />
      </div>
      {isSafeLink ? (
        <a href={value} target="_blank" rel="noopener noreferrer"
          className="block font-mono text-body text-neon-cyan/80 hover:text-neon-cyan underline break-all p-3 rounded-lg bg-orbflow-surface/30 border border-orbflow-border/40">
          {value}
        </a>
      ) : (
        <pre className="text-body font-mono text-orbflow-text-secondary whitespace-pre-wrap break-all p-3 rounded-lg bg-orbflow-surface/30 border border-orbflow-border/40 leading-relaxed">
          {value}
        </pre>
      )}
    </div>
  );
}

export function InlineList({ items }: { items: unknown[] }) {
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <span className="text-caption font-medium text-orbflow-text-ghost px-1.5 py-0.5 rounded bg-orbflow-surface/40 tabular-nums">
        {items.length} item{items.length !== 1 ? "s" : ""}
      </span>
      {items.map((item, i) => (
        <span key={i} className="inline-flex items-center px-1.5 py-0.5 rounded text-body-sm font-mono bg-orbflow-surface/30 text-orbflow-text-faint border border-orbflow-border/30">
          {String(item)}
        </span>
      ))}
    </div>
  );
}

export function PrimitiveValue({ value }: { value: unknown }) {
  if (value === null || value === undefined) {
    return <span className="text-orbflow-text-ghost italic text-body">null</span>;
  }
  if (typeof value === "boolean") return <InlineBadge value={value} />;
  if (typeof value === "number") {
    return <span className="text-body font-mono tabular-nums text-right text-orbflow-text-secondary">{value}</span>;
  }
  if (isUrl(value) && isSafeUrl(String(value))) {
    return (
      <a href={String(value)} target="_blank" rel="noopener noreferrer"
        className="text-body font-mono text-neon-cyan/80 hover:text-neon-cyan underline break-all" title={String(value)}>
        {String(value)}
      </a>
    );
  }
  const str = String(value);
  if (str.length > 200) {
    return (
      <span className="text-body text-orbflow-text-secondary break-words" title={str}>
        {str.slice(0, 200)}&hellip;
      </span>
    );
  }
  return (
    <span className="text-body text-orbflow-text-secondary break-words">
      {str}
    </span>
  );
}
