"use client";

import { memo, useState, useCallback, useRef, useEffect } from "react";
import { type NodeProps } from "@xyflow/react";
import { useCanvasStore } from "@orbflow/core/stores";
import { NodeIcon } from "../icons";

function TextAnnotationNodeInner({ id, data, selected }: NodeProps) {
  const { updateAnnotation, removeAnnotation } = useCanvasStore();
  const [editing, setEditing] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const content = (data?.content as string) || "";
  const annotationId = (data?.annotationId as string) || id;

  const handleDoubleClick = useCallback(() => {
    setEditing(true);
  }, []);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.setSelectionRange(
        inputRef.current.value.length,
        inputRef.current.value.length
      );
    }
  }, [editing]);

  const handleBlur = useCallback(() => {
    setEditing(false);
  }, []);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      updateAnnotation(annotationId, { content: e.target.value });
      useCanvasStore.getState().updateNodeData(id, { content: e.target.value });
    },
    [annotationId, id, updateAnnotation]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") setEditing(false);
    },
    []
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      removeAnnotation(annotationId);
      useCanvasStore.getState().removeNode(id);
    },
    [annotationId, id, removeAnnotation]
  );

  return (
    <div
      className={`group relative cursor-grab active:cursor-grabbing transition-all duration-150
        ${selected ? "ring-1 ring-electric-indigo/20 rounded-lg" : ""}`}
      onDoubleClick={handleDoubleClick}
      style={{ minWidth: 100, maxWidth: 400 }}
    >
      {/* Delete button */}
      <button
        onClick={handleDelete}
        className={`absolute -top-2 -right-2 ${selected ? "opacity-100" : "opacity-0 group-hover:opacity-100"}
          w-5 h-5 rounded-full bg-orbflow-surface border border-orbflow-border
          flex items-center justify-center text-orbflow-text-ghost hover:text-rose-400 hover:brightness-125
          transition-all duration-150 nodrag nopan z-10`}
        aria-label="Delete label"
        title="Delete label"
      >
        <NodeIcon name="x" className="w-2.5 h-2.5" />
      </button>

      {editing ? (
        <input
          ref={inputRef}
          type="text"
          value={content}
          onChange={handleChange}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
          className="bg-transparent outline-none text-title font-medium
            text-orbflow-text-secondary px-2 py-1 min-w-[100px] nodrag nopan nowheel
            border-b border-electric-indigo/30"
          placeholder="Type a label..."
          style={{ width: Math.max(100, content.length * 9 + 20) }}
        />
      ) : (
        <p
          className={`text-title font-medium px-2 py-1 whitespace-nowrap
            ${content ? "text-orbflow-text-secondary" : "text-orbflow-text-ghost"}`}
        >
          {content || "Double-click to edit..."}
        </p>
      )}
    </div>
  );
}

export const TextAnnotationNode = memo(TextAnnotationNodeInner);
