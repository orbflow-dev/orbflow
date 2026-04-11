"use client";

import { useCallback } from "react";
import type {
  Connection,
  Node,
  Edge,
  NodeMouseHandler,
  EdgeMouseHandler,
  IsValidConnection,
  ReactFlowInstance,
} from "@xyflow/react";
import type { DragEvent } from "react";
import { useCanvasStore, usePanelStore, usePickerStore } from "@orbflow/core/stores";
import type { NodeSchemaRegistry } from "../../schemas/registry";
import type { NodeTypeDefinition } from "../../types/schema";
import { generateNodeSlug } from "../../utils/node-slug";
import { useStickyDrop } from "./use-sticky-drop";

/** Seed input mappings with default values from the schema so the backend
 *  validation doesn't reject required fields that have sensible defaults. */
export function seedDefaultMappings(nodeId: string, schema: NodeTypeDefinition) {
  const defaults: Record<string, { targetKey: string; mode: "static"; staticValue: unknown }> = {};
  for (const field of schema.inputs) {
    if (field.default !== undefined && field.default !== null && field.default !== "") {
      defaults[field.key] = { targetKey: field.key, mode: "static", staticValue: field.default };
    }
  }
  if (Object.keys(defaults).length > 0) {
    usePanelStore.getState().loadNodeMappings(nodeId, {
      ...usePanelStore.getState().getNodeMappings(nodeId),
      ...defaults,
    });
  }
}

/** Check whether a path exists from `start` to `target` following edge directions. */
function hasDirectedPath(start: string, target: string, edges: Edge[]): boolean {
  const visited = new Set<string>();
  const stack = [start];
  while (stack.length > 0) {
    const current = stack.pop()!;
    if (current === target) return true;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const edge of edges) {
      if (edge.source === current) {
        stack.push(edge.target);
      }
    }
  }
  return false;
}

/** Create a new ReactFlow node from a schema definition. */
function createNodeFromSchema(
  schema: NodeTypeDefinition,
  position: { x: number; y: number },
): Node {
  const existingIds = useCanvasStore.getState().nodes.map((n) => n.id);
  return {
    id: generateNodeSlug(schema.name, existingIds),
    type: "task",
    position,
    data: {
      label: schema.name,
      pluginRef: schema.pluginRef,
      type: schema.category,
      nodeKind: schema.nodeKind || undefined,
    },
  };
}

/** Create a conditional edge between two nodes. */
function createConditionalEdge(source: string, target: string, idSuffix = ""): Edge {
  return {
    id: `edge_${Date.now()}${idSuffix}`,
    source,
    target,
    sourceHandle: "out",
    targetHandle: "in",
    type: "conditional",
    data: {},
  };
}

/** Position a new node 150px to the right of its source. */
function placeNodeFromSource(newNode: Node, sourceNodeId: string, nodes: Node[]) {
  const sourceNode = nodes.find((n) => n.id === sourceNodeId);
  if (sourceNode) {
    newNode.position = {
      x: sourceNode.position.x + 150,
      y: sourceNode.position.y,
    };
  }
}

/** Replace an edge by inserting a node in the middle, creating two new edges. */
function splitEdgeWithNode(
  newNode: Node,
  edgeId: string,
  nodes: Node[],
  edges: Edge[],
  addNode: (n: Node) => void,
  setEdges: (edges: Edge[]) => void,
  addCanvasEdge: (e: Edge) => void,
) {
  const oldEdge = edges.find((e) => e.id === edgeId);
  if (!oldEdge) {
    addNode(newNode);
    return;
  }

  const sourceNode = nodes.find((n) => n.id === oldEdge.source);
  const targetNode = nodes.find((n) => n.id === oldEdge.target);
  if (sourceNode && targetNode) {
    newNode.position = {
      x: (sourceNode.position.x + targetNode.position.x) / 2,
      y: (sourceNode.position.y + targetNode.position.y) / 2,
    };
  }

  addNode(newNode);
  setEdges(edges.filter((e) => e.id !== edgeId));
  addCanvasEdge(createConditionalEdge(oldEdge.source, newNode.id, "_a"));
  addCanvasEdge(createConditionalEdge(newNode.id, oldEdge.target, "_b"));
}

interface CanvasHandlerDeps {
  nodes: Node[];
  edges: Edge[];
  registry: NodeSchemaRegistry;
  reactFlowInstance: ReactFlowInstance;
  pushHistory: () => void;
  setConfigModalNodeId: (id: string | null) => void;
  setContextMenu: (menu: { x: number; y: number; nodeId?: string; edgeId?: string } | null) => void;
}

