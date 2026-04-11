import { create } from "zustand";
import type {
  AlertRule,
  CreateAlertInput,
} from "../types/api";
import { createStoreClient } from "./store-client";
import { useToastStore } from "./toast-store";

/* ═══════════════════════════════════════════════════════
   API Client injection — call once at app startup
   ═══════════════════════════════════════════════════════ */

const { setClient: setAlertApiClient, requireClient } = createStoreClient("Alert");
export { setAlertApiClient };

/* ═══════════════════════════════════════════════════════
   Store
   ═══════════════════════════════════════════════════════ */

interface AlertStore {
  alerts: AlertRule[];
  loading: boolean;
  error: string | null;

  fetchAlerts: () => Promise<void>;
  createAlert: (input: CreateAlertInput) => Promise<AlertRule>;
  updateAlert: (id: string, input: Partial<CreateAlertInput>) => Promise<AlertRule>;
  deleteAlert: (id: string) => Promise<void>;
  toggleAlert: (id: string) => Promise<void>;
}

export const useAlertStore = create<AlertStore>((set, get) => ({
  alerts: [],
  loading: false,
  error: null,

  fetchAlerts: async () => {
    const client = requireClient();
    set({ loading: true, error: null });
    try {
      const alerts = await client.alerts.list();
      set({ alerts, loading: false });
    } catch (e) {
      const msg = (e as Error).message;
      set({ error: msg, loading: false });
      useToastStore.getState().error("Failed to load alerts", msg);
      throw e;
    }
  },

  createAlert: async (input) => {
    const client = requireClient();
    try {
      const created = await client.alerts.create(input);
      set((s) => ({ alerts: [created, ...s.alerts] }));
      useToastStore
        .getState()
        .success("Alert created", "New alert rule has been saved");
      return created;
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to create alert", msg);
      throw e;
    }
  },

  updateAlert: async (id, input) => {
    const client = requireClient();
    try {
      const updated = await client.alerts.update(id, input);
      set((s) => ({
        alerts: s.alerts.map((a) => (a.id === id ? updated : a)),
      }));
      useToastStore.getState().success("Alert updated");
      return updated;
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to update alert", msg);
      throw e;
    }
  },

  deleteAlert: async (id) => {
    const client = requireClient();
    try {
      await client.alerts.delete(id);
      set((s) => ({
        alerts: s.alerts.filter((a) => a.id !== id),
      }));
      useToastStore.getState().info("Alert deleted");
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to delete alert", msg);
      throw e;
    }
  },

  toggleAlert: async (id) => {
    const client = requireClient();
    const alert = get().alerts.find((a) => a.id === id);
    if (!alert) return;
    try {
      const updated = await client.alerts.update(id, {
        enabled: !alert.enabled,
      });
      set((s) => ({
        alerts: s.alerts.map((a) => (a.id === id ? updated : a)),
      }));
      useToastStore
        .getState()
        .success(updated.enabled ? "Alert enabled" : "Alert disabled");
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to toggle alert", msg);
      throw e;
    }
  },
}));
