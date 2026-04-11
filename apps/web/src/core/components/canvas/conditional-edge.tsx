"use client";

import { memo, useState, useCallback, useRef } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
  type EdgeProps,
} from "@xyflow/react";
import { useCanvasStore, useExecutionOverlayStore, useHistoryStore, usePickerStore, useToastStore } from "@orbflow/core/stores";
import { cn } from "../../utils/cn";
import { NodeIcon } from "../icons";

function ConditionalEdgeInner({
  id,
  source,
  target,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  selected,
  data,
}: EdgeProps) {
  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  const { selectEdge } = useCanvasStore();
  const [hovered, setHovered] = useState(false);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout>>(null);

  const showHover = useCallback(() => {
    if (hideTimerRef.current) clearTimeout(hideTimerRef.current);
    setHovered(true);
  }, []);

  const hideHover = useCallback(() => {
    hideTimerRef.current = setTimeout(() => setHovered(false), 150);
  }, []);

  // -- Execution overlay ----------------------
  const isLive = useExecutionOverlayStore((s) => s.isLive);
  const sourceExecStatus = useExecutionOverlayStore(
    (s) => s.isLive ? s.nodeStatuses[source]?.status : undefined
  );
  const targetExecStatus = useExecutionOverlayStore(
    (s) => s.isLive ? s.nodeStatuses[target]?.status : undefined
  );

  // Derive edge execution state
  const execActive = isLive && sourceExecStatus === "completed" &&
    (targetExecStatus === "running" || targetExecStatus === "queued");
  const execCompleted = isLive && sourceExecStatus === "completed" &&
    targetExecStatus === "completed";
  const execFailed = isLive && targetExecStatus === "failed";

  const conditionLabel = (data?.conditionLabel as string) || "";

  const edgeColor = selected
    ? "var(--orbflow-edge-selected)"
    : execActive
      ? "var(--orbflow-exec-active)"
      : execCompleted
        ? "var(--orbflow-exec-completed)"
        : execFailed
          ? "var(--orbflow-exec-failed)"
          : hovered
            ? "var(--orbflow-edge-hover)"
            : "var(--orbflow-edge-color)";

  const edgeStrokeWidth = selected ? 2.5 : execActive ? 2.5 : hovered ? 2 : 1.5;
  const edgeDashArray = execFailed ? "5,5" : undefined;

  const handleDelete = useCallback(() => {
    const { nodes, edges, setEdges } = useCanvasStore.getState();
    const history = useHistoryStore.getState();
    history.push({ nodes: [...nodes], edges: [...edges] });
    setEdges(edges.filter((e) => e.id !== id));
    useToastStore.getState().info("Connection removed");
  }, [id]);

  const handleAdd = useCallback(() => {
    const picker = usePickerStore.getState();
    // Convert flow coordinates to screen coordinates via the viewport transform
    // The ReactFlow container provides the transform context
    const rfContainer = document.querySelector(".react-flow") as HTMLElement | null;
    if (rfContainer) {
      const rect = rfContainer.getBoundingClientRect();
      // labelX/labelY are in flow coordinates; EdgeLabelRenderer handles positioning
      // For the picker, we use the center of the viewport as a reliable position
      picker.openPicker(
        { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 - 192 },
        undefined,
        id
      );
    } else {
      picker.openPicker(
        { x: window.innerWidth / 2 - 144, y: window.innerHeight / 2 - 192 },
        undefined,
        id
      );
    }
  }, [id]);

  return (
    <>
      {/* Invisible wider hit area for hover detection */}
      <path
        d={edgePath}
        fill="none"
        stroke="transparent"
        strokeWidth={20}
        style={{ pointerEvents: "auto", cursor: "pointer" }}
        onMouseEnter={showHover}
        onMouseLeave={hideHover}
        onClick={() => selectEdge(id)}
      />

      <BaseEdge
        id={id}
        path={edgePath}
        style={{
          stroke: edgeColor,
          strokeWidth: edgeStrokeWidth,
          strokeDasharray: edgeDashArray,
          transition: "stroke 0.3s, stroke-width 0.3s",
        }}
      />

      {/* Animated flow indicator when selected */}
      {selected && (
        <path
          d={edgePath}
          fill="none"
          stroke="var(--orbflow-edge-selected)"
          strokeWidth={2}
          strokeDasharray="6 4"
          style={{
            animation: "flow-dash 1s linear infinite",
          }}
        />
      )}

      {/* Execution particles for active edges */}
      {execActive && (
        <>
          <circle r="4" fill="var(--orbflow-exec-active)" opacity="0.9">
            <animateMotion
              dur="1.5s"
              repeatCount="indefinite"
              path={edgePath}
            />
          </circle>
          <circle r="3" fill="var(--orbflow-exec-active)" opacity="0.5">
            <animateMotion
              dur="1.5s"
              begin="0.75s"
              repeatCount="indefinite"
              path={edgePath}
            />
          </circle>
        </>
      )}

      {/* Condition label -- shown when condition is set */}
      {conditionLabel && (
        <EdgeLabelRenderer>
          <div
            className={cn(
              "nodrag nopan absolute pointer-events-auto px-2 py-0.5 rounded-md text-micro font-mono border backdrop-blur-md transition-all duration-200",
              !selected && "bg-orbflow-glass-bg border-orbflow-border text-orbflow-text-muted"
            )}
            style={{
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              ...(selected ? {
                backgroundColor: "rgba(124, 92, 252, 0.12)",
                borderColor: "rgba(124, 92, 252, 0.25)",
                color: "rgba(124, 92, 252, 0.9)",
              } : {}),
            }}
            title={conditionLabel}
          >
            {conditionLabel.length > 30
              ? conditionLabel.slice(0, 30) + "\u2026"
              : conditionLabel}
          </div>
        </EdgeLabelRenderer>
      )}

      {/* Hover menu -- "+" and delete buttons */}
      {(hovered || selected) && (
        <EdgeLabelRenderer>
          <div
            className="nodrag nopan absolute pointer-events-auto"
            style={{
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              animation: "edgeMenuIn 0.15s ease both",
            }}
            onMouseEnter={showHover}
            onMouseLeave={hideHover}
          >
            <div className="flex items-center gap-1.5" style={{ marginTop: conditionLabel ? 16 : 0 }}>
              {/* Add node button */}
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleAdd();
                }}
                className="w-7 h-7 rounded-full flex items-center justify-center
                  backdrop-blur-md hover:text-electric-indigo hover:border-electric-indigo/30
                  transition-all duration-150 hover:brightness-125 hover:shadow-md
                  bg-orbflow-glass-bg border border-orbflow-border-hover text-orbflow-text-muted"
                title="Insert step"
              >
                <NodeIcon name="plus" className="w-3.5 h-3.5" />
              </button>

              {/* Delete edge button */}
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleDelete();
                }}
                className="w-7 h-7 rounded-full flex items-center justify-center
                  backdrop-blur-md hover:text-rose-400 hover:border-rose-400/30
                  transition-all duration-150 hover:brightness-125 hover:shadow-md
                  bg-orbflow-glass-bg border border-orbflow-border-hover text-orbflow-text-muted"
                title="Remove connection"
              >
                <NodeIcon name="trash" className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const ConditionalEdge = memo(ConditionalEdgeInner);
