"use client";

import {
  createContext,
  useContext,
  useMemo,
  useEffect,
  useState,
  useCallback,
  type ReactNode,
} from "react";
import { NodeSchemaRegistry } from "../schemas/registry";
import type { NodeTypeDefinition } from "../types/schema";
import type { OrbflowTheme } from "../styles/theme";
import type { Workflow, TestNodeResult } from "../types/api";

export interface OrbflowConfig {
  apiBaseUrl: string;
  /** Getter for the bearer token — avoids storing the raw token in React context / DevTools. */
  getAuthToken?: () => string | undefined;
  theme?: Partial<OrbflowTheme>;
  readOnly?: boolean;
  nodeSchemas?: NodeTypeDefinition[];
  onSave?: (workflow: Partial<Workflow>) => Workflow | void | Promise<Workflow | void>;
  onRun?: (workflow: Partial<Workflow>) => string | void | Promise<string | void>;
  onTestNode?: (workflow: Partial<Workflow>, nodeId: string) => TestNodeResult | void | Promise<TestNodeResult | void>;
  onChange?: (workflow: Partial<Workflow>) => void;
}

interface OrbflowContextValue {
  config: OrbflowConfig;
  registry: NodeSchemaRegistry;
  schemasReady: boolean;
  /** Re-fetch node schemas from the backend (e.g. after plugin install/uninstall). */
  refreshSchemas: () => Promise<void>;
}

const OrbflowContext = createContext<OrbflowContextValue | null>(null);

export function OrbflowProvider({
  config,
  children,
}: {
  config: OrbflowConfig;
  children: ReactNode;
}) {
  const registry = useMemo(() => {
    const reg = new NodeSchemaRegistry();
    if (config.nodeSchemas) {
      for (const schema of config.nodeSchemas) {
        reg.register(schema);
      }
    }
    return reg;
  }, [config.nodeSchemas]);

  // If local schemas were pre-populated, start as ready
  const [schemasReady, setSchemasReady] = useState(
    () => config.nodeSchemas != null && config.nodeSchemas.length > 0,
  );

  useEffect(() => {
    const unsub = registry.onUpdate(() => setSchemasReady(true));
    registry.loadFromServer(config.apiBaseUrl, config.getAuthToken?.());
    return unsub;
  }, [registry, config.apiBaseUrl, config.getAuthToken]);

  const refreshSchemas = useCallback(
    () => registry.loadFromServer(config.apiBaseUrl, config.getAuthToken?.()),
    [registry, config.apiBaseUrl, config.getAuthToken],
  );

  const value = useMemo(
    () => ({ config, registry, schemasReady, refreshSchemas }),
    [config, registry, schemasReady, refreshSchemas]
  );

  return (
    <OrbflowContext.Provider value={value}>{children}</OrbflowContext.Provider>
  );
}

export function useOrbflow(): OrbflowContextValue {
  const ctx = useContext(OrbflowContext);
  if (!ctx) throw new Error("useOrbflow must be used within <OrbflowProvider>");
  return ctx;
}
