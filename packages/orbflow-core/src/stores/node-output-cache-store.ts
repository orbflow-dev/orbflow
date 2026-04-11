import { create } from "zustand";
import { persist } from "zustand/middleware";

/** Maximum cached outputs per workflow to prevent localStorage bloat. */
const MAX_ENTRIES_PER_WORKFLOW = 50;

interface NodeOutputCacheState {
  /** workflowId → nodeId → output data */
  cache: Record<string, Record<string, Record<string, unknown>>>;

  getWorkflowCache: (workflowId: string) => Record<string, Record<string, unknown>>;
  setCachedOutput: (workflowId: string, nodeId: string, output: Record<string, unknown>) => void;
  mergeBulk: (workflowId: string, outputs: Record<string, Record<string, unknown>>) => void;
  clearWorkflow: (workflowId: string) => void;
  hasOutput: (workflowId: string, nodeId: string) => boolean;
}

export const useNodeOutputCacheStore = create<NodeOutputCacheState>()(
  persist(
    (set, get) => ({
      cache: {},

      getWorkflowCache: (workflowId) => {
        return get().cache[workflowId] ?? {};
      },

      setCachedOutput: (workflowId, nodeId, output) =>
        set((s) => {
          const wfCache = { ...(s.cache[workflowId] ?? {}), [nodeId]: output };
          return { cache: { ...s.cache, [workflowId]: evict(wfCache) } };
        }),

      mergeBulk: (workflowId, outputs) =>
        set((s) => {
          const wfCache = { ...(s.cache[workflowId] ?? {}), ...outputs };
          return { cache: { ...s.cache, [workflowId]: evict(wfCache) } };
        }),

      clearWorkflow: (workflowId) =>
        set((s) => {
          const { [workflowId]: _, ...rest } = s.cache;
          return { cache: rest };
        }),

      hasOutput: (workflowId, nodeId) => {
        const wfCache = get().cache[workflowId];
        return wfCache != null && nodeId in wfCache;
      },
    }),
    {
      name: "orbflow-node-output-cache",
    },
  ),
);

/** Evict oldest entries if the workflow cache exceeds the limit. */
function evict(
  wfCache: Record<string, Record<string, unknown>>,
): Record<string, Record<string, unknown>> {
  const keys = Object.keys(wfCache);
  if (keys.length <= MAX_ENTRIES_PER_WORKFLOW) return wfCache;

  const trimmed: Record<string, Record<string, unknown>> = {};
  const keep = keys.slice(keys.length - MAX_ENTRIES_PER_WORKFLOW);
  for (const k of keep) {
    trimmed[k] = wfCache[k];
  }
  return trimmed;
}
