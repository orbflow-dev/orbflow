"use client";

import { useState, useEffect } from "react";
import { NodeIcon } from "./icons";
import { useOrbflow } from "../context/orbflow-provider";
import { createApiClient } from "@orbflow/core";
import { useToastStore } from "@orbflow/core/stores";
import { cn } from "../utils/cn";
import type { ParameterValue } from "../types/schema";
import type { UpstreamOutput } from "../utils/upstream";
import type { Workflow } from "@orbflow/core";

/* -- n8n-style Fixed/Expression segmented toggle -- */

function ModeToggle({
  mode,
  onToggle,
}: {
  mode: "static" | "expression";
  onToggle: () => void;
}) {
  return (
    <div className="inline-flex rounded-md overflow-hidden border border-orbflow-border">
      <button
        onClick={mode === "expression" ? onToggle : undefined}
        aria-pressed={mode === "static"}
        className={cn(
          "px-2.5 py-1 text-body-sm font-medium transition-all",
          "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
          mode === "static" ? "bg-electric-indigo/15 text-electric-indigo" : "text-orbflow-text-faint",
        )}
      >
        Fixed
      </button>
      <button
        onClick={mode === "static" ? onToggle : undefined}
        aria-pressed={mode === "expression"}
        className={cn(
          "px-2.5 py-1 text-body-sm font-medium transition-all",
          "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
          mode === "expression" ? "bg-electric-indigo/20 text-electric-indigo" : "text-orbflow-text-faint",
        )}
      >
        Expression
      </button>
    </div>
  );
}

/* -- Parameter field with Fixed/Expression toggle -- */

interface ParameterFieldParam {
  key: string;
  label: string;
  type: string;
  required?: boolean;
  default?: unknown;
  description?: string;
  enum?: string[];
  credentialType?: string;
}

export function ParameterField({
  param,
  value,
  upstream,
  onChange,
}: {
  param: ParameterFieldParam;
  value?: ParameterValue;
  upstream: UpstreamOutput[];
  onChange: (value: ParameterValue) => void;
}) {
  const mode = value?.mode || "static";
  const staticVal = value?.value ?? param.default ?? "";
  const [dropHighlight, setDropHighlight] = useState(false);

  const handleDragOver = (e: React.DragEvent) => {
    if (e.dataTransfer.types.includes("application/orbflow-field")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      setDropHighlight(true);
    }
  };

  const handleDragLeave = () => setDropHighlight(false);

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDropHighlight(false);
    const raw = e.dataTransfer.getData("application/orbflow-field");
    if (!raw) return;
    try {
      const { celPath } = JSON.parse(raw) as { celPath: string };
      onChange({ key: param.key, mode: "expression", expression: celPath });
    } catch (err) {
      console.error("[orbflow] Failed to parse drag-drop field data");
    }
  };

  const handleToggle = () => {
    onChange({
      key: param.key,
      mode: mode === "static" ? "expression" : "static",
      value: mode === "static" ? undefined : staticVal,
      expression: mode === "expression" ? undefined : "",
    });
  };

  // Credential type fields render a dedicated selector
  if (param.type === "credential") {
    return (
      <CredentialField
        param={param}
        value={String(staticVal || "")}
        onChange={(credId) => onChange({ key: param.key, mode: "static", value: credId })}
      />
    );
  }

  return (
    <div
      className={`transition-all duration-200 ${
        dropHighlight ? "ring-1 ring-electric-indigo/40 bg-electric-indigo/[0.04] rounded-xl p-3 -m-3" : ""
      }`}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      <div className="flex items-center justify-between mb-2">
        <label className="text-body-lg font-medium text-orbflow-text-muted">
          {param.label}
          {param.required && <span className="text-rose-400/70 ml-0.5">*</span>}
        </label>
        <ModeToggle mode={mode} onToggle={handleToggle} />
      </div>

      {param.description && (
        <p className="text-body-sm mb-2 leading-relaxed text-orbflow-text-faint">{param.description}</p>
      )}

      {mode === "static" ? (
        param.enum ? (
          <select
            value={String(staticVal)}
            onChange={(e) => onChange({ key: param.key, mode: "static", value: e.target.value })}
            className="w-full rounded-lg px-3.5 py-2.5 text-body-lg transition-all border border-orbflow-border bg-orbflow-surface text-orbflow-text-secondary
              focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50
              hover:bg-orbflow-surface-hover cursor-pointer"
          >
            <option value="">Select...</option>
            {param.enum.map((opt) => (
              <option key={opt} value={opt}>{opt}</option>
            ))}
          </select>
        ) : (
          <input
            type={param.type === "number" ? "number" : "text"}
            value={String(staticVal)}
            onChange={(e) =>
              onChange({
                key: param.key,
                mode: "static",
                value: param.type === "number" ? Number(e.target.value) : e.target.value,
              })
            }
            placeholder={param.default !== undefined ? String(param.default) : `Enter ${param.label.toLowerCase()}...`}
            className="w-full rounded-lg px-3.5 py-2.5 text-body-lg transition-all duration-200 border border-orbflow-border bg-orbflow-surface text-orbflow-text-secondary
              placeholder:text-orbflow-text-ghost focus:outline-none focus:border-electric-indigo/30
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50"
          />
        )
      ) : (
        <div className="flex items-center gap-0 rounded-lg border border-electric-indigo/20 overflow-hidden bg-orbflow-surface">
          <div className="flex items-center justify-center w-9 h-10 bg-electric-indigo/10 border-r border-electric-indigo/15 shrink-0">
            <span className="text-body font-bold text-electric-indigo/70 font-mono italic">fx</span>
          </div>
          <input
            type="text"
            value={value?.expression || ""}
            onChange={(e) => onChange({ key: param.key, mode: "expression", expression: e.target.value })}
            placeholder="CEL expression..."
            className="flex-1 bg-transparent px-3 py-2.5 text-body-lg font-mono text-electric-indigo/80 placeholder:text-electric-indigo/20
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none transition-all"
          />
        </div>
      )}
    </div>
  );
}

