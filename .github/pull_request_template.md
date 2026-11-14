<!--
Read CONTRIBUTING.md first. Review capacity is limited: meeting it makes a PR eligible for
review, not certain of a merge. One PR solves one underlying problem, and drafts meet the same
bar. Title: a conventional commit in plain language, such as
"fix(app): the sidebar recedes under the Library as Arc's does". Every commit is signed off
(git commit -s).
-->

## Problem

<!-- The problem in a sentence or two. Include expected behavior and how to
reproduce it where relevant. -->

## Change

<!-- How this fixes it. If it spans layers (engine, core, UI, platform), why
each part is needed for the same fix. Split independent fixes into their own PRs.
A surface that leaves Arc's or Dia's structure, behavior or timings lists each
difference and why it's better in every way, for the owner's review (decision 6). -->

## Scope and approval

<!-- Link the triaged issue, or the issue comment where a maintainer approved the
direction and scope. A very small fix for an obvious bug needs neither: say why it
qualifies. Maintainers can skip this section. -->

## Surfaces

<!-- Walk "Hit every surface" in AGENTS.md and say which entries applied and how you
covered them: UI and agents, keyboard with the page focused, over the page, text
input (IME), window states, profiles and Spaces, platforms, hot paths, docs, reverse
states. -->

## Verification

<!-- The checks you ran and what you observed. State anything you could not
check. "It builds" alone is not enough.

UI changes: before/after window captures (tools/window-screenshot.sh); motion
or timing: a short recording. Hot paths (tab switch, new tab, command bar,
launch): numbers against the performance gate in docs/roadmap.md, before and after.
Upload images and video to GitHub and embed them here. Never commit PR-only
assets. -->

Parity: none
<!-- The docs/ux-parity/ rows this completes, such as `Parity: spaces.md "Swipe tracking",
window.md "Base tint"`, or none. Leave their ticks, and the roadmap's counts, to the
integrator's daily parity-ticks PR, which tools/parity-ticks.sh lists them for. -->
