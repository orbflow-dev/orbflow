import { create } from "zustand";
import type {
  ChangeRequest,
  ChangeRequestStatus,
  CreateChangeRequestInput,
  ReviewComment,
  AddCommentInput,
} from "../types/api";
import type { ApiClient } from "../client/api-client";
import { createStoreClient } from "./store-client";
import { useToastStore } from "./toast-store";

/* ═══════════════════════════════════════════════════════
   API Client injection — call once at app startup
   ═══════════════════════════════════════════════════════ */

const { setClient: setChangeRequestApiClient, requireClient } = createStoreClient("ChangeRequest");
export { setChangeRequestApiClient };

/* ═══════════════════════════════════════════════════════
   Store
   ═══════════════════════════════════════════════════════ */

interface ChangeRequestStoreState {
  changeRequests: ChangeRequest[];
  selectedCR: ChangeRequest | null;
  loading: boolean;
  error: string | null;

  fetchChangeRequests: (workflowId: string, status?: ChangeRequestStatus) => Promise<void>;
  selectChangeRequest: (workflowId: string, crId: string) => Promise<void>;
  createChangeRequest: (workflowId: string, input: CreateChangeRequestInput) => Promise<ChangeRequest>;
  updateCR: (workflowId: string, crId: string, input: Partial<CreateChangeRequestInput>) => Promise<ChangeRequest>;
  submitCR: (workflowId: string, crId: string) => Promise<void>;
  approveCR: (workflowId: string, crId: string) => Promise<void>;
  rejectCR: (workflowId: string, crId: string, reason?: string) => Promise<void>;
  mergeCR: (workflowId: string, crId: string) => Promise<void>;
  rebaseCR: (workflowId: string, crId: string) => Promise<void>;
  addComment: (workflowId: string, crId: string, comment: AddCommentInput) => Promise<ReviewComment>;
  resolveComment: (workflowId: string, crId: string, commentId: string) => Promise<void>;
  clearSelection: () => void;
  clearError: () => void;
}

/** Helper: re-fetch a CR and reconcile both selectedCR and the list entry. */
async function refetchCR(
  client: ApiClient,
  workflowId: string,
  crId: string,
  set: (fn: (s: ChangeRequestStoreState) => Partial<ChangeRequestStoreState>) => void,
) {
  const fresh = await client.changeRequests.get(workflowId, crId);
  set((s) => ({
    changeRequests: s.changeRequests.map((cr) => (cr.id === crId ? fresh : cr)),
    selectedCR: s.selectedCR?.id === crId ? fresh : s.selectedCR,
  }));
}

export const useChangeRequestStore = create<ChangeRequestStoreState>((set) => ({
  changeRequests: [],
  selectedCR: null,
  loading: false,
  error: null,

  fetchChangeRequests: async (workflowId, status) => {
    const client = requireClient();
    set({ loading: true, error: null });
    try {
      const result = await client.changeRequests.list(workflowId, { status });
      set({ changeRequests: result.items, loading: false });
    } catch (e) {
      const msg = (e as Error).message;
      set({ error: msg, loading: false });
      useToastStore.getState().error("Failed to load change requests", msg);
      throw e;
    }
  },

  selectChangeRequest: async (workflowId, crId) => {
    const client = requireClient();
    set({ loading: true, error: null });
    try {
      const result = await client.changeRequests.get(workflowId, crId);
      set({ selectedCR: result, loading: false });
    } catch (e) {
      const msg = (e as Error).message;
      set({ error: msg, loading: false });
      useToastStore.getState().error("Failed to load change request", msg);
      throw e;
    }
  },

  createChangeRequest: async (workflowId, input) => {
    const client = requireClient();
    try {
      const created = await client.changeRequests.create(workflowId, input);
      set((s) => ({
        changeRequests: [created, ...s.changeRequests],
      }));
      useToastStore.getState().success("Change request created");
      return created;
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to create change request", msg);
      throw e;
    }
  },

  updateCR: async (workflowId, crId, input) => {
    const client = requireClient();
    try {
      const updated = await client.changeRequests.update(workflowId, crId, input);
      set((s) => ({
        changeRequests: s.changeRequests.map((cr) =>
          cr.id === crId ? updated : cr
        ),
        selectedCR: s.selectedCR?.id === crId ? updated : s.selectedCR,
      }));
      useToastStore.getState().success("Change request updated");
      return updated;
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to update change request", msg);
      throw e;
    }
  },

  submitCR: async (workflowId, crId) => {
    const client = requireClient();
    try {
      await client.changeRequests.submit(workflowId, crId);
      await refetchCR(client, workflowId, crId, set);
      useToastStore.getState().success("Change request submitted for review");
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to submit change request", msg);
      throw e;
    }
  },

  approveCR: async (workflowId, crId) => {
    const client = requireClient();
    try {
      await client.changeRequests.approve(workflowId, crId);
      await refetchCR(client, workflowId, crId, set);
      useToastStore.getState().success("Change request approved");
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to approve change request", msg);
      throw e;
    }
  },

  rejectCR: async (workflowId, crId, reason) => {
    const client = requireClient();
    try {
      await client.changeRequests.reject(workflowId, crId, reason);
      await refetchCR(client, workflowId, crId, set);
      useToastStore.getState().success("Change request rejected");
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to reject change request", msg);
      throw e;
    }
  },

  mergeCR: async (workflowId, crId) => {
    const client = requireClient();
    try {
      await client.changeRequests.merge(workflowId, crId);
      await refetchCR(client, workflowId, crId, set);
      useToastStore.getState().success("Change request merged successfully");
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to merge change request", msg);
      throw e;
    }
  },

  rebaseCR: async (workflowId, crId) => {
    const client = requireClient();
    try {
      await client.changeRequests.rebase(workflowId, crId);
      await refetchCR(client, workflowId, crId, set);
      useToastStore.getState().success("Change request rebased to latest version");
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to rebase change request", msg);
      throw e;
    }
  },

  addComment: async (workflowId, crId, comment) => {
    const client = requireClient();
    try {
      const result = await client.changeRequests.addComment(workflowId, crId, comment);
      set((s) => ({
        selectedCR:
          s.selectedCR?.id === crId
            ? { ...s.selectedCR, comments: [...s.selectedCR.comments, result] }
            : s.selectedCR,
      }));
      return result;
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to add comment", msg);
      throw e;
    }
  },

  resolveComment: async (workflowId, crId, commentId) => {
    const client = requireClient();
    try {
      await client.changeRequests.resolveComment(workflowId, crId, commentId);
      set((s) => ({
        selectedCR:
          s.selectedCR?.id === crId
            ? {
                ...s.selectedCR,
                comments: s.selectedCR.comments.map((c) =>
                  c.id === commentId ? { ...c, resolved: true } : c
                ),
              }
            : s.selectedCR,
      }));
    } catch (e) {
      const msg = (e as Error).message;
      useToastStore.getState().error("Failed to resolve comment", msg);
      throw e;
    }
  },

  clearSelection: () => set({ selectedCR: null }),

  clearError: () => set({ error: null }),
}));
