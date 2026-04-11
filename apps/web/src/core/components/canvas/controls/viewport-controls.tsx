"use client";

import { type ReactElement, useCallback } from "react";
import { Panel, useReactFlow } from "@xyflow/react";
import { NodeIcon } from "../../icons";

type ViewportControlsProps = {
  interactive: boolean;
  onToggleInteractive: () => void;
};

const BTN_CLASS =
  "w-8 h-8 rounded-lg flex items-center justify-center backdrop-blur-md " +
  "bg-orbflow-glass-bg border border-orbflow-border text-orbflow-text-muted " +
  "hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary hover:border-orbflow-border-hover " +
  "transition-all duration-150 disabled:opacity-40 disabled:pointer-events-none";

export function ViewportControls({
  interactive,
  onToggleInteractive,
}: ViewportControlsProps): ReactElement {
  const { zoomIn, zoomOut, fitView } = useReactFlow();

  const handleZoomIn = useCallback(() => {
    zoomIn({ duration: 150 });
  }, [zoomIn]);

  const handleZoomOut = useCallback(() => {
    zoomOut({ duration: 150 });
  }, [zoomOut]);

  const handleFitView = useCallback(() => {
    fitView({ padding: 0.12, maxZoom: 1.1, duration: 340 });
  }, [fitView]);

  return (
    <Panel position="bottom-left" className="m-3 flex items-center gap-1.5">
      <button
        type="button"
        className={BTN_CLASS}
        onClick={handleZoomIn}
        aria-label="Zoom in"
      >
        <NodeIcon name="plus" className="size-4" />
      </button>
      <button
        type="button"
        className={BTN_CLASS}
        onClick={handleZoomOut}
        aria-label="Zoom out"
      >
        <NodeIcon name="minus" className="size-4" />
      </button>
      <button
        type="button"
        className={BTN_CLASS}
        onClick={handleFitView}
        aria-label="Fit view"
      >
        <NodeIcon name="maximize" className="size-4" />
      </button>
      <button
        type="button"
        className={BTN_CLASS}
        onClick={onToggleInteractive}
        aria-label={interactive ? "Lock canvas" : "Unlock canvas"}
      >
        <NodeIcon
          name={interactive ? "unlock" : "lock"}
          className="size-4"
        />
      </button>
    </Panel>
  );
}
