import type { NodeTypeDefinition, FieldSchema } from "../types/schema";

/** Safe pattern for plugin_ref values. */
const SAFE_PLUGIN_REF = /^[a-zA-Z0-9_\-.@/:]+$/;

/** Map a field schema from backend (possibly snake_case) to frontend shape. */
function mapFieldSchema(raw: Record<string, unknown>): FieldSchema {
  return {
    key: typeof raw.key === "string" ? raw.key : "",
    label: typeof raw.label === "string" ? raw.label : "",
    type: (typeof raw.type === "string" ? raw.type : "string") as FieldSchema["type"],
    required: typeof raw.required === "boolean" ? raw.required : undefined,
    default: raw.default,
    description: typeof raw.description === "string" ? raw.description : undefined,
    children: Array.isArray(raw.children)
      ? raw.children
          .filter((c): c is Record<string, unknown> => c != null && typeof c === "object")
          .map(mapFieldSchema)
      : undefined,
    enum: Array.isArray(raw.enum) ? (raw.enum as string[]) : undefined,
    credentialType: (typeof (raw.credential_type ?? raw.credentialType) === "string"
      ? (raw.credential_type ?? raw.credentialType)
      : undefined) as string | undefined,
    dynamic: (typeof (raw.is_dynamic ?? raw.dynamic) === "boolean"
      ? (raw.is_dynamic ?? raw.dynamic)
      : undefined) as boolean | undefined,
  };
}

function mapFieldSchemas(raw: unknown): FieldSchema[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .filter((f): f is Record<string, unknown> => f != null && typeof f === "object")
    .map(mapFieldSchema);
}

export class NodeSchemaRegistry {
  private schemas = new Map<string, NodeTypeDefinition>();
  private _loaded = false;
  private _listeners = new Set<() => void>();
  private _errorListeners = new Set<(error: Error) => void>();
  private _snapshot: NodeTypeDefinition[] | null = null;

  constructor(initial: NodeTypeDefinition[] = []) {
    for (const s of initial) this.register(s);
  }

  register(schema: NodeTypeDefinition): void {
    this.schemas.set(schema.pluginRef, schema);
    this._snapshot = null;
  }

  get(pluginRef: string): NodeTypeDefinition | undefined {
    return this.schemas.get(pluginRef);
  }

  getAll(): NodeTypeDefinition[] {
    if (!this._snapshot) {
      this._snapshot = Array.from(this.schemas.values());
    }
    return this._snapshot;
  }

  getByCategory(category: string): NodeTypeDefinition[] {
    return this.getAll().filter((s) => s.category === category);
  }

  getByNodeKind(kind: string): NodeTypeDefinition[] {
    return this.getAll().filter((s) => (s.nodeKind || "action") === kind);
  }

  has(pluginRef: string): boolean {
    return this.schemas.has(pluginRef);
  }

  get loaded(): boolean {
    return this._loaded;
  }

  onUpdate(fn: () => void): () => void {
    this._listeners.add(fn);
    return () => {
      this._listeners.delete(fn);
    };
  }

  onLoadError(fn: (error: Error) => void): () => void {
    this._errorListeners.add(fn);
    return () => {
      this._errorListeners.delete(fn);
    };
  }

  private notify(): void {
    this._snapshot = null;
    for (const fn of this._listeners) fn();
  }

  private notifyError(error: Error): void {
    for (const fn of this._errorListeners) fn(error);
  }

  /**
   * Resolve a relative icon URL (e.g. "/icons/globe.svg") to a full URL
   * using the API base. Absolute URLs (http/https) are returned as-is.
   */
  static resolveIconUrl(apiBaseUrl: string, url: string | undefined): string | undefined {
    return resolveIconUrl(apiBaseUrl, url);
  }

  async loadFromServer(apiBaseUrl: string, authToken?: string): Promise<void> {
    try {
      const headers: Record<string, string> = { "Content-Type": "application/json" };
      if (authToken) {
        headers["Authorization"] = `Bearer ${authToken}`;
      }
      const res = await fetch(`${apiBaseUrl}/node-types`, { headers });
      if (!res.ok) {
        this._loaded = true;
        this.notify();
        this.notifyError(new Error(`Schema load failed: HTTP ${res.status}`));
        return;
      }
      const json = await res.json();
      const raw = json.data as Record<string, unknown>[];
      if (Array.isArray(raw)) {
        // Clear existing schemas so uninstalled plugins are removed
        this.schemas.clear();
        for (const r of raw) {
          const pluginRef = typeof r.plugin_ref === "string" ? r.plugin_ref : "";
          // Validate plugin_ref against safe pattern to prevent prototype pollution
          if (!pluginRef || !SAFE_PLUGIN_REF.test(pluginRef)) continue;

          const docs = typeof r.docs === "string" ? r.docs : undefined;
          // Only allow https:// docs URLs
          const safeDocs = docs && docs.toLowerCase().startsWith("https://") ? docs : undefined;

          // Map snake_case from backend to camelCase frontend types
          const schema: NodeTypeDefinition = {
            pluginRef,
            name: typeof r.name === "string" ? r.name : "",
            description: typeof r.description === "string" ? r.description : "",
            category: (r.category as NodeTypeDefinition["category"]) || "builtin",
            nodeKind: (r.node_kind as NodeTypeDefinition["nodeKind"]) || undefined,
            icon: typeof r.icon === "string" ? r.icon : "default",
            color: typeof r.color === "string" ? r.color : "#6366f1",
            docs: safeDocs,
            imageUrl: resolveIconUrl(apiBaseUrl, typeof r.image_url === "string" ? r.image_url : undefined),
            inputs: mapFieldSchemas(r.inputs),
            outputs: mapFieldSchemas(r.outputs),
            parameters: Array.isArray(r.parameters) ? mapFieldSchemas(r.parameters) : undefined,
            capabilityPorts: (r.capability_ports as NodeTypeDefinition["capabilityPorts"]) || undefined,
            settings: Array.isArray(r.settings) ? mapFieldSchemas(r.settings) : undefined,
            providesCapability: typeof r.provides_capability === "string" ? r.provides_capability : undefined,
          };
          this.register(schema);
        }
      }
      this._loaded = true;
      this.notify();
    } catch (err) {
      // Backend unreachable — mark loaded so consumers fall back to builtin schemas
      this._loaded = true;
      this.notify();
      this.notifyError(err instanceof Error ? err : new Error(String(err)));
    }
  }
}

/** Resolve a relative icon URL to a full URL using the API base.
 *  Strict allowlist: only https://, safe data:image/ URIs, and relative paths. */
function resolveIconUrl(apiBaseUrl: string, url: string | undefined): string | undefined {
  if (!url) return undefined;
  const lower = url.trim().toLowerCase();
  // Allowlist only safe schemes
  if (lower.startsWith("https://")) return url;
  if (
    lower.startsWith("data:image/png;") ||
    lower.startsWith("data:image/jpeg;") ||
    lower.startsWith("data:image/svg+xml;") ||
    lower.startsWith("data:image/webp;") ||
    lower.startsWith("data:image/gif;")
  ) {
    return url;
  }
  // Only allow relative paths starting with /
  if (url.startsWith("/")) return `${apiBaseUrl}${url}`;
  // Reject everything else (javascript:, vbscript:, blob:, data:application/, http://, etc.)
  return undefined;
}