/* -- Credential selector field --------------------- */

function CredentialField({
  param,
  value,
  onChange,
}: {
  param: { key: string; label: string; credentialType?: string; description?: string };
  value: string;
  onChange: (credentialId: string) => void;
}) {
  const { config } = useOrbflow();
  const [credentials, setCredentials] = useState<{ id: string; name: string; type: string }[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const client = createApiClient(config.apiBaseUrl);
    client.credentials
      .list()
      .then((creds: { id: string; name: string; type: string }[]) => {
        const allowedTypes = param.credentialType
          ? new Set(param.credentialType.split(",").map((t: string) => t.trim()))
          : null;
        setCredentials(allowedTypes ? creds.filter((c) => allowedTypes.has(c.type)) : creds);
        setLoading(false);
      })
      .catch(() => { setLoading(false); });
  }, [param.credentialType, config.apiBaseUrl]);

  return (
    <div>
      <div className="flex items-center gap-2 mb-2">
        <NodeIcon name="key" className="w-3.5 h-3.5 text-amber-400/70" />
        <label className="text-body-lg font-medium text-orbflow-text-muted">{param.label}</label>
      </div>
      {param.description && (
        <p className="text-body-sm mb-2 leading-relaxed text-orbflow-text-faint">{param.description}</p>
      )}
      {loading ? (
        <div className="flex items-center gap-2 py-2.5">
          <NodeIcon name="loader" className="w-3.5 h-3.5 text-orbflow-text-ghost animate-spin" />
          <span className="text-body-sm text-orbflow-text-faint">Loading credentials...</span>
        </div>
      ) : (
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="w-full rounded-lg px-3.5 py-2.5 text-body-lg transition-all border border-orbflow-border bg-orbflow-surface text-orbflow-text-secondary
            focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50
            hover:bg-orbflow-surface-hover cursor-pointer"
        >
          <option value="">No credential (use inline settings)</option>
          {credentials.map((cred) => (
            <option key={cred.id} value={cred.id}>{cred.name}</option>
          ))}
        </select>
      )}
      {credentials.length === 0 && !loading && (
        <p className="text-caption mt-1.5 text-orbflow-text-ghost">
          No {param.credentialType || ""} credentials found. Create one in the Credentials tab.
        </p>
      )}
    </div>
  );
}

/* -- Sub-Workflow picker --------------------------- */

export function SubWorkflowPicker({
  nodeId,
  currentWorkflowId,
  onChange,
}: {
  nodeId: string;
  currentWorkflowId?: string;
  onChange: (workflowId: string) => void;
}) {
  const { config } = useOrbflow();
  const api = createApiClient(config.apiBaseUrl);
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    api.workflows
      .list()
      .then((result) => {
        if (!cancelled) { setWorkflows(result.items || []); setLoading(false); }
      })
      .catch((err) => {
        if (!cancelled) {
          setLoading(false);
          useToastStore.getState().error(
            "Failed to load workflows",
            err instanceof Error ? err.message : "Could not fetch workflow list",
          );
        }
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <div className="rounded-xl border border-electric-indigo/15 bg-electric-indigo/[0.04] p-4 space-y-2">
      <div className="flex items-center gap-2">
        <NodeIcon name="workflow" className="w-3.5 h-3.5 text-electric-indigo/60" />
        <span className="text-body font-semibold text-electric-indigo/70">Child Workflow</span>
      </div>
      {loading ? (
        <div className="flex items-center gap-2 py-2.5">
          <NodeIcon name="loader" className="w-3.5 h-3.5 text-orbflow-text-ghost animate-spin" />
          <span className="text-body-sm text-orbflow-text-faint">Loading workflows...</span>
        </div>
      ) : (
        <select
          value={currentWorkflowId || ""}
          onChange={(e) => onChange(e.target.value)}
          className="w-full rounded-lg px-3.5 py-2.5 text-body-lg transition-all border border-orbflow-border bg-orbflow-surface text-orbflow-text-secondary
            focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50
            hover:bg-orbflow-surface-hover cursor-pointer"
        >
          <option value="">Choose a workflow...</option>
          {workflows.map((wf) => (
            <option key={wf.id} value={wf.id}>{wf.name}</option>
          ))}
        </select>
      )}
      {currentWorkflowId && (
        <p className="text-caption font-mono text-electric-indigo/40">ID: {currentWorkflowId}</p>
      )}
    </div>
  );
}
