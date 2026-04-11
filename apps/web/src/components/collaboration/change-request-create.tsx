"use client";

import { useState, useCallback, useRef, useEffect } from "react";
import { useChangeRequestStore, useCanvasStore } from "@orbflow/core/stores";
import { useWorkflowStore } from "@/store/workflow-store";
import type { CreateChangeRequestInput } from "@orbflow/core/types";
import { Button } from "@/core/components/button";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";

/* =======================================================
   Props
   ======================================================= */

interface ChangeRequestCreateProps {
  workflowId: string;
  onClose: () => void;
  onCreated: (crId: string) => void;
}

/* =======================================================
   Shared styles
   ======================================================= */

const inputCls = cn(
  "w-full px-3 py-2 rounded-lg text-sm text-orbflow-text-secondary",
  "bg-orbflow-surface border border-orbflow-border transition-colors",
  "placeholder:text-orbflow-text-ghost/50",
  "focus:outline-none focus:ring-1 focus:ring-electric-indigo/30 focus:border-electric-indigo/60",
);

const inputErrorCls = "border-red-500/60 focus:ring-red-500/30 focus:border-red-500/60";

const labelCls = "text-[10px] font-semibold uppercase tracking-[0.1em] text-orbflow-text-ghost";

/* =======================================================
   Component
   ======================================================= */

