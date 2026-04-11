"use client";

import { useEffect, useState, useCallback, useMemo } from "react";
import { useBudgetStore } from "@/store/budget-store";
import { useWorkflowStore } from "@/store/workflow-store";
import { ConfirmDialog } from "@/core/components/confirm-dialog";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import type { AccountBudget, CreateBudgetInput } from "@orbflow/core";
import { BudgetForm } from "./budget-form";

/* --- Helpers -------------------------------------------- */

function formatCurrency(usd: number): string {
  if (usd >= 1000) return `$${(usd / 1000).toFixed(1)}k`;
  return `$${usd.toFixed(2)}`;
}

function usagePercent(current: number, limit: number): number {
  if (limit <= 0) return 0;
  return (current / limit) * 100;
}

function usageVariant(pct: number): "success" | "warning" | "danger" {
  if (pct > 100) return "danger";
  if (pct >= 80) return "warning";
  return "success";
}

function daysUntilReset(resetAt: string): number {
  const now = new Date();
  const reset = new Date(resetAt);
  const diff = reset.getTime() - now.getTime();
  return Math.max(0, Math.ceil(diff / (1000 * 60 * 60 * 24)));
}

function forecastOverage(current: number, limit: number, resetAt: string): { willExceed: boolean; projected: number } {
  const now = new Date();
  const reset = new Date(resetAt);
  const totalMs = reset.getTime() - (reset.getTime() - periodMs(resetAt));
  const elapsedMs = now.getTime() - (reset.getTime() - periodMs(resetAt));
  const fraction = Math.max(0.01, elapsedMs / totalMs);
  const projected = current / fraction;
  return { willExceed: projected > limit, projected };
}

function periodMs(resetAt: string): number {
  // Rough estimate for period length (30 days fallback)
  return 30 * 24 * 60 * 60 * 1000;
}

/* --- GaugeRing ----------------------------------------- */

function GaugeRing({
  percent,
  size = 80,
  strokeWidth = 5,
  variant,
}: {
  percent: number;
  size?: number;
  strokeWidth?: number;
  variant: "success" | "warning" | "danger";
}) {
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const filled = Math.min(percent, 100);
  const offset = circumference - (filled / 100) * circumference;

  const colors = {
    success: { stroke: "url(#gauge-success)", glow: "rgba(16, 185, 129, 0.25)" },
    warning: { stroke: "url(#gauge-warning)", glow: "rgba(245, 158, 11, 0.25)" },
    danger: { stroke: "url(#gauge-danger)", glow: "rgba(239, 68, 68, 0.35)" },
  };

  return (
    <svg width={size} height={size} className="transform -rotate-90">
      <defs>
        <linearGradient id="gauge-success" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#10B981" />
          <stop offset="100%" stopColor="#34D399" />
        </linearGradient>
        <linearGradient id="gauge-warning" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#F59E0B" />
          <stop offset="100%" stopColor="#FBBF24" />
        </linearGradient>
        <linearGradient id="gauge-danger" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#EF4444" />
          <stop offset="100%" stopColor="#F87171" />
        </linearGradient>
      </defs>
      {/* Track ring -- subtle tick marks effect */}
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke="currentColor"
        strokeWidth={strokeWidth}
        strokeDasharray="2 4"
        className="text-orbflow-border/40"
      />
      {/* Background ring */}
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke="currentColor"
        strokeWidth={strokeWidth - 1}
        className="text-orbflow-border/20"
      />
      {/* Value arc */}
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke={colors[variant].stroke}
        strokeWidth={strokeWidth}
        strokeDasharray={circumference}
        strokeDashoffset={offset}
        strokeLinecap="round"
        className="transition-all duration-1000 ease-out"
        style={{ filter: `drop-shadow(0 0 6px ${colors[variant].glow})` }}
      />
    </svg>
  );
}

/* --- SpendGaugeCard ------------------------------------ */

