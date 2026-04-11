"use client";

import { useState, useMemo, useCallback, useRef, useEffect } from "react";
import { NodeIcon } from "@/core/components/icons";

/* ------------------------------------------------------------------ */
/*  Types                                                              */
/* ------------------------------------------------------------------ */

export interface TemplateNode {
  id: string;
  name: string;
  type: string;
  plugin_ref: string;
  kind?: "trigger" | "action" | "capability";
  position: { x: number; y: number };
  input_mapping?: Record<string, string>;
}

export interface Template {
  name: string;
  description: string;
  icon: string;
  color: string;
  useCase: string;
  category: TemplateCategory;
  difficulty: "beginner" | "intermediate" | "advanced";
  nodes: TemplateNode[];
  edges: Array<{ id: string; source: string; target: string }>;
}

type TemplateCategory =
  | "all"
  | "integration"
  | "automation"
  | "data"
  | "security"
  | "ai"
  | "notification";

/* ------------------------------------------------------------------ */
/*  Category metadata                                                  */
/* ------------------------------------------------------------------ */

const CATEGORIES: {
  id: TemplateCategory;
  label: string;
  icon: string;
}[] = [
  { id: "all", label: "All Templates", icon: "grid" },
  { id: "integration", label: "Integration", icon: "globe" },
  { id: "automation", label: "Automation", icon: "clock" },
  { id: "data", label: "Data", icon: "layers" },
  { id: "security", label: "Security", icon: "shield" },
  { id: "ai", label: "AI / ML", icon: "cpu" },
  { id: "notification", label: "Notification", icon: "bell" },
];

const DIFFICULTY_CONFIG = {
  beginner: { label: "Beginner", dotColor: "bg-emerald-400" },
  intermediate: { label: "Intermediate", dotColor: "bg-amber-400" },
  advanced: { label: "Advanced", dotColor: "bg-rose-400" },
} as const;

/* ------------------------------------------------------------------ */
/*  Template definitions                                               */
/* ------------------------------------------------------------------ */

