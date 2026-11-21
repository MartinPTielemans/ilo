# A site as a read-only API that feeds a live page

**Question (#3810).** Can an agent turn a site into a named, read-only API that the browser then serves by itself, and can a page that lives in the browser show that data live?

**Answer.** Yes, for sites whose data arrives by GET, which was four of the five tried. It took a definition of 250 to 500 bytes with no code in it. The runner reads the site in a hidden tab on the profile's cookies. It sends only GETs, only to the definition's site, and checks the answer's shape, so a changed site shows as broken and never as wrong data. A sealed page shows the data and can't send it anywhere, with one exception the web platform can't close: WebRTC. ilo's core would have to close that. Sites whose data comes back from a POST can't be served, and many current apps work that way. ilo's agent tools also lack two things an agent needs to write these definitions: the list of a page's network requests, and a way to try a definition.

## Run it

```sh
node spikes/site-api/demo.mjs     # then open the two addresses it prints
node spikes/site-api/test.mjs     # the parameter, runner and seal tests
node spikes/site-api/bench.mjs    # the numbers below (Linux, for its memory column)
```

You need Node 22 or later and a Chromium-based browser. The scripts find the usual install on a Mac or Linux; `CHROME=/path/to/binary` names another. The demo starts that browser headless on a throwaway profile, a local test site on port 8811 and the host on port 8810. It reuses an answer younger than a minute, to keep the load on the public sites low.

On a Linux host that won't let unprivileged users make namespaces, Chromium's sandbox can't start; Ubuntu 24.04's AppArmor rule is one case, and this desktop is another. There, `./chrome-in-docker.sh CHROME_DIR` runs the same Chromium with its sandbox on, in a container allowed the namespaces it needs. Pass it the folder holding a Linux `chrome`. Then put `CDP_URL=http://127.0.0.1:9333` in front of the commands above. Every number and capture here came from that setup: Chromium 145 for testing, on Linux x86-64, in Docker with 1 GB of shared memory. The default 64 MB runs out after a few page reads and fails them with `ERR_INSUFFICIENT_RESOURCES`.

## The parts

- **`defs/*.json`, the definitions.** Each file holds a name, one site (an origin), how to read it, its parameters, and the shape the answer must have. There are three kinds:
  - **`json`** sends a GET to a data address and picks fields by path.
  - **`html`** loads the page's HTML with scripts off. It picks rows by selectors, or a table by its column headers.
  - **`page`** loads the page with its scripts, out of sight. It then waits for a selector and reads what the page rendered, or takes the body of a response the page itself fetched that matches a pattern. A `page` definition may also list other origins for the page's own GETs of scripts and styles (`allow`).
  - **Parameters** are an integer with a range, a list of values, or a string with a pattern.
  - **The shape** gives the columns with their types (text, number, url) and the least number of rows. An answer that doesn't fit it is broken.
- **`runner.mjs`, the browser's part.** It opens a hidden tab in a browser context that stands for the user's profile, and intercepts every request:
  - Anything but a GET is refused. So is a request off the site and its `allow` list, and for `json` and `html`, anything but the one document.
  - A redirect to the definition's `signin` path, a 401 or a 403 answers "needs a sign-in". A redirect off the site is refused and answers "broken".
  - It reads the selectors in an isolated world, out of the page's reach, and skips the HTTP cache.
  - The address is built from the parameters in two steps, either of which would hold alone: each value is checked against its type and pattern, and encoded. Then the address is checked to be on the site, on the definition's path and with only its query keys.
- **`host.mjs`, `host.js`, `client.js`, the sealed page.** The host is what would be browser UI in ilo. It shows a page of the user's own, from `pages/`, in a frame:
  - The frame has `sandbox="allow-scripts"`, an opaque origin, and a CSP of `default-src 'none'` with inline script and style and `data:` images only.
  - The host's own CSP lets the frame navigate only within `/sealed/`.
  - The page's one way out is a `MessageChannel` the host hands it. It asks by API name and parameters, and the host answers only for APIs in that page's grants (`pages/grants.json`). The host's data endpoint needs a token that only the host's script holds.
  - The page refreshes when the host says it's shown: on load, when the tab becomes visible again, and on the Refresh button. There is no timer.
  - A card's source link asks the host to open the address, and the host opens only addresses it delivered itself.
- **`testsite.mjs`.** A local site that sends a read without its session cookie to `/login`, and that changes its layout on a switch.
- **`inspect.mjs`.** What I used as the agent. It saves a page's accessibility outline and its text, which are what ilo's agents get today. It also saves the list of the page's requests and its rendered HTML, which they don't get. It lets the page's GETs through and refuses everything else.

## Writing the definitions, with me as the agent

| Site | What it is | Kind that worked | What I tried first | Definition size (read part / whole file, bytes) | What I needed beyond the outline and text |
| --- | --- | --- | --- | --- | --- |
| en.wikipedia.org, List of tallest buildings | A plain HTML table | `html`, table by headers | The same. The first read came back **broken**: a cell spanning two rows shifted the Year column, and the shape check refused "Tallest building in Beijing" as a year. The runner now lays the table out as a grid, and the definition didn't change. | 172 / 392 | Nothing. The column headers are in the outline. ilo's `request` tool can GET the page and answers with its outline. I haven't checked that the outline comes without scripts, which is what would show the table is in the HTML. |
| mastodon.social, Explore › Hashtags | A script app with a JSON call of its own | `json`, `/api/v1/trends/tags?limit={limit}` | The outline shows only hashtag links. The data address came from the page's request list. | 158 / 432 | **The page's network requests.** Then `request` can GET the JSON to find the field paths. |
| crates.io front page | A list rendered by scripts | `page` reading the DOM (`crates-top`). For the numbers: `json` on `/api/v1/summary`, or `page` taking the body of that call | `html`: the 2.6 KB document has no list until scripts run | 279 / 486 (DOM), 122 / 285 (json), 158 / 364 (page response) | **The rendered HTML**, for the selectors, since the outline names no classes. **The request list**, for `/api/v1/summary`. The rendered list rounds its numbers ("3.2B downloads"); the JSON has them exactly. |
| play.grafana.org | Charts drawn on canvas | **None, as expected** | `page`, waiting for the page's data call `/api/ds/query`. With only its own origin allowed, the app never starts: its scripts live on grafana-assets.grafana.net. With that origin allowed, its start sends a POST for feature flags, the runner refuses it, and the app stops there. It came back "broken: waited 20 s for the page to fetch /api/ds/query". | 177 / 339 | The request list, to see why. Even with everything else solved, the charts draw on canvas, so there are no rows to read. Grafana's own documentation says its panels get their data by POST to `/api/ds/query`, which a read-only runner won't send. I didn't see that request, since the app never got that far. |
| Local test site | Sign-in and a changed layout | `html` (`orders`) and `json` (`orders-feed`) | The same | 200 / 425 and 123 / 369 | Nothing |

So of the five, the plain table needed nothing ilo's agents lack, and the JSON call needed only the request list. The script-rendered list needed the HTML as well. The DOM definition is the most fragile: it depends on class names a site can rename on any release. A reader by accessible role and name, like the table-by-headers reader, would let an agent write it from the outline it already gets.

Not tried: a list that renders only the rows on screen (virtual scrolling). The `page` kind would read only the visible rows. If those were fewer than the shape's least number, it would come back broken; otherwise it would quietly come back short. A page whose own data call is a GET would still work through its response.

## The states

- **Sign-in.** Without its cookie, the test site redirects to `/login`. The definition's `"signin": "^/login"` turns that into "needs a sign-in", and the redirect isn't followed. A site that answers its sign-in form with a 200 at the same address would come back "broken" instead. The definition could carry a sign-in selector too; I didn't build it.
- **Broken.** The switch changes the HTML from a table to cards and the JSON from `{orders: [...]}` to `{data: {items: [...]}}`. They came back as "0 rows, the definition wants at least 1" and "orders is not a list". With the Wikipedia row-span problem above, that makes three real cases where a wrong answer showed as broken.

## The tests (`test.mjs`)

Each test was first shown red with its guard off.

- **Parameters.** Ten hostile values (`../../away`, `//127.0.0.2:8899/x`, `A-1041?extra=1`, `%2e%2e`, a full address, and others) were sent to a definition with a strict pattern, and to the same definition with a pattern of `.*`, which tests the address check alone. A parameter the definition doesn't have was tried too. Through the runner, `../../away` reads nothing, a redirect off the site is refused, and the changed layout reads as broken.
  - Guards on: all pass.
  - `--guards-off`, the naive builder: 18 fail. The test site really did get `GET /away`.
  - One finding: with `.*`, the address stays on the path only as the browser reads it. `..%2F..%2Faway` goes out as `/items/..%252F..%252Faway`, and a server that decodes `%2F` before routing could still see a path outside. So a string parameter should always need a tight pattern, and the real format shouldn't accept `.*`.
- **The seal.** `pages/leak.html` tries 17 ways to send its data to a sink of the test's own: `fetch`, XHR, `sendBeacon`, WebSocket, an image, a CSS background, a font, prefetch, a child frame, a form, a link, a link to a new window, `location`, `top.location`, `window.open`, a meta refresh, and WebRTC. Each attempt is started with a user's click, so the popup blocker isn't what stops it. A pass needs the sink to hear nothing and the browser to say why it stopped the attempt; silence alone counts as a failure.
  - `--seal-off` (no sandbox, no CSP): **all 17 got out.**
  - Seal on: **16 stopped**, each with the browser's reason: a CSP violation, a sandboxed form, a blocked popup, an unsafe top navigation, or frame-src.
  - **WebRTC got out.** A `turn:` server address in an `RTCPeerConnection` makes the browser send UDP packets to any host and port. CSP has no rule for WebRTC that Chromium enforces, and the sandbox has no flag for it. Here the sink saw 20- and 28-byte STUN packets. A server that answered would get the TURN username, which is where the page put its data. Closing this needs the browser: ilo's core would have to refuse peer connections in a sealed frame. I haven't looked for where in Chromium that's best done. Scrubbing `RTCPeerConnection` from the page's script doesn't work, since a child frame of its own has a fresh copy.

Last full run: 41 pass, 1 fail (WebRTC).

## Numbers, for our own design

Three reads each, one after another, in the setup above. "Renderer peak" is the peak resident memory of the read's own renderer process. About 100 MB of it is an empty renderer, shared libraries included.

| API | kind | time (median) | bytes (median) | requests let through / refused | renderer peak |
| --- | --- | --- | --- | --- | --- |
| orders (local) | html | 77 ms | 810 | 1 / 1 | 106 MB |
| orders-feed (local) | json | 82 ms | 583 | 1 / 0 | 103 MB |
| buildings (Wikipedia) | html, table | 579 ms | 164 KB | 1 / 143 | 172 MB |
| trending-tags (Mastodon) | json | 253 ms | 1.5 KB | 1 / 0 | 106 MB |
| crates-stats (crates.io) | json | 238 ms | 7.4 KB | 1 / 0 | 107 MB |
| crates-stats-page (crates.io) | page, response | 1,017 ms | 203 KB | 59 / 2 | 160 MB |
| crates-top (crates.io) | page, DOM | 849 ms | 203 KB | 59 / 2 | 162 MB |

What these numbers say for the design:

- **`json` and `html` don't need a renderer.** Here each read opens a tab, so it costs about 100 MB for a moment. A `json` read is one GET, and the core could send it from the profile's network context with no page at all. An `html` read needs only a parser.
- **The `page` kind costs what a page load costs.** It takes about a second, 200 KB and 60 GETs, though the data itself is 7 KB. Where a site's own data call is a GET, an agent should write `json`. The `page` kind is for sites that have no such call.
- **Every read is the same load the user's own visit would be,** so refresh-on-show at a minute's minimum age is the right order of politeness. A dashboard joining three sources makes three reads in parallel, so it waits as long as the slowest one: Wikipedia's, under 600 ms here.

## What it didn't prove

- **Nothing here ran in ilo.** The runner is Node over Chromium's debugging protocol, and the host is a local web server. In ilo, the runner, the host and the seal would live in the core and the UI, and the profile would be the user's own, not a throwaway one. No real signed-in site was tried, by the rules of the spike.
- **Only Linux.** On a Mac it should run as it is, but it hasn't.
- **POST reads.** GraphQL APIs and Grafana-style query endpoints read with POST. A rule of "GET only" leaves them out. Allowing them would mean letting a definition send a body, and the browser couldn't then tell a read from a write.
- **Data addresses that need headers.** Some sites' own JSON calls send a CSRF token or a custom header that a plain GET doesn't. None of the five did, and the format has no way to say so.
- **Sign-in shown at 200,** as above.
- **The sealed page trusts its data.** The demo pages put site text into `innerHTML`, so markup in one site's data runs as script inside the sealed page. That script can't get out, but it can call the page's other granted APIs. The real host should hand pages text, or sanitize what it hands them.
- One slip during the work: my first load of play.grafana.org, with `inspect.mjs`, let the page's own telemetry POSTs and a PUT go out, before I made the tool refuse anything but a GET. It's fixed, and every later load sent only GETs.

## Gaps in ilo's agent tools

1. **A page's network requests,** with method, type, address and size, including the bodies of its GETs. Without them an agent can't find a site's own data call, which is the cheapest and sturdiest kind of read.
2. **A way to try a definition:** run it once and answer the rows, the shape check and what was refused. That's the runner exposed as a tool, and without it an agent writes definitions blind.
3. **Selectors.** `snapshot` gives roles and names, not classes. The format should read by role and name, the way the table reader reads by headers, rather than give agents the HTML.

The outline, the text and `request`'s GETs were enough for the rest.

## Open questions for the real design

- **Permission when an API is made.** What does the user approve? I'd show the site, the address, the columns with a few sample rows, and the pages that may call it. The sample rows are data the agent saw while writing it. An API with no agent present still needs that first look.
- **The list in Settings, and revoke.** There are two things to list and revoke: each API, and each page's grant of an API. Revoking an API breaks the pages that use it, and they should show a "not allowed" state, which the demo has.
- **The activity-log entry.** Making, changing and revoking an API are clearly the user's to see. A log line for every refresh would be noise; a count per API per day may be enough.
- **Sites on the "Always ask first" list.** An agent probably shouldn't be able to make an API for them without asking. Do their reads also ask, every time the page is shown?
- **Callers outside ilo's own pages.** The answers are signed-in data, so serving them to a local port or to an agent's MCP client would hand that data to software ilo doesn't seal. The spike only lets sealed pages call. Any other caller would need its own grant, and the seal wouldn't hold for it.
- **WebRTC in sealed frames,** and the `%2F` rule for string parameters, both above.
