"use client";

import { useEffect, useRef, useId } from "react";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { Button } from "./button";
import { NodeIcon } from "./icons";

interface ConfirmDialogProps {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: "danger" | "default";
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  message,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  variant = "default",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const ref = useRef<HTMLDivElement>(null);
  const confirmRef = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const messageId = useId();

  useFocusTrap(ref);

  // Auto-focus confirm button so Enter triggers it naturally
  useEffect(() => {
    confirmRef.current?.focus();
  }, []);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onCancel]);

  const isDanger = variant === "danger";

  return (
    <div className="fixed inset-0 z-[90] flex items-center justify-center backdrop-blur-sm animate-fade-in bg-orbflow-backdrop">
      <div
        ref={ref}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={messageId}
        className="w-full max-w-sm rounded-2xl backdrop-blur-xl shadow-2xl animate-scale-in overflow-hidden border border-orbflow-border bg-orbflow-glass-bg"
      >
        <div className="px-6 py-5">
          <div className="flex items-center gap-3 mb-3">
            <div className={`w-9 h-9 rounded-xl flex items-center justify-center shrink-0 ${
              isDanger ? "bg-rose-500/10" : "bg-electric-indigo/10"
            }`}>
              <NodeIcon
                name={isDanger ? "alert-triangle" : "help-circle"}
                className={`w-4 h-4 ${isDanger ? "text-rose-400" : "text-electric-indigo"}`}
              />
            </div>
            <h2 id={titleId} className="text-sm font-semibold text-orbflow-text-secondary">{title}</h2>
          </div>
          <p id={messageId} className="text-body-lg leading-relaxed ml-12 text-orbflow-text-muted break-words">{message}</p>
        </div>

        <div className="flex items-center justify-end gap-2 px-6 py-3.5 border-t border-orbflow-border">
          <Button variant="ghost" onClick={onCancel}>
            {cancelLabel}
          </Button>
          <Button
            ref={confirmRef}
            variant={isDanger ? "danger" : "primary"}
            onClick={onConfirm}
          >
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
