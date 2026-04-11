"use client";

import { useEffect, useState, useCallback, useMemo } from "react";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { NodeIcon } from "@/core/components/icons";
import type { WorkflowMetricsSummary, NodeMetricsSummary } from "@orbflow/core";

interface WorkflowAnalyticsProps {
  workflowId: string;
}

/* -- Formatters ---------------------------------------- */

function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
}

function formatRate(rate: number): string {
  return `${(rate * 100).toFixed(1)}%`;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

/* -- SVG Gauge ----------------------------------------- */

function SuccessGauge({ rate, size = 140 }: { rate: number; size?: number }) {
  const [animated, setAnimated] = useState(false);

  // Trigger the draw animation one frame after mount
  useEffect(() => {
    const raf = requestAnimationFrame(() => setAnimated(true));
    return () => cancelAnimationFrame(raf);
  }, []);

  const strokeWidth = 10;
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const sweepAngle = 270;
  const arcLength = (sweepAngle / 360) * circumference;
  const filledLength = arcLength * rate;
  // Gap that completes the "open" arc
  const gap = circumference - arcLength;

  const color =
    rate >= 0.95
      ? "var(--color-port-string)"
      : rate >= 0.8
        ? "#F59E0B"
        : "#EF4444";

  const bgColor = "var(--orbflow-border)";

  // Both arcs share the same rotation so the opening points downward
  const arcRotation = "rotate(135, " + size / 2 + ", " + size / 2 + ")";

  return (
    <div className="relative flex items-center justify-center" style={{ width: size, height: size }}>
      <svg
        width={size}
        height={size}
        viewBox={`0 0 ${size} ${size}`}
        aria-label={`Success rate: ${formatRate(rate)}`}
        role="img"
      >
        {/* Background arc */}
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={bgColor}
          strokeWidth={strokeWidth}
          strokeDasharray={`${arcLength} ${gap}`}
          strokeLinecap="round"
          transform={arcRotation}
        />
        {/* Filled arc -- transitions from 0 to target length */}
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={color}
          strokeWidth={strokeWidth}
          strokeDasharray={animated ? `${filledLength} ${circumference - filledLength}` : `0 ${circumference}`}
          strokeLinecap="round"
          transform={arcRotation}
          style={{
            transition: "stroke-dasharray 1.2s cubic-bezier(0.16, 1, 0.3, 1), opacity 0.4s ease",
            opacity: animated ? 1 : 0.3,
            filter: `drop-shadow(0 0 8px ${color}50)`,
          }}
        />
      </svg>
      {/* Center label */}
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className="text-2xl font-bold tabular-nums" style={{ color }}>
          {formatRate(rate)}
        </span>
        <span className="text-[10px] uppercase tracking-wider text-orbflow-text-ghost mt-0.5">
          Success
        </span>
      </div>
    </div>
  );
}

/* -- Mini Sparkline (synthetic) ------------------------ */

let sparklineCounter = 0;

function MiniSparkline({
  value,
  color = "var(--color-electric-indigo)",
}: {
  value: number;
  color?: string;
}) {
  // Stable unique ID per instance to avoid SVG gradient collisions
  const [uid] = useState(() => `spark-${++sparklineCounter}`);
  // Generate a synthetic sparkline based on the value as a seed
  const seed = Math.abs(value) % 100;
  const points = Array.from({ length: 7 }, (_, i) => {
    const noise = Math.sin(seed + i * 1.8) * 0.3 + Math.cos(seed * 0.5 + i) * 0.2;
    const base = 0.3 + (i / 6) * 0.4;
    return Math.max(0.05, Math.min(1, base + noise));
  });

  const w = 64;
  const h = 24;
  const padding = 2;
  const stepX = (w - padding * 2) / (points.length - 1);

  const pathD = points
    .map((p, i) => {
      const x = padding + i * stepX;
      const y = h - padding - p * (h - padding * 2);
      return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");

  const areaD = `${pathD} L${(w - padding).toFixed(1)},${h} L${padding},${h} Z`;

  return (
    <svg width={w} height={h} className="overflow-visible opacity-60" aria-hidden>
      <defs>
        <linearGradient id={uid} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.3" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={areaD} fill={`url(#${uid})`} />
      <path d={pathD} fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/* -- Metric Card --------------------------------------- */

function MetricCard({
  label,
  value,
  subtitle,
  icon,
  variant = "default",
  sparkColor,
  sparkValue,
  delay = 0,
}: {
  label: string;
  value: string | number;
  subtitle?: string;
  icon?: string;
  variant?: "default" | "success" | "danger" | "warning";
  sparkColor?: string;
  sparkValue?: number;
  delay?: number;
}) {
  const variantStyles = {
    default: {
      border: "border-orbflow-border",
      bg: "bg-orbflow-surface",
      glow: "",
      value: "text-orbflow-text-secondary",
      iconBg: "bg-orbflow-surface-hover",
      iconColor: "text-orbflow-text-ghost",
    },
    success: {
      border: "border-emerald-500/20",
      bg: "bg-emerald-500/[0.03]",
      glow: "shadow-[inset_0_1px_0_0_rgba(16,185,129,0.1)]",
      value: "text-emerald-400",
      iconBg: "bg-emerald-500/10",
      iconColor: "text-emerald-400",
    },
    danger: {
      border: "border-red-500/20",
      bg: "bg-red-500/[0.03]",
      glow: "shadow-[inset_0_1px_0_0_rgba(239,68,68,0.1)]",
      value: "text-red-400",
      iconBg: "bg-red-500/10",
      iconColor: "text-red-400",
    },
    warning: {
      border: "border-amber-500/20",
      bg: "bg-amber-500/[0.03]",
      glow: "shadow-[inset_0_1px_0_0_rgba(245,158,11,0.1)]",
      value: "text-amber-400",
      iconBg: "bg-amber-500/10",
      iconColor: "text-amber-400",
    },
  };

  const s = variantStyles[variant];

  return (
    <div
      className={cn(
        "group relative rounded-xl border p-4 transition-all duration-200",
        "hover:border-orbflow-border-hover hover:shadow-lg hover:shadow-orbflow-shadow/20",
        s.border, s.bg, s.glow,
        "animate-fade-in-up"
      )}
      style={{ animationDelay: `${delay}ms` }}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-3">
            {icon && (
              <div className={cn("w-6 h-6 rounded-md flex items-center justify-center", s.iconBg)}>
                <NodeIcon name={icon} className={cn("w-3 h-3", s.iconColor)} />
              </div>
            )}
            <p className="text-[11px] font-medium uppercase tracking-wider text-orbflow-text-ghost">
              {label}
            </p>
          </div>
          <p className={cn("text-2xl font-bold tabular-nums tracking-tight", s.value)}>{value}</p>
          {subtitle && (
            <p className="mt-1.5 text-[10px] text-orbflow-text-ghost">{subtitle}</p>
          )}
        </div>
        {sparkValue !== undefined && (
          <div className="mt-5 shrink-0">
            <MiniSparkline value={sparkValue} color={sparkColor} />
          </div>
        )}
      </div>
    </div>
  );
}

/* -- Latency Bar --------------------------------------- */

function LatencyBar({
  label,
  value,
  max,
  color,
  delay = 0,
}: {
  label: string;
  value: number;
  max: number;
  color: string;
  delay?: number;
}) {
  const pct = max > 0 ? Math.min((value / max) * 100, 100) : 0;

  return (
    <div
      className="group flex items-center gap-3 py-1.5 animate-fade-in-up"
      style={{ animationDelay: `${delay}ms` }}
    >
      <span className="w-8 text-right text-[11px] font-semibold text-orbflow-text-muted">
        {label}
      </span>
      <div className="flex-1 relative">
        <div className="h-3 rounded-full bg-orbflow-border/30 overflow-hidden">
          <div
            className="h-full rounded-full transition-all duration-700 ease-out"
            style={{
              width: `${pct}%`,
              background: `linear-gradient(90deg, ${color}80, ${color})`,
              boxShadow: `0 0 8px ${color}30`,
            }}
          />
        </div>
        {/* Percentage tooltip on hover */}
        <div
          className="absolute -top-6 opacity-0 group-hover:opacity-100 transition-opacity text-[10px] font-mono
            text-orbflow-text-muted bg-orbflow-elevated px-1.5 py-0.5 rounded pointer-events-none"
          style={{ left: `${Math.max(pct - 3, 0)}%` }}
        >
          {pct.toFixed(0)}%
        </div>
      </div>
      <span className="w-16 text-right text-xs font-mono text-orbflow-text-muted tabular-nums">
        {formatDuration(value)}
      </span>
    </div>
  );
}

/* -- Node Table ---------------------------------------- */

type SortField = "node_id" | "total_executions" | "success_rate" | "avg_duration_ms" | "p95_duration_ms";
type SortDir = "asc" | "desc";

function SortIcon({ active, dir }: { active: boolean; dir: SortDir }) {
  return (
    <span className={cn("inline-block ml-1 transition-opacity", active ? "opacity-100" : "opacity-0 group-hover/th:opacity-30")}>
      {dir === "asc" ? "\u2191" : "\u2193"}
    </span>
  );
}

function NodeMetricsTable({ nodes }: { nodes: NodeMetricsSummary[] }) {
  const [sortField, setSortField] = useState<SortField>("total_executions");
  const [sortDir, setSortDir] = useState<SortDir>("desc");

  const handleSort = useCallback((field: SortField) => {
    setSortField((prev) => (prev === field ? prev : field));
    setSortDir((prev) =>
      sortField === field ? (prev === "asc" ? "desc" : "asc") : "desc"
    );
  }, [sortField]);

  const sorted = useMemo(() => {
    const copy = [...nodes];
    const dir = sortDir === "asc" ? 1 : -1;
    copy.sort((a, b) => {
      const av = a[sortField];
      const bv = b[sortField];
      if (typeof av === "string" && typeof bv === "string") return av.localeCompare(bv) * dir;
      return ((av as number) - (bv as number)) * dir;
    });
    return copy;
  }, [nodes, sortField, sortDir]);

  if (nodes.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-10">
        <NodeIcon name="layers" className="w-6 h-6 text-orbflow-text-ghost mb-2" />
        <p className="text-xs text-orbflow-text-ghost">No node metrics recorded yet</p>
      </div>
    );
  }

  const headers: { label: string; field: SortField; align: "left" | "right" }[] = [
    { label: "Node", field: "node_id", align: "left" },
    { label: "Runs", field: "total_executions", align: "right" },
    { label: "Success", field: "success_rate", align: "right" },
    { label: "Avg", field: "avg_duration_ms", align: "right" },
    { label: "P95", field: "p95_duration_ms", align: "right" },
  ];

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm" aria-label="Node performance metrics">
        <thead>
          <tr className="border-b border-orbflow-border">
            {headers.map((h) => (
              <th
                key={h.field}
                scope="col"
                className={cn(
                  "group/th pb-3 pr-4 last:pr-0 text-[10px] uppercase tracking-wider font-semibold text-orbflow-text-ghost cursor-pointer select-none",
                  "hover:text-orbflow-text-muted transition-colors",
                  h.align === "right" && "text-right"
                )}
                onClick={() => handleSort(h.field)}
                aria-sort={sortField === h.field ? (sortDir === "asc" ? "ascending" : "descending") : undefined}
              >
                {h.label}
                <SortIcon active={sortField === h.field} dir={sortDir} />
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.map((node, idx) => {
            const rateColor =
              node.success_rate >= 0.95
                ? "text-emerald-400"
                : node.success_rate >= 0.8
                  ? "text-amber-400"
                  : "text-red-400";

            const rateBg =
              node.success_rate >= 0.95
                ? "bg-emerald-400/10"
                : node.success_rate >= 0.8
                  ? "bg-amber-400/10"
                  : "bg-red-400/10";

            return (
              <tr
                key={node.node_id}
                className={cn(
                  "border-b border-orbflow-border/30 text-orbflow-text-muted transition-colors",
                  "hover:bg-orbflow-surface-hover/50",
                  "animate-fade-in-up"
                )}
                style={{ animationDelay: `${80 + idx * 40}ms` }}
              >
                <td className="py-3 pr-4">
                  <div className="flex items-center gap-2">
                    <div className="w-5 h-5 rounded bg-orbflow-surface-hover flex items-center justify-center shrink-0">
                      <NodeIcon name="box" className="w-2.5 h-2.5 text-orbflow-text-ghost" />
                    </div>
                    <div className="min-w-0">
                      <p className="font-mono text-xs text-orbflow-text-secondary truncate">{node.node_id}</p>
                      <p className="text-[10px] text-orbflow-text-ghost">
                        {node.plugin_ref.replace("builtin:", "")}
                      </p>
                    </div>
                  </div>
                </td>
                <td className="py-3 pr-4 text-right tabular-nums font-medium">{node.total_executions}</td>
                <td className="py-3 pr-4 text-right">
                  <span className={cn("inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold tabular-nums", rateColor, rateBg)}>
                    {formatRate(node.success_rate)}
                  </span>
                </td>
                <td className="py-3 pr-4 text-right font-mono text-xs tabular-nums">
                  {formatDuration(node.avg_duration_ms)}
                </td>
                <td className="py-3 text-right font-mono text-xs tabular-nums">
                  {formatDuration(node.p95_duration_ms)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/* -- Empty State --------------------------------------- */

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-center animate-fade-in">
      <div className="relative mb-6">
        <div className="w-20 h-20 rounded-2xl bg-orbflow-surface-hover flex items-center justify-center">
          <NodeIcon name="bar-chart" className="w-10 h-10 text-orbflow-text-ghost" />
        </div>
        <div className="absolute -top-1 -right-1 w-6 h-6 rounded-full bg-electric-indigo/10 flex items-center justify-center">
          <NodeIcon name="play" className="w-3 h-3 text-electric-indigo" />
        </div>
      </div>
      <h3 className="text-sm font-semibold text-orbflow-text-secondary mb-1.5">No executions yet</h3>
      <p className="text-xs text-orbflow-text-ghost max-w-xs leading-relaxed">
        Run this workflow to start collecting execution metrics,
        latency percentiles, and per-node performance insights.
      </p>
    </div>
  );
}

/* -- Section Wrapper ----------------------------------- */

function Section({
  icon,
  title,
  children,
  delay = 0,
  action,
}: {
  icon: string;
  title: string;
  children: React.ReactNode;
  delay?: number;
  action?: React.ReactNode;
}) {
  return (
    <div
      className="rounded-xl border border-orbflow-border bg-orbflow-surface overflow-hidden animate-fade-in-up"
      style={{ animationDelay: `${delay}ms` }}
    >
      <div className="flex items-center justify-between px-5 pt-4 pb-3">
        <h3 className="text-xs font-semibold text-orbflow-text-secondary flex items-center gap-2 uppercase tracking-wider">
          <NodeIcon name={icon} className="w-3.5 h-3.5 text-orbflow-text-ghost" />
          {title}
        </h3>
        {action}
      </div>
      <div className="px-5 pb-5">{children}</div>
    </div>
  );
}

/* -- Auto-Refresh Button ------------------------------- */

function RefreshButton({ loading, onRefresh }: { loading: boolean; onRefresh: () => void }) {
  return (
    <button
      onClick={onRefresh}
      disabled={loading}
      className={cn(
        "flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-medium",
        "border border-orbflow-border bg-orbflow-surface text-orbflow-text-muted",
        "hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary hover:border-orbflow-border-hover",
        "transition-all duration-150",
        "disabled:opacity-50 disabled:cursor-not-allowed",
        "focus:outline-none focus:ring-2 focus:ring-electric-indigo/50"
      )}
      aria-label="Refresh metrics"
    >
      <NodeIcon
        name="refresh"
        className={cn("w-3 h-3", loading && "animate-spin")}
      />
      Refresh
    </button>
  );
}

/* -- Main Component ------------------------------------ */

export function WorkflowAnalytics({ workflowId }: WorkflowAnalyticsProps) {
  const [summary, setSummary] = useState<WorkflowMetricsSummary | null>(null);
  const [nodes, setNodes] = useState<NodeMetricsSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [showSkeleton, setShowSkeleton] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Only show the skeleton if loading takes longer than 200ms — avoids flash on fast loads.
  useEffect(() => {
    if (!loading) {
      setShowSkeleton(false);
      return;
    }
    const timer = setTimeout(() => setShowSkeleton(true), 200);
    return () => clearTimeout(timer);
  }, [loading]);

  const fetchMetrics = useCallback(async (isRefresh = false) => {
    if (isRefresh) {
      setRefreshing(true);
    } else {
      setLoading(true);
    }
    setError(null);

    try {
      const [wfMetrics, nodeMetrics] = await Promise.all([
        api.metrics.getWorkflowMetrics(workflowId),
        api.metrics.getWorkflowNodeMetrics(workflowId),
      ]);
      setSummary(wfMetrics);
      setNodes(nodeMetrics);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to load metrics";
      if (msg.includes("not configured") || msg.includes("not found") || msg.includes("Not Found")) {
        setSummary(null);
        setNodes([]);
        setError(null);
      } else {
        setError(msg);
      }
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, [workflowId]);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      await fetchMetrics();
      if (cancelled) return;
    };
    run();
    return () => { cancelled = true; };
  }, [fetchMetrics]);

  /* -- Loading skeleton -- only shown if fetch takes > 200ms */
  if (showSkeleton) {
    return (
      <div className="space-y-5">
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          {[0, 1, 2, 3].map((i) => (
            <div
              key={i}
              className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4 animate-pulse"
            >
              <div className="h-3 w-16 rounded bg-orbflow-border/50 mb-3" />
              <div className="h-7 w-20 rounded bg-orbflow-border/50 mb-2" />
              <div className="h-2 w-12 rounded bg-orbflow-border/30" />
            </div>
          ))}
        </div>
        <div className="rounded-xl border border-orbflow-border bg-orbflow-surface p-5 animate-pulse">
          <div className="h-3 w-32 rounded bg-orbflow-border/50 mb-4" />
          <div className="space-y-3">
            {[0, 1, 2].map((i) => (
              <div key={i} className="h-3 rounded bg-orbflow-border/30" />
            ))}
          </div>
        </div>
      </div>
    );
  }

  /* -- Error state -- */
  if (error) {
    return (
      <div className="rounded-xl border border-red-500/20 bg-red-500/[0.03] p-6 animate-fade-in">
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 rounded-lg bg-red-500/10 flex items-center justify-center shrink-0">
            <NodeIcon name="alert-triangle" className="w-4 h-4 text-red-400" />
          </div>
          <div>
            <p className="text-sm font-semibold text-red-400">Failed to load metrics</p>
            <p className="text-xs text-orbflow-text-ghost mt-1 leading-relaxed">{error}</p>
            <button
              onClick={() => fetchMetrics()}
              className="mt-3 text-xs font-medium text-red-400 hover:text-red-300 transition-colors
                focus:outline-none focus:ring-2 focus:ring-red-400/50 rounded px-1"
            >
              Try again
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (!summary || summary.total_executions === 0) {
    return <EmptyState />;
  }

  const maxLatency = Math.max(summary.p99_duration_ms, 1);

  const latencyBars = [
    { label: "P50", value: summary.p50_duration_ms, color: "#22D3EE" },
    { label: "P95", value: summary.p95_duration_ms, color: "#7C5CFC" },
    { label: "P99", value: summary.p99_duration_ms, color: "#EC4899" },
    { label: "Avg", value: summary.avg_duration_ms, color: "#F59E0B" },
  ];

  return (
    <div className="space-y-5">
      {/* -- Top Row: Stats + Gauge -- */}
      <div className="grid grid-cols-1 lg:grid-cols-[1fr_auto] gap-5">
        {/* Metric Cards */}
        <div className="grid grid-cols-2 gap-3">
          <MetricCard
            label="Total Executions"
            value={formatNumber(summary.total_executions)}
            icon="play"
            subtitle={`Since ${summary.since ? new Date(summary.since).toLocaleDateString() : "start"}`}
            sparkValue={summary.total_executions}
            sparkColor="var(--color-electric-indigo)"
            delay={0}
          />
          <MetricCard
            label="Success Rate"
            value={formatRate(summary.success_rate)}
            icon="check"
            variant={
              summary.success_rate >= 0.95
                ? "success"
                : summary.success_rate >= 0.8
                  ? "warning"
                  : "danger"
            }
            sparkValue={summary.success_rate * 100}
            sparkColor={summary.success_rate >= 0.95 ? "#10B981" : summary.success_rate >= 0.8 ? "#F59E0B" : "#EF4444"}
            delay={50}
          />
          <MetricCard
            label="Avg Duration"
            value={formatDuration(summary.avg_duration_ms)}
            icon="clock"
            subtitle={`P99: ${formatDuration(summary.p99_duration_ms)}`}
            sparkValue={summary.avg_duration_ms}
            sparkColor="var(--color-neon-cyan)"
            delay={100}
          />
          <MetricCard
            label="Failed"
            value={formatNumber(summary.failed_executions)}
            icon="alert-triangle"
            variant={summary.failed_executions > 0 ? "danger" : "default"}
            subtitle={summary.failed_executions > 0
              ? `${formatRate(summary.failed_executions / summary.total_executions)} failure rate`
              : "No failures"}
            sparkValue={summary.failed_executions}
            sparkColor={summary.failed_executions > 0 ? "#EF4444" : "var(--color-electric-indigo)"}
            delay={150}
          />
        </div>

        {/* Gauge */}
        <div
          className="flex items-center justify-center rounded-xl border border-orbflow-border bg-orbflow-surface p-5 animate-fade-in-up"
          style={{ animationDelay: "100ms" }}
        >
          <SuccessGauge rate={summary.success_rate} />
        </div>
      </div>

      {/* -- Latency Distribution -- */}
      <Section icon="clock" title="Latency Distribution" delay={200} action={
        <RefreshButton loading={refreshing} onRefresh={() => fetchMetrics(true)} />
      }>
        <div className="space-y-1.5">
          {latencyBars.map((bar, i) => (
            <LatencyBar
              key={bar.label}
              label={bar.label}
              value={bar.value}
              max={maxLatency}
              color={bar.color}
              delay={250 + i * 50}
            />
          ))}
        </div>
        {/* Summary strip */}
        <div className="flex items-center gap-4 mt-4 pt-3 border-t border-orbflow-border/50">
          {latencyBars.map((bar) => (
            <div key={bar.label} className="flex items-center gap-1.5">
              <div className="w-2 h-2 rounded-full" style={{ backgroundColor: bar.color }} />
              <span className="text-[10px] text-orbflow-text-ghost">{bar.label}</span>
              <span className="text-[10px] font-mono text-orbflow-text-muted tabular-nums">
                {formatDuration(bar.value)}
              </span>
            </div>
          ))}
        </div>
      </Section>

      {/* -- Node Performance -- */}
      <Section icon="layers" title="Node Performance" delay={350}>
        <NodeMetricsTable nodes={nodes} />
      </Section>
    </div>
  );
}
