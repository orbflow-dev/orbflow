/**
 * SSE hook for streaming real-time node execution output.
 *
 * Connects to the server's SSE endpoint and receives incremental chunks
 * (e.g., LLM tokens) as they are produced by the worker.
 */
import { useEffect, useRef, useState, useCallback } from "react";

const BATCH_FLUSH_INTERVAL_MS = 100;

export interface StreamChunk {
  type: "data" | "done" | "error";
  payload?: unknown;
  output?: { data?: Record<string, unknown>; error?: string };
  message?: string;
}

export interface StreamMessage {
  instance_id: string;
  node_id: string;
  chunk: StreamChunk;
  seq: number;
}

export interface UseNodeStreamOptions {
  /** Full SSE URL (from apiClient.instances.streamUrl). */
  url: string | null;
  /** Set to true to start streaming. */
  enabled: boolean;
  /** Optional bearer token. When present, fetch streaming is used so the token stays in headers, not the URL. */
  authToken?: string;
  /** Called for each data chunk (e.g., LLM token). */
  onData?: (payload: unknown, seq: number) => void;
  /** Called when the stream completes. */
  onDone?: (output: Record<string, unknown>) => void;
  /** Called when the stream encounters an error. */
  onError?: (message: string) => void;
}

export interface UseNodeStreamReturn {
  /** Whether the stream is currently connected. */
  isStreaming: boolean;
  /** Accumulated tokens (for LLM streaming). */
  tokens: string[];
  /** The final output (set when done). */
  finalOutput: Record<string, unknown> | null;
  /** Error message (set on error). */
  error: string | null;
  /** Manually close the stream. */
  close: () => void;
}

export function useNodeStream(options: UseNodeStreamOptions): UseNodeStreamReturn {
  const { url, enabled, onData, onDone, onError } = options;
  const [isStreaming, setIsStreaming] = useState(false);
  const [tokens, setTokens] = useState<string[]>([]);
  const [finalOutput, setFinalOutput] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const sourceRef = useRef<EventSource | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  const tokensRef = useRef<string[]>([]);

  const close = useCallback(() => {
    if (sourceRef.current) {
      sourceRef.current.close();
      sourceRef.current = null;
    }
    abortRef.current?.abort();
    abortRef.current = null;
    setIsStreaming(false);
  }, []);

  useEffect(() => {
    if (!enabled || !url) {
      close();
      return;
    }

    // Reset state for new stream.
    tokensRef.current = [];
    setTokens([]);
    setFinalOutput(null);
    setError(null);
    setIsStreaming(true);

    const handleDataEvent = (data: string) => {
      try {
        const msg: StreamMessage = JSON.parse(data);
        const payload = msg.chunk?.payload;

        // Extract token string if present.
        if (payload && typeof payload === "object" && "token" in (payload as Record<string, unknown>)) {
          const token = (payload as Record<string, string>).token;
          tokensRef.current.push(token);
        }

        onData?.(payload, msg.seq);
      } catch {
        // Ignore parse errors on data chunks.
      }
    };

    const handleDoneEvent = (data: string) => {
      // Final flush so no tokens are lost between last interval tick and close.
      setTokens([...tokensRef.current]);
      try {
        const msg: StreamMessage = JSON.parse(data);
        const output = (msg.chunk as { output?: { data?: Record<string, unknown> } })?.output?.data || {};
        setFinalOutput(output);
        onDone?.(output);
      } catch {
        // Ignore.
      }
      close();
    };

    const handleErrorEvent = (data?: string) => {
      if (data) {
        try {
          const msg: StreamMessage = JSON.parse(data);
          const errMsg = (msg.chunk as { message?: string })?.message || "Stream error";
          setError(errMsg);
          onError?.(errMsg);
        } catch {
          setError("Stream error");
          onError?.("Stream error");
        }
      } else {
        setError("Connection lost");
        onError?.("Connection lost");
      }
      close();
    };

    if (options.authToken) {
      const controller = new AbortController();
      abortRef.current = controller;

      const readStream = async () => {
        try {
          const response = await fetch(url, {
            headers: {
              Accept: "text/event-stream",
              Authorization: `Bearer ${options.authToken}`,
            },
            signal: controller.signal,
          });
          if (!response.ok) {
            handleErrorEvent(JSON.stringify({ chunk: { message: `Stream failed with status ${response.status}` } }));
            return;
          }
          if (!response.body) {
            handleErrorEvent(JSON.stringify({ chunk: { message: "Stream response body was empty" } }));
            return;
          }

          const reader = response.body.getReader();
          const decoder = new TextDecoder();
          let buffer = "";

          while (!controller.signal.aborted) {
            const { value, done } = await reader.read();
            if (done) break;
            buffer += decoder.decode(value, { stream: true });

            let boundary = buffer.indexOf("\n\n");
            while (boundary !== -1) {
              const rawEvent = buffer.slice(0, boundary);
              buffer = buffer.slice(boundary + 2);
              const lines = rawEvent.split(/\r?\n/);
              const eventName = lines.find((line) => line.startsWith("event:"))?.slice(6).trim() || "data";
              const data = lines
                .filter((line) => line.startsWith("data:"))
                .map((line) => line.slice(5).trimStart())
                .join("\n");

              if (eventName === "done") handleDoneEvent(data);
              else if (eventName === "error") handleErrorEvent(data);
              else if (data) handleDataEvent(data);

              boundary = buffer.indexOf("\n\n");
            }
          }
        } catch (err) {
          if (!controller.signal.aborted) {
            const message = err instanceof Error ? err.message : "Connection lost";
            handleErrorEvent(JSON.stringify({ chunk: { message } }));
          }
        }
      };

      void readStream();
      return () => {
        controller.abort();
        abortRef.current = null;
      };
    }

    const source = new EventSource(url);
    sourceRef.current = source;

    source.addEventListener("data", (event) => {
      handleDataEvent(event.data);
    });

    source.addEventListener("done", (event) => {
      handleDoneEvent(event.data);
    });

    source.addEventListener("error", (event) => {
      // Check if it's a custom error event with data.
      const messageEvent = event as MessageEvent;
      if (messageEvent.data) {
        handleErrorEvent(messageEvent.data);
      } else {
        // EventSource connection error (e.g., server disconnected).
        handleErrorEvent();
      }
    });

    return () => {
      source.close();
      sourceRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [url, enabled]);

  // Batch-flush accumulated tokens to state at a fixed interval (~10 re-renders/sec).
  useEffect(() => {
    if (!enabled) return;
    const interval = setInterval(() => {
      const ref = tokensRef.current;
      setTokens((prev) =>
        ref.length > prev.length ? [...ref] : prev,
      );
    }, BATCH_FLUSH_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [enabled]);

  return { isStreaming, tokens, finalOutput, error, close };
}