function SpendGaugeCard({
  label,
  current,
  limit,
  icon,
  period,
}: {
  label: string;
  current: number;
  limit: number;
  icon: string;
  period?: string;
}) {
  const pct = usagePercent(current, limit);
  const variant = usageVariant(pct);
  const remaining = Math.max(0, limit - current);

  return (
    <div className={cn(
      "relative rounded-2xl border p-5 transition-all duration-300 group overflow-hidden",
      variant === "danger"
        ? "border-red-500/25 bg-gradient-to-br from-red-500/[0.06] to-red-900/[0.02]"
        : variant === "warning"
          ? "border-amber-500/25 bg-gradient-to-br from-amber-500/[0.06] to-amber-900/[0.02]"
          : "border-orbflow-border bg-gradient-to-br from-orbflow-surface to-orbflow-bg"
    )}>
      {/* Decorative corner glow */}
      <div className={cn(
        "absolute -top-12 -right-12 w-32 h-32 rounded-full blur-3xl opacity-0 group-hover:opacity-100 transition-opacity duration-700",
        variant === "danger" ? "bg-red-500/10"
          : variant === "warning" ? "bg-amber-500/10"
            : "bg-emerald-500/8"
      )} />

      <div className="relative flex items-center gap-5">
        {/* Gauge */}
        <div className="relative flex items-center justify-center shrink-0">
          <GaugeRing percent={pct} variant={variant} />
          <div className="absolute inset-0 flex flex-col items-center justify-center">
            <span className={cn(
              "text-base font-black tabular-nums tracking-tight",
              variant === "danger" ? "text-red-400"
                : variant === "warning" ? "text-amber-400"
                  : "text-emerald-400"
            )}>
              {pct.toFixed(0)}
            </span>
            <span className={cn(
              "text-[8px] font-bold uppercase tracking-widest -mt-0.5",
              variant === "danger" ? "text-red-400/60"
                : variant === "warning" ? "text-amber-400/60"
                  : "text-emerald-400/60"
            )}>
              %
            </span>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5 mb-1.5">
            <NodeIcon name={icon} className="w-3.5 h-3.5 text-orbflow-text-ghost" />
            <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-orbflow-text-ghost">{label}</p>
            {period && (
              <span className="ml-auto text-[9px] font-medium text-orbflow-text-ghost/60 uppercase tracking-wider">
                {period}
              </span>
            )}
          </div>
          <p className="text-xl font-black text-orbflow-text-secondary tabular-nums tracking-tight">
            {formatCurrency(current)}
            <span className="text-xs font-medium text-orbflow-text-ghost/50 ml-1.5">
              of {formatCurrency(limit)}
            </span>
          </p>
          <div className="mt-2 flex items-center gap-3">
            <span className="text-[10px] text-orbflow-text-ghost tabular-nums">
              {formatCurrency(remaining)} remaining
            </span>
          </div>
        </div>
      </div>

      {/* Bottom progress strip */}
      <div className="mt-4 h-1 rounded-full bg-orbflow-border/30 overflow-hidden">
        <div
          className={cn(
            "h-full rounded-full transition-all duration-1000 ease-out",
            variant === "danger" ? "bg-gradient-to-r from-red-500 to-red-400"
              : variant === "warning" ? "bg-gradient-to-r from-amber-500 to-amber-400"
                : "bg-gradient-to-r from-emerald-500 to-emerald-400"
          )}
          style={{ width: `${Math.min(pct, 100)}%` }}
        />
      </div>
    </div>
  );
}

/* --- MetricCard ----------------------------------------- */

