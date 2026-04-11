import { useMemo } from "react";
import { EdgeLabelRenderer } from "@xyflow/react";

export interface EdgeConditionLabelRenderData {
  truncatedLabel: string;
  fullLabel: string;
}

export interface EdgeConditionLabelProps {
  label: string;
  x: number;
  y: number;
  maxLength?: number;
  children: (data: EdgeConditionLabelRenderData) => React.ReactNode;
}

/** Headless edge condition label positioned at the edge midpoint. Returns null if label is empty. */
export function EdgeConditionLabel({
  label,
  x,
  y,
  maxLength = 30,
  children,
}: EdgeConditionLabelProps): React.ReactNode {
  const renderData = useMemo((): EdgeConditionLabelRenderData => {
    const truncatedLabel =
      label.length > maxLength
        ? label.slice(0, maxLength) + "\u2026"
        : label;
    return { truncatedLabel, fullLabel: label };
  }, [label, maxLength]);

  if (!label) return null;

  return (
    <EdgeLabelRenderer>
      <div
        style={{
          position: "absolute",
          transform: `translate(-50%, -50%) translate(${x}px, ${y}px)`,
          pointerEvents: "all",
        }}
      >
        {children(renderData)}
      </div>
    </EdgeLabelRenderer>
  );
}
