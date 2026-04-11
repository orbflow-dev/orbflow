"use client";

import { useCallback, useEffect, useRef } from "react";
import type { Node } from "@xyflow/react";
import { useExecutionOverlayStore } from "@orbflow/core/stores";
import { useExecutionPolling } from "@/hooks/use-execution-polling";
import { useToastStore } from "@orbflow/core/stores";

interface UseExecutionLifecycleReturn {
  execOverlay: ReturnType<typeof useExecutionOverlayStore>;
  startLiveRun: (instanceId: string, nodes: Node[], workflowName: string) => void;
  stopLiveRun: () => void;
  liveRunInstanceId: string | null;
  isLive: boolean;
}

export function useExecutionLifecycle(): UseExecutionLifecycleReturn {
  const toast = useToastStore();
  const execOverlay = useExecutionOverlayStore();

  // Poll for execution updates
  useExecutionPolling({
    instanceId: execOverlay.activeInstanceId,
    enabled: execOverlay.isLive,
  });

  // Track status transitions for toast notifications
  const prevStatusRef = useRef<string | null>(null);
  useEffect(() => {
    const s = execOverlay.instanceStatus;
    const prev = prevStatusRef.current;
    prevStatusRef.current = s;
    if (prev === s) return;
    if (s === "completed") {
      toast.success("Workflow completed", "All steps finished successfully");
    } else if (s === "failed") {
      const f = Object.entries(execOverlay.nodeStatuses).find(
        ([, ns]) => ns.status === "failed",
      );
      toast.error(
        `Step "${f?.[0] || "Unknown"}" failed`,
        f?.[1]?.error || "A step in the workflow failed",
      );
    } else if (s === "cancelled") {
      toast.warning("Workflow cancelled");
    }
  }, [execOverlay.instanceStatus, execOverlay.nodeStatuses, toast]);

  // Cleanup execution overlay on unmount
  useEffect(() => {
    return () => {
      useExecutionOverlayStore.getState().stopLiveRun();
    };
  }, []);

  const startLiveRun = useCallback(
    (instanceId: string, nodes: Node[], workflowName: string) => {
      useExecutionOverlayStore
        .getState()
        .startLiveRun(
          instanceId,
          nodes.filter((n) => n.type === "task").length,
          workflowName,
        );
    },
    [],
  );

  const stopLiveRun = useCallback(() => {
    useExecutionOverlayStore.getState().stopLiveRun();
  }, []);

  return {
    execOverlay,
    startLiveRun,
    stopLiveRun,
    liveRunInstanceId: execOverlay.activeInstanceId,
    isLive: execOverlay.isLive,
  };
}
