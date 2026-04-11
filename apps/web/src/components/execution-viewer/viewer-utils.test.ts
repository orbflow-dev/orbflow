import { describe, it, expect } from "vitest";
import { getDateGroup, formatOutput, instanceStats } from "./viewer-utils";

describe("getDateGroup", () => {
  it("returns Today for current date", () => {
    expect(getDateGroup(new Date().toISOString())).toBe("Today");
  });

  it("returns Yesterday for yesterday's date", () => {
    const yesterday = new Date(Date.now() - 86_400_000);
    expect(getDateGroup(yesterday.toISOString())).toBe("Yesterday");
  });

  it("returns This Week for 3 days ago", () => {
    const threeDaysAgo = new Date(Date.now() - 3 * 86_400_000);
    expect(getDateGroup(threeDaysAgo.toISOString())).toBe("This Week");
  });

  it("returns Earlier for 30 days ago", () => {
    const thirtyDaysAgo = new Date(Date.now() - 30 * 86_400_000);
    expect(getDateGroup(thirtyDaysAgo.toISOString())).toBe("Earlier");
  });
});

describe("formatOutput", () => {
  it("formats a simple object", () => {
    const result = formatOutput({ key: "value" });
    expect(result).toContain('"key"');
    expect(result).toContain('"value"');
  });

  it("parses nested JSON strings", () => {
    const result = formatOutput({ data: '{"nested": true}' });
    const parsed = JSON.parse(result);
    expect(parsed.data.nested).toBe(true);
  });

  it("handles non-JSON strings gracefully", () => {
    const result = formatOutput({ msg: "hello world" });
    expect(result).toContain("hello world");
  });
});

describe("instanceStats", () => {
  it("returns zeros for undefined input", () => {
    const stats = instanceStats(undefined);
    expect(stats).toEqual({
      completed: 0, running: 0, failed: 0, pending: 0, cancelled: 0, skipped: 0, total: 0,
    });
  });

  it("returns zeros for empty object", () => {
    const stats = instanceStats({});
    expect(stats.total).toBe(0);
  });

  it("counts statuses correctly", () => {
    const nodeStates = {
      n1: { status: "completed" },
      n2: { status: "completed" },
      n3: { status: "running" },
      n4: { status: "failed" },
      n5: { status: "pending" },
    };
    const stats = instanceStats(nodeStates);
    expect(stats.completed).toBe(2);
    expect(stats.running).toBe(1);
    expect(stats.failed).toBe(1);
    expect(stats.pending).toBe(1);
    expect(stats.cancelled).toBe(0);
    expect(stats.total).toBe(5);
  });
});
