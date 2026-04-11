"use client";

import { useMemo, useCallback, useEffect, useRef, useId } from "react";
import { createPortal } from "react-dom";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { NodeIcon, getTypeColor, getTypeLabel } from "@/core/components/icons";
import { StructuredOutput } from "./structured-output";
import { cn } from "@/lib/cn";
import { STATUS_THEMES, FALLBACK_THEME, formatDurationRange } from "@/lib/execution";
import type { NodeState, WorkflowNode } from "@/lib/api";
import type { NodeTypeDefinition, FieldSchema } from "@/core/types/schema";

/* -- Constants ---------------------------------------- */

const CREDENTIAL_KEY_PATTERNS = /credential|password|secret|token/i;

function isCredentialKey(key: string): boolean {
  return CREDENTIAL_KEY_PATTERNS.test(key);
}

/* -- Props -------------------------------------------- */

interface ReadOnlyNodeConfigProps {
  nodeId: string;
  nodeState: NodeState;
  workflowNode: WorkflowNode;
  schema?: NodeTypeDefinition;
  onClose: () => void;
}

/* -- Labeled Field Renderer --------------------------- */

function LabeledField({
  field,
  value,
}: {
  field: FieldSchema;
  value: unknown;
}) {
  const displayValue = value === undefined || value === null ? "" : value;

  return (
    <div className="px-2.5 py-2 rounded-lg hover:bg-orbflow-surface-hover/40 transition-colors">
      <div className="flex items-center gap-2 mb-1">
        <span
          className="text-caption font-bold px-1.5 py-0.5 rounded shrink-0"
          style={{
            color: getTypeColor(field.type),
            backgroundColor: getTypeColor(field.type) + "12",
          }}
        >
          {getTypeLabel(field.type)}
        </span>
        <span className="text-body font-mono truncate text-orbflow-text-muted">
          {field.key}
        </span>
      </div>
      {field.label && field.label !== field.key && (
        <p className="text-body-sm text-orbflow-text-faint mb-1">{field.label}</p>
      )}
      <div className="mt-1">
        <FieldValueDisplay value={displayValue} />
      </div>
    </div>
  );
}

/** Renders a single value inline -- handles primitives, URLs, objects */
function FieldValueDisplay({ value }: { value: unknown }) {
  if (value === undefined || value === null || value === "") {
    return (
      <span className="text-body-sm italic text-orbflow-text-ghost/50">
        (empty)
      </span>
    );
  }
  if (typeof value === "boolean") {
    return (
      <span
        className={cn(
          "inline-flex items-center px-1.5 py-0.5 rounded text-body-sm font-medium",
          value
            ? "bg-emerald-500/10 text-emerald-400"
            : "bg-rose-500/10 text-rose-400",
        )}
      >
        {String(value)}
      </span>
    );
  }
  if (typeof value === "number") {
    return (
      <span className="text-body font-mono tabular-nums text-orbflow-text-secondary">
        {value}
      </span>
    );
  }
  if (typeof value === "string") {
    if (/^https?:\/\//.test(value)) {
      return (
        <a
          href={value}
          target="_blank"
          rel="noopener noreferrer"
          className="text-body font-mono text-neon-cyan/80 hover:text-neon-cyan underline break-all block"
          title={value}
        >
          {value}
        </a>
      );
    }
    // Short strings: inline display
    if (value.length <= 120 && !value.includes("\n")) {
      return (
        <span className="text-body text-orbflow-text-secondary block break-words">
          {value}
        </span>
      );
    }
    // Long or multiline strings: scrollable code block
    return (
      <pre
        className={cn(
          "text-body-sm font-mono text-orbflow-text-secondary leading-relaxed",
          "whitespace-pre-wrap break-words",
          "bg-orbflow-surface/30 rounded-lg p-3 border border-orbflow-border/30",
          "max-h-[240px] overflow-y-auto custom-scrollbar",
        )}
      >
        {value}
      </pre>
    );
  }
  // Complex value -- render as mini JSON
  return (
    <pre className="text-body-sm font-mono text-orbflow-text-faint bg-orbflow-surface/30 rounded-lg p-3 border border-orbflow-border/30 overflow-x-auto max-h-[240px] overflow-y-auto custom-scrollbar leading-relaxed whitespace-pre-wrap break-words">
      {JSON.stringify(value, null, 2)}
    </pre>
  );
}

