"use client";

import { NodeIcon } from "@/core/components/icons";
import type { CredentialTypeSchema } from "@/lib/api";

interface CredentialEmptyStateProps {
  credentialTypes: CredentialTypeSchema[];
  onCreateWithType: (type: string) => void;
}

/** Empty state panel shown when no credential is being edited */
export function CredentialEmptyState({
  credentialTypes,
  onCreateWithType,
}: CredentialEmptyStateProps) {
  return (
    <div className="flex-1 px-6 py-8 lg:px-8 animate-fade-in">
      <div className="mx-auto flex h-full w-full max-w-3xl items-center justify-center">
        <div className="w-full rounded-[28px] border border-orbflow-border/70 bg-orbflow-surface px-8 py-10 shadow-[0_24px_60px_var(--orbflow-shadow-colored)]">
          <div className="flex flex-col items-center text-center">
            <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-electric-indigo/10 animate-fade-in-up stagger-1">
              <NodeIcon name="shield" className="h-7 w-7 text-electric-indigo/60" />
            </div>
            <h3 className="mb-1 text-title font-medium text-orbflow-text-secondary animate-fade-in-up stagger-2">
              Credential Store
            </h3>
            <p className="mb-2 max-w-xs text-center text-body text-orbflow-text-muted animate-fade-in-up stagger-3">
              Securely store connection settings and secrets for your workflow nodes.
            </p>
            <p className="mb-6 max-w-xs text-center text-caption text-orbflow-text-faint animate-fade-in-up stagger-3">
              All credentials are encrypted at rest with AES-256-GCM.
            </p>
          </div>

          {credentialTypes.length > 0 && (
            <div className="mx-auto w-full max-w-md">
              <p className="mb-2.5 text-center text-body-sm font-medium text-orbflow-text-muted animate-fade-in-up stagger-4">
                Quick add
              </p>
              <div className="grid grid-cols-2 gap-2">
                {credentialTypes.map((t, i) => (
                  <button
                    key={t.type}
                    onClick={() => onCreateWithType(t.type)}
                    className="flex items-center gap-2.5 rounded-xl border border-orbflow-border px-3 py-3 text-left transition-all
                      hover:border-orbflow-border-hover hover:bg-orbflow-surface-hover
                      active:bg-orbflow-surface-hover/80 focus-visible:outline-none focus-visible:ring-2
                      focus-visible:ring-electric-indigo/50 animate-fade-in-up"
                    style={{ animationDelay: `${200 + i * 50}ms` }}
                  >
                    <div
                      className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg"
                      style={{ backgroundColor: `${t.color}18` }}
                    >
                      <NodeIcon
                        name={t.icon}
                        className="h-4 w-4"
                        style={{ color: t.color }}
                      />
                    </div>
                    <div className="min-w-0">
                      <div className="text-body font-medium text-orbflow-text-secondary">
                        {t.name}
                      </div>
                      <div className="truncate text-caption text-orbflow-text-ghost">
                        {t.description}
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
