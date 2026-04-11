"use client";

import { useState, useCallback, useMemo } from "react";
import { useWorkflowStore } from "@/store/workflow-store";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import type {
  AlertRule,
  AlertMetric,
  AlertOperator,
  AlertChannel,
  CreateAlertInput,
} from "@orbflow/core";

/* =======================================================
   Constants
   ======================================================= */

const METRICS: { value: AlertMetric; label: string; unit: string; icon: string; hint: string }[] = [
  { value: "failure_rate", label: "Failure Rate", unit: "%", icon: "alert-triangle", hint: "% of failed runs" },
  { value: "p95_duration", label: "P95 Duration", unit: "ms", icon: "clock", hint: "95th pctl latency" },
  { value: "execution_count", label: "Exec Count", unit: "", icon: "zap", hint: "Runs in period" },
];

const OPERATORS: { value: AlertOperator; symbol: string; label: string }[] = [
  { value: "greater_than", symbol: ">", label: "greater than" },
  { value: "less_than", symbol: "<", label: "less than" },
  { value: "equals", symbol: "=", label: "equals" },
];

const METRIC_COLORS: Record<AlertMetric, string> = {
  failure_rate: "border-rose-500/30 bg-rose-500/[0.06]",
  p95_duration: "border-amber-500/30 bg-amber-500/[0.06]",
  execution_count: "border-sky-500/30 bg-sky-500/[0.06]",
};

const METRIC_TEXT: Record<AlertMetric, string> = {
  failure_rate: "text-rose-400",
  p95_duration: "text-amber-400",
  execution_count: "text-sky-400",
};

const METRIC_GLOW: Record<AlertMetric, string> = {
  failure_rate: "from-rose-500/8 to-transparent",
  p95_duration: "from-amber-500/8 to-transparent",
  execution_count: "from-sky-500/8 to-transparent",
};

/* =======================================================
   Component
   ======================================================= */

interface AlertFormProps {
  readonly editingAlert: AlertRule | null;
  readonly onSave: (input: CreateAlertInput) => Promise<void>;
  readonly onCancel: () => void;
}

