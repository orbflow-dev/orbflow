"use client";

/**
 * Real-time streaming output display for AI nodes.
 *
 * Shows tokens as they arrive via SSE, with a typing cursor effect.
 */

interface StreamingOutputProps {
  /** Accumulated tokens from the SSE stream. */
  tokens: string[];
  /** Whether the stream is currently active. */
  isStreaming: boolean;
  /** Error message if the stream failed. */
  error: string | null;
}

export function StreamingOutput({ tokens, isStreaming, error }: StreamingOutputProps) {
  const text = tokens.join("");

  if (error) {
    return (
      <div
        style={{
          padding: "12px 16px",
          borderRadius: 8,
          background: "rgba(217, 69, 79, 0.08)",
          border: "1px solid rgba(217, 69, 79, 0.2)",
          fontSize: 12,
          color: "#D9454F",
        }}
      >
        Stream error: {error}
      </div>
    );
  }

  if (tokens.length === 0 && !isStreaming) {
    return null;
  }

  return (
    <div
      style={{
        padding: "12px 16px",
        borderRadius: 8,
        background: "rgba(74, 154, 175, 0.06)",
        border: "1px solid rgba(74, 154, 175, 0.15)",
        position: "relative",
      }}
    >
      {isStreaming && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            marginBottom: 8,
            fontSize: 11,
            color: "#4A9AAF",
            fontWeight: 500,
          }}
        >
          <span
            style={{
              display: "inline-block",
              width: 6,
              height: 6,
              borderRadius: "50%",
              background: "#4A9AAF",
              animation: "pulse 1.5s ease-in-out infinite",
            }}
          />
          Streaming...
        </div>
      )}
      <pre
        style={{
          margin: 0,
          fontSize: 12,
          lineHeight: 1.6,
          color: "var(--text-primary, #E5E7EB)",
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
          fontFamily: "inherit",
        }}
      >
        {text}
        {isStreaming && (
          <span
            style={{
              display: "inline-block",
              width: 2,
              height: 14,
              background: "#4A9AAF",
              marginLeft: 1,
              animation: "blink 1s step-end infinite",
              verticalAlign: "text-bottom",
            }}
          />
        )}
      </pre>
      {!isStreaming && tokens.length > 0 && (
        <div
          style={{
            marginTop: 8,
            fontSize: 10,
            color: "var(--text-tertiary, #6B7280)",
          }}
        >
          {tokens.length} chunks received
        </div>
      )}
      <style>{`
        @keyframes blink { 50% { opacity: 0; } }
        @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
      `}</style>
    </div>
  );
}
