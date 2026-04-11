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
  const tokensRef = useRef<string[]>([]);

  const close = useCallback(() => {
    if (sourceRef.current) {
      sourceRef.current.close();
      sourceRef.current = null;
    }
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

    const source = new EventSource(url);
    sourceRef.current = source;

    source.addEventListener("data", (event) => {
      try {
        const msg: StreamMessage = JSON.parse(event.data);
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
    });

    source.addEventListener("done", (event) => {
      // Final flush so no tokens are lost between last interval tick and close.
      setTokens([...tokensRef.current]);
      try {
        const msg: StreamMessage = JSON.parse(event.data);
        const output = (msg.chunk as { output?: { data?: Record<string, unknown> } })?.output?.data || {};
        setFinalOutput(output);
        onDone?.(output);
      } catch {
        // Ignore.
      }
      close();
    });

    source.addEventListener("error", (event) => {
      // Check if it's a custom error event with data.
      const messageEvent = event as MessageEvent;
      if (messageEvent.data) {
        try {
          const msg: StreamMessage = JSON.parse(messageEvent.data);
          const errMsg = (msg.chunk as { message?: string })?.message || "Stream error";
          setError(errMsg);
          onError?.(errMsg);
        } catch {
          setError("Stream error");
          onError?.("Stream error");
        }
      } else {
        // EventSource connection error (e.g., server disconnected).
        setError("Connection lost");
        onError?.("Connection lost");
      }
      close();
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
