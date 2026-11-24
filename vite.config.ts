/// <reference types="vitest/config" />
import { readFileSync } from "node:fs";
import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;
// Single source of truth for the version: the VERSION file at the root.
const version = readFileSync(new URL("./VERSION", import.meta.url), "utf8").trim();

/** Website demo (#28): `src/lib/mock/fixtures.json` holds the maintainer's library and real
 *  catalog data, so in demo mode it loads as `null` and the mock falls back to the invented
 *  catalog of `mock/demo.ts`. The file's contents are never read. */
function noFixtures(): Plugin {
  const empty = "\0seaglass-no-fixtures";
  return {
    name: "seaglass-demo-no-fixtures",
    enforce: "pre",
    // Resolved to a virtual module: the JSON plugin never sees the real file.
    resolveId(source) {
      if (source.replace(/\\/g, "/").split("?")[0].endsWith("mock/fixtures.json")) return empty;
      if (/(^|\/)fixtures\.json(\?|$)/.test(source)) return empty;
    },
    load(id) {
      if (id === empty) return "export default null;";
    },
  };
}

// Options suited to `tauri dev`: fixed port, no console clearing, src-tauri ignored.
export default defineConfig(({ mode }) => {
  const demo = mode === "demo";
  return {
    plugins: [svelte(), ...(demo ? [noFixtures()] : [])],
    define: { __APP_VERSION__: JSON.stringify(version), __DEMO__: JSON.stringify(demo) },
    clearScreen: false,
    server: {
      port: 1420,
      strictPort: true,
      host: host || "127.0.0.1",
      hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
      watch: { ignored: ["**/src-tauri/**", "**/spike/**"] },
    },
    build: {
      target: "es2022",
      // The demo is served under `img-src 'self'`: the covers stay files, never `data:`.
      ...(demo ? { assetsInlineLimit: 0 } : {}),
      // Svelte runtime and Tauri API in their own file (#443): the app alone went past Vite's
      // 500 kB warning. No lazy loading of the views: their CSS would come after
      // `themes/*.css` and break the theme cascade (docs/themes.md).
      rolldownOptions: {
        output: {
          codeSplitting: { groups: [{ name: "vendor", test: /node_modules/ }] },
        },
      },
    },
    // Frontend tests (#264): pure logic only, without a simulated DOM (`pnpm test`).
    test: { include: ["src/**/*.test.ts"], environment: "node" },
  };
});
