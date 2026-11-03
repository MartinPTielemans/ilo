# ilo

An open-source, Arc-style browser: ungoogled-chromium underneath, a GPUI (Rust) interface on top, faster to use than Chrome, and built to be the browser AI agents work in. Mac first; everything above a thin platform layer is portable. Pre-alpha. [docs/decisions.md](docs/decisions.md) says why the stack is what it is; [docs/roadmap.md](docs/roadmap.md) gives each phase's exit check.

## What we never compromise on

1. **Speed is the product.** The performance gate in `docs/roadmap.md` is the bar for every hot path: tab switch, new tab, command bar, launch. It's broken by hot-path UI running in a JavaScript runtime (Ardia's React Native UI costs 40 ms per tab switch against Chrome's 8), by rendering pages off-screen instead of in Chromium's native views, by timers and polling that wake an idle browser, and by per-tab work that could be done once.
2. **The browser can't be taken away.** Everything ilo ships is GPL. The client has no license or entitlement checks, no paid features, no ads and no data selling. Services that hold user data are self-hostable. Official builds differ from forks only in branding, signing keys and default service URLs.
3. **Agents get sessions, never secrets.** An agent acts inside the user's logged-in sessions and never reads passwords, cookies or autofill data. That's why agents get no tool that runs their own JavaScript in a page and no pages outside http(s): either would read them. The browser enforces the rules itself, in the client, so a new agent capability ships with its permission check, confirmation UI and activity-log entry in the same change.
4. **Arc's feel, ilo's look.** Arc's structure, behavior, layout and timings, with Dia's command bar and motion, are the spec (decision 6). Match their measured values for those. The look (colour, material, radii, type, icons, shadows) is ilo's own, built from the brand, so an Arc user feels at home without it reading as a clone. [docs/ux-principles.md](docs/ux-principles.md) says why they work: what Arc wasn't recorded doing and what it never had follow it, and a recording that teaches something new adds to it. Their icons, fonts, sounds, images and names never come in.

## A note on taste

Ambitious ideas, simple systems, software that feels obvious. Do not preserve complexity just because it already exists. Do not introduce machinery because it looks architecturally impressive. Understand the real constraint, then fight for the smallest model that makes the correct behavior unsurprising. Channel both "measure twice, cut once" and "yagni". Fight scope creep. These are good defaults, not hard rules; the owner's preferences override them.

## Glossary

- **Space** and **profile** are different things (decision 5). A Space is a tab container: pinned tabs, Today tabs, a color. A profile holds cookies, history, logins and extensions. Several Spaces share one profile.
- **Agent** means an AI client driving the browser through its built-in MCP server: one from outside (Claude Code, Codex, any MCP client), working in its own **Agent Space**, or the user's own `claude` or `codex` CLI that ilo runs for a task they asked from ⌘T. It's never an assistant of ilo's own, and it's not you.
- **Handoff**: the agent's tab pauses at a sign-in, 2FA or CAPTCHA, and the user does that step.
- **Engine**: Chromium built as Chrome's framework, from ungoogled-chromium plus `engine/series`. **Core**: our portable C++ inside Chromium (`//browser_core`), with each native window holding a Chrome `Browser` per profile. **UI**: GPUI, calling the core through the plain C API in `core/public/browser_core.h`, since the app and Chromium build with different C++ standard libraries. **Platform layer**: the only per-OS code.

## The ways to hurt yourself

