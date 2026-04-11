import { useEffect } from "react";
import type { Node, Edge } from "@xyflow/react";
import type { Workflow } from "@orbflow/core";

interface WorkflowLoaderDeps {
  workflow: Partial<Workflow> | undefined;
  setNodes: (nodes: Node[]) => void;
  setEdges: (edges: Edge[]) => void;
  setCapabilityEdges: (edges: { id: string; sourceNodeId: string; targetNodeId: string; targetPortKey: string }[]) => void;
  setAnnotations: (annotations: { id: string; type: "sticky_note" | "text" | "markdown"; content: string; position: { x: number; y: number }; style?: Record<string, unknown> }[]) => void;
  clearHistory: () => void;
}

/** Loads a workflow definition into the canvas stores. */
export function useWorkflowLoader(deps: WorkflowLoaderDeps) {
  const { workflow, setNodes, setEdges, setCapabilityEdges, setAnnotations, clearHistory } = deps;

  useEffect(() => {
    if (!workflow) {
      setNodes([]);
      setEdges([]);
      setCapabilityEdges([]);
      setAnnotations([]);
      clearHistory();
      return;
    }

    const flowNodes: Node[] = (workflow.nodes || []).map((n) => ({
      id: n.id,
      type: "task",
      position: n.position,
      ...(n.parent_id
        ? { parentId: n.parent_id, extent: "parent" as const, zIndex: 1 }
        : {}),
      data: {
        label: n.name || n.id,
        pluginRef: n.plugin_ref,
        type: n.type,
        nodeKind: n.kind || undefined,
        requiresApproval: n.requires_approval || false,
      },
    }));

    const annotationNodes: Node[] = (workflow.annotations || []).map((a) => {
      if (a.type === "text") {
        return {
          id: `text_${a.id}`,
          type: "textAnnotation",
          position: a.position || { x: 0, y: 0 },
          data: { annotationId: a.id, content: a.content || "" },
        };
      }
      const sw = (a.style?.width as number) ?? 200;
      const sh = (a.style?.height as number) ?? 140;
      return {
        id: `sticky_${a.id}`,
        type: "stickyNote",
        position: a.position || { x: 0, y: 0 },
        zIndex: -1,
        style: { width: sw, height: sh },
        data: {
          annotationId: a.id,
          content: a.content || "",
          color: a.style?.color || "yellow",
          width: sw,
          height: sh,
        },
      };
    });

    const rawEdges: Edge[] = (workflow.edges || []).map((e) => ({
      id: e.id,
      source: e.source,
      target: e.target,
      type: "conditional",
      data: { conditionLabel: e.condition || "" },
    }));

    // Deduplicate edges that may exist in saved data
    const seenEdgePairs = new Set<string>();
    const flowEdges = rawEdges.filter((e) => {
      const key = `${e.source}->${e.target}`;
      if (seenEdgePairs.has(key)) return false;
      seenEdgePairs.add(key);
      return true;
    });

    setNodes([...flowNodes, ...annotationNodes]);
    setEdges(flowEdges);
    setCapabilityEdges(
      (workflow.capability_edges || []).map((ce) => ({
        id: ce.id,
        sourceNodeId: ce.source_node_id,
        targetNodeId: ce.target_node_id,
        targetPortKey: ce.target_port_key,
      })),
    );
    setAnnotations(
      (workflow.annotations || []).map((a) => ({
        id: a.id,
        type: a.type as "sticky_note" | "text" | "markdown",
        content: a.content,
        position: a.position,
        style: a.style,
      })),
    );
    clearHistory();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workflow, setNodes, setEdges]);
}
