"use client";

import { useState, useRef, useCallback, useEffect, useId, type ReactNode } from "react";
import { cn } from "../utils/cn";

type TooltipSide = "top" | "bottom" | "left" | "right";

interface TooltipProps {
  content: string;
  side?: TooltipSide;
  delay?: number;
  children: ReactNode;
  className?: string;
}

const sideStyles: Record<TooltipSide, string> = {
  top: "bottom-full left-1/2 -translate-x-1/2 mb-1.5",
  bottom: "top-full left-1/2 -translate-x-1/2 mt-1.5",
  left: "right-full top-1/2 -translate-y-1/2 mr-1.5",
  right: "left-full top-1/2 -translate-y-1/2 ml-1.5",
};

const arrowStyles: Record<TooltipSide, string> = {
  top: "top-full left-1/2 -translate-x-1/2 border-t-orbflow-elevated border-x-transparent border-b-transparent",
  bottom: "bottom-full left-1/2 -translate-x-1/2 border-b-orbflow-elevated border-x-transparent border-t-transparent",
  left: "left-full top-1/2 -translate-y-1/2 border-l-orbflow-elevated border-y-transparent border-r-transparent",
  right: "right-full top-1/2 -translate-y-1/2 border-r-orbflow-elevated border-y-transparent border-l-transparent",
};

export function Tooltip({ content, side = "top", delay = 300, children, className }: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const tooltipId = useId();

  const show = useCallback(() => {
    timerRef.current = setTimeout(() => setVisible(true), delay);
  }, [delay]);

  const hide = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setVisible(false);
  }, []);

  useEffect(() => () => { if (timerRef.current) clearTimeout(timerRef.current); }, []);

  return (
    <div
      className={cn("relative inline-flex", className)}
      aria-describedby={visible ? tooltipId : undefined}
      onMouseEnter={show}
      onMouseLeave={hide}
      onFocus={show}
      onBlur={hide}
    >
      {children}
      {visible && (
        <div
          id={tooltipId}
          role="tooltip"
          className={cn(
            "absolute z-50 pointer-events-none",
            "px-2 py-1 rounded-md",
            "bg-orbflow-elevated border border-orbflow-border shadow-lg",
            "text-caption text-orbflow-text-secondary font-medium whitespace-nowrap",
            "animate-fade-in motion-reduce:animate-none",
            sideStyles[side],
          )}
        >
          {content}
          <span className={cn("absolute border-[4px]", arrowStyles[side])} />
        </div>
      )}
    </div>
  );
}
