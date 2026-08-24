import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port; production assets use relative paths so
// they resolve under the bundled custom protocol.
export default defineConfig({
  plugins: [react()],
  base: "./",
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "es2022",
    sourcemap: false,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    // DOM component tests opt in per-file; node keeps reducer tests fast.
    environmentMatchGlobs: [
      ["src/test/**/*.test.tsx", "jsdom"],
      ["src/editor/**/*.test.ts", "jsdom"],
      ["src/command-centre/**/*.test.tsx", "jsdom"],
      ["src/settings/**/*.test.tsx", "jsdom"],
      ["src/chat/**/*.test.tsx", "jsdom"],
      ["src/agent/**/*.test.ts", "jsdom"],
    ],
    setupFiles: ["src/test/setup.ts"],
  },
});
