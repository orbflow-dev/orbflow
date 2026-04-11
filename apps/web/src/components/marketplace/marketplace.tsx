"use client";

import { useEffect, useState, useCallback, useRef } from "react";
import type { PluginSummary } from "@orbflow/core/types";
import { useOrbflow } from "@orbflow/core/context";
import { NodeIcon } from "@/core";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { PluginCard } from "./plugin-card";
import { PluginDetail } from "./plugin-detail";
import { SubmitPlugin } from "./submit-plugin";

const CATEGORIES = [
  { id: null, label: "All", icon: "layers" },
  { id: "ai", label: "AI", icon: "brain" },
  { id: "database", label: "Database", icon: "database" },
  { id: "communication", label: "Communication", icon: "mail" },
  { id: "utility", label: "Utility", icon: "settings" },
  { id: "monitoring", label: "Monitoring", icon: "bar-chart" },
  { id: "security", label: "Security", icon: "shield" },
  { id: "cloud", label: "Cloud", icon: "cloud" },
  { id: "integration", label: "Integration", icon: "link" },
] as const;

type CategoryId = (typeof CATEGORIES)[number]["id"];

const SORT_OPTIONS = [
  { value: "name:asc", label: "Name A-Z" },
  { value: "name:desc", label: "Name Z-A" },
  { value: "downloads:desc", label: "Most Downloads" },
] as const;

type TabId = "browse" | "installed";

const PAGE_SIZE = 20;

