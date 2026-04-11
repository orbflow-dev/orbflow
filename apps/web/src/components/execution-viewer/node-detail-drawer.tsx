"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import { cn } from "@/lib/cn";
import { NodeIcon } from "@/core/components/icons";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { StructuredOutput } from "./structured-output";
import { StreamingOutput } from "./streaming-output";
import { useNodeStream } from "@/hooks/use-node-stream";
import { api } from "@/lib/api";
import type { Instance, NodeState, WorkflowNode } from "@/lib/api";
import { STATUS_THEMES, FALLBACK_THEME } from "@/lib/execution";
import { extractContentType } from "@/lib/output-safety";
import { ApprovalGate } from "./approval-gate";

const AI_NODE_TYPES = ["chat", "extract", "classify", "summarize", "sentiment", "translate"] as const;

const CREDENTIAL_KEY_PATTERNS =
  /credential|password|secret|token|api_key|apikey|private_key|access_key|client_secret/i;

const CREDENTIAL_VALUE_PATTERNS =
  /(sk[_-]live|sk[_-]test|sk[_-]ant|pk[_-]live|pk[_-]test|ghp_|gho_|github_pat_|xoxb-|xoxp-|AKIA|ASIA|rk_live|rk_test|SG\.|Bearer\s|postgres:\/\/|mysql:\/\/|mongodb(\+srv)?:\/\/)/;

function maskCredentialValues(
  params: Record<string, unknown>,
): Record<string, unknown> {
  const masked: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(params)) {
    if (CREDENTIAL_KEY_PATTERNS.test(key)) {
      masked[key] = "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022";
    } else if (typeof value === "string" && CREDENTIAL_VALUE_PATTERNS.test(value)) {
      masked[key] = "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022";
    } else if (value && typeof value === "object" && !Array.isArray(value)) {
      masked[key] = maskCredentialValues(value as Record<string, unknown>);
    } else {
      masked[key] = value;
    }
  }
  return masked;
}

interface NodeDetailDrawerProps {
  nodeId: string;
  nodeState: NodeState;
  workflowNode: WorkflowNode;
  instance: Instance;
  onClose: () => void;
  onApprove?: (instanceId: string, nodeId: string, approvedBy?: string) => Promise<void>;
  onReject?: (instanceId: string, nodeId: string, reason?: string) => Promise<void>;
}


