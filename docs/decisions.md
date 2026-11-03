# Decisions

Each entry gives what was decided, why, and what would make us revisit it. Newest last.

## 1. Engine: Chromium, with the system's engine for protected video

Chromium, not WebKit, Gecko, Servo or Ladybird, renders every page by default.

- The product has to run on Mac, Windows and Linux. WebKit is only first-class on macOS.
- Chrome extensions and site compatibility are requirements.
- Chromium's engine is not what makes Chromium browsers feel slow. Bloat, UI layers and background work are, and we control those.

A tab can show its page in the system's engine, WebKit on the Mac, for a short built-in list of streaming services from their first visit, and any other site the user chose from the protected-media note, per profile. ungoogled-chromium has no Widevine, which Google licenses only to signed Chrome builds, so on the Mac Netflix, Disney+, Spotify and Prime Video can't play in Chromium. The platform's DRM plays them: FairPlay in WebKit, in the tab, where handing the page to Safari lost it, and PlayReady in Chromium on Windows 11 with x64 and hardware DRM; elsewhere the note says what's missing. Linux gets Google's Widevine module at the user's yes (decision 7). The owner wants these sites to work without a blocker, and the big services to just play, as in Arc (2026-10-27).

- **A short list, then opt-in per site.** Netflix, Disney+, HBO Max, Prime Video, Hulu, Apple TV+, Paramount+, Peacock and the Spotify web player open in the system's engine from the first load, as their main content is protected video or audio. It's a list, not a switch at a page's first DRM request, as some sites check for DRM on load without playing anything, so switching at any check would move sites that never needed it. It stays short because extensions don't run there, so a listed site loses the user's ad blocker and password manager. An entry is the service, never the domain around it: tv.apple.com, not apple.com. Any other site the user turns on from the note, in that profile. Either way its pages open there, and a link to another site goes back to Chromium. A link whose server redirects pass through another site and come back, as a sign-in through an identity site does, stays. Site Controls show the site on and turn it off. Off for a listed service stays in the profile, across launches and list updates, until On. A listed service is never added to the user's own sites, and there's no global setting: per-site Off is the way out.
- **No extensions and no agents.** Extensions run in Chromium only. The core refuses every agent call on such a tab and logs it, and an agent's Space never switches a tab. The browser can't enforce decision 8's rules inside an engine it doesn't own.
- **Kept apart per profile.** Each profile has its own WebKit store, and a private window has one that keeps nothing. Deleting the profile deletes its store, and Clear Cookies and Clear Site Data clear both engines. The site's session goes with its tab, within the profile, so the user stays signed in. As a tab enters the system's engine, the core copies the profile's cookies for the site and its subdomains, HttpOnly too, and its localStorage from Chromium into the profile's WebKit store, before the page's first request and script. Turning the site off in Site Controls copies them back. A private window copies only between its own two stores, and nothing crosses profiles or leaves the stores. Other storage (sessionStorage, IndexedDB, service workers) stays where it was made.
- **Started only when used.** No WebKit view, store or process starts before a page needs one, so launch and the other tabs don't pay for it. WebKit's code is mapped at launch anyway, by system frameworks Chromium links (AuthenticationServices, SafariServices, Quartz). It needs macOS 14 (per-profile stores). On 13 the note offers Safari instead.

Revisit when Chromium can play these sites on the Mac, through a Widevine CDM we may ship or the platform's own CDM, when they refuse Chromium PlayReady on Windows (WebView2 then), or when two engines cost more than the sites are worth.

## 2. Base: ungoogled-chromium, with Helium and Ardia as patch sources

Not raw Chromium and not all of Helium.

- De-Googling Chromium is work across hundreds of call sites every release, which ungoogled-chromium has done for a decade.
- Helium is a product on top of ungoogled-chromium. Ardia, built on it, needs about 15 skipped patches, 10 that undo Helium and a guard stage to keep that working (`docs/engine-build.md` in mantrakp04/ardia). We cherry-pick from Helium instead.
- One fewer upstream means Chromium security fixes reach us sooner.
- The Chrome Web Store is the one Google service we bring back, since Chrome's extensions are a requirement (decision 1) and ungoogled-chromium breaks installing and updating them from it, which `engine/patches/ours/chromium-extensions-web-store.patch` restores. The browser talks to Google's store directly, not through a proxy as Helium does, and with no store extension installed only on the store's own page. `engine/README.md` lists when and what it sends. Besides the security updates in the next entry, nothing else of Google's comes back: no Safe Browsing, sync or metrics, and the component updater fetches nothing more but decision 7's module.
- We take back one thing ungoogled-chromium removes: the updates that keep the browser from trusting what's known to be bad (certificate revocations, the root store, Google's malware verdicts on extensions), without which a revoked certificate or a malicious extension stays trusted until the next release. Ardia's patches fetch them from Google, identifying fields stripped. `engine/README.md` lists what is fetched and what is sent.