export function ChangeRequestCreate({
  workflowId,
  onClose,
  onCreated,
}: ChangeRequestCreateProps) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [author, setAuthor] = useState("");
  const [reviewers, setReviewers] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const titleRef = useRef<HTMLInputElement>(null);

  const store = useChangeRequestStore();
  const workflowVersion = useWorkflowStore((s) => s.selectedWorkflow?.version ?? 1);

  useEffect(() => {
    titleRef.current?.focus();
  }, []);

  const clearError = useCallback((field: string) => {
    setErrors((prev) => {
      if (!prev[field]) return prev;
      const next = { ...prev };
      delete next[field];
      return next;
    });
  }, []);

  const nodeCount = useCanvasStore((s) => s.nodes.length);
  const edgeCount = useCanvasStore((s) => s.edges.length);

  const validate = useCallback((): boolean => {
    const errs: Record<string, string> = {};
    if (!title.trim()) errs.title = "Title is required";
    else if (title.length > 200) errs.title = "Max 200 characters";
    if (!author.trim()) errs.author = "Author is required";
    else if (author.length > 100) errs.author = "Max 100 characters";
    if (description.length > 5000) errs.description = "Max 5000 characters";
    setErrors(errs);
    return Object.keys(errs).length === 0;
  }, [title, author, description]);

  const handleSubmit = useCallback(
    async (asDraft: boolean) => {
      if (!validate()) return;

      setSubmitting(true);
      try {
        const canvasNodes = useCanvasStore.getState().nodes;
        const canvasEdges = useCanvasStore.getState().edges;
        const proposed_definition = { nodes: canvasNodes, edges: canvasEdges };

        const input: CreateChangeRequestInput = {
          title: title.trim(),
          description: description.trim() || undefined,
          proposed_definition,
          base_version: workflowVersion,
          author: author.trim(),
          reviewers: reviewers
            .split(",")
            .map((r) => r.trim())
            .filter(Boolean),
        };

        const cr = await store.createChangeRequest(workflowId, input);
        if (!asDraft) {
          await store.submitCR(workflowId, cr.id);
        }
        onCreated(cr.id);
      } catch {
        // Error handled by store toast
      } finally {
        setSubmitting(false);
      }
    },
    [title, description, author, reviewers, workflowId, workflowVersion, store, onCreated, validate],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSubmit(false);
      }
      if (e.key === "Escape") onClose();
    },
    [handleSubmit, onClose],
  );

  const reviewerList = reviewers
    .split(",")
    .map((r) => r.trim())
    .filter(Boolean);

  /* Readiness checks */
  const checks = [
    { label: "Title", done: !!title.trim() },
    { label: "Author", done: !!author.trim() },
    { label: "Canvas captured", done: nodeCount > 0 },
  ];
  const readyCount = checks.filter((c) => c.done).length;

  return (
    <div className="flex flex-col h-full animate-fade-in" onKeyDown={handleKeyDown}>
      {/* -- Header -- */}
      <div className="flex items-center justify-between px-6 py-3.5 border-b border-orbflow-border">
        <div className="flex items-center gap-3">
          <div className="w-7 h-7 rounded-lg bg-electric-indigo/10 flex items-center justify-center">
            <NodeIcon name="git-pull-request" className="w-3.5 h-3.5 text-electric-indigo" />
          </div>
          <div>
            <h2 className="text-sm font-semibold text-orbflow-text-secondary">
              New Change Request
            </h2>
            <p className="text-[10px] text-orbflow-text-ghost">
              Propose changes for collaborative review
            </p>
          </div>
        </div>
        <button
          onClick={onClose}
          className="w-6 h-6 rounded-md flex items-center justify-center text-orbflow-text-ghost hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover transition-colors"
          aria-label="Close"
        >
          <NodeIcon name="x" className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* -- Body: two-column layout -- */}
      <div className="flex-1 overflow-y-auto flex items-stretch">
        {/* Left: Form fields */}
        <div className="flex-1 min-w-0 px-6 py-5 space-y-4">
          {/* Title */}
          <div className="space-y-1.5">
            <div className="flex items-baseline justify-between">
              <label htmlFor="cr-title" className={labelCls}>
                Title <span className="text-electric-indigo">*</span>
              </label>
              <span className={cn(
                "text-[10px] font-mono tabular-nums",
                title.length > 180 ? "text-amber-400" : "text-orbflow-text-ghost/50",
                title.length > 200 && "text-red-400",
              )}>
                {title.length}/200
              </span>
            </div>
            <input
              ref={titleRef}
              id="cr-title"
              value={title}
              onChange={(e) => { setTitle(e.target.value); clearError("title"); }}
              placeholder="e.g. Add retry logic to HTTP nodes"
              autoComplete="off"
              className={cn(inputCls, errors.title && inputErrorCls)}
            />
            {errors.title && (
              <p className="text-[10px] text-red-400 flex items-center gap-1">
                <NodeIcon name="alert-triangle" className="w-2.5 h-2.5" />
                {errors.title}
              </p>
            )}
          </div>

          {/* Description */}
          <div className="space-y-1.5">
            <div className="flex items-baseline justify-between">
              <label htmlFor="cr-desc" className={labelCls}>Description</label>
              <span className="text-[10px] font-mono tabular-nums text-orbflow-text-ghost/50">
                {description.length}/5000
              </span>
            </div>
            <textarea
              id="cr-desc"
              value={description}
              onChange={(e) => { setDescription(e.target.value); clearError("description"); }}
              placeholder="Why are these changes needed? What problem do they solve?"
              autoComplete="off"
              rows={3}
              className={cn(inputCls, "resize-none leading-relaxed")}
            />
            <p className="text-[10px] text-orbflow-text-ghost/50">Markdown supported</p>
          </div>

          {/* Author + Reviewers side by side */}
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <label htmlFor="cr-author" className={labelCls}>
                Author <span className="text-electric-indigo">*</span>
              </label>
              <div className="relative">
                <NodeIcon
                  name="users"
                  className="absolute left-3 top-1/2 -translate-y-1/2 w-3 h-3 text-orbflow-text-ghost"
                />
                <input
                  id="cr-author"
                  value={author}
                  onChange={(e) => { setAuthor(e.target.value); clearError("author"); }}
                  placeholder="Your name"
                  autoComplete="off"
                  className={cn(inputCls, "pl-8", errors.author && inputErrorCls)}
                />
              </div>
              {errors.author && (
                <p className="text-[10px] text-red-400 flex items-center gap-1">
                  <NodeIcon name="alert-triangle" className="w-2.5 h-2.5" />
                  {errors.author}
                </p>
              )}
            </div>

            <div className="space-y-1.5">
              <label htmlFor="cr-reviewers" className={labelCls}>Reviewers</label>
              <input
                id="cr-reviewers"
                value={reviewers}
                onChange={(e) => setReviewers(e.target.value)}
                placeholder="alice, bob, carol"
                autoComplete="off"
                className={inputCls}
              />
              {reviewerList.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {reviewerList.map((name) => (
                    <span
                      key={name}
                      className="inline-flex items-center gap-1 px-1.5 py-px rounded bg-orbflow-surface border border-orbflow-border text-[10px] text-orbflow-text-muted"
                    >
                      <span className="w-1 h-1 rounded-full bg-neon-cyan" />
                      {name}
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Right: Preview sidebar */}
        <div className="w-60 shrink-0 border-l border-orbflow-border/40 bg-orbflow-bg/40 px-4 py-5 space-y-5">
          {/* Canvas snapshot */}
          <div className="space-y-2">
            <p className={labelCls}>Canvas Snapshot</p>
            <div className="rounded-lg border border-electric-indigo/15 bg-electric-indigo/[0.03] p-3 space-y-2">
              <div className="flex items-center gap-2">
                <div className="w-6 h-6 rounded-md bg-electric-indigo/10 flex items-center justify-center">
                  <NodeIcon name="workflow" className="w-3 h-3 text-electric-indigo" />
                </div>
                <span className="text-xs font-medium text-orbflow-text-secondary">Proposed workflow</span>
              </div>
              <div className="flex items-center gap-3">
                <span className="inline-flex items-center gap-1 text-[10px] text-orbflow-text-ghost">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                  {nodeCount} nodes
                </span>
                <span className="inline-flex items-center gap-1 text-[10px] text-orbflow-text-ghost">
                  <span className="w-1.5 h-1.5 rounded-full bg-blue-500" />
                  {edgeCount} edges
                </span>
              </div>
            </div>
          </div>

          {/* Readiness */}
          <div className="space-y-2">
            <p className={labelCls}>Readiness ({readyCount}/{checks.length})</p>
            <div className="space-y-1.5">
              {checks.map((c) => (
                <div key={c.label} className="flex items-center gap-2 text-xs">
                  <div className={cn(
                    "w-3.5 h-3.5 rounded-full flex items-center justify-center shrink-0",
                    c.done ? "bg-emerald-500/15" : "bg-orbflow-surface-hover"
                  )}>
                    {c.done ? (
                      <NodeIcon name="check" className="w-2 h-2 text-emerald-400" />
                    ) : (
                      <div className="w-1 h-1 rounded-full bg-orbflow-text-ghost/40" />
                    )}
                  </div>
                  <span className={c.done ? "text-orbflow-text-muted" : "text-orbflow-text-ghost/60"}>
                    {c.label}
                  </span>
                </div>
              ))}
            </div>
          </div>

          {/* Tip */}
          <div className="rounded-lg bg-orbflow-surface/40 border border-orbflow-border/30 p-3">
            <p className="text-[10px] text-orbflow-text-ghost leading-relaxed">
              The current canvas will be captured as a snapshot. Reviewers can compare the proposed changes against the live workflow.
            </p>
          </div>
        </div>
      </div>

      {/* -- Footer -- */}
      <div className="border-t border-orbflow-border px-6 py-3">
        <div className="flex items-center justify-between">
          <p className="text-[10px] text-orbflow-text-ghost font-mono">
            {"\u2318"}+Enter to submit
          </p>
          <div className="flex items-center gap-2.5">
            <Button variant="ghost" size="sm" onClick={onClose}>
              Cancel
            </Button>
            <Button
              variant="secondary"
              size="sm"
              icon="file-text"
              onClick={() => handleSubmit(true)}
              loading={submitting}
            >
              Save Draft
            </Button>
            <Button
              variant="primary"
              size="sm"
              icon="git-pull-request"
              onClick={() => handleSubmit(false)}
              loading={submitting}
            >
              Submit for Review
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
