"use client";

import { useCallback, useMemo, useRef } from "react";
import { useWorkflowStore } from "@/store/workflow-store";
import { useCanvasStore } from "@/store/canvas-store";
import { usePanelStore } from "@/store/panel-store";
import { useToastStore } from "@/store/toast-store";
import type { Workflow, WorkflowNode } from "@/lib/api";
import type { LegacyTriggerData } from "@orbflow/core/types";

// -- Utilities -------------------------------------

export function generateUntitledName(existingNames: string[]): string {
  const base = "Untitled Workflow";
  if (!existingNames.includes(base)) return base;
  let i = 2;
  while (existingNames.includes(`${base} ${i}`)) i++;
  return `${base} ${i}`;
}

const UNSAFE_OBJECT_KEYS = new Set(["__proto__", "constructor", "prototype"]);
const NODE_KINDS = new Set(["trigger", "action", "capability"]);
const WORKFLOW_STATUSES = new Set(["draft", "active", "archived"]);
const PARAMETER_MODES = new Set(["static", "expression"]);
const TRIGGER_TYPES = new Set(["manual", "event", "schedule", "webhook", "cron"]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOwn(record: Record<string, unknown>, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key);
}

function finiteNumber(value: unknown): number | undefined {
  const n = Number(value);
  return Number.isFinite(n) ? n : undefined;
}

function nonNegativeSafeInteger(value: number): number | undefined {
  return Number.isSafeInteger(value) && value >= 0 ? value : undefined;
}

function parseRetryDelayMs(value: unknown): number | undefined {
  if (typeof value === "number") {
    return nonNegativeSafeInteger(value);
  }
  if (typeof value !== "string") return undefined;

  const trimmed = value.trim();
  if (!trimmed) return undefined;

  const match = trimmed.match(/^(\d+(?:\.\d+)?)\s*(ms|s|m|h)?$/i);
  if (!match) return undefined;

  const amount = Number(match[1]);
  if (!Number.isFinite(amount)) return undefined;

  const unit = (match[2] ?? "ms").toLowerCase();
  const multiplier = unit === "h" ? 3_600_000 : unit === "m" ? 60_000 : unit === "s" ? 1_000 : 1;
  return nonNegativeSafeInteger(Math.round(amount * multiplier));
}

function positionFrom(value: unknown): { x: number; y: number } {
  const p = isRecord(value) ? value : {};
  return {
    x: finiteNumber(p.x) ?? 0,
    y: finiteNumber(p.y) ?? 0,
  };
}

function normalizeWireTriggerType(value: string): "manual" | "event" | "schedule" | "webhook" | undefined {
  if (!TRIGGER_TYPES.has(value)) return undefined;
  return value === "cron" ? "schedule" : value as "manual" | "event" | "schedule" | "webhook";
}

function sanitizeJsonValue(value: unknown, depth = 0): unknown {
  if (depth > 20) return undefined;
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean"
  ) {
    return value;
  }
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : undefined;
  }
  if (Array.isArray(value)) {
    return value
      .map((item) => sanitizeJsonValue(item, depth + 1))
      .filter((item) => item !== undefined);
  }
  if (isRecord(value)) {
    const out: Record<string, unknown> = {};
    for (const [key, nested] of Object.entries(value)) {
      if (UNSAFE_OBJECT_KEYS.has(key)) continue;
      const sanitized = sanitizeJsonValue(nested, depth + 1);
      if (sanitized !== undefined) out[key] = sanitized;
    }
    return out;
  }
  return undefined;
}

function sanitizeJsonObject(value: unknown): Record<string, unknown> | undefined {
  const sanitized = sanitizeJsonValue(value);
  return isRecord(sanitized) ? sanitized : undefined;
}

function sanitizeParameters(value: unknown): WorkflowNode["parameters"] | undefined {
  if (!Array.isArray(value)) return undefined;

  const params = value.flatMap((item) => {
    if (!isRecord(item)) return [];
    const mode = typeof item.mode === "string" && PARAMETER_MODES.has(item.mode)
      ? item.mode
      : undefined;
    if (typeof item.key !== "string" || !mode) return [];

    const param: Record<string, unknown> = { key: item.key, mode };
    if (hasOwn(item, "value")) {
      const sanitizedValue = sanitizeJsonValue(item.value);
      if (sanitizedValue !== undefined) param.value = sanitizedValue;
    }
    if (typeof item.expression === "string") {
      param.expression = item.expression;
    }
    return [param];
  });

  return params.length ? (params as WorkflowNode["parameters"]) : undefined;
}

