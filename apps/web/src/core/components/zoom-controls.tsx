"use client";

import { useState, useCallback, useEffect } from "react";
import { cn } from "../utils/cn";
import { NodeIcon } from "./icons";
import { Tooltip } from "./tooltip";

const ZOOM_PRESETS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2];

interface ZoomControlsProps {
  zoom: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomTo: (level: number) => void;
  onFitView: () => void;
}

export function ZoomControls({ zoom, onZoomIn, onZoomOut, onZoomTo, onFitView }: ZoomControlsProps) {
  const [showPresets, setShowPresets] = useState(false);

  const zoomPercent = Math.round(zoom * 100);

  // Close dropdown on outside click
  useEffect(() => {
    if (!showPresets) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest("[data-zoom-controls]")) {
        setShowPresets(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showPresets]);

  const handleZoomTo = useCallback(
    (level: number) => {
      onZoomTo(level);
      setShowPresets(false);
    },
    [onZoomTo],
  );

  return (
    <div
      data-zoom-controls
      className="absolute bottom-6 right-6 z-10 flex items-center gap-1"
    >
      <div className="flex items-center rounded-xl backdrop-blur-xl shadow-lg bg-orbflow-glass-bg border border-orbflow-border overflow-hidden">
        <Tooltip content="Zoom out" side="top">
          <button
            onClick={onZoomOut}
            className="flex items-center justify-center w-8 h-8 text-orbflow-text-muted
              hover:bg-orbflow-controls-btn-hover active:brightness-90 transition-all
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            aria-label="Zoom out"
          >
            <NodeIcon name="minus" className="w-3.5 h-3.5" />
          </button>
        </Tooltip>

        <div className="relative">
          <button
            onClick={() => setShowPresets((s) => !s)}
            className="flex items-center justify-center min-w-[52px] h-8 px-1.5
              text-body-sm font-mono text-orbflow-text-secondary
              hover:bg-orbflow-controls-btn-hover transition-colors
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            aria-label="Zoom level"
            aria-expanded={showPresets}
            aria-haspopup="true"
          >
            {zoomPercent}%
          </button>

          {showPresets && (
            <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1.5
              min-w-[72px] rounded-xl border border-orbflow-border bg-orbflow-surface shadow-xl py-1
              animate-scale-in">
              {ZOOM_PRESETS.map((preset) => (
                <button
                  key={preset}
                  onClick={() => handleZoomTo(preset)}
                  className={cn(
                    "w-full px-3 py-1.5 text-body-sm font-mono text-center transition-colors",
                    "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
                    Math.abs(zoom - preset) < 0.01
                      ? "text-electric-indigo bg-electric-indigo/10"
                      : "text-orbflow-text-secondary hover:bg-orbflow-surface-hover"
                  )}
                >
                  {Math.round(preset * 100)}%
                </button>
              ))}
            </div>
          )}
        </div>

        <Tooltip content="Zoom in" side="top">
          <button
            onClick={onZoomIn}
            className="flex items-center justify-center w-8 h-8 text-orbflow-text-muted
              hover:bg-orbflow-controls-btn-hover active:brightness-90 transition-all
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            aria-label="Zoom in"
          >
            <NodeIcon name="plus" className="w-3.5 h-3.5" />
          </button>
        </Tooltip>

        <div className="w-px h-4 bg-orbflow-border" />

        <Tooltip content="Fit to view" side="top">
          <button
            onClick={onFitView}
            className="flex items-center justify-center w-8 h-8 text-orbflow-text-muted
              hover:bg-orbflow-controls-btn-hover active:brightness-90 transition-all
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            aria-label="Fit to view"
          >
            <NodeIcon name="zoom-fit" className="w-3.5 h-3.5" />
          </button>
        </Tooltip>
      </div>
    </div>
  );
}
