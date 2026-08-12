import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const tauriHost = process.env.TAURI_DEV_HOST;
const devHost = tauriHost || "127.0.0.1";

export default defineConfig({
  clearScreen: false,
  plugins: [react()],
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      "@liberty-contracts": fileURLToPath(new URL("../../packages/shared-types/src/generated", import.meta.url)),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    host: tauriHost || "127.0.0.1",
    hmr: {
      protocol: "ws",
      host: devHost,
      port: 5173,
    },
    watch: {
      ignored: [
        "**/src-tauri/**",
        "**/.venv/**",
        "**/.venv39/**",
        "**/.venv310/**",
        "**/.venv311/**",
        "**/runtime-bundles/**",
      ],
      usePolling: true,
      interval: 120,
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target:
      process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