const templates: Template[] = [
  {
    name: "API Integration",
    description:
      "Fetch product data from a public API, transform the response, and log the result. Ready to run instantly.",
    icon: "globe",
    color: "#3B82F6",
    useCase: "Connect APIs",
    category: "integration",
    difficulty: "beginner",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 60 },
      },
      {
        id: "fetch",
        name: "Fetch Product",
        type: "builtin",
        plugin_ref: "builtin:http",
        position: { x: 250, y: 220 },
        input_mapping: {
          method: "GET",
          url: "https://dummyjson.com/products/1",
        },
      },
      {
        id: "log",
        name: "Log Response",
        type: "builtin",
        plugin_ref: "builtin:log",
        position: { x: 250, y: 380 },
        input_mapping: {
          message: '=nodes["fetch"].body',
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "fetch" },
      { id: "e2", source: "fetch", target: "log" },
    ],
  },
  {
    name: "Scheduled Reminder",
    description:
      "Wait 5 seconds, then call an API and log the response. Demonstrates timed delays between steps.",
    icon: "clock",
    color: "#F59E0B",
    useCase: "Timed actions",
    category: "automation",
    difficulty: "beginner",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 60 },
      },
      {
        id: "wait",
        name: "Wait 5s",
        type: "builtin",
        plugin_ref: "builtin:delay",
        position: { x: 250, y: 200 },
        input_mapping: { duration: "5s" },
      },
      {
        id: "notify",
        name: "Get Server Time",
        type: "builtin",
        plugin_ref: "builtin:http",
        position: { x: 250, y: 340 },
        input_mapping: {
          method: "GET",
          url: "https://httpbin.org/get",
        },
      },
      {
        id: "log",
        name: "Log Result",
        type: "builtin",
        plugin_ref: "builtin:log",
        position: { x: 250, y: 480 },
        input_mapping: {
          message: '=nodes["notify"].body',
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "wait" },
      { id: "e2", source: "wait", target: "notify" },
      { id: "e3", source: "notify", target: "log" },
    ],
  },
  {
    name: "Data Pipeline",
    description:
      "Fetch a list of users, transform the data to extract names, and log the output. A classic ETL pattern.",
    icon: "layers",
    color: "#10B981",
    useCase: "Data pipelines",
    category: "data",
    difficulty: "intermediate",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 40 },
      },
      {
        id: "input",
        name: "Fetch Users",
        type: "builtin",
        plugin_ref: "builtin:http",
        position: { x: 250, y: 180 },
        input_mapping: {
          method: "GET",
          url: "https://dummyjson.com/users?limit=5&select=firstName,lastName,email",
        },
      },
      {
        id: "process",
        name: "Extract Names",
        type: "builtin",
        plugin_ref: "builtin:transform",
        position: { x: 250, y: 320 },
        input_mapping: {
          expression: 'input.users.map(u, u.firstName + " " + u.lastName)',
          data: '=nodes["input"].body',
        },
      },
      {
        id: "output",
        name: "Log Names",
        type: "builtin",
        plugin_ref: "builtin:log",
        position: { x: 250, y: 460 },
        input_mapping: {
          message: '=nodes["process"].result',
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "input" },
      { id: "e2", source: "input", target: "process" },
      { id: "e3", source: "process", target: "output" },
    ],
  },
  {
    name: "Parallel Processing",
    description:
      "Fetch data from two APIs simultaneously, then collect and log both results. Demonstrates fan-out/fan-in.",
    icon: "git-branch",
    color: "#A855F7",
    useCase: "Parallel tasks",
    category: "data",
    difficulty: "advanced",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 40 },
      },
      {
        id: "branch_a",
        name: "Get UUID",
        type: "builtin",
        plugin_ref: "builtin:http",
        position: { x: 80, y: 200 },
        input_mapping: {
          method: "GET",
          url: "https://httpbin.org/uuid",
        },
      },
      {
        id: "branch_b",
        name: "Get IP",
        type: "builtin",
        plugin_ref: "builtin:http",
        position: { x: 420, y: 200 },
        input_mapping: {
          method: "GET",
          url: "https://httpbin.org/ip",
        },
      },
      {
        id: "collect",
        name: "Log Both",
        type: "builtin",
        plugin_ref: "builtin:log",
        position: { x: 250, y: 380 },
        input_mapping: {
          message: "Both parallel requests completed",
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "branch_a" },
      { id: "e2", source: "trigger", target: "branch_b" },
      { id: "e3", source: "branch_a", target: "collect" },
      { id: "e4", source: "branch_b", target: "collect" },
    ],
  },
  {
    name: "Encode & Hash",
    description:
      "Base64-encode a string, then SHA-256 hash it. Demonstrates chaining data transformations.",
    icon: "shield",
    color: "#14B8A6",
    useCase: "Data transforms",
    category: "security",
    difficulty: "beginner",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 40 },
      },
      {
        id: "encode",
        name: "Base64 Encode",
        type: "builtin",
        plugin_ref: "builtin:encode",
        position: { x: 250, y: 190 },
        input_mapping: {
          input: "Hello from Orbflow!",
          operation: "base64-encode",
        },
      },
      {
        id: "hash",
        name: "SHA-256 Hash",
        type: "builtin",
        plugin_ref: "builtin:encode",
        position: { x: 250, y: 340 },
        input_mapping: {
          input: '=nodes["encode"].result',
          operation: "sha256",
        },
      },
      {
        id: "log",
        name: "Log Result",
        type: "builtin",
        plugin_ref: "builtin:log",
        position: { x: 250, y: 490 },
        input_mapping: {
          message: '=nodes["hash"].result',
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "encode" },
      { id: "e2", source: "encode", target: "hash" },
      { id: "e3", source: "hash", target: "log" },
    ],
  },
  {
    name: "Webhook Handler",
    description:
      "Receive a webhook, delay briefly for processing, then make an HTTP callback with the result.",
    icon: "zap",
    color: "#EC4899",
    useCase: "Webhooks",
    category: "integration",
    difficulty: "intermediate",
    nodes: [
      {
        id: "trigger",
        name: "Webhook",
        type: "builtin",
        plugin_ref: "builtin:trigger-webhook",
        kind: "trigger",
        position: { x: 250, y: 60 },
      },
      {
        id: "wait",
        name: "Process Delay",
        type: "builtin",
        plugin_ref: "builtin:delay",
        position: { x: 250, y: 220 },
        input_mapping: { duration: "2s" },
      },
      {
        id: "callback",
        name: "Send Callback",
        type: "builtin",
        plugin_ref: "builtin:http",
        position: { x: 250, y: 380 },
        input_mapping: {
          method: "POST",
          url: "https://httpbin.org/post",
          body: '=nodes["trigger"].body',
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "wait" },
      { id: "e2", source: "wait", target: "callback" },
    ],
  },
  {
    name: "AI Text Summarizer",
    description:
      "Fetch an article via HTTP, summarize it with an AI node, and log the summary. Great for content processing.",
    icon: "cpu",
    color: "#6366F1",
    useCase: "AI Processing",
    category: "ai",
    difficulty: "intermediate",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 40 },
      },
      {
        id: "fetch",
        name: "Fetch Article",
        type: "builtin",
        plugin_ref: "builtin:http",
        position: { x: 250, y: 190 },
        input_mapping: {
          method: "GET",
          url: "https://dummyjson.com/posts/1",
        },
      },
      {
        id: "summarize",
        name: "Summarize",
        type: "builtin",
        plugin_ref: "builtin:ai-summarize",
        position: { x: 250, y: 340 },
        input_mapping: {
          text: '=nodes["fetch"].body',
        },
      },
      {
        id: "log",
        name: "Log Summary",
        type: "builtin",
        plugin_ref: "builtin:log",
        position: { x: 250, y: 490 },
        input_mapping: {
          message: '=nodes["summarize"].summary',
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "fetch" },
      { id: "e2", source: "fetch", target: "summarize" },
      { id: "e3", source: "summarize", target: "log" },
    ],
  },
  {
    name: "Sentiment Analyzer",
    description:
      "Analyze the sentiment of text input using AI and route based on positive or negative results.",
    icon: "cpu",
    color: "#8B5CF6",
    useCase: "AI Classification",
    category: "ai",
    difficulty: "intermediate",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 40 },
      },
      {
        id: "sentiment",
        name: "Analyze Sentiment",
        type: "builtin",
        plugin_ref: "builtin:ai-sentiment",
        position: { x: 250, y: 200 },
        input_mapping: {
          text: "Orbflow makes workflow automation incredibly easy and fun!",
        },
      },
      {
        id: "log",
        name: "Log Result",
        type: "builtin",
        plugin_ref: "builtin:log",
        position: { x: 250, y: 360 },
        input_mapping: {
          message: '=nodes["sentiment"].sentiment',
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "sentiment" },
      { id: "e2", source: "sentiment", target: "log" },
    ],
  },
  {
    name: "Email Notification",
    description:
      "Trigger an email notification when a condition is met. Uses delay to simulate processing before sending.",
    icon: "mail",
    color: "#F97316",
    useCase: "Alerts & Emails",
    category: "notification",
    difficulty: "beginner",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 40 },
      },
      {
        id: "delay",
        name: "Wait 2s",
        type: "builtin",
        plugin_ref: "builtin:delay",
        position: { x: 250, y: 200 },
        input_mapping: { duration: "2s" },
      },
      {
        id: "email",
        name: "Send Email",
        type: "builtin",
        plugin_ref: "builtin:email",
        position: { x: 250, y: 360 },
        input_mapping: {
          to: "team@example.com",
          subject: "Workflow Alert",
          body: "An automated workflow has completed processing.",
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "delay" },
      { id: "e2", source: "delay", target: "email" },
    ],
  },
  {
    name: "Data Sort & Filter",
    description:
      "Fetch a dataset, filter relevant records, sort them by criteria, and output the result.",
    icon: "filter",
    color: "#06B6D4",
    useCase: "Data processing",
    category: "data",
    difficulty: "intermediate",
    nodes: [
      {
        id: "trigger",
        name: "Manual Trigger",
        type: "builtin",
        plugin_ref: "builtin:trigger-manual",
        kind: "trigger",
        position: { x: 250, y: 40 },
      },
      {
        id: "fetch",
        name: "Fetch Data",
        type: "builtin",
        plugin_ref: "builtin:http",
        position: { x: 250, y: 190 },
        input_mapping: {
          method: "GET",
          url: "https://dummyjson.com/products?limit=10",
        },
      },
      {
        id: "filter",
        name: "Filter Records",
        type: "builtin",
        plugin_ref: "builtin:filter",
        position: { x: 250, y: 340 },
        input_mapping: {
          expression: "item.price > 20",
          data: '=nodes["fetch"].body',
        },
      },
      {
        id: "log",
        name: "Log Filtered",
        type: "builtin",
        plugin_ref: "builtin:log",
        position: { x: 250, y: 490 },
        input_mapping: {
          message: '=nodes["filter"].result',
        },
      },
    ],
    edges: [
      { id: "e1", source: "trigger", target: "fetch" },
      { id: "e2", source: "fetch", target: "filter" },
      { id: "e3", source: "filter", target: "log" },
    ],
  },
];

