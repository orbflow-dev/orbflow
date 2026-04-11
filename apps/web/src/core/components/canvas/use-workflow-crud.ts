"use client";

import { useCallback, useRef, useState } from "react";
import type { Node } from "@xyflow/react";
import { useExecutionOverlayStore, useToastStore, useHistoryStore } from "@orbflow/core/stores";
import type { OrbflowConfig } from "../../context/orbflow-provider";
import type { Workflow, TestNodeResult } from "@orbflow/core";

/** Strip internal prefixes from backend error messages for user display. */
function friendlyError(err: unknown): string {
  if (!(err instanceof Error)) return "An unexpected error occurred";
  return err.message
    .replace(/^orbflow:\s*/i, "")
    .replace(/^failed to start workflow:\s*/i, "");
}

interface UseWorkflowCrudDeps {
  nodes: Node[];
  config: OrbflowConfig;
  workflowName: string;
  workflow?: Partial<Workflow>;
  getPayload: () => Promise<Partial<Workflow>>;
  validateWorkflow: () => string[];
  startLiveRun: (instanceId: string, nodes: Node[], workflowName: string) => void;
}

interface UseWorkflowCrudReturn {
  handleSave: () => Promise<Workflow | null>;
  handleRun: () => Promise<void>;
  handleTestNode: (nodeId: string) => Promise<void>;
  isSaving: boolean;
  isRunning: boolean;
  testingNodeId: string | null;
  savedWorkflowRef: React.RefObject<Workflow | null>;
}

export function useWorkflowCrud(deps: UseWorkflowCrudDeps): UseWorkflowCrudReturn {
  const {
    nodes,
    config,
    workflowName,
    workflow,
    getPayload,
    validateWorkflow,
    startLiveRun,
  } = deps;

  const toast = useToastStore();
  const history = useHistoryStore();

  const [isSaving, setIsSaving] = useState(false);
  const [isRunning, setIsRunning] = useState(false);
  const [testingNodeId, setTestingNodeId] = useState<string | null>(null);
  const runLockRef = useRef(false);
  const savedWorkflowRef = useRef<Workflow | null>((workflow as Workflow) || null);

  // -- Save ------------------------------------
  const handleSave = useCallback(async (): Promise<Workflow | null> => {
    if (nodes.length === 0) {
      toast.warning("Nothing to save", "Add at least one step to your workflow");
      return null;
    }
    setIsSaving(true);
    try {
      const wf = await getPayload();
      if (savedWorkflowRef.current?.id) wf.id = savedWorkflowRef.current.id;
      if (config.onSave) {
        const result = await config.onSave(wf);
        if (result && typeof result === "object" && "id" in result) {
          savedWorkflowRef.current = result as Workflow;
        }
      }
      if (config.onChange) config.onChange(wf);
      history.markClean();
      toast.success("Workflow saved", `${nodes.length} steps saved successfully`);
      return savedWorkflowRef.current;
    } catch (err) {
      console.error("[orbflow] Failed to save workflow:", err);
      toast.error("Failed to save workflow", friendlyError(err));
      throw err;
    } finally {
      setIsSaving(false);
    }
  }, [nodes, getPayload, config, toast, history]);

  // -- Run (with debounce lock) ----------------
  const handleRun = useCallback(async () => {
    if (runLockRef.current) return;
    runLockRef.current = true;
    try {
      if (nodes.length === 0) {
        toast.warning("Nothing to run", "Add at least one step");
        return;
      }
      const errors = validateWorkflow();
      if (errors.length > 0) {
        toast.error(
          "Missing required fields",
          errors.slice(0, 3).join("\n") +
            (errors.length > 3 ? ` (+${errors.length - 3} more)` : ""),
        );
        return;
      }
      if (!config.onRun) {
        toast.info("No run handler", "Run is not configured");
        return;
      }
      setIsRunning(true);
      try {
        // Contract: workflow must be persisted before execution so the engine
        // operates on the latest definition. handleSave() is always called
        // here -- running without saving is not supported.
        const saved = await handleSave();
        if (!saved?.id) {
          toast.error("Cannot run", "Save the workflow first");
          return;
        }
        const instanceId = await config.onRun(saved);
        if (!instanceId) {
          toast.error("Failed to start", "Could not start workflow");
          return;
        }
        startLiveRun(instanceId, nodes, workflowName);
        toast.info("Workflow running...", "Monitoring execution on canvas");
      } catch (err) {
        console.error("[orbflow] Failed to run workflow:", err);
        toast.error("Failed to run workflow", friendlyError(err));
      } finally {
        setIsRunning(false);
      }
    } finally {
      setTimeout(() => {
        runLockRef.current = false;
      }, 1000);
    }
  }, [nodes, validateWorkflow, handleSave, config, toast, workflowName, startLiveRun]);

  // -- Test single node (with auto-save) -----
  const handleTestNode = useCallback(
    async (nodeId: string) => {
      if (!config.onTestNode) {
        toast.info("Test not available", "Test node is not configured");
        return;
      }
      if (testingNodeId) {
        toast.warning("Already testing", "Wait for the current test to finish");
        return;
      }

      setTestingNodeId(nodeId);
      try {
        // Contract: workflow must be persisted before node testing so the
        // backend can resolve the correct node definition. handleSave() is
        // always called here -- testing without saving is not supported.
        const saved = await handleSave();
        if (!saved?.id) {
          toast.error("Cannot test", "Save the workflow first");
          return;
        }

        const result = (await config.onTestNode(saved, nodeId)) as
          | TestNodeResult
          | void;
        if (!result) return;

        // Merge outputs into execution overlay store so field browser picks them up
        useExecutionOverlayStore.getState().mergeTestResults(result.node_outputs);

        const targetNS = result.node_outputs[nodeId];
        if (targetNS?.status === "failed") {
          toast.error("Test failed", targetNS.error || "Node execution failed");
        } else {
          toast.success(
            "Test complete",
            "Node output is now available in the field browser",
          );
        }
      } catch (err) {
        console.error("[orbflow] Failed to test node:", err);
        toast.error("Test failed", friendlyError(err));
      } finally {
        setTestingNodeId(null);
      }
    },
    [config, testingNodeId, handleSave, toast],
  );

  return {
    handleSave,
    handleRun,
    handleTestNode,
    isSaving,
    isRunning,
    testingNodeId,
    savedWorkflowRef,
  };
}
