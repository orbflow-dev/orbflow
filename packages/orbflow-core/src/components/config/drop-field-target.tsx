"use client";

import { useState, useCallback, type ReactNode } from "react";

export interface DropFieldData {
  nodeId: string;
  path: string;
  celPath: string;
}

export interface DropFieldTargetProps {
  onDrop: (data: DropFieldData) => void;
  children: ReactNode | ((data: { isDragOver: boolean }) => ReactNode);
}

/**
 * Headless drop target wrapper for field mapping drag-and-drop.
 * Accepts drops with the `application/orbflow-field` MIME type.
 */
export function DropFieldTarget({
  onDrop,
  children,
}: DropFieldTargetProps): ReactNode {
  const [isDragOver, setIsDragOver] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    if (e.dataTransfer.types.includes("application/orbflow-field")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      setIsDragOver(true);
    }
  }, []);

  const handleDragLeave = useCallback(() => {
    setIsDragOver(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setIsDragOver(false);
      const raw = e.dataTransfer.getData("application/orbflow-field");
      if (!raw) return;
      try {
        const data = JSON.parse(raw) as DropFieldData;
        onDrop(data);
      } catch {
        // Ignore invalid JSON payloads
      }
    },
    [onDrop]
  );

  const content =
    typeof children === "function" ? children({ isDragOver }) : children;

  return (
    <div
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {content}
    </div>
  );
}
