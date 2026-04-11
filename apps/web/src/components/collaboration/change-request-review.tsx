"use client";

import { memo, useState, useEffect, useCallback, useMemo } from "react";
import { useChangeRequestStore } from "@orbflow/core/stores";
import { api } from "@/lib/api";
import { computeDiffFromDefinitions } from "./diff-utils";
import { Button } from "@/core/components/button";
import { ConfirmDialog } from "@/core/components/confirm-dialog";
import { WorkflowDiff } from "./workflow-diff";
import { CommentThread } from "./comment-thread";
import { ActivityTimeline } from "./activity-timeline";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { StatusBadge, initials } from "./shared";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ChangeRequestReviewProps {
  workflowId: string;
  changeRequestId: string;
  onBack: () => void;
}

type ReviewTab = "diff" | "comments" | "activity";

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const ChangeRequestReview = memo(function ChangeRequestReview({
  workflowId,
  changeRequestId,
  onBack,
}: ChangeRequestReviewProps) {
  const store = useChangeRequestStore();
  const cr = store.selectedCR;

  const [activeTab, setActiveTab] = useState<ReviewTab>("diff");
  const [showConfirm, setShowConfirm] = useState<"merge" | "reject" | null>(null);
  const [currentVersion, setCurrentVersion] = useState<number | null>(null);
  const [rebasing, setRebasing] = useState(false);

  // -----------------------------------------------------------------------
  // Fetch CR on mount
  // -----------------------------------------------------------------------

  useEffect(() => {
    store.selectChangeRequest(workflowId, changeRequestId);
    return () => store.clearSelection();
  }, [workflowId, changeRequestId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Fetch current workflow version to detect staleness
  useEffect(() => {
    api.workflows
      .get(workflowId)
      .then((wf) => setCurrentVersion(wf.version))
      .catch(() => setCurrentVersion(null));
  }, [workflowId]);

  // -----------------------------------------------------------------------
  // Action handlers
  // -----------------------------------------------------------------------

  const handleSubmit = useCallback(async () => {
    await store.submitCR(workflowId, changeRequestId);
  }, [store, workflowId, changeRequestId]);

  const handleApprove = useCallback(async () => {
    await store.approveCR(workflowId, changeRequestId);
  }, [store, workflowId, changeRequestId]);

  const handleReject = useCallback(async () => {
    setShowConfirm(null);
    await store.rejectCR(workflowId, changeRequestId);
  }, [store, workflowId, changeRequestId]);

  const handleMerge = useCallback(async () => {
    setShowConfirm(null);
    await store.mergeCR(workflowId, changeRequestId);
  }, [store, workflowId, changeRequestId]);

  const handleRebase = useCallback(async () => {
    setRebasing(true);
    try {
      await store.rebaseCR(workflowId, changeRequestId);
      // Refresh the current workflow version after rebase
      const wf = await api.workflows.get(workflowId);
      setCurrentVersion(wf.version);
    } finally {
      setRebasing(false);
    }
  }, [store, workflowId, changeRequestId]);

  // -----------------------------------------------------------------------
  // Fetch base version definition for diff
  // -----------------------------------------------------------------------

  // null = not yet loaded, {} = loaded but empty (version not found)
  const [baseDef, setBaseDef] = useState<Record<string, unknown> | null>(null);

  const baseVersion = cr?.base_version;

  useEffect(() => {
    if (baseVersion === undefined) {
      setBaseDef(null);
      return;
    }
    setBaseDef(null); // Reset while fetching new version
    api.versions
      .get(workflowId, baseVersion)
      .then((version) => setBaseDef(version.definition))
      .catch(() => {
        // If version not found, use empty -- the diff will show all nodes as "added"
        setBaseDef({});
      });
  }, [baseVersion, workflowId]);

  const proposedDef = useMemo(
    () => cr?.proposed_definition ?? {},
    [cr],
  );

  // Compute diff client-side from the two definitions
  const diff = useMemo(
    () => computeDiffFromDefinitions(baseDef ?? {}, proposedDef, cr?.base_version ?? 0),
    [baseDef, proposedDef, cr?.base_version],
  );

  // -----------------------------------------------------------------------
  // Comment stats
  // -----------------------------------------------------------------------

  // Detect if the CR's base version is behind the current workflow version
  const isStale =
    currentVersion !== null &&
    cr !== null &&
    cr.base_version < currentVersion &&
    !["merged", "rejected"].includes(cr.status);

  const commentCount = cr?.comments?.length ?? 0;
  const unresolvedCount =
    cr?.comments?.filter((c) => !c.resolved).length ?? 0;

  // -----------------------------------------------------------------------
  // Loading state
  // -----------------------------------------------------------------------

  if (store.loading || !cr) {
    return (
      <div className="flex flex-col items-center justify-center h-64 gap-3 animate-fade-in">
        <div className="w-5 h-5 border-2 border-electric-indigo/30 border-t-electric-indigo rounded-full animate-spin" />
        <p className="text-body-sm text-orbflow-text-muted">Loading change request...</p>
      </div>
    );
  }

  // -----------------------------------------------------------------------
  // Tab definitions
  // -----------------------------------------------------------------------

  const tabs: { id: ReviewTab; label: string }[] = [
    { id: "diff", label: "Diff View" },
    { id: "comments", label: `Comments (${commentCount})` },
    { id: "activity", label: "Activity" },
  ];

  // -----------------------------------------------------------------------
  // Render
  // -----------------------------------------------------------------------

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="px-6 py-4 border-b border-orbflow-border animate-fade-in">
        <button
          onClick={onBack}
          className="text-body-sm text-orbflow-text-muted hover:text-orbflow-text mb-2 flex items-center gap-1.5 transition-colors
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none rounded-md px-1 -ml-1"
        >
          <NodeIcon name="arrow-left" className="w-3.5 h-3.5" />
          Back to list
        </button>

        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold text-orbflow-text flex-1 truncate">
            {cr.title}
          </h2>
          <StatusBadge status={cr.status} />
        </div>

        <div className="flex items-center gap-3 mt-1.5 text-body-sm text-orbflow-text-ghost">
          <span className="flex items-center gap-1.5">
            <span className="w-5 h-5 rounded-full bg-electric-indigo/10 text-electric-indigo text-[9px] font-bold flex items-center justify-center shrink-0">
              {initials(cr.author)}
            </span>
            {cr.author}
          </span>
          <span className="text-orbflow-text-ghost/40">&middot;</span>
          <span>v{cr.base_version}</span>
          <span className="text-orbflow-text-ghost/40">&middot;</span>
          <span className="flex items-center gap-1">
            <NodeIcon name="message-circle" className="w-3 h-3" />
            {commentCount}
          </span>
          {unresolvedCount > 0 && (
            <span className="text-amber-400 text-body-sm font-medium">
              {unresolvedCount} unresolved
            </span>
          )}
        </div>
      </div>

      {/* Stale base version banner */}
      {isStale && (
        <div className="flex items-center gap-3 px-6 py-2.5 bg-amber-500/10 border-b border-amber-500/20 animate-fade-in">
          <NodeIcon name="alert-triangle" className="w-4 h-4 text-amber-400 shrink-0" />
          <span className="text-body-sm text-amber-300 flex-1">
            Base version is outdated (v{cr.base_version} → v{currentVersion}).
            Rebase to update the diff against the latest workflow.
          </span>
          <Button
            variant="secondary"
            onClick={handleRebase}
            disabled={rebasing}
          >
            {rebasing ? (
              <>
                <div className="w-3 h-3 border-2 border-amber-400/30 border-t-amber-400 rounded-full animate-spin" />
                Rebasing...
              </>
            ) : (
              <>
                <NodeIcon name="git-branch" className="w-3.5 h-3.5" />
                Rebase to v{currentVersion}
              </>
            )}
          </Button>
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-1 px-6 py-2 border-b border-orbflow-border">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={cn(
              "text-body px-3 py-1.5 rounded-md transition-colors",
              "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
              activeTab === tab.id
                ? "bg-electric-indigo/15 text-electric-indigo font-medium"
                : "text-orbflow-text-muted hover:text-orbflow-text hover:bg-orbflow-surface-hover",
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {activeTab === "diff" && (
          <div className="h-full p-4">
            {baseDef !== null ? (
              <WorkflowDiff
                baseDefinition={baseDef}
                proposedDefinition={proposedDef}
                diff={diff}
                comments={cr.comments ?? []}
              />
            ) : (
              <div className="flex items-center justify-center h-full gap-3">
                <div className="w-5 h-5 border-2 border-electric-indigo/30 border-t-electric-indigo rounded-full animate-spin" />
                <span className="text-body-sm text-orbflow-text-muted">Loading diff...</span>
              </div>
            )}
          </div>
        )}

        {activeTab === "comments" && (
          <CommentThread
            workflowId={workflowId}
            changeRequestId={changeRequestId}
            comments={cr.comments ?? []}
          />
        )}

        {activeTab === "activity" && (
          <ActivityTimeline changeRequest={cr} />
        )}
      </div>

      {/* Action buttons */}
      <div className="flex items-center justify-end gap-3 px-6 py-3 border-t border-orbflow-border">
        {cr.status === "draft" && (
          <Button variant="primary" onClick={handleSubmit}>
            Submit for Review
          </Button>
        )}

        {cr.status === "open" && (
          <>
            <Button
              variant="danger"
              onClick={() => setShowConfirm("reject")}
            >
              Reject
            </Button>
            <Button variant="primary" onClick={handleApprove}>
              Approve
            </Button>
          </>
        )}

        {cr.status === "approved" && (
          <Button
            variant="primary"
            onClick={() => setShowConfirm("merge")}
          >
            <NodeIcon name="git-merge" className="w-3.5 h-3.5" />
            Merge
          </Button>
        )}

        {cr.status === "merged" && (
          <span className="inline-flex items-center gap-1.5 text-body-sm text-purple-400">
            <NodeIcon name="git-merge" className="w-3.5 h-3.5" />
            This change request has been merged.
          </span>
        )}

        {cr.status === "rejected" && (
          <span className="inline-flex items-center gap-1.5 text-body-sm text-red-400">
            <NodeIcon name="x" className="w-3.5 h-3.5" />
            This change request was rejected.
          </span>
        )}
      </div>

      {/* Confirm dialogs */}
      {showConfirm === "merge" && (
        <ConfirmDialog
          title="Merge Change Request"
          message="Are you sure you want to merge? This will update the workflow to the proposed definition."
          confirmLabel="Merge"
          onConfirm={handleMerge}
          onCancel={() => setShowConfirm(null)}
        />
      )}

      {showConfirm === "reject" && (
        <ConfirmDialog
          title="Reject Change Request"
          message="Reject this change request? The author will be notified."
          confirmLabel="Reject"
          variant="danger"
          onConfirm={handleReject}
          onCancel={() => setShowConfirm(null)}
        />
      )}
    </div>
  );
});