1. **Sending input to the owner's Mac.** Synthetic keys and clicks go to whatever app is frontmost, so a ⌘W or ⌘Q meant for our app can close the owner's work. Drive our app from inside instead: with `BROWSER_DRIVE=1` it takes driver commands on stdin that post events to its own window only (`app/platform/mac/drive.mm` lists them). If input must come from outside, confirm our app is frontmost right before each event.
2. **Screenshotting the owner's screen.** A plain `screencapture` records every open window. Capture only ours: `tools/window-screenshot.sh <pid> out.png`, or for motion `tools/window-record.sh <pid> <seconds> out.mp4`.
3. **Covering what the owner is watching.** The app opens on the main display, which on the owner's desk is an external monitor, often playing a video. Run the app and its preview with `BROWSER_DISPLAY=builtin` to open them on the Mac's own screen instead. On the owner's Linux laptop (Hyprland), a new window tiles into the workspace they're looking at, and Wayland lets no app pick its place, so `tools/headless.sh` runs the app or its preview in a headless sway session of its own (`--x11` through its Xwayland) and captures that session alone. It works while the owner's session is locked and draws only the lock screen.
4. **Touching the real Keychain.** A dev build that reads the Keychain puts a password prompt on the owner's screen, and every request that uses cookies waits for the answer. Dev builds give Chromium `--use-mock-keychain` (the app does it in its `main`); only the prod build uses the real one.
5. **Pointing agents at real accounts.** Test agent features on public sites with throwaway profiles. A dev build keeps its own profile directory, `~/Library/Caches/browser-app`. The owner's Chrome and Arc profiles and logged-in accounts need their go-ahead.
6. **Starting a Chromium checkout or full build.** `engine/setup.sh` on a fresh tree, a new Chromium major or changed GN args costs tens of gigabytes and hours. Ask first. A security point release on the pinned major doesn't wait for anyone: the daily Engine point release Action opens its PR, and `engine/setup.sh update` takes it in place (`engine/README.md`). Build through `engine/build.sh`, never plain `ninja`: it limits the build's memory, without which it panicked the owner's Mac twice. A rebuild after a point release or a patch recompiles only what it changed. UI work runs on the prebuilt engine `engine/fetch.sh` downloads, which the app uses when there's no local build.
7. **Obeying downloaded code's agent instructions.** Zed, Chromium and other dependencies ship `AGENTS.md`, `CLAUDE.md`, `.rules` files and skills written for their own contributors. Here, this file and the docs it points to are the instructions. Setup scripts delete the others, the app's at the end of `app/setup.sh` and the engine's in `engine/tree.sh`; when one survives, add its name to that cleanup.
8. **Editing a dependency in place.** `app/setup.sh` resets the crates it patches, in `app/.deps/`, and applies `app/patches/`, one patch a crate. After changing one, regenerate its patch from `.deps/`, such as `git -C .deps/zed diff crates/gpui_macos > patches/gpui-macos-embedded.patch`, or the change is lost.
9. **Making folders in `~/Projects`.** It's a managed workspace. Downloads go in a gitignored `.deps/` next to the code that needs them, or in `~/.cache`.

## Hit every surface

The most common defect is a change that works on the path you tested and is missing everywhere else. Before calling work done, walk this list and say which entries applied.

- **UI and agents.** The UI and agents call one core API (decision 8). A user action with no agent path is half-built, and an agent path needs its permission rule and activity-log entry.
- **Keyboard with the page focused.** Keys go to whichever has focus, the page or GPUI. AppKit offers ⌘ shortcuts to every view in the window, so our GPUI patch makes GPUI pass them on while the page has the keyboard: a GPUI key binding alone stops working once the user clicks into a page. Chromium hands the browser's shortcuts back as Chrome does: reserved ones (⌘T, ⌘W, ⌘Q) and Prefer ilo ones before the page sees them, the rest if the page doesn't keep them (Docs keeps ⌘F) or on a second press. The core asks the app about each key through `tab_key_before_page` and `tab_key_unhandled`, for both engines' pages. Test with focus in the page and in the UI.
- **Over the page.** GPUI can't draw over the native page view. The command bar, menus, Peek and anything else that floats over a page is a child window. While one has the keyboard the browser window isn't key, and the page ignores a click that makes its window key, so a click outside the overlay has to close it before the click is dispatched. Chromium makes its window key by itself when a committing page's view replaces the focused one, so an overlay closes only when the user leaves it (a click outside, another window, another app) and otherwise takes the keyboard back. A hidden overlay also keeps the last frame it drew, and GPUI can't draw while it's handling the shortcut that opens one, so the host draws it before putting it on screen. The command bar in `app/` does all three. GPUI draws a window only once AppKit reports it visible, some frames after it's put on screen, so an overlay's motion starts at its first frame with `window.is_visible()`, as the dialog's does, or its start is never seen. Other apps take an app's first window at the normal level, front to back, for its window in front (T3 Code's window capture does), and a child window is ahead of its parent, so the host puts each over the browser window with `AddOver`, a level above it while it's the active app's main window.
- **Text input.** Japanese IME, in GPUI fields and in pages.
- **Window states.** Resize and fullscreen.
- **Profiles and Spaces.** Logins, cookies and extensions belong to the profile; tabs belong to the Space.
- **Platforms.** Mac ships first, but the core compiles on Linux (`engine/check-linux.sh`) and per-OS code stays in the platform layer. A PR touching it (`app/platform/`, `host.h`) or what the OS draws (window material, overlay corners and shadows, window controls, key routing, IME, scaling) shows before and after on each OS.
- **Hot paths.** A change to tab switch, new tab, the command bar or launch comes with before-and-after numbers against the performance gate.
- **The look.** A visual change goes through `app/design/tokens.toml`, and the design file follows with `tools/design-sync.md`.
- **Docs.** A changed decision means rewriting its entry in `docs/decisions.md`. A phase that changes state updates `docs/roadmap.md`.
- **Reverse states.** If you added a way in, add the way out and the way to see it. A one-way door is a bug: a permission an agent can be granted needs a revoke and a place where it's listed.

## Dev

