import topLevelAwait from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  server: { port: 5173 },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