Revisit if cherry-picking from Helium costs more per release than taking its whole series would.

## 3. Architecture: Chrome's framework plus our own window host

Following Ardia's design: Chrome's `chrome_framework` with our layer linked in. One native window, the core's Views widget, holds a Chrome `Browser` per profile, each with a UI-less stand-in `BrowserWindow`. Tab views are parented into it, GPUI draws in it under them, and pages' popups become tabs. Extensions' windows stay Chrome's, like undocked DevTools (owner, 2026-10-29).

- This keeps the whole `chrome/` layer, so extensions, the Web Store, autofill, passkeys and DevTools keep working.
- Ardia ships on this design today.
- Ardia's layer is about 80% Objective-C++; ours is portable C++ with a thin platform shim.

On Windows two things differ, both forced by Chromium there. Aura draws all of a widget's views into its one HWND, so nothing native can sit between GPUI and a page in it. GPUI's window is instead a child HWND that fills the browser window, and each page shown is in a child widget's HWND above it (`core/platform/win/window_views.cc`). The sandbox needs every process to be Chromium's own `chrome.exe`, so the app can't be the executable that calls `bc_main`. The app is `Browser.dll` beside it, and the core loads it in the browser process as Chromium starts (`bc_main` in `core/public/browser_core.h`).

On Linux the same two hold, and Wayland adds a third. The zygote starts every process from Chromium's own `chrome`, so the app is `libBrowser.so` beside it, loaded the same way (`core/platform/linux/app_library.cc`). Wayland nests surfaces only when they were made on one connection, and GPUI and Chromium each open their own. So Linux comes in two steps, both needed before it ships. First X11, under XWayland on Wayland desktops: GPUI's window is an X window the core reparents into the browser's, over it, with holes in its shape where the pages show. The pages are aura windows in the browser window, as in Chrome, so the keys reach them through its focus, after the browser's shortcuts (`core/platform/linux/window_views.cc`). Then native Wayland: GPUI draws through Chromium's Wayland connection, as a subsurface, in an embedded mode like the Mac's GPUI patch. On both, the core routes focus, IME and the clipboard between GPUI and the page.

## 4. UI: GPUI, with Chromium Views as the fallback

- GPUI is compiled, fast, and already runs on Mac, Windows and Linux.
- Ardia's React Native UI measures 40 ms per tab switch against Chrome's 8 ms (`docs/perf/vs-browsers.md` in mantrakp04/ardia). Hot-path UI must not run in a JavaScript runtime.
- Pages render in Chromium's own native views, not off-screen, so we keep Chrome's compositing, latency, video overlays and battery life.
- GPUI can't draw over a native view. Anything that floats over the page (command bar, menus, Peek) is a borderless child window, which is also how Chrome does its bubbles.

Settled by the Phase 1 spikes, `spikes/gpui-cef/` and `spikes/gpui-ardia/` at 91ddfe6, since removed. On CEF and on Ardia's engine, GPUI runs inside Chromium's run loop, and focus, the browser's shortcuts with the page focused, IME composition, the command bar over the page, resize and full screen all work across the boundary. A long page scrolls at 120 fps with GPUI in the window.

Revisit if a later engine or GPUI change breaks run-loop sharing or input across the boundary in a way we can't patch. The fallback is Views (C++): equally cross-platform, with none of these integration risks.

## 5. Product model: Spaces are separate from profiles

A Space is a tab container (pinned tabs, Today, color). Several Spaces can share one profile, and the profile holds cookies, history, logins and extensions. This is Arc's model. Ardia merges the two, which costs memory and repeated sign-ins.

## 6. UX: Arc's feel, ilo's look

No open-ended UX exploration. Arc's structure (sidebar, Spaces, pinned and Today tabs, favorites, archive, Peek, Little Arc, split view, Boosts), its behavior, layout and timings, and Dia's command bar and motion are the spec. Features beyond Arc and Dia come in only when the owner picks them, and `docs/ux-spec.md` lists each one.

The look is ilo's own: colour, window material, corner radii, type, icons, shadows and edges. It's built from ilo's brand and `docs/ux-principles.md`, not from Arc's or Dia's values. A Space's theme stays the user's, chosen freely; ilo owns the look of a Space with no theme, the preset swatches, and everything drawn on top, which has to read on any theme. ilo is Arc's successor, so an Arc user has to find everything where their hands expect it and feel the same speed and calm. It can't read as one more Arc clone. Familiarity comes from where things are and how they move, so those stay measured: the sidebar's layout, row pitch and plates, the state alphas, every timing and spring, and the nav glyphs.

Dia's surfaces built around its assistant aren't copied, since ilo has no assistant of its own and the user's agents (decision 8) are asked from Arc's bar. So ⌘T opens Arc's command bar over the page, not Dia's New Tab page, whose box is the assistant's chat field and whose empty card holds its answers. Without the assistant, that page was Arc's bar moved into the card with the page hidden behind it.

