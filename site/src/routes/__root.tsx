import {
  Asset,
  HeadContent,
  Scripts,
  createRootRoute,
  useMatches,
  useTags,
} from "@tanstack/react-router";
import type { ReactNode } from "react";
import { FEED, SITE_META } from "../seo";
import css from "../styles.css?url";

export const Route = createRootRoute({
  // Each page adds its own title, description and canonical address with pageHead (src/seo.ts).
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { name: "theme-color", content: "#16140F" },
      ...SITE_META,
    ],
    links: [
      // The .ico is for browsers and crawlers that take no SVG, and those that ask for it unbidden.
      { rel: "icon", href: "/favicon.ico", sizes: "32x32" },
      { rel: "icon", href: "/favicon.svg", type: "image/svg+xml" },
      { rel: "apple-touch-icon", href: "/apple-touch-icon.png" },
      {
        rel: "preload",
        href: "/fonts/outfit.woff2",
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

// A page gets React in the browser only if its route says it needs it (staticData.hydrates), as the
// home page does. Any other is finished as HTML: it gets no script of the framework's and no
// preload of one, and what little it does is its own few lines (src/reading.ts).
function Document({ children }: { children: ReactNode }) {
  const hydrates = useMatches({
    select: (matches) => matches.some((match) => match.staticData.hydrates),
  });
  return (
    // The home page's script in the head marks the document before React takes it over
    // (WILL_LIVE in src/live.ts), which is no mismatch to report.
    <html lang="en" suppressHydrationWarning>
      <head>{hydrates ? <HeadContent /> : <Head />}</head>
      <body>
        {children}
        {hydrates && <Scripts />}
        {!hydrates && import.meta.env.DEV && (
          <script type="module" dangerouslySetInnerHTML={{ __html: RELOAD }} />
        )}
      </body>
    </html>
  );
}

// While developing, a page without React still reloads when the site's source changes: the dev
// server says so (vite.config.ts).
const RELOAD = `import { createHotContext } from "/@vite/client";
createHotContext("/reading").on("ilo:changed", () => location.reload());`;

/** HeadContent without the scripts it would have the browser fetch ahead. */
function Head() {
  return useTags()
    .filter((tag) => !(tag.tag === "link" && tag.attrs?.rel === "modulepreload"))
    .map((tag) => <Asset {...tag} key={JSON.stringify(tag)} />);
}
