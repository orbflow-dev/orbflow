"use client";

import { useEffect, useState, useCallback, useMemo } from "react";
import { useAlertStore } from "@/store/alert-store";
import { useWorkflowStore } from "@/store/workflow-store";
import { ConfirmDialog } from "@/core/components/confirm-dialog";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { AlertForm } from "./alert-form";
import type { AlertRule, AlertMetric, CreateAlertInput } from "@orbflow/core";

/* =======================================================
   Display helpers
   ======================================================= */

const METRIC_LABELS: Record<AlertMetric, string> = {
  failure_rate: "Failure Rate",
  p95_duration: "P95 Duration",
  execution_count: "Execution Count",
};

const METRIC_ICONS: Record<AlertMetric, string> = {
  failure_rate: "alert-triangle",
  p95_duration: "clock",
  execution_count: "zap",
};

const METRIC_COLORS: Record<AlertMetric, { text: string; bg: string; border: string }> = {
  failure_rate: { text: "text-rose-400", bg: "bg-rose-500/10", border: "border-rose-500/20" },
  p95_duration: { text: "text-amber-400", bg: "bg-amber-500/10", border: "border-amber-500/20" },
  execution_count: { text: "text-sky-400", bg: "bg-sky-500/10", border: "border-sky-500/20" },
};

const OPERATOR_SYMBOLS: Record<string, string> = {
  greater_than: ">",
  less_than: "<",
  equals: "=",
};

function formatThreshold(metric: AlertMetric, threshold: number): string {
  switch (metric) {
    case "failure_rate":
      return `${threshold}%`;
    case "p95_duration":
      return `${threshold}ms`;
    default:
      return String(threshold);
  }
}

/* --- SeverityIndicator ----------------------------------- */

function SeverityIndicator({ metric, threshold }: { metric: AlertMetric; threshold: number }) {
  const isCritical =
    (metric === "failure_rate" && threshold >= 50) ||
    (metric === "p95_duration" && threshold >= 5000);

  return (
    <div className={cn(
      "flex items-center gap-1.5 rounded-md px-2 py-0.5 text-xs font-medium",
      isCritical
        ? "bg-red-500/10 text-red-400"
        : "bg-amber-500/10 text-amber-400"
    )}>
      <span className={cn(
        "w-1.5 h-1.5 rounded-full",
        isCritical ? "bg-red-400 animate-pulse-soft" : "bg-amber-400"
      )} />
      {isCritical ? "Critical" : "Warning"}
    </div>
  );
}

/* --- AlertSummaryCards ------------------------------------ */

