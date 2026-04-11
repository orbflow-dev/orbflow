"use client";

import { useEffect, useCallback, useState } from "react";
import { useWorkflowStore } from "@/store/workflow-store";
import { ConfirmDialog } from "@/core/components/confirm-dialog";
import { api, type Workflow } from "@/lib/api";
import { useInstanceFilters } from "./use-instance-filters";
import { InstanceListSidebar } from "./instance-list-sidebar";
import { InstanceDetailPanel } from "./instance-detail-panel";

interface ExecutionViewerProps {
  isActive?: boolean;
}

export function ExecutionViewer({ isActive = false }: ExecutionViewerProps) {
  const {
    workflows, instances, instancesLoading,
    selectedInstance, selectInstance, cancelInstance,
    approveNode, rejectNode,
    startWorkflow, fetchInstances,
  } = useWorkflowStore();

  const {
    statusFilter, setStatusFilter, search, setSearch,
    getWorkflowName, triggerTypeMap, filteredInstances,
    groupedInstances, flatInstances, statusCounts,
  } = useInstanceFilters(instances, workflows);

  const [confirmCancel, setConfirmCancel] = useState(false);
  const [executionWorkflow, setExecutionWorkflow] = useState<Workflow | null>(null);
  const [executionWorkflowError, setExecutionWorkflowError] = useState(false);
  const [viewMode, setViewMode] = useState<"graph" | "timeline" | "duration">("graph");
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

  const fetchExecutionWorkflow = useCallback((workflowId: string) => {
    setExecutionWorkflowError(false);
    api.workflows
      .get(workflowId)
      .then(setExecutionWorkflow)
      .catch((err) => {
        console.error("[orbflow] Failed to load execution workflow:", err);
        setExecutionWorkflow(null);
        setExecutionWorkflowError(true);
      });
  }, []);

  // Fetch the workflow definition at the version recorded when the instance
  // was created, so the execution map reflects the exact definition that ran.
  // Falls back to the live workflow for older instances without a version.
  useEffect(() => {
    if (!selectedInstance) {
      setExecutionWorkflow(null);
      setExecutionWorkflowError(false);
      setSelectedNodeId(null);
      return;
    }
    setSelectedNodeId(null);
    if (selectedInstance.workflow_version != null) {
      setExecutionWorkflowError(false);
      api.versions
        .get(selectedInstance.workflow_id, selectedInstance.workflow_version)
        .then((wv) => setExecutionWorkflow(wv.definition as unknown as Workflow))
        .catch(() => {
          // Version not found — fall back to the live workflow definition
          fetchExecutionWorkflow(selectedInstance.workflow_id);
        });
    } else {
      fetchExecutionWorkflow(selectedInstance.workflow_id);
    }
  }, [selectedInstance?.id, selectedInstance?.workflow_id, selectedInstance?.workflow_version, fetchExecutionWorkflow]);

  // Poll instance list — only when tab is active AND page is visible
  useEffect(() => {
    if (!isActive) return;
    const POLL_MS = 5000;
    let id: ReturnType<typeof setInterval>;

    const start = () => { id = setInterval(fetchInstances, POLL_MS); };
    const stop = () => clearInterval(id);
    const onVisibility = () => { document.hidden ? stop() : start(); };

    if (!document.hidden) start();
    document.addEventListener("visibilitychange", onVisibility);
    return () => { stop(); document.removeEventListener("visibilitychange", onVisibility); };
  }, [isActive, fetchInstances]);

  // Auto-refresh selected instance while running — only when tab is active AND page is visible
  useEffect(() => {
    if (!isActive || !selectedInstance || selectedInstance.status !== "running") return;
    const POLL_MS = 3000;
    let id: ReturnType<typeof setInterval>;

    const poll = () => selectInstance(selectedInstance.id);
    const start = () => { id = setInterval(poll, POLL_MS); };
    const stop = () => clearInterval(id);
    const onVisibility = () => { document.hidden ? stop() : start(); };

    if (!document.hidden) start();
    document.addEventListener("visibilitychange", onVisibility);
    return () => { stop(); document.removeEventListener("visibilitychange", onVisibility); };
  }, [isActive, selectedInstance?.id, selectedInstance?.status, selectInstance]);

  // Retry fetch for failed instances missing error details
  useEffect(() => {
    if (!selectedInstance || selectedInstance.status !== "failed") return;
    const hasError = Object.values(selectedInstance.node_states || {}).some((ns) => ns.error);
    if (hasError) return;
    const timer = setTimeout(() => selectInstance(selectedInstance.id), 2000);
    return () => clearTimeout(timer);
  }, [selectedInstance?.id, selectedInstance?.status, selectedInstance?.node_states, selectInstance]);

  const handleRerun = useCallback(async () => {
    if (!selectedInstance) return;
    try { await startWorkflow(selectedInstance.workflow_id); }
    catch (err) { console.error("[orbflow] Failed to rerun workflow:", err); /* store handles toast */ }
  }, [selectedInstance, startWorkflow]);

  const handleCancelConfirmed = useCallback(async () => {
    if (selectedInstance) {
      try {
        await cancelInstance(selectedInstance.id);
      } catch {
        // Store handles toast notification
      }
    }
    setConfirmCancel(false);
  }, [selectedInstance, cancelInstance]);

  const handleRetryWorkflowFetch = useCallback(() => {
    if (!selectedInstance) return;
    if (selectedInstance.workflow_version != null) {
      setExecutionWorkflowError(false);
      api.versions
        .get(selectedInstance.workflow_id, selectedInstance.workflow_version)
        .then((wv) => setExecutionWorkflow(wv.definition as unknown as Workflow))
        .catch(() => {
          // Version not found — fall back to the live workflow definition
          fetchExecutionWorkflow(selectedInstance.workflow_id);
        });
    } else {
      fetchExecutionWorkflow(selectedInstance.workflow_id);
    }
  }, [selectedInstance, fetchExecutionWorkflow]);

  const canRerun = !!selectedInstance && ["completed", "failed", "cancelled"].includes(selectedInstance.status);
  const isRunning = selectedInstance?.status === "running";

  return (
    <div className="activity-shell flex h-full min-h-0 flex-col overflow-hidden xl:flex-row">
      <InstanceListSidebar
        instances={instances}
        instancesLoading={instancesLoading}
        selectedInstance={selectedInstance}
        filteredInstances={filteredInstances}
        groupedInstances={groupedInstances}
        flatInstances={flatInstances}
        statusFilter={statusFilter}
        setStatusFilter={setStatusFilter}
        search={search}
        setSearch={setSearch}
        statusCounts={statusCounts}
        getWorkflowName={getWorkflowName}
        triggerTypeMap={triggerTypeMap}
        onSelectInstance={selectInstance}
      />

      <div className="activity-detail-shell flex-1 min-h-0 flex flex-col overflow-hidden bg-orbflow-bg">
        <InstanceDetailPanel
          selectedInstance={selectedInstance}
          executionWorkflow={executionWorkflow}
          executionWorkflowError={executionWorkflowError}
          isActive={isActive}
          viewMode={viewMode}
          setViewMode={setViewMode}
          selectedNodeId={selectedNodeId}
          setSelectedNodeId={setSelectedNodeId}
          getWorkflowName={getWorkflowName}
          isRunning={!!isRunning}
          canRerun={canRerun}
          onRerun={handleRerun}
          onCancel={() => setConfirmCancel(true)}
          onRetryWorkflowFetch={handleRetryWorkflowFetch}
          onApprove={approveNode}
          onReject={rejectNode}
        />
      </div>

      {confirmCancel && (
        <ConfirmDialog
          title="Cancel this run?"
          message="The workflow execution will be stopped. Any in-progress steps will be terminated."
          confirmLabel="Cancel Run"
          variant="danger"
          onConfirm={handleCancelConfirmed}
          onCancel={() => setConfirmCancel(false)}
        />
      )}
    </div>
  );
}