export function AlertForm({ editingAlert, onSave, onCancel }: AlertFormProps) {
  const workflows = useWorkflowStore((s) => s.workflows);

  const [metric, setMetric] = useState<AlertMetric>(editingAlert?.metric ?? "failure_rate");
  const [operator, setOperator] = useState<AlertOperator>(editingAlert?.operator ?? "greater_than");
  const [threshold, setThreshold] = useState<string>(editingAlert?.threshold?.toString() ?? "");
  const [channelType, setChannelType] = useState<"webhook" | "log">((editingAlert?.channel.type as "webhook" | "log") ?? "webhook");
  const [webhookUrl, setWebhookUrl] = useState<string>(
    editingAlert?.channel.type === "webhook" ? editingAlert.channel.url ?? "" : ""
  );
  const [workflowId, setWorkflowId] = useState<string>(editingAlert?.workflow_id ?? "");
  const [enabled, setEnabled] = useState<boolean>(editingAlert?.enabled ?? true);
  const [saving, setSaving] = useState(false);

  const selectedMetric = METRICS.find((m) => m.value === metric);
  const selectedOperator = OPERATORS.find((o) => o.value === operator);
  const selectedWorkflow = workflows.find((w) => w.id === workflowId);

  const isValid = useMemo(() => {
    if (!threshold || Number(threshold) < 0) return false;
    if (channelType === "webhook" && !webhookUrl.trim()) return false;
    return true;
  }, [threshold, channelType, webhookUrl]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!isValid) return;
      setSaving(true);

      const channel: AlertChannel =
        channelType === "webhook"
          ? { type: "webhook", url: webhookUrl }
          : { type: "log" };

      const input: CreateAlertInput = {
        metric,
        operator,
        threshold: Number(threshold),
        channel,
        enabled,
        ...(workflowId ? { workflow_id: workflowId } : {}),
      };

      try {
        await onSave(input);
      } finally {
        setSaving(false);
      }
    },
    [metric, operator, threshold, channelType, webhookUrl, workflowId, enabled, onSave, isValid]
  );

  const inputCls =
    "w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3 py-2 text-sm text-orbflow-text-secondary outline-none focus:border-electric-indigo/60 focus:ring-1 focus:ring-electric-indigo/30 transition-colors";

  /* --- Readiness checklist for preview --- */
  const steps = [
    { label: "Metric selected", done: true },
    { label: "Threshold set", done: !!threshold && Number(threshold) >= 0 },
    { label: "Channel configured", done: channelType === "log" || (channelType === "webhook" && !!webhookUrl.trim()) },
  ];
  const readyCount = steps.filter((s) => s.done).length;

  return (
    <div className="flex items-stretch">
      {/* --- Left: Form fields --- */}
      <form onSubmit={handleSubmit} className="flex-1 min-w-0 space-y-5 pr-5">
        <h3 className="text-sm font-semibold text-orbflow-text-secondary">
          {editingAlert ? "Edit Alert Rule" : "New Alert Rule"}
        </h3>

        {/* Row 1: Metric selector */}
        <div className="space-y-1.5">
          <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost">
            1. Metric
          </label>
          <div className="grid grid-cols-3 gap-2">
            {METRICS.map((m) => (
              <button
                key={m.value}
                type="button"
                onClick={() => setMetric(m.value)}
                className={cn(
                  "flex items-center gap-2.5 rounded-xl border px-3 py-2.5 text-left transition-all",
                  metric === m.value
                    ? cn(METRIC_COLORS[m.value], "ring-1 ring-electric-indigo/20")
                    : "border-orbflow-border bg-orbflow-surface hover:border-orbflow-border-hover"
                )}
              >
                <NodeIcon
                  name={m.icon}
                  className={cn("w-3.5 h-3.5 shrink-0", metric === m.value ? METRIC_TEXT[m.value] : "text-orbflow-text-ghost")}
                />
                <div className="min-w-0">
                  <span className={cn(
                    "text-xs font-semibold block truncate",
                    metric === m.value ? METRIC_TEXT[m.value] : "text-orbflow-text-secondary"
                  )}>
                    {m.label}
                  </span>
                  <span className="text-[10px] text-orbflow-text-ghost truncate block">{m.hint}</span>
                </div>
              </button>
            ))}
          </div>
        </div>

        {/* Row 2: Condition + Channel side by side */}
        <div className="grid grid-cols-2 gap-4">
          {/* Condition */}
          <div className="space-y-1.5">
            <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost">
              2. Condition
            </label>
            <div className="flex items-center gap-1.5">
              <div className="flex rounded-lg border border-orbflow-border bg-orbflow-surface p-0.5 shrink-0">
                {OPERATORS.map((o) => (
                  <button
                    key={o.value}
                    type="button"
                    onClick={() => setOperator(o.value)}
                    className={cn(
                      "rounded-md w-8 h-8 flex items-center justify-center text-sm font-mono font-bold transition-colors",
                      operator === o.value
                        ? "bg-electric-indigo/20 text-electric-indigo"
                        : "text-orbflow-text-ghost hover:text-orbflow-text-secondary"
                    )}
                    title={o.label}
                  >
                    {o.symbol}
                  </button>
                ))}
              </div>
              <div className="relative flex-1">
                <input
                  type="number"
                  value={threshold}
                  onChange={(e) => setThreshold(e.target.value)}
                  placeholder="50"
                  required
                  min={0}
                  step="any"
                  className={cn(inputCls, "pr-10 font-mono tabular-nums")}
                />
                {selectedMetric?.unit && (
                  <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] text-orbflow-text-ghost font-mono">
                    {selectedMetric.unit}
                  </span>
                )}
              </div>
            </div>
          </div>

          {/* Channel */}
          <div className="space-y-1.5">
            <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost">
              3. Channel
            </label>
            <div className="flex gap-1.5">
              <button
                type="button"
                onClick={() => setChannelType("webhook")}
                className={cn(
                  "flex-1 flex items-center gap-2 rounded-lg border px-3 py-2 text-left transition-all",
                  channelType === "webhook"
                    ? "border-electric-indigo/30 bg-electric-indigo/[0.06] ring-1 ring-electric-indigo/15"
                    : "border-orbflow-border bg-orbflow-surface hover:border-orbflow-border-hover"
                )}
              >
                <NodeIcon name="webhook" className={cn("w-3.5 h-3.5 shrink-0", channelType === "webhook" ? "text-electric-indigo" : "text-orbflow-text-ghost")} />
                <span className={cn("text-xs font-semibold", channelType === "webhook" ? "text-electric-indigo" : "text-orbflow-text-secondary")}>
                  Webhook
                </span>
              </button>
              <button
                type="button"
                onClick={() => setChannelType("log")}
                className={cn(
                  "flex-1 flex items-center gap-2 rounded-lg border px-3 py-2 text-left transition-all",
                  channelType === "log"
                    ? "border-electric-indigo/30 bg-electric-indigo/[0.06] ring-1 ring-electric-indigo/15"
                    : "border-orbflow-border bg-orbflow-surface hover:border-orbflow-border-hover"
                )}
              >
                <NodeIcon name="terminal" className={cn("w-3.5 h-3.5 shrink-0", channelType === "log" ? "text-electric-indigo" : "text-orbflow-text-ghost")} />
                <span className={cn("text-xs font-semibold", channelType === "log" ? "text-electric-indigo" : "text-orbflow-text-secondary")}>
                  Log
                </span>
              </button>
            </div>
          </div>
        </div>

        {/* Webhook URL */}
        {channelType === "webhook" && (
          <div className="space-y-1.5 -mt-2">
            <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost">
              Webhook URL
            </label>
            <input
              type="url"
              value={webhookUrl}
              onChange={(e) => setWebhookUrl(e.target.value)}
              placeholder="https://hooks.example.com/alerts"
              required
              className={cn(inputCls, "font-mono text-xs")}
            />
          </div>
        )}

        {/* Row 3: Scope + Status */}
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost">
              4. Scope
            </label>
            <select
              value={workflowId}
              onChange={(e) => setWorkflowId(e.target.value)}
              className={cn(inputCls, "appearance-none cursor-pointer")}
            >
              <option value="">All workflows</option>
              {workflows.map((wf) => (
                <option key={wf.id} value={wf.id}>{wf.name}</option>
              ))}
            </select>
          </div>
          <div className="space-y-1.5">
            <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost">
              Status
            </label>
            <label className="flex items-center gap-3 cursor-pointer bg-orbflow-surface border border-orbflow-border rounded-lg px-3 py-2">
              <button
                type="button"
                role="switch"
                aria-checked={enabled}
                onClick={() => setEnabled(!enabled)}
                className={cn(
                  "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors",
                  enabled ? "bg-emerald-500" : "bg-orbflow-border"
                )}
              >
                <span className={cn(
                  "inline-block h-3.5 w-3.5 rounded-full bg-white shadow-sm transition-transform",
                  enabled ? "translate-x-[18px]" : "translate-x-[3px]"
                )} />
              </button>
              <span className="text-sm text-orbflow-text-muted">{enabled ? "Enabled" : "Disabled"}</span>
            </label>
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-3 pt-3 border-t border-orbflow-border/40">
          <button
            type="submit"
            disabled={saving || !isValid}
            className="flex items-center gap-2 rounded-lg bg-electric-indigo px-4 py-2 text-sm font-semibold text-white transition-all hover:bg-electric-indigo/85 disabled:opacity-40 disabled:cursor-not-allowed shadow-sm shadow-electric-indigo/20"
          >
            {saving ? (
              <>
                <div className="w-3.5 h-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white" />
                Saving...
              </>
            ) : (
              <>
                <NodeIcon name={editingAlert ? "check" : "plus"} className="w-3.5 h-3.5" />
                {editingAlert ? "Update Alert" : "Create Alert"}
              </>
            )}
          </button>
          <button
            type="button"
            onClick={onCancel}
            className="rounded-lg px-4 py-2 text-sm font-medium text-orbflow-text-ghost hover:text-orbflow-text-secondary transition-colors"
          >
            Cancel
          </button>
        </div>
      </form>

      {/* --- Right: Live preview -- integrated sidebar --- */}
      <div className="w-64 shrink-0 border-l border-orbflow-border/40 bg-orbflow-bg/60 overflow-hidden -my-5 -mr-5 pl-0">
        {/* Preview header with gradient accent */}
        <div className={cn(
          "px-4 py-3 border-b border-orbflow-border/30 bg-gradient-to-r",
          METRIC_GLOW[metric]
        )}>
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-orbflow-text-ghost">
              Preview
            </span>
            <span className={cn(
              "inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium",
              enabled ? "bg-emerald-500/10 text-emerald-400" : "bg-orbflow-surface-hover text-orbflow-text-ghost"
            )}>
              <span className={cn("w-1.5 h-1.5 rounded-full", enabled ? "bg-emerald-500" : "bg-orbflow-text-ghost")} />
              {enabled ? "Active" : "Paused"}
            </span>
          </div>
        </div>

        <div className="p-4 space-y-4">
          {/* Condition sentence */}
          <div className="space-y-1.5">
            <p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost">
              Trigger when
            </p>
            <div className="flex items-baseline gap-1.5 flex-wrap">
              <span className={cn("text-sm font-bold", METRIC_TEXT[metric])}>
                {selectedMetric?.label}
              </span>
              <span className="text-xs text-orbflow-text-ghost">is</span>
              <span className="text-sm font-semibold text-orbflow-text-secondary">
                {selectedOperator?.label ?? "..."}
              </span>
              {threshold ? (
                <span className="text-sm font-bold font-mono text-orbflow-text-secondary tabular-nums">
                  {threshold}{selectedMetric?.unit}
                </span>
              ) : (
                <span className="text-sm text-orbflow-text-ghost/50 italic">threshold</span>
              )}
            </div>
          </div>

          {/* Details */}
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-xs">
              <NodeIcon
                name={channelType === "webhook" ? "webhook" : "terminal"}
                className="w-3 h-3 text-orbflow-text-ghost shrink-0"
              />
              <span className="text-orbflow-text-muted">
                {channelType === "webhook" ? "Webhook" : "Server log"}
              </span>
            </div>
            {channelType === "webhook" && webhookUrl && (
              <p className="text-[10px] font-mono text-orbflow-text-ghost truncate pl-5" title={webhookUrl}>
                {webhookUrl}
              </p>
            )}
            <div className="flex items-center gap-2 text-xs">
              <NodeIcon name="workflow" className="w-3 h-3 text-orbflow-text-ghost shrink-0" />
              <span className="text-orbflow-text-muted">
                {selectedWorkflow?.name ?? "All workflows"}
              </span>
            </div>
          </div>

          {/* Readiness */}
          <div className="border-t border-orbflow-border/40 pt-3 space-y-1.5">
            <p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost">
              Readiness ({readyCount}/{steps.length})
            </p>
            {steps.map((s) => (
              <div key={s.label} className="flex items-center gap-2 text-xs">
                <div className={cn(
                  "w-3.5 h-3.5 rounded-full flex items-center justify-center shrink-0",
                  s.done ? "bg-emerald-500/15" : "bg-orbflow-surface-hover"
                )}>
                  {s.done ? (
                    <NodeIcon name="check" className="w-2 h-2 text-emerald-400" />
                  ) : (
                    <div className="w-1 h-1 rounded-full bg-orbflow-text-ghost/40" />
                  )}
                </div>
                <span className={cn(
                  s.done ? "text-orbflow-text-muted" : "text-orbflow-text-ghost/60"
                )}>
                  {s.label}
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
