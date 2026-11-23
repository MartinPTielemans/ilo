# Window material

**Question (#4087).** Which public AppKit material, with which tint, gives the sidebar's dark ground of the mocks: `#0a0a0b` over black, `#242426` over mid grey and `#3f3f40` over white, where Paper at 0.6 holds about 4.5:1? And in light, do today's material and tint stay near `#e7e7e7` over black and `#fdfdfd` over white and keep the Space's glow, or what's the smallest change that does?

**Answer.** Not yet: the numbers that count come from the check VM, and the answer goes on #4087.

## How it's measured

`./build.sh OUT` builds a small Mac app, `OUT/material`, with Xcode's command-line tools alone. `./capture.sh OUT` runs it and prints the table, which it also writes to `OUT/table.md`. It needs nothing else: no network, no sudo and no image tool. The app reads the captures itself.

For each candidate, in dark and in light, the app shows a window like the browser's: titled, its content the whole window, its title bar clear. In it are the material, a tint over the material as the UI draws one, and a sidebar-sized column of text samples: Paper (`0xf3f0e8`) in dark, Ink (`0x16140f`) in light, at 1.0, 0.6 and 0.42, at 13 and 11 pt. Behind it is a plain window of the app's own: white, 50% grey, black, or a busy picture the app draws of slanted bands and a checker.

**The capture is the screen as composed.** The window server blends what's behind into the material on the screen alone. A capture of the app's windows (`tools/window-screenshot.sh`) leaves that out, which made the first runs' tables flat. So `capture.sh` takes the screen at the candidate window's frame with `screencapture -R`.
- **It runs only in a VM.** It checks the hardware (`kern.hv_vmm_present`, `hw.model`), never an environment variable, and refuses anywhere else. The check VM and GitHub's hosted Macs pass. A capture of the screen on the owner's Mac, or anyone's, is never allowed.
- **It refuses a dirty screen.**
  - **The spot.** The app puts its windows where no window that says by its bounds where it draws meets the backdrop: apps' windows, alerts and desktop widgets. It keeps 64 pt clear of each, as a window's bounds leave out its shadow, and out of the top right's 400 by 200 pt, where a banner can slide in, and goes nearest the screen's foot and left. The Dock's, Notification Center's and the window server's windows cover the whole screen, see-through, so their bounds say nothing.
  - **Each shot.** Before each one the app checks those windows again. If one meets the backdrop, the shot fails and names it.
  - **The pixel proof.** Before the shots and again at the end, the backdrop is shown alone in a flat colour no one meets (`#13a37f`, then `#d0158e`), captured as composed, and every pixel must be that colour within 2 a channel. Otherwise the run fails and names the rectangle that isn't, keeping the capture. That catches the Dock, a banner, a notification or anything else drawn over the spot.
  - **The pointer.** It's never in a capture: `screencapture` leaves it out without `-C`.
- **The wallpaper is never the backdrop.** The backdrop reaches 160 pt past the candidate window on each side, more than the blur's reach. Each chosen candidate is also shot over a ring: black, with white in the margin's outer half. The notes say whether its ground matches the ground over black, which shows that nothing past the backdrop reaches the material.

The candidates are `.underWindowBackground`, `.sidebar`, `.hudWindow`, `.fullScreenUI` and `.windowBackground`, plus the material the app uses today. Each is captured in `.behindWindow` blending, untinted and with the tint that brings it nearest its targets. Today's dark material is one AppKit doesn't name (29), so it's here only as today's row, never as a candidate.

- **The tint.** In dark, both the brief's colour `rgb(20,20,23)` and black, as the app tints today. The brief's colour can't take a ground below its own over black, `#141417`, so it can't reach the target `#0a0a0b` there. Black can. In light, the tint is white, as today. Its alpha is fitted by least squares to the untinted grounds over white, grey and black: `a = Σ(C−G)(T−G) / Σ(C−G)²`, where C is the tint, G a ground and T its target. Light has no grey target. The fitted tint is then captured, not computed.
- **The named candidates.** These are the window lane's picks, captured beside the fitted ones: `.hudWindow` under black at 0.64 and 0.66 in dark, and under white at 0.83 in light, next to the fitted 0.817 and today's 0.8. Two more see-through levels follow each, for a setting of three: black at 0.52 and 0.44 in dark, white at 0.70 and 0.58 in light.
- **What floats.** A menu or the command bar over a page blurs behind its window too, so the same window stands in for it: `.hudWindow` in dark under `#1f1d17` at 0.88, 0.78 and 0.56, over white, black and the picture.
- **The least tint (predicted).** The table also gives, for each untinted row and tint colour, the least alpha at which the text at 0.6 holds 4.5:1 over white, grey and black, with the grounds it predicts there. The prediction is `a·C + (1−a)·G`, which is how the tinted rows blend. The captured tinted rows check it.
- **The ground.** The mean of an 80 pt patch clear of the text.
- **Contrast.** WCAG's ratio of the text colour at each step, blended over the ground, against the ground, taken over the backdrop where it's lowest. It's computed from the ground rather than read from the glyphs, as the UI blends its text with the ground the same way.
- **The chosen candidate in each appearance, the named ones, what floats and today's.** These are also captured with the window inactive, with the Space's glow over the picture, and, with `--reduce-transparency`, with Reduce Transparency on. The glow is a radial gradient from the foot's left corner, `rgba(122,112,244,.36)` to clear, 560 by 460 pt. `capture.sh`'s header says how it sets Reduce Transparency and puts it back.

Files are named `<material>-<tint>-<appearance>-<behind>.png`, with `-inactive`, `-glow` or `-reduce` at the end for those, and `ring` as the behind for the margin's check. Each is the candidate window's frame alone, at about 55 KB, so a run's output is about 10 MB with the named candidates. A tint is `none` or `<rrggbb>a<alpha in thousandths>`.

What it doesn't measure:

- **The window's own active state.** A window of a process started from a script is seldom the key one, so the material's state is set to active or inactive rather than following the window, as the app's does.
- **The frame cost of each material.** That needs the real app, with its pages and the UI's frames (#4089).
- **Windows and Linux.** They draw no material. They draw the mid-grey composite (#3713), which is the table's "over grey" column.
