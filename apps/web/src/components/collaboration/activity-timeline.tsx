"use client";

import { memo, useMemo } from "react";
import type { ChangeRequest } from "@orbflow/core/types";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { timeAgo } from "./shared";

interface ActivityTimelineProps {
  changeRequest: ChangeRequest;
}

interface TimelineEvent {
  type: "created" | "submitted" | "commented" | "approved" | "rejected" | "merged";
  description: string;
  timestamp: string;
  actor?: string;
}

function buildTimeline(cr: ChangeRequest): TimelineEvent[] {
  const events: TimelineEvent[] = [];

  events.push({
    type: "created",
    description: `Created by ${cr.author}`,
    timestamp: cr.created_at,
    actor: cr.author,
  });

  for (const comment of cr.comments ?? []) {
    const target = comment.node_id ? ` on ${comment.node_id}` : "";
    events.push({
      type: "commented",
      description: `${comment.author} commented${target}`,
      timestamp: comment.created_at,
      actor: comment.author,
    });
  }

  if (cr.status === "open" && cr.updated_at !== cr.created_at) {
    events.push({ type: "submitted", description: "Submitted for review", timestamp: cr.updated_at });
  }
  if (cr.status === "approved") {
    events.push({ type: "approved", description: "Approved", timestamp: cr.updated_at });
  }
  if (cr.status === "rejected") {
    events.push({ type: "rejected", description: "Rejected", timestamp: cr.updated_at });
  }
  if (cr.status === "merged") {
    events.push({ type: "merged", description: "Merged", timestamp: cr.updated_at });
  }

  events.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());

  return events;
}

const EVENT_CONFIG: Record<TimelineEvent["type"], { icon: string; bg: string; iconColor: string }> = {
  created: { icon: "plus", bg: "bg-orbflow-surface-hover", iconColor: "text-orbflow-text-muted" },
  submitted: { icon: "git-pull-request", bg: "bg-blue-500/15", iconColor: "text-blue-400" },
  commented: { icon: "message-circle", bg: "bg-orbflow-surface-hover", iconColor: "text-orbflow-text-muted" },
  approved: { icon: "check", bg: "bg-emerald-500/15", iconColor: "text-emerald-400" },
  rejected: { icon: "x", bg: "bg-red-500/15", iconColor: "text-red-400" },
  merged: { icon: "git-merge", bg: "bg-purple-500/15", iconColor: "text-purple-400" },
};

export const ActivityTimeline = memo(function ActivityTimeline({ changeRequest }: ActivityTimelineProps) {
  const events = useMemo(() => buildTimeline(changeRequest), [changeRequest]);

  return (
    <div className="px-5 py-4">
      <h3 className="text-body font-medium text-orbflow-text mb-4">Activity</h3>

      {events.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-12 text-center animate-fade-in">
          <div className="w-10 h-10 rounded-xl bg-orbflow-surface-hover flex items-center justify-center mb-3">
            <NodeIcon name="clock" className="w-5 h-5 text-orbflow-text-ghost" />
          </div>
          <p className="text-body text-orbflow-text-muted">No activity yet</p>
        </div>
      ) : (
        <div className="relative">
          {/* Vertical line */}
          <div className="absolute left-[13px] top-4 bottom-4 w-px bg-orbflow-border" />

          <div className="space-y-4">
            {events.map((event, i) => {
              const config = EVENT_CONFIG[event.type];
              return (
                <div
                  key={`${event.type}-${event.timestamp}-${i}`}
                  className="flex items-start gap-3 relative animate-fade-in-up"
                  style={{ animationDelay: `${Math.min(i * 40, 200)}ms` }}
                >
                  {/* Icon dot */}
                  <div className={cn(
                    "w-[26px] h-[26px] rounded-full flex items-center justify-center shrink-0 z-10 border-2 border-orbflow-bg",
                    config.bg,
                  )}>
                    <NodeIcon name={config.icon} className={cn("w-3 h-3", config.iconColor)} />
                  </div>

                  {/* Content */}
                  <div className="flex-1 flex items-baseline justify-between min-w-0 pt-0.5">
                    <p className="text-body text-orbflow-text">{event.description}</p>
                    <span className="text-caption text-orbflow-text-ghost shrink-0 ml-2">
                      {timeAgo(event.timestamp)}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
});
