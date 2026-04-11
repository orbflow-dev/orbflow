"use client";

import type { ComponentProps, ReactElement } from "react";
import { cn } from "../../../utils/cn";

export function BaseNode({
  className,
  ...props
}: ComponentProps<"div">): ReactElement {
  return (
    <div
      className={cn(
        "bg-orbflow-node-bg text-orbflow-text-secondary relative rounded-md border border-orbflow-node-border transition-[border-color,box-shadow] duration-150",
        "hover:border-electric-indigo/40 hover:ring-1 hover:ring-electric-indigo/20 hover:shadow-sm",
        "[.react-flow__node.selected_&]:border-electric-indigo/45",
        "[.react-flow__node.selected_&]:ring-1 [.react-flow__node.selected_&]:ring-electric-indigo/25",
        "[.react-flow__node.selected_&]:shadow-md",
        className,
      )}
      tabIndex={0}
      {...props}
    />
  );
}

export function BaseNodeHeader({
  className,
  ...props
}: ComponentProps<"header">): ReactElement {
  return (
    <header
      {...props}
      className={cn(
        "mx-0 my-0 -mb-1 flex flex-row items-center justify-between gap-2 px-3 py-2",
        className,
      )}
    />
  );
}

export function BaseNodeHeaderTitle({
  className,
  ...props
}: ComponentProps<"h3">): ReactElement {
  return (
    <h3
      data-slot="base-node-title"
      className={cn("select-none flex-1 font-semibold", className)}
      {...props}
    />
  );
}

export function BaseNodeContent({
  className,
  ...props
}: ComponentProps<"div">): ReactElement {
  return (
    <div
      data-slot="base-node-content"
      className={cn("flex flex-col gap-y-2 p-3", className)}
      {...props}
    />
  );
}

export function BaseNodeFooter({
  className,
  ...props
}: ComponentProps<"div">): ReactElement {
  return (
    <div
      data-slot="base-node-footer"
      className={cn(
        "flex flex-col items-center gap-y-2 border-t border-orbflow-border px-3 pt-2 pb-3",
        className,
      )}
      {...props}
    />
  );
}