function MetricCard({
  label,
  value,
  subValue,
  icon,
  variant = "default",
}: {
  label: string;
  value: string | number;
  subValue?: string;
  icon?: string;
  variant?: "default" | "success" | "danger" | "warning";
}) {
  const config = {
    default: {
      border: "border-orbflow-border",
      bg: "bg-orbflow-surface",
      iconBg: "bg-orbflow-surface-hover",
      iconColor: "text-orbflow-text-ghost",
      valueColor: "text-orbflow-text-secondary",
    },
    success: {
      border: "border-emerald-500/20",
      bg: "bg-gradient-to-br from-emerald-500/[0.06] to-emerald-900/[0.02]",
      iconBg: "bg-emerald-500/10",
      iconColor: "text-emerald-400",
      valueColor: "text-emerald-400",
    },
    danger: {
      border: "border-red-500/20",
      bg: "bg-gradient-to-br from-red-500/[0.06] to-red-900/[0.02]",
      iconBg: "bg-red-500/10",
      iconColor: "text-red-400",
      valueColor: "text-red-400",
    },
    warning: {
      border: "border-amber-500/20",
      bg: "bg-gradient-to-br from-amber-500/[0.06] to-amber-900/[0.02]",
      iconBg: "bg-amber-500/10",
      iconColor: "text-amber-400",
      valueColor: "text-amber-400",
    },
  };

  const c = config[variant];

  return (
    <div className={cn(
      "relative rounded-2xl border p-4 transition-all duration-200 group overflow-hidden hover:border-orbflow-border-hover",
      c.border, c.bg
    )}>
      {/* Top row: icon + label */}
      <div className="flex items-center gap-2.5 mb-3">
        {icon && (
          <div className={cn("w-7 h-7 rounded-lg flex items-center justify-center", c.iconBg)}>
            <NodeIcon name={icon} className={cn("w-3.5 h-3.5", c.iconColor)} />
          </div>
        )}
        <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-orbflow-text-ghost">
          {label}
        </p>
      </div>

      {/* Value -- large and prominent */}
      <p className={cn("text-2xl font-black tabular-nums tracking-tight", c.valueColor)}>
        {value}
      </p>

      {/* Sub-value */}
      {subValue && (
        <p className="text-[10px] text-orbflow-text-ghost/60 mt-1.5 font-medium">{subValue}</p>
      )}

      {/* Decorative bottom accent line */}
      <div className={cn(
        "absolute bottom-0 left-0 right-0 h-[2px] opacity-0 group-hover:opacity-100 transition-opacity duration-300",
        variant === "danger" ? "bg-gradient-to-r from-transparent via-red-500/40 to-transparent"
          : variant === "warning" ? "bg-gradient-to-r from-transparent via-amber-500/40 to-transparent"
            : variant === "success" ? "bg-gradient-to-r from-transparent via-emerald-500/40 to-transparent"
              : "bg-gradient-to-r from-transparent via-electric-indigo/30 to-transparent"
      )} />
    </div>
  );
}

/* --- UsageBar ------------------------------------------- */

function UsageBar({ current, limit }: { current: number; limit: number }) {
  const pct = usagePercent(current, limit);
  const variant = usageVariant(pct);
  const barColors = {
    success: "bg-emerald-500",
    warning: "bg-amber-500",
    danger: "bg-red-500",
  };

  return (
    <div className="flex items-center gap-2 min-w-[120px]">
      <div className="flex-1 h-2 rounded-full bg-orbflow-border/50 overflow-hidden">
        <div
          className={cn("h-full rounded-full transition-all duration-500", barColors[variant])}
          style={{ width: `${Math.min(pct, 100)}%` }}
        />
      </div>
      <span
        className={cn(
          "text-xs font-mono tabular-nums w-12 text-right",
          variant === "danger"
            ? "text-red-400"
            : variant === "warning"
              ? "text-amber-400"
              : "text-orbflow-text-muted"
        )}
      >
        {pct.toFixed(0)}%
      </span>
    </div>
  );
}

/* --- StatusBadge ---------------------------------------- */

