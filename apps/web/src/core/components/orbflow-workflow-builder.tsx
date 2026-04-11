"use client";

import {
  useCallback,
  useId,
  useMemo,
  useEffect,
  useRef,
  useState,
  type DragEvent,
} from "react";
import {
  ReactFlow,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  useReactFlow,
  ReactFlowProvider,
  type Connection,
  type Node,
  type Edge,
  type NodeMouseHandler,
  type EdgeMouseHandler,
  type IsValidConnection,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { useOrbflow } from "../context/orbflow-provider";
import { useTheme } from "../context/theme-provider";
import { useCanvasStore } from "@/store/canvas-store";
import { usePanelStore } from "@/store/panel-store";
import { useHistoryStore } from "@/store/history-store";
import { usePickerStore } from "@/store/picker-store";
import { useToastStore } from "@/store/toast-store";
import { useExecutionOverlayStore } from "@/store/execution-overlay-store";
import { useNodeOutputCacheStore } from "@/store/node-output-cache-store";
import { NodePickerPopover } from "./node-picker-popover";
import { NodeConfigModal } from "./node-config-modal";
import { WorkflowNode } from "./canvas/workflow-node";
import { StickyNoteNode } from "./canvas/sticky-note-node";
import { TextAnnotationNode } from "./canvas/text-annotation-node";
import { ConditionalEdge } from "./canvas/conditional-edge";
import { DataEdge } from "./canvas/rf-ui/data-edge";
import { LayoutControls, type LayoutDirection } from "./canvas/controls/layout-controls";
import type { LayoutAlgorithm } from "@orbflow/core/utils";
import { ViewportControls } from "./canvas/controls/viewport-controls";
import { InternalsSync } from "./canvas/controls/internals-sync";
import { NodeIcon } from "./icons";
import { BuilderToolbar } from "./builder-toolbar";
import { ContextMenu, type ContextMenuItem } from "./context-menu";
import { ShortcutHelp } from "./shortcut-help";
import { ConfirmDialog } from "./confirm-dialog";
import { useExecutionPolling } from "@/hooks/use-execution-polling";
import { ExecutionStatusBar } from "./execution-status-bar";
import { VersionHistory } from "@/components/workflow-builder/version-history";
import { layoutWorkflow } from "@orbflow/core/utils";
import { generateNodeSlug } from "../utils/node-slug";
import { useTriggerDetection } from "./canvas/use-trigger-detection";
import type { Workflow } from "@/lib/api";
import type { FieldMapping, ParameterValue } from "../types/schema";

const nodeTypes = { task: WorkflowNode, stickyNote: StickyNoteNode, textAnnotation: TextAnnotationNode };
const edgeTypes = { conditional: ConditionalEdge, data: DataEdge };

// Pre-filled input mapping for quick templates
type TM = { targetKey: string; mode: "static" | "expression"; staticValue?: unknown; celExpression?: string };
const sf = (k: string, v: unknown): TM => ({ targetKey: k, mode: "static", staticValue: v });
const ex = (k: string, c: string): TM => ({ targetKey: k, mode: "expression", celExpression: c });
const edge = (id: string, source: string, target: string) => ({
  id, source, target, sourceHandle: "out", targetHandle: "in", type: "conditional" as const, data: {},
});

interface QuickTemplate {
  id: string; name: string; description: string; icon: string; color: string;
  nodes: Node[]; edges: Edge[];
  inputMappings?: Record<string, Record<string, TM>>;
}

// Quick-start templates for empty canvas -- each has a trigger + pre-filled fields
const QUICK_TEMPLATES: QuickTemplate[] = [
  {
    id: "api-chain",
    name: "API Integration",
    description: "Fetch product data from an API and log it",
    icon: "globe",
    color: "#3B82F6",
    nodes: [
      { id: "trigger_1", type: "task" as const, position: { x: 80, y: 150 }, data: { label: "Manual Trigger", pluginRef: "builtin:trigger-manual", type: "builtin", nodeKind: "trigger" } },
      { id: "http_1", type: "task" as const, position: { x: 300, y: 150 }, data: { label: "Fetch Product", pluginRef: "builtin:http", type: "builtin" } },
      { id: "log_1", type: "task" as const, position: { x: 520, y: 150 }, data: { label: "Log Result", pluginRef: "builtin:log", type: "builtin" } },
    ],
    edges: [edge("e1", "trigger_1", "http_1"), edge("e2", "http_1", "log_1")],
    inputMappings: {
      http_1: { method: sf("method", "GET"), url: sf("url", "https://dummyjson.com/products/1") },
      log_1: { message: ex("message", 'nodes["http_1"].body') },
    },
  },
  {
    id: "scheduled",
    name: "Scheduled Task",
    description: "Wait 3 seconds, call an API, log the result",
    icon: "clock",
    color: "#F59E0B",
    nodes: [
      { id: "trigger_1", type: "task" as const, position: { x: 60, y: 150 }, data: { label: "Manual Trigger", pluginRef: "builtin:trigger-manual", type: "builtin", nodeKind: "trigger" } },
      { id: "delay_1", type: "task" as const, position: { x: 260, y: 150 }, data: { label: "Wait 3s", pluginRef: "builtin:delay", type: "builtin" } },
      { id: "http_1", type: "task" as const, position: { x: 460, y: 150 }, data: { label: "Get Server Info", pluginRef: "builtin:http", type: "builtin" } },
      { id: "log_1", type: "task" as const, position: { x: 660, y: 150 }, data: { label: "Log Output", pluginRef: "builtin:log", type: "builtin" } },
    ],
    edges: [edge("e1", "trigger_1", "delay_1"), edge("e2", "delay_1", "http_1"), edge("e3", "http_1", "log_1")],
    inputMappings: {
      delay_1: { duration: sf("duration", "3s") },
      http_1: { method: sf("method", "GET"), url: sf("url", "https://httpbin.org/get") },
      log_1: { message: ex("message", 'nodes["http_1"].body') },
    },
  },
  {
    id: "pipeline",
    name: "Data Pipeline",
    description: "Fetch users, extract names, and log them",
    icon: "layers",
    color: "#10B981",
    nodes: [
      { id: "trigger_1", type: "task" as const, position: { x: 60, y: 150 }, data: { label: "Manual Trigger", pluginRef: "builtin:trigger-manual", type: "builtin", nodeKind: "trigger" } },
      { id: "http_1", type: "task" as const, position: { x: 260, y: 150 }, data: { label: "Fetch Users", pluginRef: "builtin:http", type: "builtin" } },
      { id: "transform_1", type: "task" as const, position: { x: 460, y: 150 }, data: { label: "Extract Names", pluginRef: "builtin:transform", type: "builtin" } },
      { id: "log_1", type: "task" as const, position: { x: 660, y: 150 }, data: { label: "Log Names", pluginRef: "builtin:log", type: "builtin" } },
    ],
    edges: [edge("e1", "trigger_1", "http_1"), edge("e2", "http_1", "transform_1"), edge("e3", "transform_1", "log_1")],
    inputMappings: {
      http_1: { method: sf("method", "GET"), url: sf("url", "https://dummyjson.com/users?limit=5&select=firstName,lastName") },
      transform_1: { expression: sf("expression", 'input.users.map(u, u.firstName + " " + u.lastName)'), data: ex("data", 'nodes["http_1"].body') },
      log_1: { message: ex("message", 'nodes["transform_1"].result') },
    },
  },
  {
    id: "parallel",
    name: "Parallel Processing",
    description: "Fetch two APIs at the same time, collect results",
    icon: "git-branch",
    color: "#A855F7",
    nodes: [
      { id: "trigger_1", type: "task" as const, position: { x: 250, y: 30 }, data: { label: "Manual Trigger", pluginRef: "builtin:trigger-manual", type: "builtin", nodeKind: "trigger" } },
      { id: "http_a", type: "task" as const, position: { x: 80, y: 190 }, data: { label: "Get UUID", pluginRef: "builtin:http", type: "builtin" } },
      { id: "http_b", type: "task" as const, position: { x: 420, y: 190 }, data: { label: "Get IP", pluginRef: "builtin:http", type: "builtin" } },
      { id: "log_1", type: "task" as const, position: { x: 250, y: 360 }, data: { label: "Log Both", pluginRef: "builtin:log", type: "builtin" } },
    ],
    edges: [edge("e1", "trigger_1", "http_a"), edge("e2", "trigger_1", "http_b"), edge("e3", "http_a", "log_1"), edge("e4", "http_b", "log_1")],
    inputMappings: {
      http_a: { method: sf("method", "GET"), url: sf("url", "https://httpbin.org/uuid") },
      http_b: { method: sf("method", "GET"), url: sf("url", "https://httpbin.org/ip") },
      log_1: { message: sf("message", "Both parallel requests completed") },
    },
  },
];

interface Props {
  workflow?: Partial<Workflow>;
  defaultName?: string;
}

/** Wrapper that provides the ReactFlowProvider context. */
export function OrbflowWorkflowBuilderInner(props: Props) {
  return (
    <ReactFlowProvider>
      <BuilderInner {...props} />
    </ReactFlowProvider>
  );
}

/** Inline comment input dialog -- replaces window.prompt */
function CommentInputDialog({
  initialValue,
  onSubmit,
  onCancel,
}: {
  initialValue: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}) {
  const [value, setValueState] = useState(initialValue);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const titleId = useId();

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onCancel]);

  return (
    <div className="fixed inset-0 z-[90] flex items-center justify-center bg-black/50 backdrop-blur-sm animate-fade-in">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="w-full max-w-sm rounded-2xl backdrop-blur-xl shadow-2xl animate-scale-in overflow-hidden border border-orbflow-border bg-orbflow-glass-bg"
      >
        <div className="px-6 py-5">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-9 h-9 rounded-xl flex items-center justify-center shrink-0 bg-amber-500/10">
              <NodeIcon name="message-square" className="w-4 h-4 text-amber-400" />
            </div>
            <h2 id={titleId} className="text-sm font-semibold text-orbflow-text-secondary">
              {initialValue ? "Edit Note" : "Add Note"}
            </h2>
          </div>
          <textarea
            ref={inputRef}
            value={value}
            onChange={(e) => setValueState(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                onSubmit(value);
              }
            }}
            placeholder="Add a note to this step..."
            rows={3}
            className="w-full rounded-lg px-3 py-2
              text-[12px] resize-none
              focus:outline-none focus:border-electric-indigo/30 transition-all border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary"
          />
          <p className="text-[9px] mt-1.5 text-orbflow-text-faint">
            Ctrl+Enter to save
          </p>
        </div>
        <div className="flex items-center justify-end gap-2 px-6 py-3.5 border-t border-orbflow-border">
          <button
            onClick={onCancel}
            className="px-4 py-2 rounded-lg text-[12px] font-medium hover:bg-orbflow-surface-hover transition-all text-orbflow-text-muted"
          >
            Cancel
          </button>
          <button
            onClick={() => onSubmit(value)}
            className="px-4 py-2 rounded-lg text-[12px] font-medium bg-amber-500/15 border border-amber-500/20 text-amber-400 hover:bg-amber-500/25 transition-all active:scale-[0.97]"
          >
            Save Note
          </button>
        </div>
      </div>
    </div>
  );
}