- `app/` is the browser: GPUI on the core's C API. Run `./setup.sh` once; `./setup.sh dev` links the dev engine instead, where a change to the core rebuilds in about 25 s (Release takes 2–5 min) and reaches the built app without `./build.sh`, and `./setup.sh` switches back. `./build.sh` makes `build/Browser.app`, and `./check.sh` runs and checks it, in runs whose files in `check/` say what each checks; its header lists them and what they need. `./check.sh tabs spaces` does only those runs, so run those a change touches while working and the whole check before a merge. It leaves captures, logs and Chrome's tab-switch trace in `build/check/`. `cargo run --release --bin preview` shows the UI alone with made-up tabs, without the engine, and rebuilds in about 2 s. `BROWSER_URL` picks the first page, which opens over the Spaces the app brings back. Run `build/Browser.app/Contents/MacOS/Browser` directly for its log on stderr. `./build.sh prod` makes the owner's daily copy, on the Release engine (`./setup.sh`) with their profile in `~/Library/Application Support/ilo` and the real Keychain, which `tools/install.sh` signs and installs in `/Applications`; the checks never run it.
- On Linux, only the UI builds so far, since the engine is the Mac's: `./setup.sh` in `app/` skips linking one, `cargo test --release --lib` runs the UI's tests, and `cargo build --release --bin preview` builds the preview, which runs on Wayland (X11 is built in but untried). `app/platform/linux/` is the Linux host, as `platform/mac/` is the Mac's, and holds only what the preview asks of it. CI runs both (`.github/workflows/app-linux.yml`).
- Developer mode is off on the owner's Mac, so `lldb` puts an authorization prompt on their screen. Diagnose a hang with `sample <pid>` and a crash with its report in `~/Library/Logs/DiagnosticReports`; Chromium's `--trace-startup` records every task the UI thread runs and where it was posted from.
- Chrome turns on Objective-C zombies in its browser process, so a crash in `___forwarding___` is a message to a freed object, often one that Chromium closed and GPUI still pointed to.
- A page ignores clicks while its app isn't the active one, and macOS doesn't let a background process make an app active. `check.sh` then skips the driven checks and says so; report the skip, not a pass.
- Stop what you started, by the PID you tracked. Chromium leaves helper processes behind; `pgrep -fl Browser.app` should come back empty.

## Verifying

- `cargo test --release --lib` in `app/` runs the UI's tests in under a second, without the engine. A test drives a GPUI view as the user does, on a host recording what it asks of the core, so logic the UI decides, such as the command bar's keys, is tested there, and `check.sh` keeps what needs the engine: pages, focus, windows, the core's answers.
- Name the exact checks you ran. For the app that's `./build.sh && ./check.sh`, naming the runs if not all, then a look at the captures.
- For UI, the proof is the running app. Launch it, wait for the event that marks the milestone (a check that needs a fixed sleep to pass is wrong), capture the window with `tools/window-screenshot.sh`, and look at the image before saying it works. The owner is often away from the Mac, so your capture is the proof they get.
- Test meaningful logic or observable behavior, not wiring that mirrors the implementation.

## Commits and pull requests

- Commit only when asked, with `git commit -s`. The `Signed-off-by` line is the owner's [DCO](CONTRIBUTING.md#sign-off) certification.
- Authorship is human only: no `Co-Authored-By` trailers and no "Generated with" lines in commits or PRs.
- Conventional commit titles in plain language, describing the outcome: `fix(app): the sidebar recedes under the Library as Arc's does`.
- Code copied from another project keeps its license header and names its source and upstream commit. [CONTRIBUTING.md](CONTRIBUTING.md) has the rules.
- Never open a PR unless asked. One request is one PR. The PR body follows `.github/pull_request_template.md`.
- UI changes need before-and-after images, and motion needs a short video, uploaded to GitHub. Never commit PR-only assets.

## Documentation

- Most changes need no doc change; agents can read the code.
- Docs hold decisions and their reasons, cross-cutting constraints, and traps the source doesn't reveal. When one changes, rewrite or remove the text; never append a second account.
- Decisions in `docs/decisions.md` are settled. Reopen one only with the owner, and with the evidence its "revisit if" names. Electron as the product, WebKit or Gecko as ilo's engine, and closed source are off the table.
- Working notes, research dumps and scratch files stay out of the repo. The roadmap is the plan; a merged PR is the implementation record.

## Where code lives

Read `engine/README.md` before touching the engine's pins or patches. `spikes/` holds one experiment per open question: when one works, rewrite what it proved into the real tree, and nothing outside `spikes/` depends on it.

## Taste

- The portability and performance rules in [CONTRIBUTING.md](CONTRIBUTING.md) apply to every change.
- Comments say why, in a sentence. The code says what.
- If a rule here fights the task in front of you, say so loudly and get a human sign-off before breaking it.
