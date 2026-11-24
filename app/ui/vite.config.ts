import { resolve } from "node:path";
import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  // 出力は app/dist-web（src-tauri/tauri.conf.json の frontendDist）。メイン画面、設定画面、このソフトについての 3 ページ。
  build: {
    outDir: "../dist-web",
    emptyOutDir: true,
    target: "es2022",
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "index.html"),
        settings: resolve(import.meta.dirname, "settings.html"),
        about: resolve(import.meta.dirname, "about.html"),
      },
    },
  },
});