function StatusBadge({ current, limit, resetAt }: { current: number; limit: number; resetAt?: string }) {
  const pct = usagePercent(current, limit);

  if (pct > 100) {
    return (
      <span className="inline-flex items-center gap-1 rounded-md bg-red-500/10 px-1.5 py-0.5 text-xs font-medium text-red-400">
        <span className="w-1.5 h-1.5 rounded-full bg-red-400 animate-pulse-soft" />
        Over budget
      </span>
    );
  }
  if (pct >= 80) {
    const forecast = resetAt ? forecastOverage(current, limit, resetAt) : null;
    return (
      <span className="inline-flex items-center gap-1 rounded-md bg-amber-500/10 px-1.5 py-0.5 text-xs font-medium text-amber-400">
        <span className="w-1.5 h-1.5 rounded-full bg-amber-400" />
        {forecast?.willExceed ? "Projected over" : "Warning"}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 rounded-md bg-emerald-500/10 px-1.5 py-0.5 text-xs font-medium text-emerald-400">
      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
      On track
    </span>
  );
}

/* --- EmptyState ----------------------------------------- */

function EmptyState({ onCreateClick }: { onCreateClick: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-center animate-fade-in-up">
      {/* Layered decorative icon */}
      <div className="relative mb-6">
        <div className="absolute -inset-4 rounded-3xl bg-gradient-to-br from-electric-indigo/5 to-neon-cyan/5 blur-xl" />
        <div className="relative w-20 h-20 rounded-2xl bg-gradient-to-br from-electric-indigo/10 to-neon-cyan/10 border border-electric-indigo/15 flex items-center justify-center shadow-lg shadow-electric-indigo/5">
          <NodeIcon name="wallet" className="w-9 h-9 text-electric-indigo/50" />
          {/* Decorative badge */}
          <div className="absolute -top-1.5 -right-1.5 w-5 h-5 rounded-full bg-electric-indigo/20 border border-electric-indigo/30 flex items-center justify-center">
            <NodeIcon name="plus" className="w-2.5 h-2.5 text-electric-indigo" />
          </div>
        </div>
      </div>
      <h3 className="text-sm font-bold text-orbflow-text-secondary mb-2 tracking-tight">No budgets configured</h3>
      <p className="text-xs text-orbflow-text-ghost max-w-sm mb-6 leading-relaxed">
        Set spending limits per workflow or team. Get alerts when costs
        approach thresholds and prevent runaway execution costs.
      </p>
      <button
        onClick={onCreateClick}
        className="flex items-center gap-2 rounded-xl bg-electric-indigo px-5 py-2.5 text-sm font-semibold text-white
          hover:bg-electric-indigo/85 active:bg-electric-indigo/75 transition-all duration-200
          shadow-md shadow-electric-indigo/20 hover:shadow-lg hover:shadow-electric-indigo/30 hover:-translate-y-0.5
          focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
      >
        <NodeIcon name="plus" className="w-4 h-4" />
        Create First Budget
      </button>
    </div>
  );
}

/* --- CostOverview --------------------------------------- */

function CostOverview({
  budgets,
  costRange,
  onCostRangeChange,
}: {
  budgets: AccountBudget[];
  costRange: string;
  onCostRangeChange: (range: string) => void;
}) {
  const costAnalytics = useBudgetStore((s) => s.costAnalytics);
  const fetchCosts = useBudgetStore((s) => s.fetchCosts);
  const [costError, setCostError] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        await fetchCosts(costRange);
        if (!cancelled) setCostError(false);
      } catch {
        if (!cancelled) {
          setCostError(true);
          setTimeout(() => { if (!cancelled) load(); }, 5000);
        }
      }
    };
    load();
    return () => { cancelled = true; };
  }, [fetchCosts, costRange]);

  const overBudgetCount = budgets.filter(
    (b) => usagePercent(b.current_usd, b.limit_usd) > 100
  ).length;

  const warningCount = budgets.filter((b) => {
    const pct = usagePercent(b.current_usd, b.limit_usd);
    return pct >= 80 && pct <= 100;
  }).length;

  const totalSpend = costAnalytics?.total_cost_usd ?? 0;

  // Find highest-spend budget for gauge
  const topBudget = budgets.length > 0
    ? budgets.reduce((max, b) => usagePercent(b.current_usd, b.limit_usd) > usagePercent(max.current_usd, max.limit_usd) ? b : max, budgets[0])
    : null;

  const avgCostPerExecution = costAnalytics?.workflow_costs
    ? costAnalytics.workflow_costs.reduce((sum, wc) => sum + wc.avg_cost_per_execution, 0) / Math.max(costAnalytics.workflow_costs.length, 1)
    : 0;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-orbflow-text-secondary flex items-center gap-2.5">
          <div className="w-7 h-7 rounded-lg bg-neon-cyan/10 flex items-center justify-center">
            <NodeIcon name="bar-chart" className="w-3.5 h-3.5 text-neon-cyan" />
          </div>
          Cost Overview
        </h3>
        <div className="flex gap-0.5 rounded-xl border border-orbflow-border bg-orbflow-bg p-1">
          {(["7d", "30d", "90d"] as const).map((range) => (
            <button
              key={range}
              onClick={() => onCostRangeChange(range)}
              className={cn(
                "rounded-lg px-3 py-1.5 text-xs font-semibold transition-all duration-200",
                "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                costRange === range
                  ? "bg-electric-indigo/15 text-electric-indigo shadow-sm shadow-electric-indigo/10"
                  : "text-orbflow-text-ghost hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover/50"
              )}
            >
              {range}
            </button>
          ))}
        </div>
      </div>

      {/* Top gauge cards for most-used budgets */}
      {topBudget && budgets.length > 0 && (
        <div className={cn(
          "grid gap-3",
          budgets.length === 1 ? "grid-cols-1 max-w-md"
            : budgets.length === 2 ? "grid-cols-1 sm:grid-cols-2"
              : "grid-cols-1 sm:grid-cols-2 lg:grid-cols-3"
        )}>
          {budgets
            .sort((a, b) => usagePercent(b.current_usd, b.limit_usd) - usagePercent(a.current_usd, a.limit_usd))
            .slice(0, 3)
            .map((b) => (
              <SpendGaugeCard
                key={b.id}
                label={b.team ?? b.workflow_id ?? "Account"}
                current={b.current_usd}
                limit={b.limit_usd}
                icon={b.workflow_id ? "workflow" : "layers"}
                period={b.period}
              />
            ))}
        </div>
      )}

      {/* Summary metrics row */}
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <MetricCard
          label="Total Spend"
          value={formatCurrency(totalSpend)}
          subValue={`${costRange} period`}
          icon="dollar-sign"
        />
        <MetricCard
          label="Active Budgets"
          value={budgets.length}
          icon="layers"
        />
        <MetricCard
          label="Warnings"
          value={warningCount}
          icon="alert-triangle"
          variant={warningCount > 0 ? "warning" : "default"}
        />
        <MetricCard
          label="Over Budget"
          value={overBudgetCount}
          icon="alert-triangle"
          variant={overBudgetCount > 0 ? "danger" : "default"}
        />
      </div>

      {/* Cost by workflow breakdown */}
      {costAnalytics && costAnalytics.workflow_costs.length > 0 && (
        <div className="rounded-2xl border border-orbflow-border bg-orbflow-surface overflow-hidden">
          <div className="flex items-center justify-between px-5 py-3.5 border-b border-orbflow-border/50 bg-gradient-to-r from-orbflow-surface to-orbflow-bg">
            <h4 className="text-[10px] font-semibold uppercase tracking-[0.12em] text-orbflow-text-ghost flex items-center gap-2">
              <NodeIcon name="workflow" className="w-3 h-3 text-orbflow-text-ghost/60" />
              Cost by Workflow
            </h4>
            <span className="text-[10px] font-medium text-orbflow-text-ghost/60 tabular-nums">
              Avg {formatCurrency(avgCostPerExecution)}/exec
            </span>
          </div>
          <div className="p-5 space-y-3">
            {costAnalytics.workflow_costs
              .sort((a, b) => b.total_cost_usd - a.total_cost_usd)
              .map((wc, idx) => {
                const pct = totalSpend > 0 ? (wc.total_cost_usd / totalSpend) * 100 : 0;
                return (
                  <div key={wc.workflow_id} className="group flex items-center gap-3 py-0.5">
                    <span className={cn(
                      "w-5 h-5 rounded-md flex items-center justify-center text-[9px] font-bold tabular-nums",
                      idx === 0 ? "bg-electric-indigo/15 text-electric-indigo"
                        : "bg-orbflow-surface-hover text-orbflow-text-ghost"
                    )}>
                      {idx + 1}
                    </span>
                    <span className="w-36 truncate text-xs font-medium text-orbflow-text-secondary">
                      {wc.workflow_name}
                    </span>
                    <div className="flex-1 h-2 rounded-full bg-orbflow-border/30 overflow-hidden">
                      <div
                        className={cn(
                          "h-full rounded-full transition-all duration-700 ease-out",
                          idx === 0 ? "bg-gradient-to-r from-electric-indigo to-neon-cyan"
                            : "bg-gradient-to-r from-electric-indigo/60 to-neon-cyan/60"
                        )}
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                    <span className="w-16 text-right text-[10px] font-mono text-orbflow-text-ghost tabular-nums">
                      {pct.toFixed(1)}%
                    </span>
                    <span className="w-20 text-right text-xs font-mono text-orbflow-text-muted tabular-nums font-semibold">
                      {formatCurrency(wc.total_cost_usd)}
                    </span>
                    <span className="w-16 text-right text-[10px] text-orbflow-text-ghost tabular-nums">
                      {wc.execution_count} runs
                    </span>
                    <span className="w-20 text-right text-[10px] text-orbflow-text-ghost tabular-nums sm:opacity-0 sm:group-hover:opacity-100 sm:focus-within:opacity-100 transition-opacity">
                      {formatCurrency(wc.avg_cost_per_execution)}/ea
                    </span>
                  </div>
                );
              })}
          </div>
        </div>
      )}
    </div>
  );
}

