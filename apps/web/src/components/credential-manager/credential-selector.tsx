"use client";

import { useEffect, useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { useCredentialStore } from "@/store/credential-store";

interface CredentialSelectorProps {
  value?: string;
  onChange: (id: string) => void;
  /** Comma-separated credential types to filter (e.g. "openai,anthropic,google_ai"). Shows all if omitted. */
  credentialType?: string;
}

export function CredentialSelector({ value, onChange, credentialType }: CredentialSelectorProps) {
  const { credentials, loading, error, fetchCredentials } = useCredentialStore(
    useShallow((s) => ({
      credentials: s.credentials,
      loading: s.loading,
      error: s.error,
      fetchCredentials: s.fetchCredentials,
    })),
  );

  useEffect(() => {
    if (credentials.length === 0 && !loading && !error) fetchCredentials().catch(() => { /* store handles toast */ });
  }, [credentials.length, loading, error, fetchCredentials]);

  // Filter credentials by type when credentialType is specified (supports comma-separated)
  const filtered = useMemo(() => {
    if (!credentialType) return credentials;
    const allowedTypes = new Set(credentialType.split(",").map((t) => t.trim()));
    return credentials.filter((c) => allowedTypes.has(c.type));
  }, [credentials, credentialType]);

  if (error) {
    return (
      <div className="flex items-center gap-2">
        <span className="text-body-sm text-rose-400">Failed to load credentials</span>
        <button
          onClick={fetchCredentials}
          className="text-body-sm font-medium text-electric-indigo hover:underline
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none rounded"
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <select
      value={value || ""}
      onChange={(e) => onChange(e.target.value)}
      disabled={loading}
      className="w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5
        text-body-lg text-orbflow-text-secondary transition-colors cursor-pointer
        disabled:opacity-50 disabled:cursor-not-allowed
        focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50
        hover:bg-orbflow-surface-hover"
    >
      <option value="" className="bg-orbflow-surface">
        {loading ? "Loading credentials..." : "Select credential..."}
      </option>
      {filtered.map((cred) => (
        <option key={cred.id} value={cred.id} className="bg-orbflow-surface">
          {cred.name} ({cred.type})
        </option>
      ))}
    </select>
  );
}
