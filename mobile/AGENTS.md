# ilo for iPhone

The browser the owner uses on the phone, in place of Safari, not a companion beside it. Sync with ilo on the computer is what makes it ilo: the same Spaces, pinned tabs and Today tabs. The test for phone work is a week without Safari.

It's native Swift in `mobile/ios`, and paths below start there. This file adds to the root `AGENTS.md` for work under `mobile/ios/`; the Android app, in `mobile/android`, has its own `AGENTS.md`.

## What differs from the desktop

- **The engine is WebKit**, in `WKWebView`, because iOS gives an app no other outside the EU and Japan (`docs/decisions.md`, decision 1). The root's Chromium, core and GPUI rules have nothing here to apply to; the rest of it holds.
- **No agent drives the phone app**, so the root's "UI and agents" surface has no phone side yet.
- **The phone's look has its own source**, `App/Look/Look.swift`, which follows `app/design/tokens.toml` by hand: sizes, colours and motion come from it, never from a view.

## The sync contract

`IloSync` (`Sync/`) writes what `core/sync_spaces.cc` reads, in the messages `core/sync_spaces.proto` defines. Its codec is written by hand, so nothing compiles the two sides together: a field one side changes still builds on both and loses data at the next sync. A change to either side changes both in one PR, with a test on each.

- **What a device shows of the account is that device's choice.** Hiding a tile on the phone never changes the account.
- **What changes the account must be something the desktop understands**, since every computer on the account reads it.
- **A saved password is Chromium's own record, under the name `core/sync_guard.cc` gives it.** `Sync/Sources/IloSync/PasswordWrite.swift` makes that name by hand, and a desktop drops a record whose name doesn't fit without saying so. A change to either side's name changes both.
- **A desktop file the phone reads starts the phone's checks** only if `mobile_ios` in `.github/scripts/ci-changes.cjs` names it. A test or script that starts reading another adds it there in the same PR.

## The ways to hurt yourself

1. **Reaching the real account.** Tests and agents sign in only to a local ilo-server, with throwaway `check-…@example.org` addresses: a write to a real account syncs to every device on it. The sign-in sheet's server is the production one by default, so open it and capture it, and leave its fields empty and "Sign in" unpressed. A debug build launched with `-fixture` is signed in to a made-up account (`App/Debug/FixtureAccount.swift`).
2. **Showing a secret.** Never read, print or log a password, cookie, token, recovery word, username or decrypted account data, outside a test's own made-up data: a log outlives the run that wrote it. A saved password is filled only on the user's tap, after Face ID, into a page whose origin matches exactly.
3. **Opening a simulator on the owner's screen.** Drive one only through T3 Code's device tools: `device_open`, then the `agent-device` command it returns, which the owner watches in the Device panel. Never Simulator.app or Xcode's window; `simctl` is for installs and logs.
4. **Calling it smooth.** A simulator proves nothing about frame rate. Say "looks right in frames", and leave "smooth" to the owner's phone.
5. **Apple's side.** Agreements, the export declaration, entitlement requests and App Store Connect are the owner's. Stop at each and ask. Apple has granted the owner's team the default-browser entitlement, which `DEFAULT_BROWSER_ENTITLEMENT` in `project.yml` turns on. It's also what lets a page make and use a passkey: without it WebKit refuses each request at once, with no sheet. A build another team signs sets it to `NO`, or signing fails.

## Designing for the phone