/* --- BudgetTable ---------------------------------------- */

function BudgetTable({
  budgets,
  onEdit,
  onDelete,
}: {
  budgets: AccountBudget[];
  onEdit: (budget: AccountBudget) => void;
  onDelete: (budget: AccountBudget) => void;
}) {
  const [sortBy, setSortBy] = useState<"usage" | "limit" | "team">("usage");

  const sorted = useMemo(() => {
    const arr = [...budgets];
    switch (sortBy) {
      case "usage":
        return arr.sort((a, b) => usagePercent(b.current_usd, b.limit_usd) - usagePercent(a.current_usd, a.limit_usd));
      case "limit":
        return arr.sort((a, b) => b.limit_usd - a.limit_usd);
      case "team":
        return arr.sort((a, b) => (a.team ?? "").localeCompare(b.team ?? ""));
      default:
        return arr;
    }
  }, [budgets, sortBy]);

  if (budgets.length === 0) {
    return (
      <p className="text-sm text-orbflow-text-ghost py-6 text-center">
        No budgets match the current filters
      </p>
    );
  }

  return (
    <div className="overflow-x-auto">
      {/* Sort controls */}
      <div className="flex items-center gap-2 mb-3 px-1">
        <span className="text-xs text-orbflow-text-ghost">Sort:</span>
        {(["usage", "limit", "team"] as const).map((s) => (
          <button
            key={s}
            onClick={() => setSortBy(s)}
            className={cn(
              "rounded-md px-2 py-1 text-xs font-medium transition-colors capitalize",
              "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
              sortBy === s
                ? "bg-electric-indigo/15 text-electric-indigo"
                : "text-orbflow-text-ghost hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover"
            )}
          >
            {s}
          </button>
        ))}
      </div>

      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-orbflow-border text-left text-xs uppercase tracking-wider text-orbflow-text-ghost">
            <th className="pb-3 pr-4 font-medium">Workflow</th>
            <th className="pb-3 pr-4 font-medium">Team</th>
            <th className="pb-3 pr-4 font-medium">Period</th>
            <th className="pb-3 pr-4 text-right font-medium">Budget</th>
            <th className="pb-3 pr-4 text-right font-medium">Spent</th>
            <th className="pb-3 pr-4 font-medium">Usage</th>
            <th className="pb-3 pr-4 font-medium">Status</th>
            <th className="pb-3 pr-4 font-medium">Resets</th>
            <th className="pb-3 font-medium" />
          </tr>
        </thead>
        <tbody>
          {sorted.map((budget) => {
            const pct = usagePercent(budget.current_usd, budget.limit_usd);
            const variant = usageVariant(pct);
            return (
              <tr
                key={budget.id}
                className={cn(
                  "border-b border-orbflow-border/50 text-orbflow-text-muted transition-colors",
                  variant === "danger" && "bg-red-500/[0.02]",
                  "hover:bg-orbflow-surface-hover/30"
                )}
              >
                <td className="py-2.5 pr-4 font-mono text-xs text-orbflow-text-secondary">
                  {budget.workflow_id ?? (
                    <span className="flex items-center gap-1">
                      <NodeIcon name="globe" className="w-3 h-3 text-orbflow-text-ghost" />
                      All workflows
                    </span>
                  )}
                </td>
                <td className="py-2.5 pr-4 text-xs">
                  {budget.team ? (
                    <span className="rounded-md bg-orbflow-surface-hover px-1.5 py-0.5">
                      {budget.team}
                    </span>
                  ) : (
                    <span className="text-orbflow-text-ghost">--</span>
                  )}
                </td>
                <td className="py-2.5 pr-4 text-xs capitalize">{budget.period}</td>
                <td className="py-2.5 pr-4 text-right font-mono text-xs tabular-nums">
                  {formatCurrency(budget.limit_usd)}
                </td>
                <td className="py-2.5 pr-4 text-right font-mono text-xs tabular-nums">
                  {formatCurrency(budget.current_usd)}
                </td>
                <td className="py-2.5 pr-4">
                  <UsageBar current={budget.current_usd} limit={budget.limit_usd} />
                </td>
                <td className="py-2.5 pr-4">
                  <StatusBadge current={budget.current_usd} limit={budget.limit_usd} resetAt={budget.reset_at} />
                </td>
                <td className="py-2.5 pr-4 text-xs text-orbflow-text-ghost tabular-nums">
                  {budget.reset_at ? `${daysUntilReset(budget.reset_at)}d` : "--"}
                </td>
                <td className="py-2.5">
                  <div className="flex items-center gap-1">
                    <button
                      onClick={() => onEdit(budget)}
                      className="rounded-md p-1.5 text-orbflow-text-ghost hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover
                        transition-colors focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                      aria-label={`Edit budget${budget.team ? ` for ${budget.team}` : ""}`}
                      title="Edit budget"
                    >
                      <NodeIcon name="edit" className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => onDelete(budget)}
                      className="rounded-md p-1.5 text-orbflow-text-ghost hover:text-red-400 hover:bg-red-500/10
                        transition-colors focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                      aria-label={`Delete budget${budget.team ? ` for ${budget.team}` : ""}`}
                      title="Delete budget"
                    >
                      <NodeIcon name="trash" className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/* --- BudgetManager (main export) ------------------------ */

export function BudgetManager() {
  const budgets = useBudgetStore((s) => s.budgets);
  const loading = useBudgetStore((s) => s.loading);
  const error = useBudgetStore((s) => s.error);
  const fetchBudgets = useBudgetStore((s) => s.fetchBudgets);
  const createBudget = useBudgetStore((s) => s.createBudget);
  const updateBudget = useBudgetStore((s) => s.updateBudget);
  const deleteBudget = useBudgetStore((s) => s.deleteBudget);

  const workflows = useWorkflowStore((s) => s.workflows);
  const fetchWorkflows = useWorkflowStore((s) => s.fetchWorkflows);

  const [showForm, setShowForm] = useState(false);
  const [editingBudget, setEditingBudget] = useState<AccountBudget | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<AccountBudget | null>(null);
  const [costRange, setCostRange] = useState("30d");

  useEffect(() => {
    fetchBudgets().catch(() => { /* store handles toast */ });
    fetchWorkflows().catch(() => { /* store handles toast */ });
  }, [fetchBudgets, fetchWorkflows]);

  const handleSave = useCallback(
    async (input: CreateBudgetInput) => {
      if (editingBudget) {
        await updateBudget(editingBudget.id, input);
      } else {
        await createBudget(input);
      }
      setShowForm(false);
      setEditingBudget(null);
    },
    [editingBudget, createBudget, updateBudget]
  );

  const handleEdit = useCallback((budget: AccountBudget) => {
    setEditingBudget(budget);
    setShowForm(true);
  }, []);

  const handleCreate = useCallback(() => {
    setEditingBudget(null);
    setShowForm(true);
  }, []);

  const handleCancelForm = useCallback(() => {
    setShowForm(false);
    setEditingBudget(null);
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    if (!confirmDelete) return;
    await deleteBudget(confirmDelete.id);
    setConfirmDelete(null);
    if (editingBudget?.id === confirmDelete.id) {
      setShowForm(false);
      setEditingBudget(null);
    }
  }, [confirmDelete, deleteBudget, editingBudget]);

  /* --- Loading --- */
  if (loading && budgets.length === 0) {
    return (
      <div className="flex items-center justify-center py-16">
        <div className="flex items-center gap-3">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-orbflow-border border-t-electric-indigo" />
          <span className="text-sm text-orbflow-text-ghost">Loading budgets...</span>
        </div>
      </div>
    );
  }

  /* --- Error --- */
  if (error && budgets.length === 0) {
    return (
      <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-5 animate-fade-in">
        <div className="flex items-start gap-3">
          <NodeIcon name="alert-triangle" className="w-4 h-4 text-red-400 mt-0.5 shrink-0" />
          <div>
            <p className="text-sm font-medium text-red-400">Failed to load budgets</p>
            <p className="text-xs text-orbflow-text-ghost mt-1">{error}</p>
            <button
              onClick={() => fetchBudgets().catch(() => { /* store handles toast */ })}
              className="mt-2 flex items-center gap-1.5 text-xs font-medium text-red-400 hover:text-red-300
                rounded-md px-2.5 py-1 border border-red-500/20 hover:bg-red-500/10
                transition-colors focus-visible:ring-2 focus-visible:ring-red-500/50 focus-visible:outline-none"
            >
              <NodeIcon name="refresh-cw" className="w-3 h-3" />
              Try again
            </button>
          </div>
        </div>
      </div>
    );
  }

  /* --- Empty --- */
  if (budgets.length === 0 && !showForm) {
    return <EmptyState onCreateClick={handleCreate} />;
  }

  return (
    <div className="space-y-6">
      {/* Cost Overview Section */}
      <CostOverview
        budgets={budgets}
        costRange={costRange}
        onCostRangeChange={setCostRange}
      />

      {/* Budget Management Section */}
      <div className="rounded-2xl border border-orbflow-border bg-orbflow-surface overflow-hidden">
        <div className="flex items-center justify-between px-5 py-4 border-b border-orbflow-border/50 bg-gradient-to-r from-orbflow-surface to-orbflow-bg">
          <h3 className="text-sm font-semibold text-orbflow-text-secondary flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-electric-indigo/10 flex items-center justify-center">
              <NodeIcon name="layers" className="w-3.5 h-3.5 text-electric-indigo" />
            </div>
            Budget Management
            <span className="text-[10px] text-orbflow-text-ghost font-medium tabular-nums px-1.5 py-0.5 rounded-md bg-orbflow-surface-hover">
              {budgets.length}
            </span>
          </h3>
          <button
            onClick={handleCreate}
            className="flex items-center gap-1.5 rounded-lg bg-electric-indigo px-3.5 py-2 text-xs font-semibold text-white
              hover:bg-electric-indigo/85 active:bg-electric-indigo/75 transition-all duration-200
              shadow-sm shadow-electric-indigo/20 hover:shadow-md hover:shadow-electric-indigo/25
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          >
            <NodeIcon name="plus" className="w-3.5 h-3.5" />
            New Budget
          </button>
        </div>
        <div className="p-5">

        {/* Inline form */}
        {showForm && (
          <div className="mb-5 rounded-xl border border-electric-indigo/20 bg-electric-indigo/[0.02] p-4 animate-scale-in">
            <BudgetForm
              workflows={workflows}
              editingId={editingBudget?.id}
              initialData={
                editingBudget
                  ? {
                      workflow_id: editingBudget.workflow_id ?? undefined,
                      team: editingBudget.team ?? undefined,
                      period: editingBudget.period,
                      limit_usd: editingBudget.limit_usd,
                    }
                  : undefined
              }
              onSave={handleSave}
              onCancel={handleCancelForm}
            />
          </div>
        )}

        <BudgetTable
          budgets={budgets}
          onEdit={handleEdit}
          onDelete={setConfirmDelete}
        />
        </div>
      </div>

      {/* Delete confirmation */}
      {confirmDelete && (
        <ConfirmDialog
          title="Delete budget?"
          message={`This budget${confirmDelete.team ? ` for team "${confirmDelete.team}"` : ""} will be permanently deleted. Cost tracking will stop for this budget.`}
          confirmLabel="Delete"
          cancelLabel="Cancel"
          variant="danger"
          onConfirm={handleConfirmDelete}
          onCancel={() => setConfirmDelete(null)}
        />
      )}
    </div>
  );
}
