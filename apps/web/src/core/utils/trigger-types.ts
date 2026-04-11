/**
 * Canonical mapping from builtin trigger pluginRef -> trigger_type string.
 * Used by both build-workflow-payload.ts and use-trigger-detection.ts.
 */
export const TRIGGER_TYPE_MAP: Record<string, string> = {
  "builtin:trigger-webhook": "webhook",
  "builtin:trigger-cron": "cron",
  "builtin:trigger-event": "event",
  "builtin:trigger-manual": "manual",
};
