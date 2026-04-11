import type { Node, Edge } from "@xyflow/react";

/** Category accent colors for quick-start templates. */
const TEMPLATE_COLORS = {
  integration: "#3B82F6",
  pipeline: "#F59E0B",
  monitoring: "#10B981",
  notification: "#A855F7",
} as const;

/** Pre-filled input mapping for a single field. */
interface StaticMapping {
  targetKey: string;
  mode: "static";
  staticValue: unknown;
}

export interface QuickTemplate {
  id: string;
  name: string;
  description: string;
  icon: string;
  color: string;
  nodes: Node[];
  edges: Edge[];
  /** Pre-filled input values keyed by nodeId -> fieldKey -> mapping. */
  inputMappings?: Record<string, Record<string, StaticMapping>>;
}

// ---------------------------------------------------------------------------
// Helper to create a static mapping
// ---------------------------------------------------------------------------
function staticField(key: string, value: unknown): StaticMapping {
  return { targetKey: key, mode: "static", staticValue: value };
}

// ---------------------------------------------------------------------------
// 1. API Integration
//    Manual Trigger -> HTTP Request (GET public API) -> Log the response
// ---------------------------------------------------------------------------
export const QUICK_TEMPLATES: QuickTemplate[] = [
  {
    id: "api-chain",
    name: "API Integration",
    description: "Fetch a post from a public API and log the result",
    icon: "globe",
    color: TEMPLATE_COLORS.integration,
    nodes: [
      {
        id: "trigger_1",
        type: "task" as const,
        position: { x: 80, y: 150 },
        data: {
          label: "Manual Trigger",
          pluginRef: "builtin:trigger-manual",
          type: "builtin",
          nodeKind: "trigger",
        },
      },
      {
        id: "http_1",
        type: "task" as const,
        position: { x: 300, y: 150 },
        data: {
          label: "Fetch Post",
          pluginRef: "builtin:http",
          type: "builtin",
        },
      },
      {
        id: "log_1",
        type: "task" as const,
        position: { x: 520, y: 150 },
        data: {
          label: "Log Result",
          pluginRef: "builtin:log",
          type: "builtin",
        },
      },
    ],
    edges: [
      {
        id: "edge_t1_h1",
        source: "trigger_1",
        target: "http_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
      {
        id: "edge_h1_l1",
        source: "http_1",
        target: "log_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
    ],
    inputMappings: {
      http_1: {
        method: staticField("method", "GET"),
        url: staticField("url", "https://jsonplaceholder.typicode.com/posts/1"),
      },
      log_1: {
        message: staticField("message", '=nodes["http_1"].body'),
      },
    },
  },

  // ---------------------------------------------------------------------------
  // 2. Scheduled Task
  //    Manual Trigger -> Wait 3s -> HTTP GET -> Log
  // ---------------------------------------------------------------------------
  {
    id: "scheduled",
    name: "Scheduled Task",
    description: "Wait 3 seconds, call an API, then log the result",
    icon: "clock",
    color: TEMPLATE_COLORS.pipeline,
    nodes: [
      {
        id: "trigger_1",
        type: "task" as const,
        position: { x: 60, y: 150 },
        data: {
          label: "Manual Trigger",
          pluginRef: "builtin:trigger-manual",
          type: "builtin",
          nodeKind: "trigger",
        },
      },
      {
        id: "delay_1",
        type: "task" as const,
        position: { x: 260, y: 150 },
        data: { label: "Wait 3s", pluginRef: "builtin:delay", type: "builtin" },
      },
      {
        id: "http_1",
        type: "task" as const,
        position: { x: 460, y: 150 },
        data: {
          label: "Fetch Data",
          pluginRef: "builtin:http",
          type: "builtin",
        },
      },
      {
        id: "log_1",
        type: "task" as const,
        position: { x: 660, y: 150 },
        data: {
          label: "Log Output",
          pluginRef: "builtin:log",
          type: "builtin",
        },
      },
    ],
    edges: [
      {
        id: "edge_t1_d1",
        source: "trigger_1",
        target: "delay_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
      {
        id: "edge_d1_h1",
        source: "delay_1",
        target: "http_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
      {
        id: "edge_h1_l1",
        source: "http_1",
        target: "log_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
    ],
    inputMappings: {
      delay_1: {
        duration: staticField("duration", "3s"),
      },
      http_1: {
        method: staticField("method", "GET"),
        url: staticField("url", "https://jsonplaceholder.typicode.com/todos/1"),
      },
      log_1: {
        message: staticField("message", '=nodes["http_1"].body'),
      },
    },
  },

  // ---------------------------------------------------------------------------
  // 3. Data Pipeline
  //    Manual Trigger -> Fetch users -> Transform (extract names) -> Log result
  // ---------------------------------------------------------------------------
  {
    id: "pipeline",
    name: "Data Pipeline",
    description: "Fetch users, transform the data, and log the output",
    icon: "layers",
    color: TEMPLATE_COLORS.monitoring,
    nodes: [
      {
        id: "trigger_1",
        type: "task" as const,
        position: { x: 60, y: 150 },
        data: {
          label: "Manual Trigger",
          pluginRef: "builtin:trigger-manual",
          type: "builtin",
          nodeKind: "trigger",
        },
      },
      {
        id: "http_1",
        type: "task" as const,
        position: { x: 260, y: 150 },
        data: {
          label: "Fetch Users",
          pluginRef: "builtin:http",
          type: "builtin",
        },
      },
      {
        id: "transform_1",
        type: "task" as const,
        position: { x: 460, y: 150 },
        data: {
          label: "Extract Names",
          pluginRef: "builtin:transform",
          type: "builtin",
        },
      },
      {
        id: "log_1",
        type: "task" as const,
        position: { x: 660, y: 150 },
        data: {
          label: "Log Names",
          pluginRef: "builtin:log",
          type: "builtin",
        },
      },
    ],
    edges: [
      {
        id: "edge_t1_h1",
        source: "trigger_1",
        target: "http_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
      {
        id: "edge_h1_tr1",
        source: "http_1",
        target: "transform_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
      {
        id: "edge_tr1_l1",
        source: "transform_1",
        target: "log_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
    ],
    inputMappings: {
      http_1: {
        method: staticField("method", "GET"),
        url: staticField("url", "https://jsonplaceholder.typicode.com/users"),
      },
      transform_1: {
        expression: staticField("expression", 'input.map(u, u.name)'),
        data: staticField("data", '=nodes["http_1"].body'),
      },
      log_1: {
        message: staticField("message", '=nodes["transform_1"].result'),
      },
    },
  },

  // ---------------------------------------------------------------------------
  // 4. Parallel Processing
  //    Manual Trigger -> (Post 1 | Post 2) -> Collect results
  // ---------------------------------------------------------------------------
  {
    id: "parallel",
    name: "Parallel Processing",
    description: "Fetch two API endpoints at the same time, then collect results",
    icon: "git-branch",
    color: TEMPLATE_COLORS.notification,
    nodes: [
      {
        id: "trigger_1",
        type: "task" as const,
        position: { x: 250, y: 30 },
        data: {
          label: "Manual Trigger",
          pluginRef: "builtin:trigger-manual",
          type: "builtin",
          nodeKind: "trigger",
        },
      },
      {
        id: "http_a",
        type: "task" as const,
        position: { x: 80, y: 190 },
        data: {
          label: "Fetch Post 1",
          pluginRef: "builtin:http",
          type: "builtin",
        },
      },
      {
        id: "http_b",
        type: "task" as const,
        position: { x: 420, y: 190 },
        data: {
          label: "Fetch Post 2",
          pluginRef: "builtin:http",
          type: "builtin",
        },
      },
      {
        id: "log_1",
        type: "task" as const,
        position: { x: 250, y: 360 },
        data: {
          label: "Log Both",
          pluginRef: "builtin:log",
          type: "builtin",
        },
      },
    ],
    edges: [
      {
        id: "edge_t1_ha",
        source: "trigger_1",
        target: "http_a",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
      {
        id: "edge_t1_hb",
        source: "trigger_1",
        target: "http_b",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
      {
        id: "edge_ha_l1",
        source: "http_a",
        target: "log_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
      {
        id: "edge_hb_l1",
        source: "http_b",
        target: "log_1",
        sourceHandle: "out",
        targetHandle: "in",
        type: "conditional" as const,
        data: {},
      },
    ],
    inputMappings: {
      http_a: {
        method: staticField("method", "GET"),
        url: staticField("url", "https://jsonplaceholder.typicode.com/posts/1"),
      },
      http_b: {
        method: staticField("method", "GET"),
        url: staticField("url", "https://jsonplaceholder.typicode.com/posts/2"),
      },
      log_1: {
        message: staticField("message", "Both requests completed"),
      },
    },
  },
];
