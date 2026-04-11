/**
 * Output safety utilities for rendering workflow node outputs.
 * Handles binary detection, size limits, content-type awareness, and XSS prevention.
 */

/** Size thresholds */
export const OUTPUT_SIZE_WARNING_BYTES = 512 * 1024; // 512KB

/** Known binary content-type prefixes */
const BINARY_CONTENT_TYPE_PREFIXES = [
  "image/",
  "audio/",
  "video/",
  "application/octet-stream",
  "application/pdf",
  "application/zip",
  "application/gzip",
  "application/x-tar",
  "application/vnd.",
] as const;

export function isBinaryContentType(contentType: string): boolean {
  const ct = contentType.toLowerCase().trim();
  return BINARY_CONTENT_TYPE_PREFIXES.some((prefix) => ct.startsWith(prefix));
}

/**
 * Heuristic binary detection: if >10% of the first 512 chars are
 * non-printable control characters, treat the value as binary.
 */
export function looksLikeBinary(value: string, sampleSize = 512): boolean {
  const sample = value.slice(0, sampleSize);
  if (sample.length === 0) return false;
  let nonPrintable = 0;
  for (let i = 0; i < sample.length; i++) {
    const code = sample.charCodeAt(i);
    // Allow tab (9), newline (10), carriage return (13)
    if (code < 32 && code !== 9 && code !== 10 && code !== 13) nonPrintable++;
  }
  return nonPrintable / sample.length > 0.1;
}

/**
 * Attempt to parse a string as JSON. Only tries if the string starts
 * with `{` or `[` to avoid wasting cycles on obvious non-JSON.
 */
export function tryParseJson(value: string): unknown | null {
  const trimmed = value.trim();
  if (trimmed[0] !== "{" && trimmed[0] !== "[") return null;
  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

/**
 * Estimate byte size of a value. Uses Blob when available (browser),
 * falls back to string length for SSR.
 */
export function estimateByteSize(value: unknown): number {
  if (value === null || value === undefined) return 0;
  if (typeof value === "string") {
    if (typeof Blob !== "undefined") return new Blob([value]).size;
    return value.length;
  }
  try {
    const serialized = JSON.stringify(value);
    if (typeof Blob !== "undefined") return new Blob([serialized]).size;
    return serialized.length;
  } catch {
    return 0;
  }
}

/** Format byte count as a human-readable string. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

/**
 * Case-insensitive extraction of the content-type header from a headers
 * object (keys may be any casing).
 */
export function extractContentType(headers: unknown): string | null {
  if (!headers || typeof headers !== "object") return null;
  const h = headers as Record<string, unknown>;
  for (const key of Object.keys(h)) {
    if (key.toLowerCase() === "content-type") {
      return typeof h[key] === "string" ? (h[key] as string) : null;
    }
  }
  return null;
}

/** Block unsafe URL schemes that are XSS vectors (javascript:, data:text/html, etc). */
const UNSAFE_URL_SCHEMES = ["javascript:", "data:text/html", "data:application/"] as const;

export function isSafeUrl(url: string): boolean {
  const lower = url.trim().toLowerCase();
  return !UNSAFE_URL_SCHEMES.some((scheme) => lower.startsWith(scheme));
}

interface OutputAnalysis {
  value: unknown;
  isBinary: boolean;
  isTooLarge: boolean;
  sizeBytes: number;
  sizeFormatted: string;
  parsedJson: unknown | null;
  contentType: string | null;
}

/**
 * Unified output analysis: checks size, binary, JSON parsing, and content-type.
 * Call once per render, memoize the result.
 */
export function analyzeOutput(
  value: unknown,
  contentType?: string | null,
): OutputAnalysis {
  // Check for backend binary marker: { _binary: true, content_type, size_bytes }
  if (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    (value as Record<string, unknown>)["_binary"] === true
  ) {
    const meta = value as Record<string, unknown>;
    const ct = (meta.content_type as string) ?? null;
    return {
      value,
      isBinary: true,
      isTooLarge: false,
      sizeBytes: (meta.size_bytes as number) ?? 0,
      sizeFormatted: formatBytes((meta.size_bytes as number) ?? 0),
      parsedJson: null,
      contentType: ct,
    };
  }

  const sizeBytes = estimateByteSize(value);
  const sizeFormatted = formatBytes(sizeBytes);
  const isTooLarge = sizeBytes > OUTPUT_SIZE_WARNING_BYTES;
  let isBinary = false;
  let parsedJson: unknown | null = null;
  const ct = contentType ?? null;

  if (ct && isBinaryContentType(ct)) {
    isBinary = true;
  } else if (typeof value === "string") {
    if (looksLikeBinary(value)) {
      isBinary = true;
    } else {
      parsedJson = tryParseJson(value);
    }
  }

  return { value, isBinary, isTooLarge, sizeBytes, sizeFormatted, parsedJson, contentType: ct };
}
