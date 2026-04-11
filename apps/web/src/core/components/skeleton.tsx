"use client";

import { cn } from "../utils/cn";

interface SkeletonProps {
  className?: string;
}

function Skeleton({ className }: SkeletonProps) {
  return (
    <div
      className={cn(
        "relative overflow-hidden rounded border border-orbflow-border/30 bg-orbflow-surface-hover/80",
        className
      )}
    >
      <div className="absolute inset-0 animate-shimmer bg-gradient-to-r from-transparent via-orbflow-border-hover/70 to-transparent" />
    </div>
  );
}

interface SkeletonRowProps {
  widths?: string[];
  className?: string;
}

export function SkeletonRow({ widths = ["w-24", "w-16"], className }: SkeletonRowProps) {
  return (
    <div className={cn("flex items-center gap-3 px-3 py-3", className)}>
      <Skeleton className="h-8 w-8 rounded-lg shrink-0" />
      <div className="flex-1 space-y-1.5">
        <Skeleton className={cn("h-2.5 rounded-full", widths[0])} />
        <Skeleton className={cn("h-2 rounded-full", widths[1])} />
      </div>
    </div>
  );
}

export function SkeletonCard({ className }: SkeletonProps) {
  return (
    <div className={cn("rounded-xl border border-orbflow-border p-3.5", className)}>
      <div className="flex justify-between mb-2">
        <Skeleton className="h-2.5 w-16 rounded-full" />
        <Skeleton className="h-2.5 w-10 rounded-full" />
      </div>
      <Skeleton className="h-3.5 w-36 rounded mb-2" />
      <div className="flex justify-between">
        <Skeleton className="h-2.5 w-16 rounded-full" />
        <Skeleton className="h-2.5 w-8 rounded-full" />
      </div>
      <Skeleton className="mt-2 h-[3px] w-full rounded-full" />
    </div>
  );
}
