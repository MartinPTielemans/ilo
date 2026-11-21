# ilo-site

The website for ilo, the open-source Arc-style browser. The browser itself is
[MartinPTielemans/ilo](https://github.com/MartinPTielemans/ilo); its `AGENTS.md`
doesn't apply here, and nothing here is built into the browser.

The site is [TanStack Start](https://tanstack.com/start) (React) on
[Vite+](https://viteplus.dev/guide), whose `vp` CLI runs everything: `vp install`, `vp dev`,
`vp test`, `vp check` (Oxfmt, Oxlint and the type check; `--fix` formats), `vp build` and
`vp preview`. `vp build` prerenders every page, so `dist/client` is static files any host can
serve.

## What we never compromise on

1. **The site is as fast as the browser claims to be.** Every page is prerendered HTML that reads
   in full before any script runs; script only adds the interactions. Nothing renders on the
   client alone, nothing joins the client bundle the page can do without, and there are no web
   fonts it can do without and no third-party scripts: no analytics, trackers or chat widgets.
   Video is lazy and muted, and loads only when it's on screen.
2. **Native scrolling.** No smooth-scroll libraries and no scroll-jacking. Everything that moves
   can be interrupted: a click during an animation acts at once.
3. **One goal per page.** The home page exists to get people to download ilo. Every section
   leads there or one layer deeper, and nothing competes with the download button.
4. **Show the real product.** Real captures of ilo, never mockups passed off as the app, and
   never Arc's or Dia's icons, fonts, images or names.

## Traps

- A component's first render is the prerendered page, so its starting state is what a visitor
  sees before hydration, and what React expects to find. Read the window, the scroll or media
  queries in effects and handlers only.
- `src/entrances.ts` adds its classes outside React, which keeps them only on elements whose
  `className` React never changes. Listing one whose class follows state loses its entrance.
- On a wide screen with a mouse the home page opens inside ilo: its head script (`WILL_LIVE` in
  `src/live.ts`) marks `<html>` `will-live` before the first paint, the body stops scrolling, ilo's
  interface draws around the page, and `.pitch` is the scroller. Whatever watches or sets the scroll
  asks `scroller()` for it (null is the window), and sizes inside `.pitch` are the container's
  (`cqw`), not the viewport's. `src/live.ts` adds its canvas, its frames, `data-live` and
  `--px`, `--pw`, `--ph` outside React, so React renders none of them and never sets `style` or
  those attributes on `.stage` or `.pages`.
- Only the home page runs React in the browser: a route does when it sets
  `staticData: { hydrates: true }`. Every other page ships as its HTML and one inline script,
  `src/reading.ts`, so state, effects and handlers in its components never run, and what such a page
  does goes in that script. The script reaches the page as its own source: `reading` and the
  functions handed to it use only their arguments and the browser.
- A route file's `component` loads with its page; its `head`, and everything the file imports
  outside the component, loads with the home page's script. Data as large as the changelog is
  imported by components only.
- The build's crawler takes every link that starts with `/` as a page to render and list in the
  sitemap, `/help#part` as one of its own. Link to the page alone, and give a file the build writes
  an entry in `pages` in `vite.config.ts` that turns both off, as the feed has.
- `/changelog.xml` is an Atom feed the build writes from `src/changelog.ts` (`src/feed.ts`). A
  release's `date` reads "29 October 2026", and the build fails on any other shape.
- `src/routeTree.gen.ts` is written by the router plugin on every `vp dev` and `vp build`.
  Commit it, or a fresh clone fails the type check before its first build.
- Files in `public/` are served from the root, so they're referenced as `/media/…`.
- Every page's head comes from `pageHead` in `src/seo.ts`: its title, description, canonical
  address and link preview. A new page that skips it shows the home page's preview when shared, and
  link scrapers take only absolute URLs, so the share image is named with the site's origin.
  `vp build` writes `sitemap.xml` from the pages it prerenders.
- `public/llms.txt` is the site's summary for AI answer engines, written by hand: a new page, a new
  platform or a change in what ilo needs goes there too. The answers in `src/faq.ts` reach search
  engines as structured data, so each has to stand alone and say no more than the site does.
- `public/demo` is the browser repo's build of the live interface (GPL), committed as built and
  left out of fmt and lint. Publishing a release in the browser repo replaces it:
  `.github/workflows/release.yml` takes the release's `ilo-demo.tar.gz`, points
  `public/demo/SOURCE` at the tag's commit, which the home page's fine print links to as the GPL's
  offer of source, commits that to main as github-actions and deploys. Swap it by hand only in the same
  shape: the files, the licences and SOURCE's commit in one commit.
- Every push to main deploys to Cloudflare (`.github/workflows/deploy.yml`, `wrangler.jsonc`) once
  `vp check`, `vp test` and `vp build` pass, so main is always what ilobrowser.com serves.
- The download buttons link to `releases/latest/download/<file>` in the browser repo, with each
  platform's fixed file name from `src/downloads.ts`. GitHub serves whichever release is marked
  Latest, so every other release there (the engine builds) has to be made with `--latest=false`,
  or the buttons 404. The file names are the browser's `release.yml`'s.

## Brand

- The name is **ilo**, lowercase everywhere, even at the start of a sentence. The tagline is
  "a good tool."
- Palette: Paper `#F3F0E8`, Ink `#16140F`, Signal green `#0E9F55`.

## Commits and pull requests

- Commit only when asked, with `git commit -s`. The `Signed-off-by` line is the owner's DCO
  certification.
- Authorship is human only: no `Co-Authored-By` trailers and no "Generated with" lines.
- Conventional commit titles in plain language, describing the outcome.
- Never open a PR unless asked. The body follows `.github/pull_request_template.md`. A visual
  change carries before-and-after captures, and motion a short video.