function BuilderInner({ workflow, defaultName }: Props) {
  const { config, registry } = useOrbflow();
  const { mode: themeMode } = useTheme();
  const reactFlowInstance = useReactFlow();
  const {
    nodes,
    edges,
    capabilityEdges,
    annotations,
    setNodes,
    setEdges,
    setCapabilityEdges,
    setAnnotations,
    onNodesChange,
    onEdgesChange,
    addNode,
    addEdge: addCanvasEdge,
    addCapabilityEdge,
    removeNode,
    updateNodeData,
    addAnnotation,
    selectNode,
    selectEdge,
    clearSelection,
  } = useCanvasStore();

  const selectedNodeId = useCanvasStore((s) => {
    const it = s.selectedNodeIds?.values().next();
    return it?.done ? null : (it?.value ?? null);
  });
  const selectedEdgeId = useCanvasStore((s) => {
    const it = s.selectedEdgeIds?.values().next();
    return it?.done ? null : (it?.value ?? null);
  });

  const {
    closePanel,
    inputMappings,
    parameterValues,
    edgeConditions,
  } = usePanelStore();

  const picker = usePickerStore();

  const history = useHistoryStore();
  const toast = useToastStore();

  const isEmpty = nodes.length === 0;

  // -- Trigger detection -------------------------
  const { triggerType, triggerInfo } = useTriggerDetection(nodes, parameterValues);

  // -- Config modal state -----------------------
  const [configModalNodeId, setConfigModalNodeId] = useState<string | null>(null);

  // -- Workflow name state ---------------------
  const [workflowName, setWorkflowName] = useState(
    workflow?.name || defaultName || "Untitled Workflow"
  );
  useEffect(() => {
    setWorkflowName(workflow?.name || defaultName || "Untitled Workflow");
  }, [workflow?.name, defaultName]);

  // -- Workflow description state --------------
  const [workflowDescription, setWorkflowDescription] = useState(
    workflow?.description || ""
  );
  useEffect(() => {
    setWorkflowDescription(workflow?.description || "");
  }, [workflow?.description]);

  // -- Version history panel state ---------------
  const [showVersionHistory, setShowVersionHistory] = useState(false);

  // -- Context menu state ----------------------
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    nodeId?: string;
    edgeId?: string;
  } | null>(null);

  // -- Loading states --------------------------
  const [isSaving, setIsSaving] = useState(false);
  const [isRunning, setIsRunning] = useState(false);

  // -- Shortcut help, search & grid state ------
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [snapToGrid, setSnapToGrid] = useState(false);
  const [layoutDirection, setLayoutDirection] = useState<LayoutDirection>("LR");
  const [layoutAlgorithm, setLayoutAlgorithm] = useState<LayoutAlgorithm>("auto");
  const [canvasInteractive, setCanvasInteractive] = useState(true);
  const [confirmAction, setConfirmAction] = useState<{
    title: string;
    message: string;
    confirmLabel: string;
    variant: "danger" | "default";
    onConfirm: () => void;
  } | null>(null);

  // -- Comment input dialog state ------------
  const [commentDialog, setCommentDialog] = useState<{
    nodeId: string;
    initialValue: string;
  } | null>(null);

  // -- Reset execution overlay when workflow changes & cleanup on unmount ---
  const currentWorkflowId = workflow?.id;
  const prevWorkflowIdRef = useRef<string | undefined>(currentWorkflowId);
  useEffect(() => {
    // Only clear overlay when switching to a genuinely different workflow,
    // not when the same workflow re-mounts after a first save (key change).
    if (prevWorkflowIdRef.current !== undefined && prevWorkflowIdRef.current !== currentWorkflowId) {
      useExecutionOverlayStore.getState().reset();
    }
    prevWorkflowIdRef.current = currentWorkflowId;
    return () => {
      useExecutionOverlayStore.getState().stopLiveRun();
    };
  }, [currentWorkflowId]);

  // -- History helpers -------------------------
  const pushHistory = useCallback(() => {
    history.push({ nodes: [...nodes], edges: [...edges] });
  }, [history, nodes, edges]);

  const handleUndo = useCallback(() => {
    const snapshot = history.undo({ nodes, edges });
    if (snapshot) {
      setNodes(snapshot.nodes);
      setEdges(snapshot.edges);
    }
  }, [history, nodes, edges, setNodes, setEdges]);

  const handleRedo = useCallback(() => {
    const snapshot = history.redo({ nodes, edges });
    if (snapshot) {
      setNodes(snapshot.nodes);
      setEdges(snapshot.edges);
    }
  }, [history, nodes, edges, setNodes, setEdges]);

  // -- Load workflow into canvas ---------------
  useEffect(() => {
    if (!workflow) {
      setNodes([]);
      setEdges([]);
      setCapabilityEdges([]);
      setAnnotations([]);
      usePanelStore.getState().clearAll();
      history.clear();
      return;
    }

    const flowNodes: Node[] = (workflow.nodes || []).map((n) => ({
      id: n.id,
      type: "task",
      position: n.position,
      ...(n.parent_id ? {
        parentId: n.parent_id,
        extent: "parent" as const,
        zIndex: 1,
      } : {}),
      data: {
        label: n.name || n.id,
        pluginRef: n.plugin_ref,
        type: n.type,
        nodeKind: n.kind || undefined,
        requiresApproval: n.requires_approval || undefined,
      },
    }));

    // Create xyflow nodes for annotations
    const annotationNodes: Node[] = (workflow.annotations || []).map((a) => {
      if (a.type === "text") {
        return {
          id: `text_${a.id}`,
          type: "textAnnotation",
          position: a.position || { x: 0, y: 0 },
          data: { annotationId: a.id, content: a.content || "" },
        };
      }
      // Default: sticky_note
      const sw = (a.style?.width as number) ?? 200;
      const sh = (a.style?.height as number) ?? 140;
      return {
        id: `sticky_${a.id}`,
        type: "stickyNote",
        position: a.position || { x: 0, y: 0 },
        zIndex: -1,
        style: { width: sw, height: sh },
        data: { annotationId: a.id, content: a.content || "", color: a.style?.color || "yellow", width: sw, height: sh },
      };
    });

    const flowEdges: Edge[] = (workflow.edges || []).map((e) => ({
      id: e.id,
      source: e.source,
      target: e.target,
      type: "conditional",
      data: { conditionLabel: e.condition || "" },
    }));

    setNodes([...flowNodes, ...annotationNodes]);
    setEdges(flowEdges);
    setCapabilityEdges(
      (workflow.capability_edges || []).map((ce) => ({
        id: ce.id,
        sourceNodeId: ce.source_node_id,
        targetNodeId: ce.target_node_id,
        targetPortKey: ce.target_port_key,
      }))
    );
    setAnnotations(
      (workflow.annotations || []).map((a) => ({
        id: a.id,
        type: a.type as "sticky_note" | "text" | "markdown",
        content: a.content,
        position: a.position,
        style: a.style,
      }))
    );
    history.clear();

    // Smart viewport: fit nodes with per-side padding for toolbars
    // xyflow v12.5+ supports calling fitView directly after setNodes
    setTimeout(() => {
      reactFlowInstance.fitView({
        padding: { top: "80px", right: "40px", bottom: "80px", left: "40px" },
        maxZoom: 1.5,
        minZoom: 0.3,
        duration: 300,
      });
    }, 50);

    // Hydrate panel store from saved workflow data
    const panel = usePanelStore.getState();
    panel.clearAll();

    for (const n of workflow.nodes || []) {
      // Restore input mappings (CEL convention: "=" prefix -> expression, else static)
      if (n.input_mapping && Object.keys(n.input_mapping).length > 0) {
        const mappings: Record<string, FieldMapping> = {};
        for (const [key, rawValue] of Object.entries(n.input_mapping)) {
          const str = String(rawValue);
          if (str.startsWith("=")) {
            mappings[key] = { targetKey: key, mode: "expression", celExpression: str.slice(1) };
          } else {
            mappings[key] = { targetKey: key, mode: "static", staticValue: rawValue };
          }
        }
        panel.loadNodeMappings(n.id, mappings);
      }

      // Restore parameter values
      if (n.parameters && n.parameters.length > 0) {
        const params: Record<string, ParameterValue> = {};
        for (const p of n.parameters) {
          params[p.key] = { key: p.key, mode: p.mode, value: p.value, expression: p.expression };
        }
        panel.loadNodeParameters(n.id, params);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workflow, setNodes, setEdges]);

  const isValidConnection: IsValidConnection = useCallback(
    (connection) => {
      if (!connection.source || !connection.target) return false;
      if (connection.source === connection.target) return false;

      const targetHandle = connection.targetHandle || "";

      // -- Capability connection validation --
      if (targetHandle.startsWith("cap:")) {
        const portKey = targetHandle.slice(4);
        const sourceNode = nodes.find((n) => n.id === connection.source);
        const targetNode = nodes.find((n) => n.id === connection.target);
        if (!sourceNode || !targetNode) return false;

        const sourceSchema = registry.get(sourceNode.data?.pluginRef as string);
        const targetSchema = registry.get(targetNode.data?.pluginRef as string);
        if (!sourceSchema || !targetSchema) return false;

        // Source must be a capability node.
        if (sourceSchema.nodeKind !== "capability") return false;

        // Find the target port and validate type match.
        const port = targetSchema.capabilityPorts?.find((p) => p.key === portKey);
        if (!port) return false;

        return sourceSchema.providesCapability === port.capabilityType;
      }

      // -- Regular edge: DAG cycle detection --
      // A cycle exists if there's already a path from target -> source.
      const visited = new Set<string>();
      const stack = [connection.target];
      while (stack.length > 0) {
        const current = stack.pop()!;
        if (current === connection.source) return false; // cycle!
        if (visited.has(current)) continue;
        visited.add(current);
        for (const edge of edges) {
          if (edge.source === current) {
            stack.push(edge.target);
          }
        }
      }
      return true;
    },
    [edges, nodes, registry]
  );

  // -- Connections -----------------------------
  const onConnect = useCallback(
    (connection: Connection) => {
      pushHistory();
      const targetHandle = connection.targetHandle || "";

      // Capability edge -- store in separate capabilityEdges array.
      if (targetHandle.startsWith("cap:")) {
        const portKey = targetHandle.slice(4);
        addCapabilityEdge({
          id: `cap_edge_${Date.now()}`,
          sourceNodeId: connection.source!,
          targetNodeId: connection.target!,
          targetPortKey: portKey,
        });
        return;
      }

      // Regular data edge.
      const edge: Edge = {
        id: `edge_${Date.now()}`,
        source: connection.source!,
        target: connection.target!,
        sourceHandle: connection.sourceHandle || undefined,
        targetHandle: connection.targetHandle || undefined,
        type: "conditional",
        data: {},
      };
      addCanvasEdge(edge);
    },
    [pushHistory, addCanvasEdge, addCapabilityEdge]
  );

  // -- Click handlers --------------------------
  const onNodeClick: NodeMouseHandler = useCallback(
    (_event, node) => {
      selectNode(node.id);
      // Don't open config modal for annotation nodes (sticky notes, text labels)
      if (node.type === "stickyNote" || node.type === "textAnnotation") return;
      setConfigModalNodeId(node.id);
    },
    [selectNode]
  );

  const onEdgeClick: EdgeMouseHandler = useCallback(
    (_event, edge) => {
      selectEdge(edge.id);
    },
    [selectEdge]
  );

  const onPaneClick = useCallback(() => {
    clearSelection();
    closePanel();
    setContextMenu(null);
    picker.closePicker();
  }, [clearSelection, closePanel, picker]);

  // -- Drag & drop -----------------------------
  const onDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  }, []);

  const onDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      const pluginRef = e.dataTransfer.getData("application/orbflow-node");
      if (!pluginRef) return;
      const schema = registry.get(pluginRef);
      if (!schema) return;

      const position = reactFlowInstance.screenToFlowPosition({
        x: e.clientX,
        y: e.clientY,
      });

      pushHistory();
      const existingIds = useCanvasStore.getState().nodes.map((n) => n.id);
      const newNode: Node = {
        id: generateNodeSlug(schema.name, existingIds),
        type: "task",
        position,
        data: {
          label: schema.name,
          pluginRef: schema.pluginRef,
          type: schema.pluginRef.startsWith("plugin:") ? "plugin" : "builtin",
          nodeKind: schema.nodeKind || undefined,
        },
      };
      addNode(newNode);
      selectNode(newNode.id);
    },
    [registry, reactFlowInstance, pushHistory, addNode, selectNode]
  );

  // -- Stick node in note (parent/child) -------
  const hoveredStickyRef = useRef<string | null>(null);

  /** Helper: find sticky note under a node's center */
  const findStickyUnder = useCallback((nodeId: string) => {
    const { nodes: currentNodes } = useCanvasStore.getState();
    const freshNode = currentNodes.find((n) => n.id === nodeId);
    if (!freshNode) return null;

    const stickyNodes = currentNodes.filter((n) => n.type === "stickyNote");

    // Compute absolute position
    let absX = freshNode.position.x;
    let absY = freshNode.position.y;
    if (freshNode.parentId) {
      const parent = currentNodes.find((n) => n.id === freshNode.parentId);
      if (parent) {
        absX += parent.position.x;
        absY += parent.position.y;
      }
    }

    const nodeW = freshNode.measured?.width ?? 64;
    const nodeH = freshNode.measured?.height ?? 64;
    const nodeCx = absX + nodeW / 2;
    const nodeCy = absY + nodeH / 2;

    return stickyNodes.find((sticky) => {
      const sw = (sticky.data?.width as number) ?? 200;
      const sh = (sticky.data?.height as number) ?? 140;
      return (
        nodeCx >= sticky.position.x &&
        nodeCx <= sticky.position.x + sw &&
        nodeCy >= sticky.position.y &&
        nodeCy <= sticky.position.y + sh
      );
    }) || null;
  }, []);

  /** During drag: highlight the sticky note under cursor */
  const onNodeDrag = useCallback(
    (_event: React.MouseEvent, draggedNode: Node) => {
      if (draggedNode.type === "stickyNote" || draggedNode.type === "textAnnotation") return;

      const { updateNodeData } = useCanvasStore.getState();
      const hitSticky = findStickyUnder(draggedNode.id);
      const hitId = hitSticky?.id || null;

      // Only update if hovered sticky changed
      if (hitId !== hoveredStickyRef.current) {
        // Clear previous highlight
        if (hoveredStickyRef.current) {
          updateNodeData(hoveredStickyRef.current, { dropHighlight: false });
        }
        // Set new highlight
        if (hitId) {
          updateNodeData(hitId, { dropHighlight: true });
        }
        hoveredStickyRef.current = hitId;
      }
    },
    [findStickyUnder]
  );

  const onNodeDragStop = useCallback(
    (_event: React.MouseEvent, draggedNode: Node) => {
      // Clear any drag highlight
      if (hoveredStickyRef.current) {
        useCanvasStore.getState().updateNodeData(hoveredStickyRef.current, { dropHighlight: false });
        hoveredStickyRef.current = null;
      }

      if (draggedNode.type === "stickyNote" || draggedNode.type === "textAnnotation") return;

      const { updateNode, nodes: currentNodes } = useCanvasStore.getState();
      const freshNode = currentNodes.find((n) => n.id === draggedNode.id);
      if (!freshNode) return;

      const stickyNodes = currentNodes.filter((n) => n.type === "stickyNote");
      if (stickyNodes.length === 0 && !freshNode.parentId) return;

      // Get absolute position from fresh store state
      let absX = freshNode.position.x;
      let absY = freshNode.position.y;
      if (freshNode.parentId) {
        const parent = currentNodes.find((n) => n.id === freshNode.parentId);
        if (parent) {
          absX += parent.position.x;
          absY += parent.position.y;
        }
      }

      const nodeW = freshNode.measured?.width ?? 64;
      const nodeH = freshNode.measured?.height ?? 64;
      const nodeCx = absX + nodeW / 2;
      const nodeCy = absY + nodeH / 2;

      // Hit-test: is the node center inside any sticky note?
      const hitSticky = stickyNodes.find((sticky) => {
        const sw = (sticky.data?.width as number) ?? 200;
        const sh = (sticky.data?.height as number) ?? 140;
        return (
          nodeCx >= sticky.position.x &&
          nodeCx <= sticky.position.x + sw &&
          nodeCy >= sticky.position.y &&
          nodeCy <= sticky.position.y + sh
        );
      });

      if (hitSticky && hitSticky.id !== freshNode.parentId) {
        // Reparent: absolute -> relative position
        updateNode(draggedNode.id, {
          parentId: hitSticky.id,
          extent: undefined,
          position: { x: absX - hitSticky.position.x, y: absY - hitSticky.position.y },
          zIndex: 1,
        });
      } else if (!hitSticky && freshNode.parentId) {
        // Unparent: relative -> absolute position
        updateNode(draggedNode.id, {
          parentId: undefined,
          extent: undefined,
          position: { x: absX, y: absY },
          zIndex: 0,
        });
      }
    },
    [findStickyUnder]
  );

  // -- Picker node insertion -------------------
  const handlePickerSelect = useCallback(
    (pluginRef: string) => {
      const schema = registry.get(pluginRef);
      if (!schema) return;

      pushHistory();
      const existingIds = useCanvasStore.getState().nodes.map((n) => n.id);
      const newNode: Node = {
        id: generateNodeSlug(schema.name, existingIds),
        type: "task",
        position: { x: 0, y: 0 },
        data: {
          label: schema.name,
          pluginRef: schema.pluginRef,
          type: schema.pluginRef.startsWith("plugin:") ? "plugin" : "builtin",
          nodeKind: schema.nodeKind || undefined,
        },
      };

      const { sourceNodeId, sourceEdgeId, position: pickerPos } = picker;

      if (sourceNodeId) {
        // Clicked "+" on a node -- place to the right and auto-connect
        const sourceNode = nodes.find((n) => n.id === sourceNodeId);
        if (sourceNode) {
          newNode.position = {
            x: sourceNode.position.x + 150,
            y: sourceNode.position.y,
          };
        }
        addNode(newNode);
        const edge: Edge = {
          id: `edge_${Date.now()}`,
          source: sourceNodeId,
          target: newNode.id,
          sourceHandle: "out",
          targetHandle: "in",
          type: "conditional",
          data: {},
        };
        addCanvasEdge(edge);
      } else if (sourceEdgeId) {
        // Clicked "+" on an edge -- split the edge
        const oldEdge = edges.find((e) => e.id === sourceEdgeId);
        if (oldEdge) {
          const sourceNode = nodes.find((n) => n.id === oldEdge.source);
          const targetNode = nodes.find((n) => n.id === oldEdge.target);
          if (sourceNode && targetNode) {
            newNode.position = {
              x: (sourceNode.position.x + targetNode.position.x) / 2,
              y: (sourceNode.position.y + targetNode.position.y) / 2,
            };
          }
          addNode(newNode);
          // Remove old edge
          setEdges(edges.filter((e) => e.id !== sourceEdgeId));
          // Create two new edges
          addCanvasEdge({
            id: `edge_${Date.now()}_a`,
            source: oldEdge.source,
            target: newNode.id,
            sourceHandle: "out",
            targetHandle: "in",
            type: "conditional",
            data: {},
          });
          addCanvasEdge({
            id: `edge_${Date.now()}_b`,
            source: newNode.id,
            target: oldEdge.target,
            sourceHandle: "out",
            targetHandle: "in",
            type: "conditional",
            data: {},
          });
        } else {
          addNode(newNode);
        }
      } else {
        // No source -- place at picker screen position
        const flowPos = reactFlowInstance.screenToFlowPosition({
          x: pickerPos.x,
          y: pickerPos.y,
        });
        newNode.position = flowPos;
        addNode(newNode);
      }

      selectNode(newNode.id);
      // Only open config modal when the node is actually connected
      if (sourceNodeId || sourceEdgeId) {
        setConfigModalNodeId(newNode.id);
      }
      picker.closePicker();
    },
    [registry, pushHistory, picker, nodes, edges, addNode, addCanvasEdge, setEdges, selectNode, reactFlowInstance]
  );

  // -- Template loading ------------------------
  const loadTemplate = useCallback(
    (template: QuickTemplate) => {
      pushHistory();
      setNodes(template.nodes);
      setEdges(template.edges);
      // Apply pre-filled input mappings from template
      if (template.inputMappings) {
        const ps = usePanelStore.getState();
        for (const [nodeId, fields] of Object.entries(template.inputMappings)) {
          const existing = ps.getNodeMappings(nodeId);
          ps.loadNodeMappings(nodeId, { ...existing, ...fields });
        }
      }
    },
    [pushHistory, setNodes, setEdges]
  );

  // -- Add sticky note ------------------------
  const handleAddStickyNote = useCallback(
    (screenX?: number, screenY?: number) => {
      pushHistory();
      const id = `note_${Date.now()}`;
      const position = reactFlowInstance.screenToFlowPosition({
        x: screenX ?? window.innerWidth / 2,
        y: screenY ?? window.innerHeight / 2,
      });
      const annotation = {
        id,
        type: "sticky_note" as const,
        content: "",
        position,
        style: { color: "yellow", width: 200, height: 140 },
      };
      addAnnotation(annotation);
      addNode({
        id: `sticky_${id}`,
        type: "stickyNote",
        position,
        zIndex: -1,
        style: { width: 200, height: 140 },
        data: {
          annotationId: id,
          content: "",
          color: "yellow",
          width: 200,
          height: 140,
        },
      });
    },
    [pushHistory, reactFlowInstance, addAnnotation, addNode]
  );

  // -- Add text annotation ----------------------
  const handleAddTextAnnotation = useCallback(
    (screenX?: number, screenY?: number) => {
      pushHistory();
      const id = `note_${Date.now()}`;
      const position = reactFlowInstance.screenToFlowPosition({
        x: screenX ?? window.innerWidth / 2,
        y: screenY ?? window.innerHeight / 2,
      });
      const annotation = {
        id,
        type: "text" as const,
        content: "",
        position,
      };
      addAnnotation(annotation);
      addNode({
        id: `text_${id}`,
        type: "textAnnotation",
        position,
        data: {
          annotationId: id,
          content: "",
        },
      });
    },
    [pushHistory, reactFlowInstance, addAnnotation, addNode]
  );

  // -- Add annotation dispatcher ----------------
  const handleAddAnnotation = useCallback(
    (type: "sticky_note" | "text") => {
      if (type === "sticky_note") handleAddStickyNote();
      else if (type === "text") handleAddTextAnnotation();
    },
    [handleAddStickyNote, handleAddTextAnnotation]
  );

  // -- Delete helpers --------------------------
  const executeDelete = useCallback(() => {
    if (selectedNodeId) {
      pushHistory();
      removeNode(selectedNodeId);
      closePanel();
      toast.info("Step deleted");
    } else if (selectedEdgeId) {
      pushHistory();
      setEdges(edges.filter((e) => e.id !== selectedEdgeId));
      clearSelection();
      closePanel();
      toast.info("Connection removed");
    }
  }, [
    selectedNodeId,
    selectedEdgeId,
    pushHistory,
    removeNode,
    closePanel,
    setEdges,
    edges,
    clearSelection,
    toast,
  ]);

  const handleDelete = useCallback(() => {
    if (!selectedNodeId && !selectedEdgeId) return;
    const target = selectedNodeId
      ? nodes.find((n) => n.id === selectedNodeId)
      : null;
    const label = target
      ? `"${(target.data?.label as string) || target.id}"`
      : "this connection";
    setConfirmAction({
      title: selectedNodeId ? "Delete step?" : "Remove connection?",
      message: `Are you sure you want to delete ${label}? This action cannot be undone.`,
      confirmLabel: "Delete",
      variant: "danger",
      onConfirm: executeDelete,
    });
  }, [selectedNodeId, selectedEdgeId, nodes, executeDelete]);

  // -- Duplicate selected node -----------------
  const handleDuplicate = useCallback(() => {
    if (!selectedNodeId) return;
    const original = nodes.find((n) => n.id === selectedNodeId);
    if (!original) return;

    pushHistory();
    const existingIds = useCanvasStore.getState().nodes.map((n) => n.id);
    const slugName = (original.data?.label as string) || "node";
    const isAnnotation = original.type === "stickyNote" || original.type === "textAnnotation";
    const newNode: Node = {
      id: generateNodeSlug(slugName, existingIds),
      type: original.type,
      position: {
        x: original.position.x + 40,
        y: original.position.y + 40,
      },
      data: { ...original.data },
      ...(original.style ? { style: { ...original.style } } : {}),
      ...(original.width != null ? { width: original.width } : {}),
      ...(original.height != null ? { height: original.height } : {}),
    };
    addNode(newNode);
    selectNode(newNode.id);
    if (!isAnnotation) {
      setConfigModalNodeId(newNode.id);
    }
    toast.info(isAnnotation ? "Note duplicated" : "Step duplicated");
  }, [selectedNodeId, nodes, pushHistory, addNode, selectNode, toast]);

  // -- Auto-layout with animation --------------
  const handleAutoLayout = useCallback(() => {
    if (nodes.length === 0) return;
    pushHistory();

    // Collect measured dimensions from React Flow's internal node data
    const rfNodes = reactFlowInstance.getNodes();
    const nodeDimensions = new Map<string, { width: number; height: number }>();
    for (const rfNode of rfNodes) {
      if (rfNode.measured?.width && rfNode.measured?.height) {
        nodeDimensions.set(rfNode.id, {
          width: rfNode.measured.width,
          height: rfNode.measured.height,
        });
      }
    }

    const targetNodes = layoutWorkflow(nodes, edges, {
      direction: layoutDirection,
      algorithm: layoutAlgorithm,
      nodeDimensions,
    });

    // Animate from current positions to target positions
    type Pos = { x: number; y: number };
    const startPositions = new Map<string, Pos>(
      nodes.map((n): [string, Pos] => [n.id, { x: n.position.x, y: n.position.y }]),
    );
    const targetPositions = new Map<string, Pos>(
      targetNodes.map((n): [string, Pos] => [n.id, { x: n.position.x, y: n.position.y }]),
    );

    const duration = 400;
    const startTime = performance.now();

    const animate = (now: number) => {
      const elapsed = now - startTime;
      const progress = Math.min(elapsed / duration, 1);
      // Ease-out cubic: 1 - (1 - t)^3
      const eased = 1 - Math.pow(1 - progress, 3);

      const interpolated = nodes.map((node) => {
        const from = startPositions.get(node.id);
        const to = targetPositions.get(node.id);
        if (!from || !to) return node;
        return {
          ...node,
          position: {
            x: from.x + (to.x - from.x) * eased,
            y: from.y + (to.y - from.y) * eased,
          },
        };
      });

      setNodes(interpolated);

      if (progress < 1) {
        requestAnimationFrame(animate);
      } else {
        // Final: set exact target positions
        setNodes(targetNodes);
        setTimeout(() => reactFlowInstance.fitView({ padding: 0.2, duration: 300 }), 50);
      }
    };

    requestAnimationFrame(animate);
    toast.info("Layout applied");
  }, [nodes, edges, layoutDirection, layoutAlgorithm, pushHistory, setNodes, reactFlowInstance, toast]);

  const handleToggleDirection = useCallback(() => {
    setLayoutDirection((d) => (d === "LR" ? "TB" : "LR"));
  }, []);

  const handleToggleInteractive = useCallback(() => {
    setCanvasInteractive((v) => !v);
  }, []);

  // -- Zoom to fit -----------------------------
  const handleZoomFit = useCallback(() => {
    reactFlowInstance.fitView({ padding: 0.2, duration: 300 });
  }, [reactFlowInstance]);

  // -- Add/edit note on node --------------------
  const handleAddComment = useCallback(
    (nodeId: string) => {
      const node = nodes.find((n) => n.id === nodeId);
      const existing = (node?.data?.comment as string) || "";
      setCommentDialog({ nodeId, initialValue: existing });
    },
    [nodes]
  );

  const handleCommentSubmit = useCallback(
    (value: string) => {
      if (commentDialog) {
        updateNodeData(commentDialog.nodeId, { comment: value.trim() });
      }
      setCommentDialog(null);
    },
    [commentDialog, updateNodeData]
  );

  // -- Context menu ----------------------------
  const onNodeContextMenu = useCallback(
    (event: React.MouseEvent, node: Node) => {
      event.preventDefault();
      selectNode(node.id);
      setContextMenu({ x: event.clientX, y: event.clientY, nodeId: node.id });
    },
    [selectNode]
  );

  const onEdgeContextMenu = useCallback(
    (event: React.MouseEvent, edge: Edge) => {
      event.preventDefault();
      selectEdge(edge.id);
      setContextMenu({ x: event.clientX, y: event.clientY, edgeId: edge.id });
    },
    [selectEdge]
  );

  const onPaneContextMenu = useCallback(
    (event: React.MouseEvent | MouseEvent) => {
      event.preventDefault();
      setContextMenu({
        x: (event as React.MouseEvent).clientX,
        y: (event as React.MouseEvent).clientY,
      });
    },
    []
  );

  const contextMenuItems = useMemo((): ContextMenuItem[] => {
    if (contextMenu?.nodeId) {
      const nodeId = contextMenu.nodeId;
      const node = nodes.find((n) => n.id === nodeId);
      const isAnnotationNode = node?.type === "stickyNote" || node?.type === "textAnnotation";
      const hasComment = !!(node?.data?.comment as string);
      const items: ContextMenuItem[] = [];
      if (!isAnnotationNode) {
        items.push({
          label: "Configure",
          icon: "settings",
          onClick: () => {
            if (contextMenu.nodeId) setConfigModalNodeId(contextMenu.nodeId);
          },
        });
        items.push({
          label: hasComment ? "Edit Note" : "Add Note",
          icon: "message-square",
          onClick: () => handleAddComment(nodeId),
        });
      }
      items.push({
        label: "Duplicate",
        icon: "copy",
        shortcut: "Ctrl+D",
        onClick: handleDuplicate,
      });
      items.push({
        label: "Delete",
        icon: "trash",
        shortcut: "Del",
        danger: true,
        onClick: handleDelete,
      });
      return items;
    }
    if (contextMenu?.edgeId) {
      return [
        {
          label: "Remove",
          icon: "trash",
          shortcut: "Del",
          danger: true,
          onClick: handleDelete,
        },
      ];
    }
    // Pane context menu
    return [
      {
        label: "Add Sticky Note",
        icon: "message-square",
        onClick: () => handleAddStickyNote(contextMenu?.x, contextMenu?.y),
      },
      {
        label: "Add Text Label",
        icon: "type",
        onClick: () => handleAddTextAnnotation(contextMenu?.x, contextMenu?.y),
      },
      {
        label: "Auto Layout",
        icon: "auto-layout",
        onClick: handleAutoLayout,
        disabled: nodes.length === 0,
      },
      {
        label: "Zoom to Fit",
        icon: "zoom-fit",
        onClick: handleZoomFit,
        disabled: nodes.length === 0,
      },
      {
        label: "Undo",
        icon: "undo",
        shortcut: "Ctrl+Z",
        onClick: handleUndo,
        disabled: !history.canUndo(),
      },
      {
        label: "Redo",
        icon: "redo",
        shortcut: "Ctrl+Shift+Z",
        onClick: handleRedo,
        disabled: !history.canRedo(),
      },
    ];
  }, [
    contextMenu,
    nodes,
    handleDelete,
    handleDuplicate,
    handleAddComment,
    handleAddStickyNote,
    handleAddTextAnnotation,
    handleAutoLayout,
    handleZoomFit,
    handleUndo,
    handleRedo,
    history,
  ]);

  // -- Saved workflow ref ----------------------
  const savedWorkflowRef = useRef<Workflow | null>(
    (workflow as Workflow) || null
  );
  useEffect(() => {
    savedWorkflowRef.current = (workflow as Workflow) || null;
  }, [workflow]);

  // -- Validation ------------------------------
  const validateWorkflow = useCallback((): string[] => {
    const errors: string[] = [];
    for (const node of nodes) {
      const pluginRef = (node.data?.pluginRef as string) || "";
      const schema = registry.get(pluginRef);
      if (!schema) continue;
      const nodeName = (node.data?.label as string) || node.id;

      for (const field of schema.inputs) {
        if (!field.required) continue;
        const mapping =
          usePanelStore.getState().getNodeMappings(node.id)[field.key];
        const hasStaticValue =
          mapping?.mode === "static" &&
          mapping.staticValue !== undefined &&
          mapping.staticValue !== "";
        const hasExpression =
          mapping?.mode === "expression" && !!mapping.celExpression;
        const isWired = edges.some(
          (e) =>
            e.target === node.id &&
            (e.data?.targetField as string) === field.key
        );
        const hasDefault =
          field.default !== undefined && field.default !== "";

        if (!hasStaticValue && !hasExpression && !isWired && !hasDefault) {
          errors.push(
            `"${nodeName}" is missing required field "${field.label}"`
          );
        }
      }
    }
    return errors;
  }, [nodes, edges, registry]);

  // -- Live execution overlay -----------------
  const execOverlay = useExecutionOverlayStore();

  // Execution polling hook -- drives the overlay store
  useExecutionPolling({
    instanceId: execOverlay.activeInstanceId,
    enabled: execOverlay.isLive,
  });

  // Show toast on terminal status
  const prevInstanceStatusRef = useRef<string | null>(null);
  useEffect(() => {
    const status = execOverlay.instanceStatus;
    const prev = prevInstanceStatusRef.current;
    prevInstanceStatusRef.current = status;

    // Only toast when transitioning to terminal
    if (prev === status) return;
    if (status === "completed") {
      toast.success("Workflow completed", "All steps finished successfully");
    } else if (status === "failed") {
      const failedNode = Object.entries(execOverlay.nodeStatuses).find(
        ([, ns]) => ns.status === "failed"
      );
      toast.error(
        `Step "${failedNode?.[0] || "Unknown"}" failed`,
        failedNode?.[1]?.error || "A step in the workflow failed"
      );
    } else if (status === "cancelled") {
      toast.warning("Workflow cancelled");
    }
  }, [execOverlay.instanceStatus, execOverlay.nodeStatuses, toast]);

  // -- Persist run outputs to cache when execution finishes --
  const wasLiveRef = useRef(false);
  useEffect(() => {
    if (wasLiveRef.current && !execOverlay.isLive) {
      // Transition from live -> not-live: run just completed
      const wfId = savedWorkflowRef.current?.id;
      if (wfId) {
        const outputs: Record<string, Record<string, unknown>> = {};
        for (const [nid, ns] of Object.entries(execOverlay.nodeStatuses)) {
          if (ns.output) outputs[nid] = ns.output;
        }
        if (Object.keys(outputs).length > 0) {
          useNodeOutputCacheStore.getState().mergeBulk(wfId, outputs);
        }
      }
    }
    wasLiveRef.current = execOverlay.isLive;
  }, [execOverlay.isLive, execOverlay.nodeStatuses]);

  // -- Build payload ---------------------------
  const buildWorkflowPayload =
    useCallback(async (): Promise<Partial<Workflow>> => {
      const nodeMappings: Record<string, Record<string, unknown>> = {};

      for (const [nodeId, fields] of Object.entries(inputMappings)) {
        if (!nodeMappings[nodeId]) nodeMappings[nodeId] = {};
        for (const [key, mapping] of Object.entries(fields)) {
          if (
            mapping.mode === "static" &&
            mapping.staticValue !== undefined &&
            mapping.staticValue !== ""
          ) {
            nodeMappings[nodeId][key] = String(mapping.staticValue);
          } else if (mapping.mode === "expression" && mapping.celExpression) {
            nodeMappings[nodeId][key] = `=${mapping.celExpression}`;
          }
        }
      }

      const [{ buildConditionExpression }, { TRIGGER_TYPE_MAP }] = await Promise.all([
        import("../utils/cel-builder"),
        import("../utils/trigger-types"),
      ]);

      return {
        name: workflowName,
        description: workflowDescription || undefined,
        nodes: nodes
          .filter((n) => n.type === "task")
          .map((n) => {
            const nodeId = n.id;
            const nodeParams = parameterValues[nodeId];
            const serializedParams = nodeParams
              ? Object.values(nodeParams).map((pv) => ({
                  key: pv.key,
                  mode: pv.mode,
                  value: pv.mode === "static" ? pv.value : undefined,
                  expression: pv.mode === "expression" ? pv.expression : undefined,
                }))
              : undefined;

            // Build trigger_config for trigger nodes
            const pluginRef = (n.data.pluginRef as string) || "";
            const nodeKind = (n.data.nodeKind as "trigger" | "action" | "capability") || undefined;
            let triggerConfig: { trigger_type: string; cron?: string; event_name?: string; path?: string } | undefined;
            if (nodeKind === "trigger") {
              const triggerType = TRIGGER_TYPE_MAP[pluginRef] || "manual";
              triggerConfig = { trigger_type: triggerType };
              if (triggerType === "cron" && nodeParams) {
                const cronParam = Object.values(nodeParams).find((p) => p.key === "cron");
                if (cronParam?.value) triggerConfig.cron = String(cronParam.value);
              }
              if (triggerType === "webhook" && nodeParams) {
                const pathParam = Object.values(nodeParams).find((p) => p.key === "path");
                if (pathParam?.value) triggerConfig.path = String(pathParam.value);
              }
              if (triggerType === "event" && nodeParams) {
                const eventParam = Object.values(nodeParams).find((p) => p.key === "event_name");
                if (eventParam?.value) triggerConfig.event_name = String(eventParam.value);
              }
            }

            return {
              id: nodeId,
              name: (n.data.label as string) || nodeId,
              kind: nodeKind,
              type: pluginRef.startsWith("plugin:") ? "plugin" : "builtin",
              plugin_ref: pluginRef,
              input_mapping: nodeMappings[nodeId] || undefined,
              parameters: serializedParams?.length ? serializedParams : undefined,
              trigger_config: triggerConfig,
              requires_approval: (n.data.requiresApproval as boolean) || undefined,
              position: n.position,
              parent_id: n.parentId || undefined,
            };
          }),
        edges: edges.map((e) => {
          const condition = edgeConditions[e.id];
          const hasValidRules =
            condition &&
            condition.rules.length > 0 &&
            condition.rules.every((r) =>
              "field" in r ? r.field && r.value !== "" : r.rules.length > 0
            );
          const celExpr = hasValidRules
            ? buildConditionExpression(condition)
            : "";
          return {
            id: e.id,
            source: e.source,
            target: e.target,
            condition: celExpr || undefined,
          };
        }),
        capability_edges: capabilityEdges.length
          ? capabilityEdges.map((ce) => ({
              id: ce.id,
              source_node_id: ce.sourceNodeId,
              target_node_id: ce.targetNodeId,
              target_port_key: ce.targetPortKey,
            }))
          : undefined,
        annotations: annotations.length
          ? annotations.map((a) => {
              // Sync position from xyflow node (may have been dragged)
              const prefix = a.type === "text" ? "text_" : "sticky_";
              const xyNode = nodes.find((n) => n.id === `${prefix}${a.id}`);
              return {
                id: a.id,
                type: a.type,
                content: a.content,
                position: xyNode?.position ?? a.position,
                style: a.style,
              };
            })
          : undefined,
      };
    }, [workflowName, workflowDescription, nodes, edges, inputMappings, parameterValues, edgeConditions, capabilityEdges, annotations]);

  // -- Save ------------------------------------
  const handleSave = useCallback(async (): Promise<Workflow | null> => {
    if (nodes.length === 0) {
      toast.warning(
        "Nothing to save",
        "Add at least one step to your workflow"
      );
      return null;
    }
    setIsSaving(true);
    try {
      const wf = await buildWorkflowPayload();
      if (savedWorkflowRef.current?.id) {
        wf.id = savedWorkflowRef.current.id;
      }
      if (config.onSave) {
        const result = await config.onSave(wf);
        if (result && typeof result === "object" && "id" in result) {
          savedWorkflowRef.current = result as Workflow;
        }
      }
      if (config.onChange) config.onChange(wf);
      history.markClean();
      toast.success(
        "Workflow saved",
        `${nodes.length} steps saved successfully`
      );
      return savedWorkflowRef.current;
    } catch (err) {
      const msg =
        err instanceof Error ? err.message : "An unexpected error occurred";
      toast.error("Failed to save workflow", msg);
      return null;
    } finally {
      setIsSaving(false);
    }
  }, [nodes, buildWorkflowPayload, config, toast, history]);

  // -- Run -------------------------------------
  const handleRun = useCallback(async () => {
    if (nodes.length === 0) {
      toast.warning(
        "Nothing to run",
        "Add at least one step to your workflow"
      );
      return;
    }
    const errors = validateWorkflow();
    if (errors.length > 0) {
      const shown = errors.slice(0, 3);
      const more = errors.length > 3 ? ` (+${errors.length - 3} more)` : "";
      toast.error("Missing required fields", shown.join("\n") + more);
      return;
    }
    if (!config.onRun) {
      toast.info("No run handler", "Run is not configured");
      return;
    }
    setIsRunning(true);
    try {
      const saved = await handleSave();
      if (!saved || !saved.id) {
        toast.error(
          "Cannot run",
          "Workflow must be saved before running. Check for save errors above."
        );
        return;
      }
      const instanceId = await config.onRun(saved);
      if (!instanceId) {
        toast.error(
          "Failed to start",
          "The workflow was saved but could not be started."
        );
        return;
      }
      // Start live execution overlay
      const taskNodeCount = nodes.filter((n) => n.type === "task").length;
      useExecutionOverlayStore.getState().startLiveRun(
        instanceId,
        taskNodeCount,
        workflowName
      );
      toast.info("Workflow running...", "Monitoring execution on canvas");
    } catch (err) {
      const msg =
        err instanceof Error ? err.message : "An unexpected error occurred";
      toast.error("Failed to run workflow", msg);
    } finally {
      setIsRunning(false);
    }
  }, [nodes, validateWorkflow, handleSave, config, toast, workflowName]);

  // -- Unsaved changes warning -----------------
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (history.isDirty) {
        e.preventDefault();
      }
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [history.isDirty]);

  // -- Keyboard shortcuts ----------------------
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        handleDelete();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "z" && !e.shiftKey) {
        e.preventDefault();
        handleUndo();
        return;
      }
      if (
        ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "Z") ||
        ((e.ctrlKey || e.metaKey) && e.key === "y")
      ) {
        e.preventDefault();
        handleRedo();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        handleSave();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        handleRun();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "d") {
        e.preventDefault();
        handleDuplicate();
        return;
      }
      if (e.key === "Escape") {
        clearSelection();
        closePanel();
        setContextMenu(null);
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "g") {
        e.preventDefault();
        setSnapToGrid((s) => !s);
        return;
      }
      if (e.key === "?" && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        setShowShortcuts((s) => !s);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [
    handleDelete,
    handleUndo,
    handleRedo,
    handleSave,
    handleRun,
    handleDuplicate,
    clearSelection,
    closePanel,
  ]);

  // -- Render ----------------------------------
  return (
    <div className="flex h-full w-full relative bg-orbflow-bg">
      <div
        className="flex-1 relative"
        onDragOver={onDragOver}
        onDrop={onDrop}
      >
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onNodeClick={onNodeClick}
          onEdgeClick={onEdgeClick}
          onPaneClick={onPaneClick}
          onNodeContextMenu={onNodeContextMenu}
          onEdgeContextMenu={onEdgeContextMenu}
          onPaneContextMenu={onPaneContextMenu}
          onNodeDrag={onNodeDrag}
          onNodeDragStop={onNodeDragStop}
          isValidConnection={isValidConnection}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          defaultEdgeOptions={{ type: "conditional" }}
          fitView
          className={themeMode === "light" ? "bg-orbflow-bg" : "bg-obsidian"}
          colorMode={themeMode}
          snapToGrid={snapToGrid}
          snapGrid={[20, 20]}
          deleteKeyCode={null}
          nodesDraggable={canvasInteractive}
          nodesConnectable={canvasInteractive}
          elementsSelectable={canvasInteractive}
          proOptions={{ hideAttribution: true }}
        >
          <Background
            color={snapToGrid
              ? (themeMode === "light" ? "rgba(0,0,0,0.18)" : "rgba(255,255,255,0.12)")
              : (themeMode === "light" ? "#000000" : "#ffffff")}
            gap={snapToGrid ? 20 : 40}
            size={1}
            variant={snapToGrid ? BackgroundVariant.Lines : BackgroundVariant.Dots}
            className={snapToGrid ? undefined : (themeMode === "light" ? "opacity-[0.08]" : "opacity-[0.03]")}
          />
          <LayoutControls
            direction={layoutDirection}
            algorithm={layoutAlgorithm}
            onLayout={handleAutoLayout}
            onToggleDirection={handleToggleDirection}
            onAlgorithmChange={setLayoutAlgorithm}
          />
          <ViewportControls
            interactive={canvasInteractive}
            onToggleInteractive={handleToggleInteractive}
          />
          <InternalsSync nodeIds={nodes.map((n) => n.id)} />
          <Controls position="bottom-right" className="!m-6" />
          <MiniMap
            className="!m-6"
            nodeColor={(node) => {
              const ref = node.data?.pluginRef as string | undefined;
              if (ref) {
                const schema = registry.get(ref);
                if (schema?.color) return schema.color;
              }
              return "#7C5CFC";
            }}
            maskColor={themeMode === "light" ? "rgba(245, 245, 247, 0.7)" : "rgba(10, 10, 12, 0.7)"}
          />
        </ReactFlow>

        {/* Welcome overlay when canvas is empty */}
        {isEmpty && !config.readOnly && (
          <div className="absolute inset-0 flex items-center justify-center z-10 pointer-events-none">
            <div className="pointer-events-auto max-w-lg w-full px-4">
              <div className="text-center mb-8 animate-fade-in-up">
                <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-electric-indigo/10 border border-electric-indigo/20 mb-5 animate-float">
                  <NodeIcon
                    name="workflow"
                    className="w-8 h-8 text-electric-indigo"
                  />
                </div>
                <h2 className="text-2xl font-bold mb-2 tracking-tight text-orbflow-text-secondary">
                  What would you like to automate?
                </h2>
                <p className="text-sm max-w-sm mx-auto leading-relaxed text-orbflow-text-faint">
                  Pick a template to get started, or press + to add steps
                </p>
              </div>

              <div className="grid grid-cols-2 gap-3 animate-fade-in-up stagger-2">
                {QUICK_TEMPLATES.map((t) => (
                  <button
                    key={t.id}
                    onClick={() => loadTemplate(t)}
                    className="group text-left p-4 rounded-xl backdrop-blur-sm
                      hover:bg-orbflow-surface-hover transition-all duration-300
                      active:scale-[0.98] border border-orbflow-border bg-orbflow-glass-bg"
                  >
                    <div className="flex items-start gap-3">
                      <div
                        className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0 transition-transform duration-300 group-hover:scale-110"
                        style={{ backgroundColor: t.color + "15" }}
                      >
                        <NodeIcon
                          name={t.icon}
                          className="w-4 h-4"
                          style={{ color: t.color }}
                        />
                      </div>
                      <div className="min-w-0">
                        <div className="text-[13px] font-semibold mb-0.5 text-orbflow-text-secondary">
                          {t.name}
                        </div>
                        <div className="text-[10px] leading-relaxed text-orbflow-text-faint">
                          {t.description}
                        </div>
                      </div>
                    </div>
                    <div className="mt-3 flex items-center gap-1 flex-wrap">
                      {t.nodes.map((n, i) => (
                        <div key={n.id} className="flex items-center">
                          <span className="text-[9px] font-mono px-1.5 py-0.5 rounded text-orbflow-text-faint bg-orbflow-add-btn-bg">
                            {(n.data.label as string).split(" ")[0]}
                          </span>
                          {i < t.nodes.length - 1 && (
                            <NodeIcon
                              name="arrow-right"
                              className="w-3 h-3 mx-0.5 text-orbflow-text-ghost"
                            />
                          )}
                        </div>
                      ))}
                    </div>
                  </button>
                ))}
              </div>

              <div className="text-center mt-6 animate-fade-in stagger-4">
                <button
                  onClick={() => {
                    const canvasEl = document.querySelector(".react-flow");
                    if (canvasEl) {
                      const rect = canvasEl.getBoundingClientRect();
                      picker.openPicker({
                        x: rect.left + rect.width / 2 - 144,
                        y: rect.top + rect.height / 2 - 192,
                      });
                    }
                  }}
                  className="mx-auto mb-4 w-12 h-12 rounded-full flex items-center justify-center
                    bg-electric-indigo/15 border border-electric-indigo/25
                    text-electric-indigo hover:bg-electric-indigo/25 hover:border-electric-indigo/40
                    transition-all duration-200 active:scale-95 shadow-lg shadow-electric-indigo/10"
                  title="Add your first step"
                >
                  <NodeIcon name="plus" className="w-6 h-6" />
                </button>
                <div className="flex items-center justify-center gap-4 text-[10px] text-orbflow-text-ghost">
                  <span className="uppercase tracking-widest">
                    or pick a template above
                  </span>
                </div>
                <div className="flex justify-center gap-6 mt-3 text-[9px] font-mono text-orbflow-text-ghost">
                  <span>Ctrl+Z undo</span>
                  <span>Ctrl+S save</span>
                  <span>Ctrl+Enter run</span>
                  <span>Del delete</span>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Toolbar */}
        {!config.readOnly && !isEmpty && (
          <BuilderToolbar
            workflowName={workflowName}
            onNameChange={setWorkflowName}
            workflowDescription={workflowDescription}
            onDescriptionChange={setWorkflowDescription}
            onSave={handleSave}
            onRun={config.onRun ? handleRun : undefined}
            onUndo={handleUndo}
            onRedo={handleRedo}
            onDelete={handleDelete}
            onDuplicate={handleDuplicate}
            onAutoLayout={handleAutoLayout}
            onZoomFit={handleZoomFit}
            viewControls={{
              onAddAnnotation: handleAddAnnotation,
              onShowShortcuts: () => setShowShortcuts((s) => !s),
              onToggleGrid: () => setSnapToGrid((s) => !s),
              snapToGrid,
            }}
            isSaving={isSaving}
            isRunning={isRunning}
            triggerType={triggerType}
            triggerInfo={triggerInfo}
            onShowHistory={workflow?.id ? () => setShowVersionHistory(true) : undefined}
          />
        )}

        {/* Execution status bar overlay */}
        <ExecutionStatusBar />
      </div>

      {/* Floating "+" button -- right center of canvas */}
      {!config.readOnly && (
        <button
          onClick={() => {
            const canvasEl = document.querySelector(".react-flow");
            if (canvasEl) {
              const rect = canvasEl.getBoundingClientRect();
              picker.openPicker({
                x: rect.right - 320,
                y: rect.top + rect.height / 2 - 192,
              });
            }
          }}
          className="absolute right-6 top-1/2 -translate-y-1/2 z-20
            w-10 h-10 rounded-full flex items-center justify-center
            backdrop-blur-md
            hover:text-electric-indigo hover:border-electric-indigo/30
            hover:bg-electric-indigo/10 transition-all duration-200
            shadow-lg hover:shadow-electric-indigo/10 active:scale-95
            bg-orbflow-glass-bg border border-orbflow-border text-orbflow-text-muted"
          title="Add a step"
        >
          <NodeIcon name="plus" className="w-5 h-5" />
        </button>
      )}

      {/* Node picker popover */}
      {picker.open && (
        <NodePickerPopover
          position={picker.position}
          allowedKinds={picker.allowedKinds}
          onSelect={handlePickerSelect}
          onClose={picker.closePicker}
        />
      )}

      {/* Node config modal */}
      {configModalNodeId && (
        <NodeConfigModal
          nodeId={configModalNodeId}
          onClose={() => setConfigModalNodeId(null)}
          workflowId={savedWorkflowRef.current?.id}
        />
      )}

      {/* Context menu */}
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenuItems}
          onClose={() => setContextMenu(null)}
        />
      )}

      {/* Shortcut help overlay */}
      {showShortcuts && (
        <ShortcutHelp onClose={() => setShowShortcuts(false)} />
      )}

      {/* Confirmation dialog */}
      {confirmAction && (
        <ConfirmDialog
          title={confirmAction.title}
          message={confirmAction.message}
          confirmLabel={confirmAction.confirmLabel}
          variant={confirmAction.variant}
          onConfirm={() => {
            confirmAction.onConfirm();
            setConfirmAction(null);
          }}
          onCancel={() => setConfirmAction(null)}
        />
      )}

      {/* Comment input dialog */}
      {commentDialog && (
        <CommentInputDialog
          initialValue={commentDialog.initialValue}
          onSubmit={handleCommentSubmit}
          onCancel={() => setCommentDialog(null)}
        />
      )}

      {/* Version history panel */}
      {workflow?.id && (
        <VersionHistory
          workflowId={workflow.id}
          currentVersion={workflow.version ?? 1}
          open={showVersionHistory}
          onClose={() => setShowVersionHistory(false)}
        />
      )}
    </div>
  );
}
