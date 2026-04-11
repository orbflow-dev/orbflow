import { describe, it, expect, vi } from "vitest";
import { NodeSchemaRegistry } from "./registry";
import type { NodeTypeDefinition } from "../types/schema";

function makeSchema(overrides: Partial<NodeTypeDefinition> = {}): NodeTypeDefinition {
  return {
    pluginRef: overrides.pluginRef ?? "test/node",
    name: overrides.name ?? "Test Node",
    description: overrides.description ?? "A test node",
    category: overrides.category ?? "builtin",
    nodeKind: overrides.nodeKind,
    icon: overrides.icon ?? "test-icon",
    color: overrides.color ?? "#000000",
    inputs: overrides.inputs ?? [],
    outputs: overrides.outputs ?? [],
    ...overrides,
  };
}

const httpNode = makeSchema({
  pluginRef: "builtin/http",
  name: "HTTP Request",
  category: "builtin",
  nodeKind: "action",
});

const emailNode = makeSchema({
  pluginRef: "builtin/email",
  name: "Email",
  category: "builtin",
  nodeKind: "action",
});

const cronTrigger = makeSchema({
  pluginRef: "builtin/cron",
  name: "Cron Trigger",
  category: "builtin",
  nodeKind: "trigger",
});

const customPlugin = makeSchema({
  pluginRef: "plugin/slack",
  name: "Slack",
  category: "plugin",
  nodeKind: "action",
});

const capabilityNode = makeSchema({
  pluginRef: "builtin/postgres-cap",
  name: "Postgres Capability",
  category: "builtin",
  nodeKind: "capability",
});

// A node with no explicit nodeKind — should default to "action" in getByNodeKind
const implicitActionNode = makeSchema({
  pluginRef: "builtin/transform",
  name: "Transform",
  category: "builtin",
  nodeKind: undefined,
});

