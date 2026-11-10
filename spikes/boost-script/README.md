# A Boost's script in an isolated world

**Question.** A Boost's script would run in an isolated world of its own (#2297's 1.1). Two things need answering before that's built:
- **1.4.** Does the page's Content-Security-Policy govern what the script does: its requests, `eval`, and Trusted Types?
- **1.2.** Which frames does each frame rule pick?

**Answer.**
- **1.4.** Yes, for everything but `eval`: a world with no CSP of its own sends under the page's CSP and its Trusted Types. Most pages send no CSP, though, so there the script reaches any origin.
- **1.2.** For scripts, take the rule that needs the frame's site, the top's and every ancestor's to be the Boost's. Today's CSS rule would run the script in the site's frames on other sites' pages.

## How it's measured

`./run.sh [OUT]` runs the app as `check.sh` does on Linux, from `app/build/Browser`, which `app/setup.sh` and `app/build.sh` make on the engine `engine/fetch.sh` downloads: ungoogled-chromium 156 with ilo's series. The sandbox stays on. It runs under Xvfb and openbox, and `spike.py` (standard library only) drives it over DevTools' pipe, as `check/cdp.py` does. A server of the run's own on 127.0.0.1 serves the made-up hosts `a.test`, `sub.a.test` and `b.test`, and logs every request that reaches it. That log, not the page, says whether a request was sent.

- **1.4.** `a.test/strict` sends a strict CSP: `connect-src`, `img-src` and `script-src` `'self'`, no `'unsafe-eval'`, and Trusted Types required. `a.test/open` sends none. On each page the same probes run twice: in the page's own world, and in a world made with `Page.createIsolatedWorld`. Like the world the browser would make, that world has no CSP of its own. The probes are fetch (same origin, another origin with and without CORS), an image, a beacon, `eval`, `new Function`, `innerHTML`, a `<script>` element from another origin and an inline one, a CSS `url()`, and `document.cookie`.
- **1.2.** `a.test/frames` holds `sub.a.test`, `a.test`, a `b.test` frame holding an `a.test` frame, and a `srcdoc` frame. `b.test/embeds-a` holds an `a.test` frame. For each frame, the script's own world reads its origin and `location.ancestorOrigins`, and the table says which frames each rule picks for a Boost of `a.test`.

What it doesn't measure:
- **A world with a CSP of its own.** Blink lets the browser give an isolated world its own CSP, as extensions' content scripts have, and DevTools can't make one. This measures a world with none, as the browser makes one by default.
- **The Mac and Windows.** It's the same Blink. The run is Linux's.

## Results

From [run 42797965331](https://github.com/MartinPTielemans/ilo/actions/runs/42797965331), Chrome/156.0.8078.79 on Linux. "Reached" means the run's server logged the request.

**The sandbox was on.** All nine renderers were in a user namespace of their own under seccomp (mode 2). The tenth line in `results.json`, in init's namespace with no seccomp, is the probe's own `sh`, whose command line `pgrep -f` matched.

**1.4. The page's CSP governs the isolated world, except for `eval`.**

| Probe | Strict, page's world | Strict, isolated world | Open, both worlds |
| --- | --- | --- | --- |
| fetch, same origin | reached | reached | reached |
| fetch, another origin (no-cors, CORS) | blocked | blocked | reached |
| image, another origin | not reached | not reached | reached |
| CSS `url()`, another origin | not reached | not reached | reached |
| `<script src>`, another origin | Trusted Types refuse it | Trusted Types refuse it | reached, ran |
| inline `<script>` | Trusted Types refuse it | Trusted Types refuse it | ran |
| `innerHTML` | Trusted Types refuse it | Trusted Types refuse it | works |
| `eval`, `new Function` | CSP refuses them | **work** | work |
| `document.cookie` | reads the page's | reads the page's | reads the page's |
| beacon, another origin | queued, not reached | queued, not reached | queued, not reached |

- **Requests.** A world with no CSP of its own sends under the page's: `connect-src`, `img-src` and `style-src` stop it reaching another origin, as they stop the page. On a page with no CSP, which is most pages, it reaches any origin, with the page's cookies on the same origin.
- **Trusted Types.** They bind the isolated world too, so a script that sets `innerHTML` breaks on a page that requires them. It has to build nodes.
- **`eval`.** It's the one thing the isolated world is exempt from.
- **Beacons are unmeasured.** `sendBeacon` queued each one and returned true, but none reached the server, even from the open page. They most likely went after the run read its log.
- **What follows for #2297.**
  - The dialog's "can send what it reads to other sites" is true of almost every page, so the consent has to carry it. The page's CSP narrows it only where the site sends one.
  - Giving the world a CSP of its own would close requests to other origins on every page, as extensions' content scripts are closed. Blink supports that, but DevTools can't make it, so it's unmeasured here. It's the follow-up worth a spike if the owner wants scripts unable to send anywhere.

**1.2. Which frames each rule picks, for a Boost of `a.test`.**

| Frame (ancestors) | Out of process | CSS today: the frame's own site | Top's site and the frame's | Every ancestor's too |
| --- | --- | --- | --- | --- |
| `a.test` top | no | yes | yes | yes |
| `a.test/child` (in `a.test`) | no | yes | yes | yes |
| `sub.a.test/child` (in `a.test`) | no | yes | yes | yes |
| `b.test/nest` (in `a.test`) | yes | no | no | no |
| `a.test/leaf` (in `b.test` in `a.test`) | yes | yes | yes | **no** |
| `a.test/child` (in `b.test` top) | yes | **yes** | no | no |
| `about:srcdoc` (in `a.test`) | no | no | no | no |

- **Today's CSS rule runs in `a.test` wherever it's framed**, including on another site's page. For CSS that's harmless. For a script it's the user's code acting as them inside a page another site chose and can talk to.
- **The two stricter rules differ in one frame:** `a.test` with another site between it and an `a.test` top.
- **"Every ancestor's too" is the rule that leaves no other site in the chain.** It costs nothing in this run but that odd nesting, so it's the one to take for scripts. Ancestors come from `location.ancestorOrigins`, which Blink fills in the renderer; the browser should decide from the frame tree it holds instead.
- **The `srcdoc` frame read its origin as `null`**, not its parent's, so no rule picked it. That's unexplained here. It's worth checking before a rule depends on a `srcdoc` frame's origin.
