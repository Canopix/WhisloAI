import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

// Multi-page setup:
// - settings.html is the React SPA entry (processed by Vite + React plugin)
// - All other windows (index, widget, anchor) and shared assets live in /public
//   and are copied verbatim to dist/ by Vite.
export default defineConfig({
  plugins: [react()],
  // Tauri expects the dev server on this port (matches tauri.conf.json devUrl)
  server: {
    port: 4173,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        settings: resolve(__dirname, "settings.html"),
      },
      output: {
        // Flatten the entry HTML so it lands at dist/settings.html
        // (matches what Tauri expects and what the other windows reference).
        entryFileNames: "assets/settings-[hash].js",
        chunkFileNames: "assets/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
});
