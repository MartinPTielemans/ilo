# A Boost's script in an isolated world

**Question.** A Boost's script would run in an isolated world of its own (#2297's 1.1). Two things need answering before that's built:
- **1.4.** Does the page's Content-Security-Policy govern what the script does: its requests, `eval`, and Trusted Types?
- **1.2.** Which frames does each frame rule pick?

**Answer.** See Results, filled in from the CI run.

## How it's measured

`./run.sh [OUT]` downloads ilo's Linux engine with `engine/fetch.sh`: ungoogled-chromium 156 with ilo's series, as the app runs it. It runs the engine's `chrome` headless with its sandbox on and drives it over DevTools' pipe (`spike.py`, standard library only). A server of the run's own on 127.0.0.1 serves the made-up hosts `a.test`, `sub.a.test` and `b.test`, and logs every request that reaches it. That log, not the page, says whether a request was sent.

- **1.4.** `a.test/strict` sends a strict CSP: `connect-src`, `img-src` and `script-src` `'self'`, no `'unsafe-eval'`, and Trusted Types required. `a.test/open` sends none. On each page the same probes run twice: in the page's own world, and in a world made with `Page.createIsolatedWorld`. Like the world the browser would make, that world has no CSP of its own. The probes are fetch (same origin, another origin with and without CORS), an image, a beacon, `eval`, `new Function`, `innerHTML`, a `<script>` element from another origin and an inline one, a CSS `url()`, and `document.cookie`.
- **1.2.** `a.test/frames` holds `sub.a.test`, `a.test`, a `b.test` frame holding an `a.test` frame, and a `srcdoc` frame. `b.test/embeds-a` holds an `a.test` frame. For each frame, the script's own world reads its origin and `location.ancestorOrigins`, and the table says which frames each rule picks for a Boost of `a.test`.

What it doesn't measure:
- **A world with a CSP of its own.** Blink lets the browser give an isolated world its own CSP, as extensions' content scripts have, and DevTools can't make one. This measures a world with none, as the browser makes one by default.
- **The Mac and Windows.** It's the same Blink. The run is Linux's.

## Results