function sanitizeCapabilityPorts(value: unknown): unknown[] | undefined {
  if (!Array.isArray(value)) return undefined;

  const ports = value.flatMap((item) => {
    if (!isRecord(item)) return [];
    if (typeof item.key !== "string" || typeof item.capability_type !== "string") {
      return [];
    }

    const port: Record<string, unknown> = {
      key: item.key,
      capability_type: item.capability_type,
    };
    if (typeof item.required === "boolean") port.required = item.required;
    if (typeof item.description === "string") port.description = item.description;
    return [port];
  });

  return ports.length ? ports : undefined;
}

function sanitizeRetry(value: unknown): unknown {
  if (!isRecord(value)) return undefined;
  const maxAttempts = finiteNumber(value.max_attempts);
  const multiplier = finiteNumber(value.multiplier);
  const delay = parseRetryDelayMs(value.delay);
  if (
    maxAttempts === undefined ||
    multiplier === undefined ||
    delay === undefined
  ) {
    return undefined;
  }
  return {
    max_attempts: maxAttempts,
    delay,
    multiplier,
  };
}

function sanitizeCompensate(value: unknown): unknown {
  if (!isRecord(value) || typeof value.plugin_ref !== "string") return undefined;
  const compensate: Record<string, unknown> = { plugin_ref: value.plugin_ref };
  const inputMapping = sanitizeJsonObject(value.input_mapping);
  if (inputMapping) compensate.input_mapping = inputMapping;
  return compensate;
}

function sanitizeMetadata(value: unknown): unknown {
  if (!isRecord(value)) return undefined;
  const metadata: Record<string, unknown> = {};
  if (typeof value.description === "string") metadata.description = value.description;
  if (typeof value.docs === "string") metadata.docs = value.docs;
  if (typeof value.image_url === "string") metadata.image_url = value.image_url;
  return Object.keys(metadata).length ? metadata : undefined;
}

function sanitizeTriggerConfig(value: unknown): unknown {
  if (!isRecord(value) || typeof value.trigger_type !== "string") return undefined;
  const triggerType = normalizeWireTriggerType(value.trigger_type);
  if (!triggerType) return undefined;
  const config: Record<string, unknown> = { trigger_type: triggerType };
  if (typeof value.cron === "string") config.cron = value.cron;
  if (typeof value.event_name === "string") config.event_name = value.event_name;
  if (typeof value.path === "string") config.path = value.path;
  return config;
}

function sanitizeWorkflowNode(raw: unknown): WorkflowNode {
  const n = isRecord(raw) ? raw : {};
  const node: Record<string, unknown> = {
    id: String(n.id ?? ""),
    name: String(n.name ?? ""),
    type: String(n.type ?? "builtin"),
    plugin_ref: String(n.plugin_ref ?? ""),
    position: positionFrom(n.position),
    input_mapping: sanitizeJsonObject(n.input_mapping),
  };

  if (typeof n.kind === "string" && NODE_KINDS.has(n.kind)) node.kind = n.kind;
  const config = sanitizeJsonObject(n.config);
  if (config) node.config = config;
  const parameters = sanitizeParameters(n.parameters);
  if (parameters) node.parameters = parameters;
  const retry = sanitizeRetry(n.retry);
  if (retry) node.retry = retry;
  const compensate = sanitizeCompensate(n.compensate);
  if (compensate) node.compensate = compensate;
  const capabilityPorts = sanitizeCapabilityPorts(n.capability_ports);
  if (capabilityPorts) node.capability_ports = capabilityPorts;
  const metadata = sanitizeMetadata(n.metadata);
  if (metadata) node.metadata = metadata;
  const triggerConfig = sanitizeTriggerConfig(n.trigger_config);
  if (triggerConfig) node.trigger_config = triggerConfig;
  if (typeof n.requires_approval === "boolean") {
    node.requires_approval = n.requires_approval;
  }
  if (typeof n.parent_id === "string") node.parent_id = n.parent_id;

  return node as unknown as WorkflowNode;
}

function sanitizeWorkflowEdge(raw: unknown): Workflow["edges"][number] {
  const e = isRecord(raw) ? raw : {};
  const edge: Workflow["edges"][number] = {
    id: String(e.id ?? ""),
    source: String(e.source ?? ""),
    target: String(e.target ?? ""),
  };
  if (typeof e.condition === "string") edge.condition = e.condition;
  return edge;
}