export function Marketplace() {
  const { refreshSchemas } = useOrbflow();
  const [plugins, setPlugins] = useState<PluginSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [activeCategory, setActiveCategory] = useState<CategoryId>(null);
  const [selectedPlugin, setSelectedPlugin] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>("browse");
  const [sortKey, setSortKey] = useState("name:asc");
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(0);
  const [showSubmit, setShowSubmit] = useState(false);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const abortRef = useRef<AbortController | null>(null);

  const fetchPlugins = useCallback(
    async (query: string, category: CategoryId, tab: TabId, sort: string, pageNum: number) => {
      // Cancel any in-flight request
      abortRef.current?.abort();
      const controller = new AbortController();
      abortRef.current = controller;

      const showLoadingTimer = setTimeout(() => setLoading(true), 300);
      setError(null);
      try {
        const [sortField, sortOrder] = sort.split(":");
        const params: {
          query?: string;
          category?: string;
          sort?: string;
          order?: string;
          installed_only?: boolean;
          limit: number;
          offset: number;
        } = {
          limit: PAGE_SIZE,
          offset: pageNum * PAGE_SIZE,
          sort: sortField,
          order: sortOrder,
        };
        if (query.trim()) params.query = query.trim();
        if (category) params.category = category;
        if (tab === "installed") params.installed_only = true;
        const result = await api.marketplace.list(params);
        // Ignore results from aborted requests
        if (controller.signal.aborted) return;
        setPlugins(result.items);
        setTotal(result.total);
      } catch (err: unknown) {
        if (controller.signal.aborted) return;
        const raw = err instanceof Error ? err.message : "";
        if (raw.includes("not configured") || raw.includes("Not Found")) {
          setPlugins([]);
          setTotal(0);
          setError(null);
        } else {
          setError("Unable to load plugins. Please try again.");
          setPlugins([]);
          setTotal(0);
        }
      } finally {
        clearTimeout(showLoadingTimer);
        if (!controller.signal.aborted) setLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    fetchPlugins("", null, "browse", "name:asc", 0);
  }, [fetchPlugins]);

  const refetch = useCallback(() => {
    fetchPlugins(searchQuery, activeCategory, activeTab, sortKey, page);
  }, [fetchPlugins, searchQuery, activeCategory, activeTab, sortKey, page]);

  const handleSearchChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = e.target.value;
      setSearchQuery(value);
      setPage(0);
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
      searchTimerRef.current = setTimeout(() => {
        fetchPlugins(value, activeCategory, activeTab, sortKey, 0);
      }, 300);
    },
    [fetchPlugins, activeCategory, activeTab, sortKey],
  );

  const cancelPendingSearch = useCallback(() => {
    if (searchTimerRef.current) {
      clearTimeout(searchTimerRef.current);
      searchTimerRef.current = null;
    }
  }, []);

  const handleCategoryClick = useCallback(
    (category: CategoryId) => {
      cancelPendingSearch();
      setActiveCategory(category);
      setPage(0);
      fetchPlugins(searchQuery, category, activeTab, sortKey, 0);
    },
    [fetchPlugins, searchQuery, activeTab, sortKey, cancelPendingSearch],
  );

  const handleTabChange = useCallback(
    (tab: TabId) => {
      cancelPendingSearch();
      setActiveTab(tab);
      setPage(0);
      fetchPlugins(searchQuery, activeCategory, tab, sortKey, 0);
    },
    [fetchPlugins, searchQuery, activeCategory, sortKey, cancelPendingSearch],
  );

  const handleSortChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      cancelPendingSearch();
      const value = e.target.value;
      setSortKey(value);
      setPage(0);
      fetchPlugins(searchQuery, activeCategory, activeTab, value, 0);
    },
    [fetchPlugins, searchQuery, activeCategory, activeTab, cancelPendingSearch],
  );

  const handlePageChange = useCallback(
    (newPage: number) => {
      cancelPendingSearch();
      setPage(newPage);
      fetchPlugins(searchQuery, activeCategory, activeTab, sortKey, newPage);
    },
    [fetchPlugins, searchQuery, activeCategory, activeTab, sortKey, cancelPendingSearch],
  );

  const handlePluginClick = useCallback((plugin: PluginSummary) => {
    setSelectedPlugin(plugin.name);
  }, []);

  const handleCloseDetail = useCallback(() => {
    setSelectedPlugin(null);
  }, []);

  const handlePluginChanged = useCallback(() => {
    refetch();
    refreshSchemas();
  }, [refetch, refreshSchemas]);

  useEffect(() => {
    return () => {
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
      abortRef.current?.abort();
    };
  }, []);

  // Keyboard shortcut: / to focus search
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "/" && !e.ctrlKey && !e.metaKey && document.activeElement?.tagName !== "INPUT") {
        e.preventDefault();
        searchInputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const isEmpty = !loading && !error && plugins.length === 0;
  const totalPages = Math.ceil(total / PAGE_SIZE);
  const showFrom = page * PAGE_SIZE + 1;
  const showTo = Math.min((page + 1) * PAGE_SIZE, total);

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Hero header with mesh gradient */}
      <div className="shrink-0 relative overflow-hidden">
        {/* Gradient mesh background */}
        <div className="absolute inset-0 bg-gradient-to-br from-electric-indigo/[0.08] via-transparent to-neon-cyan/[0.05]" />
        <div
          className="absolute top-0 right-0 w-[400px] h-[200px] opacity-20 blur-[80px]"
          style={{ background: "radial-gradient(circle, rgba(124,92,252,0.4) 0%, transparent 70%)" }}
        />
        <div
          className="absolute bottom-0 left-1/3 w-[300px] h-[150px] opacity-15 blur-[60px]"
          style={{ background: "radial-gradient(circle, rgba(34,211,238,0.3) 0%, transparent 70%)" }}
        />

        <div className="relative p-6 pb-5 space-y-5">
          {/* Title row */}
          <div className="flex items-start justify-between gap-4">
            <div className="flex items-center gap-4">
              <div className="w-11 h-11 rounded-xl bg-gradient-to-br from-electric-indigo to-electric-indigo/60 flex items-center justify-center shadow-lg shadow-electric-indigo/20">
                <NodeIcon name="plug" className="w-5 h-5 text-white" />
              </div>
              <div>
                <h2 className="text-lg font-bold text-orbflow-text-secondary tracking-tight">
                  Marketplace
                </h2>
                <p className="text-xs text-orbflow-text-ghost mt-0.5">
                  {total > 0
                    ? `${total} plugin${total !== 1 ? "s" : ""} available`
                    : "Discover plugins & integrations"}
                </p>
              </div>
            </div>
            <button
              type="button"
              onClick={() => setShowSubmit(true)}
              className="shrink-0 inline-flex items-center gap-2 rounded-lg bg-orbflow-surface
                border border-orbflow-border/50 px-4 py-2 text-xs font-medium text-orbflow-text-muted
                hover:text-orbflow-text-secondary hover:border-electric-indigo/40
                hover:bg-orbflow-surface-hover transition-all duration-200"
            >
              <NodeIcon name="upload" className="w-3.5 h-3.5" />
              Submit Plugin
            </button>
          </div>

          {/* Tabs: Browse | Installed */}
          <div className="flex items-center gap-6">
            <div role="tablist" aria-label="Plugin view" className="flex items-center gap-1 bg-orbflow-surface/60 rounded-lg p-1 ring-1 ring-orbflow-border/30">
              {(["browse", "installed"] as const).map((tab) => (
                <button
                  key={tab}
                  role="tab"
                  type="button"
                  id={`marketplace-tab-${tab}`}
                  aria-selected={activeTab === tab}
                  aria-controls="marketplace-tabpanel"
                  tabIndex={activeTab === tab ? 0 : -1}
                  onClick={() => handleTabChange(tab)}
                  className={cn(
                    "px-4 py-1.5 rounded-md text-xs font-semibold transition-all duration-200 capitalize",
                    activeTab === tab
                      ? "bg-electric-indigo text-white shadow-md shadow-electric-indigo/25"
                      : "text-orbflow-text-muted hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover",
                  )}
                >
                  {tab === "browse" ? "Browse" : "Installed"}
                </button>
              ))}
            </div>

            {/* Sort dropdown */}
            <select
              aria-label="Sort plugins by"
              value={sortKey}
              onChange={handleSortChange}
              className="rounded-lg border border-orbflow-border/50 bg-orbflow-bg/60 backdrop-blur-sm
                px-3 py-1.5 text-xs text-orbflow-text-muted
                focus:outline-none focus:ring-2 focus:ring-electric-indigo/40
                transition-all duration-200 cursor-pointer"
            >
              {SORT_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>

          {/* Search bar */}
          <div className="relative max-w-xl">
            <NodeIcon
              name="search"
              className="absolute left-3.5 top-1/2 -translate-y-1/2 w-4 h-4 text-orbflow-text-ghost pointer-events-none"
            />
            <input
              ref={searchInputRef}
              type="text"
              placeholder="Search plugins, integrations, tools..."
              value={searchQuery}
              onChange={handleSearchChange}
              className="w-full rounded-xl border border-orbflow-border/60 bg-orbflow-bg/60 backdrop-blur-sm
                pl-10 pr-12 py-2.5 text-sm text-orbflow-text-secondary placeholder:text-orbflow-text-ghost/60
                focus:outline-none focus:ring-2 focus:ring-electric-indigo/40 focus:border-electric-indigo/40
                transition-all duration-200"
            />
            <kbd className="absolute right-3 top-1/2 -translate-y-1/2 hidden sm:inline-flex items-center
              rounded-md border border-orbflow-border/40 bg-orbflow-surface/60 px-1.5 py-0.5
              text-[10px] font-mono text-orbflow-text-ghost/50">
              /
            </kbd>
          </div>

          {/* Category tabs */}
          <div className="flex items-center gap-1.5 overflow-x-auto pb-0.5 scrollbar-none -mx-1 px-1">
            {CATEGORIES.map((cat) => (
              <button
                key={cat.label}
                type="button"
                aria-pressed={activeCategory === cat.id}
                onClick={() => handleCategoryClick(cat.id)}
                className={cn(
                  "shrink-0 inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-all duration-200",
                  activeCategory === cat.id
                    ? "bg-electric-indigo text-white shadow-md shadow-electric-indigo/25"
                    : "text-orbflow-text-muted hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary",
                )}
              >
                <NodeIcon name={cat.icon} className="w-3 h-3" />
                {cat.label}
              </button>
            ))}
          </div>
        </div>

        {/* Separator with gradient */}
        <div className="h-px bg-gradient-to-r from-transparent via-orbflow-border to-transparent" />
      </div>

      {/* Plugin grid */}
      <div id="marketplace-tabpanel" role="tabpanel" aria-labelledby={`marketplace-tab-${activeTab}`} className="flex-1 overflow-y-auto">
        {/* Screen reader announcements for state transitions */}
        <div aria-live="polite" aria-atomic="true" className="sr-only">
          {loading
            ? "Loading plugins..."
            : error
              ? `Error: ${error}`
              : isEmpty
                ? "No plugins found."
                : `Showing ${showFrom} to ${showTo} of ${total} plugins.`}
        </div>
        <div className="p-6">
          {loading && <SkeletonGrid />}

          {error && (
            <div className="flex flex-col items-center justify-center py-20 text-center">
              <div className="w-16 h-16 rounded-2xl bg-rose-500/10 flex items-center justify-center mb-4
                ring-1 ring-rose-500/20">
                <NodeIcon name="alert-triangle" className="w-7 h-7 text-rose-400" />
              </div>
              <p className="text-sm font-medium text-orbflow-text-muted mb-1">Failed to load plugins</p>
              <p className="text-xs text-orbflow-text-ghost mb-5 max-w-xs">{error}</p>
              <button
                type="button"
                onClick={refetch}
                className="rounded-lg bg-electric-indigo text-white px-5 py-2 text-sm font-medium
                  shadow-md shadow-electric-indigo/20 hover:shadow-lg hover:shadow-electric-indigo/30
                  hover:brightness-110 transition-all duration-200"
              >
                Try Again
              </button>
            </div>
          )}

          {isEmpty && <EmptyState query={searchQuery} category={activeCategory} tab={activeTab} />}

          {!loading && !error && plugins.length > 0 && (
            <>
              {/* Result count */}
              <div className="flex items-center justify-between mb-4">
                <p className="text-xs text-orbflow-text-ghost">
                  Showing {showFrom}&ndash;{showTo} of {total} plugin{total !== 1 ? "s" : ""}
                </p>
              </div>

              {/* Featured section: top plugins on first page of Browse/All with no search */}
              {activeTab === "browse" && activeCategory === null && !searchQuery.trim() && page === 0 && plugins.length >= 4 && (
                <div className="mb-6">
                  <h3 className="text-xs font-semibold text-orbflow-text-ghost uppercase tracking-widest mb-3">
                    Featured
                  </h3>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 animate-[fadeIn_300ms_ease-out]">
                    {plugins.slice(0, 4).map((plugin) => (
                      <button
                        key={`featured-${plugin.name}`}
                        type="button"
                        onClick={() => handlePluginClick(plugin)}
                        className="text-left rounded-xl border border-orbflow-border/50 bg-orbflow-surface
                          marketplace-feature-card
                          hover:border-orbflow-border hover:shadow-lg transition-all duration-250
                          p-5 flex items-start gap-4 group relative overflow-hidden
                          focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                      >
                        <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300
                          pointer-events-none bg-gradient-to-br from-electric-indigo/[0.06] to-transparent" />
                        <div className="w-12 h-12 rounded-xl flex items-center justify-center shrink-0
                          bg-gradient-to-br from-electric-indigo/15 to-electric-indigo/5 shadow-md
                          transition-transform duration-200 group-hover:scale-105">
                          <NodeIcon name={plugin.icon ?? "package"} className="w-6 h-6 text-electric-indigo" />
                        </div>
                        <div className="relative min-w-0 flex-1">
                          <h4 className="text-sm font-semibold text-orbflow-text-secondary truncate
                            group-hover:text-electric-indigo transition-colors duration-200">
                            {plugin.name}
                          </h4>
                          <p className="text-xs text-orbflow-text-muted mt-1 line-clamp-2 leading-relaxed">
                            {plugin.description ?? "No description"}
                          </p>
                          <div className="flex items-center gap-2.5 mt-2 text-[11px] text-orbflow-text-ghost">
                            <span className="font-mono bg-orbflow-bg/60 rounded-md px-1.5 py-0.5 ring-1 ring-orbflow-border/30">
                              v{plugin.latest_version}
                            </span>
                            <span>by {plugin.author ?? "Unknown"}</span>
                            {plugin.installed && (
                              <span className="inline-flex items-center gap-1 text-emerald-400">
                                <NodeIcon name="check-circle" className="w-3 h-3" />
                                Installed
                              </span>
                            )}
                          </div>
                        </div>
                      </button>
                    ))}
                  </div>
                  <div className="h-px bg-gradient-to-r from-transparent via-orbflow-border/50 to-transparent mt-6" />
                  <h3 className="text-xs font-semibold text-orbflow-text-ghost uppercase tracking-widest mt-5 mb-3">
                    All Plugins
                  </h3>
                </div>
              )}

              <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4 gap-4 items-stretch
                animate-[fadeIn_300ms_ease-out]">
                {plugins.map((plugin, i) => (
                  <div
                    key={plugin.name}
                    style={{ animationDelay: `${Math.min(i * 50, 300)}ms` }}
                    className="h-full animate-[fadeInUp_400ms_ease-out_both]"
                  >
                    <PluginCard plugin={plugin} onClick={handlePluginClick} />
                  </div>
                ))}
              </div>

              {/* Pagination */}
              {totalPages > 1 && (
                <div className="flex items-center justify-center gap-3 mt-6 pt-4 border-t border-orbflow-border/30">
                  <button
                    type="button"
                    onClick={() => handlePageChange(page - 1)}
                    disabled={page === 0}
                    className="inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium
                      text-orbflow-text-muted hover:bg-orbflow-surface-hover transition-all duration-200
                      disabled:opacity-30 disabled:cursor-not-allowed"
                  >
                    <NodeIcon name="chevron-left" className="w-3.5 h-3.5" />
                    Previous
                  </button>
                  <span className="text-xs text-orbflow-text-ghost font-mono">
                    {page + 1} / {totalPages}
                  </span>
                  <button
                    type="button"
                    onClick={() => handlePageChange(page + 1)}
                    disabled={page >= totalPages - 1}
                    className="inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium
                      text-orbflow-text-muted hover:bg-orbflow-surface-hover transition-all duration-200
                      disabled:opacity-30 disabled:cursor-not-allowed"
                  >
                    Next
                    <NodeIcon name="chevron-right" className="w-3.5 h-3.5" />
                  </button>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* Detail panel */}
      {selectedPlugin && (
        <PluginDetail
          pluginName={selectedPlugin}
          onClose={handleCloseDetail}
          onPluginChanged={handlePluginChanged}
        />
      )}

      {/* Submit wizard */}
      {showSubmit && <SubmitPlugin onClose={() => setShowSubmit(false)} />}
    </div>
  );
}

function EmptyState({
  query,
  category,
  tab,
}: {
  readonly query: string;
  readonly category: CategoryId;
  readonly tab: TabId;
}) {
  const hasFilters = query.trim().length > 0 || category !== null;

  if (tab === "installed") {
    return (
      <div className="flex flex-col items-center justify-center py-24 text-center">
        <div className="relative mb-6">
          <div className="w-20 h-20 rounded-2xl bg-orbflow-surface flex items-center justify-center
            ring-1 ring-orbflow-border/50 shadow-xl shadow-black/20">
            <NodeIcon name="package" className="w-9 h-9 text-orbflow-text-ghost/50" />
          </div>
          <div className="absolute -inset-3 rounded-3xl bg-electric-indigo/[0.04] blur-xl -z-10" />
        </div>
        <p className="text-sm font-semibold text-orbflow-text-muted">No plugins installed</p>
        <p className="text-xs text-orbflow-text-ghost mt-1.5 max-w-xs leading-relaxed">
          Browse the marketplace to discover and install plugins for your workflows.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center justify-center py-24 text-center">
      {/* Decorative orb */}
      <div className="relative mb-6">
        <div className="w-20 h-20 rounded-2xl bg-orbflow-surface flex items-center justify-center
          ring-1 ring-orbflow-border/50 shadow-xl shadow-black/20">
          <NodeIcon
            name={hasFilters ? "search" : "plug"}
            className="w-9 h-9 text-orbflow-text-ghost/50"
          />
        </div>
        <div className="absolute -inset-3 rounded-3xl bg-electric-indigo/[0.04] blur-xl -z-10" />
      </div>
      {hasFilters ? (
        <>
          <p className="text-sm font-semibold text-orbflow-text-muted">No plugins match your search</p>
          <p className="text-xs text-orbflow-text-ghost mt-1.5 max-w-xs leading-relaxed">
            Try different keywords or clear the category filter to see all available plugins.
          </p>
        </>
      ) : (
        <>
          <p className="text-sm font-semibold text-orbflow-text-muted">Community Plugins</p>
          <p className="text-xs text-orbflow-text-ghost mt-1.5 max-w-md leading-relaxed">
            Plugins are discovered from the community index on GitHub.
            No plugins have been published yet &mdash; be the first!
          </p>
          <div className="mt-5 flex flex-col items-center gap-3">
            <a
              href="https://github.com/orbflow-dev/orbflow-plugins"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 rounded-lg bg-electric-indigo text-white
                px-5 py-2 text-sm font-medium shadow-md shadow-electric-indigo/20
                hover:shadow-lg hover:brightness-110 transition-all duration-200"
            >
              <NodeIcon name="link" className="w-4 h-4" />
              Submit Your Plugin
            </a>
            <p className="text-[11px] text-orbflow-text-ghost/50 max-w-xs text-center leading-relaxed">
              Create a plugin, publish it on GitHub, then submit a PR
              to the orbflow-plugins repo to list it here.
            </p>
          </div>
        </>
      )}
    </div>
  );
}

function SkeletonGrid() {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4 gap-4">
      {Array.from({ length: 8 }).map((_, i) => (
        <div
          key={i}
          className="rounded-xl border border-orbflow-border/50 bg-orbflow-surface p-5 space-y-4"
          style={{ animationDelay: `${i * 80}ms` }}
        >
          <div className="flex items-start gap-3 animate-pulse">
            <div className="w-11 h-11 rounded-xl bg-orbflow-surface-hover shrink-0" />
            <div className="flex-1 space-y-2 pt-0.5">
              <div className="h-4 w-24 rounded-md bg-orbflow-surface-hover" />
              <div className="h-3 w-full rounded-md bg-orbflow-surface-hover/70" />
              <div className="h-3 w-3/4 rounded-md bg-orbflow-surface-hover/50" />
            </div>
          </div>
          <div className="flex gap-2 animate-pulse">
            <div className="h-5 w-14 rounded-md bg-orbflow-surface-hover/60" />
            <div className="h-5 w-16 rounded-md bg-orbflow-surface-hover/40" />
          </div>
        </div>
      ))}
    </div>
  );
}
