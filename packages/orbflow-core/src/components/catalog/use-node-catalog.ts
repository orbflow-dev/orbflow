import { useMemo } from "react";
import { useOrbflow } from "../../context/orbflow-provider";
import type { NodeKind, NodeTypeDefinition } from "../../types/schema";

export interface KindMeta {
  label: string;
  icon: string;
  order: number;
}

export const KIND_META: Record<string, KindMeta> = {
  trigger: { label: "Triggers", icon: "zap", order: 0 },
  action: { label: "Actions", icon: "play", order: 1 },
  capability: { label: "Connections", icon: "link", order: 2 },
};

export interface UseNodeCatalogOptions {
  search?: string;
  kindFilter?: NodeKind | "all";
  allowedKinds?: NodeKind[];
}

export interface CatalogGroup {
  kind: string;
  meta: KindMeta;
  schemas: NodeTypeDefinition[];
}

export interface NodeCatalogResult {
  /** Flat filtered list of schemas */
  schemas: NodeTypeDefinition[];
  /** Grouped by kind, sorted by KIND_META.order */
  grouped: CatalogGroup[];
  /** Total count of filtered results */
  totalCount: number;
}

const DEFAULT_KIND_META: KindMeta = { label: "Other", icon: "layers", order: 99 };

export function useNodeCatalog(options?: UseNodeCatalogOptions): NodeCatalogResult {
  const { registry, schemasReady } = useOrbflow();

  const allSchemas = useMemo(() => registry.getAll(), [registry, schemasReady]);

  const schemas = useMemo(() => {
    let list = allSchemas;

    // Filter by allowedKinds if provided
    const allowedKinds = options?.allowedKinds;
    if (allowedKinds && allowedKinds.length > 0) {
      list = list.filter((s) => {
        const kind = s.nodeKind || "action";
        return allowedKinds.includes(kind as NodeKind);
      });
    }

    // Filter by kindFilter (unless "all")
    const kindFilter = options?.kindFilter;
    if (kindFilter && kindFilter !== "all") {
      list = list.filter((s) => (s.nodeKind || "action") === kindFilter);
    }

    // Filter by search (case-insensitive substring match)
    const search = options?.search?.trim().toLowerCase();
    if (search) {
      list = list.filter(
        (s) =>
          s.name.toLowerCase().includes(search) ||
          s.pluginRef.toLowerCase().includes(search) ||
          s.description.toLowerCase().includes(search),
      );
    }

    return list;
  }, [allSchemas, options?.search, options?.kindFilter, options?.allowedKinds]);

  const grouped = useMemo(() => {
    const groups: Record<string, NodeTypeDefinition[]> = {};

    for (const schema of schemas) {
      const kind = schema.nodeKind || "action";
      if (!groups[kind]) {
        groups[kind] = [];
      }
      groups[kind].push(schema);
    }

    const result: CatalogGroup[] = Object.entries(groups)
      .map(([kind, kindSchemas]) => ({
        kind,
        meta: KIND_META[kind] ?? DEFAULT_KIND_META,
        schemas: kindSchemas,
      }))
      .sort((a, b) => a.meta.order - b.meta.order);

    return result;
  }, [schemas]);

  return {
    schemas,
    grouped,
    totalCount: schemas.length,
  };
}
