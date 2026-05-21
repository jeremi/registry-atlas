import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5177,
    strictPort: false,
    proxy: {
      "/api": "http://127.0.0.1:3717",
    },
  },
  preview: {
    port: 4177,
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./vitest.setup.ts",
    coverage: {
      reporter: ["text", "html"],
    },
  },
});
