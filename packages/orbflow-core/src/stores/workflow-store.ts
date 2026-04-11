import { create } from "zustand";
import type { Workflow, Instance } from "../types/api";
import { createStoreClient } from "./store-client";
import { useToastStore } from "./toast-store";
import { toMessage } from "./to-message";

/* ═══════════════════════════════════════════════════════
   API Client injection — call once at app startup
   ═══════════════════════════════════════════════════════ */

const { setClient: setWorkflowApiClient, requireClient } = createStoreClient("Workflow");
export { setWorkflowApiClient };

/* ═══════════════════════════════════════════════════════
   Store
   ═══════════════════════════════════════════════════════ */

interface WorkflowStore {
  workflows: Workflow[];
  instances: Instance[];
  selectedWorkflow: Workflow | null;
  selectedInstance: Instance | null;
  loading: boolean;
  instancesLoading: boolean;
  error: string | null;
  fetchWorkflows: () => Promise<void>;
  fetchInstances: () => Promise<void>;
  selectWorkflow: (id: string) => Promise<void>;
  selectInstance: (id: string) => Promise<void>;
  createWorkflow: (wf: Partial<Workflow>) => Promise<Workflow>;
  startWorkflow: (
    id: string,
    input?: Record<string, unknown>
  ) => Promise<Instance>;
  cancelInstance: (id: string) => Promise<void>;
  approveNode: (instanceId: string, nodeId: string, approvedBy?: string) => Promise<void>;
  rejectNode: (instanceId: string, nodeId: string, reason?: string) => Promise<void>;
  clearSelectedWorkflow: () => void;
}

export const useWorkflowStore = create<WorkflowStore>((set) => ({
  workflows: [],
  instances: [],
  selectedWorkflow: null,
  selectedInstance: null,
  loading: false,
  instancesLoading: false,
  error: null,

  fetchWorkflows: async () => {
    const client = requireClient();
    set({ loading: true, error: null });
    try {
      const result = await client.workflows.list();
      set({ workflows: result.items, loading: false });
    } catch (e) {
      const msg = toMessage(e);
      set({ error: msg, loading: false });
      useToastStore.getState().error("Failed to load workflows", msg);
    }
  },

  fetchInstances: async () => {
    const client = requireClient();
    set({ instancesLoading: true, error: null });
    try {
      const result = await client.instances.list({ limit: 50 });
      set({ instances: result.items, instancesLoading: false });
    } catch (e) {
      const msg = toMessage(e);
      set({ error: msg, instancesLoading: false });
      useToastStore.getState().error("Failed to load instances", msg);
    }
  },

  selectWorkflow: async (id) => {
    const client = requireClient();
    try {
      const wf = await client.workflows.get(id);
      set({ selectedWorkflow: wf });
    } catch (e) {
      const msg = toMessage(e);
      set({ error: msg });
      useToastStore.getState().error("Failed to load workflow", msg);
      throw e;
    }
  },

  selectInstance: async (id) => {
    const client = requireClient();
    try {
      const inst = await client.instances.get(id);
      set({ selectedInstance: inst });
    } catch (e) {
      const msg = toMessage(e);
      set({ error: msg });
      useToastStore.getState().error("Failed to load instance", msg);
      throw e;
    }
  },

  createWorkflow: async (wf) => {
    const client = requireClient();
    try {
      const created = await client.workflows.create(wf);
      set((s) => ({ workflows: [...s.workflows, created] }));
      useToastStore
        .getState()
        .success("Workflow created", `"${created.name}" has been created`);
      return created;
    } catch (e) {
      const msg = toMessage(e);
      useToastStore.getState().error("Failed to create workflow", msg);
      throw e;
    }
  },

  startWorkflow: async (id, input) => {
    const client = requireClient();
    try {
      const inst = await client.workflows.start(id, input);
      set((s) => ({ instances: [inst, ...s.instances] }));
      useToastStore
        .getState()
        .success("Workflow started", "Check the Activity tab for progress");
      return inst;
    } catch (e) {
      const msg = toMessage(e);
      useToastStore.getState().error("Failed to start workflow", msg);
      throw e;
    }
  },

  cancelInstance: async (id) => {
    const client = requireClient();
    try {
      await client.instances.cancel(id);
      set((s) => ({
        instances: s.instances.map((i) =>
          i.id === id ? { ...i, status: "cancelled" } : i
        ),
      }));
      useToastStore.getState().info("Run cancelled");
    } catch (e) {
      const msg = toMessage(e);
      useToastStore.getState().error("Failed to cancel run", msg);
      throw e;
    }
  },

  clearSelectedWorkflow: () => set({ selectedWorkflow: null }),

  approveNode: async (instanceId, nodeId, approvedBy) => {
    const client = requireClient();
    try {
      await client.instances.approveNode(instanceId, nodeId, approvedBy ? { approved_by: approvedBy } : undefined);
      useToastStore.getState().success("Node approved", "Execution will continue");
      // Re-fetch the instance so the UI reflects the new node status immediately
      const updated = await client.instances.get(instanceId);
      set((s) => ({
        selectedInstance:
          s.selectedInstance?.id === instanceId ? updated : s.selectedInstance,
        instances: s.instances.map((i) => (i.id === instanceId ? updated : i)),
      }));
    } catch (e) {
      const msg = toMessage(e);
      useToastStore.getState().error("Failed to approve node", msg);
      throw e;
    }
  },

  rejectNode: async (instanceId, nodeId, reason) => {
    const client = requireClient();
    try {
      await client.instances.rejectNode(instanceId, nodeId, reason ? { reason } : undefined);
      useToastStore.getState().info("Node rejected");
      // Re-fetch the instance so the UI reflects the new node status immediately
      const updated = await client.instances.get(instanceId);
      set((s) => ({
        selectedInstance:
          s.selectedInstance?.id === instanceId ? updated : s.selectedInstance,
        instances: s.instances.map((i) => (i.id === instanceId ? updated : i)),
      }));
    } catch (e) {
      const msg = toMessage(e);
      useToastStore.getState().error("Failed to reject node", msg);
      throw e;
    }
  },
}));
