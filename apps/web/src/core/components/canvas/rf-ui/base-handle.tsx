"use client";

import type { ComponentProps, ReactElement } from "react";
import { Handle, type HandleProps } from "@xyflow/react";
import { cn } from "../../../utils/cn";

export type BaseHandleProps = HandleProps;

export function BaseHandle({
  className,
  children,
  ...props
}: ComponentProps<typeof Handle>): ReactElement {
  return (
    <Handle
      {...props}
      className={cn(
        "h-[12px] w-[12px] rounded-full border border-orbflow-border/80 bg-orbflow-surface",
        "shadow-[0_0_0_1px_var(--orbflow-bg)]",
        "transition-all hover:scale-110 hover:border-electric-indigo/70 hover:bg-electric-indigo/20",
        className,
      )}
    >
      {children}
    </Handle>
  );
}
