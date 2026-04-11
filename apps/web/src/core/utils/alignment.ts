/** Pure alignment & distribution utilities for canvas nodes. */

export interface NodeRect {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export type AlignDirection = "left" | "center" | "right" | "top" | "middle" | "bottom";
export type DistributeDirection = "horizontal" | "vertical";

/**
 * Align nodes along one axis. Returns new positions (immutable).
 * Nodes must have at least 2 elements.
 */
export function alignNodes(nodes: NodeRect[], direction: AlignDirection): NodeRect[] {
  if (nodes.length < 2) return nodes;

  switch (direction) {
    case "left": {
      const minX = Math.min(...nodes.map((n) => n.x));
      return nodes.map((n) => ({ ...n, x: minX }));
    }
    case "center": {
      const centers = nodes.map((n) => n.x + n.width / 2);
      const avg = centers.reduce((a, b) => a + b, 0) / centers.length;
      return nodes.map((n) => ({ ...n, x: avg - n.width / 2 }));
    }
    case "right": {
      const maxRight = Math.max(...nodes.map((n) => n.x + n.width));
      return nodes.map((n) => ({ ...n, x: maxRight - n.width }));
    }
    case "top": {
      const minY = Math.min(...nodes.map((n) => n.y));
      return nodes.map((n) => ({ ...n, y: minY }));
    }
    case "middle": {
      const middles = nodes.map((n) => n.y + n.height / 2);
      const avg = middles.reduce((a, b) => a + b, 0) / middles.length;
      return nodes.map((n) => ({ ...n, y: avg - n.height / 2 }));
    }
    case "bottom": {
      const maxBottom = Math.max(...nodes.map((n) => n.y + n.height));
      return nodes.map((n) => ({ ...n, y: maxBottom - n.height }));
    }
    default:
      return nodes;
  }
}

/**
 * Distribute nodes evenly along an axis. Returns new positions (immutable).
 * Needs at least 3 nodes to distribute meaningfully.
 */
export function distributeNodes(nodes: NodeRect[], direction: DistributeDirection): NodeRect[] {
  if (nodes.length < 3) return nodes;

  if (direction === "horizontal") {
    const sorted = [...nodes].sort((a, b) => a.x - b.x);
    const first = sorted[0];
    const last = sorted[sorted.length - 1];
    const totalWidth = sorted.reduce((sum, n) => sum + n.width, 0);
    const totalSpace = (last.x + last.width) - first.x - totalWidth;
    // Clamp to 0 -- when nodes overlap, they pack tightly from the leftmost position
    const gap = Math.max(0, totalSpace / (sorted.length - 1));

    let currentX = first.x;
    return sorted.map((n) => {
      const result = { ...n, x: currentX };
      currentX += n.width + gap;
      return result;
    });
  }

  // vertical
  const sorted = [...nodes].sort((a, b) => a.y - b.y);
  const first = sorted[0];
  const last = sorted[sorted.length - 1];
  const totalHeight = sorted.reduce((sum, n) => sum + n.height, 0);
  const totalSpace = (last.y + last.height) - first.y - totalHeight;
  const gap = Math.max(0, totalSpace / (sorted.length - 1));

  let currentY = first.y;
  return sorted.map((n) => {
    const result = { ...n, y: currentY };
    currentY += n.height + gap;
    return result;
  });
}
