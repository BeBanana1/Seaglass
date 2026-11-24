// Project website (#517, Astro + Starlight since #18). Served by the maintainer's server next to
// /updates/ (docs/release.md, "Website"), whose CSP allows no inline code: stylesheets are never
// inlined, and scripts/no-inline.mjs moves the inline scripts left after the build into files.
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

const REPO = "https://github.com/BeBanana1/Seaglass";

export default defineConfig({
  site: "https://seaglass.bebanana.ch",
  trailingSlash: "always",
  build: { inlineStylesheets: "never" },
  vite: { build: { assetsInlineLimit: 0 } },
  integrations: [
    starlight({
      title: "Seaglass",
      description:
        "Lossless music streaming client for Windows, Android and Linux, with bit-perfect playback.",
      defaultLocale: "root",
      locales: { root: { label: "English", lang: "en" } },
      logo: { src: "./src/assets/seaglass.svg", alt: "Seaglass" },
      favicon: "/favicon.png",
      customCss: ["./src/styles/custom.css"],
      // Pagefind needs 'wasm-unsafe-eval', which the CSP does not allow; a handful of pages
      // need no search anyway.
      pagefind: false,
      social: [{ icon: "github", label: "Source code", href: REPO }],
      editLink: { baseUrl: `${REPO}/edit/main/site/` },
      lastUpdated: false,
      sidebar: [{ label: "Download", slug: "download" }],
    }),
  ],
});
