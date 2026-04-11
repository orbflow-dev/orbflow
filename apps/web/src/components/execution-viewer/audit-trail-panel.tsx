"use client";

import { useState, useCallback, useEffect, useRef } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { copyToClipboard } from "@/lib/clipboard";
import { api } from "@/lib/api";
import type {
  AuditRecord,
  AuditProofResult,
  AuditVerifyResult,
  ComplianceFormat,
} from "@orbflow/core";

/* -- Types ------------------------------------------- */

interface AuditTrailPanelProps {
  instanceId: string;
  auditResult: AuditVerifyResult | null;
  auditError: string | null;
  auditState: "idle" | "loading" | "done";
  onVerify: () => void;
  onClose: () => void;
}

type TrailState = "idle" | "loading" | "loaded" | "error";

/* -- Helpers ----------------------------------------- */

function parseEventType(eventData: string): string {
  try {
    const parsed = JSON.parse(eventData);
    return parsed.type || parsed.event_type || parsed.kind || "Event";
  } catch {
    return "Event";
  }
}

function formatTimestamp(eventData: string): string | null {
  try {
    const parsed = JSON.parse(eventData);
    const ts = parsed.timestamp || parsed.created_at || parsed.at;
    if (!ts) return null;
    return new Date(ts).toLocaleString();
  } catch {
    return null;
  }
}

/* -- Export Dropdown ---------------------------------- */

const COMPLIANCE_FORMATS: { key: ComplianceFormat; label: string }[] = [
  { key: "soc2", label: "SOC2" },
  { key: "hipaa", label: "HIPAA" },
  { key: "pci", label: "PCI" },
];