/* ------------------------------------------------------------------ */
/*  Mini Flow Preview -- inline DAG visualization                       */
/* ------------------------------------------------------------------ */

function MiniFlowPreview({
  nodes,
  edges,
}: {
  nodes: TemplateNode[];
  edges: Array<{ id: string; source: string; target: string }>;
}) {
  // Build a map of node id -> index for positioning
  const nodeMap = useMemo(() => {
    const m = new Map<string, { idx: number; node: TemplateNode }>();
    nodes.forEach((n, i) => m.set(n.id, { idx: i, node: n }));
    return m;
  }, [nodes]);

  // Calculate positions -- simple horizontal layout, handling branches
  const layout = useMemo(() => {
    // Find root (trigger / no incoming edges)
    const hasIncoming = new Set(edges.map((e) => e.target));
    const roots = nodes.filter((n) => !hasIncoming.has(n.id));
    if (roots.length === 0) return { positions: new Map<string, { x: number; y: number }>(), width: 0, height: 0 };

    const positions = new Map<string, { x: number; y: number }>();
    const visited = new Set<string>();
    const queue: { id: string; col: number; row: number }[] = roots.map((r, i) => ({
      id: r.id,
      col: 0,
      row: i,
    }));

    const outgoing = new Map<string, string[]>();
    for (const e of edges) {
      const arr = outgoing.get(e.source) ?? [];
      arr.push(e.target);
      outgoing.set(e.source, arr);
    }

    let maxCol = 0;
    let maxRow = 0;

    while (queue.length > 0) {
      const { id, col, row } = queue.shift()!;
      if (visited.has(id)) continue;
      visited.add(id);
      positions.set(id, { x: col, y: row });
      maxCol = Math.max(maxCol, col);
      maxRow = Math.max(maxRow, row);

      const children = outgoing.get(id) ?? [];
      children.forEach((childId, i) => {
        if (!visited.has(childId)) {
          queue.push({ id: childId, col: col + 1, row: children.length > 1 ? row + i : row });
        }
      });
    }

    return { positions, width: maxCol, height: maxRow };
  }, [nodes, edges]);

  const nodeRadius = 4;
  const spacingX = 28;
  const spacingY = 18;
  const paddingX = 12;
  const paddingY = 10;

  const svgWidth = (layout.width + 1) * spacingX + paddingX * 2;
  const svgHeight = (layout.height + 1) * spacingY + paddingY * 2;

  return (
    <svg
      width={svgWidth}
      height={svgHeight}
      viewBox={`0 0 ${svgWidth} ${svgHeight}`}
      className="shrink-0"
      aria-hidden="true"
    >
      {/* Edges -- matches canvas edge color */}
      {edges.map((e) => {
        const from = layout.positions.get(e.source);
        const to = layout.positions.get(e.target);
        if (!from || !to) return null;
        const x1 = from.x * spacingX + paddingX;
        const y1 = from.y * spacingY + paddingY;
        const x2 = to.x * spacingX + paddingX;
        const y2 = to.y * spacingY + paddingY;
        return (
          <line
            key={e.id}
            x1={x1}
            y1={y1}
            x2={x2}
            y2={y2}
            stroke="var(--orbflow-node-border, rgba(255,255,255,0.08))"
            strokeWidth={1.5}
          />
        );
      })}
      {/* Nodes -- uniform electric-indigo */}
      {nodes.map((n) => {
        const pos = layout.positions.get(n.id);
        if (!pos) return null;
        const cx = pos.x * spacingX + paddingX;
        const cy = pos.y * spacingY + paddingY;
        return (
          <circle
            key={n.id}
            cx={cx}
            cy={cy}
            r={nodeRadius}
            fill="#7C5CFC"
            fillOpacity={0.7}
            strokeWidth={0}
          />
        );
      })}
    </svg>
  );
}

