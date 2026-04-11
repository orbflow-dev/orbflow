"use client";

import { type ReactElement, useCallback, useEffect, useRef, useState } from "react";
import { Panel } from "@xyflow/react";
import { NodeIcon } from "../../icons";
import type { LayoutAlgorithm, LayoutDirection } from "@orbflow/core/utils";

export type { LayoutDirection };

type LayoutControlsProps = {
  direction: LayoutDirection;
  algorithm: LayoutAlgorithm;
  onLayout: () => void;
  onToggleDirection: () => void;
  onAlgorithmChange: (algorithm: LayoutAlgorithm) => void;
};

const ALGORITHM_LABELS: Record<LayoutAlgorithm, string> = {
  auto: "Auto",
  dagre: "Dagre",
  compact: "Compact",
};

const ALGORITHM_ORDER: LayoutAlgorithm[] = ["auto", "dagre", "compact"];

export function LayoutControls({
  direction,
  algorithm,
  onLayout,
  onToggleDirection,
  onAlgorithmChange,
}: LayoutControlsProps): ReactElement {
  const [showAlgoPicker, setShowAlgoPicker] = useState(false);
  const pickerRef = useRef<HTMLDivElement>(null);

  // Close dropdown on outside click
  useEffect(() => {
    if (!showAlgoPicker) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as HTMLElement)) {
        setShowAlgoPicker(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [showAlgoPicker]);

  // onLayout (handleAutoLayout) already handles animation + fitView,
  // so these handlers just trigger direction/algorithm changes then re-layout.
  const handleToggleDirection = useCallback(() => {
    onToggleDirection();
    // Re-layout after direction state settles
    requestAnimationFrame(() => {
      onLayout();
    });
  }, [onLayout, onToggleDirection]);

  const handleAlgorithmSelect = useCallback(
    (algo: LayoutAlgorithm) => {
      onAlgorithmChange(algo);
      setShowAlgoPicker(false);
      // Re-layout after algorithm state settles
      requestAnimationFrame(() => {
        onLayout();
      });
    },
    [onAlgorithmChange, onLayout],
  );

  return (
    <Panel position="top-left" className="m-3 flex items-center gap-2">
      <button
        type="button"
        onClick={onLayout}
        className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium
          bg-orbflow-glass-bg border border-orbflow-border text-orbflow-text-muted
          hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary hover:border-orbflow-border-hover
          transition-all duration-150 backdrop-blur-md"
      >
        <NodeIcon name="auto-layout" className="w-3.5 h-3.5" />
        Auto layout
      </button>

      <button
        type="button"
        onClick={handleToggleDirection}
        className="flex items-center gap-1 px-2 py-1.5 rounded-lg text-xs font-mono font-medium
          bg-orbflow-glass-bg border border-orbflow-border text-orbflow-text-muted
          hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary hover:border-orbflow-border-hover
          transition-all duration-150 backdrop-blur-md"
        title={direction === "LR" ? "Left to Right" : "Top to Bottom"}
      >
        {direction}
      </button>

      {/* Algorithm picker */}
      <div className="relative" ref={pickerRef}>
        <button
          type="button"
          onClick={() => setShowAlgoPicker((v) => !v)}
          className="flex items-center gap-1 px-2 py-1.5 rounded-lg text-xs font-medium
            bg-orbflow-glass-bg border border-orbflow-border text-orbflow-text-muted
            hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary hover:border-orbflow-border-hover
            transition-all duration-150 backdrop-blur-md"
          title="Layout algorithm"
        >
          {ALGORITHM_LABELS[algorithm]}
          <svg
            className="w-3 h-3 ml-0.5"
            viewBox="0 0 12 12"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
          >
            <path d="M3 5L6 8L9 5" />
          </svg>
        </button>

        {showAlgoPicker && (
          <div
            className="absolute top-full left-0 mt-1 min-w-[120px] rounded-lg
              bg-orbflow-glass-bg border border-orbflow-border backdrop-blur-md
              shadow-lg z-50 py-1"
          >
            {ALGORITHM_ORDER.map((algo) => (
              <button
                key={algo}
                type="button"
                onClick={() => handleAlgorithmSelect(algo)}
                className={`w-full text-left px-3 py-1.5 text-xs font-medium
                  transition-colors duration-100
                  ${
                    algo === algorithm
                      ? "text-orbflow-text-secondary bg-orbflow-surface-hover"
                      : "text-orbflow-text-muted hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary"
                  }`}
              >
                {ALGORITHM_LABELS[algo]}
              </button>
            ))}
          </div>
        )}
      </div>
    </Panel>
  );
}