function sanitizeCapabilityEdges(value: unknown): Workflow["capability_edges"] | undefined {
  if (!Array.isArray(value)) return undefined;
  const edges = value.flatMap((item) => {
    if (!isRecord(item)) return [];
    if (
      typeof item.id !== "string" ||
      typeof item.source_node_id !== "string" ||
      typeof item.target_node_id !== "string" ||
      typeof item.target_port_key !== "string"
    ) {
      return [];
    }
    return [{
      id: item.id,
      source_node_id: item.source_node_id,
      target_node_id: item.target_node_id,
      target_port_key: item.target_port_key,
    }];
  });
  return edges.length ? edges : undefined;
}

function sanitizeAnnotations(value: unknown): Workflow["annotations"] | undefined {
  if (!Array.isArray(value)) return undefined;
  const annotations = value.flatMap((item) => {
    if (!isRecord(item)) return [];
    if (
      typeof item.id !== "string" ||
      typeof item.type !== "string" ||
      typeof item.content !== "string"
    ) {
      return [];
    }
    const annotation: NonNullable<Workflow["annotations"]>[number] = {
      id: item.id,
      type: item.type,
      content: item.content,
      position: positionFrom(item.position),
    };
    const style = sanitizeJsonObject(item.style);
    if (style) annotation.style = style;
    return [annotation];
  });
  return annotations.length ? annotations : undefined;
}

function sanitizeTriggers(value: unknown): LegacyTriggerData[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const triggers = value.flatMap((item) => {
    if (!isRecord(item)) return [];
    const triggerType = typeof item.type === "string"
      ? item.type
      : typeof item.trigger_type === "string"
        ? item.trigger_type
        : undefined;
    const normalizedTriggerType = triggerType ? normalizeWireTriggerType(triggerType) : undefined;
    if (!normalizedTriggerType) return [];

    const config = isRecord(item.config) ? item.config : {};
    const trigger: LegacyTriggerData & {
      config: NonNullable<LegacyTriggerData["config"]>;
    } = {
      type: normalizedTriggerType,
      config: {},
    };
    const triggerConfig = trigger.config;
    if (typeof config.cron === "string") triggerConfig.cron = config.cron;
    if (typeof config.event_name === "string") {
      triggerConfig.event_name = config.event_name;
    }
    if (typeof config.path === "string") triggerConfig.path = config.path;
    return [trigger];
  });
  return triggers.length ? triggers : undefined;
}

/**
 * Sanitize a raw parsed JSON object into a safe Partial<Workflow>.
 * Only picks known workflow schema fields -- never passes raw objects through.
 * Returns null if the object is not a valid workflow shape.
 */
export function sanitizeImportedWorkflow(
  raw: Record<string, unknown>,
): Partial<Workflow> | null {
  if (typeof raw.name !== "string" || !Array.isArray(raw.nodes)) {
    return null;
  }

  const sanitized: Partial<Workflow> = {
    name: `${raw.name} (imported)`,
    description:
      typeof raw.description === "string" ? raw.description : undefined,
    nodes: raw.nodes.map(sanitizeWorkflowNode),
    edges: Array.isArray(raw.edges)
      ? raw.edges.map(sanitizeWorkflowEdge)
      : [],
  };

  if (typeof raw.status === "string" && WORKFLOW_STATUSES.has(raw.status)) {
    sanitized.status = raw.status as Workflow["status"];
  }
  const capabilityEdges = sanitizeCapabilityEdges(raw.capability_edges);
  if (capabilityEdges) sanitized.capability_edges = capabilityEdges;
  const annotations = sanitizeAnnotations(raw.annotations);
  if (annotations) sanitized.annotations = annotations;
  const triggers = sanitizeTriggers(raw.triggers);
  if (triggers) sanitized.triggers = triggers;

  return sanitized;
}

/**
 * Build an export-ready workflow by merging live canvas positions into the
 * stored workflow. Canvas nodes may have been moved since the last save,
 * so we read the current positions from the canvas store.
 */
