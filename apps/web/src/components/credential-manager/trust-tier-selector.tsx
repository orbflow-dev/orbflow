"use client";

import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import type { CredentialAccessTier } from "@/lib/api";

interface TierOption {
  readonly tier: CredentialAccessTier;
  readonly label: string;
  readonly icon: string;
  readonly description: string;
  readonly borderColor: string;
  readonly selectedBg: string;
  readonly selectedBorder: string;
  readonly iconColor: string;
  readonly badge?: string;
}

const TIER_OPTIONS: readonly TierOption[] = [
  {
    tier: "proxy",
    label: "Proxy",
    icon: "shield",
    description: "Orbflow proxies API calls -- plugin never sees your key",
    borderColor: "border-emerald-500/20",
    selectedBg: "bg-emerald-500/[0.06]",
    selectedBorder: "border-emerald-500/40",
    iconColor: "text-emerald-400",
    badge: "Recommended",
  },
  {
    tier: "scoped_token",
    label: "Scoped Token",
    icon: "clock",
    description: "Plugin gets a temporary limited token",
    borderColor: "border-amber-500/20",
    selectedBg: "bg-amber-500/[0.06]",
    selectedBorder: "border-amber-500/40",
    iconColor: "text-amber-400",
  },
  {
    tier: "raw",
    label: "Raw",
    icon: "alert-triangle",
    description: "Plugin receives your full API key",
    borderColor: "border-rose-500/20",
    selectedBg: "bg-rose-500/[0.06]",
    selectedBorder: "border-rose-500/40",
    iconColor: "text-rose-400",
  },
] as const;

interface TrustTierSelectorProps {
  value: CredentialAccessTier;
  onChange: (tier: CredentialAccessTier) => void;
  allowedDomains: string;
  onAllowedDomainsChange: (domains: string) => void;
}

export function TrustTierSelector({
  value,
  onChange,
  allowedDomains,
  onAllowedDomainsChange,
}: TrustTierSelectorProps) {
  return (
    <div>
      <div className="flex items-center gap-2 mb-3">
        <NodeIcon name="shield" className="w-3.5 h-3.5 text-orbflow-text-faint" />
        <h4 className="text-body-sm font-bold uppercase tracking-[0.12em] text-orbflow-text-faint">
          Access Tier
        </h4>
        <div className="flex-1 h-px bg-orbflow-border" />
      </div>

      <div className="space-y-2">
        {TIER_OPTIONS.map((option) => {
          const isSelected = value === option.tier;
          return (
            <button
              key={option.tier}
              type="button"
              onClick={() => onChange(option.tier)}
              className={cn(
                "w-full flex items-start gap-3 px-3.5 py-3 rounded-xl border transition-all text-left",
                isSelected
                  ? `${option.selectedBorder} ${option.selectedBg}`
                  : "border-orbflow-border hover:border-orbflow-border-hover hover:bg-orbflow-surface-hover"
              )}
            >
              <div
                className={cn(
                  "w-8 h-8 rounded-lg flex items-center justify-center shrink-0 mt-0.5",
                  isSelected ? option.iconColor : "text-orbflow-text-ghost"
                )}
                style={
                  isSelected
                    ? undefined
                    : { backgroundColor: "var(--orbflow-surface-hover)" }
                }
              >
                <NodeIcon name={option.icon} className="w-4 h-4" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      "text-body font-medium",
                      isSelected
                        ? "text-orbflow-text-secondary"
                        : "text-orbflow-text-muted"
                    )}
                  >
                    {option.label}
                  </span>
                  {option.badge && (
                    <span className="text-micro font-medium px-1.5 py-0.5 rounded-md bg-emerald-500/15 text-emerald-400">
                      {option.badge}
                    </span>
                  )}
                </div>
                <p className="text-body-sm text-orbflow-text-ghost mt-0.5">
                  {option.description}
                </p>
              </div>
              <div className="shrink-0 mt-1">
                <div
                  className={cn(
                    "w-4 h-4 rounded-full border-2 flex items-center justify-center transition-colors",
                    isSelected
                      ? option.selectedBorder.replace("/40", "")
                      : "border-orbflow-border"
                  )}
                >
                  {isSelected && (
                    <div
                      className={cn(
                        "w-2 h-2 rounded-full",
                        option.tier === "proxy" && "bg-emerald-400",
                        option.tier === "scoped_token" && "bg-amber-400",
                        option.tier === "raw" && "bg-rose-400"
                      )}
                    />
                  )}
                </div>
              </div>
            </button>
          );
        })}
      </div>

      {/* Raw tier warning banner */}
      {value === "raw" && (
        <div className="mt-3 flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-rose-500/10 border border-rose-500/20">
          <NodeIcon
            name="alert-triangle"
            className="w-4 h-4 text-rose-400 shrink-0 mt-0.5"
          />
          <p className="text-body-sm text-rose-300">
            Warning: The plugin will have full access to this credential. Only
            use this tier with plugins you fully trust.
          </p>
        </div>
      )}

      {/* Allowed Domains input -- shown when Proxy tier is selected */}
      {value === "proxy" && (
        <div className="mt-4">
          <label className="text-body font-medium text-orbflow-text-faint block mb-1.5">
            Allowed Domains
          </label>
          <input
            type="text"
            value={allowedDomains}
            onChange={(e) => onAllowedDomainsChange(e.target.value)}
            placeholder="api.openai.com, api.anthropic.com"
            className="w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5
              text-body-lg text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
              focus:outline-none focus:border-emerald-500/30 focus-visible:ring-2 focus-visible:ring-emerald-500/50 transition-colors"
          />
          <p className="text-body-sm text-orbflow-text-ghost mt-1.5">
            Only proxy requests to these domains (leave empty for any)
          </p>
        </div>
      )}
    </div>
  );
}

/** Small badge showing the access tier of a credential. */
export function TierBadge({ tier }: { tier?: CredentialAccessTier }) {
  if (!tier) return null;

  const config = {
    proxy: {
      label: "Proxied",
      className: "bg-emerald-500/15 text-emerald-400",
      icon: null as string | null,
    },
    scoped_token: {
      label: "Scoped",
      className: "bg-amber-500/15 text-amber-400",
      icon: null as string | null,
    },
    raw: {
      label: "Raw",
      className: "bg-rose-500/15 text-rose-400",
      icon: "alert-triangle" as string | null,
    },
  }[tier];

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-micro font-medium",
        config.className
      )}
    >
      {config.icon && (
        <NodeIcon name={config.icon} className="w-2.5 h-2.5" />
      )}
      {config.label}
    </span>
  );
}
