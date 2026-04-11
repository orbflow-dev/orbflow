"use client";

import { memo, useState, useCallback, useRef, useEffect } from "react";
import { NodeResizer, type NodeProps } from "@xyflow/react";
import { useCanvasStore, childCountForNode } from "@orbflow/core/stores";
import { useTheme } from "../../context/theme-provider";
import { NodeIcon } from "../icons";
import { cn } from "../../utils/cn";
import { LIGHT_COLORS, DARK_COLORS, COLOR_NAMES } from "./sticky-note-colors";

function StickyNoteNodeInner({ id, data, selected }: NodeProps) {
  const { updateAnnotation, updateAnnotationStyle, removeAnnotation } = useCanvasStore();
  const { mode } = useTheme();
  const [editing, setEditing] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const content = (data?.content as string) || "";
  const colorName = (data?.color as string) || "yellow";
  const annotationId = (data?.annotationId as string) || id;
  const dropHighlight = !!data?.dropHighlight;
  const justAttached = !!data?.justAttached;
  const justDetached = !!data?.justDetached;

  const COLORS = mode === "dark" ? DARK_COLORS : LIGHT_COLORS;
  const color = COLORS[colorName] || COLORS.yellow;

  const childCount = useCanvasStore((s) => childCountForNode(s, id));

  const handleDoubleClick = useCallback(() => {
    setEditing(true);
  }, []);

  useEffect(() => {
    if (editing && textareaRef.current) {
      textareaRef.current.focus();
      textareaRef.current.setSelectionRange(
        textareaRef.current.value.length,
        textareaRef.current.value.length
      );
    }
  }, [editing]);

  const handleBlur = useCallback(() => {
    setEditing(false);
  }, []);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      updateAnnotation(annotationId, { content: e.target.value });
      useCanvasStore.getState().updateNodeData(id, { content: e.target.value });
    },
    [annotationId, id, updateAnnotation]
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      removeAnnotation(annotationId);
      useCanvasStore.getState().removeNode(id);
    },
    [annotationId, id, removeAnnotation]
  );

  const handleColorChange = useCallback(
    (newColor: string) => {
      updateAnnotationStyle(annotationId, { color: newColor });
      useCanvasStore.getState().updateNodeData(id, { color: newColor });
    },
    [annotationId, id, updateAnnotationStyle]
  );

  const handleResizeEnd = useCallback(
    (_event: unknown, params: { width: number; height: number }) => {
      const { width: w, height: h } = params;
      useCanvasStore.getState().updateNodeData(id, { width: w, height: h });
      updateAnnotationStyle(annotationId, { width: w, height: h });
    },
    [id, annotationId, updateAnnotationStyle]
  );

  return (
    <div
      className={cn(
        "group relative w-full h-full flex flex-col overflow-hidden rounded-lg shadow-md",
        "animate-scale-in transition-all duration-200",
        dropHighlight
          ? "ring-2 ring-electric-indigo/60 scale-[1.02]"
          : selected
            ? "shadow-lg ring-2 ring-electric-indigo/30"
            : "hover:shadow-lg",
        justAttached && "animate-sticky-snap",
        justDetached && "animate-sticky-detach"
      )}
      style={{
        backgroundColor: color.bg,
        borderWidth: 1,
        borderColor: color.border,
        ...(dropHighlight ? {
          boxShadow: `0 0 16px 4px ${color.border}40, 0 0 6px 2px ${color.border}30`,
          animation: "stickyDropGlow 1.2s ease-in-out infinite",
          "--sticky-glow-color": `${color.border}35`,
        } : {}),
      }}
      onDoubleClick={handleDoubleClick}
    >
      {/* Resize handles -- visible when selected */}
      <NodeResizer
        color={color.border}
        isVisible={!!selected}
        minWidth={160}
        minHeight={100}
        onResizeEnd={handleResizeEnd}
      />

      {/* Drag handle bar */}
      <div
        className="flex items-center justify-between px-2.5 py-1.5 rounded-t-lg cursor-grab active:cursor-grabbing shrink-0"
        style={{ background: `linear-gradient(135deg, ${color.border}35 0%, ${color.border}18 100%)` }}
      >
        <div className="flex items-center gap-1.5">
          <NodeIcon
            name="sticky-note"
            className="w-3 h-3"
            style={{ color: color.text + "50" }}
          />
          {childCount > 0 && (
            <span
              className="animate-sticky-badge-pop inline-flex items-center px-1.5 py-px rounded-full text-micro font-medium"
              style={{
                backgroundColor: color.border + "30",
                color: color.text + "90",
              }}
            >
              {childCount} {childCount === 1 ? "item" : "items"}
            </span>
          )}
        </div>

        {/* Color swatches + delete -- top right, visible on hover/selection */}
        <div
          className={cn(
            "flex items-center gap-1 nodrag nopan transition-opacity duration-150",
            selected ? "opacity-100" : "opacity-0 group-hover:opacity-100"
          )}
        >
          {COLOR_NAMES.map((c) => {
            const isActive = colorName === c;
            return (
              <button
                key={c}
                tabIndex={selected ? 0 : -1}
                onClick={(e) => { e.stopPropagation(); handleColorChange(c); }}
                className={cn(
                  "w-2.5 h-2.5 rounded-full transition-all duration-150",
                  "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-white/40",
                  isActive
                    ? "ring-1 ring-white/50 scale-125"
                    : "opacity-50 hover:opacity-100 hover:scale-110"
                )}
                style={{ backgroundColor: COLORS[c].border }}
                aria-label={`Set color to ${c}`}
                aria-pressed={isActive}
                title={c}
              />
            );
          })}

          <div className="w-px h-3 mx-0.5" style={{ backgroundColor: color.text + "20" }} />

          <button
            onClick={handleDelete}
            className={cn(
              "w-4 h-4 flex items-center justify-center rounded transition-colors duration-150",
              "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-white/40",
              "hover:bg-black/10"
            )}
            style={{ color: color.text + "60" }}
            aria-label="Delete note"
            title="Delete note"
          >
            <NodeIcon name="x" className="w-2.5 h-2.5" />
          </button>
        </div>
      </div>

      {/* Content -- flex-1 fills remaining height, min-h-0 allows shrinking */}
      <div className="px-2.5 py-2 flex-1 min-h-0 overflow-y-auto nodrag nopan nowheel">
        {editing ? (
          <textarea
            ref={textareaRef}
            value={content}
            onChange={handleChange}
            onBlur={handleBlur}
            className="w-full h-full bg-transparent resize-none outline-none
              text-body-lg leading-relaxed"
            style={{ color: color.text }}
            placeholder="Type a note..."
          />
        ) : (
          <p
            className={cn(
              "text-body-lg leading-relaxed whitespace-pre-wrap break-words select-none",
              !content && "italic"
            )}
            style={{ color: content ? color.text : color.text + "40" }}
          >
            {content || "Double-click to edit…"}
          </p>
        )}
      </div>
    </div>
  );
}

export const StickyNoteNode = memo(StickyNoteNodeInner);