describe("NodeSchemaRegistry", () => {
  // ── 1. Constructor with initial schemas ─────────────────────────────
  describe("constructor", () => {
    it("registers initial schemas passed to constructor", () => {
      const registry = new NodeSchemaRegistry([httpNode, emailNode]);
      expect(registry.get("builtin/http")).toEqual(httpNode);
      expect(registry.get("builtin/email")).toEqual(emailNode);
    });

    it("creates an empty registry when no initial schemas provided", () => {
      const registry = new NodeSchemaRegistry();
      expect(registry.getAll()).toEqual([]);
    });

    it("creates an empty registry when empty array provided", () => {
      const registry = new NodeSchemaRegistry([]);
      expect(registry.getAll()).toEqual([]);
    });
  });

  // ── 2. register + get ───────────────────────────────────────────────
  describe("register / get", () => {
    it("registers a schema and retrieves it by pluginRef", () => {
      const registry = new NodeSchemaRegistry();
      registry.register(httpNode);
      expect(registry.get("builtin/http")).toEqual(httpNode);
    });

    it("returns undefined for unregistered pluginRef", () => {
      const registry = new NodeSchemaRegistry();
      expect(registry.get("nonexistent")).toBeUndefined();
    });

    it("overwrites a previously registered schema with the same pluginRef", () => {
      const registry = new NodeSchemaRegistry();
      registry.register(httpNode);

      const updated = makeSchema({
        pluginRef: "builtin/http",
        name: "HTTP Request v2",
      });
      registry.register(updated);

      expect(registry.get("builtin/http")?.name).toBe("HTTP Request v2");
      expect(registry.getAll()).toHaveLength(1);
    });
  });

  // ── 3. getAll ───────────────────────────────────────────────────────
  describe("getAll", () => {
    it("returns all registered schemas", () => {
      const registry = new NodeSchemaRegistry([httpNode, emailNode, cronTrigger]);
      const all = registry.getAll();
      expect(all).toHaveLength(3);
      expect(all).toContainEqual(httpNode);
      expect(all).toContainEqual(emailNode);
      expect(all).toContainEqual(cronTrigger);
    });

    it("returns an empty array when nothing is registered", () => {
      const registry = new NodeSchemaRegistry();
      expect(registry.getAll()).toEqual([]);
    });

    it("returns a new array each time (no reference sharing)", () => {
      const registry = new NodeSchemaRegistry([httpNode]);
      const a = registry.getAll();
      const b = registry.getAll();
      expect(a).not.toBe(b);
      expect(a).toEqual(b);
    });
  });

  // ── 4. getByCategory ───────────────────────────────────────────────
  describe("getByCategory", () => {
    it("filters schemas by category", () => {
      const registry = new NodeSchemaRegistry([httpNode, emailNode, customPlugin]);
      const builtins = registry.getByCategory("builtin");
      expect(builtins).toHaveLength(2);
      expect(builtins.map((s) => s.pluginRef)).toContain("builtin/http");
      expect(builtins.map((s) => s.pluginRef)).toContain("builtin/email");
    });

    it("returns plugin category schemas", () => {
      const registry = new NodeSchemaRegistry([httpNode, customPlugin]);
      const plugins = registry.getByCategory("plugin");
      expect(plugins).toHaveLength(1);
      expect(plugins[0].pluginRef).toBe("plugin/slack");
    });

    it("returns empty array when no schemas match the category", () => {
      const registry = new NodeSchemaRegistry([httpNode]);
      expect(registry.getByCategory("custom")).toEqual([]);
    });
  });

  // ── 5. getByNodeKind ───────────────────────────────────────────────
  describe("getByNodeKind", () => {
    it("filters schemas by explicit nodeKind", () => {
      const registry = new NodeSchemaRegistry([httpNode, cronTrigger, capabilityNode]);
      const triggers = registry.getByNodeKind("trigger");
      expect(triggers).toHaveLength(1);
      expect(triggers[0].pluginRef).toBe("builtin/cron");
    });

    it("returns action nodes including those with explicit nodeKind", () => {
      const registry = new NodeSchemaRegistry([httpNode, emailNode, cronTrigger]);
      const actions = registry.getByNodeKind("action");
      expect(actions).toHaveLength(2);
    });

    it("treats undefined nodeKind as 'action' (default)", () => {
      const registry = new NodeSchemaRegistry([implicitActionNode, httpNode]);
      const actions = registry.getByNodeKind("action");
      expect(actions).toHaveLength(2);
      expect(actions.map((s) => s.pluginRef)).toContain("builtin/transform");
      expect(actions.map((s) => s.pluginRef)).toContain("builtin/http");
    });

    it("does not return implicit-action nodes when filtering for trigger", () => {
      const registry = new NodeSchemaRegistry([implicitActionNode, cronTrigger]);
      const triggers = registry.getByNodeKind("trigger");
      expect(triggers).toHaveLength(1);
      expect(triggers[0].pluginRef).toBe("builtin/cron");
    });

    it("filters capability nodes", () => {
      const registry = new NodeSchemaRegistry([httpNode, capabilityNode]);
      const caps = registry.getByNodeKind("capability");
      expect(caps).toHaveLength(1);
      expect(caps[0].pluginRef).toBe("builtin/postgres-cap");
    });
  });

  // ── 6. has ─────────────────────────────────────────────────────────
  describe("has", () => {
    it("returns true for registered schema", () => {
      const registry = new NodeSchemaRegistry([httpNode]);
      expect(registry.has("builtin/http")).toBe(true);
    });

    it("returns false for unregistered schema", () => {
      const registry = new NodeSchemaRegistry();
      expect(registry.has("builtin/http")).toBe(false);
    });

    it("returns true after registering a schema", () => {
      const registry = new NodeSchemaRegistry();
      expect(registry.has("builtin/http")).toBe(false);
      registry.register(httpNode);
      expect(registry.has("builtin/http")).toBe(true);
    });
  });

  // ── 7. loaded ──────────────────────────────────────────────────────
  describe("loaded", () => {
    it("starts as false", () => {
      const registry = new NodeSchemaRegistry();
      expect(registry.loaded).toBe(false);
    });

    it("remains false after manually registering schemas", () => {
      const registry = new NodeSchemaRegistry([httpNode]);
      registry.register(emailNode);
      expect(registry.loaded).toBe(false);
    });
  });

  // ── 8. onUpdate listener ───────────────────────────────────────────
  describe("onUpdate", () => {
    it("calls listener when loadFromServer succeeds", async () => {
      const registry = new NodeSchemaRegistry();
      const listener = vi.fn();
      registry.onUpdate(listener);

      // Mock a successful fetch
      const mockResponse = {
        ok: true,
        json: async () => ({
          data: [
            {
              plugin_ref: "builtin/http",
              name: "HTTP",
              description: "Make HTTP requests",
              category: "builtin",
              icon: "globe",
              color: "#3b82f6",
              inputs: [],
              outputs: [],
            },
          ],
        }),
      };
      globalThis.fetch = vi.fn().mockResolvedValue(mockResponse);

      await registry.loadFromServer("http://localhost:8080");

      expect(listener).toHaveBeenCalledTimes(1);
      expect(registry.loaded).toBe(true);

      // Cleanup
      vi.restoreAllMocks();
    });

    it("supports multiple listeners", async () => {
      const registry = new NodeSchemaRegistry();
      const listener1 = vi.fn();
      const listener2 = vi.fn();
      registry.onUpdate(listener1);
      registry.onUpdate(listener2);

      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ data: [] }),
      });

      await registry.loadFromServer("http://localhost:8080");

      expect(listener1).toHaveBeenCalledTimes(1);
      expect(listener2).toHaveBeenCalledTimes(1);

      vi.restoreAllMocks();
    });
  });

  // ── 9. onUpdate unsubscribe ────────────────────────────────────────
  describe("onUpdate unsubscribe", () => {
    it("removes the listener when unsubscribe is called", async () => {
      const registry = new NodeSchemaRegistry();
      const listener = vi.fn();
      const unsubscribe = registry.onUpdate(listener);

      unsubscribe();

      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ data: [] }),
      });

      await registry.loadFromServer("http://localhost:8080");

      expect(listener).not.toHaveBeenCalled();

      vi.restoreAllMocks();
    });

    it("only removes the specific listener, not others", async () => {
      const registry = new NodeSchemaRegistry();
      const listener1 = vi.fn();
      const listener2 = vi.fn();
      registry.onUpdate(listener1);
      const unsub2 = registry.onUpdate(listener2);

      unsub2();

      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ data: [] }),
      });

      await registry.loadFromServer("http://localhost:8080");

      expect(listener1).toHaveBeenCalledTimes(1);
      expect(listener2).not.toHaveBeenCalled();

      vi.restoreAllMocks();
    });
  });

  // ── onLoadError listener ───────────────────────────────────────────
  describe("onLoadError", () => {
    it("calls error listener when fetch fails", async () => {
      const registry = new NodeSchemaRegistry();
      const errorListener = vi.fn();
      registry.onLoadError(errorListener);

      globalThis.fetch = vi.fn().mockRejectedValue(new Error("Network error"));

      await registry.loadFromServer("http://localhost:8080");

      expect(errorListener).toHaveBeenCalledTimes(1);
      expect(errorListener).toHaveBeenCalledWith(expect.any(Error));
      expect(errorListener.mock.calls[0][0].message).toBe("Network error");

      vi.restoreAllMocks();
    });

    it("calls error listener when response is not ok", async () => {
      const registry = new NodeSchemaRegistry();
      const errorListener = vi.fn();
      registry.onLoadError(errorListener);

      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: false,
        status: 500,
      });

      await registry.loadFromServer("http://localhost:8080");

      expect(errorListener).toHaveBeenCalledTimes(1);
      expect(errorListener.mock.calls[0][0].message).toContain("500");

      vi.restoreAllMocks();
    });

    it("unsubscribes error listener correctly", async () => {
      const registry = new NodeSchemaRegistry();
      const errorListener = vi.fn();
      const unsub = registry.onLoadError(errorListener);

      unsub();

      globalThis.fetch = vi.fn().mockRejectedValue(new Error("Network error"));

      await registry.loadFromServer("http://localhost:8080");

      expect(errorListener).not.toHaveBeenCalled();

      vi.restoreAllMocks();
    });
  });

  // ── 10. resolveIconUrl ─────────────────────────────────────────────
  describe("resolveIconUrl", () => {
    const base = "http://localhost:8080";

    it("returns undefined for undefined url", () => {
      expect(NodeSchemaRegistry.resolveIconUrl(base, undefined)).toBeUndefined();
    });

    it("returns undefined for empty string", () => {
      // empty string is falsy, so the function returns undefined
      expect(NodeSchemaRegistry.resolveIconUrl(base, "")).toBeUndefined();
    });

    it("resolves relative paths by prepending API base URL", () => {
      expect(NodeSchemaRegistry.resolveIconUrl(base, "/icons/globe.svg")).toBe(
        "http://localhost:8080/icons/globe.svg"
      );
    });

    it("resolves relative paths without leading slash", () => {
      expect(NodeSchemaRegistry.resolveIconUrl(base, "icons/globe.svg")).toBe(
        "http://localhost:8080icons/globe.svg"
      );
    });

    it("returns absolute http URLs as-is", () => {
      const url = "http://example.com/icon.png";
      expect(NodeSchemaRegistry.resolveIconUrl(base, url)).toBe(url);
    });

    it("returns absolute https URLs as-is", () => {
      const url = "https://cdn.example.com/icon.png";
      expect(NodeSchemaRegistry.resolveIconUrl(base, url)).toBe(url);
    });

    it("allows data:image URIs (safe)", () => {
      const dataUri = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0c...";
      expect(NodeSchemaRegistry.resolveIconUrl(base, dataUri)).toBe(dataUri);
    });

    it("allows data:image/png URIs", () => {
      const dataUri = "data:image/png;base64,iVBORw0KGgo...";
      expect(NodeSchemaRegistry.resolveIconUrl(base, dataUri)).toBe(dataUri);
    });

    it("blocks javascript: scheme", () => {
      expect(NodeSchemaRegistry.resolveIconUrl(base, "javascript:alert(1)")).toBeUndefined();
    });

    it("blocks javascript: scheme with mixed case", () => {
      expect(NodeSchemaRegistry.resolveIconUrl(base, "JavaScript:alert(1)")).toBeUndefined();
    });

    it("blocks data:text/html (XSS vector)", () => {
      expect(
        NodeSchemaRegistry.resolveIconUrl(base, "data:text/html,<script>alert(1)</script>")
      ).toBeUndefined();
    });

    it("blocks data:text/javascript", () => {
      expect(
        NodeSchemaRegistry.resolveIconUrl(base, "data:text/javascript,alert(1)")
      ).toBeUndefined();
    });

    it("blocks data:application/javascript", () => {
      expect(
        NodeSchemaRegistry.resolveIconUrl(base, "data:application/javascript,alert(1)")
      ).toBeUndefined();
    });

    it("handles whitespace-padded URLs by trimming", () => {
      expect(NodeSchemaRegistry.resolveIconUrl(base, "  /icons/globe.svg  ")).toBe(
        "http://localhost:8080  /icons/globe.svg  "
      );
    });

    it("blocks javascript: with leading whitespace after trim", () => {
      expect(NodeSchemaRegistry.resolveIconUrl(base, "  javascript:alert(1)  ")).toBeUndefined();
    });
  });
});
