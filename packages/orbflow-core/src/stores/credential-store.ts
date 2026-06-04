import { create } from "zustand";
import type {
  CredentialSummary,
  Credential,
  CredentialCreate,
} from "../types/api";
import { createStoreClient } from "./store-client";
import { useToastStore } from "./toast-store";
import { toMessage } from "./to-message";

/** Project credential to summary shape (excludes secret values). */
function credentialToSummary(c: Credential): CredentialSummary {
  return {
    id: c.id,
    name: c.name,
    type: c.type,
    description: c.description,
    access_tier: c.access_tier,
    policy: c.policy,
    created_at: c.created_at,
    updated_at: c.updated_at,
  };
}

const SECRET_DATA_KEY_RE =
  /credential|password|secret|token|api[_-]?key|apikey|private[_-]?key|access[_-]?key|client[_-]?secret/i;

function isRedactedValue(value: unknown): boolean {
  return (
    typeof value === "string" &&
    (value.toLowerCase().includes("redacted") || /[*\u2022]{3,}/.test(value))
  );
}

/** Drop secret-like or redacted fields before storing credential data in UI state. */
function credentialWithoutSecretData(c: Credential): Credential {
  const data: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(c.data ?? {})) {
    if (SECRET_DATA_KEY_RE.test(key) || isRedactedValue(value)) continue;
    data[key] = value;
  }
  return { ...c, data };
}

/* ═══════════════════════════════════════════════════════
   API Client injection — call once at app startup
   ═══════════════════════════════════════════════════════ */

const { setClient: setCredentialApiClient, requireClient } = createStoreClient("Credential");
export { setCredentialApiClient };

/* ═══════════════════════════════════════════════════════
   Store
   ═══════════════════════════════════════════════════════ */

interface CredentialStore {
  credentials: CredentialSummary[];
  selectedCredential: Credential | null;
  loading: boolean;
  error: string | null;

  fetchCredentials: () => Promise<void>;
  selectCredential: (id: string) => Promise<Credential>;
  createCredential: (cred: CredentialCreate) => Promise<Credential>;
  updateCredential: (
    id: string,
    cred: Partial<CredentialCreate>
  ) => Promise<Credential>;
  deleteCredential: (id: string) => Promise<void>;
  clearSelectedCredential: () => void;
}

export const useCredentialStore = create<CredentialStore>((set) => ({
  credentials: [],
  selectedCredential: null,
  loading: false,
  error: null,

  fetchCredentials: async () => {
    const client = requireClient();
    set({ loading: true, error: null });
    try {
      const credentials = await client.credentials.list();
      set({ credentials, loading: false });
    } catch (e) {
      const msg = toMessage(e);
      set({ error: msg, loading: false });
      useToastStore.getState().error("Failed to load credentials", msg);
    }
  },

  selectCredential: async (id) => {
    const client = requireClient();
    try {
      const cred = await client.credentials.get(id);
      const safeCredential = credentialWithoutSecretData(cred);
      set({ selectedCredential: safeCredential });
      return safeCredential;
    } catch (e) {
      const msg = toMessage(e);
      set({ error: msg });
      useToastStore.getState().error("Failed to load credential", msg);
      throw e;
    }
  },

  createCredential: async (cred) => {
    const client = requireClient();
    try {
      const created = await client.credentials.create(cred);
      const safeCreated = credentialWithoutSecretData(created);
      set((s) => ({
        credentials: [credentialToSummary(created), ...s.credentials],
      }));
      useToastStore
        .getState()
        .success("Credential created", `"${created.name}" has been saved`);
      return safeCreated;
    } catch (e) {
      const msg = toMessage(e);
      useToastStore.getState().error("Failed to create credential", msg);
      throw e;
    }
  },

  updateCredential: async (id, cred) => {
    const client = requireClient();
    try {
      const updated = await client.credentials.update(id, cred);
      const safeUpdated = credentialWithoutSecretData(updated);
      set((s) => ({
        credentials: s.credentials.map((c) =>
          c.id === id ? credentialToSummary(updated) : c
        ),
        selectedCredential:
          s.selectedCredential?.id === id ? safeUpdated : s.selectedCredential,
      }));
      useToastStore.getState().success("Credential updated");
      return safeUpdated;
    } catch (e) {
      const msg = toMessage(e);
      useToastStore.getState().error("Failed to update credential", msg);
      throw e;
    }
  },

  deleteCredential: async (id) => {
    const client = requireClient();
    try {
      await client.credentials.delete(id);
      set((s) => ({
        credentials: s.credentials.filter((c) => c.id !== id),
        selectedCredential:
          s.selectedCredential?.id === id ? null : s.selectedCredential,
      }));
      useToastStore.getState().info("Credential deleted");
    } catch (e) {
      const msg = toMessage(e);
      useToastStore.getState().error("Failed to delete credential", msg);
      throw e;
    }
  },

  clearSelectedCredential: () => set({ selectedCredential: null }),
}));
