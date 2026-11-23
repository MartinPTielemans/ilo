# ilobrowser.com

The website for ilo, the open-source browser, in `site/` of the browser's repository. The
repository's `AGENTS.md` is the browser's and doesn't apply here, nothing here is built into the
browser, and the site is MIT (`LICENSE`) where the browser is GPL. Run every command below in
`site/`.

The site is [TanStack Start](https://tanstack.com/start) (React) on
[Vite+](https://viteplus.dev/guide), whose `vp` CLI runs everything: `vp install`, `vp dev`,
`vp test`, `vp check` (Oxfmt, Oxlint and the type check; `--fix` formats), `vp build` and
`vp preview`. `vp build` prerenders every page, so `dist/client` is static files any host can
serve.

## What we never compromise on

1. **The site is as fast as the browser claims to be.** Every page is prerendered HTML that reads
   in full before any script runs; script only adds the land behind it and the interactions.
   Nothing renders on the client alone, no page loads a script of the framework's, and there are
   no web fonts it can do without and no third-party scripts: no analytics, trackers or chat
   widgets. Video is lazy and muted, and loads only when it's on screen.
2. **Native scrolling.** No smooth-scroll libraries and no scroll-jacking. Everything that moves
   can be interrupted: a click during an animation acts at once.
3. **One goal per page.** The home page exists to get people to download ilo. Every section
   leads there or one layer deeper, and nothing competes with the download button.
4. **One site, not nine pages.** Every page is the same shell (`src/components/shell.tsx`): the
   land, the bar, the foot and the two scripts. A page adds only what is its own, in the shapes
   the others already use (`.top`, `.cols`, `.side`, `.panel`, `.list` in `src/styles.css`), and
   something new on one page is reachable from the bar, the foot or a page that has it. A feature
   no other page leads to or follows from isn't finished.
5. **Show the real product.** Real captures of ilo, never mockups passed off as the app, and
   never another browser's icons, fonts, images or names.

## Traps

- No page runs React in the browser. A component renders once, at build time, so state, effects
  and handlers in it never run. What a page does is in two inline scripts the shell ends it with:
  `PAGE` (`src/page.ts`: the download for the visitor's computer, the index's mark, the
  changelog's fold, filter and clip view), then `src/land.js`. `page` reaches the browser as its
  own source, so it and the functions handed to it use only their arguments and the browser.
- `src/land.js` draws the fields behind every page on one fixed canvas and runs the bar. It
  measures the page after `PAGE` has changed the download's words, clears the crop behind every
  `[data-clear]` element so its text reads, and takes its parts from `main [data-part]`, or slices
  of `main` on a page that marks none. Text on bare ground without `data-clear` is unreadable, and
  anything that changes a page's height after load has to leave the land a `resize` to measure by.
- The index beside a page (`src/components/side.tsx`) scrolls in a list inside its panel. Give a
  scroller that sits over the land no solid ground of its own: headless Chrome blacked out the land
  above one.
- A route file's `component` loads with its page; its `head`, and everything the file imports
  outside the component, loads with every page's head. Data as large as the changelog is imported
  by components only.
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
- `src/changelog.ts` is the changelog. A release is its `changes`, one for each thing a reader
  would name: its `kind` (security, new, changed or fixed), its `area` (the part of ilo, in a name
  an earlier release already uses where one fits), its `text`, `only` the platforms it's for when
  not all (the text then doesn't open with "On the Mac,"), and `clip`, a clip's name when one shows
  it (`tools/clip.sh` makes the clip's three files, which a test checks are there). The page, the
  feed, the home page's version line and each platform's page all read it.
- The changelog shows a release only once it's published: the deploy lists the published versions
  in `ILO_PUBLISHED`, and `isOut` in `src/changelog.ts` reads it. Unset, as in the checks and
  `vp dev`, every dated release shows.
- The stars and contributors on the home page are `src/github.json` and the faces in
  `public/media/contributors`, which `tools/numbers.mjs` rewrites at each deploy and nothing
  commits. What's committed is what shows when GitHub doesn't answer. The site never asks GitHub
  from a visitor's browser.
- `tools/walk.mjs` walks every page in headless Chrome at a desk's width and a phone's, fails on
  what a reader would trip over and leaves pictures of each page. Run it in a check VM or on a
  Linux runner after a change to the shell, the land or a page's layout, and look at the pictures.
- A change to `site/` runs the site's checks and nothing of the browser's
  (`.github/scripts/ci-changes.cjs`). On main it then deploys to Cloudflare
  (`.github/workflows/site.yml`, `wrangler.jsonc`), so main is what ilobrowser.com serves.
  Publishing a release deploys too, with the workflow as it stood at the release's tag, so main's
  `site/` has to build under the last tag's `site.yml` as well as main's.
- The download buttons link to `releases/latest/download/<file>` in the browser repo, with each
  platform's fixed file name from `src/downloads.ts`. GitHub serves whichever release is marked
  Latest, so every other release there (the engine builds) has to be made with `--latest=false`,
  or the buttons 404. The file names are the browser's `release.yml`'s.

## Brand

- The name is **ilo**, lowercase everywhere, even at the start of a sentence. The line under it is
  "a browser you get to keep."
- The look is fields of dots in a Space's colour, with dark panels (`--panel` `#0E0E11`) and white
  text on them. The five Spaces' colours are `SLOT` in `src/land.js`; green, `#07803F`, is the first
  and the one every page opens on. Type is Figtree, with DM Mono for keys and commands.
- ilo is never sold on speed, never set against another browser by name, and isn't an AI
  assistant.

## Commits and pull requests

- Commit only when asked, with `git commit -s`. The `Signed-off-by` line is the owner's DCO
  certification.
- Authorship is human only: no `Co-Authored-By` trailers and no "Generated with" lines.
- Conventional commit titles in plain language, describing the outcome.
- Never open a PR unless asked. The body follows the repository's
  `.github/pull_request_template.md`. A visual change carries before-and-after captures, and
  motion a short video.
