"use client";

import { useEffect, useState, useCallback, useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { useMediaQuery } from "@/hooks/use-media-query";
import { useCredentialStore } from "@/store/credential-store";
import { ConfirmDialog } from "@/core/components/confirm-dialog";
import { cn } from "@/lib/cn";
import { api } from "@/lib/api";
import type { CredentialSummary, CredentialTypeSchema } from "@/lib/api";
import { CredentialList } from "./credential-list";
import { CredentialForm } from "./credential-form";
import { CredentialEmptyState } from "./credential-empty-state";
import { useCredentialForm } from "./use-credential-form";

export function CredentialManager() {
  const { credentials, loading, fetchCredentials, deleteCredential } = useCredentialStore(
    useShallow((s) => ({
      credentials: s.credentials,
      loading: s.loading,
      fetchCredentials: s.fetchCredentials,
      deleteCredential: s.deleteCredential,
    })),
  );

  const [credentialTypes, setCredentialTypes] = useState<CredentialTypeSchema[]>([]);
  const [confirmDelete, setConfirmDelete] = useState<CredentialSummary | null>(null);
  const [filterType, setFilterType] = useState<string | null>(null);

  const {
    form,
    dispatch,
    activeSchema,
    handleTypeChange,
    resetForm,
    handleCreate,
    handleEdit,
    handleSave,
  } = useCredentialForm(credentialTypes);

  useEffect(() => {
    api.credentialTypes.list().then(setCredentialTypes).catch(() => { /* toast handled by store */ });
  }, []);

  useEffect(() => {
    fetchCredentials().catch(() => { /* store handles toast */ });
  }, [fetchCredentials]);

  // Clear secret credential data from global state on unmount
  useEffect(() => {
    return () => useCredentialStore.getState().clearSelectedCredential();
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    if (!confirmDelete) return;
    await deleteCredential(confirmDelete.id);
    setConfirmDelete(null);
    if (form.editingId === confirmDelete.id) resetForm();
  }, [confirmDelete, deleteCredential, form.editingId, resetForm]);

  const isNarrow = useMediaQuery("(max-width: 768px)");

  const filteredCredentials = useMemo(
    () => filterType ? credentials.filter((c) => c.type === filterType) : credentials,
    [credentials, filterType]
  );

  const getSchemaForType = useCallback(
    (type: string) => credentialTypes.find((s) => s.type === type),
    [credentialTypes]
  );

  return (
    <div className="flex h-full">
      <CredentialList
        credentials={filteredCredentials}
        loading={loading}
        filterType={filterType}
        onFilterTypeChange={setFilterType}
        onEdit={handleEdit}
        onDelete={setConfirmDelete}
        onCreate={() => handleCreate()}
        editingId={form.editingId}
        showForm={form.showForm}
        isNarrow={isNarrow}
        credentialTypes={credentialTypes}
        getSchemaForType={getSchemaForType}
      />

      <div className={cn(
        "flex-1 flex flex-col bg-orbflow-bg",
        isNarrow && !form.showForm && "hidden",
      )}>
        {form.showForm ? (
          <CredentialForm
            form={form}
            dispatch={dispatch}
            activeSchema={activeSchema}
            credentialTypes={credentialTypes}
            isNarrow={isNarrow}
            onTypeChange={handleTypeChange}
            onSave={handleSave}
            onCancel={resetForm}
          />
        ) : (
          <CredentialEmptyState
            credentialTypes={credentialTypes}
            onCreateWithType={handleCreate}
          />
        )}
      </div>

      {confirmDelete && (
        <ConfirmDialog
          title="Delete credential?"
          message={`"${confirmDelete.name}" will be permanently deleted. Any workflows referencing this credential will stop working.`}
          confirmLabel="Delete"
          cancelLabel="Cancel"
          variant="danger"
          onConfirm={handleConfirmDelete}
          onCancel={() => setConfirmDelete(null)}
        />
      )}
    </div>
  );
}
