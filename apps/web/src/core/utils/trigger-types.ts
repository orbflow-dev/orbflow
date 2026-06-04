/**
 * UI mapping from builtin trigger pluginRef -> trigger type.
 * Cron remains the UI label because canvas controls and toolbar copy use it.
 * Use toWireTriggerType before sending workflow definitions to the backend.
 */
export const TRIGGER_TYPE_MAP: Record<string, string> = {
  "builtin:trigger-webhook": "webhook",
  "builtin:trigger-cron": "cron",
  "builtin:trigger-event": "event",
  "builtin:trigger-manual": "manual",
};

export type WireTriggerType = "manual" | "webhook" | "schedule" | "event";

export function toWireTriggerType(triggerType: string): WireTriggerType {
  switch (triggerType) {
    case "cron":
    case "schedule":
      return "schedule";
    case "webhook":
      return "webhook";
    case "event":
      return "event";
    case "manual":
    default:
      return "manual";
  }
}
