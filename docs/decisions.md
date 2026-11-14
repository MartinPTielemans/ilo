# Decisions

Each entry gives what was decided, why, and what would make us revisit it. Newest last.

## 1. Engine: Chromium, with the system's engine for protected video

Chromium, not WebKit, Gecko, Servo or Ladybird, renders every page by default.

- The product runs on Mac, Windows and Linux; WebKit is first-class only on macOS.
- Chrome extensions and site compatibility are requirements.
- Chromium browsers feel slow from bloat, UI layers and background work, not the engine, and we control those.

A tab can show its page in the system's engine (WebKit on the Mac): a short built-in list of streaming services from their first visit, and any site the user picks from the protected-media note, per profile. ungoogled-chromium lacks Widevine, which Google licenses only to signed Chrome builds, so on the Mac Netflix, Disney+, Spotify and Prime Video can't play in Chromium. The platform's DRM plays them: FairPlay in WebKit, in the tab (handing the page to Safari lost it), and PlayReady in Chromium on Windows 11 with x64 and hardware DRM; elsewhere the note says what's missing. Linux gets Google's Widevine module at the user's yes (decision 7). The owner wants these sites to work without a blocker, the big ones just playing, as in Arc (2026-10-27).

- **A short list, then opt-in per site.** Netflix, Disney+, HBO Max, Prime Video, Hulu, Apple TV+, Paramount+, Peacock and the Spotify web player open there from the first load, their main content being protected video or audio. A list, not a switch at a page's first DRM request: some sites check for DRM on load and never play. It stays short: extensions don't run there, so a listed site loses the user's ad blocker and password manager. An entry is the service, never its domain: tv.apple.com, not apple.com. Other sites the user turns on from the note, per profile. Either way its pages open there, and a link to another site goes back to Chromium, unless its redirects come back, as a sign-in through an identity site does. Site Controls show it on and turn it off; Off for a listed service stays in the profile, across launches and list updates, until On. A listed service never joins the user's own sites, and there's no global setting: per-site Off is the way out.
- **No extensions and no agents.** The core refuses and logs every agent call on such a tab, and an agent's Space never switches a tab: the browser can't enforce decision 8's rules in an engine it doesn't own.
- **Kept apart per profile.** Each profile has its own WebKit store; a private window's keeps nothing. Deleting a profile deletes its store; Clear Cookies and Clear Site Data clear both engines. The site's session follows its tab within the profile, so the user stays signed in: as a tab enters the system's engine, before the page's first request and script, the core copies the profile's cookies for the site and its subdomains (HttpOnly too) and its localStorage into the profile's WebKit store. Turning the site off copies them back. A private window copies only between its own two stores; nothing crosses profiles or leaves them. Other storage (sessionStorage, IndexedDB, service workers) stays where it was made.
- **Started only when used.** No WebKit view, store or process starts before a page needs one, so launch and other tabs don't pay for it. (System frameworks Chromium links map WebKit's code at launch anyway: AuthenticationServices, SafariServices, Quartz.) It needs macOS 14 for per-profile stores; on 13 the note offers Safari.

Revisit when Chromium can play these sites on the Mac (a Widevine CDM we ship, or the platform's), when they refuse Chromium PlayReady on Windows (WebView2 then), or when two engines cost more than the sites are worth.

## 2. Base: ungoogled-chromium, with Helium and Ardia as patch sources

Not raw Chromium and not all of Helium.

- De-Googling Chromium touches hundreds of call sites every release; ungoogled-chromium has done it for a decade.
- Helium is a product on top of ungoogled-chromium. Ardia, built on Helium, needs about 15 skipped patches, 10 that undo Helium and a guard stage (`docs/engine-build.md` in mantrakp04/ardia). We cherry-pick from Helium instead.
- One fewer upstream gets us Chromium's security fixes sooner.
- The Chrome Web Store is the one Google service we bring back: extensions are a requirement (decision 1), and ungoogled-chromium breaks installing and updating them from it, which `engine/patches/ours/chromium-extensions-web-store.patch` restores. The browser talks to the store directly, not through a proxy as Helium does, and with no store extension installed, only on the store's own page. `engine/README.md` lists when and what it sends. Besides the next entry's security updates, nothing else of Google's comes back: no Safe Browsing, sync or metrics, and the component updater fetches only decision 7's module.
- We take back one thing ungoogled-chromium removes: the updates that stop the browser trusting what's known to be bad (certificate revocations, the root store, Google's malware verdicts on extensions). Without them a revoked certificate or malicious extension stays trusted until the next release. Ardia's patches fetch them from Google with identifying fields stripped; `engine/README.md` lists what's fetched and sent.