export function useCanvasHandlers(deps: CanvasHandlerDeps) {
  const {
    nodes,
    edges,
    registry,
    reactFlowInstance,
    pushHistory,
    setConfigModalNodeId,
    setContextMenu,
  } = deps;

  const {
    addNode,
    addEdge: addCanvasEdge,
    addCapabilityEdge,
    setEdges,
    selectNode,
    selectEdge,
    toggleNodeSelection,
    toggleEdgeSelection,
    clearSelection,
  } = useCanvasStore();

  const { closePanel } = usePanelStore();
  const picker = usePickerStore();
  const { onNodeDrag, onNodeDragStop } = useStickyDrop();

  const isValidConnection: IsValidConnection = useCallback(
    (connection) => {
      if (!connection.source || !connection.target) return false;
      if (connection.source === connection.target) return false;

      const targetHandle = connection.targetHandle || "";

      if (targetHandle.startsWith("cap:")) {
        const portKey = targetHandle.slice(4);
        const sourceNode = nodes.find((n) => n.id === connection.source);
        const targetNode = nodes.find((n) => n.id === connection.target);
        if (!sourceNode || !targetNode) return false;

        const sourceSchema = registry.get(sourceNode.data?.pluginRef as string);
        const targetSchema = registry.get(targetNode.data?.pluginRef as string);
        if (!sourceSchema || !targetSchema) return false;

        if (sourceSchema.nodeKind !== "capability") return false;

        const port = targetSchema.capabilityPorts?.find((p) => p.key === portKey);
        if (!port) return false;

        if (sourceSchema.providesCapability !== port.capabilityType) return false;

        // Prevent duplicate capability edges to the same port
        const capDuplicate = edges.some(
          (e) =>
            e.source === connection.source &&
            e.target === connection.target &&
            e.targetHandle === connection.targetHandle,
        );
        return !capDuplicate;
      }

      // Prevent duplicate edges between the same source and target
      const duplicateExists = edges.some(
        (e) => e.source === connection.source && e.target === connection.target,
      );
      if (duplicateExists) return false;

      // Reject if connecting would create a cycle
      return !hasDirectedPath(connection.target, connection.source!, edges);
    },
    [edges, nodes, registry],
  );

  const onConnect = useCallback(
    (connection: Connection) => {
      pushHistory();
      const targetHandle = connection.targetHandle || "";

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

      addCanvasEdge({
        ...createConditionalEdge(connection.source!, connection.target!),
        sourceHandle: connection.sourceHandle || undefined,
        targetHandle: connection.targetHandle || undefined,
      });
    },
    [pushHistory, addCanvasEdge, addCapabilityEdge],
  );

  // Multi-select aware: Shift/Ctrl+click toggles, plain click replaces
  const onNodeClick: NodeMouseHandler = useCallback(
    (event, node) => {
      if (event.shiftKey || event.ctrlKey || event.metaKey) {
        toggleNodeSelection(node.id);
        return;
      }
      selectNode(node.id);
      if (node.type === "stickyNote" || node.type === "textAnnotation") return;
      setConfigModalNodeId(node.id);
    },
    [selectNode, toggleNodeSelection, setConfigModalNodeId],
  );

  const onEdgeClick: EdgeMouseHandler = useCallback(
    (event, edge) => {
      if (event.shiftKey || event.ctrlKey || event.metaKey) {
        toggleEdgeSelection(edge.id);
        return;
      }
      selectEdge(edge.id);
    },
    [selectEdge, toggleEdgeSelection],
  );

  const onPaneClick = useCallback(() => {
    clearSelection();
    closePanel();
    setContextMenu(null);
    picker.closePicker();
  }, [clearSelection, closePanel, setContextMenu, picker]);

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
      const newNode = createNodeFromSchema(schema, position);
      addNode(newNode);
      seedDefaultMappings(newNode.id, schema);
      selectNode(newNode.id);
    },
    [registry, reactFlowInstance, pushHistory, addNode, selectNode],
  );

  const handlePickerSelect = useCallback(
    (pluginRef: string) => {
      const schema = registry.get(pluginRef);
      if (!schema) return;

      pushHistory();
      const { sourceNodeId, sourceEdgeId, position: pickerPos } = picker;
      const newNode = createNodeFromSchema(schema, { x: 0, y: 0 });

      if (sourceNodeId) {
        placeNodeFromSource(newNode, sourceNodeId, nodes);
        addNode(newNode);
        addCanvasEdge(createConditionalEdge(sourceNodeId, newNode.id));
      } else if (sourceEdgeId) {
        splitEdgeWithNode(newNode, sourceEdgeId, nodes, edges, addNode, setEdges, addCanvasEdge);
      } else {
        newNode.position = reactFlowInstance.screenToFlowPosition({
          x: pickerPos.x,
          y: pickerPos.y,
        });
        addNode(newNode);
      }

      seedDefaultMappings(newNode.id, schema);
      selectNode(newNode.id);
      if (sourceNodeId || sourceEdgeId) {
        setConfigModalNodeId(newNode.id);
      }
      picker.closePicker();
    },
    [registry, pushHistory, picker, nodes, edges, addNode, addCanvasEdge, setEdges, selectNode, reactFlowInstance, setConfigModalNodeId],
  );

  // Context menu: if right-clicked node is NOT in selection, replace selection
  const onNodeContextMenu = useCallback(
    (event: React.MouseEvent, node: Node) => {
      event.preventDefault();
      const { selectedNodeIds } = useCanvasStore.getState();
      if (!selectedNodeIds.has(node.id)) {
        selectNode(node.id);
      }
      setContextMenu({ x: event.clientX, y: event.clientY, nodeId: node.id });
    },
    [selectNode, setContextMenu],
  );

  const onEdgeContextMenu = useCallback(
    (event: React.MouseEvent, edge: Edge) => {
      event.preventDefault();
      const { selectedEdgeIds } = useCanvasStore.getState();
      if (!selectedEdgeIds.has(edge.id)) {
        selectEdge(edge.id);
      }
      setContextMenu({ x: event.clientX, y: event.clientY, edgeId: edge.id });
    },
    [selectEdge, setContextMenu],
  );

  const onPaneContextMenu = useCallback(
    (event: React.MouseEvent | MouseEvent) => {
      event.preventDefault();
      setContextMenu({
        x: (event as React.MouseEvent).clientX,
        y: (event as React.MouseEvent).clientY,
      });
    },
    [setContextMenu],
  );

  return {
    isValidConnection,
    onConnect,
    onNodeClick,
    onEdgeClick,
    onPaneClick,
    onDragOver,
    onDrop,
    onNodeDrag,
    onNodeDragStop,
    handlePickerSelect,
    onNodeContextMenu,
    onEdgeContextMenu,
    onPaneContextMenu,
  };
}
