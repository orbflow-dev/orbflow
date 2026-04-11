"use client";

/**
 * Displays cost and resource metrics for a node execution.
 *
 * Extracts cost_usd, usage (tokens), and duration from the node output.
 */

interface NodeMetricsDisplayProps {
  output: Record<string, unknown> | undefined;
  startedAt: string | undefined;
  endedAt: string | undefined;
}

interface MetricItem {
  label: string;
  value: string;
  color?: string;
}

export function NodeMetricsDisplay({ output, startedAt, endedAt }: NodeMetricsDisplayProps) {
  const metrics: MetricItem[] = [];

  // Duration
  if (startedAt && endedAt) {
    const ms = Math.max(0, new Date(endedAt).getTime() - new Date(startedAt).getTime());
    metrics.push({
      label: "Duration",
      value: ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`,
    });
  }

  if (!output) {
    if (metrics.length === 0) return null;
    return <MetricsBar metrics={metrics} />;
  }

  // Cost
  const costUsd = typeof output.cost_usd === "number" ? output.cost_usd : null;
  if (costUsd !== null) {
    metrics.push({
      label: "Cost",
      value: costUsd < 0.01 ? `$${costUsd.toFixed(4)}` : `$${costUsd.toFixed(2)}`,
      color: costUsd > 0.1 ? "#E8A317" : "#10B981",
    });
  }

  // Tokens
  const usage = output.usage as Record<string, unknown> | undefined;
  if (usage && typeof usage === "object") {
    const total = typeof usage.total_tokens === "number" ? usage.total_tokens : null;
    const prompt = typeof usage.prompt_tokens === "number" ? usage.prompt_tokens : null;
    const completion = typeof usage.completion_tokens === "number" ? usage.completion_tokens : null;

    if (total !== null) {
      const detail =
        prompt !== null && completion !== null
          ? `${prompt} in / ${completion} out`
          : `${total} total`;
      metrics.push({ label: "Tokens", value: detail });
    }
  }

  // Model
  if (typeof output.model === "string") {
    metrics.push({ label: "Model", value: output.model as string });
  }

  // HTTP status
  if (typeof output.status === "number") {
    const status = output.status as number;
    metrics.push({
      label: "HTTP",
      value: `${status}`,
      color: status >= 400 ? "#D9454F" : status >= 300 ? "#E8A317" : "#10B981",
    });
  }

  if (metrics.length === 0) return null;

  return <MetricsBar metrics={metrics} />;
}

function MetricsBar({ metrics }: { metrics: MetricItem[] }) {
  return (
    <div
      style={{
        display: "flex",
        flexWrap: "wrap",
        gap: 12,
        padding: "8px 12px",
        borderRadius: 6,
        background: "rgba(255,255,255,0.03)",
        border: "1px solid rgba(255,255,255,0.06)",
        fontSize: 11,
      }}
    >
      {metrics.map((m) => (
        <div key={m.label} style={{ display: "flex", gap: 4, alignItems: "center" }}>
          <span style={{ color: "var(--text-tertiary, #6B7280)" }}>{m.label}:</span>
          <span
            style={{
              color: m.color || "var(--text-primary, #E5E7EB)",
              fontWeight: 500,
              fontVariantNumeric: "tabular-nums",
            }}
          >
            {m.value}
          </span>
        </div>
      ))}
    </div>
  );
}
