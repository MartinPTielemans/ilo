import { readFileSync } from "node:fs";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";
import { changelogFeed } from "./src/feed";
import { FEED } from "./src/seo";

// The versions published on GitHub, which the deploy lists (.github/workflows/site.yml), so a
// changelog entry shows once its release is out. Unset, as in the checks, every dated entry shows.
const PUBLISHED = process.env.ILO_PUBLISHED;

export default defineConfig({
  plugins: [
    // Every page is rendered to HTML at build time, so the site is static files any host can serve.
    tanstackStart({
      prerender: { enabled: true, crawlLinks: true },
      // Cloudflare serves 404.html for any address with no file (wrangler.jsonc).
      pages: [
        { path: "/404", prerender: { outputPath: "/404.html" }, sitemap: { exclude: true } },
        // The feed is a file the build writes below, not a page to render or to list.
        { path: FEED, prerender: { enabled: false }, sitemap: { exclude: true } },
      ],
      // Lists every prerendered page in dist/client/sitemap.xml, which robots.txt points to.
      sitemap: { enabled: true, host: "https://ilobrowser.com" },
    }),
    viteReact(),
    // The changelog's feed, a file beside the pages, which the dev server answers with too.
    {
      name: "ilo:changelog-feed",
      generateBundle() {
        if (this.environment.name !== "client") return;
        const source = changelogFeed(undefined, PUBLISHED);
        this.emitFile({ type: "asset", fileName: FEED.slice(1), source });
      },
      configureServer(server) {
        server.middlewares.use(FEED, (_, response) => {
          response.setHeader("content-type", "application/atom+xml; charset=utf-8");
          response.end(changelogFeed(undefined, PUBLISHED));
        });
      },
    },
    // Only the home page runs React in the browser, so only it hears of changes by itself. This
    // tells the other pages, which reload (src/routes/__root.tsx).
    {
      name: "ilo:reload",
      configureServer(server) {
        server.watcher.on("change", (file) => {
          if (file.includes("/src/")) server.ws.send({ type: "custom", event: "ilo:changed" });
        });
      },
    },
  ],
  // ILO_PUBLISHED reaches the pages as import.meta.env.ILO_PUBLISHED, unset when it's unset.
  envPrefix: ["VITE_", "ILO_PUBLISHED"],
  // The live interface is the browser repo's build in public/demo. Its SOURCE file names the commit
  // it was built from, and the hero's caption links there, as the GPL asks of a site that serves it.
  define: {
    "import.meta.env.ILO_DEMO_SOURCE": JSON.stringify(
      readFileSync(new URL("./public/demo/SOURCE", import.meta.url), "utf8").match(
        /https:\/\/\S+/,
      )?.[0],
    ),
  },
  // The router plugin writes the route tree on every build, and public/demo is the browser repo's
  // build of the live interface.
  fmt: { ignorePatterns: ["src/routeTree.gen.ts", "public/demo/**"] },
  lint: {
    ignorePatterns: ["src/routeTree.gen.ts", "public/demo/**"],
    options: { typeAware: true, typeCheck: true },
  },
});
