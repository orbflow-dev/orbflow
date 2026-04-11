import { defineConfig } from "vitest/config";
import path from "path";

export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
      "@orbflow/core": path.resolve(__dirname, "../../packages/orbflow-core/src"),
      "@orbflow/core/types": path.resolve(__dirname, "../../packages/orbflow-core/src/types"),
      "@orbflow/core/client": path.resolve(__dirname, "../../packages/orbflow-core/src/client"),
      "@orbflow/core/utils": path.resolve(__dirname, "../../packages/orbflow-core/src/utils"),
    },
  },
});
