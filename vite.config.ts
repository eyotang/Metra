import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/node_modules/**"] },
  },
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    minify: "esbuild",
  },
});