- **Everything touched is within thumb reach, at the foot.** Nothing tappable sits at the top, where one hand doesn't reach.
- **A phone surface is designed for the phone**, never the desktop's sidebar made small.
- **Mock it first**, at real scale among its neighbours, before building.
- **One look, in Liquid Glass.** Every glass in the app is made in `App/Look/Glass.swift`, and a view asks it for its ground.
- **Smooth motion outranks glass**, since glass that SwiftUI changes is drawn again by the app, each frame. SwiftUI's glass keeps its value, tint and shape while anything animates, is never interactive, never fades or scales, and takes only the built-in shapes.
- **Glass that changes shape is `Morphed`** (`App/Look/Morphed.swift`): one UIKit glass whose frame the system animates, so the app makes no frame of the way. The page's bar melts into its small pill with it. SwiftUI's own morph (`glassEffectID`) costs what resizing the glass each frame does, and stays out. What's on a `Morphed` is a SwiftUI world of its own: it gets the environment values `Morphed` names, so a value the app adds to its environment is added there, and a change outside reaches it at its own pace only through `Held`.
- **The page's foot is laid out as Safari's.** `WebTab` tells WebKit what the bar covers, and WebKit does the rest: where a page pins something to its foot it fills what's covered with that colour, so the bar rests on a band, and elsewhere the page shows around the bar. Which pages get the band changes with the iOS version (GitHub does on 26, not on 27), so before calling one a fault, open the page in Safari on the same simulator.

## Verifying

Xcode 27.1, for iOS 26.0 and later. `ilo.xcodeproj` and `Blocking/Rules` aren't in the tree.

```sh
cd mobile/ios
Blocking/build.sh    # once in a new checkout, and after a pin or convert/ changes
xcodegen generate    # after adding, moving or removing any file
xcodebuild -project ilo.xcodeproj -scheme ilo -configuration Debug \
  -destination 'platform=iOS Simulator,name=<an iPhone on iOS 26 or later>' test
(cd Sync && swift test)
xcodebuild -project ilo.xcodeproj -scheme ilo -configuration Release \
  -destination 'generic/platform=iOS' build CODE_SIGNING_ALLOWED=NO
```

- **Done** is both test suites green, the Release compile, and a look at it running, in light and in dark, through the device tools.
- **Read the skipped count.** IloSync's tests start an ilo-server of their own from `~/.cache/ilo-ios-spike/ilo-server-target/release/ilo-server` and skip without it. Report a skip, not a pass.
- **CI runs the same three** (`.github/workflows/mobile-ios.yml`) for a change to `mobile/ios/` or to what the phone reads of the desktop's. The app's run on iOS 27 simulators alone: iOS 26, the oldest it runs on, is yours to run. IloSync's run with GitHub's `macos-26` Swift, an earlier one than Xcode 27.1's, so the package keeps to what its manifest's 6.0 allows.
- **CI runs a suite with a failed test once more, after the rest**, and the second run is its result: the runner's simulator holds a page's load for a minute or more at times in its first ten minutes, and a test run again at once meets the same hold. The job's summary names each suite run again; one named often has a fault of its own. A test's waits go by the clock (`eventually` in `Tests/Loopback.swift`) and add up to well under its suite's time limit, since a test that outlasts the limit ends the whole test process.
- **`ci_scripts/ci_post_clone.sh` makes the rules and the project** in CI, and is the script Xcode Cloud runs after it clones. A tool or a made file the build comes to need goes there.
- **A PR that changes only `mobile/ios/`** starts none of the desktop's jobs and needs none of its Mac runs or trains: a green Check is its gate.

## Versions

- **The version is in `project.yml`**: `MARKETING_VERSION`, and `CURRENT_PROJECT_VERSION`, the TestFlight build number, which goes up with every upload.
- **A phone release is the tag `ios-v<version>`**, such as `ios-v0.1.0`. `v<version>` is the desktop's and starts its release.
- **A phone tag gets no GitHub release**: the site's download buttons and the desktop's notes read the repository's latest one.
- **Its notes come from `git log ios-v<previous>..ios-v<version> -- mobile`**, so the desktop's changes stay out; the desktop's leave out a PR that changed only `mobile/`.
- **Uploading is by hand.** No workflow signs or uploads a build, and no signing secret is in the repository. Xcode Cloud, which signs with Apple's own certificates, isn't connected yet: connecting it is the owner's.
