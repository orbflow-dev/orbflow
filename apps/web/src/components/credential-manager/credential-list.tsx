"use client";

import { useState, useMemo } from "react";
import { NodeIcon } from "@/core/components/icons";
import { SkeletonRow } from "@/core/components/skeleton";
import { EmptyState } from "@/core/components/empty-state";
import { cn } from "@/lib/cn";
import type { CredentialSummary, CredentialTypeSchema } from "@/lib/api";
import { TierBadge } from "./trust-tier-selector";

interface CredentialListProps {
  credentials: CredentialSummary[];
  loading: boolean;
  filterType: string | null;
  onFilterTypeChange: (type: string | null) => void;
  onEdit: (cred: CredentialSummary) => void;
  onDelete: (cred: CredentialSummary) => void;
  onCreate: () => void;
  editingId: string | null;
  showForm: boolean;
  isNarrow: boolean;
  credentialTypes: CredentialTypeSchema[];
  getSchemaForType: (type: string) => CredentialTypeSchema | undefined;
}

export function CredentialList({
  credentials,
  loading,
  filterType,
  onFilterTypeChange,
  onEdit,
  onDelete,
  onCreate,
  editingId,
  showForm,
  isNarrow,
  credentialTypes,
  getSchemaForType,
}: CredentialListProps) {
  const [search, setSearch] = useState("");

  const visibleCredentials = useMemo(
    () =>
      search.trim()
        ? credentials.filter((c) =>
            c.name.toLowerCase().includes(search.trim().toLowerCase())
          )
        : credentials,
    [credentials, search]
  );

  return (
    <div
      className={cn(
        "border-r border-orbflow-border flex flex-col bg-orbflow-bg",
        isNarrow ? (showForm ? "hidden" : "flex-1") : "w-80"
      )}
    >
      <div className="p-4 border-b border-orbflow-border">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <h2 className="text-heading font-semibold text-orbflow-text-secondary">
              Credentials
            </h2>
            {credentials.length > 0 && (
              <span className="text-micro font-medium px-1.5 py-0.5 rounded-md bg-orbflow-surface-hover text-orbflow-text-ghost">
                {credentials.length}
              </span>
            )}
          </div>
          <button
            onClick={onCreate}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body font-medium
              bg-electric-indigo/10 text-electric-indigo hover:bg-electric-indigo/20
              active:bg-electric-indigo/25 transition-colors
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          >
            <NodeIcon name="plus" className="w-3 h-3" />
            New
          </button>
        </div>

        {/* Search input */}
        {credentials.length > 3 && (
          <div className="relative mb-2.5">
            <NodeIcon
              name="search"
              className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-orbflow-text-ghost pointer-events-none"
            />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search credentials..."
              className="w-full rounded-lg border border-orbflow-border bg-orbflow-surface pl-8.5 pr-3 py-2
                text-body text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
                focus:outline-none focus:border-electric-indigo/30
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
            />
            {search && (
              <button
                onClick={() => setSearch("")}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 p-0.5 rounded text-orbflow-text-ghost
                  hover:text-orbflow-text-muted transition-colors"
                aria-label="Clear search"
              >
                <NodeIcon name="x" className="w-3 h-3" />
              </button>
            )}
          </div>
        )}

        {/* Type filter pills */}
        {credentialTypes.length > 0 && (
          <div className="flex flex-wrap gap-1">
            <button
              onClick={() => onFilterTypeChange(null)}
              className={cn(
                "px-2 py-1 rounded-md text-caption font-medium transition-colors",
                filterType === null
                  ? "bg-electric-indigo/15 text-electric-indigo"
                  : "text-orbflow-text-muted hover:bg-orbflow-surface-hover"
              )}
            >
              All
            </button>
            {credentialTypes
              .filter((t) => t.type !== "custom")
              .map((t) => (
                <button
                  key={t.type}
                  onClick={() =>
                    onFilterTypeChange(filterType === t.type ? null : t.type)
                  }
                    className={cn(
                      "px-2 py-1 rounded-md text-caption font-medium transition-colors",
                      filterType === t.type
                        ? "text-white"
                        : "text-orbflow-text-muted hover:bg-orbflow-surface-hover"
                    )}
                  style={
                    filterType === t.type
                      ? { backgroundColor: t.color }
                      : undefined
                  }
                >
                  {t.name}
                </button>
              ))}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto custom-scrollbar">
        {loading && credentials.length === 0 ? (
          <div className="p-3 space-y-1">
            <SkeletonRow />
            <SkeletonRow widths={["w-20", "w-12"]} />
            <SkeletonRow widths={["w-28", "w-14"]} />
            <SkeletonRow widths={["w-16", "w-10"]} />
          </div>
        ) : visibleCredentials.length === 0 ? (
          search.trim() ? (
            <div className="flex flex-col items-center justify-center px-6 py-12 text-center">
              <NodeIcon name="search" className="w-5 h-5 text-orbflow-text-ghost mb-2" />
              <p className="text-body text-orbflow-text-faint">
                No credentials matching &ldquo;{search.trim()}&rdquo;
              </p>
            </div>
          ) : (
            <EmptyState
              icon="key"
              title={
                filterType
                  ? "No credentials of this type"
                  : "No credentials yet"
              }
              description="Store connection settings and secrets for your workflows."
              action={
                !filterType
                  ? { label: "Add Credential", onClick: onCreate }
                  : undefined
              }
            />
          )
        ) : (
          <div aria-label="Saved credentials" role="list" className="p-2 space-y-0.5">
            {visibleCredentials.map((cred, index) => {
              const schema = getSchemaForType(cred.type);
              return (
                <div
                  key={cred.id}
                  role="listitem"
                  aria-label={`Edit credential: ${cred.name}`}
                  tabIndex={0}
                  onClick={() => onEdit(cred)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onEdit(cred);
                    }
                  }}
                  className={cn(
                    "w-full text-left rounded-lg px-3 py-2.5 transition-all duration-150 group cursor-pointer",
                    "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                    "animate-fade-in-up",
                    editingId === cred.id
                      ? "bg-electric-indigo/10 border border-electric-indigo/20"
                      : "hover:bg-orbflow-surface-hover border border-transparent"
                  )}
                  style={{ animationDelay: `${Math.min(index * 30, 150)}ms` }}
                >
                  <div className="flex items-center gap-2.5">
                    <div
                      className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
                      style={{
                        backgroundColor: `${schema?.color || "#6B7280"}15`,
                      }}
                    >
                      <NodeIcon
                        name={schema?.icon || "key"}
                        className="w-4 h-4"
                        style={{ color: schema?.color || "#6B7280" }}
                      />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-1.5">
                        <span className="text-body-lg font-medium text-orbflow-text-secondary truncate">
                          {cred.name}
                        </span>
                        <TierBadge tier={cred.access_tier} />
                      </div>
                      <div className="flex items-center gap-1.5">
                        <span className="text-body-sm text-orbflow-text-ghost truncate">
                          {schema?.name || cred.type}
                        </span>
                        {cred.description && (
                          <>
                            <span className="text-orbflow-text-ghost/40">·</span>
                            <span className="text-body-sm text-orbflow-text-ghost/70 truncate">
                              {cred.description}
                            </span>
                          </>
                        )}
                      </div>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onDelete(cred);
                      }}
                      className={cn(
                        "p-1.5 rounded-md text-orbflow-text-ghost transition-all",
                        "hover:text-rose-400 hover:bg-rose-400/10",
                        "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                        "sm:opacity-0 sm:group-hover:opacity-100 sm:focus-visible:opacity-100"
                      )}
                      aria-label={`Delete credential ${cred.name}`}
                      title="Delete"
                    >
                      <NodeIcon name="trash" className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