function NodeDetailDrawer({
  nodeId,
  nodeState,
  workflowNode,
  instance,
  onClose,
  onApprove,
  onReject,
}: NodeDetailDrawerProps) {
  const [isOpen, setIsOpen] = useState(false);
  const drawerRef = useRef<HTMLDivElement>(null);
  const closeTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useFocusTrap(drawerRef);

  // Streaming: only activate for AI nodes in running state
  const isNodeRunning = nodeState.status === "running" || nodeState.status === "queued";
  const isAiNode =
    workflowNode.plugin_ref?.startsWith("ai_") ||
    AI_NODE_TYPES.includes(workflowNode.plugin_ref?.replace("builtin:", "") as typeof AI_NODE_TYPES[number]);
  const shouldStream = isNodeRunning && isAiNode;

  const streamUrl =
    shouldStream && instance?.id && nodeId
      ? api.instances.streamUrl(instance.id, nodeId)
      : null;

  const { isStreaming, tokens, finalOutput, error: streamError } = useNodeStream({
    url: streamUrl,
    enabled: !!streamUrl,
  });

  // Animate in on mount
  useEffect(() => {
    const id = requestAnimationFrame(() => setIsOpen(true));
    return () => cancelAnimationFrame(id);
  }, []);

  // Clean up close timer on unmount
  useEffect(() => {
    return () => {
      if (closeTimerRef.current) clearTimeout(closeTimerRef.current);
    };
  }, []);

  // Handle close with animation
  const handleClose = useCallback(() => {
    setIsOpen(false);
    if (closeTimerRef.current) clearTimeout(closeTimerRef.current);
    closeTimerRef.current = setTimeout(onClose, 200);
  }, [onClose]);

  // Escape key to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleClose]);

  const status = STATUS_THEMES[nodeState.status] ?? FALLBACK_THEME;

  return (
    <div className="absolute inset-0 z-40 flex">
      {/* Backdrop */}
      <div
        className={cn(
          "flex-1 bg-black/20 backdrop-blur-[2px] transition-opacity duration-200",
        )}
        style={{ opacity: isOpen ? 1 : 0 }}
        onClick={handleClose}
        aria-hidden="true"
      />

      {/* Drawer panel */}
      <div
        ref={drawerRef}
        role="dialog"
        aria-label={`Details for ${workflowNode.name || nodeId}`}
        className={cn(
          "w-[460px] max-w-[90vw] h-full bg-orbflow-bg border-l border-orbflow-border",
          "overflow-y-auto custom-scrollbar flex flex-col",
        )}
        style={{
          transform: isOpen ? "translateX(0)" : "translateX(100%)",
          transition: "transform 0.2s ease-out",
        }}
      >
        {/* Header */}
        <div
          className={cn(
            "sticky top-0 z-10 bg-orbflow-bg/90 backdrop-blur-md",
            "border-b border-orbflow-border px-5 py-4",
          )}
        >
          <div className="flex items-center gap-3">
            <button
              onClick={handleClose}
              aria-label="Close drawer"
              className={cn(
                "flex items-center gap-1.5 text-body rounded-lg px-1.5 py-1",
                "text-orbflow-text-ghost hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover transition-colors",
                "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
              )}
            >
              <NodeIcon name="arrow-right" className="w-3.5 h-3.5 rotate-180" />
              Back
            </button>
            <div className="flex-1" />
            <span
              className={cn(
                "text-heading font-semibold text-orbflow-text-secondary",
                "truncate max-w-[200px] flex items-center gap-2",
              )}
              title={workflowNode.name || nodeId}
            >
              {workflowNode.name || nodeId}
              {isStreaming && (
                <span
                  className="inline-block w-2 h-2 rounded-full shrink-0"
                  style={{
                    background: "#4A9AAF",
                    animation: "pulse 1.5s ease-in-out infinite",
                  }}
                />
              )}
            </span>
          </div>
        </div>

        {/* Status bar */}
        <div className="px-5 py-3 flex items-center gap-3 border-b border-orbflow-border/40">
          <div className="flex items-center gap-2">
            <span
              className="w-2 h-2 rounded-full shrink-0"
              style={{ backgroundColor: status.accent }}
            />
            <span
              className="text-body font-medium"
              style={{ color: `var(--exec-text-${nodeState.status})` }}
            >
              {status.label}
            </span>
          </div>

          {nodeState.attempt > 0 && (
            <span className="text-body-sm text-orbflow-text-ghost tabular-nums">
              Attempt {nodeState.attempt}
            </span>
          )}

          <div className="flex-1" />

          {isStreaming && (
            <span className="text-caption tabular-nums" style={{ color: "#4A9AAF" }}>
              {tokens.length} tokens
            </span>
          )}

          <span
            className={cn(
              "text-caption font-mono px-2 py-0.5 rounded-full",
              "bg-orbflow-surface/40 text-orbflow-text-ghost border border-orbflow-border/30",
            )}
          >
            {workflowNode.plugin_ref}
          </span>
        </div>

        {/* Approval gate (WaitingApproval nodes only) */}
        {nodeState.status === "waiting_approval" && onApprove && onReject && (
          <section className="px-5 py-4">
            <ApprovalGate
              instanceId={instance.id}
              nodeId={nodeId}
              nodeName={workflowNode.name || nodeId}
              onApprove={onApprove}
              onReject={onReject}
            />
          </section>
        )}

        {/* Input section */}
        <section className="px-5 py-4">
          <div className="flex items-center gap-2 mb-3">
            <NodeIcon name="inbox" className="w-3.5 h-3.5 text-orbflow-text-ghost" />
            <h3
              className={cn(
                "text-body-sm uppercase tracking-wider font-semibold",
                "text-orbflow-text-ghost",
              )}
            >
              Input
            </h3>
          </div>
          {nodeState.input ? (
            <div className="rounded-lg border border-orbflow-border/60 p-3 bg-orbflow-surface/20 max-h-[320px] overflow-y-auto custom-scrollbar">
              <StructuredOutput
                data={nodeState.input}
                pluginRef={workflowNode.plugin_ref}
              />
            </div>
          ) : (
            <p className="text-body text-orbflow-text-ghost/50 italic">
              No input data
            </p>
          )}
        </section>

        {/* Parameters section */}
        {nodeState.parameters &&
        Object.keys(nodeState.parameters).length > 0 && (
        <section className="px-5 py-4 border-t border-orbflow-border/20">
          <div className="flex items-center gap-2 mb-3">
            <NodeIcon name="settings" className="w-3.5 h-3.5 text-orbflow-text-ghost" />
            <h3
              className={cn(
                "text-body-sm uppercase tracking-wider font-semibold",
                "text-orbflow-text-ghost",
              )}
            >
              Parameters
            </h3>
          </div>
          <div className="rounded-lg border border-orbflow-border/60 p-3 bg-orbflow-surface/20 max-h-[280px] overflow-y-auto custom-scrollbar">
            <StructuredOutput
              data={maskCredentialValues(nodeState.parameters)}
              pluginRef={workflowNode.plugin_ref}
            />
          </div>
        </section>
        )}

        {/* Output section */}
        <OutputSection
          nodeState={nodeState}
          workflowNode={workflowNode}
          shouldStream={shouldStream}
          isStreaming={isStreaming}
          tokens={tokens}
          finalOutput={finalOutput}
          streamError={streamError}
        />

        {/* Error section (failed nodes only) */}
        {nodeState.status === "failed" && nodeState.error && (
          <section className="px-5 py-4 border-t border-orbflow-border/20">
            <div className="flex items-center gap-2 mb-3">
              <NodeIcon name="alert-triangle" className="w-3.5 h-3.5" style={{ color: "var(--exec-text-failed)" }} />
              <h3
                className="text-body-sm uppercase tracking-wider font-semibold"
                style={{ color: "var(--exec-text-failed)" }}
              >
                Error
              </h3>
            </div>
            <div className="rounded-lg border border-rose-500/20 p-4 bg-rose-500/5 max-h-[200px] overflow-y-auto custom-scrollbar">
              <pre
                className={cn(
                  "text-body font-mono text-rose-400/80",
                  "whitespace-pre-wrap break-words leading-relaxed",
                )}
              >
                {nodeState.error}
              </pre>
            </div>
            {nodeState.attempt > 1 && (
              <p className="mt-3 text-body-sm text-orbflow-text-ghost">
                Failed after {nodeState.attempt} attempts
              </p>
            )}
          </section>
        )}

        {/* Node metadata footer */}
        <div className="mt-auto px-5 py-3 border-t border-orbflow-border/30 flex items-center gap-3">
          <span className="font-mono text-caption text-orbflow-text-ghost/40 truncate">{nodeId}</span>
          <span className="font-mono text-caption text-orbflow-text-ghost/30 px-2 py-0.5 rounded bg-orbflow-surface/30 border border-orbflow-border/20 shrink-0">
            {workflowNode.plugin_ref}
          </span>
        </div>
      </div>
    </div>
  );
}

