// Project website (#517, Astro + Starlight since #18). Served by the maintainer's server next to
// /updates/ (docs/release.md, "Website"), whose CSP allows no inline code: stylesheets are never
// inlined, and scripts/no-inline.mjs moves the inline scripts left after the build into files.
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

const SITE = "https://seaglass.bebanana.ch";
const REPO = "https://github.com/BeBanana1/Seaglass";
// Link preview image (#48), written by scripts/assets.mjs; og:image needs an absolute address.
const PREVIEW = `${SITE}/og.jpg`;
const PREVIEW_ALT = "A playlist in Seaglass, a lossless music streaming client";

export default defineConfig({
  site: SITE,
  trailingSlash: "always",
  build: { inlineStylesheets: "never" },
  // tsconfig pinned: without it, Vite looks up the folders for the files Starlight ships and
  // reaches the app's tsconfig.json at the root, whose `extends` needs the app's dependencies
  // (the CI `site` job installs only the site's).
  vite: { tsconfig: "./tsconfig.json", build: { assetsInlineLimit: 0 } },
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
      head: [
        { tag: "meta", attrs: { property: "og:image", content: PREVIEW } },
        { tag: "meta", attrs: { property: "og:image:width", content: "1200" } },
        { tag: "meta", attrs: { property: "og:image:height", content: "630" } },
        { tag: "meta", attrs: { property: "og:image:alt", content: PREVIEW_ALT } },
        { tag: "meta", attrs: { name: "twitter:image", content: PREVIEW } },
        { tag: "meta", attrs: { name: "twitter:image:alt", content: PREVIEW_ALT } },
      ],
      // Pagefind needs 'wasm-unsafe-eval', which the CSP does not allow; a handful of pages
      // need no search anyway.
      pagefind: false,
      social: [{ icon: "github", label: "Source code", href: REPO }],
      editLink: { baseUrl: `${REPO}/edit/main/site/` },
      lastUpdated: false,
      sidebar: [
        { label: "Download", slug: "download" },
        { label: "Installation", slug: "install" },
        // The app demo (#28): built from the app by scripts/publish-site.ps1, not a Starlight page.
        { label: "Try it in your browser", link: "/try/" },
      ],
    }),
  ],
});
