"use client";

import { useState, useCallback, useEffect, useRef } from "react";
import { cn } from "@/lib/cn";
import { copyToClipboard } from "@/lib/clipboard";
import { NodeIcon } from "@/core/components/icons";

export function CopyBtn({ text, className }: { text: string; className?: string }) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => { if (timerRef.current) clearTimeout(timerRef.current); }, []);

  const handleCopy = useCallback(async () => {
    await copyToClipboard(text);
    setCopied(true);
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => setCopied(false), 1500);
  }, [text]);

  return (
    <button
      onClick={handleCopy}
      aria-label={copied ? "Copied" : "Copy to clipboard"}
      className={cn(
        "flex items-center gap-1 px-1.5 py-0.5 rounded text-caption font-medium transition-all shrink-0",
        "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
        copied ? "bg-emerald-500/10 text-emerald-400" : "bg-orbflow-add-btn-bg text-orbflow-text-faint hover:text-orbflow-text-muted",
        className,
      )}
    >
      <NodeIcon name={copied ? "check" : "clipboard"} className="w-2.5 h-2.5" />
      {copied ? "Copied" : "Copy"}
    </button>
  );
}
