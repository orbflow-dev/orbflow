import { create } from "zustand";
import {
  applyNodeChanges,
  applyEdgeChanges,
  type Node,
  type Edge,
  type NodeChange,
  type EdgeChange,
} from "@xyflow/react";
import type { CapabilityEdge, Annotation } from "../types/schema";

/**
 * xyflow requires parent nodes to appear BEFORE their children in the nodes
 * array.  Without this ordering xyflow emits "Parent node ... not found" warnings
 * and ignores the parentId relationship entirely.
 *
 * This helper partitions nodes into root nodes (no parentId) and child nodes,
 * returning roots first followed by children.  Single-level nesting is all we
 * need (sticky notes -> task nodes).
 */
function sortNodesParentFirst(nodes: Node[]): Node[] {
  const roots: Node[] = [];
  const children: Node[] = [];
  for (const n of nodes) {
    if (n.parentId) {
      children.push(n);
    } else {
      roots.push(n);
    }
  }
  return [...roots, ...children];
}

interface CanvasStore {
  nodes: Node[];
  edges: Edge[];
  capabilityEdges: CapabilityEdge[];
  annotations: Annotation[];
  selectedNodeIds: Set<string>;
  selectedEdgeIds: Set<string>;

  setNodes: (nodes: Node[]) => void;
  setEdges: (edges: Edge[]) => void;
  setCapabilityEdges: (edges: CapabilityEdge[]) => void;
  setAnnotations: (annotations: Annotation[]) => void;
  onNodesChange: (changes: NodeChange[]) => void;
  onEdgesChange: (changes: EdgeChange[]) => void;
  addNode: (node: Node) => void;
  removeNode: (nodeId: string) => void;
  removeNodes: (nodeIds: string[]) => void;
  updateNodeData: (nodeId: string, data: Record<string, unknown>) => void;
  updateEdgeData: (edgeId: string, data: Record<string, unknown>) => void;
  addEdge: (edge: Edge) => void;
  addCapabilityEdge: (edge: CapabilityEdge) => void;
  removeCapabilityEdge: (edgeId: string) => void;
  addAnnotation: (annotation: Annotation) => void;
  removeAnnotation: (id: string) => void;
  updateAnnotation: (id: string, data: Partial<Annotation>) => void;
  updateAnnotationStyle: (id: string, style: Record<string, unknown>) => void;
  updateNode: (nodeId: string, update: Partial<Node>) => void;
  updateNodes: (updates: { id: string; update: Partial<Node> }[]) => void;

  // Selection — multi-select aware
  selectNode: (id: string | null) => void;
  selectEdge: (id: string | null) => void;
  selectNodes: (ids: string[]) => void;
  selectEdges: (ids: string[]) => void;
  setSelection: (nodeIds: string[], edgeIds: string[]) => void;
  toggleNodeSelection: (id: string) => void;
  toggleEdgeSelection: (id: string) => void;
  selectAll: () => void;
  clearSelection: () => void;
}

/** Selector: get first selected node ID (backward compat) */
export function selectedNodeId(state: CanvasStore): string | null {
  const it = state.selectedNodeIds.values().next();
  return it.done ? null : it.value;
}

/** Selector: get first selected edge ID (backward compat) */
export function selectedEdgeId(state: CanvasStore): string | null {
  const it = state.selectedEdgeIds.values().next();
  return it.done ? null : it.value;
}

/** Selector: count child nodes parented to a given node */
export function childCountForNode(state: CanvasStore, parentId: string): number {
  let count = 0;
  for (const n of state.nodes) {
    if (n.parentId === parentId) count++;
  }
  return count;
}