/* -- Read-Only Parameter Field ------------------------ */

function ReadOnlyParameterField({
  field,
  value,
}: {
  field: FieldSchema;
  value: unknown;
}) {
  // Credential fields are always masked
  if (field.type === "credential" || isCredentialKey(field.key)) {
    return (
      <div className="space-y-1.5">
        <div className="flex items-center gap-2">
          <NodeIcon name="key" className="w-3.5 h-3.5 text-amber-400/70" />
          <label className="text-body-lg font-medium text-orbflow-text-muted">
            {field.label}
          </label>
        </div>
        {field.description && (
          <p className="text-body-sm leading-relaxed text-orbflow-text-faint">
            {field.description}
          </p>
        )}
        <input
          type="text"
          value={"\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022"}
          disabled
          className="w-full rounded-lg px-3.5 py-2.5 text-body-lg
            border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-ghost
            cursor-not-allowed opacity-70"
        />
      </div>
    );
  }

  const displayValue =
    value === undefined || value === null ? "" : String(value);

  return (
    <div className="space-y-1.5">
      <label className="text-body-lg font-medium text-orbflow-text-muted">
        {field.label}
        {field.required && (
          <span className="text-rose-400/70 ml-0.5">*</span>
        )}
      </label>
      {field.description && (
        <p className="text-body-sm leading-relaxed text-orbflow-text-faint">
          {field.description}
        </p>
      )}
      {field.enum ? (
        <select
          value={displayValue}
          disabled
          className="w-full rounded-lg px-3.5 py-2.5 text-body-lg
            border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary
            cursor-not-allowed opacity-70"
        >
          <option value="">(none)</option>
          {field.enum.map((opt) => (
            <option key={opt} value={opt}>
              {opt}
            </option>
          ))}
        </select>
      ) : typeof value === "boolean" ? (
        <div className="flex items-center gap-2 py-1">
          <input
            type="checkbox"
            checked={value}
            disabled
            className="rounded cursor-not-allowed opacity-70"
          />
          <span className="text-body text-orbflow-text-secondary">
            {value ? "Enabled" : "Disabled"}
          </span>
        </div>
      ) : (
        <input
          type="text"
          value={displayValue}
          disabled
          className="w-full rounded-lg px-3.5 py-2.5 text-body-lg
            border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary
            cursor-not-allowed opacity-70"
        />
      )}
    </div>
  );
}

/* -- Fallback Parameter (no schema) ------------------- */

function FallbackParameterField({
  paramKey,
  value,
}: {
  paramKey: string;
  value: unknown;
}) {
  const isSensitive = isCredentialKey(paramKey);
  const displayValue = isSensitive
    ? "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022"
    : value === undefined || value === null
      ? ""
      : String(value);

  return (
    <div className="space-y-1.5">
      <label className="text-body-lg font-medium text-orbflow-text-muted">
        {paramKey}
      </label>
      <input
        type="text"
        value={displayValue}
        disabled
        className="w-full rounded-lg px-3.5 py-2.5 text-body-lg
          border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary
          cursor-not-allowed opacity-70"
      />
    </div>
  );
}

/* -- Main Component ----------------------------------- */