function AlertSummaryCards({ alerts }: { alerts: AlertRule[] }) {
  const totalAlerts = alerts.length;
  const enabledCount = alerts.filter((a) => a.enabled).length;
  const disabledCount = totalAlerts - enabledCount;
  const byMetric = alerts.reduce<Record<string, number>>((acc, a) => {
    acc[a.metric] = (acc[a.metric] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
      <div className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4">
        <div className="flex items-center gap-1.5 mb-2">
          <NodeIcon name="bell" className="w-3 h-3 text-orbflow-text-ghost" />
          <p className="text-xs font-medium uppercase tracking-wider text-orbflow-text-ghost">Total</p>
        </div>
        <p className="text-2xl font-bold text-orbflow-text-secondary tabular-nums">{totalAlerts}</p>
      </div>
      <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
        <div className="flex items-center gap-1.5 mb-2">
          <span className="w-2 h-2 rounded-full bg-emerald-500" />
          <p className="text-xs font-medium uppercase tracking-wider text-orbflow-text-ghost">Active</p>
        </div>
        <p className="text-2xl font-bold text-emerald-400 tabular-nums">{enabledCount}</p>
      </div>
      <div className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4">
        <div className="flex items-center gap-1.5 mb-2">
          <span className="w-2 h-2 rounded-full bg-orbflow-text-ghost" />
          <p className="text-xs font-medium uppercase tracking-wider text-orbflow-text-ghost">Paused</p>
        </div>
        <p className="text-2xl font-bold text-orbflow-text-muted tabular-nums">{disabledCount}</p>
      </div>
      <div className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4">
        <div className="flex items-center gap-1.5 mb-2">
          <NodeIcon name="layers" className="w-3 h-3 text-orbflow-text-ghost" />
          <p className="text-xs font-medium uppercase tracking-wider text-orbflow-text-ghost">Metrics</p>
        </div>
        <div className="flex flex-wrap gap-1.5 mt-1">
          {Object.entries(byMetric).map(([metric, count]) => (
            <span
              key={metric}
              className={cn(
                "inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-xs font-medium",
                METRIC_COLORS[metric as AlertMetric]?.bg,
                METRIC_COLORS[metric as AlertMetric]?.text
              )}
            >
              {count}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

/* --- AlertCard -------------------------------------------- */

function AlertCard({
  alert,
  workflowName,
  onToggle,
  onEdit,
  onDelete,
}: {
  alert: AlertRule;
  workflowName: string;
  onToggle: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const metricColor = METRIC_COLORS[alert.metric];

  return (
    <div
      className={cn(
        "group rounded-xl border bg-orbflow-surface p-4 transition-all duration-200",
        alert.enabled
          ? "border-orbflow-border hover:border-orbflow-border-hover"
          : "border-orbflow-border/50 opacity-60 hover:opacity-80"
      )}
    >
      <div className="flex items-start justify-between gap-4">
        {/* Left: Info */}
        <div className="flex-1 min-w-0">
          {/* Header row: metric icon + name + severity */}
          <div className="flex items-center gap-2.5 flex-wrap">
            <div className={cn(
              "flex items-center justify-center w-7 h-7 rounded-lg",
              metricColor.bg
            )}>
              <NodeIcon name={METRIC_ICONS[alert.metric]} className={cn("w-3.5 h-3.5", metricColor.text)} />
            </div>
            <div>
              <span className={cn("text-sm font-semibold", metricColor.text)}>
                {METRIC_LABELS[alert.metric]}
              </span>
              <span className="text-sm text-orbflow-text-muted ml-2">
                {OPERATOR_SYMBOLS[alert.operator]}{" "}
                {formatThreshold(alert.metric, alert.threshold)}
              </span>
            </div>
            <SeverityIndicator metric={alert.metric} threshold={alert.threshold} />
          </div>

          {/* Detail row */}
          <div className="mt-3 flex flex-wrap items-center gap-3 text-xs">
            {/* Channel */}
            <div className="flex items-center gap-1.5 rounded-md bg-orbflow-surface-hover px-2 py-1">
              <NodeIcon
                name={alert.channel.type === "webhook" ? "webhook" : "terminal"}
                className="w-3 h-3 text-orbflow-text-ghost"
              />
              <span className="text-orbflow-text-muted">
                {alert.channel.type === "webhook" ? "Webhook" : "Log"}
              </span>
            </div>

            {/* Scope */}
            <div className="flex items-center gap-1.5 rounded-md bg-orbflow-surface-hover px-2 py-1">
              <NodeIcon name="workflow" className="w-3 h-3 text-orbflow-text-ghost" />
              <span className="text-orbflow-text-muted">{workflowName}</span>
            </div>

            {/* Webhook URL preview */}
            {alert.channel.type === "webhook" && (
              <span
                className="truncate max-w-[200px] text-orbflow-text-ghost font-mono"
                title={alert.channel.url}
              >
                {alert.channel.url}
              </span>
            )}
          </div>
        </div>

        {/* Right: Actions */}
        <div className="flex items-center gap-1.5 shrink-0">
          {/* Toggle */}
          <button
            type="button"
            role="switch"
            aria-checked={alert.enabled}
            aria-label={alert.enabled ? "Disable alert" : "Enable alert"}
            onClick={onToggle}
            className={cn(
              "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors",
              alert.enabled ? "bg-emerald-500" : "bg-orbflow-border"
            )}
          >
            <span
              className={cn(
                "inline-block h-3.5 w-3.5 rounded-full bg-white shadow-sm transition-transform",
                alert.enabled ? "translate-x-[18px]" : "translate-x-[3px]"
              )}
            />
          </button>

          {/* Edit */}
          <button
            onClick={onEdit}
            className="rounded-md p-1.5 text-orbflow-text-ghost transition-colors hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary"
            title="Edit"
          >
            <NodeIcon name="edit" className="w-3.5 h-3.5" />
          </button>

          {/* Delete */}
          <button
            onClick={onDelete}
            className="rounded-md p-1.5 text-orbflow-text-ghost transition-colors hover:bg-rose-500/10 hover:text-rose-400"
            title="Delete"
          >
            <NodeIcon name="trash" className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </div>
  );
}

/* =======================================================
   Main Component
   ======================================================= */

export function AlertManager() {
  const alerts = useAlertStore((s) => s.alerts);
  const loading = useAlertStore((s) => s.loading);
  const error = useAlertStore((s) => s.error);
  const fetchAlerts = useAlertStore((s) => s.fetchAlerts);
  const createAlert = useAlertStore((s) => s.createAlert);
  const updateAlert = useAlertStore((s) => s.updateAlert);
  const deleteAlert = useAlertStore((s) => s.deleteAlert);
  const toggleAlert = useAlertStore((s) => s.toggleAlert);

  const workflows = useWorkflowStore((s) => s.workflows);
  const fetchWorkflows = useWorkflowStore((s) => s.fetchWorkflows);

  const [showForm, setShowForm] = useState(false);
  const [editingAlert, setEditingAlert] = useState<AlertRule | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<AlertRule | null>(null);
  const [filterMetric, setFilterMetric] = useState<AlertMetric | "all">("all");
  const [filterStatus, setFilterStatus] = useState<"all" | "enabled" | "disabled">("all");

  useEffect(() => {
    fetchAlerts().catch(() => { /* store handles toast */ });
    fetchWorkflows().catch(() => { /* store handles toast */ });
  }, [fetchAlerts, fetchWorkflows]);

  const handleCreate = useCallback(() => {
    setEditingAlert(null);
    setShowForm(true);
  }, []);

  const handleEdit = useCallback((alert: AlertRule) => {
    setEditingAlert(alert);
    setShowForm(true);
  }, []);

  const handleSave = useCallback(
    async (input: CreateAlertInput) => {
      if (editingAlert) {
        await updateAlert(editingAlert.id, input);
      } else {
        await createAlert(input);
      }
      setShowForm(false);
      setEditingAlert(null);
    },
    [editingAlert, updateAlert, createAlert]
  );

  const handleCancelForm = useCallback(() => {
    setShowForm(false);
    setEditingAlert(null);
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    if (!confirmDelete) return;
    await deleteAlert(confirmDelete.id);
    setConfirmDelete(null);
    if (editingAlert?.id === confirmDelete.id) {
      setShowForm(false);
      setEditingAlert(null);
    }
  }, [confirmDelete, deleteAlert, editingAlert]);

  const workflowName = useCallback(
    (id: string | null | undefined) => {
      if (!id) return "All workflows";
      const wf = workflows.find((w) => w.id === id);
      return wf?.name ?? id;
    },
    [workflows]
  );

  const filteredAlerts = useMemo(() => {
    return alerts.filter((a) => {
      if (filterMetric !== "all" && a.metric !== filterMetric) return false;
      if (filterStatus === "enabled" && !a.enabled) return false;
      if (filterStatus === "disabled" && a.enabled) return false;
      return true;
    });
  }, [alerts, filterMetric, filterStatus]);

  /* --- Loading state --- */
  if (loading && alerts.length === 0) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="flex items-center gap-3 text-orbflow-text-ghost">
          <div className="h-5 w-5 animate-spin rounded-full border-2 border-orbflow-border border-t-electric-indigo" />
          <span className="text-sm">Loading alerts...</span>
        </div>
      </div>
    );
  }

  /* --- Error state --- */
  if (error && alerts.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="text-center">
          <div className="w-12 h-12 rounded-xl bg-red-500/10 flex items-center justify-center mx-auto mb-3">
            <NodeIcon name="alert-triangle" className="w-6 h-6 text-red-400" />
          </div>
          <p className="text-sm font-medium text-red-400 mb-1">Failed to load alerts</p>
          <p className="text-xs text-orbflow-text-ghost mb-3">{error}</p>
          <button
            onClick={() => fetchAlerts().catch(() => { /* store handles toast */ })}
            className="rounded-lg border border-orbflow-border px-4 py-2 text-sm text-orbflow-text-muted transition-colors hover:text-orbflow-text-secondary hover:border-orbflow-border-hover"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* --- Header --- */}
      <div className="flex items-center justify-between border-b border-orbflow-border px-6 py-4">
        <div>
          <h2 className="text-lg font-semibold text-orbflow-text-secondary">Alert Rules</h2>
          <p className="mt-0.5 text-xs text-orbflow-text-ghost">
            Monitor workflow metrics and get notified when thresholds are breached
          </p>
        </div>
        <button
          onClick={handleCreate}
          className="flex items-center gap-2 rounded-lg bg-electric-indigo px-4 py-2 text-sm font-medium text-white transition-opacity hover:opacity-90"
        >
          <NodeIcon name="plus" className="w-3.5 h-3.5" />
          New Alert
        </button>
      </div>

      {/* --- Content --- */}
      <div className="flex-1 overflow-y-auto p-6 space-y-5">
        {/* Summary cards */}
        {alerts.length > 0 && <AlertSummaryCards alerts={alerts} />}

        {/* Inline form */}
        {showForm && (
          <div className="rounded-xl border border-electric-indigo/20 bg-electric-indigo/[0.02] p-5 animate-scale-in">
            <AlertForm
              editingAlert={editingAlert}
              onSave={handleSave}
              onCancel={handleCancelForm}
            />
          </div>
        )}

        {alerts.length === 0 && !showForm ? (
          /* --- Empty state --- */
          <div className="flex flex-col items-center justify-center py-20 text-center animate-fade-in">
            <div className="mb-5 flex h-20 w-20 items-center justify-center rounded-2xl bg-gradient-to-br from-electric-indigo/10 to-rose-500/10 border border-orbflow-border">
              <NodeIcon name="bell" className="w-10 h-10 text-electric-indigo/60" />
            </div>
            <p className="text-sm font-semibold text-orbflow-text-secondary mb-1.5">
              No alert rules configured
            </p>
            <p className="text-xs text-orbflow-text-ghost max-w-sm leading-relaxed">
              Set up alerts to monitor failure rates, latency, and execution counts.
              Get notified via webhooks before issues impact users.
            </p>
            <button
              onClick={handleCreate}
              className="mt-5 flex items-center gap-2 rounded-lg bg-electric-indigo px-5 py-2.5 text-sm font-medium text-white transition-opacity hover:opacity-90"
            >
              <NodeIcon name="plus" className="w-4 h-4" />
              Create your first alert
            </button>
          </div>
        ) : alerts.length > 0 && (
          <>
            {/* --- Filters --- */}
            <div className="flex items-center gap-4 flex-wrap">
              {/* Metric filter */}
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-orbflow-text-ghost">Metric:</span>
                <div className="flex gap-1 rounded-lg border border-orbflow-border bg-orbflow-surface p-0.5">
                  {(["all", "failure_rate", "p95_duration", "execution_count"] as const).map((m) => (
                    <button
                      key={m}
                      onClick={() => setFilterMetric(m)}
                      className={cn(
                        "rounded-md px-2 py-0.5 text-xs font-medium transition-colors",
                        filterMetric === m
                          ? "bg-electric-indigo/20 text-electric-indigo"
                          : "text-orbflow-text-ghost hover:text-orbflow-text-secondary"
                      )}
                    >
                      {m === "all" ? "All" : METRIC_LABELS[m]}
                    </button>
                  ))}
                </div>
              </div>
              {/* Status filter */}
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-orbflow-text-ghost">Status:</span>
                <div className="flex gap-1 rounded-lg border border-orbflow-border bg-orbflow-surface p-0.5">
                  {(["all", "enabled", "disabled"] as const).map((s) => (
                    <button
                      key={s}
                      onClick={() => setFilterStatus(s)}
                      className={cn(
                        "rounded-md px-2 py-0.5 text-xs font-medium transition-colors capitalize",
                        filterStatus === s
                          ? "bg-electric-indigo/20 text-electric-indigo"
                          : "text-orbflow-text-ghost hover:text-orbflow-text-secondary"
                      )}
                    >
                      {s}
                    </button>
                  ))}
                </div>
              </div>
              {/* Count indicator */}
              <span className="text-xs text-orbflow-text-ghost tabular-nums ml-auto">
                {filteredAlerts.length} of {alerts.length} alerts
              </span>
            </div>

            {/* --- Alert cards --- */}
            <div className="space-y-3">
              {filteredAlerts.length === 0 ? (
                <p className="text-sm text-orbflow-text-ghost py-8 text-center">
                  No alerts match the current filters
                </p>
              ) : (
                filteredAlerts.map((alert) => (
                  <AlertCard
                    key={alert.id}
                    alert={alert}
                    workflowName={workflowName(alert.workflow_id)}
                    onToggle={() => toggleAlert(alert.id)}
                    onEdit={() => handleEdit(alert)}
                    onDelete={() => setConfirmDelete(alert)}
                  />
                ))
              )}
            </div>
          </>
        )}
      </div>

      {/* --- Confirm Delete --- */}
      {confirmDelete && (
        <ConfirmDialog
          title="Delete alert rule?"
          message={`The "${METRIC_LABELS[confirmDelete.metric]}" alert will be permanently deleted. You will no longer receive notifications for this threshold.`}
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
