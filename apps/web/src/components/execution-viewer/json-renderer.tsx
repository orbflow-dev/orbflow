"use client";

import { type ReactNode } from "react";
import { CopyBtn } from "./copy-button";

export function highlightJson(json: string): ReactNode[] {
  const parts: ReactNode[] = [];
  const regex = /("(?:\\.|[^"\\])*"\s*:)|("(?:\\.|[^"\\])*")|([-+]?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|(true|false|null)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(json)) !== null) {
    if (match.index > lastIndex) parts.push(json.slice(lastIndex, match.index));
    if (match[1]) parts.push(<span key={match.index} className="text-neon-cyan/70">{match[1]}</span>);
    else if (match[2]) parts.push(<span key={match.index} className="text-orbflow-text-faint">{match[2]}</span>);
    else if (match[3]) parts.push(<span key={match.index} className="text-amber-400/80">{match[3]}</span>);
    else if (match[4]) parts.push(<span key={match.index} className="text-purple-400/80">{match[4]}</span>);
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < json.length) parts.push(json.slice(lastIndex));
  return parts;
}

export function RawJson({ data }: { data: unknown }) {
  const json = JSON.stringify(data, null, 2) ?? "";
  return (
    <div className="relative">
      <div className="absolute top-2 right-2 z-10"><CopyBtn text={json} /></div>
      <pre className="text-body-sm font-mono leading-relaxed p-3 pr-16 overflow-x-auto rounded-lg bg-orbflow-surface/30 border border-orbflow-border/40">
        {highlightJson(json)}
      </pre>
    </div>
  );
}