function ReadOnlyNodeConfig({
  nodeId,
  nodeState,
  workflowNode,
  schema,
  onClose,
}: ReadOnlyNodeConfigProps) {
  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef);
  const titleId = useId();

  const status = STATUS_THEMES[nodeState.status] ?? FALLBACK_THEME;

  // Escape key to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const handleBackdropClick = useCallback(
    (e: React.MouseEvent) => {
      if (
        modalRef.current &&
        !modalRef.current.contains(e.target as HTMLElement)
      ) {
        onClose();
      }
    },
    [onClose],
  );

  // Compute duration
  const duration = useMemo(() => {
    if (nodeState.started_at && nodeState.ended_at) {
      return formatDurationRange(nodeState.started_at, nodeState.ended_at);
    }
    return null;
  }, [nodeState.started_at, nodeState.ended_at]);

  // Resolve input data with schema labels
  const inputEntries = useMemo(() => {
    if (!nodeState.input) return [];
    if (schema && schema.inputs.length > 0) {
      return schema.inputs.map((field) => ({
        field,
        value: nodeState.input?.[field.key],
      }));
    }
    return null; // fallback to raw
  }, [nodeState.input, schema]);

  // Resolve output data with schema labels
  const outputEntries = useMemo(() => {
    if (!nodeState.output) return [];
    if (schema && schema.outputs.length > 0) {
      return schema.outputs.map((field) => ({
        field,
        value: nodeState.output?.[field.key],
      }));
    }
    return null; // fallback to raw
  }, [nodeState.output, schema]);

  // Resolve parameter data with schema definitions
  const paramEntries = useMemo(() => {
    if (!nodeState.parameters) return [];
    if (schema?.parameters && schema.parameters.length > 0) {
      return schema.parameters.map((field) => ({
        field,
        value: nodeState.parameters?.[field.key],
      }));
    }
    return null; // fallback to raw
  }, [nodeState.parameters, schema]);

  const nodeKind = schema?.nodeKind || "action";

  return createPortal(
    <div
      className="fixed inset-0 z-[80] flex items-center justify-center"
      style={{ animation: "modalBackdropIn 0.2s ease both" }}
      onClick={handleBackdropClick}
    >
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />

      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="relative w-full max-w-[95vw] max-h-[92vh] m-4 rounded-2xl shadow-2xl flex flex-col overflow-hidden border border-orbflow-border bg-orbflow-surface"
        style={{
          animation:
            "modalSlideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1) both",
        }}
      >
        {/* -- Header bar ---------------------------- */}
        <div className="flex items-center gap-3 px-5 h-12 shrink-0 border-b border-orbflow-border bg-orbflow-surface">
          <div className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 bg-orbflow-add-btn-bg">
            <NodeIcon
              name={schema?.icon || "default"}
              className="w-4 h-4"
              style={{
                color: schema?.color || "var(--orbflow-text-muted)",
              }}
            />
          </div>
          <h2
            id={titleId}
            className="text-heading font-semibold truncate text-orbflow-text-secondary"
          >
            {workflowNode.name || nodeId}
          </h2>
          {nodeKind !== "action" && (
            <span
              className="text-micro font-bold uppercase tracking-[0.1em] px-1.5 py-0.5 rounded border shrink-0"
              style={{
                color:
                  nodeKind === "trigger" ? "var(--orbflow-exec-completed)" : "var(--orbflow-exec-active)",
                borderColor:
                  nodeKind === "trigger"
                    ? "rgba(16, 185, 129, 0.15)"
                    : "rgba(74, 154, 175, 0.15)",
                backgroundColor:
                  nodeKind === "trigger"
                    ? "rgba(16, 185, 129, 0.03)"
                    : "rgba(74, 154, 175, 0.03)",
              }}
            >
              {nodeKind}
            </span>
          )}
          <div className="flex-1" />

          {/* Status badge */}
          <span
            className="flex items-center gap-1.5 text-body-sm font-semibold px-2.5 py-1 rounded-lg shrink-0"
            style={{
              color: status.text,
              backgroundColor: `rgba(${status.accentRgb},0.08)`,
              border: `1px solid rgba(${status.accentRgb},0.15)`,
            }}
          >
            <span
              className="w-2 h-2 rounded-full"
              style={{ backgroundColor: status.accent }}
            />
            {status.label}
          </span>

          <button
            onClick={onClose}
            aria-label="Close node details"
            className="w-7 h-7 rounded-lg flex items-center justify-center hover:bg-orbflow-surface-hover transition-all shrink-0 text-orbflow-text-faint
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          >
            <NodeIcon name="x" className="w-4 h-4" />
          </button>
        </div>

        {/* -- Three-panel body ----------------------- */}
        {(() => {
          const hasParams = nodeState.parameters && Object.keys(nodeState.parameters).length > 0;
          const hasSchemaParams = paramEntries !== null && paramEntries.length > 0;
          const showParamsPanel = hasParams || hasSchemaParams;
          return (
        <div className="flex-1 flex min-h-[300px] max-h-[calc(92vh-88px)]">
          {/* == LEFT PANEL -- INPUT ================== */}
          <div className={cn("flex flex-col shrink-0 border-r border-orbflow-border bg-orbflow-surface", showParamsPanel ? "w-[280px] min-w-[240px]" : "w-1/2 min-w-[300px]")}>
            <div className="px-4 pt-3 pb-2 border-b border-orbflow-border">
              <div className="text-body-sm font-bold uppercase tracking-[0.15em] text-orbflow-text-faint">
                Input
              </div>
            </div>
            <div className="flex-1 overflow-y-auto custom-scrollbar px-3 py-3">
              {nodeState.input &&
              Object.keys(nodeState.input).length > 0 ? (
                inputEntries !== null ? (
                  <div className="space-y-1">
                    {inputEntries.map(({ field, value }) => (
                      <LabeledField
                        key={field.key}
                        field={field}
                        value={value}
                      />
                    ))}
                    {/* Show extra keys not in schema */}
                    {Object.keys(nodeState.input)
                      .filter(
                        (k) =>
                          !schema?.inputs.some(
                            (f) => f.key === k,
                          ),
                      )
                      .map((k) => (
                        <div
                          key={k}
                          className="px-2.5 py-2 rounded-lg"
                        >
                          <span className="text-body font-mono text-orbflow-text-ghost">
                            {k}:
                          </span>{" "}
                          <FieldValueDisplay
                            value={nodeState.input?.[k]}
                          />
                        </div>
                      ))}
                  </div>
                ) : (
                  <StructuredOutput
                    data={nodeState.input}
                    pluginRef={workflowNode.plugin_ref}
                  />
                )
              ) : (
                <div className="flex flex-col items-center justify-center h-full text-center px-4">
                  <NodeIcon
                    name="inbox"
                    className="w-8 h-8 mb-3 text-orbflow-text-ghost"
                  />
                  <p className="text-body font-medium text-orbflow-text-faint">
                    No input data
                  </p>
                </div>
              )}
            </div>
          </div>

          {/* == CENTER PANEL -- PARAMETERS (hidden when empty) =========== */}
          {showParamsPanel && (
          <div className="flex-1 flex flex-col min-w-0 bg-orbflow-surface">
            <div className="px-4 pt-3 pb-2 border-b border-orbflow-border">
              <div className="text-body-sm font-bold uppercase tracking-[0.15em] text-orbflow-text-faint">
                Parameters
              </div>
            </div>
            <div className="flex-1 overflow-y-auto custom-scrollbar">
              <div className="max-w-[640px] mx-auto px-6 py-5 space-y-5">
                {nodeState.parameters &&
                Object.keys(nodeState.parameters).length > 0 ? (
                  paramEntries !== null ? (
                    <div className="space-y-4">
                      <div className="flex items-center gap-2">
                        <NodeIcon
                          name="settings"
                          className="w-3.5 h-3.5 text-orbflow-text-faint"
                        />
                        <h4 className="text-body-sm font-bold uppercase tracking-[0.12em] text-orbflow-text-faint">
                          Parameters
                        </h4>
                        <div className="flex-1 h-px bg-orbflow-border" />
                      </div>
                      {paramEntries.map(({ field, value }) => (
                        <ReadOnlyParameterField
                          key={field.key}
                          field={field}
                          value={value}
                        />
                      ))}
                      {/* Show extra keys not in schema */}
                      {Object.keys(nodeState.parameters)
                        .filter(
                          (k) =>
                            !schema?.parameters?.some(
                              (f) => f.key === k,
                            ),
                        )
                        .map((k) => (
                          <FallbackParameterField
                            key={k}
                            paramKey={k}
                            value={
                              nodeState.parameters?.[k]
                            }
                          />
                        ))}
                    </div>
                  ) : (
                    /* No schema -- raw display with masking */
                    <StructuredOutput
                      data={maskAllCredentialValues(
                        nodeState.parameters,
                      )}
                      pluginRef={workflowNode.plugin_ref}
                    />
                  )
                ) : (
                  <div className="text-center py-12">
                    <NodeIcon
                      name="settings"
                      className="w-8 h-8 mx-auto mb-3 text-orbflow-text-ghost"
                    />
                    <p className="text-body text-orbflow-text-faint">
                      No parameters for this execution
                    </p>
                  </div>
                )}
              </div>
            </div>
          </div>
          )}

          {/* == RIGHT PANEL -- OUTPUT ================ */}
          <div className={cn("flex flex-col shrink-0 border-l border-orbflow-border bg-orbflow-surface", showParamsPanel ? "w-[280px] min-w-[240px]" : "w-1/2 min-w-[300px]")}>
            <div className="px-4 pt-3 pb-2 border-b border-orbflow-border">
              <div className="text-body-sm font-bold uppercase tracking-[0.15em] text-orbflow-text-faint">
                Output
              </div>
            </div>
            <div className="flex-1 overflow-y-auto custom-scrollbar px-3 py-3">
              {nodeState.output &&
              Object.keys(nodeState.output).length > 0 ? (
                outputEntries !== null ? (
                  <div className="space-y-1">
                    {outputEntries.map(({ field, value }) => (
                      <LabeledField
                        key={field.key}
                        field={field}
                        value={value}
                      />
                    ))}
                    {/* Show extra keys not in schema */}
                    {Object.keys(nodeState.output)
                      .filter(
                        (k) =>
                          !schema?.outputs.some(
                            (f) => f.key === k,
                          ),
                      )
                      .map((k) => (
                        <div
                          key={k}
                          className="px-2.5 py-2 rounded-lg"
                        >
                          <span className="text-body font-mono text-orbflow-text-ghost">
                            {k}:
                          </span>{" "}
                          <FieldValueDisplay
                            value={nodeState.output?.[k]}
                          />
                        </div>
                      ))}
                  </div>
                ) : (
                  <StructuredOutput
                    data={nodeState.output}
                    pluginRef={workflowNode.plugin_ref}
                  />
                )
              ) : nodeState.status === "failed" && nodeState.error ? (
                <div className="space-y-3 p-2">
                  <div className="rounded-lg border border-rose-500/20 p-3 bg-rose-500/5">
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
                    <p className="text-body-sm text-orbflow-text-ghost">
                      Failed after {nodeState.attempt} attempts
                    </p>
                  )}
                </div>
              ) : (
                <div className="flex flex-col items-center justify-center h-full text-center px-4">
                  <NodeIcon
                    name="send"
                    className="w-8 h-8 mb-3 text-orbflow-text-ghost"
                  />
                  <p className="text-body font-medium text-orbflow-text-faint">
                    No output data
                  </p>
                  {nodeState.status === "running" && (
                    <p className="text-caption mt-1 text-orbflow-text-ghost">
                      Output will appear when the step completes
                    </p>
                  )}
                </div>
              )}
            </div>
          </div>
        </div>
          );
        })()}

        {/* -- Footer bar ---------------------------- */}
        <div className="flex items-center gap-4 px-5 h-10 shrink-0 border-t border-orbflow-border bg-orbflow-surface text-body-sm text-orbflow-text-ghost">
          {duration && (
            <span className="flex items-center gap-1.5">
              <NodeIcon name="clock" className="w-3 h-3" />
              Duration: {duration}
            </span>
          )}
          {nodeState.attempt > 0 && (
            <span className="flex items-center gap-1.5">
              <NodeIcon name="repeat" className="w-3 h-3" />
              Attempt: {nodeState.attempt}
            </span>
          )}
          <div className="flex-1" />
          <span className="font-mono text-caption text-orbflow-text-ghost/50 px-2 py-0.5 rounded bg-orbflow-add-btn-bg border border-orbflow-border/30">
            {workflowNode.plugin_ref}
          </span>
        </div>
      </div>
    </div>,
    document.body,
  );
}

/** Mask credential-like values in a plain record (no schema available). */
function maskAllCredentialValues(
  params: Record<string, unknown>,
): Record<string, unknown> {
  const masked: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(params)) {
    masked[key] = isCredentialKey(key)
      ? "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022"
      : value;
  }
  return masked;
}

export { ReadOnlyNodeConfig };
