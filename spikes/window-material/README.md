# Window material

**Question (#4087).** Which public AppKit material, with which tint, gives the sidebar's dark ground of the mocks: `#0a0a0b` over black, `#242426` over mid grey and `#3f3f40` over white, where Paper at 0.6 holds about 4.5:1? And in light, do today's material and tint stay near `#e7e7e7` over black and `#fdfdfd` over white and keep the Space's glow, or what's the smallest change that does?

**Answer.** Not yet: the numbers that count come from the check VM, and the answer goes on #4087.

## How it's measured

`./build.sh OUT` builds a small Mac app, `OUT/material`, with Xcode's command-line tools alone. `./capture.sh OUT` runs it and prints the table, which it also writes to `OUT/table.md`. It needs nothing else: no network, no sudo and no image tool. The app reads the captures itself.

For each candidate, in dark and in light, the app shows a window like the browser's: titled, its content the whole window, its title bar clear. In it are the material, a tint over the material as the UI draws one, and a sidebar-sized column of text samples: Paper (`0xf3f0e8`) in dark, Ink (`0x16140f`) in light, at 1.0, 0.6 and 0.42, at 13 and 11 pt. Behind it is a plain window of the app's own: white, 50% grey, black, or a busy picture the app draws of slanted bands and a checker. `tools/window-screenshot.sh` captures the app's windows alone, at the plain window's frame, so nothing else on screen gets in.

The candidates are `.underWindowBackground`, `.sidebar`, `.hudWindow`, `.fullScreenUI` and `.windowBackground`, plus the material the app uses today. Each is captured in `.behindWindow` blending, untinted and with the tint that brings it nearest its targets. Today's dark material is one AppKit doesn't name (29), so it's here only as today's row, never as a candidate.

- **The tint.** In dark, the brief's colour `rgb(20,20,23)`; in light, white, as today. Its alpha is fitted by least squares to the untinted grounds over white, grey and black: `a = Σ(C−G)(T−G) / Σ(C−G)²`, where C is the tint, G a ground and T its target. Light has no grey target. The fitted tint is then captured, not computed.
- **The ground.** The mean of an 80 pt patch clear of the text.
- **Contrast.** WCAG's ratio of the text colour at each step, blended over the ground, against the ground, taken over the backdrop where it's lowest. It's computed from the ground rather than read from the glyphs, as the UI blends its text with the ground the same way.
- **The chosen candidate in each appearance, and today's.** These are also captured with the window inactive, with the Space's glow over the picture, and, with `--reduce-transparency`, with Reduce Transparency on. The glow is a radial gradient from the foot's left corner, `rgba(122,112,244,.36)` to clear, 560 by 460 pt. `capture.sh`'s header says how it sets Reduce Transparency and puts it back.

Files are named `<material>-<tint>-<appearance>-<behind>.png`, with `-inactive`, `-glow` or `-reduce` at the end for those. A tint is `none` or `<rrggbb>a<alpha in thousandths>`.

What it doesn't measure:

- **The window's own active state.** A window of a process started from a script is seldom the key one, so the material's state is set to active or inactive rather than following the window, as the app's does.
- **The frame cost of each material.** That needs the real app, with its pages and the UI's frames (#4089).
- **Windows and Linux.** They draw no material. They draw the mid-grey composite (#3713), which is the table's "over grey" column.
