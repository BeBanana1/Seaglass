import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;
// Source de vérité unique de la version : le fichier VERSION à la racine.
const version = readFileSync(new URL("./VERSION", import.meta.url), "utf8").trim();

// Options adaptées à `tauri dev` : port fixe, pas d'effacement de la console, src-tauri ignoré.
export default defineConfig({
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/spike/**"] },
  },
  build: { target: "es2022" },
});
