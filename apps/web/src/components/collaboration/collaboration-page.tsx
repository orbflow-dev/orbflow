"use client";

import { useState, useCallback, useEffect, useRef } from "react";
import { useWorkflowStore } from "@/store/workflow-store";
import { useChangeRequestStore } from "@/store/change-request-store";
import { NodeIcon } from "@/core/components/icons";
import { ChangeRequestList } from "./change-request-list";
import { ChangeRequestCreate } from "./change-request-create";
import { ChangeRequestReview } from "./change-request-review";

type View =
  | { type: "list" }
  | { type: "create" }
  | { type: "review"; crId: string };

export function CollaborationPage() {
  const selectedWorkflow = useWorkflowStore((s) => s.selectedWorkflow);
  const { clearSelection } = useChangeRequestStore();
  const [view, setView] = useState<View>({ type: "list" });

  // Reset view and clear stale selection when workflow changes
  const prevWorkflowId = useRef(selectedWorkflow?.id);
  useEffect(() => {
    if (prevWorkflowId.current !== selectedWorkflow?.id) {
      setView({ type: "list" });
      clearSelection();
      prevWorkflowId.current = selectedWorkflow?.id;
    }
  }, [selectedWorkflow?.id, clearSelection]);

  // Cleanup on unmount
  useEffect(() => () => clearSelection(), [clearSelection]);

  const handleSelect = useCallback((crId: string) => {
    setView({ type: "review", crId });
  }, []);

  const handleCreate = useCallback(() => {
    setView({ type: "create" });
  }, []);

  const handleBack = useCallback(() => {
    setView({ type: "list" });
  }, []);

  const handleCreated = useCallback((crId: string) => {
    setView({ type: "review", crId });
  }, []);

  if (!selectedWorkflow) {
    return (
      <div className="flex flex-col items-center justify-center h-full py-20 animate-fade-in">
        <div className="w-16 h-16 rounded-2xl bg-electric-indigo/5 flex items-center justify-center mb-4 animate-fade-in-up stagger-1">
          <NodeIcon name="git-pull-request" className="w-7 h-7 text-electric-indigo/30" />
        </div>
        <h3 className="text-body-lg font-medium text-orbflow-text-muted mb-1 animate-fade-in-up stagger-2">
          Change Requests
        </h3>
        <p className="text-body text-orbflow-text-ghost text-center max-w-xs animate-fade-in-up stagger-3">
          Select a workflow from the Builder tab to propose and review changes collaboratively.
        </p>
      </div>
    );
  }

  return (
    <div className="h-full overflow-hidden">
      {view.type === "list" && (
        <ChangeRequestList
          workflowId={selectedWorkflow.id}
          onSelect={handleSelect}
          onCreate={handleCreate}
        />
      )}
      {view.type === "create" && (
        <ChangeRequestCreate
          workflowId={selectedWorkflow.id}
          onClose={handleBack}
          onCreated={handleCreated}
        />
      )}
      {view.type === "review" && (
        <ChangeRequestReview
          workflowId={selectedWorkflow.id}
          changeRequestId={view.crId}
          onBack={handleBack}
        />
      )}
    </div>
  );
}
