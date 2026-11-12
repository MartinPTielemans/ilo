# adblock-rust against uBlock Origin

**Question.** Could ilo block ads with a native engine instead of the uBlock Origin extension, and do it at least as well (#2069)? Is Brave's [adblock-rust](https://github.com/brave/adblock-rust) (MPL-2.0) that engine, or would ilo need a matcher of its own?

**Answer.** adblock-rust is that engine, and a matcher of ilo's own isn't needed. On the lists ilo's uBO turns on:

- adblock-rust blocks the same requests as uBO, all but 25 of 242,945.
- On Linux, Windows and the Mac, it compiles the lists 13 to 17 times faster. It loads from its cache 15 to 32 times faster. Once loaded, it holds a fifth of uBO's memory or less, and its peak is about a tenth of uBO's.
- Both engines take about 10 µs a request. uBO's median is a few µs lower, and adblock-rust's p99 is the same or lower.

What it doesn't do is mostly what any engine inside ilo has to do in the browser itself: pop-ups, the element picker, list updates. Its gaps in filter syntax are a short list of options, which would go upstream to Brave (below).

## How it's measured

`./run.sh [WORK]` needs cargo, node, Python 3, git, curl and unzip, and runs under Git Bash on Windows too. It downloads into `WORK` (`~/.cache/ilo-adblock-spike`):

- uBlock Origin's release that `app/ublock.ini` names, checked against its sha256. Its `assets.json` says which lists uBO turns on, and it has the public suffix list.
- uBO's source at that release. Its `tools/make-nodejs.sh` builds uBO's own network filtering engine for Node.
- The nine lists uBO turns on by default, with their `!#include`s resolved as uBO does: uBlock filters, Badware risks, Privacy, Unbreak, Quick fixes, EasyList, EasyPrivacy, URLhaus and Peter Lowe's. That's 268,000 lines and 16 MB.
- Brave's crawl of 242,945 requests from about 600 sites, made in 2019, each with its frame and type. A sample of every 48th, 5,062 requests, gives the time per request.

Each engine's step runs in a process of its own: compile the lists and write the cache, load from the cache (three times each), then match every request twice, timing the second pass. `compare.py` sets their answers side by side request by request. `adblock-spike syntax` counts the lines adblock-rust can't use. `resources.mjs` turns uBO's 151 scriptlets and 47 redirect stand-ins into adblock-rust's resources, so scriptlets and `$redirect` are measured on uBO's own code.

What it doesn't measure:

- **uBO in Node is its network engine alone.** Its cosmetic, scriptlet and redirect engines need a browser. The adblock-rust numbers include its cosmetic filters, which makes the comparison generous to uBO. The whole uBO extension process is measured in the browser by `app/check/adblock-live.sh`.
- **The crawl gives each frame's document as its own frame.** A page can't be told from a frame, so both engines take documents as frames.

## Results

From `run.sh` on a Linux desktop (x86-64, Ryzen), and on GitHub's `windows-latest` and `macos-latest` runners (Apple silicon), from the workflow on the proof branch `spike/adblock-rust-2`. In each cell, adblock-rust 0.13.5 comes first and uBO 1.75.0's engine in Node 24 second. Compile and load times are medians of three, each in a fresh process.

Memory is the process's resident set, which is the working set on Windows. "Held" is what the engine adds once it's built. "Peak" is the most the process reached. On the Mac, Rust can only read the peak without Mach calls, so adblock-rust's "held" figure there is its peak.

| | Linux | Windows | Mac |
| --- | --- | --- | --- |
| Compile the nine lists | 217 / 2,851 ms | 296 / 5,109 ms | 271 / 4,574 ms |
| Write its cache | 8 / 1,453 ms | 6 / 1,900 ms | 4 / 2,397 ms |
| Size of its cache | 21 / 43 MB | 21 / 43 MB | 21 / 43 MB |
| Load from its cache | 17 / 460 ms | 15 / 478 ms | 39 / 572 ms |
| Memory held once compiled | 28 / 357 MB | 23 / 317 MB | 126 / 1,313 MB |
| Memory held once loaded from its cache | 22 / 112 MB | 21 / 267 MB | 44 / 292 MB |
| Peak while compiling | 93 / 790 MB | 90 / 680 MB | 126 / 1,746 MB |
| Peak while loading | 45 / 419 MB | 45 / 402 MB | 44 / 450 MB |
| Time a request, mean (all 242,945) | 7.5 / 6.5 µs | 12.5 / 9.0 µs | 10.3 / 9.1 µs |
| p50 | 6.2 / 4.2 µs | 9.4 / 5.5 µs | 7.3 / 4.8 µs |
| p99 | 26 / 33 µs | 51 / 51 µs | 54 / 63 µs |
| Slowest | 0.2 / 8.5 ms | 17.7 / 8.2 ms | 1.5 / 8.3 ms |
| Requests blocked, of 242,945 | 109,432 / 109,457 | the same | the same |
| Of those, answered with a stand-in | 7,257 / 7,273 | the same | the same |

The answers are identical on all three. adblock-rust's slowest request on Windows, 17.7 ms, was a single request: its slowest in the sample pass was 0.16 ms. On the Mac, Node's resident set stayed above 1.4 GB after compiling, so the peak is the fairer comparison there.

### What each blocks

Both block 109,432 requests, and both let 133,488 through. adblock-rust blocks nothing that uBO doesn't. uBO blocks 25 more, on 9 hosts, for three reasons:

- `@@||google-analytics.com/analytics.js$script,redirect-rule=…,domain=stern.de`: in uBO an exception with `redirect-rule` lifts only the redirect, and the request stays blocked. adblock-rust takes it as an exception to the block, so Analytics and Tag Manager load on stern.de.
- `*$script,3p,denyallow=…,from=igg-games.com`: adblock-rust has no `denyallow`, so it drops the filter.
- `/webtracking/*$~frame,~document,from=~wwwapps.ups.com`: it doesn't know `~frame`.

### Lines adblock-rust can't use

Of the lists' 263,927 filters, it takes 216,284 network filters, 32,681 cosmetic filters and 10,445 scriptlet filters. It can't use 4,517 (1.7%):

| Lines | What | Gap against ilo's uBO? |
| --- | --- | --- |
| 3,136 | `$popup`, `$popunder` | No. adblock-rust leaves pop-ups to the browser, which any engine in ilo needs anyway (Brave does it in its browser code). |
| 413 | HTML filtering, `##^` | No. uBO does it only on Firefox. Chromium gives an extension no response bodies. |
| 70 | `$replace` | No, for the same reason. |
| 31 | `$cname`, `$ipaddress` | No. Firefox only. |
| 280 | `$doc` | Yes. It's uBO's short name for `$document`, which adblock-rust knows, so it's an alias to add. |
| 261 | `denyallow` | Yes. It's a small option to add. |
| 50 | `$urlskip`, `$uritransform` | Yes. These rewrite a navigation and need the browser's help. |
| 37 | `$match-case` | Yes, small. |
| 47 | `$strict1p`, `$strict3p`, `$to`, `$method`, `$header` | Yes, small to medium. `$header` needs the response's headers. |
| 41 | `@@…$redirect-rule` | Yes, and it's a semantic difference (above). |
| 25 | `$removeparam` with a regex, or empty | Partly. |
| 19 | scriptlet lines it can't parse | Yes, a parser fix. |
| 11 | `$permissions` | Yes. It sets a response header. |
| the rest | 16 cosmetic and a few others | Small. |

### Cosmetic filters and scriptlets

With uBO's resources loaded, adblock-rust gives a page what to hide and what to run in under 0.5 ms:

| Page | CSS selectors to hide | Scriptlet code |
| --- | --- | --- |
| youtube.com, a video | 19 (it has `generichide`) | 86 KB |
| bbc.com/news | 566 | 15 KB |
| cnn.com | 572 | 54 KB |
| allrecipes.com | 564 | 26 KB |

That YouTube's ads go is for the parity gate to show in a browser. adblock-rust runs uBO's scriptlets as uBO ships them, so the code is the same. What ilo has to supply is the injection: into the page's main world, at document start, in every frame. Brave does this in its browser code, and no agent can reach it (principle 3).

## What a native engine still needs from ilo

- Pop-ups, `$urlskip` and the strict-blocking page (uBO's "page blocked"): browser code, as for Brave.
- Injecting CSS and scriptlets into frames at document start: browser code, in the renderer.
- The element picker and My filters: UI, plus compiling the user's own filters locally.
- List updates: the plan issue covers them.
- The few options above, which belong upstream in adblock-rust.

## Manifest V2 on Chromium 156

uBO needs Manifest V2. ilo's engine keeps it with no patch of its own: ungoogled-chromium carries `core/ungoogled-chromium/extensions-manifestv2.patch`, 60 lines over four files. It makes `ShouldDisableLegacyExtensions()` return false, and it takes out the deprecation warning and the extensions page's MV2 notices. It has been rebased every major since 126 (31 updates through 156). Each major so far has cost ilo nothing for it.

The risk is Chromium deleting the code MV2 runs on: blocking `webRequest` and persistent background pages. A one-function patch can't bring those back. Ending up there would mean carrying a large revert each major, or a native engine.
