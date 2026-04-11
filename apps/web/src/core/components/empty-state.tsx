"use client";

import { cn } from "../utils/cn";
import { NodeIcon } from "./icons";
import { Button } from "./button";

interface EmptyStateProps {
  icon: string;
  title: string;
  description: string;
  action?: {
    label: string;
    onClick: () => void;
  };
  className?: string;
}

export function EmptyState({ icon, title, description, action, className }: EmptyStateProps) {
  return (
    <div className={cn("flex flex-col items-center justify-center text-center px-6 py-12 animate-fade-in", className)}>
      <div className="w-11 h-11 rounded-2xl bg-electric-indigo/10 flex items-center justify-center mb-4 animate-fade-in-up stagger-1">
        <NodeIcon name={icon} className="w-5 h-5 text-electric-indigo/60" />
      </div>
      <h3 className="text-body-lg font-semibold text-orbflow-text-secondary mb-1">{title}</h3>
      <p className="text-body text-orbflow-text-faint max-w-[240px]">{description}</p>
      {action && (
        <Button variant="primary" size="sm" onClick={action.onClick} className="mt-4">
          {action.label}
        </Button>
      )}
    </div>
  );
}
