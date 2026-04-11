import { describe, it, expect } from "vitest";
import { createStoreClient } from "./store-client";

describe("createStoreClient", () => {
  it("throws when requireClient is called before setClient", () => {
    const { requireClient } = createStoreClient("Test");
    expect(() => requireClient()).toThrow(
      "Test store: API client not initialized"
    );
  });

  it("returns the client after setClient is called", () => {
    const { setClient, requireClient } = createStoreClient("Test");
    const fakeClient = { get: () => {}, post: () => {} } as never;
    setClient(fakeClient);
    expect(requireClient()).toBe(fakeClient);
  });

  it("includes store name in the error message", () => {
    const { requireClient } = createStoreClient("Workflow");
    expect(() => requireClient()).toThrow("Workflow store");
    expect(() => requireClient()).toThrow("setWorkflowApiClient");
  });

  it("creates independent instances per call", () => {
    const a = createStoreClient("A");
    const b = createStoreClient("B");
    const fakeClient = {} as never;

    a.setClient(fakeClient);

    expect(a.requireClient()).toBe(fakeClient);
    expect(() => b.requireClient()).toThrow("B store");
  });
});
