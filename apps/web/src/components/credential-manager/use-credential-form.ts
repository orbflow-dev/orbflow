"use client";

import { useCallback, useEffect, useReducer, useMemo } from "react";
import { useCredentialStore } from "@/store/credential-store";
import type { CredentialSummary, CredentialTypeSchema, CredentialAccessTier } from "@/lib/api";

export interface FormState {
  name: string;
  type: string;
  description: string;
  data: Record<string, unknown>;
  accessTier: CredentialAccessTier;
  allowedDomains: string;
  saving: boolean;
  editingId: string | null;
  showForm: boolean;
}

export type FormAction =
  | { type: "SET_NAME"; name: string }
  | { type: "SET_TYPE"; formType: string; defaults: Record<string, unknown> }
  | { type: "SET_DESCRIPTION"; description: string }
  | { type: "SET_DATA"; data: Record<string, unknown> }
  | { type: "SET_FIELD"; key: string; value: unknown }
  | { type: "ADD_CUSTOM_FIELD"; key: string }
  | { type: "REMOVE_CUSTOM_FIELD"; key: string }
  | { type: "SET_SAVING"; saving: boolean }
  | { type: "SET_ACCESS_TIER"; tier: CredentialAccessTier }
  | { type: "SET_ALLOWED_DOMAINS"; domains: string }
  | { type: "START_EDIT"; id: string; name: string; formType: string; description: string; accessTier?: CredentialAccessTier; allowedDomains?: string }
  | { type: "START_CREATE"; formType?: string; defaults?: Record<string, unknown> }
  | { type: "RESET" };

const INITIAL_STATE: FormState = {
  name: "",
  type: "",
  description: "",
  data: {},
  accessTier: "proxy",
  allowedDomains: "",
  saving: false,
  editingId: null,
  showForm: false,
};

function formReducer(state: FormState, action: FormAction): FormState {
  switch (action.type) {
    case "SET_NAME":
      return { ...state, name: action.name };
    case "SET_TYPE":
      return { ...state, type: action.formType, data: action.defaults };
    case "SET_DESCRIPTION":
      return { ...state, description: action.description };
    case "SET_DATA":
      return { ...state, data: action.data };
    case "SET_FIELD":
      return { ...state, data: { ...state.data, [action.key]: action.value } };
    case "ADD_CUSTOM_FIELD":
      return { ...state, data: { ...state.data, [action.key]: "" } };
    case "REMOVE_CUSTOM_FIELD": {
      const next = { ...state.data };
      delete next[action.key];
      return { ...state, data: next };
    }
    case "SET_SAVING":
      return { ...state, saving: action.saving };
    case "SET_ACCESS_TIER":
      return { ...state, accessTier: action.tier };
    case "SET_ALLOWED_DOMAINS":
      return { ...state, allowedDomains: action.domains };
    case "START_EDIT":
      return { ...state, editingId: action.id, name: action.name, type: action.formType, description: action.description ?? "", data: {}, accessTier: action.accessTier ?? "proxy", allowedDomains: action.allowedDomains ?? "", showForm: true };
    case "START_CREATE":
      return { ...INITIAL_STATE, showForm: true, type: action.formType ?? "", data: action.defaults ?? {} };
    case "RESET":
      return INITIAL_STATE;
    default:
      return state;
  }
}

export function useCredentialForm(credentialTypes: CredentialTypeSchema[]) {
  const { selectedCredential, selectCredential, createCredential, updateCredential, clearSelectedCredential } = useCredentialStore();

  const [form, dispatch] = useReducer(formReducer, INITIAL_STATE);

  const activeSchema = useMemo(
    () => credentialTypes.find((s) => s.type === form.type) ?? null,
    [credentialTypes, form.type]
  );

  const getDefaults = useCallback((type: string) => {
    const schema = credentialTypes.find((s) => s.type === type);
    if (!schema?.fields) return {};
    const defaults: Record<string, unknown> = {};
    for (const field of schema.fields) {
      if (field.default !== undefined) defaults[field.key] = field.default;
    }
    return defaults;
  }, [credentialTypes]);

  const handleTypeChange = useCallback((newType: string) => {
    dispatch({ type: "SET_TYPE", formType: newType, defaults: getDefaults(newType) });
  }, [getDefaults]);

  const resetForm = useCallback(() => {
    dispatch({ type: "RESET" });
    clearSelectedCredential();
  }, [clearSelectedCredential]);

  const handleCreate = useCallback((preselectedType?: string) => {
    dispatch({
      type: "START_CREATE",
      formType: preselectedType,
      defaults: preselectedType ? getDefaults(preselectedType) : undefined,
    });
  }, [getDefaults]);

  const handleEdit = useCallback(async (cred: CredentialSummary) => {
    await selectCredential(cred.id);
    dispatch({
      type: "START_EDIT",
      id: cred.id,
      name: cred.name,
      formType: cred.type,
      description: cred.description,
      accessTier: cred.access_tier,
      allowedDomains: cred.policy?.allowed_domains?.join(", ") ?? "",
    });
  }, [selectCredential]);

  // Populate formData when selectedCredential loads during edit
  useEffect(() => {
    if (form.editingId && selectedCredential?.id === form.editingId && selectedCredential.data) {
      dispatch({ type: "SET_DATA", data: { ...selectedCredential.data } });
    }
  }, [form.editingId, selectedCredential]);

  const handleSave = useCallback(async () => {
    if (!form.name.trim() || !form.type) return;
    dispatch({ type: "SET_SAVING", saving: true });
    try {
      const parsedDomains = form.allowedDomains
        .split(",")
        .map((d) => d.trim())
        .filter(Boolean);

      const payload = {
        name: form.name,
        type: form.type,
        description: form.description,
        data: form.data,
        access_tier: form.accessTier,
        policy: {
          allowed_tiers: [form.accessTier],
          allowed_domains: parsedDomains,
          rate_limit_per_minute: 0,
        },
      };
      if (form.editingId) {
        await updateCredential(form.editingId, payload);
      } else {
        await createCredential(payload);
      }
      resetForm();
    } catch (err) {
      // Toast handled by store
      // Toast handled by store
    } finally {
      dispatch({ type: "SET_SAVING", saving: false });
    }
  }, [form, createCredential, updateCredential, resetForm]);

  const handleFieldChange = useCallback((key: string, value: unknown) => {
    dispatch({ type: "SET_FIELD", key, value });
  }, []);

  const handleAddCustomField = useCallback(() => {
    dispatch({ type: "ADD_CUSTOM_FIELD", key: `field_${Object.keys(form.data).length + 1}` });
  }, [form.data]);

  const handleRemoveCustomField = useCallback((key: string) => {
    dispatch({ type: "REMOVE_CUSTOM_FIELD", key });
  }, []);

  return {
    form,
    dispatch,
    activeSchema,
    handleTypeChange,
    resetForm,
    handleCreate,
    handleEdit,
    handleSave,
    handleFieldChange,
    handleAddCustomField,
    handleRemoveCustomField,
  };
}
