"use client";

import { useMemo, useCallback, useState, useEffect, useRef, useId } from "react";
import { createPortal } from "react-dom";
import { useOrbflow } from "../context/orbflow-provider";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { useCanvasStore, usePanelStore, useWorkflowStore } from "@orbflow/core/stores";
import { resolveUpstreamOutputs } from "../utils/upstream";
import { MappingField } from "./config-panel/mapping-field";
import { FieldBrowser } from "./config-panel/field-browser";
import { NodeIcon, getTypeColor, getTypeLabel } from "./icons";
import { cn } from "../utils/cn";
import { ParameterField, SubWorkflowPicker } from "./node-config-fields";
import { useExecutionOverlayStore } from "@orbflow/core/stores";
import { useNodeOutputCacheStore } from "@orbflow/core/stores";
import type { FieldMapping, ParameterValue, NodeKind } from "../types/schema";

interface NodeConfigModalProps {
  nodeId: string;
  onClose: () => void;
  onTestNode?: (nodeId: string) => void;
  testingNodeId?: string | null;
  workflowId?: string;
}

export function NodeConfigModal({ nodeId, onClose, onTestNode, testingNodeId, workflowId }: NodeConfigModalProps) {
  const { registry } = useOrbflow();
  const { nodes, edges } = useCanvasStore();
  const { getNodeMappings, setInputMapping, getNodeParameters, setParameterValue } = usePanelStore();
  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef);
  const titleId = useId();
  const [centerTab, setCenterTab] = useState<"parameters" | "settings">("parameters");
  const [activeFieldKey, setActiveFieldKey] = useState<string | null>(null);

  const node = useMemo(() => nodes.find((n) => n.id === nodeId), [nodes, nodeId]);
  const pluginRef = (node?.data?.pluginRef as string) || "";
  const schema = useMemo(() => registry.get(pluginRef), [registry, pluginRef]);
  const isSubWorkflow = pluginRef === "builtin:sub-workflow";
  const nodeKind: NodeKind = (schema?.nodeKind as NodeKind) || "action";

  // Build runtime outputs map from execution overlay + persistent cache
  const storeWorkflowId = useWorkflowStore((s) => s.selectedWorkflow?.id);
  const effectiveWorkflowId = workflowId || storeWorkflowId;
  const nodeStatuses = useExecutionOverlayStore((s) => s.nodeStatuses);
  const cachedOutputs = useNodeOutputCacheStore((s) => effectiveWorkflowId ? s.cache[effectiveWorkflowId] : undefined);
  const runtimeOutputs = useMemo(() => {
    const outputs: Record<string, Record<string, unknown>> = {};
    // Persistent cache (lower priority)
    if (cachedOutputs) {
      for (const [nid, out] of Object.entries(cachedOutputs)) {
        outputs[nid] = out;
      }
    }
    // Execution overlay (higher priority, overwrites cache)
    for (const [nid, ns] of Object.entries(nodeStatuses)) {
      if (ns.output) outputs[nid] = ns.output;
    }
    return Object.keys(outputs).length > 0 ? outputs : undefined;
  }, [nodeStatuses, cachedOutputs]);

  const upstream = useMemo(
    () => resolveUpstreamOutputs(nodeId, nodes, edges, registry, runtimeOutputs),
    [nodeId, nodes, edges, registry, runtimeOutputs],
  );

  const mappings = getNodeMappings(nodeId);
  const parameters = getNodeParameters(nodeId);

  const wiredFields = useMemo(() => {
    const wired: Record<string, { sourceNodeId: string; sourceField: string }> = {};
    for (const edge of edges) {
      if (edge.target !== nodeId) continue;
      const sf = (edge.data?.sourceField as string) || "";
      const tf = (edge.data?.targetField as string) || "";
      if (sf && tf) wired[tf] = { sourceNodeId: edge.source, sourceField: sf };
    }
    return wired;
  }, [edges, nodeId]);

  const handleMappingChange = useCallback(
    (mapping: FieldMapping) => setInputMapping(nodeId, mapping.targetKey, mapping),
    [nodeId, setInputMapping],
  );

  const handleParameterChange = useCallback(
    (key: string, value: ParameterValue) => setParameterValue(nodeId, key, value),
    [nodeId, setParameterValue],
  );

  // Click-to-insert: when a field is clicked in the left panel's Available Data browser,
  // insert it as a CEL expression into the active (or first available) input mapping field.
  const handleFieldBrowserSelect = useCallback(
    (_nodeIdSource: string, _path: string, celPath: string) => {
      const inputs = schema?.inputs || [];
      if (inputs.length === 0) return;

      // Determine which field to target: activeFieldKey > first input
      const targetKey = activeFieldKey && inputs.some((f) => f.key === activeFieldKey)
        ? activeFieldKey
        : inputs[0].key;

      setInputMapping(nodeId, targetKey, {
        targetKey,
        mode: "expression",
        sourceNodeId: _nodeIdSource === "__context__" ? undefined : _nodeIdSource,
        sourcePath: _path,
        celExpression: celPath,
      });
    },
    [schema, activeFieldKey, nodeId, setInputMapping],
  );

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const handleBackdropClick = useCallback(
    (e: React.MouseEvent) => {
      if (modalRef.current && !modalRef.current.contains(e.target as HTMLElement)) onClose();
    },
    [onClose],
  );

  if (!node || !schema) {
    return createPortal(
      <div
        className="fixed inset-0 z-[80] flex items-center justify-center bg-black/60 backdrop-blur-sm"
        style={{ animation: "modalBackdropIn 0.2s ease both" }}
        onClick={onClose}
      >
        <div className="rounded-2xl p-12 text-center border border-orbflow-border bg-orbflow-surface">
          <NodeIcon name="help-circle" className="w-10 h-10 mx-auto mb-3 text-orbflow-text-ghost" />
          <p className="text-xs text-orbflow-text-faint">{!node ? "Step not found" : "Unknown step type"}</p>
        </div>
      </div>,
      document.body,
    );
  }

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
        className="relative w-full h-full max-w-[95vw] max-h-[92vh] m-4 rounded-2xl shadow-2xl flex flex-col overflow-hidden border border-orbflow-border bg-orbflow-surface"
        style={{ animation: "modalSlideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1) both" }}
      >
        {/* -- Header bar ---------------------------- */}
        <div className="flex items-center gap-3 px-5 h-12 shrink-0 border-b border-orbflow-border bg-orbflow-surface">
          <div className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0 bg-orbflow-add-btn-bg">
            <NodeIcon name={schema.icon || "default"} className="w-4 h-4" style={{ color: schema.color || "var(--orbflow-text-muted)" }} />
          </div>
          <h2 id={titleId} className="text-heading font-semibold truncate text-orbflow-text-secondary">
            {(node.data.label as string) || schema.name}
          </h2>
          {nodeKind !== "action" && (
            <span
              className="text-micro font-bold uppercase tracking-[0.1em] px-1.5 py-0.5 rounded border shrink-0"
              style={{
                color: nodeKind === "trigger" ? "var(--orbflow-exec-completed)" : "var(--orbflow-exec-active)",
                borderColor: nodeKind === "trigger" ? "rgba(16, 185, 129, 0.15)" : "rgba(74, 154, 175, 0.15)",
                backgroundColor: nodeKind === "trigger" ? "rgba(16, 185, 129, 0.03)" : "rgba(74, 154, 175, 0.03)",
              }}
            >
              {nodeKind}
            </span>
          )}
          <div className="flex-1" />
          {onTestNode && nodeKind !== "trigger" && (
            <button
              onClick={() => onTestNode(nodeId)}
              disabled={!!testingNodeId}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium transition-all shrink-0",
                "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                testingNodeId === nodeId
                  ? "bg-electric-indigo/10 text-electric-indigo cursor-wait"
                  : "bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 border border-emerald-500/20"
              )}
              title="Run this node to discover output structure"
            >
              {testingNodeId === nodeId ? (
                <>
                  <NodeIcon name="loader" className="w-3 h-3 animate-spin" />
                  Testing...
                </>
              ) : (
                <>
                  <NodeIcon name="play" className="w-3 h-3" />
                  Test
                </>
              )}
            </button>
          )}
          <button
            onClick={onClose}
            aria-label="Close configuration"
            className="w-7 h-7 rounded-lg flex items-center justify-center hover:bg-orbflow-surface-hover transition-all shrink-0 text-orbflow-text-faint
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          >
            <NodeIcon name="x" className="w-4 h-4" />
          </button>
        </div>

        {/* -- Three-panel body ----------------------- */}
        <div className="flex-1 flex min-h-0">
          {/* == LEFT PANEL -- INPUT ================== */}
          <div className="w-[280px] min-w-[240px] flex flex-col shrink-0 border-r border-orbflow-border bg-orbflow-surface">
            <div className="px-4 pt-3 pb-2 border-b border-orbflow-border">
              <div className="text-body-sm font-bold uppercase tracking-[0.15em] text-orbflow-text-faint">Available Data</div>
            </div>
            <div className="flex-1 overflow-y-auto custom-scrollbar px-3 py-3">
              {upstream.length > 0 ? (
                <FieldBrowser upstream={upstream} onSelect={handleFieldBrowserSelect} />
              ) : (
                <div className="flex flex-col items-center justify-center h-full text-center px-4">
                  <NodeIcon name="inbox" className="w-8 h-8 mb-3 text-orbflow-text-ghost" />
                  <p className="text-body font-medium text-orbflow-text-faint">No input connected</p>
                  <p className="text-caption mt-1 text-orbflow-text-ghost">Connect upstream nodes to see their output data</p>
                </div>
              )}
            </div>
          </div>

          {/* == CENTER PANEL -- PARAMETERS =========== */}
          <div className="flex-1 flex flex-col min-w-0 bg-orbflow-surface">
            <div className="flex items-center gap-0 px-5 shrink-0 border-b border-orbflow-border">
              <button
                onClick={() => setCenterTab("parameters")}
                className={cn("px-4 py-3 text-body-lg font-medium border-b-2 transition-all",
                  "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                  centerTab === "parameters" ? "text-electric-indigo border-electric-indigo" : "border-transparent text-orbflow-text-faint")}
              >
                Parameters
              </button>
              {schema.settings && schema.settings.length > 0 && (
                <button
                  onClick={() => setCenterTab("settings")}
                  className={cn("px-4 py-3 text-body-lg font-medium border-b-2 transition-all",
                    "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                    centerTab === "settings" ? "text-electric-indigo border-electric-indigo" : "border-transparent text-orbflow-text-faint")}
                >
                  Settings
                </button>
              )}
              <div className="flex-1" />
              <div className="flex items-center gap-2 py-2">
                <span className="text-caption text-orbflow-text-faint">Name:</span>
                <input
                  type="text"
                  value={(node.data.label as string) || ""}
                  onChange={(e) => useCanvasStore.getState().updateNodeData(nodeId, { label: e.target.value })}
                  className="rounded-lg text-body px-2.5 py-1 transition-colors w-36 bg-orbflow-add-btn-bg border border-orbflow-border text-orbflow-text-secondary
                    focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                />
              </div>
            </div>

            <div className="flex-1 overflow-y-auto custom-scrollbar">
              <div className="max-w-[640px] mx-auto px-6 py-5 space-y-5">
                {centerTab === "parameters" && (
                  <>
                    {isSubWorkflow && (
                      <SubWorkflowPicker
                        nodeId={nodeId}
                        currentWorkflowId={mappings["workflow_id"]?.staticValue as string}
                        onChange={(wfId) => setInputMapping(nodeId, "workflow_id", { targetKey: "workflow_id", mode: "static", staticValue: wfId })}
                      />
                    )}

                    {schema.parameters && schema.parameters.length > 0 && (
                      <div className="space-y-4">
                        <SectionHeader icon="settings" label="Parameters" />
                        {schema.parameters.map((param) => (
                          <ParameterField key={param.key} param={param} value={parameters[param.key]} upstream={upstream}
                            onChange={(val) => handleParameterChange(param.key, val)} />
                        ))}
                      </div>
                    )}

                    {schema.inputs.length > 0 && (
                      <div className="space-y-4">
                        <SectionHeader icon="arrow-right" label="Input Mappings" />
                        {schema.inputs.map((field) => {
                          const wired = wiredFields[field.key];
                          const sourceNodeLabel = wired
                            ? (nodes.find((n) => n.id === wired.sourceNodeId)?.data?.label as string) || wired.sourceNodeId
                            : undefined;
                          return (
                            <MappingField
                              key={field.key} field={field} mapping={mappings[field.key]}
                              upstream={upstream} onChange={handleMappingChange}
                              onFocus={setActiveFieldKey}
                              wiredFrom={wired ? { sourceNodeId: wired.sourceNodeId, sourceNodeLabel: sourceNodeLabel || wired.sourceNodeId, sourceField: wired.sourceField } : undefined}
                            />
                          );
                        })}
                      </div>
                    )}

                    {schema.capabilityPorts && schema.capabilityPorts.length > 0 && (
                      <div className="space-y-3">
                        <SectionHeader icon="plug" label="Connections" />
                        {schema.capabilityPorts.map((port) => (
                          <div key={port.key} className="flex items-center gap-2.5 px-3 py-2.5 rounded-xl bg-orbflow-add-btn-bg border border-orbflow-border">
                            <div className="w-2.5 h-2.5 rounded-sm rotate-45 bg-sky-400/40 shrink-0" />
                            <div className="flex-1 min-w-0">
                              <span className="text-body text-orbflow-text-muted">{port.key}</span>
                              {port.description && <p className="text-caption mt-0.5 text-orbflow-text-faint">{port.description}</p>}
                            </div>
                            <span className="text-micro font-mono text-sky-300/30">{port.capabilityType}</span>
                            {port.required && <span className="text-micro text-rose-400/50 font-medium">req</span>}
                          </div>
                        ))}
                      </div>
                    )}

                    {!schema.parameters?.length && !schema.inputs.length && !schema.capabilityPorts?.length && !isSubWorkflow && (
                      <div className="text-center py-12">
                        <NodeIcon name="settings" className="w-8 h-8 mx-auto mb-3 text-orbflow-text-ghost" />
                        <p className="text-body text-orbflow-text-faint">No parameters for this step</p>
                      </div>
                    )}

                    {/* -- Approval Gate Toggle -------------------- */}
                    {nodeKind !== "trigger" && (
                      <ApprovalGateToggle
                        enabled={!!(node.data.requiresApproval as boolean)}
                        onChange={(enabled) =>
                          useCanvasStore.getState().updateNodeData(nodeId, { requiresApproval: enabled })
                        }
                      />
                    )}
                  </>
                )}

                {centerTab === "settings" && schema.settings?.map((field) => (
                  <ParameterField key={field.key} param={field} value={parameters[field.key]} upstream={upstream}
                    onChange={(val) => handleParameterChange(field.key, val)} />
                ))}
              </div>
            </div>
          </div>

          {/* == RIGHT PANEL -- OUTPUT ================ */}
          <div className="w-[280px] min-w-[240px] flex flex-col shrink-0 border-l border-orbflow-border bg-orbflow-surface">
            <div className="px-4 pt-3 pb-2 border-b border-orbflow-border">
              <div className="text-body-sm font-bold uppercase tracking-[0.15em] text-orbflow-text-faint">Output</div>
            </div>
            <div className="flex-1 overflow-y-auto custom-scrollbar px-3 py-3">
              {schema.outputs.length > 0 ? (
                <div className="space-y-0.5">
                  {schema.outputs.some((f) => f.dynamic) && (
                    <div className="mb-2 px-3 py-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
                      <p className="text-body-sm text-amber-400/60">
                        <NodeIcon name="zap" className="w-3 h-3 inline mr-1" />
                        Some outputs are dynamic. Execute this node to see actual data.
                      </p>
                    </div>
                  )}
                  {schema.outputs.map((field) => (
                    <div key={field.key} className="px-2.5 py-2 rounded-lg hover:bg-orbflow-surface-hover transition-colors">
                      <div className="flex items-center gap-2">
                        <span className="text-caption font-bold px-1.5 py-0.5 rounded shrink-0"
                          style={{ color: getTypeColor(field.type), backgroundColor: getTypeColor(field.type) + "12" }}>
                          {getTypeLabel(field.type)}
                        </span>
                        <span className="text-body font-mono truncate text-orbflow-text-muted">{field.key}</span>
                        {field.dynamic && (
                          <span className="text-[9px] font-medium px-1.5 py-0.5 rounded
                            bg-amber-500/10 text-amber-400/70 border border-amber-500/15 shrink-0"
                            title="Actual structure determined at runtime">
                            dynamic
                          </span>
                        )}
                      </div>
                      {field.description && (
                        <p className="text-caption mt-1 ml-[calc(0.625rem+theme(spacing.2))] leading-relaxed text-orbflow-text-faint">{field.description}</p>
                      )}
                      {field.dynamic && (
                        <p className="text-caption mt-0.5 ml-[calc(0.625rem+theme(spacing.2))] leading-relaxed text-amber-400/40 italic">
                          Execute this node to see the actual output structure
                        </p>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="flex flex-col items-center justify-center h-full text-center px-4">
                  <NodeIcon name="send" className="w-8 h-8 mb-3 text-orbflow-text-ghost" />
                  <p className="text-body font-medium text-orbflow-text-faint">No output data</p>
                  <p className="text-caption mt-1 text-orbflow-text-ghost">Output will appear here once the step is run</p>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

/* -- Section header helper ------------------------- */

function SectionHeader({ icon, label }: { icon: string; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <NodeIcon name={icon} className="w-3.5 h-3.5 text-orbflow-text-faint" />
      <h4 className="text-body-sm font-bold uppercase tracking-[0.12em] text-orbflow-text-faint">{label}</h4>
      <div className="flex-1 h-px bg-orbflow-border" />
    </div>
  );
}

/* -- Approval Gate Toggle ------------------------- */

function ApprovalGateToggle({
  enabled,
  onChange,
}: {
  enabled: boolean;
  onChange: (enabled: boolean) => void;
}) {
  return (
    <div className="rounded-xl border border-orbflow-border bg-orbflow-add-btn-bg p-4 space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <NodeIcon name="shield" className="w-4 h-4 text-orbflow-text-faint" />
          <div>
            <span className="text-body font-medium text-orbflow-text-secondary">Requires Approval</span>
          </div>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          onClick={() => onChange(!enabled)}
          className={cn(
            "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors duration-200",
            "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
            enabled ? "bg-electric-indigo" : "bg-orbflow-border"
          )}
        >
          <span
            className={cn(
              "pointer-events-none inline-block h-3.5 w-3.5 rounded-full bg-white shadow-sm transition-transform duration-200",
              enabled ? "translate-x-[18px]" : "translate-x-[3px]"
            )}
          />
        </button>
      </div>
      <p className="text-caption leading-relaxed text-orbflow-text-ghost">
        When enabled, execution pauses at this node until a human approves or rejects it.
      </p>
      {enabled && (
        <div
          className="flex items-start gap-2 rounded-lg px-3 py-2.5 border border-electric-indigo/15 bg-electric-indigo/[0.04]"
          style={{ animation: "fadeInUp 0.2s ease both" }}
        >
          <NodeIcon name="info" className="w-3.5 h-3.5 mt-0.5 shrink-0 text-electric-indigo/60" />
          <p className="text-caption leading-relaxed text-electric-indigo/70">
            Approvers can review the node&apos;s input data before execution proceeds.
          </p>
        </div>
      )}
    </div>
  );
}
