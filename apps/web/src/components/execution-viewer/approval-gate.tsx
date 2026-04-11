"use client";

/**
 * Approval gate UI for nodes in WaitingApproval state.
 *
 * Shows approve/reject buttons with optional reason input.
 */
import { useState, useCallback } from "react";
import { cn } from "@/lib/cn";

type ErrorState = { action: string; message: string } | null;

interface ApprovalGateProps {
  instanceId: string;
  nodeId: string;
  nodeName: string;
  onApprove: (instanceId: string, nodeId: string, approvedBy?: string) => Promise<void>;
  onReject: (instanceId: string, nodeId: string, reason?: string) => Promise<void>;
}

export function ApprovalGate({
  instanceId,
  nodeId,
  nodeName,
  onApprove,
  onReject,
}: ApprovalGateProps) {
  const [loading, setLoading] = useState(false);
  const [showRejectInput, setShowRejectInput] = useState(false);
  const [rejectReason, setRejectReason] = useState("");
  const [result, setResult] = useState<"approved" | "rejected" | null>(null);
  const [error, setError] = useState<ErrorState>(null);

  const handleApprove = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      await onApprove(instanceId, nodeId);
      setResult("approved");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "An unexpected error occurred";
      setError({ action: "approve", message: msg });
    } finally {
      setLoading(false);
    }
  }, [instanceId, nodeId, onApprove]);

  const handleReject = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      await onReject(instanceId, nodeId, rejectReason || undefined);
      setResult("rejected");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "An unexpected error occurred";
      setError({ action: "reject", message: msg });
    } finally {
      setLoading(false);
    }
  }, [instanceId, nodeId, rejectReason, onReject]);

  if (result) {
    const isApproved = result === "approved";
    return (
      <div
        className="flex items-center gap-2 px-4 py-3 rounded-lg text-body-sm font-medium border"
        style={{
          color: isApproved ? "var(--orbflow-exec-completed)" : "var(--orbflow-exec-failed)",
          borderColor: isApproved
            ? "color-mix(in srgb, var(--orbflow-exec-completed) 36%, transparent)"
            : "color-mix(in srgb, var(--orbflow-exec-failed) 36%, transparent)",
          backgroundColor: isApproved
            ? "color-mix(in srgb, var(--orbflow-exec-completed) 12%, transparent)"
            : "color-mix(in srgb, var(--orbflow-exec-failed) 12%, transparent)",
        }}
      >
        <span className="text-base">{result === "approved" ? "\u2713" : "\u2717"}</span>
        <span>
          Node <strong>{nodeName}</strong> was {result}.
        </span>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-amber-500/30 bg-amber-500/[0.08] p-4">
      <div className="flex items-center gap-2 mb-3 text-body-sm font-semibold text-amber-400">
        <span className="text-base">{"\u23F8"}</span>
        <span>Approval Required</span>
      </div>
      <p className="text-caption leading-relaxed mb-3 text-orbflow-text-faint">
        Node <strong className="text-orbflow-text-secondary">{nodeName}</strong> is
        waiting for human approval before it can execute.
      </p>

      {showRejectInput && (
        <div className="mb-3">
          <input
            type="text"
            placeholder="Reason for rejection (optional)"
            value={rejectReason}
            onChange={(e) => setRejectReason(e.target.value)}
            className="w-full px-3 py-2 rounded-md border border-orbflow-border bg-orbflow-surface text-orbflow-text-secondary text-caption outline-none
              focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50"
          />
        </div>
      )}

      {error && (
        <p className="text-caption mb-2" style={{ color: "var(--orbflow-exec-failed)" }}>
          Failed to {error.action}: {error.message}
        </p>
      )}

      <div className="flex gap-2">
        <button
          onClick={handleApprove}
          disabled={loading}
          className={cn(
            "flex-1 px-4 py-2 rounded-md border-none text-caption font-semibold text-white transition-opacity",
            "bg-emerald-500 hover:bg-emerald-600",
            "focus-visible:ring-2 focus-visible:ring-emerald-500/50 focus-visible:outline-none",
            loading && "opacity-60 cursor-wait"
          )}
        >
          {loading ? "..." : "Approve"}
        </button>
        {!showRejectInput ? (
          <button
            onClick={() => setShowRejectInput(true)}
            disabled={loading}
            className={cn(
              "flex-1 px-4 py-2 rounded-md border border-rose-500/50 bg-transparent text-caption font-semibold text-rose-400 transition-opacity",
              "hover:bg-rose-500/10",
              "focus-visible:ring-2 focus-visible:ring-rose-500/50 focus-visible:outline-none",
              loading && "opacity-60 cursor-wait"
            )}
          >
            Reject
          </button>
        ) : (
          <button
            onClick={handleReject}
            disabled={loading}
            className={cn(
              "flex-1 px-4 py-2 rounded-md border-none text-caption font-semibold text-white transition-opacity",
              "bg-rose-500 hover:bg-rose-600",
              "focus-visible:ring-2 focus-visible:ring-rose-500/50 focus-visible:outline-none",
              loading && "opacity-60 cursor-wait"
            )}
          >
            {loading ? "..." : "Confirm Reject"}
          </button>
        )}
      </div>
    </div>
  );
}
