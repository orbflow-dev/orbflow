import { describe, it, expect } from "vitest";
import {
  OUTPUT_SIZE_WARNING_BYTES,
  isBinaryContentType,
  looksLikeBinary,
  tryParseJson,
  estimateByteSize,
  formatBytes,
  extractContentType,
  isSafeUrl,
  analyzeOutput,
} from "./output-safety";

describe("isBinaryContentType", () => {
  it("returns true for image types", () => {
    expect(isBinaryContentType("image/png")).toBe(true);
    expect(isBinaryContentType("image/jpeg")).toBe(true);
  });

  it("returns true for audio/video types", () => {
    expect(isBinaryContentType("audio/mpeg")).toBe(true);
    expect(isBinaryContentType("video/mp4")).toBe(true);
  });

  it("returns true for binary application types", () => {
    expect(isBinaryContentType("application/octet-stream")).toBe(true);
    expect(isBinaryContentType("application/pdf")).toBe(true);
    expect(isBinaryContentType("application/zip")).toBe(true);
    expect(isBinaryContentType("application/gzip")).toBe(true);
    expect(isBinaryContentType("application/x-tar")).toBe(true);
    expect(isBinaryContentType("application/vnd.openxmlformats")).toBe(true);
  });

  it("returns false for text types", () => {
    expect(isBinaryContentType("text/plain")).toBe(false);
    expect(isBinaryContentType("text/html")).toBe(false);
    expect(isBinaryContentType("application/json")).toBe(false);
  });

  it("is case-insensitive", () => {
    expect(isBinaryContentType("Image/PNG")).toBe(true);
    expect(isBinaryContentType("APPLICATION/PDF")).toBe(true);
  });

  it("trims whitespace", () => {
    expect(isBinaryContentType("  image/png  ")).toBe(true);
  });
});

describe("looksLikeBinary", () => {
  it("returns false for empty string", () => {
    expect(looksLikeBinary("")).toBe(false);
  });

  it("returns false for normal text", () => {
    expect(looksLikeBinary("Hello, world!")).toBe(false);
  });

  it("returns false for text with allowed control chars", () => {
    expect(looksLikeBinary("line1\nline2\ttab\r")).toBe(false);
  });

  it("returns true when >10% non-printable chars", () => {
    // 20 chars, 3 are control (15%)
    const binary = "\x00\x01\x02abcdefghijklmnopq";
    expect(looksLikeBinary(binary)).toBe(true);
  });

  it("returns false when <=10% non-printable chars", () => {
    // 100 chars, 5 are control (5%)
    const text = "\x00\x01\x02\x03\x04" + "a".repeat(95);
    expect(looksLikeBinary(text)).toBe(false);
  });

  it("respects custom sampleSize", () => {
    // First 5 chars are all control -> 100% non-printable
    const data = "\x00\x01\x02\x03\x04" + "a".repeat(100);
    expect(looksLikeBinary(data, 5)).toBe(true);
  });
});

describe("tryParseJson", () => {
  it("parses valid JSON objects", () => {
    expect(tryParseJson('{"key": "value"}')).toEqual({ key: "value" });
  });

  it("parses valid JSON arrays", () => {
    expect(tryParseJson("[1, 2, 3]")).toEqual([1, 2, 3]);
  });

  it("returns null for non-JSON strings", () => {
    expect(tryParseJson("hello")).toBeNull();
    expect(tryParseJson("123")).toBeNull();
    expect(tryParseJson("true")).toBeNull();
  });

  it("returns null for invalid JSON that starts with { or [", () => {
    expect(tryParseJson("{invalid}")).toBeNull();
    expect(tryParseJson("[broken")).toBeNull();
  });

  it("handles whitespace-padded JSON", () => {
    expect(tryParseJson('  {"a": 1}  ')).toEqual({ a: 1 });
  });
});

