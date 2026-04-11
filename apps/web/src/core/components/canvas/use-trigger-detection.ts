import { useMemo } from "react";
import type { Node } from "@xyflow/react";
import type { ParameterValue } from "../../types/schema";
import { TRIGGER_TYPE_MAP } from "../../utils/trigger-types";

type TriggerType = "manual" | "webhook" | "cron" | "event";

interface TriggerDetection {
  triggerType: TriggerType;
  triggerInfo: Record<string, string>;
}

export function useTriggerDetection(
  nodes: Node[],
  parameterValues: Record<string, Record<string, ParameterValue>>,
): TriggerDetection {
  return useMemo((): TriggerDetection => {
    const triggerNode = nodes.find((n) => {
      const kind = n.data?.nodeKind as string | undefined;
      const pluginRef = n.data?.pluginRef as string | undefined;
      return kind === "trigger" || pluginRef?.startsWith("builtin:trigger-");
    });
    if (!triggerNode) {
      return { triggerType: "manual", triggerInfo: {} };
    }
    const pluginRef = triggerNode.data?.pluginRef as string | undefined;
    const nodeId = triggerNode.id;
    const nodeParams = parameterValues[nodeId] || {};

    const extractParam = (key: string): string | undefined => {
      const pv = nodeParams[key];
      if (pv?.value != null) return String(pv.value);
      if (pv?.expression) return pv.expression;
      return undefined;
    };

    const triggerType = (TRIGGER_TYPE_MAP[pluginRef ?? ""] ?? "manual") as TriggerType;
    switch (triggerType) {
      case "webhook":
        return { triggerType, triggerInfo: { webhookPath: extractParam("path") || "/webhooks/..." } };
      case "cron":
        return { triggerType, triggerInfo: { cronExpression: extractParam("cron") || "* * * * *" } };
      case "event":
        return { triggerType, triggerInfo: { eventName: extractParam("event_name") || "event" } };
      case "manual":
      default:
        return { triggerType: "manual", triggerInfo: {} };
    }
  }, [nodes, parameterValues]);
}
