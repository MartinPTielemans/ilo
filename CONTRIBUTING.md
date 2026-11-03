# Contributing

ilo is pre-alpha, with one maintainer and limited time to review. A pull request doesn't oblige
anyone to review or merge it: we may close or defer it, ask for a smaller one, or build the idea
another way later. Meeting this guide makes a PR eligible for review, not certain of a merge. The
same bar applies to drafts.

Focused bug fixes, reliability and performance work are the most likely to land. Features nobody
agreed to, rewrites and unrelated cleanup are not.

## Where things go

| What | Where |
|---|---|
| A bug in ilo | An issue, with the **Bug report** form |
| A site that works in Chrome but not in ilo | The **Site doesn't work** form |
| A crash or a hang | The **Crash or hang** form, with the crash report or a `sample` |
| An agent or the MCP server misbehaving | The **Agents and MCP** form |
| An idea | The **Feature request** form, after reading [docs/decisions.md](docs/decisions.md) and [docs/ux-principles.md](docs/ux-principles.md) |
| A security vulnerability in ilo | [Private vulnerability reporting](https://github.com/MartinPTielemans/ilo/security/advisories/new), never an issue. See [SECURITY.md](.github/SECURITY.md) |
| A bug that happens in Chrome too | [Chromium's tracker](https://issues.chromium.org) |

Reproduce on public pages. Never put private URLs, cookies, account data or what a logged-in page
shows in an issue, a PR or an attachment.

A coding agent can investigate a problem on your Mac and draft the issue for you: ask it to follow
[.github/triage/PLAYBOOK.md](.github/triage/PLAYBOOK.md). It files with the `via-triage` form, and
shows you the issue before posting it.

## Vouching

[.github/VOUCHED.td](.github/VOUCHED.td) lists the people ilo trusts, using
[vouch](https://github.com/mitchellh/vouch). On every PR, a check labels its author
`vouch:trusted` (in the list, or a collaborator with write access), `vouch:unvouched`, or
`vouch:denounced`. A comment of `/recheck-vouch` on the PR labels it again.

The label is context for triage, not a gate: an unvouched author's PR is triaged like anyone's, and
a vouched author's still goes through triage. A denounced author's PRs and issues are closed
without review.

Maintainers vouch. Once you've shown care here, in issues or a PR that landed, a maintainer adds
you to the list in a commit to `main`, which relabels every open PR. You can't vouch for yourself
in your own PR. A maintainer denounces the same way, with a reason, for bad faith such as spam or
repeated low-effort PRs.

## Before you start

Agree on the problem and the scope first:

- **Features and changes to behavior** need a maintainer's approval of the direction and scope in
  an issue, before you build them. Link the approval in your PR. Calling a behavior change a bug
  fix doesn't exempt it.
- **A substantial bug fix** links an issue a maintainer has triaged, which establishes the failure
  and the intended behavior.
- **A very small, focused fix for an obvious bug** can come without an issue. Say why it qualifies.
  There's no line cutoff: we judge purpose and effect.

Settled decisions stay settled. One reopens only with the evidence its entry in
[docs/decisions.md](docs/decisions.md) names, and some are off the table entirely: Electron as the
product, WebKit or Gecko as ilo's engine, and closed source.

## One problem per PR

A PR that solves several independent problems is split, even when each fix is useful. Count the
underlying problems, not the linked issues. Include the engine, core, UI, platform, test and doc
changes that one problem needs, and say how they relate when it isn't obvious. Maintainers may
batch related fixes.

## Evidence

Say how you established the problem, how you checked the change, and what you saw. Name the checks
you ran and state what you couldn't check. A checkbox or "it builds" doesn't show the change works.
[AGENTS.md](AGENTS.md) says how to verify each part of the browser.

- **Every surface.** Walk "Hit every surface" in AGENTS.md and say which entries applied. The most
  common defect is a change that works on the path you tested and nowhere else.
- **UI.** Before-and-after window captures (`tools/window-screenshot.sh`), and a short recording
  (`tools/window-record.sh`) when motion or timing matters. Upload them to the PR; never commit
  PR-only assets.
- **Hot paths.** A change to tab switch, new tab, the command bar or launch comes with before and
  after numbers against the performance gate in [docs/roadmap.md](docs/roadmap.md).

We judge the problem, the scope and the evidence, not the writing style or whether an agent helped.

## Triage

Triage checks that the problem is established, any approval it needs is present, the scope is one
problem, and the evidence is enough. Passing it isn't approval of correctness, security or a
merge; review does that.

New issues get `needs-triage`. An issue a maintainer labels `needs more info` closes after 7 days
without a reply, so answer before then. PRs get a `size:` label from their changed lines, tests
left out. A PR that makes [AGENTS.md](AGENTS.md) and the docs it links to larger fails the agent
context check, unless a maintainer accepts the growth with the `agent-context-increase` label:
every byte there is read by every agent on every task.

CI runs only the jobs a change touches (`.github/scripts/ci-changes.cjs`), on GitHub's runners.

A PR closed for one of these rules is told which rule, with the evidence and a link here. Fix it
and ask for another look, or open the focused PRs and link them to the first.

## Sign-off

Every commit needs a `Signed-off-by:` line. It certifies the [Developer Certificate of Origin](https://developercertificate.org/): you wrote the change, or you have the right to submit it under this project's license.

```sh
git commit -s
```

Authorship is human: you sign off and answer for the change, however it was written. Leave out
`Co-Authored-By` trailers for AI tools and "Generated with" lines.

## Code from other projects

The browser is GPL-3.0-or-later, so code under GPL-3.0-compatible licenses can come in: Chromium (BSD-3-Clause), ungoogled-chromium (BSD-3-Clause), Helium (GPL-3.0), Ardia (Apache-2.0 / GPL-3.0), GPUI (Apache-2.0), Lucide's icons (ISC, some MIT), T3 Code (MIT).

When you copy or adapt a patch or file:

- Keep its original copyright and license header.
- Name the source and the upstream commit in the patch header or the commit message.
- Add it to `NOTICE` if its license asks for attribution.

Don't bring in assets from Arc, Dia or any other browser: icons, fonts, sounds, images or names. Measured values (sizes, timings, colors) and behavior are fine.

## Performance

Speed is the product. Changes on hot paths (tab switch, new tab, command bar, launch) must not regress the benchmarks in `docs/roadmap.md`. A PR that does needs a stated reason.

## Portability

No Swift, Objective-C or Metal outside `platform/mac/`. Core code must compile on Linux.