Revisit if cherry-picking from Helium costs more per release than taking its whole series.

## 3. Architecture: Chrome's framework plus our own window host

Ardia's design: Chrome's `chrome_framework` with our layer linked in. One native window, the core's Views widget, holds a Chrome `Browser` per profile, each with a UI-less stand-in `BrowserWindow`. Tab views are parented into it, GPUI draws under them, and pages' popups become tabs. Extensions' windows stay Chrome's, like undocked DevTools (owner, 2026-10-29).

- It keeps the whole `chrome/` layer, so extensions, the Web Store, autofill, passkeys and DevTools work.
- Ardia ships on this design today.
- Ardia's layer is about 80% Objective-C++; ours is portable C++ with a thin platform shim.

On Windows two things differ, both forced by Chromium. Aura draws all of a widget's views into its one HWND, so nothing native can sit between GPUI and a page in it: GPUI's window is a child HWND filling the browser window, and each shown page is in a child widget's HWND above it (`core/platform/win/window_views.cc`). The sandbox needs every process to be Chromium's own `chrome.exe`, so the app can't be the executable calling `bc_main`: it's `Browser.dll` beside it, which the core loads in the browser process as Chromium starts (`bc_main` in `core/public/browser_core.h`).

On Linux the same two hold, and Wayland adds a third. The zygote starts every process from Chromium's own `chrome`, so the app is `libBrowser.so` beside it, loaded the same way (`core/platform/linux/app_library.cc`). Wayland nests surfaces only if made on one connection, and GPUI and Chromium each open their own. So Linux comes in two steps, both needed before it ships. First X11, under XWayland on Wayland desktops: GPUI's window is an X window the core reparents over the browser's, with holes in its shape where pages show. Pages are aura windows in the browser window, as in Chrome, so keys reach them through its focus, after the browser's shortcuts (`core/platform/linux/window_views.cc`). Then native Wayland: GPUI draws as a subsurface through Chromium's Wayland connection, in an embedded mode like the Mac's GPUI patch. On both, the core routes focus, IME and the clipboard between GPUI and the page.

## 4. UI: GPUI, with Chromium Views as the fallback

- GPUI is compiled, fast, and runs on Mac, Windows and Linux.
- Ardia's React Native UI measures 40 ms per tab switch against Chrome's 8 ms (`docs/perf/vs-browsers.md` in mantrakp04/ardia). Hot-path UI must not run in a JavaScript runtime.
- Pages render in Chromium's native views, not off-screen, keeping Chrome's compositing, latency, video overlays and battery life.
- GPUI can't draw over a native view, so anything floating over the page (command bar, menus, Peek) is a borderless child window, as Chrome's bubbles are.

Settled by the Phase 1 spikes (`spikes/gpui-cef/` and `spikes/gpui-ardia/` at 91ddfe6, since removed). On CEF and Ardia's engine, GPUI runs inside Chromium's run loop, and focus, the browser's shortcuts with the page focused, IME composition, the command bar over the page, resize and full screen work across the boundary. A long page scrolls at 120 fps with GPUI in the window.

Revisit if an engine or GPUI change breaks run-loop sharing or input across the boundary unpatchably. The fallback is Views (C++): as cross-platform, without these integration risks.

## 5. Product model: Spaces are separate from profiles

A Space is a tab container (pinned tabs, Today, color). Several Spaces can share one profile, which holds cookies, history, logins and extensions: Arc's model. Ardia merges the two, costing memory and repeated sign-ins.

## 6. UX: Arc's feel, ilo's look

