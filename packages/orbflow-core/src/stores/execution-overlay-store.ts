import { create } from "zustand";
import type { Instance } from "../types/api";
import type { ExecutionStatus } from "../execution/execution-status";

export interface NodeExecutionStatus {
  status: ExecutionStatus;
  error?: string;
  attempt: number;
  startedAt?: string;
  endedAt?: string;
  output?: Record<string, unknown>;
}

interface Progress {
  total: number;
  completed: number;
  failed: number;
  running: number;
  pending: number;
  skipped: number;
  cancelled: number;
}

interface ExecutionOverlayState {
  // --- State ---
  activeInstanceId: string | null;
  nodeStatuses: Record<string, NodeExecutionStatus>;
  prevNodeStatuses: Record<string, NodeExecutionStatus>;
  isLive: boolean;
  instanceStatus: ExecutionStatus | null;
  workflowName: string;
  progress: Progress;
  /** Track which nodes just transitioned to a terminal state (for one-shot animations). */
  transitions: Record<string, string>;

  // --- Actions ---
  startLiveRun: (instanceId: string, totalNodes: number, workflowName: string) => void;
  syncFromInstance: (instance: Instance) => void;
  stopLiveRun: () => void;
  clearTransition: (nodeId: string) => void;
  /** Merge test-node results into the overlay without touching live-run state. */
  mergeTestResults: (nodeOutputs: Record<string, { status: ExecutionStatus; error?: string; attempt: number; started_at?: string; ended_at?: string; output?: Record<string, unknown> }>) => void;
  /** Full reset — clears all state including node statuses. Used when switching workflows. */
  reset: () => void;
}

const EMPTY_PROGRESS: Progress = {
  total: 0,
  completed: 0,
  failed: 0,
  running: 0,
  pending: 0,
  skipped: 0,
  cancelled: 0,
};

export function computeProgress(
  nodeStatuses: Record<string, NodeExecutionStatus>,
  total: number,
): Progress {
  let completed = 0;
  let failed = 0;
  let running = 0;
  let pending = 0;
  let skipped = 0;
  let cancelled = 0;

  const entries = Object.values(nodeStatuses);
  for (const ns of entries) {
    switch (ns.status) {
      case "completed":
        completed++;
        break;
      case "failed":
        failed++;
        break;
      case "running":
        running++;
        break;
      case "pending":
      case "queued":
        pending++;
        break;
      case "skipped":
        skipped++;
        break;
      case "cancelled":
        cancelled++;
        break;
      case "waiting_approval":
        pending++;
        break;
      default:
        break;
    }
  }

  // Nodes not yet in node_states are still pending
  const tracked = completed + failed + running + pending + skipped + cancelled;
  const untracked = Math.max(0, total - tracked);

  return {
    total,
    completed,
    failed,
    running,
    pending: pending + untracked,
    skipped,
    cancelled,
  };
}

const TERMINAL_STATUSES = new Set(["completed", "failed"]);

export function detectTransitions(
  prev: Record<string, NodeExecutionStatus>,
  next: Record<string, NodeExecutionStatus>,
): Record<string, string> {
  const transitions: Record<string, string> = {};

  for (const [nodeId, nextStatus] of Object.entries(next)) {
    const prevStatus = prev[nodeId];
    const changed = !prevStatus || prevStatus.status !== nextStatus.status;
    if (changed && TERMINAL_STATUSES.has(nextStatus.status)) {
      transitions[nodeId] = nextStatus.status;
    }
  }

  return transitions;
}

export const useExecutionOverlayStore = create<ExecutionOverlayState>(
  (set, get) => ({
    // --- Initial State ---
    activeInstanceId: null,
    nodeStatuses: {},
    prevNodeStatuses: {},
    isLive: false,
    instanceStatus: null,
    workflowName: "",
    progress: { ...EMPTY_PROGRESS },
    transitions: {},

    // --- Actions ---

    startLiveRun: (instanceId, totalNodes, workflowName) =>
      set({
        activeInstanceId: instanceId,
        isLive: true,
        workflowName,
        nodeStatuses: {},
        prevNodeStatuses: {},
        transitions: {},
        instanceStatus: "running",
        progress: {
          ...EMPTY_PROGRESS,
          total: totalNodes,
          pending: totalNodes,
        },
      }),

    syncFromInstance: (instance) => {
      const state = get();

      // Ignore stale responses
      if (instance.id !== state.activeInstanceId) {
        return;
      }

      // Build new nodeStatuses from instance.node_states
      const newNodeStatuses: Record<string, NodeExecutionStatus> = {};
      for (const [nodeId, ns] of Object.entries(instance.node_states)) {
        newNodeStatuses[nodeId] = {
          status: ns.status,
          error: ns.error,
          attempt: ns.attempt,
          startedAt: ns.started_at,
          endedAt: ns.ended_at,
          output: ns.status === "completed" ? ns.output : undefined,
        };
      }

      // Detect transitions (compare previous statuses with new)
      const newTransitions = detectTransitions(
        state.nodeStatuses,
        newNodeStatuses,
      );

      // Merge new transitions with existing ones that haven't been cleared yet
      const mergedTransitions =
        Object.keys(newTransitions).length > 0
          ? { ...state.transitions, ...newTransitions }
          : state.transitions;

      // Recompute progress
      const newProgress = computeProgress(
        newNodeStatuses,
        state.progress.total,
      );

      set({
        prevNodeStatuses: state.nodeStatuses,
        nodeStatuses: newNodeStatuses,
        transitions: mergedTransitions,
        instanceStatus: instance.status,
        progress: newProgress,
      });
    },

    stopLiveRun: () =>
      set((s) => ({
        isLive: false,
        activeInstanceId: null,
        // Keep nodeStatuses so runtime outputs remain available for field browsing
        nodeStatuses: s.nodeStatuses,
        prevNodeStatuses: {},
        transitions: {},
        instanceStatus: s.instanceStatus,
        progress: s.progress,
      })),

    clearTransition: (nodeId) => {
      const { transitions } = get();
      if (!(nodeId in transitions)) {
        return;
      }
      const { [nodeId]: _, ...rest } = transitions;
      set({ transitions: rest });
    },

    mergeTestResults: (nodeOutputs) => {
      const state = get();
      const merged = { ...state.nodeStatuses };
      for (const [nid, ns] of Object.entries(nodeOutputs)) {
        merged[nid] = {
          status: ns.status,
          error: ns.error,
          attempt: ns.attempt,
          startedAt: ns.started_at,
          endedAt: ns.ended_at,
          output: ns.output,
        };
      }
      set({ nodeStatuses: merged, isLive: false });
    },

    reset: () =>
      set({
        activeInstanceId: null,
        nodeStatuses: {},
        prevNodeStatuses: {},
        isLive: false,
        instanceStatus: null,
        workflowName: "",
        progress: { ...EMPTY_PROGRESS },
        transitions: {},
      }),
  }),
);
