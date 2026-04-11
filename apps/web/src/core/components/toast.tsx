"use client";

import { useEffect, useState } from "react";
import { useToastStore, type Toast as ToastItem } from "@orbflow/core/stores";
import { NodeIcon } from "./icons";

const TOAST_CONFIG: Record<
  string,
  { icon: string; color: string; bg: string; border: string }
> = {
  success: {
    icon: "check",
    color: "text-emerald-400",
    bg: "bg-emerald-500/8",
    border: "border-emerald-500/15",
  },
  error: {
    icon: "x",
    color: "text-rose-400",
    bg: "bg-rose-500/8",
    border: "border-rose-500/15",
  },
  warning: {
    icon: "bell",
    color: "text-amber-400",
    bg: "bg-amber-500/8",
    border: "border-amber-500/15",
  },
  info: {
    icon: "help-circle",
    color: "text-blue-400",
    bg: "bg-blue-500/8",
    border: "border-blue-500/15",
  },
};

function ToastCard({ toast, index }: { toast: ToastItem; index: number }) {
  const { remove } = useToastStore();
  const [exiting, setExiting] = useState(false);
  const cfg = TOAST_CONFIG[toast.type] || TOAST_CONFIG.info;
  const isUrgent = toast.type === "error" || toast.type === "warning";

  useEffect(() => {
    const exitTime = (toast.duration || 4000) - 300;
    const timer = setTimeout(() => setExiting(true), exitTime);
    return () => clearTimeout(timer);
  }, [toast.duration]);

  return (
    <div
      role={isUrgent ? "alert" : "status"}
      aria-live={isUrgent ? "assertive" : "polite"}
      className={`flex items-start gap-3 px-4 py-3 rounded-xl border backdrop-blur-md shadow-xl
        transition-all duration-300
        ${cfg.bg} ${cfg.border}
        ${exiting ? "opacity-0 translate-x-4" : "opacity-100 translate-x-0"}`}
      style={{
        animation: `slideInRight 0.3s cubic-bezier(0.16, 1, 0.3, 1) ${index * 50}ms both`,
        minWidth: 280,
        maxWidth: 400,
      }}
    >
      <div className={`shrink-0 mt-0.5 ${cfg.color}`} aria-hidden="true">
        <NodeIcon name={cfg.icon} className="w-4 h-4" />
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-body-lg font-semibold text-orbflow-text-secondary leading-tight">
          {toast.title}
        </p>
        {toast.message && (
          <p className="text-body text-orbflow-text-faint mt-0.5 leading-relaxed">
            {toast.message}
          </p>
        )}
      </div>
      <button
        onClick={() => remove(toast.id)}
        className="shrink-0 mt-0.5 p-0.5 rounded text-orbflow-text-ghost hover:text-orbflow-text-muted
          hover:bg-orbflow-controls-btn-hover transition-all duration-150
          focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
        aria-label="Dismiss notification"
      >
        <NodeIcon name="x" className="w-3 h-3" />
      </button>
    </div>
  );
}

export function ToastContainer() {
  const { toasts } = useToastStore();

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-6 right-6 z-[100] flex flex-col-reverse gap-2 pointer-events-auto" aria-label="Notifications">
      {toasts.map((toast, i) => (
        <ToastCard key={toast.id} toast={toast} index={i} />
      ))}
    </div>
  );
}