/* ------------------------------------------------------------------ */
/*  Template Card                                                      */
/* ------------------------------------------------------------------ */

function TemplateCard({
  template: t,
  onUse,
  index,
}: {
  template: Template;
  onUse: (t: Template) => void;
  index: number;
}) {
  const diff = DIFFICULTY_CONFIG[t.difficulty];

  return (
    <div
      className="group relative rounded-2xl transition-all duration-200
        border border-orbflow-border bg-orbflow-surface
        hover:border-orbflow-text-ghost/30 hover:shadow-lg hover:shadow-black/5
        focus-within:ring-2 focus-within:ring-electric-indigo/40 focus-within:outline-none"
      style={{
        animationDelay: `${index * 50}ms`,
      }}
    >
      <div className="p-5 flex flex-col h-full">
        {/* Header row */}
        <div className="flex items-start gap-3 mb-3">
          <div
            className="w-10 h-10 rounded-xl flex items-center justify-center shrink-0
              transition-transform duration-200 group-hover:scale-105"
            style={{ backgroundColor: t.color + "10" }}
          >
            <NodeIcon
              name={t.icon}
              className="w-5 h-5"
              style={{ color: t.color, opacity: 0.7 }}
            />
          </div>
          <div className="min-w-0 flex-1">
            <h3 className="text-sm font-semibold leading-tight text-orbflow-text-secondary">
              {t.name}
            </h3>
            <div className="flex items-center gap-2 mt-1.5">
              <span
                className="inline-flex items-center text-[10px] font-medium px-1.5 py-0.5 rounded-md
                  bg-orbflow-add-btn-bg text-orbflow-text-muted"
              >
                {t.useCase}
              </span>
              <span className="inline-flex items-center gap-1 text-[10px] text-orbflow-text-ghost">
                <span className={`w-1.5 h-1.5 rounded-full ${diff.dotColor}`} />
                {diff.label}
              </span>
            </div>
          </div>
        </div>

        {/* Description */}
        <p className="text-xs leading-relaxed text-orbflow-text-faint mb-4 line-clamp-2 flex-1">
          {t.description}
        </p>

        {/* Mini flow diagram + node chain */}
        <div className="mb-4 flex items-center gap-3">
          <MiniFlowPreview nodes={t.nodes} edges={t.edges} />
          <div className="flex flex-col gap-0.5 min-w-0 flex-1">
            {t.nodes.slice(0, 4).map((n, i) => (
              <span
                key={n.id}
                className="text-[11px] font-mono text-orbflow-text-ghost truncate"
                title={n.name}
              >
                {i > 0 && <span className="text-orbflow-text-ghost/40 mr-1">{">"}</span>}
                {n.name}
              </span>
            ))}
            {t.nodes.length > 4 && (
              <span className="text-[11px] font-mono text-orbflow-text-ghost/50">
                +{t.nodes.length - 4} more
              </span>
            )}
          </div>
        </div>

        {/* Footer: stats + CTA */}
        <div className="flex items-center justify-between pt-3 border-t border-orbflow-border/50">
          <div className="flex items-center gap-3 text-[10px] text-orbflow-text-ghost">
            <span className="flex items-center gap-1">
              <NodeIcon name="circle" className="w-2.5 h-2.5" />
              {t.nodes.length} nodes
            </span>
            <span className="flex items-center gap-1">
              <NodeIcon name="chevron-right" className="w-2.5 h-2.5" />
              {t.edges.length} edges
            </span>
          </div>
          <button
            onClick={() => onUse(t)}
            className="text-xs font-medium px-3.5 py-2 min-h-[36px] rounded-lg
              transition-all duration-200
              text-orbflow-text-secondary bg-orbflow-add-btn-bg
              hover:bg-electric-indigo/15 hover:text-electric-indigo
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none
              active:scale-[0.97]"
            aria-label={`Use template: ${t.name}`}
          >
            Use template
          </button>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  TemplateGallery                                                    */
/* ------------------------------------------------------------------ */

interface TemplateGalleryProps {
  onUseTemplate: (template: Template) => void;
}

export function TemplateGallery({ onUseTemplate }: TemplateGalleryProps) {
  const [search, setSearch] = useState("");
  const [activeCategory, setActiveCategory] = useState<TemplateCategory>("all");
  const searchRef = useRef<HTMLInputElement>(null);

  // Keyboard shortcut: / to focus search
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "/" && document.activeElement?.tagName !== "INPUT") {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const filtered = useMemo(() => {
    const q = search.toLowerCase().trim();
    return templates.filter((t) => {
      const matchesCategory = activeCategory === "all" || t.category === activeCategory;
      if (!matchesCategory) return false;
      if (!q) return true;
      return (
        t.name.toLowerCase().includes(q) ||
        t.description.toLowerCase().includes(q) ||
        t.useCase.toLowerCase().includes(q) ||
        t.nodes.some((n) => n.name.toLowerCase().includes(q))
      );
    });
  }, [search, activeCategory]);

  const categoryCount = useCallback(
    (cat: TemplateCategory) =>
      cat === "all"
        ? templates.length
        : templates.filter((t) => t.category === cat).length,
    [],
  );

  const clearSearch = useCallback(() => {
    setSearch("");
    searchRef.current?.focus();
  }, []);

  return (
    <div className="h-full overflow-y-auto custom-scrollbar">
      <div className="p-6 lg:p-8 max-w-6xl mx-auto">
        {/* Header */}
        <header className="mb-8">
          <div className="flex items-center gap-3 mb-1">
            <div className="w-10 h-10 rounded-xl bg-electric-indigo/10 flex items-center justify-center">
              <NodeIcon name="layers" className="w-5 h-5 text-electric-indigo" />
            </div>
            <div>
              <h2 className="text-lg font-bold tracking-tight text-orbflow-text-secondary">
                Templates
              </h2>
              <p className="text-xs text-orbflow-text-faint">
                {templates.length} pre-built workflows to get you started
              </p>
            </div>
          </div>
        </header>

        {/* Search + Filters */}
        <div className="mb-6 space-y-4">
          {/* Search bar */}
          <div className="relative max-w-md">
            <NodeIcon
              name="search"
              className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-orbflow-text-ghost pointer-events-none"
            />
            <input
              ref={searchRef}
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search templates..."
              aria-label="Search templates"
              className="w-full pl-9 pr-8 py-2 text-sm rounded-xl
                border border-orbflow-border bg-orbflow-surface
                text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
                focus:outline-none focus:ring-2 focus:ring-electric-indigo/40 focus:border-electric-indigo/40
                transition-all duration-200"
            />
            {search ? (
              <button
                onClick={clearSearch}
                className="absolute right-1.5 top-1/2 -translate-y-1/2 text-orbflow-text-ghost hover:text-orbflow-text-muted
                  transition-colors p-2 rounded-md"
                aria-label="Clear search"
              >
                <NodeIcon name="x" className="w-3.5 h-3.5" />
              </button>
            ) : (
              <kbd className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[10px] font-mono text-orbflow-text-ghost
                border border-orbflow-border rounded px-1 py-0.5 pointer-events-none hidden sm:inline-block">
                /
              </kbd>
            )}
          </div>

          {/* Category pills */}
          <div className="flex items-center gap-1.5 flex-wrap" role="tablist" aria-label="Template categories">
            {CATEGORIES.map((cat) => {
              const count = categoryCount(cat.id);
              if (count === 0 && cat.id !== "all") return null;
              const isActive = activeCategory === cat.id;
              return (
                <button
                  key={cat.id}
                  role="tab"
                  id={`cat-tab-${cat.id}`}
                  aria-selected={isActive}
                  aria-controls="template-grid-panel"
                  onClick={() => setActiveCategory(cat.id)}
                  className={`inline-flex items-center gap-1.5 px-3 py-2.5 rounded-lg text-xs font-medium
                    transition-all duration-200
                    focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none
                    ${
                      isActive
                        ? "bg-electric-indigo/10 text-electric-indigo border border-electric-indigo/20"
                        : "text-orbflow-text-muted hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary border border-transparent"
                    }`}
                >
                  <NodeIcon name={cat.icon} className="w-3 h-3" />
                  {cat.label}
                  <span
                    className={`text-[10px] font-mono ml-0.5 ${
                      isActive ? "text-electric-indigo/60" : "text-orbflow-text-ghost"
                    }`}
                  >
                    {count}
                  </span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Results count */}
        {(search || activeCategory !== "all") && (
          <div className="mb-4 text-xs text-orbflow-text-ghost">
            {filtered.length === 0
              ? "No templates found"
              : `Showing ${filtered.length} template${filtered.length !== 1 ? "s" : ""}`}
            {search && (
              <span>
                {" "}for &ldquo;<span className="text-orbflow-text-muted">{search}</span>&rdquo;
              </span>
            )}
          </div>
        )}

        {/* Grid */}
        {filtered.length > 0 ? (
          <div
            id="template-grid-panel"
            role="tabpanel"
            aria-labelledby={`cat-tab-${activeCategory}`}
            className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-4"
          >
            {filtered.map((t, index) => (
              <TemplateCard
                key={t.name}
                template={t}
                onUse={onUseTemplate}
                index={index}
              />
            ))}
          </div>
        ) : (
          /* Empty state */
          <div
            id="template-grid-panel"
            role="tabpanel"
            aria-labelledby={`cat-tab-${activeCategory}`}
            className="flex flex-col items-center justify-center py-20"
          >
            <div className="w-16 h-16 rounded-2xl bg-orbflow-surface-hover flex items-center justify-center mb-4">
              <NodeIcon name="search" className="w-7 h-7 text-orbflow-text-ghost" />
            </div>
            <h3 className="text-sm font-medium text-orbflow-text-muted mb-1">
              No templates found
            </h3>
            <p className="text-xs text-orbflow-text-ghost mb-4 text-center max-w-xs">
              Try adjusting your search or selecting a different category.
            </p>
            <button
              onClick={() => {
                setSearch("");
                setActiveCategory("all");
              }}
              className="text-xs font-medium text-electric-indigo hover:text-electric-indigo/80
                transition-colors focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none
                px-3 py-1.5 rounded-lg hover:bg-electric-indigo/10"
            >
              Clear all filters
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