describe("estimateByteSize", () => {
  it("returns 0 for null and undefined", () => {
    expect(estimateByteSize(null)).toBe(0);
    expect(estimateByteSize(undefined)).toBe(0);
  });

  it("returns byte size for strings", () => {
    const size = estimateByteSize("hello");
    expect(size).toBe(5);
  });

  it("returns byte size for objects", () => {
    const size = estimateByteSize({ a: 1 });
    expect(size).toBeGreaterThan(0);
  });

  it("returns 0 for circular objects", () => {
    const obj: Record<string, unknown> = {};
    obj.self = obj;
    expect(estimateByteSize(obj)).toBe(0);
  });
});

describe("formatBytes", () => {
  it("formats bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("formats kilobytes", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
  });

  it("formats megabytes", () => {
    expect(formatBytes(1024 * 1024)).toBe("1.00 MB");
    expect(formatBytes(2.5 * 1024 * 1024)).toBe("2.50 MB");
  });
});

describe("extractContentType", () => {
  it("extracts content-type header (case-insensitive)", () => {
    expect(extractContentType({ "Content-Type": "application/json" })).toBe("application/json");
    expect(extractContentType({ "content-type": "text/plain" })).toBe("text/plain");
    expect(extractContentType({ "CONTENT-TYPE": "image/png" })).toBe("image/png");
  });

  it("returns null for missing header", () => {
    expect(extractContentType({ "X-Custom": "value" })).toBeNull();
  });

  it("returns null for non-string values", () => {
    expect(extractContentType({ "content-type": 123 })).toBeNull();
  });

  it("returns null for null/undefined/non-object", () => {
    expect(extractContentType(null)).toBeNull();
    expect(extractContentType(undefined)).toBeNull();
    expect(extractContentType("string")).toBeNull();
  });
});

describe("isSafeUrl", () => {
  it("allows http/https URLs", () => {
    expect(isSafeUrl("https://example.com")).toBe(true);
    expect(isSafeUrl("http://localhost:3000")).toBe(true);
  });

  it("blocks javascript: URLs", () => {
    expect(isSafeUrl("javascript:alert(1)")).toBe(false);
    expect(isSafeUrl("JAVASCRIPT:alert(1)")).toBe(false);
  });

  it("blocks data:text/html URLs", () => {
    expect(isSafeUrl("data:text/html,<script>alert(1)</script>")).toBe(false);
  });

  it("blocks data:application URLs", () => {
    expect(isSafeUrl("data:application/javascript,alert(1)")).toBe(false);
  });

  it("allows relative paths", () => {
    expect(isSafeUrl("/api/data")).toBe(true);
  });

  it("trims whitespace", () => {
    expect(isSafeUrl("  javascript:alert(1)  ")).toBe(false);
  });
});

describe("analyzeOutput", () => {
  it("detects backend binary marker", () => {
    const result = analyzeOutput({ _binary: true, content_type: "image/png", size_bytes: 1024 });
    expect(result.isBinary).toBe(true);
    expect(result.contentType).toBe("image/png");
    expect(result.sizeBytes).toBe(1024);
    expect(result.sizeFormatted).toBe("1.0 KB");
  });

  it("detects binary content type", () => {
    const result = analyzeOutput("some data", "image/png");
    expect(result.isBinary).toBe(true);
  });

  it("detects binary-looking strings", () => {
    const binaryStr = "\x00\x01\x02\x03\x04\x05\x06\x07\x08\x0B\x0C\x0E\x0F";
    const result = analyzeOutput(binaryStr);
    expect(result.isBinary).toBe(true);
  });

  it("parses JSON strings", () => {
    const result = analyzeOutput('{"key": "value"}');
    expect(result.parsedJson).toEqual({ key: "value" });
    expect(result.isBinary).toBe(false);
  });

  it("flags large outputs", () => {
    const large = "x".repeat(OUTPUT_SIZE_WARNING_BYTES + 1);
    const result = analyzeOutput(large);
    expect(result.isTooLarge).toBe(true);
  });

  it("handles null values", () => {
    const result = analyzeOutput(null);
    expect(result.sizeBytes).toBe(0);
    expect(result.isBinary).toBe(false);
  });
});