Arc's structure (sidebar, Spaces, pinned and Today tabs, favorites, archive, Peek, Little Arc, split view, Boosts), its behavior, layout and timings, and Dia's command bar and motion are where every surface starts: their measured values hold unless a change is better in every way, its PR saying why for each difference, for the owner's review. Features beyond Arc and Dia come in only when the owner picks them; `docs/ux-spec.md` lists each.

The look is ilo's own: colour, window material, corner radii, type, icons, shadows and edges, built from ilo's brand and `docs/ux-principles.md`, not Arc's or Dia's values. A Space's theme stays the user's free choice; ilo owns the look of a Space with no theme, the preset swatches, and everything drawn on top, which must read on any theme. ilo is Arc's successor: an Arc user must find everything where their hands expect it, with the same speed and calm, yet it can't read as one more Arc clone. Familiarity comes from where things are and how they move: the sidebar's layout, row pitch and plates, the state alphas, every timing and spring, and the nav glyphs.

Dia's surfaces built around its assistant aren't copied: ilo has none, and the user's agents (decision 8) are asked from Arc's bar. So ⌘T opens Arc's command bar over the page, not Dia's New Tab page, whose box is the assistant's chat field and whose card holds its answers: without the assistant, it's Arc's bar moved into a card that hides the page.

- Start from Ardia's measured `docs/dia-spec.md` and `docs/dia-feature-parity.md` (Apache-2.0, credited).
- Measure Arc ourselves before it shuts down on 2026-12-22.
- Never ship their assets or names.

Revisit the look if Arc users moving over say ilo doesn't feel like Arc continued, traced to the look rather than layout or motion.

## 7. License: open source, GPL-3.0-or-later

- The first users are Arc refugees whose browser was taken away. A browser that can't be is the pitch, and only source code makes that credible.
- We can use Helium's and Ardia's GPL patches directly.
- Our code is GPL too, so its improvements can't go into a closed browser.
- What's proprietary the user chose to get, like an extension: on Linux, Google's Widevine module, never bundled, fetched only at the user's yes and removed in Settings.
- Money comes from services (end-to-end encrypted sync, later AI), search revenue sharing and an optional Pro tier, not closed client code.
- The name and logo are trademarks: that's what protects the product.
- Contributions use DCO sign-off, not a CLA.

## 8. Agents: the user's own agents, in their real browser

Agents drive the user's real browser through ilo's built-in MCP server, a first-class capability. They come two ways, which the core treats alike, with the same tools, permission checks, confirmations and activity log:

- **From outside.** Any MCP client (Claude Code, Codex, others) connects to the server.
- **From ilo.** The user asks from ⌘T, and ilo runs the `claude` or `codex` CLI they installed and signed into, with ilo's MCP server added for that task, on their own subscription. ilo never handles their login, token or key; when the CLI is missing or signed out it says how to fix it.

ilo has no assistant of its own: no model, agent loop or AI account. What answers is the user's agent, which can do in a page only what the MCP server lets any agent do.

- **Built-in MCP server.** Local-only at first, on a socket only this user's processes reach; cloud agents come later, by explicit pairing.
- **Where the work shows.** Each agent works in its own Space, listed in no window; its pages and steps show on a card at the sidebar's foot, not as tabs. A task asked from ilo and an outside agent's session get the same card; an outside agent's conversation stays in its terminal. An agent never takes the user's focus or cursor; the user can watch, take over and hand back anytime.
- **Engine-native page access.** The core serves the accessibility tree, a cleaned-up DOM, stable element references and screenshots straight from Chromium, with no extension or remote-debugging port.
- **Permissions per agent and per site,** plus an activity log. Defaults are conservative: no payments, no messages or forms sent on sensitive sites without confirmation, no banking sites unless allowed.
- **Handoff to the human.** At a login, 2FA prompt or CAPTCHA, the agent's tab pauses and the sidebar asks the user to step in.
- **Browser tools only.** A CLI ilo runs gets ilo's MCP tools and nothing else: its shell, file and web tools, the user's other MCP servers, plugins, hooks and settings are off, it runs in an empty folder, and ilo declines any other approval it asks. Page text can steer an agent, and a shell would reach the user's files, keys included. A CLI that can't be limited so isn't offered.
- **Sessions, never secrets.** Agents act in the user's logged-in sessions but never read passwords, cookies or autofill data.