function ExportDropdown({ instanceId }: { instanceId: string }) {
  const [open, setOpen] = useState(false);

  const handleExport = (format: ComplianceFormat) => {
    const url = api.instances.exportAuditTrail(instanceId, format);
    window.open(url);
    setOpen(false);
  };

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((prev) => !prev)}
        className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-body-sm font-medium transition-colors
          border border-orbflow-border text-orbflow-text-faint
          hover:text-orbflow-text-secondary hover:border-orbflow-border-hover
          focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
      >
        <NodeIcon name="download" className="w-3 h-3" />
        Export
        <NodeIcon name="chevron-down" className="w-3 h-3" />
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-[89]" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-full mt-1 z-[90] w-36 rounded-lg border border-orbflow-border bg-orbflow-surface shadow-xl py-1">
            {COMPLIANCE_FORMATS.map((f) => (
              <button
                key={f.key}
                onClick={() => handleExport(f.key)}
                className="w-full text-left px-3 py-2 text-body-sm text-orbflow-text-faint
                  hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover transition-colors"
              >
                <NodeIcon name="file-text" className="w-3 h-3 inline mr-2 opacity-60" />
                {f.label}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

/* -- Verification Badge ------------------------------ */

function VerificationBadge({
  auditState,
  auditResult,
  auditError,
  onVerify,
}: {
  auditState: "idle" | "loading" | "done";
  auditResult: AuditVerifyResult | null;
  auditError: string | null;
  onVerify: () => void;
}) {
  if (auditState === "idle") {
    return (
      <button
        onClick={onVerify}
        className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-body-sm font-medium transition-colors
          border border-orbflow-border text-orbflow-text-faint
          hover:text-orbflow-text-secondary hover:border-orbflow-border-hover
          focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
      >
        <NodeIcon name="shield" className="w-3 h-3" />
        Verify Chain
      </button>
    );
  }

  if (auditState === "loading") {
    return (
      <span className="flex items-center gap-1.5 px-3 py-1.5 text-body-sm text-orbflow-text-faint">
        <NodeIcon name="loader" className="w-3 h-3 animate-spin" />
        Verifying...
      </span>
    );
  }

  if (auditResult?.valid) {
    return (
      <span
        className="flex items-center gap-1.5 px-3 py-1.5 text-body-sm font-medium"
        style={{ color: "var(--orbflow-exec-completed)" }}
      >
        <NodeIcon name="check" className="w-3.5 h-3.5" />
        Chain verified ({auditResult.event_count} events)
      </span>
    );
  }

  return (
    <span
      className="flex items-center gap-1.5 px-3 py-1.5 text-body-sm font-medium"
      style={{ color: "var(--orbflow-exec-failed)" }}
      title={auditError || auditResult?.error}
    >
      <NodeIcon name="x" className="w-3.5 h-3.5" />
      {auditError || auditResult?.error || "Chain broken"}
    </span>
  );
}

/* -- Hash Preview (click-to-copy) -------------------- */

function HashPreview({ hash }: { hash: string }) {
  const [copied, setCopied] = useState(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    };
  }, []);

  const handleCopy = async () => {
    await copyToClipboard(hash);
    if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    setCopied(true);
    copyTimerRef.current = setTimeout(() => setCopied(false), 2000);
  };

  return (
    <button
      onClick={handleCopy}
      title={copied ? "Copied!" : `Click to copy: ${hash}`}
      className={cn(
        "font-mono text-xs px-1.5 py-0.5 rounded transition-colors",
        copied
          ? "bg-emerald-500/10 text-emerald-400"
          : "bg-orbflow-surface-hover/80 text-orbflow-text-muted hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover",
      )}
    >
      {copied ? "Copied!" : hash.slice(0, 8)}
    </button>
  );
}

/* -- Merkle Proof Viewer ----------------------------- */

function MerkleProofViewer({
  instanceId,
  seq,
}: {
  instanceId: string;
  seq: number;
}) {
  const [state, setState] = useState<"idle" | "loading" | "done" | "error">("idle");
  const [proof, setProof] = useState<AuditProofResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleVerify = async () => {
    setState("loading");
    try {
      const result = await api.instances.getAuditProof(instanceId, seq);
      setProof(result);
      setState("done");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Proof failed");
      setState("error");
    }
  };

  if (state === "idle") {
    return (
      <button
        onClick={handleVerify}
        className="text-micro font-medium text-orbflow-text-ghost hover:text-electric-indigo transition-colors"
        title="Verify Merkle proof for this event"
      >
        Proof
      </button>
    );
  }

  if (state === "loading") {
    return (
      <span className="text-micro text-orbflow-text-ghost flex items-center gap-1">
        <NodeIcon name="loader" className="w-2.5 h-2.5 animate-spin" />
      </span>
    );
  }

  if (state === "error") {
    return (
      <span
        className="text-micro"
        style={{ color: "var(--orbflow-exec-failed)" }}
        title={error || undefined}
      >
        Failed
      </span>
    );
  }

  if (!proof) return null;

  return (
    <span
      className="text-micro font-medium"
      style={{
        color: proof.valid ? "var(--orbflow-exec-completed)" : "var(--orbflow-exec-failed)",
      }}
      title={`Root: ${proof.merkle_root} | Leaf: ${proof.leaf_hash} | ${proof.proof.length} nodes`}
    >
      {proof.valid ? "Valid" : "Invalid"}
    </span>
  );
}

/* -- Audit Event Row --------------------------------- */

function AuditEventRow({
  record,
  instanceId,
  isLast,
}: {
  record: AuditRecord;
  instanceId: string;
  isLast: boolean;
}) {
  const eventType = parseEventType(record.event_data);
  const timestamp = formatTimestamp(record.event_data);
  const isSigned = !!record.signature;

  return (
    <div className="flex gap-3 group">
      {/* Chain visualization column */}
      <div className="flex flex-col items-center shrink-0 w-6">
        <div
          className={cn(
            "w-2 h-2 rounded-full shrink-0 mt-2 ring-2 ring-orbflow-surface",
            isSigned ? "bg-emerald-400" : "bg-orbflow-text-ghost/25",
          )}
        />
        {!isLast && (
          <div className="w-px flex-1 bg-orbflow-border mt-1" />
        )}
      </div>

      {/* Event content */}
      <div className="flex-1 min-w-0 pb-4">
        <div className="flex items-center gap-2 flex-wrap">
          {/* Sequence badge */}
          <span className="text-micro font-mono font-bold text-orbflow-text-ghost/60">
            #{record.seq}
          </span>

          {/* Event type */}
          <span className="text-body-sm font-medium text-orbflow-text-secondary truncate">
            {eventType}
          </span>

          {/* Signature badge */}
          <span
            className={cn(
              "text-micro font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded",
              isSigned
                ? "text-emerald-400 bg-emerald-500/10"
                : "text-orbflow-text-muted bg-orbflow-surface-hover/80",
            )}
          >
            {isSigned ? "Signed" : "Unsigned"}
          </span>

          {/* Merkle proof button */}
          <MerkleProofViewer instanceId={instanceId} seq={record.seq} />
        </div>

        {/* Timestamp + hashes row */}
        <div className="flex items-center gap-2 mt-1.5 flex-wrap">
          {timestamp && (
            <span className="text-caption text-orbflow-text-ghost/60">
              {timestamp}
            </span>
          )}
          <span className="w-0.5 h-0.5 rounded-full bg-orbflow-text-ghost/20 shrink-0" />
          <div className="flex items-center gap-1 text-caption text-orbflow-text-ghost/50">
            <NodeIcon name="link" className="w-2.5 h-2.5 opacity-40" />
            <HashPreview hash={record.prev_hash} />
            <span className="text-orbflow-text-ghost/30 mx-0.5">&rarr;</span>
            <HashPreview hash={record.event_hash} />
          </div>
        </div>
      </div>
    </div>
  );
}

/* -- Main Panel -------------------------------------- */

export function AuditTrailPanel({
  instanceId,
  auditResult,
  auditError,
  auditState,
  onVerify,
  onClose,
}: AuditTrailPanelProps) {
  const [trailState, setTrailState] = useState<TrailState>("idle");
  const [records, setRecords] = useState<AuditRecord[]>([]);
  const [trailError, setTrailError] = useState<string | null>(null);

  const loadTrail = useCallback(async () => {
    setTrailState("loading");
    setTrailError(null);
    try {
      const data = await api.instances.getAuditTrail(instanceId);
      setRecords(data);
      setTrailState("loaded");
    } catch (err) {
      setTrailError(err instanceof Error ? err.message : "Failed to load audit trail");
      setTrailState("error");
    }
  }, [instanceId]);

  // Auto-load on mount
  const loadedRef = useRef(false);
  useEffect(() => {
    if (!loadedRef.current) {
      loadedRef.current = true;
      loadTrail();
    }
  }, [loadTrail]);

  return (
    <div className="flex flex-col h-full bg-orbflow-surface">
      {/* Header */}
      <div className="shrink-0 flex items-center justify-between gap-3 px-6 py-4 border-b border-orbflow-border/60">
        <div className="flex items-center gap-3 min-w-0">
          <button
            onClick={onClose}
            className="shrink-0 p-1 rounded-md text-orbflow-text-ghost hover:text-orbflow-text-faint hover:bg-orbflow-surface-hover transition-colors
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            aria-label="Close audit trail"
          >
            <NodeIcon name="x" className="w-4 h-4" />
          </button>
          <h3 className="text-display font-semibold text-orbflow-text-secondary truncate">
            Audit Trail
          </h3>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <VerificationBadge
            auditState={auditState}
            auditResult={auditResult}
            auditError={auditError}
            onVerify={onVerify}
          />
          <ExportDropdown instanceId={instanceId} />
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar">
        {/* Loading */}
        {trailState === "loading" && (
          <div className="flex items-center justify-center py-16">
            <div className="flex items-center gap-2.5 text-orbflow-text-faint">
              <NodeIcon name="loader" className="w-4 h-4 animate-spin" />
              <span className="text-body">Loading audit trail...</span>
            </div>
          </div>
        )}

        {/* Error */}
        {trailState === "error" && (
          <div className="flex flex-col items-center justify-center py-16 gap-3">
            <NodeIcon name="alert-triangle" className="w-6 h-6 text-rose-400/60" />
            <p className="text-body text-rose-400/80">{trailError}</p>
            <button
              onClick={loadTrail}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium
                border border-orbflow-border text-orbflow-text-faint
                hover:text-orbflow-text-secondary hover:border-orbflow-border-hover transition-colors
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            >
              <NodeIcon name="repeat" className="w-3 h-3" />
              Retry
            </button>
          </div>
        )}

        {/* Empty state */}
        {trailState === "loaded" && records.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 gap-3">
            <NodeIcon name="shield" className="w-6 h-6 text-orbflow-text-ghost/40" />
            <p className="text-body text-orbflow-text-ghost">No audit events recorded</p>
          </div>
        )}

        {/* Event list */}
        {trailState === "loaded" && records.length > 0 && (
          <div className="px-6 py-4">
            {records.map((record, idx) => (
              <AuditEventRow
                key={record.seq}
                record={record}
                instanceId={instanceId}
                isLast={idx === records.length - 1}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
