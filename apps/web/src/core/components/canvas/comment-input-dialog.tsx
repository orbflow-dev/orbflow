"use client";

import { useState, useEffect, useRef, useId } from "react";
import { Button } from "../button";
import { NodeIcon } from "../icons";

interface CommentInputDialogProps {
  initialValue: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

export function CommentInputDialog({
  initialValue,
  onSubmit,
  onCancel,
}: CommentInputDialogProps) {
  const [value, setValueState] = useState(initialValue);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const titleId = useId();

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onCancel]);

  return (
    <div className="fixed inset-0 z-[90] flex items-center justify-center bg-black/50 backdrop-blur-sm animate-fade-in">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="w-full max-w-sm rounded-2xl backdrop-blur-xl shadow-2xl animate-scale-in overflow-hidden border border-orbflow-border bg-orbflow-glass-bg"
      >
        <div className="px-6 py-5">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-9 h-9 rounded-xl flex items-center justify-center shrink-0 bg-amber-500/10">
              <NodeIcon name="message-square" className="w-4 h-4 text-amber-400" />
            </div>
            <h2 id={titleId} className="text-sm font-semibold text-orbflow-text-secondary">
              {initialValue ? "Edit Note" : "Add Note"}
            </h2>
          </div>
          <textarea
            ref={inputRef}
            value={value}
            onChange={(e) => setValueState(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                onSubmit(value);
              }
            }}
            placeholder="Add a note to this step..."
            rows={3}
            className="w-full rounded-lg px-3 py-2
              text-body-lg resize-none
              focus:outline-none focus:border-electric-indigo/30
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50
              transition-all border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary
              placeholder:text-orbflow-text-ghost"
          />
          <p className="text-caption mt-1.5 text-orbflow-text-faint">
            Ctrl+Enter to save
          </p>
        </div>
        <div className="flex items-center justify-end gap-2 px-6 py-3.5 border-t border-orbflow-border">
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={() => onSubmit(value)}
            className="bg-amber-500/15 border border-amber-500/20 text-amber-400 hover:bg-amber-500/25 shadow-none"
          >
            Save Note
          </Button>
        </div>
      </div>
    </div>
  );
}
