import { defineConfig } from "vite";
import { configDefaults } from "vitest/config";
import react from "@vitejs/plugin-react";
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [react()],
  // Keep Rust errors visible; Tauri expects the fixed port.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  // Secondary windows are pages of their own (`src/windows/<name>/`).
  build: {
    rollupOptions: {
      input: { main: "index.html", timer: "src/windows/timer/index.html" },
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    globals: false,
    // React's production build has no `act`; a shell exporting NODE_ENV=production would load it.
    env: { NODE_ENV: "test" },
    // Agent worktrees under .claude/ hold their own copies of the tests.
    exclude: [...configDefaults.exclude, ".claude/**"],
  },
}));
