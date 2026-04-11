import { describe, it, expect } from "vitest";
import { deriveEdgeStatus } from "./derive-edge-status";

describe("deriveEdgeStatus", () => {
  it("returns active when source completed and target running", () => {
    expect(deriveEdgeStatus("completed", "running")).toBe("active");
  });

  it("returns active when source completed and target queued", () => {
    expect(deriveEdgeStatus("completed", "queued")).toBe("active");
  });

  it("returns completed when both source and target completed", () => {
    expect(deriveEdgeStatus("completed", "completed")).toBe("completed");
  });

  it("returns failed when target failed", () => {
    expect(deriveEdgeStatus("completed", "failed")).toBe("failed");
    expect(deriveEdgeStatus("running", "failed")).toBe("failed");
    expect(deriveEdgeStatus(undefined, "failed")).toBe("failed");
  });

  it("returns idle when source is not completed", () => {
    expect(deriveEdgeStatus("running", "pending")).toBe("idle");
    expect(deriveEdgeStatus("pending", "pending")).toBe("idle");
  });

  it("returns idle when both undefined", () => {
    expect(deriveEdgeStatus(undefined, undefined)).toBe("idle");
  });

  it("returns idle when source completed but target pending", () => {
    expect(deriveEdgeStatus("completed", "pending")).toBe("idle");
  });
});
