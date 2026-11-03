# Security policy

ilo is pre-alpha: there are no releases yet, and people run it from source. Report vulnerabilities
privately all the same, and don't disclose them until a fix has had a fair chance to ship.

## Where to report

**It reproduces in Chrome.** Then it's Chromium's. Report it to Chromium
([how](https://www.chromium.org/Home/chromium-security/reporting-security-bugs/)), where it can also
earn a reward. We take the fix with the Chromium release that carries it.

**It's ilo's own.** Report it through
[GitHub's private vulnerability reporting](https://github.com/MartinPTielemans/ilo/security/advisories/new).
Never in a public issue, pull request or discussion. ilo's own is:

- **The core**, our C++ in Chromium's browser process (`core/`), which runs outside the sandbox.
- **The C API** (`core/public/browser_core.h`) and the UI that calls it (`app/`).
- **The agent tools and their permission rules.** An agent must act only within the user's
  sessions. It's a vulnerability if one can read a password, cookie or autofill data, run its own
  JavaScript in a page, reach a page that isn't http(s), act without the permission, confirmation or
  handoff its rules require, or act without an entry in the activity log. So is a page that can
  make an agent do any of these.
- **Updates**: how ilo and its engine are fetched, pinned and installed (`engine/fetch.sh`,
  `engine/pins.ini`, and the updater once there is one).
- **The engine patches** (`engine/series`, `engine/patches/`), including a Chromium protection they
  turn off.

## What a report needs

- The ilo commit, the Chromium version (`chromium.version` in `engine/pins.ini`), and the macOS
  version and chip.
- Whether it reproduces in Chrome at the same version.
- Steps or a proof of concept on pages you control. Test only on your own machine, with throwaway
  profiles and accounts.
- The impact: what an attacker gains, and what they need first (a visited page, an installed
  extension, a connected agent, local access).
- For agents: the MCP client, the tool calls, and the permissions the agent had.

We reply within three working days, fix ilo's own bugs before other work, and publish an advisory
that credits you, unless you'd rather not be named, once the fix is on `main`.

## Patch lag

The time between a Chrome security release and the same fixes in ilo is the number that matters
most for a Chromium browser. We track it, and set a goal for it once Chromium's point releases reach
ilo automatically ([#299](https://github.com/MartinPTielemans/ilo/issues/299)); until that path
exists, a number would be a promise we can't keep. ilo has no updater yet, so a build from source has
the fixes once it's pulled and rebuilt.
