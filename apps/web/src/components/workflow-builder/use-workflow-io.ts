"use client";

import { useCallback, useMemo, useRef } from "react";
import { useWorkflowStore } from "@/store/workflow-store";
import { useCanvasStore } from "@/store/canvas-store";
import { usePanelStore } from "@/store/panel-store";
import { useToastStore } from "@/store/toast-store";
import type { Workflow } from "@/lib/api";

// -- Utilities -------------------------------------

export function generateUntitledName(existingNames: string[]): string {
  const base = "Untitled Workflow";
  if (!existingNames.includes(base)) return base;
  let i = 2;
  while (existingNames.includes(`${base} ${i}`)) i++;
  return `${base} ${i}`;
}

/**
 * Sanitize a raw parsed JSON object into a safe Partial<Workflow>.
 * Only picks known fields -- never passes raw objects to the backend.
 * Returns null if the object is not a valid workflow shape.
 */
export function sanitizeImportedWorkflow(
  raw: Record<string, unknown>,
): Partial<Workflow> | null {
  if (typeof raw.name !== "string" || !Array.isArray(raw.nodes)) {
    return null;
  }

  return {
    name: `${raw.name} (imported)`,
    description:
      typeof raw.description === "string" ? raw.description : undefined,
    nodes: (raw.nodes as Record<string, unknown>[]).map((n) => ({
      id: String(n.id ?? ""),
      name: String(n.name ?? ""),
      type: String(n.type ?? "builtin"),
      plugin_ref: String(n.plugin_ref ?? ""),
      position: {
        x: Number((n.position as Record<string, unknown>)?.x ?? 0),
        y: Number((n.position as Record<string, unknown>)?.y ?? 0),
      },
      input_mapping: n.input_mapping as Record<string, unknown> | undefined,
    })),
    edges: Array.isArray(raw.edges)
      ? (raw.edges as Record<string, unknown>[]).map((e) => ({
          id: String(e.id ?? ""),
          source: String(e.source ?? ""),
          target: String(e.target ?? ""),
          condition: typeof e.condition === "string" ? e.condition : undefined,
        }))
      : [],
  };
}

/**
 * Build an export-ready workflow by merging live canvas positions into the
 * stored workflow. Canvas nodes may have been moved since the last save,
 * so we read the current positions from the canvas store.
 */
export function buildExportPayload(
  workflow: Workflow,
  canvasNodes: { id: string; position: { x: number; y: number } }[],
): Workflow {
  const positionMap = new Map(
    canvasNodes.map((n) => [n.id, n.position]),
  );

  return {
    ...workflow,
    nodes: workflow.nodes.map((node) => {
      const livePos = positionMap.get(node.id);
      return livePos ? { ...node, position: livePos } : node;
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
