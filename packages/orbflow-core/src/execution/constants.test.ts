import { describe, it, expect } from "vitest";
import {
  formatDurationMs,
  formatDurationRange,
  STATUS_COLORS,
  STATUS_THEMES,
  STATUS_LABELS,
  STATUS_BADGE,
  FALLBACK_THEME,
  SEGMENT_ORDER,
  NODE_SIZES,
} from "./constants";

describe("formatDurationMs", () => {
  it("formats sub-second as <1s", () => {
    expect(formatDurationMs(0)).toBe("<1s");
    expect(formatDurationMs(500)).toBe("<1s");
    expect(formatDurationMs(999)).toBe("<1s");
  });

  it("formats seconds with one decimal", () => {
    expect(formatDurationMs(1000)).toBe("1.0s");
    expect(formatDurationMs(3200)).toBe("3.2s");
    expect(formatDurationMs(59999)).toBe("60.0s");
  });

  it("formats minutes and seconds", () => {
    expect(formatDurationMs(60000)).toBe("1m 0s");
    expect(formatDurationMs(135000)).toBe("2m 15s");
  });
});

describe("formatDurationRange", () => {
  it("formats sub-second range as <1s", () => {
    expect(formatDurationRange("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z")).toBe("<1s");
  });

  it("formats second-level range", () => {
    expect(formatDurationRange("2026-01-01T00:00:00Z", "2026-01-01T00:00:45Z")).toBe("45s");
  });

  it("formats minute-level range", () => {
    expect(formatDurationRange("2026-01-01T00:00:00Z", "2026-01-01T00:02:30Z")).toBe("2m 30s");
  });

  it("formats hour-level range", () => {
    expect(formatDurationRange("2026-01-01T00:00:00Z", "2026-01-01T01:05:00Z")).toBe("1h 5m");
  });

  it("handles reversed dates gracefully (clamps to 0)", () => {
    expect(formatDurationRange("2026-01-01T01:00:00Z", "2026-01-01T00:00:00Z")).toBe("<1s");
  });
});

describe("STATUS_COLORS", () => {
  it("has entries for all standard statuses", () => {
    const statuses = ["pending", "queued", "running", "completed", "failed", "skipped", "cancelled", "waiting_approval"];
    for (const s of statuses) {
      expect(STATUS_COLORS[s]).toBeDefined();
      expect(STATUS_COLORS[s]).toMatch(/^#[0-9A-Fa-f]{6}$/);
    }
  });
});

describe("STATUS_THEMES", () => {
  it("has theme for each status", () => {
    for (const key of Object.keys(STATUS_COLORS)) {
      expect(STATUS_THEMES[key]).toBeDefined();
      expect(STATUS_THEMES[key].accent).toBeDefined();
      expect(STATUS_THEMES[key].label).toBeDefined();
    }
  });
});

describe("FALLBACK_THEME", () => {
  it("is the pending theme", () => {
    expect(FALLBACK_THEME).toBe(STATUS_THEMES.pending);
  });
});

describe("STATUS_LABELS", () => {
  it("has labels for all statuses", () => {
    for (const key of Object.keys(STATUS_COLORS)) {
      expect(STATUS_LABELS[key]).toBeDefined();
      expect(typeof STATUS_LABELS[key]).toBe("string");
    }
  });
});

describe("NODE_SIZES", () => {
  it("has expected node kinds", () => {
    expect(NODE_SIZES.trigger).toBe(68);
    expect(NODE_SIZES.action).toBe(64);
    expect(NODE_SIZES.capability).toBe(52);
  });
});

describe("SEGMENT_ORDER", () => {
  it("starts with completed and ends with pending", () => {
    expect(SEGMENT_ORDER[0]).toBe("completed");
    expect(SEGMENT_ORDER[SEGMENT_ORDER.length - 1]).toBe("pending");
  });

  it("includes all key statuses", () => {
    expect(SEGMENT_ORDER).toContain("failed");
    expect(SEGMENT_ORDER).toContain("running");
  });
});