export const useCanvasStore = create<CanvasStore>((set) => ({
  nodes: [],
  edges: [],
  capabilityEdges: [],
  annotations: [],
  selectedNodeIds: new Set<string>(),
  selectedEdgeIds: new Set<string>(),

  setNodes: (nodes) => set({ nodes: sortNodesParentFirst(nodes) }),
  setEdges: (edges) => set({ edges }),
  setCapabilityEdges: (capEdges) => set({ capabilityEdges: capEdges }),
  setAnnotations: (annotations) => set({ annotations }),

  onNodesChange: (changes) =>
    set((s) => {
      // Filter out select changes — store is source of truth for selection
      const filtered = changes.filter((c) => c.type !== "select");
      if (filtered.length === 0) return s;
      return { nodes: applyNodeChanges(filtered, s.nodes) };
    }),

  onEdgesChange: (changes) =>
    set((s) => {
      const filtered = changes.filter((c) => c.type !== "select");
      if (filtered.length === 0) return s;
      return { edges: applyEdgeChanges(filtered, s.edges) };
    }),

  addNode: (node) =>
    set((s) => {
      const next = [...s.nodes, node];
      return { nodes: node.parentId ? sortNodesParentFirst(next) : next };
    }),

  removeNode: (nodeId) =>
    set((s) => {
      const parent = s.nodes.find((n) => n.id === nodeId);
      const unparented = s.nodes.map((n) =>
        n.parentId === nodeId
          ? {
              ...n,
              parentId: undefined,
              extent: undefined,
              position: {
                x: n.position.x + (parent?.position.x ?? 0),
                y: n.position.y + (parent?.position.y ?? 0),
              },
              zIndex: 0,
            }
          : n
      );
      const nextSelected = new Set(s.selectedNodeIds);
      nextSelected.delete(nodeId);
      return {
        nodes: unparented.filter((n) => n.id !== nodeId),
        edges: s.edges.filter(
          (e) => e.source !== nodeId && e.target !== nodeId
        ),
        capabilityEdges: s.capabilityEdges.filter(
          (e) => e.sourceNodeId !== nodeId && e.targetNodeId !== nodeId
        ),
        selectedNodeIds: nextSelected,
      };
    }),

  removeNodes: (nodeIds) =>
    set((s) => {
      const idSet = new Set(nodeIds);
      const unparented = s.nodes.map((n) => {
        if (!n.parentId || !idSet.has(n.parentId)) return n;
        const parent = s.nodes.find((p) => p.id === n.parentId);
        return {
          ...n,
          parentId: undefined,
          extent: undefined,
          position: {
            x: n.position.x + (parent?.position.x ?? 0),
            y: n.position.y + (parent?.position.y ?? 0),
          },
          zIndex: 0,
        };
      });
      const nextSelected = new Set(s.selectedNodeIds);
      for (const id of nodeIds) nextSelected.delete(id);
      return {
        nodes: unparented.filter((n) => !idSet.has(n.id)),
        edges: s.edges.filter(
          (e) => !idSet.has(e.source) && !idSet.has(e.target)
        ),
        capabilityEdges: s.capabilityEdges.filter(
          (e) => !idSet.has(e.sourceNodeId) && !idSet.has(e.targetNodeId)
        ),
        selectedNodeIds: nextSelected,
      };
    }),

  updateNodeData: (nodeId, data) =>
    set((s) => ({
      nodes: s.nodes.map((n) =>
        n.id === nodeId ? { ...n, data: { ...n.data, ...data } } : n
      ),
    })),

  updateEdgeData: (edgeId, data) =>
    set((s) => ({
      edges: s.edges.map((e) =>
        e.id === edgeId ? { ...e, data: { ...e.data, ...data } } : e
      ),
    })),

  addEdge: (edge) =>
    set((s) => {
      // Prevent duplicate edges between the same source and target
      const exists = s.edges.some(
        (e) => e.source === edge.source && e.target === edge.target,
      );
      if (exists) return s;
      return { edges: [...s.edges, edge] };
    }),

  addCapabilityEdge: (edge) =>
    set((s) => ({ capabilityEdges: [...s.capabilityEdges, edge] })),

  removeCapabilityEdge: (edgeId) =>
    set((s) => ({
      capabilityEdges: s.capabilityEdges.filter((e) => e.id !== edgeId),
    })),

  addAnnotation: (annotation) =>
    set((s) => ({ annotations: [...s.annotations, annotation] })),

  removeAnnotation: (id) =>
    set((s) => ({
      annotations: s.annotations.filter((a) => a.id !== id),
    })),

  updateAnnotation: (id, data) =>
    set((s) => ({
      annotations: s.annotations.map((a) =>
        a.id === id ? { ...a, ...data } : a
      ),
    })),

  updateAnnotationStyle: (id, style) =>
    set((s) => ({
      annotations: s.annotations.map((a) =>
        a.id === id ? { ...a, style: { ...a.style, ...style } } : a
      ),
    })),

  updateNode: (nodeId, update) =>
    set((s) => {
      const updated = s.nodes.map((n) =>
        n.id === nodeId ? { ...n, ...update } : n
      );
      // Re-sort when parentId changes so parents always precede children
      return { nodes: "parentId" in update ? sortNodesParentFirst(updated) : updated };
    }),

  updateNodes: (updates) =>
    set((s) => {
      const updateMap = new Map(updates.map((u) => [u.id, u.update]));
      const hasParentChange = updates.some((u) => "parentId" in u.update);
      const mapped = s.nodes.map((n) => {
        const u = updateMap.get(n.id);
        return u ? { ...n, ...u } : n;
      });
      return { nodes: hasParentChange ? sortNodesParentFirst(mapped) : mapped };
    }),

  // ── Selection ──────────────────────────────────────

  selectNode: (id) =>
    set({
      selectedNodeIds: id ? new Set([id]) : new Set<string>(),
      selectedEdgeIds: new Set<string>(),
    }),

  selectEdge: (id) =>
    set({
      selectedEdgeIds: id ? new Set([id]) : new Set<string>(),
      selectedNodeIds: new Set<string>(),
    }),

  selectNodes: (ids) =>
    set({ selectedNodeIds: new Set(ids), selectedEdgeIds: new Set<string>() }),

  selectEdges: (ids) =>
    set({ selectedEdgeIds: new Set(ids), selectedNodeIds: new Set<string>() }),

  setSelection: (nodeIds, edgeIds) =>
    set({ selectedNodeIds: new Set(nodeIds), selectedEdgeIds: new Set(edgeIds) }),

  toggleNodeSelection: (id) =>
    set((s) => {
      const next = new Set(s.selectedNodeIds);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return { selectedNodeIds: next };
    }),

  toggleEdgeSelection: (id) =>
    set((s) => {
      const next = new Set(s.selectedEdgeIds);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return { selectedEdgeIds: next };
    }),

  selectAll: () =>
    set((s) => ({
      selectedNodeIds: new Set(s.nodes.map((n) => n.id)),
      selectedEdgeIds: new Set(s.edges.map((e) => e.id)),
    })),

  clearSelection: () =>
    set({
      selectedNodeIds: new Set<string>(),
      selectedEdgeIds: new Set<string>(),
    }),
}));
