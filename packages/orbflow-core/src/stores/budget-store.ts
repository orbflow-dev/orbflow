import { create } from "zustand";
import type {
  AccountBudget,
  CreateBudgetInput,
  CostAnalytics,
} from "../types/api";
import { createStoreClient } from "./store-client";
import { useToastStore } from "./toast-store";

/* ═══════════════════════════════════════════════════════
   API Client injection — call once at app startup
   ═══════════════════════════════════════════════════════ */

const { setClient: setBudgetApiClient, requireClient } = createStoreClient("Budget");
export { setBudgetApiClient };

/* ═══════════════════════════════════════════════════════
   Store
   ═══════════════════════════════════════════════════════ */

interface BudgetStore {
  budgets: AccountBudget[];
  costAnalytics: CostAnalytics | null;
  loading: boolean;
  error: string | null;

  fetchBudgets: () => Promise<void>;
  createBudget: (budget: CreateBudgetInput) => Promise<AccountBudget>;
  updateBudget: (
    id: string,
    budget: Partial<CreateBudgetInput>
  ) => Promise<AccountBudget>;
  deleteBudget: (id: string) => Promise<void>;
  fetchCosts: (range?: string) => Promise<void>;
}

export const useBudgetStore = create<BudgetStore>((set) => ({
  budgets: [],
  costAnalytics: null,
  loading: false,
  error: null,

  fetchBudgets: async () => {
    const client = requireClient();
    set({ loading: true, error: null });
    try {
      const budgets = await client.budgets.list();
      set({ budgets, loading: false });
    } catch (e) {
      const msg = (e as Error).message;
      set({ error: msg, loading: false });
      useToastStore.getState().error("Failed to load budgets", msg);
      throw e;
    }
  },

  createBudget: async (budget) => {
    const client = requireClient();
    try {
      const created = await client.budgets.create(budget);
      set((s) => ({
        budgets: [created, ...s.budgets],
      }));
      useToastStore
        .getState()
        .success("Budget created", "New budget has been saved");
      return created;
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to create budget", msg);
      throw e;
    }
  },

  updateBudget: async (id, budget) => {
    const client = requireClient();
    try {
      const updated = await client.budgets.update(id, budget);
      set((s) => ({
        budgets: s.budgets.map((b) => (b.id === id ? updated : b)),
      }));
      useToastStore.getState().success("Budget updated");
      return updated;
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to update budget", msg);
      throw e;
    }
  },

  deleteBudget: async (id) => {
    const client = requireClient();
    try {
      await client.budgets.delete(id);
      set((s) => ({
        budgets: s.budgets.filter((b) => b.id !== id),
      }));
      useToastStore.getState().info("Budget deleted");
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to delete budget", msg);
      throw e;
    }
  },

  fetchCosts: async (range) => {
    const client = requireClient();
    try {
      const costAnalytics = await client.budgets.costs(range);
      set({ costAnalytics });
    } catch (e) {
      // No toast — caller handles retry silently
      throw e;
    }
  },
}));
