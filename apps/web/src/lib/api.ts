import { createApiClient } from "@orbflow/core/client";

/** Root server URL (without version prefix) -- used for health checks. */
export const API_ROOT = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";
/** Versioned API base URL -- used for all API calls. */
export const BASE_URL = `${API_ROOT}/api/v1`;

/** Returns the configured bearer token, if this deployment uses one. */
export function getAuthToken(): string | undefined {
  const envToken = process.env.NEXT_PUBLIC_ORBFLOW_AUTH_TOKEN;
  if (envToken) return envToken;
  if (typeof window === "undefined") return undefined;
  return window.localStorage.getItem("orbflow.authToken") ?? undefined;
}

/**
 * Module-level singleton API client.
 *
 * Boot-order note: this module is evaluated once at import time. The
 * BASE_URL is resolved from the environment variable at that point and
 * cannot change afterwards. If you need a client pointed at a different
 * URL (e.g. in tests or multi-tenant scenarios) use `createApiClient(url)`
 * from "@orbflow/core/client" directly rather than importing this singleton.
 */
const authToken = getAuthToken();
export const api = createApiClient(authToken ? { baseUrl: BASE_URL, authToken } : BASE_URL);

// Re-export API types from the canonical source (@orbflow/core)
export type {
  Workflow,
  WorkflowNode,
  Instance,
  NodeState,
  TestNodeResult,
  CredentialAccessTier,
  CredentialPolicy,
  CredentialSummary,
  CredentialTypeSchema,
  WorkflowVersion,
  WorkflowDiff,
} from "@orbflow/core/types";
