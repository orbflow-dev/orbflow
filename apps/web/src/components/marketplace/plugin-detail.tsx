"use client";

import { useEffect, useState, useCallback } from "react";
import type { PluginDetail as PluginDetailType } from "@orbflow/core/types";
import { NodeIcon } from "@/core";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";

const CAT_GRADIENT: Record<string, string> = {
  ai: "from-violet-500/20 to-purple-500/10",
  database: "from-emerald-500/20 to-green-500/10",
  communication: "from-sky-500/20 to-blue-500/10",
  utility: "from-amber-500/20 to-orange-500/10",
  monitoring: "from-cyan-500/20 to-teal-500/10",
  security: "from-rose-500/20 to-red-500/10",
  cloud: "from-blue-500/20 to-indigo-500/10",
  integration: "from-fuchsia-500/20 to-pink-500/10",
};

const CAT_ACCENT: Record<string, string> = {
  ai: "text-violet-400", database: "text-emerald-400", communication: "text-sky-400",
  utility: "text-amber-400", monitoring: "text-cyan-400", security: "text-rose-400",
  cloud: "text-blue-400", integration: "text-fuchsia-400",
};

interface PluginDetailProps {
  readonly pluginName: string;
  readonly onClose: () => void;
  readonly onPluginChanged?: () => void;
}