Why:

- Every current option fails an agent working beside a human. Headless Playwright has no logins and is flagged as a bot; so are cloud browsers, which also take the data off the machine. Remote-debugging the user's Chrome is fragile and insecure, and Chrome 136 blocks it on the default profile.
- Owning the Chromium core layer (decision 3) makes this possible; Spaces (decision 5) are the natural unit of isolation.
- It sets us apart from Ardia and other Arc-style browsers.
- People want to hand over a task from where they are, without a terminal, and many already pay for Claude or Codex. Their own CLI costs them nothing more, needs no AI service of ilo's, and keeps one path, so every agent rule lives once in the core.

Revisit asking from ilo if Anthropic or OpenAI stop other apps running their CLIs on a subscription; ilo then takes the vendor's sign-in program where there is one, or the user's API key.

Consequences now:

- The core exposes one API to both the UI and agents; agents aren't a bolt-on.
- "Agent Space" is a kind of Space in the data model from the start.
- The main risk is prompt injection from page content. Permissions and confirmations limit the damage without solving it, so confirmation UI and the activity log ship with the MCP server, never after.

## 9. Crash reports: opt-in, to Sentry (EU region)

No crash report leaves the device without the user's yes, in Send Crash Reports (off) or the offer after a crash. A report can hold a URL or page text but no user folder, and asks Sentry to infer no IP. Official builds send them to ilo's Sentry in the EU, which takes minidumps; the collector is a default service URL, so a fork names its own or none.

Revisit if Sentry stops taking minidumps or keeping them in the EU.

## 10. WebRTC: a page learns no address HTTP doesn't give it

WebRTC gathers only on the default route's public interface (`default_public_interface_only`), so calls and peer-to-peer pages connect directly without a page learning a local address. While a profile uses a proxy, WebRTC goes only through it and TURN relays (`disable_non_proxied_udp`, ungoogled-chromium's default), so the proxy hides the real address. Settings › Privacy › Relay calls makes every profile relay-only always. Chrome's `default` isn't ours: ungoogled-chromium builds without mDNS (`enable_mdns=false`), so it would give pages the raw local addresses Chrome's hides. Evidence: #1774.

Revisit if the engine builds with mDNS again, or a full-tunnel VPN is found leaking its real address under `default_public_interface_only`.

## 11. A daily usage count: anonymous, on, with a switch

Once a UTC day a copy sends one count, unless Settings › Privacy › Send a Daily Usage Count is off (owner, 2026-11-06, #1977). It's how we know how many use ilo: crash reports count only copies that crash and opt in.

- **Sent:** version, channel, OS and major version, chip, and the flags daily, first run, first this ISO week and first this month.
- **Never sent:** an ID, cookie, time of day, or anything about pages, tabs, Spaces, profiles, agents, accounts or settings. The copy keeps only its last count's date; the server keeps each day's totals, never the address or user agent.
- **How:** about 30 s after launch or on returning on a new day, with no timer after; through the core and Chromium's network stack, without credentials, to the one endpoint the build names (`app/branding/daily-count`, prod only), never a general fetch. A build naming none sends nothing.

Revisit if the totals stop telling releases apart without telling copies apart.

## 12. Web defaults: stricter than upstream's where the spec is

Where a page's web platform default differs from Chrome's, it's listed here, each stricter than upstream rather than behind it. A site written to the spec works in ilo as in the other engines.

| Default | Chrome's | ilo's | Why | Revisit if |
|---|---|---|---|---|
| Screen sharing needs a gesture | A page calls `getDisplayMedia()` whenever it likes, and the picker comes up unasked | Only within a click's or a key's activation, as the spec and the other two engines have it. Otherwise the page's promise is refused with `InvalidStateError`, and ilo shows nothing. An administrator's `ScreenCaptureWithoutGestureAllowedForOrigins` exempts an origin (the orchestrator's call, listed for the owner, 2026-11-12, #2456) | No page can put a share picker in front of the user they didn't ask for | Chrome turns `GetDisplayMediaRequiresUserActivation` on, or a named site breaks on it |
| WebRTC addresses | `default` | `default_public_interface_only` | Decision 10 | Decision 10 |
