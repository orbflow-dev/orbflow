"use client";

import type { PluginSummary } from "@orbflow/core/types";
import { NodeIcon } from "@/core";
import { cn } from "@/lib/cn";

const CATEGORY_COLORS: Record<string, { bg: string; text: string; glow: string; icon: string }> = {
  ai:            { bg: "from-violet-500/15 to-purple-500/10", text: "text-violet-400",  glow: "shadow-violet-500/10", icon: "brain" },
  database:      { bg: "from-emerald-500/15 to-green-500/10", text: "text-emerald-400", glow: "shadow-emerald-500/10", icon: "database" },
  communication: { bg: "from-sky-500/15 to-blue-500/10",      text: "text-sky-400",     glow: "shadow-sky-500/10", icon: "mail" },
  utility:       { bg: "from-amber-500/15 to-orange-500/10",  text: "text-amber-400",   glow: "shadow-amber-500/10", icon: "settings" },
  monitoring:    { bg: "from-cyan-500/15 to-teal-500/10",     text: "text-cyan-400",    glow: "shadow-cyan-500/10", icon: "bar-chart" },
  security:      { bg: "from-rose-500/15 to-red-500/10",      text: "text-rose-400",    glow: "shadow-rose-500/10", icon: "shield" },
  cloud:         { bg: "from-blue-500/15 to-indigo-500/10",   text: "text-blue-400",    glow: "shadow-blue-500/10", icon: "cloud" },
  integration:   { bg: "from-fuchsia-500/15 to-pink-500/10",  text: "text-fuchsia-400", glow: "shadow-fuchsia-500/10", icon: "link" },
};

const DEFAULT_CATEGORY = { bg: "from-electric-indigo/15 to-electric-indigo/5", text: "text-electric-indigo", glow: "shadow-electric-indigo/10", icon: "plug" };

function formatDownloads(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}k`;
  return String(count);
}

interface PluginCardProps {
  readonly plugin: PluginSummary;
  readonly onClick: (plugin: PluginSummary) => void;
}

export function PluginCard({ plugin, onClick }: PluginCardProps) {
  const cat = CATEGORY_COLORS[plugin.category ?? ""] ?? DEFAULT_CATEGORY;
  const iconName = plugin.icon ?? cat.icon;

  return (
    <button
      type="button"
      onClick={() => onClick(plugin)}
      className={cn(
        "marketplace-card",
        "w-full h-full text-left rounded-xl border border-orbflow-border/50 bg-orbflow-surface",
        "hover:border-orbflow-border hover:shadow-lg transition-all duration-250",
        "p-5 flex flex-col gap-3.5 group relative overflow-hidden",
        "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
        `hover:${cat.glow}`,
      )}
    >
      {/* Subtle gradient glow on hover */}
      <div className={cn(
        "absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none",
        `bg-gradient-to-br ${cat.bg}`,
      )} />

      {/* Content */}
      <div className="relative flex-1">
        {/* Header: icon + name */}
        <div className="flex items-start gap-3.5">
          <div className={cn(
            "w-11 h-11 rounded-xl flex items-center justify-center shrink-0",
            "bg-gradient-to-br shadow-md transition-transform duration-200 group-hover:scale-105",
            cat.bg, cat.glow,
          )}>
            <NodeIcon name={iconName} className={cn("w-5 h-5", cat.text)} />
          </div>
          <div className="min-w-0 flex-1 pt-0.5">
            <h3 className="text-sm font-semibold text-orbflow-text-secondary truncate
              group-hover:text-electric-indigo transition-colors duration-200">
              {plugin.name}
            </h3>
            <p className="text-xs text-orbflow-text-muted mt-1 line-clamp-2 leading-relaxed">
              {plugin.description ?? "No description"}
            </p>
          </div>
        </div>
      </div>

      {/* Meta row */}
      <div className="relative flex items-center gap-2.5 text-xs text-orbflow-text-ghost">
        <span className="inline-flex items-center rounded-md bg-orbflow-bg/60 px-1.5 py-0.5 font-mono text-[11px] ring-1 ring-orbflow-border/30">
          v{plugin.latest_version}
        </span>
        <span className="inline-flex items-center gap-1">
          <NodeIcon name="download" className="w-3 h-3 opacity-60" />
          {formatDownloads(plugin.downloads)}
        </span>
        <span className="ml-auto text-orbflow-text-ghost/70 truncate text-[11px]">
          by {plugin.author ?? "Unknown"}
        </span>
      </div>

      {/* Tags + badges */}
      <div className="relative flex flex-wrap gap-1.5">
        {plugin.installed && (
          <span className="inline-flex items-center gap-1 rounded-md bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-400 ring-1 ring-emerald-500/20">
            <NodeIcon name="check-circle" className="w-2.5 h-2.5" />
            Installed
          </span>
        )}
        {plugin.update_available && (
          <span className="inline-flex items-center gap-1 rounded-md bg-amber-500/15 px-2 py-0.5 text-[10px] font-semibold text-amber-400 ring-1 ring-amber-500/20">
            <NodeIcon name="arrow-up-circle" className="w-2.5 h-2.5" />
            Update
          </span>
        )}
        {plugin.category && (
          <span className={cn(
            "inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider",
            cat.text, "bg-current/10",
          )}>
            <span className={cn("opacity-100", cat.text)}>{plugin.category}</span>
          </span>
        )}
        {plugin.tags.slice(0, 2).map((tag) => (
          <span
            key={tag}
            className="rounded-md bg-orbflow-surface-hover/70 px-2 py-0.5 text-[10px] font-medium text-orbflow-text-ghost ring-1 ring-orbflow-border/20"
          >
            {tag}
          </span>
        ))}
      </div>
    </button>
  );
}