- Start from Ardia's measured `docs/dia-spec.md` and `docs/dia-feature-parity.md` (Apache-2.0, credited).
- Measure Arc ourselves before it is shut down on 2026-12-22.
- Copy behavior, layout and timings. Never ship their assets or names.

AI features of ilo's own are out of scope for v1. The user's agents (decision 8) are the one way AI comes in.

Revisit the look if Arc users moving over say ilo doesn't feel like Arc continued, and it traces to a look change rather than layout or motion.

## 7. License: open source, GPL-3.0-or-later

- The first users are Arc refugees whose browser was taken away. A browser that can't be taken away is the pitch, and only source code makes it credible.
- We can use Helium's and Ardia's GPL patches directly.
- Our own code is GPL too, so improvements to it can't be taken into a closed browser.
- What's proprietary the user chose to get, like an extension: on Linux, Google's Widevine module, never bundled, fetched only at the user's yes and removed in Settings.
- Money comes from services (end-to-end encrypted sync, later AI), search revenue sharing and an optional Pro tier, not from closed client code.
- The name and logo are trademarks, which is what actually protects the product.
- Contributions use DCO sign-off, not a CLA.

## 8. Agents: the user's own agents, in their real browser

Agents drive the user's real browser through ilo's built-in MCP server, as a first-class, built-in capability. They come two ways, and both take that one path:

- **From outside.** Any MCP client (Claude Code, Codex, others) connects to the server.
- **From ilo.** The user asks from ⌘T, and ilo runs the `claude` or `codex` CLI the user installed and signed into, with ilo's MCP server added for that task. The user's own subscription pays. ilo never handles their login, token or key, and when the CLI is missing or signed out it says how to fix that.

ilo has no assistant of its own: no model, agent loop or AI account. What answers is the user's agent, and it can do in a page only what the MCP server lets any agent do.

- **Built-in MCP server.** Local-only at first, on a socket only this user's processes can reach. Cloud agents come later, through explicit pairing.
- **Where the work shows.** An outside agent works in its own Agent Space, visible in the sidebar. A task asked from ilo shows among the user's tabs without becoming tabs: its pages and its conversation live in the task. An agent never takes the user's focus or cursor. The user can watch, take over and hand back at any time.
- **Engine-native page access.** The core serves the accessibility tree, a cleaned-up DOM, stable element references and screenshots straight from Chromium. No extension, and no remote-debugging port.
- **Permissions per agent and per site,** plus an activity log. Defaults are conservative: no payments, no sending messages or forms on sensitive sites without confirmation, and no banking sites unless the user allows them.
- **Handoff to the human.** At a login, 2FA prompt or CAPTCHA, the agent's tab pauses and the sidebar asks the user to step in.
- **Same rules either way.** To the core, a task asked from ilo is an agent like any other: the same tools, permission checks, confirmations and activity log.
- **Browser tools only.** A CLI that ilo runs gets ilo's MCP tools and nothing else. Its shell, file and web tools, the user's other MCP servers, plugins, hooks and settings are all off, it runs in an empty folder, and ilo declines any other approval it asks for. Page text can steer an agent, and a shell would reach the user's files, keys included. A CLI that can't be limited this way isn't offered.
- **Sessions, never secrets.** Agents act inside the user's logged-in sessions but can never read passwords, cookies or autofill data.

Why:

- Every current option fails an agent working beside a human. Headless Playwright has no logins and gets flagged as a bot; cloud browsers too, and they take the data off the machine. Remote-debugging the user's Chrome is fragile and insecure, and Chrome 136 blocks it on the default profile.
- Owning the Chromium core layer (decision 3) is what makes this possible. Spaces (decision 5) are the natural unit of isolation.
- It sets us apart from Ardia and the other Arc-style browsers.
- People want to hand a task over from where they are, without a terminal, and many already pay for Claude or Codex. Running their own CLI costs them nothing more, needs no AI service of ilo's, and keeps one path, so every agent rule lives in the core once.

Revisit asking from ilo if Anthropic or OpenAI stop other apps from running their CLIs on a subscription. Then ilo takes the vendor's sign-in program where there is one, or the user's API key.

Consequences now:

- The core exposes one API that both the UI and agents use. Agents are not a bolt-on.
- "Agent Space" is a kind of Space in the data model from the start.
- The main risk is prompt injection from page content. Permissions and confirmations limit the damage but don't solve it, so confirmation UI and the activity log ship with the MCP server, never after it.

## 9. Crash reports: opt-in, to Sentry (EU region)

No crash report leaves the device without the user's yes, in Send Crash Reports (off) or the offer after a crash. A report can hold a URL or page text but no user folder, and it asks Sentry to infer no IP. Official builds send them to ilo's Sentry in the EU, which takes minidumps. The collector is a default service URL: a fork names its own or none.

Revisit if Sentry stops taking minidumps or keeping them in the EU.
