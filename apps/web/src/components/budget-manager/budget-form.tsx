"use client";

import { useState } from "react";
import { cn } from "@/lib/cn";
import type { BudgetPeriod, CreateBudgetInput } from "@orbflow/core";
import type { Workflow } from "@orbflow/core";

interface BudgetFormProps {
  workflows: Workflow[];
  initialData?: Partial<CreateBudgetInput>;
  editingId?: string | null;
  onSave: (budget: CreateBudgetInput) => Promise<void>;
  onCancel: () => void;
}

const PERIOD_OPTIONS: { value: BudgetPeriod; label: string }[] = [
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "monthly", label: "Monthly" },
];

export function BudgetForm({
  workflows,
  initialData,
  editingId,
  onSave,
  onCancel,
}: BudgetFormProps) {
  const [workflowId, setWorkflowId] = useState(initialData?.workflow_id ?? "");
  const [team, setTeam] = useState(initialData?.team ?? "");
  const [period, setPeriod] = useState<BudgetPeriod>(initialData?.period ?? "monthly");
  const [limitUsd, setLimitUsd] = useState(
    initialData?.limit_usd?.toString() ?? ""
  );
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = parseFloat(limitUsd);
    if (isNaN(parsed) || parsed <= 0) return;

    setSaving(true);
    try {
      await onSave({
        workflow_id: workflowId || undefined,
        team: team || undefined,
        period,
        limit_usd: parsed,
      });
    } finally {
      setSaving(false);
    }
  };

  const isValid = limitUsd && parseFloat(limitUsd) > 0;

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div>
        <h3 className="text-sm font-medium text-orbflow-text-secondary">
          {editingId ? "Edit Budget" : "New Budget"}
        </h3>
        <p className="text-caption text-orbflow-text-ghost mt-0.5">
          {editingId ? "Update spending limits and scope" : "Set spending limits per workflow or team"}
        </p>
      </div>

      {/* Workflow Selector */}
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-orbflow-text-ghost uppercase tracking-wider">
          Workflow
        </label>
        <select
          value={workflowId}
          onChange={(e) => setWorkflowId(e.target.value)}
          className={cn(
            "w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3 py-2",
            "text-sm text-orbflow-text-secondary",
            "focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50",
            "appearance-none cursor-pointer"
          )}
        >
          <option value="">All workflows (account-wide)</option>
          {workflows.map((wf) => (
            <option key={wf.id} value={wf.id}>
              {wf.name}
            </option>
          ))}
        </select>
      </div>

      {/* Team */}
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-orbflow-text-ghost uppercase tracking-wider">
          Team (optional)
        </label>
        <input
          type="text"
          value={team}
          onChange={(e) => setTeam(e.target.value)}
          placeholder="e.g. engineering, marketing"
          className={cn(
            "w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3 py-2",
            "text-sm text-orbflow-text-secondary placeholder:text-orbflow-text-ghost/50",
            "focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50"
          )}
        />
      </div>

      {/* Period Selector */}
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-orbflow-text-ghost uppercase tracking-wider">
          Period
        </label>
        <div className="flex gap-1 rounded-lg border border-orbflow-border bg-orbflow-surface p-0.5">
          {PERIOD_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setPeriod(opt.value)}
              className={cn(
                "flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                period === opt.value
                  ? "bg-electric-indigo/20 text-electric-indigo"
                  : "text-orbflow-text-ghost hover:text-orbflow-text-secondary"
              )}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </div>

      {/* Limit USD */}
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-orbflow-text-ghost uppercase tracking-wider">
          Limit (USD)
        </label>
        <div className="relative">
          <span className="absolute left-3 top-1/2 -translate-y-1/2 text-sm text-orbflow-text-ghost">
            $
          </span>
          <input
            type="number"
            step="0.01"
            min="0.01"
            value={limitUsd}
            onChange={(e) => setLimitUsd(e.target.value)}
            placeholder="0.00"
            className={cn(
              "w-full rounded-lg border border-orbflow-border bg-orbflow-surface pl-7 pr-3 py-2",
              "text-sm text-orbflow-text-secondary placeholder:text-orbflow-text-ghost/50",
              "focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50",
              "[appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
            )}
          />
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 pt-2">
        <button
          type="submit"
          disabled={!isValid || saving}
          className={cn(
            "flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-all",
            "bg-electric-indigo text-white hover:bg-electric-indigo/80 active:bg-electric-indigo/70",
            "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
            "disabled:opacity-40 disabled:cursor-not-allowed"
          )}
        >
          {saving && (
            <div className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
          )}
          {saving ? "Saving..." : editingId ? "Update Budget" : "Create Budget"}
        </button>
        <button
          type="button"
          onClick={onCancel}
          disabled={saving}
          className="rounded-lg px-4 py-2 text-sm font-medium text-orbflow-text-ghost hover:text-orbflow-text-secondary
            transition-colors focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none
            disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