export function buildExportPayload(
  workflow: Workflow,
  canvasNodes: {
    id: string;
    position: { x: number; y: number };
    data?: Record<string, unknown>;
    style?: {
      width?: unknown;
      height?: unknown;
    };
  }[],
): Workflow {
  const positionMap = new Map(
    canvasNodes.map((n) => [n.id, n.position]),
  );
  const nodeMap = new Map(canvasNodes.map((n) => [n.id, n]));

  return {
    ...workflow,
    nodes: workflow.nodes.map((node) => {
      const livePos = positionMap.get(node.id);
      return livePos ? { ...node, position: livePos } : node;
    }),
    annotations: workflow.annotations?.map((annotation) => {
      const prefix = annotation.type === "text" ? "text_" : "sticky_";
      const canvasNode = nodeMap.get(`${prefix}${annotation.id}`);
      if (!canvasNode) return annotation;

      const width = canvasNode.data?.width ?? canvasNode.style?.width;
      const height = canvasNode.data?.height ?? canvasNode.style?.height;
      const style = {
        ...(annotation.style ?? {}),
        ...(typeof width === "number" ? { width } : {}),
        ...(typeof height === "number" ? { height } : {}),
      };

      return {
        ...annotation,
        position: canvasNode.position,
        style: Object.keys(style).length ? style : undefined,
      };
    }),
  };
}

/**
 * Serialize a workflow to a JSON Blob and trigger a browser download.
 * Returns the filename used.
 */
export function exportWorkflowAsJson(workflow: Workflow): string {
  const json = JSON.stringify(workflow, null, 2);
  const blob = new Blob([json], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  const filename = `${workflow.name.replace(/\s+/g, "-").toLowerCase()}.json`;
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
  return filename;
}

// -- Hook ------------------------------------------

export interface UseWorkflowIoReturn {
  /** Currently selected workflow */
  selectedWorkflow: Workflow | null;
  /** All available workflows */
  workflows: Workflow[];
  /** Default name for a new (unsaved) workflow */
  defaultName: string | undefined;
  /** Ref for the hidden file input element */
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  /** Handle workflow selection / deselection */
  handleSelect: (id: string) => void;
  /** Trigger import file dialog */
  handleImport: () => void;
  /** Export the selected workflow as JSON download */
  handleExport: () => void;
  /** Process the selected file from the import dialog */
  handleFileChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
}

export function useWorkflowIo(): UseWorkflowIoReturn {
  const { selectedWorkflow, workflows, selectWorkflow, clearSelectedWorkflow } =
    useWorkflowStore();
  const toast = useToastStore();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const defaultName = useMemo(() => {
    if (selectedWorkflow) return undefined;
    return generateUntitledName(workflows.map((w) => w.name));
  }, [selectedWorkflow, workflows]);

  const handleSelect = useCallback(
    (value: string) => {
      usePanelStore.getState().clearAll();
      if (value) {
        selectWorkflow(value);
      } else {
        clearSelectedWorkflow();
        useCanvasStore.getState().setNodes([]);
        useCanvasStore.getState().setEdges([]);
      }
    },
    [selectWorkflow, clearSelectedWorkflow],
  );

  const handleExport = useCallback(() => {
    if (!selectedWorkflow) {
      toast.warning("Nothing to export", "Select or save a workflow first");
      return;
    }
    const canvasNodes = useCanvasStore.getState().nodes;
    const payload = buildExportPayload(selectedWorkflow, canvasNodes);
    exportWorkflowAsJson(payload);
    toast.success("Exported", `"${selectedWorkflow.name}" downloaded as JSON`);
  }, [selectedWorkflow, toast]);

  const handleImport = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        const raw = JSON.parse(text) as Record<string, unknown>;
        const sanitized = sanitizeImportedWorkflow(raw);
        if (!sanitized) {
          toast.error(
            "Invalid file",
            "The JSON file doesn't appear to be a valid workflow",
          );
          return;
        }
        const created = await useWorkflowStore
          .getState()
          .createWorkflow(sanitized);
        selectWorkflow(created.id);
        toast.success("Imported", `"${created.name}" has been imported`);
      } catch (err) {
        console.error("[orbflow] Failed to import workflow file:", err);
        toast.error("Import failed", "Could not parse the selected file");
      }
      if (fileInputRef.current) fileInputRef.current.value = "";
    },
    [selectWorkflow, toast],
  );

  return {
    selectedWorkflow,
    workflows,
    defaultName,
    fileInputRef,
    handleSelect,
    handleImport,
    handleExport,
    handleFileChange,
  };
}
