import { describe, it, expect } from "vitest";
import { generateNodeSlug } from "./node-slug";

describe("generateNodeSlug", () => {
  it("converts schema name to snake_case with counter", () => {
    expect(generateNodeSlug("HTTP Request", [])).toBe("http_request_1");
  });

  it("increments counter to avoid collisions", () => {
    const existing = ["http_request_1", "http_request_2"];
    expect(generateNodeSlug("HTTP Request", existing)).toBe("http_request_3");
  });

  it("starts at 1 when no collisions", () => {
    const existing = ["email_1", "filter_1"];
    expect(generateNodeSlug("HTTP Request", existing)).toBe("http_request_1");
  });

  it("strips leading/trailing underscores", () => {
    expect(generateNodeSlug("  --My Node--  ", [])).toBe("my_node_1");
  });

  it("handles single-word names", () => {
    expect(generateNodeSlug("Filter", [])).toBe("filter_1");
  });

  it("replaces non-alphanumeric characters with underscores", () => {
    expect(generateNodeSlug("Send Email (SMTP)", [])).toBe(
      "send_email_smtp_1",
    );
  });

  it("handles empty existing list", () => {
    expect(generateNodeSlug("Delay", [])).toBe("delay_1");
  });

  it("skips multiple occupied slots", () => {
    const existing = ["log_1", "log_2", "log_3"];
    expect(generateNodeSlug("Log", existing)).toBe("log_4");
  });
});
