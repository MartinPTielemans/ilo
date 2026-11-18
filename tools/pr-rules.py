#!/usr/bin/env python3
"""Checks a pull request's body against the rules a script can decide (#3392), before a merge:

- No line credits an AI tool. Authorship is human (CONTRIBUTING.md#sign-off): a Co-Authored-By
  with a tool's address or its fixed trailer's name, or a "Generated with" line naming a tool.
  These are the same rules DCO applies to commit messages.
- A "Parity:" line names the docs/ux-parity/ rows the PR completes, or none, as the template asks.
  A batch PR (batch/*, scratch/batch-*, scratch/landing-*) carries its parts' lines and is skipped.
- A PR that changes the look, app/design/tokens.toml or app/assets/, shows it: an image or a
  video in its body (CONTRIBUTING.md#evidence).

It reads the body, branch and files through gh, so it never checks out the PR's code. It prints one
line per broken rule, saying the rule and what to do, and exits 1 if there are any.

Usage: python3 -I tools/pr-rules.py <PR number> [--repo owner/name]
"""
import argparse
import json
import re
import subprocess
import sys

# A tool's Co-Authored-By is told by what only a tool has: its maker's address, a bot's GitHub
# address, or a name that is the tool's own fixed trailer. A person called Claude or Devin passes.
CO_AUTHOR = re.compile(r"^co-authored-by:\s*([^<]*?)\s*(?:<([^<>\s]*)>)?\s*$", re.I)
TOOL_ADDRESS = re.compile(r"@(?:[\w-]+\.)*(?:anthropic\.com|openai\.com|cursor\.com|cognition\.ai)$"
                          r"|^(?:\d+\+)?(?:copilot|[\w-]+\[bot\])@users\.noreply\.github\.com$", re.I)
TOOL_NAME = re.compile(r"^(?:claude code|claude (?:opus|sonnet|haiku|fable)\b.*|(?:github )?copilot"
                       r"|cursor agent|devin ai|chatgpt)$", re.I)
# Nobody signs as generated, so any tool's name there will do. The line starts with it, or with the
# robot Claude Code puts before it, so one quoted in a table or in code is a quotation.
GENERATED = re.compile(r"^(?:\U0001F916\s*)?generated (with|by)\b.*\b(claude|anthropic|openai|codex|copilot|cursor"
                       r"|chatgpt|gpt|gemini|devin)\b", re.I)
PARITY = re.compile(r"^Parity: \S", re.M)
BATCH = re.compile(r"^(batch/|scratch/batch-|scratch/landing-)")
LOOK = re.compile(r"^(app/design/tokens\.toml$|app/assets/)")
PICTURE = re.compile(r"!\[|<img\b|<video\b|github\.com/user-attachments/", re.I)


def credits_a_tool(line):
    line = line.strip()
    found = CO_AUTHOR.match(line)
    if found and (TOOL_ADDRESS.search(found.group(2) or "") or TOOL_NAME.match(found.group(1))):
        return True
    return bool(GENERATED.match(line))


def broken(body, branch, files):
    """The rules the PR breaks, each as what to do."""
    found = []
    for line in body.splitlines():
        if credits_a_tool(line):
            found.append(f"a line credits an AI tool, \"{line.strip()}\": authorship is human "
                         "(CONTRIBUTING.md#sign-off). Take the line out of the body.")
            break
    if not BATCH.match(branch) and not PARITY.search(body):
        found.append("no \"Parity:\" line: add one naming the docs/ux-parity/ rows this completes, "
                     "such as `Parity: spaces.md \"Swipe tracking\"`, or `Parity: none`, as the "
                     "PR template has it.")
    look = [f for f in files if LOOK.match(f)]
    if look and not PICTURE.search(body):
        found.append(f"it changes the look ({', '.join(look[:3])}{', …' if len(look) > 3 else ''}) "
                     "with no picture: upload before-and-after window captures "
                     "(tools/window-screenshot.sh) to GitHub and embed them in the body "
                     "(CONTRIBUTING.md#evidence).")
    return found


def gh(*args):
    return subprocess.run(["gh", *args], capture_output=True, text=True, check=True).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("number", type=int)
    parser.add_argument("--repo", default="MartinPTielemans/ilo")
    args = parser.parse_args()
    pr = json.loads(gh("pr", "view", str(args.number), "--repo", args.repo,
                       "--json", "body,headRefName"))
    # The API's file list, as gh pr view's stops at 100.
    files = gh("api", "--paginate", f"repos/{args.repo}/pulls/{args.number}/files",
               "--jq", ".[].filename").split()
    found = broken(pr["body"] or "", pr["headRefName"], files)
    for message in found:
        print(f"#{args.number}: {message}")
    if not found:
        print(f"#{args.number}: passes")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
