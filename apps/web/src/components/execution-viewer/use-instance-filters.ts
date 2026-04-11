"use client";

import { useState, useCallback, useMemo } from "react";
import type { Instance } from "@/lib/api";
import { getDateGroup } from "./viewer-utils";

export function useInstanceFilters(instances: Instance[], workflows: { id: string; name: string; nodes: { plugin_ref: string }[] }[]) {
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [search, setSearch] = useState("");

  const workflowNameMap = useMemo(() => {
    const map: Record<string, string> = {};
    for (const wf of workflows) map[wf.id] = wf.name;
    return map;
  }, [workflows]);

  const getWorkflowName = useCallback((wfId: string) => workflowNameMap[wfId] || "Workflow unavailable", [workflowNameMap]);

  const triggerTypeMap = useMemo(() => {
    const map: Record<string, { icon: string; label: string }> = {};
    for (const wf of workflows) {
      const ref = wf.nodes.find((n) => n.plugin_ref?.startsWith("builtin:trigger-"))?.plugin_ref;
      if (ref?.includes("webhook")) map[wf.id] = { icon: "webhook", label: "Webhook" };
      else if (ref?.includes("cron")) map[wf.id] = { icon: "clock", label: "Schedule" };
      else if (ref?.includes("event")) map[wf.id] = { icon: "zap", label: "Event" };
      else map[wf.id] = { icon: "play", label: "Manual" };
    }
    return map;
  }, [workflows]);

  const filteredInstances = useMemo(() => {
    let result = instances;
    if (statusFilter !== "all") result = result.filter((i) => i.status === statusFilter);
    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter((i) => getWorkflowName(i.workflow_id).toLowerCase().includes(q) || i.id.toLowerCase().includes(q));
    }
    return result;
  }, [instances, statusFilter, search, getWorkflowName]);

  const groupedInstances = useMemo(() => {
    const groups: Record<string, Instance[]> = {};
    const order = ["Today", "Yesterday", "This Week", "Earlier"];
    for (const inst of filteredInstances) {
      const group = getDateGroup(inst.created_at);
      if (!groups[group]) groups[group] = [];
      groups[group].push(inst);
    }
    return order.filter((g) => groups[g]?.length).map((g) => ({ label: g, items: groups[g] }));
  }, [filteredInstances]);

  const flatInstances = useMemo(() => groupedInstances.flatMap((g) => g.items), [groupedInstances]);

  const statusCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const inst of instances) counts[inst.status] = (counts[inst.status] || 0) + 1;
    return counts;
  }, [instances]);

  return {
    statusFilter, setStatusFilter, search, setSearch,
    getWorkflowName, triggerTypeMap, filteredInstances, groupedInstances, flatInstances, statusCounts,
  };
}
