"use client";

import { useState, useCallback, type ReactNode } from "react";

export interface DragFieldData {
  nodeId: string;
  path: string;
  celPath: string;
}

export interface DragFieldSourceProps {
  nodeId: string;
  path: string;
  celPath: string;
  children: ReactNode | ((data: { isDragging: boolean }) => ReactNode);
}

/**
 * Headless draggable wrapper for field mapping drag-and-drop.
 * Sets `application/orbflow-field` MIME type with a JSON payload on drag start.
 */
export function DragFieldSource({
  nodeId,
  path,
  celPath,
  children,
}: DragFieldSourceProps): ReactNode {
  const [isDragging, setIsDragging] = useState(false);

  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      const payload: DragFieldData = { nodeId, path, celPath };
      e.dataTransfer.setData(
        "application/orbflow-field",
        JSON.stringify(payload)
      );
      e.dataTransfer.effectAllowed = "copy";
      setIsDragging(true);
    },
    [nodeId, path, celPath]
  );

  const handleDragEnd = useCallback(() => {
    setIsDragging(false);
  }, []);

  const content =
    typeof children === "function" ? children({ isDragging }) : children;

  return (
    <div
      draggable="true"
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
    >
      {content}
    </div>
  );
}