export function PluginDetail({ pluginName, onClose, onPluginChanged }: PluginDetailProps) {
  const [plugin, setPlugin] = useState<PluginDetailType | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    api.marketplace
      .get(pluginName)
      .then((data) => { if (!cancelled) setPlugin(data); })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : "Failed to load plugin");
      })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [pluginName]);

  const handleBackdropClick = useCallback(
    (e: React.MouseEvent) => { if (e.target === e.currentTarget) onClose(); },
    [onClose],
  );

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Plugin: ${pluginName}`}
      onClick={handleBackdropClick}
      className="fixed inset-0 z-[80] flex justify-end bg-black/50 backdrop-blur-sm
        animate-[fadeIn_150ms_ease-out]"
    >
      <div className="w-full max-w-lg h-full bg-orbflow-bg border-l border-orbflow-border overflow-y-auto
        animate-[slideInRight_250ms_cubic-bezier(0.16,1,0.3,1)] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-orbflow-border shrink-0">
          <h2 className="text-sm font-semibold text-orbflow-text-secondary tracking-wide uppercase">
            Plugin Details
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center
              hover:bg-orbflow-surface-hover transition-colors
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            aria-label="Close"
          >
            <NodeIcon name="x" className="w-4 h-4 text-orbflow-text-muted" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1">
          {loading && <DetailSkeleton />}
          {error && (
            <div className="flex flex-col items-center justify-center py-16 text-center px-5">
              <div className="w-14 h-14 rounded-2xl bg-rose-500/10 ring-1 ring-rose-500/20 flex items-center justify-center mb-4">
                <NodeIcon name="alert-triangle" className="w-6 h-6 text-rose-400" />
              </div>
              <p className="text-sm text-orbflow-text-muted">{error}</p>
            </div>
          )}
          {plugin && !loading && (
            <DetailContent plugin={plugin} onPluginChanged={onPluginChanged} />
          )}
        </div>
      </div>
    </div>
  );
}

function DetailContent({
  plugin,
  onPluginChanged,
}: {
  readonly plugin: PluginDetailType;
  readonly onPluginChanged?: () => void;
}) {
  const iconName = plugin.icon ?? "package";
  const cat = plugin.category ?? "";
  const gradient = CAT_GRADIENT[cat] ?? "from-electric-indigo/20 to-electric-indigo/5";
  const accent = CAT_ACCENT[cat] ?? "text-electric-indigo";

  return (
    <div className="space-y-0">
      {/* Hero card with gradient */}
      <div className={cn("relative overflow-hidden p-6 bg-gradient-to-br", gradient)}>
        <div
          className="absolute top-0 right-0 w-48 h-48 opacity-30 blur-[60px]"
          style={{ background: "radial-gradient(circle, currentColor 0%, transparent 70%)" }}
        />
        <div className="relative flex items-start gap-4">
          <div className={cn(
            "w-16 h-16 rounded-2xl flex items-center justify-center shrink-0",
            "bg-orbflow-surface/90 backdrop-blur-sm border border-orbflow-border/60 shadow-lg",
          )}>
            <NodeIcon name={iconName} className={cn("w-8 h-8", accent)} />
          </div>
          <div className="min-w-0 flex-1 pt-1">
            <h3 className="text-xl font-bold text-orbflow-text-secondary tracking-tight">{plugin.name}</h3>
            <p className="text-sm text-orbflow-text-muted mt-1.5 leading-relaxed">{plugin.description ?? "No description"}</p>
            <div className="flex items-center gap-3 mt-3 text-xs text-orbflow-text-ghost">
              <span className="font-mono bg-orbflow-surface/80 rounded-md px-1.5 py-0.5 ring-1 ring-orbflow-border/40">
                v{plugin.version}
              </span>
              <span>by {plugin.author ?? "Unknown"}</span>
              <span className="inline-flex items-center gap-1">
                <NodeIcon name="download" className="w-3 h-3" />
                {(plugin.downloads ?? 0).toLocaleString()}
              </span>
            </div>
          </div>
        </div>
      </div>

      <div className="p-6 space-y-6">
        {/* Metadata grid */}
        <div className="grid grid-cols-3 gap-2.5">
          <MetaItem label="License" value={plugin.license || "—"} />
          <MetaItem label="Min Orbflow" value={plugin.orbflow_version ? `>=${plugin.orbflow_version}` : "Any"} />
          <MetaItem label="Language" value={plugin.language || "—"} />
        </div>

        {/* Repository */}
        {plugin.repository && (
          <div>
            <SectionTitle>Repository</SectionTitle>
            <a
              href={plugin.repository}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 text-sm text-electric-indigo
                hover:text-electric-indigo/80 transition-colors group"
            >
              <NodeIcon name="link" className="w-3.5 h-3.5 group-hover:scale-110 transition-transform" />
              <span className="truncate underline underline-offset-2 decoration-electric-indigo/30
                group-hover:decoration-electric-indigo/60">
                {plugin.repository.replace(/^https?:\/\//, "")}
              </span>
            </a>
          </div>
        )}

        {/* Node types */}
        {plugin.node_types.length > 0 && (
          <div>
            <SectionTitle>Nodes Provided ({plugin.node_types.length})</SectionTitle>
            <div className="space-y-1.5">
              {plugin.node_types.map((nt) => (
                <div
                  key={nt}
                  className="flex items-center gap-2.5 rounded-lg bg-orbflow-surface border border-orbflow-border/50
                    px-3 py-2 text-xs font-mono text-orbflow-text-muted"
                >
                  <div className={cn("w-5 h-5 rounded flex items-center justify-center", `bg-gradient-to-br ${gradient}`)}>
                    <NodeIcon name="package" className={cn("w-3 h-3", accent)} />
                  </div>
                  {nt}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Tags */}
        {plugin.tags.length > 0 && (
          <div>
            <SectionTitle>Tags</SectionTitle>
            <div className="flex flex-wrap gap-1.5">
              {plugin.tags.map((tag) => (
                <span
                  key={tag}
                  className="rounded-md bg-orbflow-surface-hover px-2.5 py-1 text-xs font-medium
                    text-orbflow-text-muted ring-1 ring-orbflow-border/20"
                >
                  {tag}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* README */}
        {plugin.readme && (
          <div>
            <SectionTitle>README</SectionTitle>
            <div className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4
              text-sm text-orbflow-text-muted whitespace-pre-wrap font-mono leading-relaxed
              max-h-72 overflow-y-auto scrollbar-thin">
              {plugin.readme}
            </div>
          </div>
        )}

        {/* Install / Uninstall buttons */}
        <PluginActions plugin={plugin} onPluginChanged={onPluginChanged} />
      </div>
    </div>
  );
}

function PluginActions({
  plugin,
  onPluginChanged,
}: {
  readonly plugin: PluginDetailType;
  readonly onPluginChanged?: () => void;
}) {
  const [installing, setInstalling] = useState(false);
  const [installStage, setInstallStage] = useState<string | null>(null);
  const [uninstalling, setUninstalling] = useState(false);
  const [installed, setInstalled] = useState(plugin.installed ?? false);
  const [error, setError] = useState<string | null>(null);
  const [showConfirm, setShowConfirm] = useState(false);

  const handleInstall = useCallback(async () => {
    setInstalling(true);
    setError(null);
    setInstallStage("Downloading...");
    try {
      // Staged progress (optimistic timing since install is a single request)
      const stageTimer1 = setTimeout(() => setInstallStage("Extracting..."), 1500);
      const stageTimer2 = setTimeout(() => setInstallStage("Verifying..."), 3000);
      await api.marketplace.install(plugin.name);
      clearTimeout(stageTimer1);
      clearTimeout(stageTimer2);
      setInstallStage("Complete!");
      setInstalled(true);
      onPluginChanged?.();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Installation failed");
    } finally {
      setInstalling(false);
      setInstallStage(null);
    }
  }, [plugin.name, onPluginChanged]);

  const handleUninstall = useCallback(async () => {
    setUninstalling(true);
    setError(null);
    setShowConfirm(false);
    try {
      await api.marketplace.uninstall(plugin.name);
      setInstalled(false);
      onPluginChanged?.();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Uninstall failed");
    } finally {
      setUninstalling(false);
    }
  }, [plugin.name, onPluginChanged]);

  if (installed) {
    return (
      <div className="pt-2 pb-4 space-y-3">
        <div
          className="w-full rounded-xl bg-emerald-500/15 text-emerald-400 py-3 text-sm font-semibold
            flex items-center justify-center gap-2 ring-1 ring-emerald-500/20"
        >
          <NodeIcon name="check-circle" className="w-4 h-4" />
          Installed
        </div>

        {showConfirm ? (
          <div className="rounded-xl border border-rose-500/30 bg-rose-500/5 p-4 space-y-3">
            <p className="text-xs text-orbflow-text-muted text-center">
              Are you sure you want to uninstall <strong>{plugin.name}</strong>? This will remove all plugin files.
            </p>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => setShowConfirm(false)}
                className="flex-1 rounded-lg border border-orbflow-border py-2 text-xs font-medium
                  text-orbflow-text-muted hover:bg-orbflow-surface-hover transition-all duration-200"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleUninstall}
                disabled={uninstalling}
                className="flex-1 rounded-lg bg-rose-500 text-white py-2 text-xs font-medium
                  flex items-center justify-center gap-1.5
                  hover:brightness-110 transition-all duration-200
                  disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {uninstalling ? (
                  <>
                    <NodeIcon name="loader" className="w-3.5 h-3.5 animate-spin" />
                    Removing...
                  </>
                ) : (
                  "Confirm Uninstall"
                )}
              </button>
            </div>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setShowConfirm(true)}
            className="w-full rounded-xl border border-rose-500/30 text-rose-400 py-2.5 text-sm font-medium
              flex items-center justify-center gap-2
              hover:bg-rose-500/10 transition-all duration-200"
          >
            <NodeIcon name="trash-2" className="w-4 h-4" />
            Uninstall
          </button>
        )}

        <p className="text-center text-[11px] text-orbflow-text-ghost/60">
          This plugin is active and available in the node picker.
        </p>
        {error && <p className="text-center text-[11px] text-rose-400">{error}</p>}
      </div>
    );
  }

  return (
    <div className="pt-2 pb-4">
      <button
        type="button"
        onClick={handleInstall}
        disabled={installing}
        className="w-full rounded-xl bg-electric-indigo text-white py-3 text-sm font-semibold
          flex items-center justify-center gap-2 shadow-md shadow-electric-indigo/20
          hover:shadow-lg hover:brightness-110 transition-all duration-200
          disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {installing ? (
          <>
            <NodeIcon name="loader" className="w-4 h-4 animate-spin" />
            {installStage ?? "Installing..."}
          </>
        ) : (
          <>
            <NodeIcon name="download" className="w-4 h-4" />
            Install Plugin
          </>
        )}
      </button>
      {error && (
        <p className="text-center text-[11px] text-rose-400 mt-2.5">{error}</p>
      )}
    </div>
  );
}

function SectionTitle({ children }: { readonly children: React.ReactNode }) {
  return (
    <h4 className="text-[11px] font-semibold text-orbflow-text-ghost uppercase tracking-widest mb-2.5">
      {children}
    </h4>
  );
}

function MetaItem({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="rounded-lg bg-orbflow-surface border border-orbflow-border/50 p-3">
      <p className="text-[10px] font-semibold text-orbflow-text-ghost/70 uppercase tracking-wider">{label}</p>
      <p className="text-sm font-medium text-orbflow-text-secondary mt-0.5 truncate">{value}</p>
    </div>
  );
}

function DetailSkeleton() {
  return (
    <div className="animate-pulse">
      <div className="h-36 bg-orbflow-surface-hover/30" />
      <div className="p-6 space-y-5">
        <div className="grid grid-cols-3 gap-2.5">
          {Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="h-14 rounded-lg bg-orbflow-surface-hover/50" />
          ))}
        </div>
        <div className="space-y-2">
          <div className="h-3 w-20 rounded bg-orbflow-surface-hover/50" />
          <div className="h-10 rounded-lg bg-orbflow-surface-hover/30" />
          <div className="h-10 rounded-lg bg-orbflow-surface-hover/30" />
        </div>
      </div>
    </div>
  );
}

