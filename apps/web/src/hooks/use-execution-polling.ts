"use client";

import { useExecutionPolling as usePollingCore } from "@orbflow/core/hooks";
import { BASE_URL } from "@/lib/api";

interface UseExecutionPollingOptions {
  instanceId: string | null;
  interval?: number;
  enabled: boolean;
}

export function useExecutionPolling(options: UseExecutionPollingOptions) {
  return usePollingCore({ ...options, baseUrl: BASE_URL });
}
