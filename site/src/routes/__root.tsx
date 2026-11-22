import { Asset, createRootRoute, useTags } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { FEED, SITE_META } from "../seo";
import css from "../styles.css?url";

export const Route = createRootRoute({
  // Each page adds its own title, description and canonical address with pageHead (src/seo.ts).
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      // The first Space's ground, until the land says which the page is on (src/land.js).
      { name: "theme-color", content: "#07803F" },
      ...SITE_META,
    ],
    links: [
      // The .ico is for browsers and crawlers that take no SVG, and those that ask for it unbidden.
      { rel: "icon", href: "/favicon.ico", sizes: "32x32" },
      { rel: "icon", href: "/favicon.svg", type: "image/svg+xml" },
      { rel: "apple-touch-icon", href: "/apple-touch-icon.png" },
      {
        rel: "preload",
        href: "/fonts/figtree.woff2",
        as: "font",
        type: "font/woff2",
        crossOrigin: "anonymous",
      },
      { rel: "stylesheet", href: css },
      // The changelog as a feed, which a feed reader finds from any page.
      { rel: "alternate", type: "application/atom+xml", title: "ilo's changelog", href: FEED },
    ],
  }),
  shellComponent: Document,
});

// Every page is finished as HTML: none gets React in the browser, a script of the framework's or a
// preload of one. What a page does is its own two scripts, which the shell ends it with
// (src/components/shell.tsx).
function Document({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <head>
        <Head />
      </head>
      <body>
        {children}
        {import.meta.env.DEV && (
          <script type="module" dangerouslySetInnerHTML={{ __html: RELOAD }} />
        )}
      </body>
    </html>
  );
}

// While developing, a page reloads when the site's source changes: the dev server says so
// (vite.config.ts).
const RELOAD = `import { createHotContext } from "/@vite/client";
createHotContext("/page").on("ilo:changed", () => location.reload());`;

/** HeadContent without the scripts it would have the browser fetch ahead. */
function Head() {
  return useTags()
    .filter((tag) => !(tag.tag === "link" && tag.attrs?.rel === "modulepreload"))
    .map((tag) => <Asset {...tag} key={JSON.stringify(tag)} />);
}
