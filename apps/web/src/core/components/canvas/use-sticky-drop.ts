"use client";

import { useCallback, useRef } from "react";
import type { Node } from "@xyflow/react";
import { useCanvasStore } from "@orbflow/core/stores";

interface StickyHit {
  id: string;
  position: { x: number; y: number };
  width: number;
  height: number;
}

function getAbsolutePosition(node: Node, allNodes: Node[]): { x: number; y: number } {
  let x = node.position.x;
  let y = node.position.y;
  if (node.parentId) {
    const parent = allNodes.find((n) => n.id === node.parentId);
    if (parent) {
      x += parent.position.x;
      y += parent.position.y;
    }
  }
  return { x, y };
}

function findStickyAt(nodeId: string, allNodes: Node[]): StickyHit | null {
  const node = allNodes.find((n) => n.id === nodeId);
  if (!node) return null;

  const abs = getAbsolutePosition(node, allNodes);
  const nodeW = node.measured?.width ?? 64;
  const nodeH = node.measured?.height ?? 64;
  const cx = abs.x + nodeW / 2;
  const cy = abs.y + nodeH / 2;

  const sticky = allNodes
    .filter((n) => n.type === "stickyNote")
    .find((s) => {
      const sw = (s.data?.width as number) ?? 200;
      const sh = (s.data?.height as number) ?? 140;
      return cx >= s.position.x && cx <= s.position.x + sw && cy >= s.position.y && cy <= s.position.y + sh;
    });

  if (!sticky) return null;
  return {
    id: sticky.id,
    position: sticky.position,
    width: (sticky.data?.width as number) ?? 200,
    height: (sticky.data?.height as number) ?? 140,
  };
}

export function useStickyDrop() {
  const hoveredStickyRef = useRef<string | null>(null);

  const onNodeDrag = useCallback(
    (_event: React.MouseEvent, draggedNode: Node) => {
      if (draggedNode.type === "stickyNote" || draggedNode.type === "textAnnotation") return;

      const { nodes: currentNodes, updateNodeData } = useCanvasStore.getState();
      const hitSticky = findStickyAt(draggedNode.id, currentNodes);
      const hitId = hitSticky?.id || null;

      if (hitId !== hoveredStickyRef.current) {
        if (hoveredStickyRef.current) {
          updateNodeData(hoveredStickyRef.current, { dropHighlight: false });
        }
        if (hitId) {
          updateNodeData(hitId, { dropHighlight: true });
        }
        hoveredStickyRef.current = hitId;
      }
    },
    [],
  );

  const onNodeDragStop = useCallback(
    (_event: React.MouseEvent, draggedNode: Node) => {
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

      const abs = getAbsolutePosition(freshNode, currentNodes);
      const nodeW = freshNode.measured?.width ?? 64;
      const nodeH = freshNode.measured?.height ?? 64;
      const cx = abs.x + nodeW / 2;
      const cy = abs.y + nodeH / 2;

      const hitSticky = stickyNodes.find((sticky) => {
        const sw = (sticky.data?.width as number) ?? 200;
        const sh = (sticky.data?.height as number) ?? 140;
        return cx >= sticky.position.x && cx <= sticky.position.x + sw && cy >= sticky.position.y && cy <= sticky.position.y + sh;
      });

      if (hitSticky && hitSticky.id !== freshNode.parentId) {
        updateNode(draggedNode.id, {
          parentId: hitSticky.id,
          extent: undefined,
          position: { x: abs.x - hitSticky.position.x, y: abs.y - hitSticky.position.y },
          zIndex: 1,
        });
        useCanvasStore.getState().updateNodeData(hitSticky.id, { justAttached: true });
        setTimeout(() => {
          useCanvasStore.getState().updateNodeData(hitSticky.id, { justAttached: false });
        }, 300);
      } else if (!hitSticky && freshNode.parentId) {
        const oldParentId = freshNode.parentId;
        updateNode(draggedNode.id, {
          parentId: undefined,
          extent: undefined,
          position: { x: abs.x, y: abs.y },
          zIndex: 0,
        });
        if (oldParentId) {
          useCanvasStore.getState().updateNodeData(oldParentId, { justDetached: true });
          setTimeout(() => {
            useCanvasStore.getState().updateNodeData(oldParentId, { justDetached: false });
          }, 250);
        }
      }
    },
    [],
  );

  return { onNodeDrag, onNodeDragStop };
}