/** Output section that extracts content-type for HTTP nodes and passes it to StructuredOutput */
function OutputSection({
  nodeState,
  workflowNode,
  shouldStream,
  isStreaming,
  tokens,
  finalOutput,
  streamError,
}: {
  nodeState: NodeState;
  workflowNode: WorkflowNode;
  shouldStream: boolean;
  isStreaming: boolean;
  tokens: string[];
  finalOutput: Record<string, unknown> | null;
  streamError: string | null;
}) {
  // For HTTP nodes, extract content-type from output headers to enable smart rendering
  const outputContentType =
    workflowNode.plugin_ref === "builtin:http" && nodeState.output
      ? extractContentType((nodeState.output as Record<string, unknown>).headers)
      : null;

  // Determine what output to show: finalOutput from stream, or nodeState.output
  const resolvedOutput = finalOutput ?? nodeState.output;

  return (
    <section className="px-5 py-4 border-t border-orbflow-border/20">
      <div className="flex items-center gap-2 mb-3">
        <NodeIcon name="send" className="w-3.5 h-3.5 text-orbflow-text-ghost" />
        <h3
          className={cn(
            "text-body-sm uppercase tracking-wider font-semibold",
            "text-orbflow-text-ghost",
          )}
        >
          Output
        </h3>
      </div>

      {/* Show streaming output for AI nodes that are actively streaming */}
      {shouldStream && (isStreaming || tokens.length > 0 || streamError) && (
        <div className="mb-3">
          <StreamingOutput
            tokens={tokens}
            isStreaming={isStreaming}
            error={streamError}
          />
        </div>
      )}

      {/* Show structured output when available (from stream finalOutput or nodeState) */}
      {resolvedOutput ? (
        <div className="rounded-lg border border-orbflow-border/60 p-3 bg-orbflow-surface/20 max-h-[400px] overflow-y-auto custom-scrollbar">
          <StructuredOutput
            data={resolvedOutput}
            pluginRef={workflowNode.plugin_ref}
            contentType={outputContentType}
          />
        </div>
      ) : (
        !shouldStream && (
          <p className="text-body text-orbflow-text-ghost/50 italic">
            No output data
          </p>
        )
      )}

      <style>{`
        @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
      `}</style>
    </section>
  );
}

export { NodeDetailDrawer };
