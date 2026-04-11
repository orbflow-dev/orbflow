"use client";

import { memo, useState, useRef, useEffect, useMemo, useCallback } from "react";
import { useChangeRequestStore } from "@orbflow/core/stores";
import type { ReviewComment } from "@orbflow/core/types";
import { Button } from "@/core/components/button";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { timeAgo, initials } from "./shared";

/* =======================================================
   Types & helpers
   ======================================================= */

interface CommentThreadProps {
  workflowId: string;
  changeRequestId: string;
  comments: ReviewComment[];
  nodeId?: string;
  edgeRef?: [string, string];
}

type CommentFilter = "all" | "unresolved";

/* =======================================================
   CommentThread
   ======================================================= */

export const CommentThread = memo(function CommentThread({
  workflowId,
  changeRequestId,
  comments,
  nodeId,
  edgeRef,
}: CommentThreadProps) {
  const store = useChangeRequestStore();
  const [filter, setFilter] = useState<CommentFilter>("all");
  const [newComment, setNewComment] = useState("");
  const [author, setAuthor] = useState("");
  const [posting, setPosting] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  /* -- Filtered + sorted comments -- */
  const filteredComments = useMemo(() => {
    let result = comments;

    if (nodeId) {
      result = result.filter((c) => c.node_id === nodeId);
    } else if (edgeRef) {
      result = result.filter(
        (c) => c.edge_ref?.[0] === edgeRef[0] && c.edge_ref?.[1] === edgeRef[1],
      );
    } else {
      result = result.filter((c) => !c.node_id && !c.edge_ref);
    }

    if (filter === "unresolved") {
      result = result.filter((c) => !c.resolved);
    }

    return result.sort(
      (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
    );
  }, [comments, nodeId, edgeRef, filter]);

  /* -- Auto-scroll on new comment -- */
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [filteredComments.length]);

  /* -- Handlers -- */
  const handlePost = useCallback(async () => {
    if (!newComment.trim() || !author.trim()) return;
    setPosting(true);
    try {
      await store.addComment(workflowId, changeRequestId, {
        author: author.trim(),
        body: newComment.trim(),
        node_id: nodeId,
        edge_ref: edgeRef,
      });
      setNewComment("");
    } catch {
      /* handled by store */
    } finally {
      setPosting(false);
    }
  }, [newComment, author, workflowId, changeRequestId, nodeId, edgeRef]);

  const handleResolve = useCallback(async (commentId: string) => {
    await store.resolveComment(workflowId, changeRequestId, commentId);
  }, [workflowId, changeRequestId]);

  const unresolvedCount = filteredComments.filter((c) => !c.resolved).length;

  /* -- Render -- */
  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-orbflow-border">
        <span className="text-body font-medium text-orbflow-text">
          Comments ({filteredComments.length})
          {unresolvedCount > 0 && (
            <span className="ml-1.5 text-amber-400 font-medium">
              · {unresolvedCount} unresolved
            </span>
          )}
        </span>
        <div className="flex gap-1">
          {(["all", "unresolved"] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={cn(
                "text-caption px-2 py-1 rounded-md capitalize transition-colors",
                "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                filter === f
                  ? "bg-electric-indigo/15 text-electric-indigo font-medium"
                  : "text-orbflow-text-muted hover:text-orbflow-text hover:bg-orbflow-surface-hover",
              )}
            >
              {f}
            </button>
          ))}
        </div>
      </div>

      {/* Comment list */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-3 custom-scrollbar">
        {filteredComments.length === 0 && (
          <div className="flex flex-col items-center justify-center py-12 text-center animate-fade-in">
            <div className="w-10 h-10 rounded-xl bg-orbflow-surface-hover flex items-center justify-center mb-3">
              <NodeIcon name="message-circle" className="w-5 h-5 text-orbflow-text-ghost" />
            </div>
            <p className="text-body text-orbflow-text-muted mb-1">No comments yet</p>
            <p className="text-caption text-orbflow-text-ghost">Be the first to leave feedback</p>
          </div>
        )}
        {filteredComments.map((comment, index) => (
          <div
            key={comment.id}
            className={cn(
              "px-3.5 py-3 rounded-lg border border-orbflow-border animate-fade-in-up",
              comment.resolved ? "opacity-50" : "bg-orbflow-surface",
            )}
            style={{ animationDelay: `${Math.min(index * 30, 150)}ms` }}
          >
            <div className="flex items-center justify-between mb-1.5">
              <span className="flex items-center gap-1.5 text-body-sm text-orbflow-text-secondary">
                <span className="w-5 h-5 rounded-full bg-electric-indigo/10 text-electric-indigo text-[9px] font-bold flex items-center justify-center shrink-0">
                  {initials(comment.author)}
                </span>
                <span className="font-medium">{comment.author}</span>
                <span className="text-orbflow-text-ghost">
                  · {timeAgo(comment.created_at)}
                </span>
              </span>
              {!comment.resolved ? (
                <button
                  onClick={() => handleResolve(comment.id)}
                  className="text-caption px-2.5 py-1 rounded-md border border-orbflow-border text-orbflow-text-muted
                    hover:text-emerald-400 hover:border-emerald-500/40 hover:bg-emerald-500/5 transition-colors
                    focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                >
                  Resolve
                </button>
              ) : (
                <span className="text-caption text-emerald-400 flex items-center gap-1">
                  <NodeIcon name="check" className="w-3 h-3" />
                  Resolved
                </span>
              )}
            </div>
            <p className="text-body text-orbflow-text pl-6.5">{comment.body}</p>
          </div>
        ))}
        <div ref={bottomRef} />
      </div>

      {/* Add comment form */}
      <div className="px-4 py-3 border-t border-orbflow-border space-y-2">
        <input
          value={author}
          onChange={(e) => setAuthor(e.target.value)}
          placeholder="Your name"
          className="w-full px-3 py-2 rounded-lg bg-orbflow-bg border border-orbflow-border text-body-sm text-orbflow-text
            placeholder:text-orbflow-text-ghost focus:outline-none focus:border-electric-indigo/30
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
        />
        <div className="flex gap-2">
          <textarea
            value={newComment}
            onChange={(e) => setNewComment(e.target.value)}
            placeholder="Add a comment..."
            rows={2}
            className="flex-1 px-3 py-2 rounded-lg bg-orbflow-bg border border-orbflow-border text-body text-orbflow-text
              placeholder:text-orbflow-text-ghost focus:outline-none focus:border-electric-indigo/30
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 resize-none transition-colors"
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) handlePost();
            }}
          />
          <Button
            variant="primary"
            size="sm"
            onClick={handlePost}
            loading={posting}
            disabled={!newComment.trim() || !author.trim()}
          >
            Comment
          </Button>
        </div>
        <p className="text-micro text-orbflow-text-ghost/50">
          {navigator.platform?.includes("Mac") ? "\u2318" : "Ctrl"}+Enter to submit
        </p>
      </div>
    </div>
  );
});
