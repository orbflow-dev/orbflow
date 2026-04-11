"use client";

import { useEffect, useState, useCallback, useMemo, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import dynamic from "next/dynamic";
import { useWorkflowStore } from "@/store/workflow-store";
import { useCanvasStore } from "@/store/canvas-store";
import { OrbflowProvider, type OrbflowConfig } from "@/core/context/orbflow-provider";
import { WorkflowBuilder } from "@/components/workflow-builder/workflow-builder";
import { ExecutionViewer } from "@/components/execution-viewer/execution-viewer";
import type { Template } from "@/components/template-gallery/template-gallery";
import { NodeIcon } from "@/core/components/icons";

/* Lazy-loaded tabs — reduces initial JS bundle and defers side-effect fetches */
const TemplateGallery = dynamic(() => import("@/components/template-gallery/template-gallery").then((m) => ({ default: m.TemplateGallery })));
const CredentialManager = dynamic(() => import("@/components/credential-manager/credential-manager").then((m) => ({ default: m.CredentialManager })));
const Marketplace = dynamic(() => import("@/components/marketplace/marketplace").then((m) => ({ default: m.Marketplace })));
const CollaborationPage = dynamic(() => import("@/components/collaboration/collaboration-page").then((m) => ({ default: m.CollaborationPage })));
const BudgetManager = dynamic(() => import("@/components/budget-manager").then((m) => ({ default: m.BudgetManager })));
const AlertManager = dynamic(() => import("@/components/alert-manager").then((m) => ({ default: m.AlertManager })));
const RbacEditor = dynamic(() => import("@/components/rbac").then((m) => ({ default: m.RbacEditor })));
const WorkflowAnalytics = dynamic(() => import("@/components/analytics/workflow-analytics").then((m) => ({ default: m.WorkflowAnalytics })));
import { ConfirmDialog } from "@/core/components/confirm-dialog";
import { ToastContainer } from "@/core/components/toast";
import { ErrorBoundary } from "@/core/components/error-boundary";
import { ThemeProvider, useTheme } from "@/core/context/theme-provider";
import { api, BASE_URL, API_ROOT } from "@/lib/api";
import "@/store/change-request-store"; // side-effect: initializes CR store with API client
import "@/store/budget-store"; // side-effect: initializes budget store with API client
import "@/store/alert-store"; // side-effect: initializes alert store with API client
import { useNodeOutputCacheStore } from "@/store/node-output-cache-store";
import { cn } from "@/lib/cn";

type Tab = "builder" | "executions" | "analytics" | "templates" | "credentials" | "marketplace" | "reviews" | "budgets" | "alerts" | "rbac";

const NAV_ITEMS: { id: Tab; label: string; icon: string; description: string }[] = [
  { id: "builder", label: "Builder", icon: "workflow", description: "Create automations" },
  { id: "executions", label: "Activity", icon: "play", description: "Monitor runs" },
  { id: "analytics", label: "Analytics", icon: "bar-chart", description: "Execution metrics & SLA" },
  { id: "templates", label: "Templates", icon: "layers", description: "Start from a template" },
  { id: "credentials", label: "Credentials", icon: "key", description: "Manage secrets & API keys" },
  { id: "marketplace", label: "Marketplace", icon: "plug", description: "Browse plugins & integrations" },
  { id: "reviews", label: "Reviews", icon: "git-pull-request", description: "Collaborative change requests" },
  { id: "budgets", label: "Budgets", icon: "wallet", description: "Cost tracking & budget limits" },
  { id: "alerts", label: "Alerts", icon: "bell", description: "Monitoring & alert rules" },
  { id: "rbac", label: "Access", icon: "shield", description: "Roles & permissions" },
];

type HealthStatus = "online" | "offline" | "connecting";

function ThemeToggle({ collapsed }: { collapsed: boolean }) {
  const { mode, toggleTheme } = useTheme();
  return (
    <button
      onClick={toggleTheme}
      className={`mt-2 flex items-center gap-2 rounded-lg transition-all duration-200
        hover:bg-orbflow-surface-hover focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none
        ${collapsed ? "justify-center px-2 py-2" : "px-3 py-2 w-full"}`}
      title={mode === "dark" ? "Switch to light mode" : "Switch to dark mode"}
      aria-label={mode === "dark" ? "Switch to light mode" : "Switch to dark mode"}
    >
      <NodeIcon
        name={mode === "dark" ? "sun" : "moon"}
        className="w-3.5 h-3.5 shrink-0 text-orbflow-text-muted"
      />
      {!collapsed && (
        <span className="text-body font-medium text-orbflow-text-muted">
          {mode === "dark" ? "Light Mode" : "Dark Mode"}
        </span>
      )}
    </button>
  );
}

function AnalyticsPage() {
  const workflows = useWorkflowStore((s) => s.workflows);
  const [selectedWorkflowId, setSelectedWorkflowId] = useState<string | null>(null);
  const hasAutoSelected = useRef(false);

  // Auto-select first workflow once when workflows first load.
  // Uses a ref so manual selection changes are never overridden.
  useEffect(() => {
    if (hasAutoSelected.current || workflows.length === 0) return;
    setSelectedWorkflowId(workflows[0].id);
    hasAutoSelected.current = true;
  }, [workflows.length]);

  // If the selected workflow was deleted, fall back to the first available
  useEffect(() => {
    if (!selectedWorkflowId || workflows.length === 0) return;
    if (!workflows.some((wf) => wf.id === selectedWorkflowId)) {
      setSelectedWorkflowId(workflows[0].id);
    }
  }, [workflows, selectedWorkflowId]);

  const handleWorkflowChange = useCallback((e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    setSelectedWorkflowId(value || null);
    // Mark as user-interacted so auto-select never overrides
    hasAutoSelected.current = true;
  }, []);

  return (
    <div className="h-full p-6 space-y-5 overflow-auto">
      {/* Header */}
      <div className="flex items-center justify-between animate-fade-in-up">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-electric-indigo/10 flex items-center justify-center shadow-[inset_0_1px_0_0_rgba(124,92,252,0.15)]">
            <NodeIcon name="bar-chart" className="w-4 h-4 text-electric-indigo" />
          </div>
          <div>
            <h2 className="text-sm font-semibold text-orbflow-text-secondary">Analytics</h2>
            <p className="text-[10px] text-orbflow-text-ghost">Execution metrics & SLA</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <label htmlFor="analytics-workflow-select" className="sr-only">Select workflow</label>
          <select
            id="analytics-workflow-select"
            value={selectedWorkflowId ?? ""}
            onChange={handleWorkflowChange}
            className="rounded-lg border border-orbflow-border bg-orbflow-surface px-3 py-1.5 text-xs text-orbflow-text-secondary
              min-w-[200px] focus:outline-none focus:ring-2 focus:ring-electric-indigo/50
              hover:border-orbflow-border-hover transition-colors cursor-pointer"
          >
            <option value="">Select a workflow...</option>
            {workflows.map((wf) => (
              <option key={wf.id} value={wf.id}>{wf.name || wf.id}</option>
            ))}
          </select>
        </div>
      </div>

      {/* Content -- key forces clean remount when selection changes */}
      {selectedWorkflowId ? (
        <WorkflowAnalytics key={selectedWorkflowId} workflowId={selectedWorkflowId} />
      ) : (
        <div className="flex flex-col items-center justify-center py-20 animate-fade-in">
          <div className="relative mb-5">
            <div className="w-16 h-16 rounded-2xl bg-orbflow-surface-hover flex items-center justify-center">
              <NodeIcon name="bar-chart" className="w-8 h-8 text-orbflow-text-ghost" />
            </div>
          </div>
          <p className="text-sm font-medium text-orbflow-text-muted mb-1">Select a workflow</p>
          <p className="text-xs text-orbflow-text-ghost">Choose a workflow above to view execution metrics</p>
        </div>
      )}
    </div>
  );
}

export default function Home() {
  const [tab, setTab] = useState<Tab>("builder");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [healthStatus, setHealthStatus] = useState<HealthStatus>("connecting");
  const [currentDate, setCurrentDate] = useState("");
  const { fetchWorkflows, fetchInstances, createWorkflow, selectWorkflow } = useWorkflowStore(
    useShallow((s) => ({
      fetchWorkflows: s.fetchWorkflows,
      fetchInstances: s.fetchInstances,
      createWorkflow: s.createWorkflow,
      selectWorkflow: s.selectWorkflow,
    })),
  );
  const [confirmDialog, setConfirmDialog] = useState<{ template: Template } | null>(null);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const handleTabKeyDown = useCallback((e: React.KeyboardEvent, index: number) => {
    const total = NAV_ITEMS.length;
    let nextIndex: number | null = null;

    if (e.key === "ArrowDown") {
      nextIndex = (index + 1) % total;
    } else if (e.key === "ArrowUp") {
      nextIndex = (index - 1 + total) % total;
    } else if (e.key === "Home") {
      nextIndex = 0;
    } else if (e.key === "End") {
      nextIndex = total - 1;
    }

    if (nextIndex !== null) {
      e.preventDefault();
      setTab(NAV_ITEMS[nextIndex].id);
      tabRefs.current[nextIndex]?.focus();
    }
  }, []);

  const applyTemplate = useCallback(async (template: Template) => {
    try {
      const created = await createWorkflow({
        name: template.name,
        description: template.description,
        nodes: template.nodes,
        edges: template.edges,
      });
      await selectWorkflow(created.id);
      setConfirmDialog(null);
      setTab("builder");
    } catch {
      // store shows a toast — leave confirmDialog open so user can retry
    }
  }, [createWorkflow, selectWorkflow]);

  const handleUseTemplate = useCallback((template: Template) => {
    const { nodes } = useCanvasStore.getState();
    if (nodes.length > 0) {
      setConfirmDialog({ template });
      return;
    }
    applyTemplate(template);
  }, [applyTemplate]);

  // Responsive: auto-collapse sidebar on narrow screens
  useEffect(() => {
    const mq = window.matchMedia("(max-width: 768px)");
    if (mq.matches) setSidebarCollapsed(true);
    const handler = (e: MediaQueryListEvent) => setSidebarCollapsed(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  // Health check ping
  useEffect(() => {
    const checkHealth = async () => {
      try {
        const res = await fetch(`${API_ROOT}/health`, { signal: AbortSignal.timeout(5000) });
        setHealthStatus(res.ok ? "online" : "offline");
      } catch {
        setHealthStatus("offline");
      }
    };
    checkHealth();
    const interval = setInterval(checkHealth, 30000);
    return () => clearInterval(interval);
  }, []);

  // Live date
  useEffect(() => {
    const updateDate = () =>
      setCurrentDate(
        new Date().toLocaleDateString("en-US", {
          weekday: "short",
          month: "short",
          day: "numeric",
        })
      );
    updateDate();
    const interval = setInterval(updateDate, 60000);
    return () => clearInterval(interval);
  }, []);

  const orbflowConfig: OrbflowConfig = useMemo(
    () => ({
      apiBaseUrl: BASE_URL,
      onSave: async (wf) => {
        if (wf.id) {
          const updated = await api.workflows.update(wf.id, wf);
          return updated;
        }
        const created = await api.workflows.create(wf);
        // Refresh the workflow list so new workflow appears in dropdown
        fetchWorkflows().catch(() => { /* store handles toast */ });
        // Auto-select the newly created workflow (fire-and-forget to avoid remounting mid-save)
        selectWorkflow(created.id);
        return created;
      },
      onRun: async (wf) => {
        if (!wf.id) return;
        const inst = await api.workflows.start(wf.id);
        if (!inst?.id) throw new Error("Backend did not return an instance ID");
        // Refresh instances list
        fetchInstances().catch(() => { /* store handles toast */ });
        return inst.id;
      },
      onTestNode: async (wf, nodeId) => {
        if (!wf.id) return;
        const cachedOutputs = useNodeOutputCacheStore.getState().getWorkflowCache(wf.id);
        const result = await api.workflows.testNode(wf.id, { nodeId, cachedOutputs });
        // Cache all outputs from the test run
        const outputs: Record<string, Record<string, unknown>> = {};
        for (const [nid, ns] of Object.entries(result.node_outputs)) {
          if (ns.output) outputs[nid] = ns.output;
        }
        if (Object.keys(outputs).length > 0) {
          useNodeOutputCacheStore.getState().mergeBulk(wf.id, outputs);
        }
        return result;
      },
    }),
    [fetchWorkflows, fetchInstances, selectWorkflow]
  );

  useEffect(() => {
    fetchWorkflows().catch(() => { /* store handles toast */ });
    fetchInstances().catch(() => { /* store handles toast */ });
  }, [fetchWorkflows, fetchInstances]);

  return (
    <ThemeProvider>
    <OrbflowProvider config={orbflowConfig}>
      <div className="flex h-screen overflow-hidden bg-orbflow-bg">
        {/* Sidebar */}
        <aside
          className={`border-r border-orbflow-border bg-orbflow-bg flex flex-col shrink-0
            transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]
            ${sidebarCollapsed ? "w-[68px]" : "w-60"}`}
        >
          {/* Logo */}
          <div className={`flex items-center gap-3 p-4 ${sidebarCollapsed ? "justify-center" : ""}`}>
            <button
              onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
              aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
              className="w-9 h-9 rounded-xl bg-gradient-to-br from-electric-indigo to-[#5B4CD6] flex items-center justify-center
                text-white font-bold text-sm shadow-lg shadow-indigo-500/20 shrink-0
                hover:shadow-indigo-500/30 transition-shadow
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            >
              <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <ellipse cx="12" cy="12" rx="10" ry="4" />
                <ellipse cx="12" cy="12" rx="10" ry="4" transform="rotate(60 12 12)" />
                <ellipse cx="12" cy="12" rx="10" ry="4" transform="rotate(-60 12 12)" />
                <circle cx="12" cy="12" r="2.5" fill="currentColor" stroke="none" />
              </svg>
            </button>
            {!sidebarCollapsed && (
              <div className="animate-fade-in">
                <h1 className="text-display font-bold tracking-tight text-orbflow-text-secondary">
                  Orbflow
                </h1>
                <p className="text-caption text-orbflow-text-ghost font-medium">
                  Workflow Automation
                </p>
              </div>
            )}
          </div>

          {/* Nav */}
          <nav className="flex-1 px-2.5 py-3 space-y-1" role="tablist" aria-label="Main navigation" aria-orientation="vertical">
            {NAV_ITEMS.map((item, i) => (
              <button
                key={item.id}
                ref={(el) => { tabRefs.current[i] = el; }}
                role="tab"
                id={`tab-${item.id}`}
                aria-selected={tab === item.id}
                aria-controls={`tabpanel-${item.id}`}
                tabIndex={tab === item.id ? 0 : -1}
                onClick={() => setTab(item.id)}
                onKeyDown={(e) => handleTabKeyDown(e, i)}
                className={cn(
                  "w-full flex items-center gap-3 rounded-xl text-left transition-all duration-200",
                  sidebarCollapsed ? "justify-center px-2 py-2.5" : "px-3 py-2.5",
                  tab === item.id
                    ? "bg-electric-indigo/10 text-electric-indigo"
                    : "hover:bg-orbflow-surface-hover text-orbflow-text-muted",
                )}
                title={sidebarCollapsed ? item.label : undefined}
              >
                <NodeIcon
                  name={item.icon}
                  className={`w-4 h-4 shrink-0 ${
                    tab === item.id ? "text-electric-indigo" : ""
                  }`}
                />
                {!sidebarCollapsed && (
                  <div className="min-w-0 flex-1">
                    <div className="text-heading font-medium truncate">
                      {item.label}
                    </div>
                  </div>
                )}
              </button>
            ))}
          </nav>

          {/* Footer */}
          <div className="p-3 border-t border-orbflow-border">
            <div className={`flex items-center ${sidebarCollapsed ? "justify-center" : "gap-2.5 px-2 py-1.5"} rounded-lg`}>
              <div
                role="status"
                aria-label={
                  healthStatus === "online" ? "Engine online"
                    : healthStatus === "connecting" ? "Connecting to engine"
                    : "Engine offline"
                }
                className={`w-2 h-2 rounded-full shrink-0 ${
                  healthStatus === "online" ? "bg-neon-cyan animate-pulse-soft"
                    : healthStatus === "connecting" ? "bg-amber-400 animate-pulse-soft"
                    : "bg-rose-400"
                }`}
              />
              {!sidebarCollapsed && (
                <>
                  <span className="text-body-sm font-medium text-orbflow-text-faint">
                    {healthStatus === "online" ? "Engine Online"
                      : healthStatus === "connecting" ? "Connecting..."
                      : "Engine Offline"}
                  </span>
                  <span className="text-caption font-mono ml-auto text-orbflow-text-ghost">
                    v0.5
                  </span>
                </>
              )}
            </div>
            <ThemeToggle collapsed={sidebarCollapsed} />
          </div>
        </aside>

        {/* Main */}
        <main className="flex-1 flex flex-col relative overflow-hidden">
          {/* Top bar */}
          <header
            className="h-12 flex items-center justify-between px-5 border-b border-orbflow-border backdrop-blur-sm z-10 shrink-0"
            style={{ background: "color-mix(in srgb, var(--orbflow-bg) 80%, transparent)" }}
          >
            <div className="flex items-center gap-2.5">
              <span className="text-body font-medium text-orbflow-text-faint">
                {NAV_ITEMS.find((t) => t.id === tab)?.description}
              </span>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-caption font-mono text-orbflow-text-ghost">
                {currentDate}
              </span>
            </div>
          </header>

          {/* Content -- all tabs stay mounted, hidden via CSS to preserve state */}
          <div className="flex-1 overflow-hidden relative">
            <div id="tabpanel-builder" role="tabpanel" aria-labelledby="tab-builder" className={`absolute inset-0 ${tab === "builder" ? "" : "hidden"}`}>
              <ErrorBoundary section="Builder">
                <WorkflowBuilder />
              </ErrorBoundary>
            </div>
            <div id="tabpanel-executions" role="tabpanel" aria-labelledby="tab-executions" className={`absolute inset-0 ${tab === "executions" ? "" : "hidden"}`}>
              <ErrorBoundary section="Activity">
                <ExecutionViewer isActive={tab === "executions"} />
              </ErrorBoundary>
            </div>
            {tab === "analytics" && (
              <div id="tabpanel-analytics" role="tabpanel" aria-labelledby="tab-analytics" className="absolute inset-0 overflow-auto">
                <ErrorBoundary section="Analytics">
                  <AnalyticsPage />
                </ErrorBoundary>
              </div>
            )}
            {tab === "templates" && (
              <div id="tabpanel-templates" role="tabpanel" aria-labelledby="tab-templates" className="absolute inset-0">
                <ErrorBoundary section="Templates">
                  <TemplateGallery onUseTemplate={handleUseTemplate} />
                </ErrorBoundary>
              </div>
            )}
            {tab === "credentials" && (
              <div id="tabpanel-credentials" role="tabpanel" aria-labelledby="tab-credentials" className="absolute inset-0">
                <ErrorBoundary section="Credentials">
                  <CredentialManager />
                </ErrorBoundary>
              </div>
            )}
            {tab === "marketplace" && (
              <div id="tabpanel-marketplace" role="tabpanel" aria-labelledby="tab-marketplace" className="absolute inset-0">
                <ErrorBoundary section="Marketplace">
                  <Marketplace />
                </ErrorBoundary>
              </div>
            )}
            {tab === "reviews" && (
              <div id="tabpanel-reviews" role="tabpanel" aria-labelledby="tab-reviews" className="absolute inset-0">
                <ErrorBoundary section="Reviews">
                  <CollaborationPage />
                </ErrorBoundary>
              </div>
            )}
            {tab === "budgets" && (
              <div id="tabpanel-budgets" role="tabpanel" aria-labelledby="tab-budgets" className="absolute inset-0 overflow-y-auto">
                <div className="px-4 py-3">
                  <ErrorBoundary section="Budgets">
                    <BudgetManager />
                  </ErrorBoundary>
                </div>
              </div>
            )}
            {tab === "alerts" && (
              <div id="tabpanel-alerts" role="tabpanel" aria-labelledby="tab-alerts" className="absolute inset-0 overflow-y-auto">
                <ErrorBoundary section="Alerts">
                  <AlertManager />
                </ErrorBoundary>
              </div>
            )}
            {tab === "rbac" && (
              <div id="tabpanel-rbac" role="tabpanel" aria-labelledby="tab-rbac" className="absolute inset-0 overflow-y-auto">
                <ErrorBoundary section="Access Control">
                  <RbacEditor />
                </ErrorBoundary>
              </div>
            )}
          </div>
        </main>
      </div>
      {confirmDialog && (
        <ConfirmDialog
          title="Replace current workflow?"
          message="The Builder has an existing workflow. Using this template will replace it with a new workflow."
          confirmLabel="Replace & Continue"
          cancelLabel="Cancel"
          variant="danger"
          onConfirm={() => applyTemplate(confirmDialog.template)}
          onCancel={() => setConfirmDialog(null)}
        />
      )}
      <ToastContainer />
    </OrbflowProvider>
    </ThemeProvider>
  );
}
