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
    rollupOptions: {
      output: {
        // Review P2-4: pull @codemirror/* (a small eager shell slice plus the
        // whole lazy editor renderer) into one parallel chunk so the index
        // chunk clears the 500 KiB Vite warning. react-aria-components was
        // evaluated and left out: the module inventory (2026-08-31) shows it
        // is not statically imported by the index chunk, and a top-level
        // chunk would either count toward the shell budget (filename-lane
        // gate in scripts/bundle-budget.mjs) or drag lazy-only modules into
        // startup.
        manualChunks(id: string) {
          if (id.includes("/node_modules/@codemirror/")) return "codemirror";
          // Plan 108 task 8: the shared AG-UI state/relay modules are
          // chat-lane machinery (one daemon stream); naming the chunk keeps
          // the filename-lane budget gate from counting them as startup
          // shell when a second lazy surface (CodingAgentPanel) imports them.
          if (/[\\/]frontend[\\/]src[\\/]agent[\\/]/.test(id)) return "chat-agent-core";
        },
      },
    },
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
    testTimeout: 15000,
  },
});
