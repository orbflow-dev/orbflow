"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useExecutionOverlayStore } from "../stores/execution-overlay-store";

const TERMINAL_STATUSES = new Set(["completed", "failed", "cancelled"]);

const DEFAULT_INTERVAL = 2000;
const GRACE_INTERVAL = 1000;
const GRACE_POLL_COUNT = 2;
const ERROR_WARNING_THRESHOLD = 5;
const ERROR_STOP_THRESHOLD = 10;

export interface UseExecutionPollingOptions {
  instanceId: string | null;
  interval?: number;
  enabled: boolean;
  /** Base URL for the API. Required — no process.env fallback. */
  baseUrl: string;
}

export interface UseExecutionPollingReturn {
  isPolling: boolean;
  error: string | null;
  consecutiveErrors: number;
}

export function useExecutionPolling(
  options: UseExecutionPollingOptions
): UseExecutionPollingReturn {
  const { instanceId, interval = DEFAULT_INTERVAL, enabled, baseUrl } = options;

  const [isPolling, setIsPolling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [consecutiveErrorsState, setConsecutiveErrorsState] = useState(0);

  const intervalIdRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const graceTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const graceCountRef = useRef(0);
  const consecutiveErrorsRef = useRef(0);
  const instanceIdRef = useRef(instanceId);

  // Keep instanceId ref in sync
  useEffect(() => {
    instanceIdRef.current = instanceId;
  }, [instanceId]);

  const clearPollingInterval = useCallback(() => {
    if (intervalIdRef.current !== null) {
      clearInterval(intervalIdRef.current);
      intervalIdRef.current = null;
    }
    if (graceTimeoutRef.current !== null) {
      clearTimeout(graceTimeoutRef.current);
      graceTimeoutRef.current = null;
    }
  }, []);

  const stopPolling = useCallback(() => {
    clearPollingInterval();
    setIsPolling(false);
  }, [clearPollingInterval]);

  const poll = useCallback(async () => {
    const currentInstanceId = instanceIdRef.current;
    if (!currentInstanceId) return;

    try {
      const res = await fetch(`${baseUrl}/instances/${currentInstanceId}`, {
        headers: { "Content-Type": "application/json" },
      });
      const json = await res.json();

      if (!res.ok || json.error) {
        throw new Error(json.error || `HTTP ${res.status}`);
      }

      const instance = json.data;

      // Reset error state on success
      consecutiveErrorsRef.current = 0;
      setConsecutiveErrorsState(0);
      setError(null);

      // Sync the instance data to the overlay store
      useExecutionOverlayStore.getState().syncFromInstance(instance);

      // If terminal, perform grace polls to catch in-flight node state updates
      if (TERMINAL_STATUSES.has(instance.status)) {
        if (graceCountRef.current < GRACE_POLL_COUNT) {
          // Switch from the regular interval to grace-poll mode:
          // clear the fast interval and schedule one more poll after GRACE_INTERVAL
          clearPollingInterval();
          graceCountRef.current += 1;
          graceTimeoutRef.current = setTimeout(poll, GRACE_INTERVAL);
        } else {
          // Grace polls exhausted — fully stop
          stopPolling();
          useExecutionOverlayStore.getState().stopLiveRun();
        }
      }
    } catch (err) {
      consecutiveErrorsRef.current += 1;
      const errorCount = consecutiveErrorsRef.current;
      setConsecutiveErrorsState(errorCount);

      if (errorCount >= ERROR_STOP_THRESHOLD) {
        setError("Lost connection. View run in Activity tab.");
        stopPolling();
        useExecutionOverlayStore.getState().stopLiveRun();
      } else if (errorCount >= ERROR_WARNING_THRESHOLD) {
        setError("Connection lost — retrying...");
      } else {
        console.warn(`Poll error ${errorCount}/${ERROR_STOP_THRESHOLD}:`, err);
      }
    }
  }, [baseUrl, stopPolling, clearPollingInterval]);

  useEffect(() => {
    // Clear any existing interval whenever dependencies change
    clearPollingInterval();

    if (!enabled || !instanceId) {
      setIsPolling(false);
      return;
    }

    // Reset error and grace state when starting fresh
    consecutiveErrorsRef.current = 0;
    graceCountRef.current = 0;
    setConsecutiveErrorsState(0);
    setError(null);
    setIsPolling(true);

    // Fire an immediate poll, then start the interval
    poll();
    intervalIdRef.current = setInterval(poll, interval);

    return () => {
      clearPollingInterval();
    };
  }, [enabled, instanceId, interval, poll, clearPollingInterval]);

  return {
    isPolling,
    error,
    consecutiveErrors: consecutiveErrorsState,
  };
}
