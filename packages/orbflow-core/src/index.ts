/**
 * @orbflow/core — Headless workflow builder SDK.
 *
 * Zero CSS. Pure logic, types, stores, hooks, and utilities.
 * Consumer apps import this library and apply their own styling.
 */

// ── Types ────────────────────────────────────────────
export * from "./types/schema";
export * from "./types/api";

// ── Schemas ──────────────────────────────────────────
export { NodeSchemaRegistry } from "./schemas/registry";
export * from "./schemas/builtin";

// ── Utilities ────────────────────────────────────────
export * from "./utils/cel-builder";
export * from "./utils/upstream";
export * from "./utils/auto-layout";
export * from "./utils/node-slug";

// ── API Client ───────────────────────────────────────
export { createApiClient } from "./client/api-client";
export type { ApiClient } from "./client/api-client";

// ── Execution Constants ──────────────────────────────
export * from "./execution/constants";

// ── Theme ────────────────────────────────────────────
export * from "./styles/theme";

// ── Context ──────────────────────────────────────────
export { OrbflowProvider, useOrbflow } from "./context/orbflow-provider";
export type { OrbflowConfig } from "./context/orbflow-provider";
export { useThemeState } from "./context/theme-context";
export type { ThemeMode, ThemeState } from "./context/theme-context";

// ── Stores (re-exported for direct access) ───────────
export * from "./stores";

// ── Hooks ────────────────────────────────────────────
export * from "./hooks";

// ── Components ──────────────────────────────────────
export * from "./components";
