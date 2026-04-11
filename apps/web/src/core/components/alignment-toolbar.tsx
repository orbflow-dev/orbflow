"use client";

import { useCallback } from "react";
import { cn } from "../utils/cn";
import { NodeIcon } from "./icons";
import { Tooltip } from "./tooltip";
import { useCanvasStore } from "@/store/canvas-store";
import { useHistoryStore } from "@/store/history-store";
import {
  alignNodes,
  distributeNodes,
  type AlignDirection,
  type DistributeDirection,
  type NodeRect,
} from "../utils/alignment";

interface AlignmentToolbarProps {
  className?: string;
}

const ALIGN_ACTIONS: { direction: AlignDirection; icon: string; label: string }[] = [
  { direction: "left", icon: "align-left", label: "Align left" },
  { direction: "center", icon: "align-center-horizontal", label: "Align center" },
  { direction: "right", icon: "align-right", label: "Align right" },
  { direction: "top", icon: "align-top", label: "Align top" },
  { direction: "middle", icon: "align-center-vertical", label: "Align middle" },
  { direction: "bottom", icon: "align-bottom", label: "Align bottom" },
];

const DISTRIBUTE_ACTIONS: { direction: DistributeDirection; icon: string; label: string }[] = [
  { direction: "horizontal", icon: "distribute-horizontal", label: "Distribute horizontally" },
  { direction: "vertical", icon: "distribute-vertical", label: "Distribute vertically" },
];

export function AlignmentToolbar({ className }: AlignmentToolbarProps) {
  const { selectedNodeIds, updateNodes } = useCanvasStore();
  const history = useHistoryStore();

  const count = selectedNodeIds.size;

  const handleAlign = useCallback(
    (direction: AlignDirection) => {
      const { nodes: allNodes, edges: allEdges, selectedNodeIds: ids } = useCanvasStore.getState();
      const rects: NodeRect[] = allNodes
        .filter((n) => ids.has(n.id))
        .map((n) => ({
          id: n.id, x: n.position.x, y: n.position.y,
          width: n.measured?.width ?? 64, height: n.measured?.height ?? 64,
        }));
      if (rects.length < 2) return;
      history.push({ nodes: [...allNodes], edges: [...allEdges] });
      const aligned = alignNodes(rects, direction);
      updateNodes(aligned.map((r) => ({ id: r.id, update: { position: { x: r.x, y: r.y } } })));
    },
    [history, updateNodes],
  );

  const handleDistribute = useCallback(
    (direction: DistributeDirection) => {
      const { nodes: allNodes, edges: allEdges, selectedNodeIds: ids } = useCanvasStore.getState();
      const rects: NodeRect[] = allNodes
        .filter((n) => ids.has(n.id))
        .map((n) => ({
          id: n.id, x: n.position.x, y: n.position.y,
          width: n.measured?.width ?? 64, height: n.measured?.height ?? 64,
        }));
      if (rects.length < 3) return;
      history.push({ nodes: [...allNodes], edges: [...allEdges] });
      const distributed = distributeNodes(rects, direction);
      updateNodes(distributed.map((r) => ({ id: r.id, update: { position: { x: r.x, y: r.y } } })));
    },
    [history, updateNodes],
  );

  if (count < 2) return null;

  return (
    <div className={cn("absolute top-[72px] left-1/2 -translate-x-1/2 z-10 animate-fade-in-up", className)}>
      <div className="flex items-center gap-0.5 px-1.5 py-1 rounded-xl backdrop-blur-xl shadow-lg bg-orbflow-glass-bg border border-orbflow-border">
        {ALIGN_ACTIONS.map((action) => (
          <Tooltip key={action.direction} content={action.label} side="bottom">
            <button
              onClick={() => handleAlign(action.direction)}
              className="flex items-center justify-center w-7 h-7 rounded-lg text-orbflow-text-muted
                hover:bg-orbflow-controls-btn-hover active:brightness-90 transition-all duration-150
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
              aria-label={action.label}
            >
              <NodeIcon name={action.icon} className="w-3.5 h-3.5" />
            </button>
          </Tooltip>
        ))}

        <div className="w-px h-4 mx-0.5 bg-orbflow-border" />

        {DISTRIBUTE_ACTIONS.map((action) => (
          <Tooltip key={action.direction} content={action.label} side="bottom">
            <button
              onClick={() => handleDistribute(action.direction)}
              disabled={count < 3}
              className={cn(
                "flex items-center justify-center w-7 h-7 rounded-lg transition-all duration-150",
                "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                count < 3
                  ? "opacity-25 cursor-not-allowed text-orbflow-text-muted"
                  : "text-orbflow-text-muted hover:bg-orbflow-controls-btn-hover active:brightness-90"
              )}
              aria-label={action.label}
            >
              <NodeIcon name={action.icon} className="w-3.5 h-3.5" />
            </button>
          </Tooltip>
        ))}
      </div>
    </div>
  );
}
