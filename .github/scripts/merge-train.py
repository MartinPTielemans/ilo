#!/usr/bin/env python3
"""The train the integrator would cut from the queue:ready pull requests, as a dry run (#3917 A3,
slice 1): each pull request in or held, by the rules the integrator's train applies, with whether
the orchestrator's word is on its head and whether the integrator merges it by hand. It pushes,
labels, comments and merges nothing.

It runs nothing from a pull request: each head is fetched as data, for git merge-tree, and the
rules it reads, tools/pr-rules.py and .github/merge-gated.txt, come from main's tree. A read that
fails or comes back empty is UNREAD, which makes the run fail, as a dry run that decided nothing
mustn't pass.

Usage: python3 -I merge-train.py <out dir> [<PR number> ...]
With no numbers, it takes the open queue:ready pull requests.
"""
import json
import os
import re
import subprocess
import sys

REPO = os.environ.get("GITHUB_REPOSITORY", "")
OWNER = REPO.split("/")[0]
WORD = "orchestrator/word"
# What a core batch carries: never a train's.
CORE = re.compile(r"^(core/|engine/(patches/|series$|pins\.ini$))")
# The other engine files, which the integrator merges by hand.
ENGINE = re.compile(r"^engine/")
# A merge by the workflow's own token can't hold a workflow file.
WORKFLOWS = re.compile(r"^\.github/workflows/")
# Bots' review checks, which the integrator's reading leaves out too.
IGNORED_CHECKS = re.compile(r"coderabbit", re.I)
# Green once something on the head has succeeded: a skipped run alone ran nothing.
GREEN = {"success", "skipped", "neutral"}


class Unread(Exception):
    """A read that failed or came back empty, so nothing about it can be decided."""


def run(*args, check=True):
    done = subprocess.run(args, capture_output=True, text=True)
    if check and done.returncode:
        raise Unread(f"{' '.join(args[:3])} failed: {done.stderr.strip()[:300]}")
    return done


def api(path, paginate=False):
    args = ["gh", "api", "-H", "Accept: application/vnd.github+json", path]
    if paginate:
        args[2:2] = ["--paginate", "--slurp"]
    out = run(*args).stdout
    try:
        data = json.loads(out)
    except ValueError:
        raise Unread(f"{path} gave no JSON")
    if paginate:
        data = [item for page in data for item in (page if isinstance(page, list) else [page])]
    return data


def git(*args, check=True):
    return run("git", *args, check=check)


def main_file(path):
    """A file as main has it, or None."""
    done = git("show", f"origin/main:{path}", check=False)
    return done.stdout if done.returncode == 0 else None


def word(sha):
    """The orchestrator's word on this commit: its allowclose list, or None. Only a success status
    the repository's owner posted counts; this job's token can't post one."""
    statuses = api(f"repos/{REPO}/commits/{sha}/statuses?per_page=100", paginate=True)
    mine = [s for s in statuses if s.get("context") == WORD]
    if not mine:
        return None
    newest = mine[0]  # The API lists the newest first.
    if newest.get("state") != "success" or (newest.get("creator") or {}).get("login") != OWNER:
        return None
    found = re.search(r"allowclose=([\d,]*)", newest.get("description") or "")
    return sorted(int(n) for n in found.group(1).split(",") if n) if found else []


def changed_files(base, head):
    """Every path the head changes from base. A rename is its old path and its new one, so a file
    moved out of a gated path is a change to that path."""
    return [f for f in git("diff", "--name-only", "--no-renames", base, head).stdout.splitlines() if f]


def not_green(runs, statuses, sha):
    """What's not green among the newest run of each workflow and event, and the other statuses.
    Empty when all are green. Unread when nothing on the head has succeeded, as a head whose runs
    were all skipped, such as a draft's, ran nothing."""
    newest = {}
    for r in sorted(runs, key=lambda r: r["created_at"]):
        if not IGNORED_CHECKS.search(r["name"]):
            newest[(r["workflow_id"], r["event"])] = r
    if not any(r["status"] == "completed" and r["conclusion"] == "success" for r in newest.values()):
        raise Unread(f"nothing ran on this head, {sha}: no run on it and its branch succeeded")
    bad = []
    for r in newest.values():
        if r["status"] != "completed":
            bad.append(f"{r['name']} ({r['event']}) {r['id']} not finished")
        elif r["conclusion"] not in GREEN:
            bad.append(f"{r['name']} ({r['event']}) {r['id']} {r['conclusion']}")
    for s in statuses:
        if s["context"] != WORD and not IGNORED_CHECKS.search(s["context"]) and s["state"] != "success":
            bad.append(f"status {s['context']} {s['state']}")
    return bad


def checks(sha, branch):
    """not_green() of the runs on this head and branch, and its statuses. A copy of the head on
    another branch, whose runs are often cancelled, doesn't count."""
    runs = [r for r in api(f"repos/{REPO}/actions/runs?head_sha={sha}&per_page=100").get("workflow_runs") or []
            if r["head_branch"] == branch]
    return not_green(runs, api(f"repos/{REPO}/commits/{sha}/status").get("statuses", []), sha)


def queue_order(number):
    """When the PR last got queue:ready, for the train's order."""
    events = api(f"repos/{REPO}/issues/{number}/events?per_page=100", paginate=True)
    times = [e["created_at"] for e in events
             if e.get("event") == "labeled" and (e.get("label") or {}).get("name") == "queue:ready"]
    return max(times) if times else ""


def closes(number):
    query = ("query($o:String!,$n:String!,$p:Int!){repository(owner:$o,name:$n){pullRequest(number:$p)"
             "{closingIssuesReferences(first:50){nodes{number}}}}}")
    out = run("gh", "api", "graphql", "-f", f"query={query}", "-f", f"o={OWNER}",
              "-f", f"n={REPO.split('/')[1]}", "-F", f"p={number}").stdout
    try:
        nodes = json.loads(out)["data"]["repository"]["pullRequest"]["closingIssuesReferences"]["nodes"]
    except (ValueError, KeyError, TypeError):
        raise Unread(f"#{number}'s closing issues couldn't be read")
    return sorted(n["number"] for n in nodes)


def decide(number, gated, rules):
    """One PR: (row for the summary, its head, the issues it closes, whether it's in)."""
    pr = api(f"repos/{REPO}/pulls/{number}")
    head = pr["head"]["sha"]
    row = {"pr": number, "head": head, "title": pr["title"], "held": []}
    held = row["held"]
    labels = {l["name"] for l in pr["labels"]}
    if pr["state"] != "open":
        held.append(f"{pr['state']}")
    if pr["draft"]:
        held.append("a draft")
    if pr["base"]["ref"] != "main":
        held.append(f"based on {pr['base']['ref']}, not main")
    if "queue:ready" not in labels:
        row["note"] = "no queue:ready (given by number)"
    fork = (pr["head"]["repo"] or {}).get("full_name") != REPO

    # The head as data: its commits, for the file list and merge-tree. Nothing of it is run.
    git("fetch", "-q", "--no-tags", "origin", f"+refs/pull/{number}/head:refs/mq/{number}")
    if git("rev-parse", f"refs/mq/{number}").stdout.strip() != head:
        raise Unread(f"#{number}'s fetched head isn't {head}")
    base = git("merge-base", "origin/main", head).stdout.strip()
    files = changed_files(base, head)
    if not files:
        raise Unread(f"#{number} changes no files from main")
    merges = git("rev-list", "--min-parents=2", f"{base}..{head}").stdout.split()

    # What's never a train's.
    if fork:
        held.append("a fork's head: the integrator's")
    if any(CORE.match(f) for f in files):
        held.append("a core batch's")
    # What rides the train but the integrator merges by hand. Slice 3, which merges, holds these.
    by = []
    if any(ENGINE.match(f) for f in files) and not any(CORE.match(f) for f in files):
        by.append("it changes engine/ files")
    if any(WORKFLOWS.match(f) for f in files):
        by.append("a workflow file, which this workflow's token can't merge")
    if merges:
        by.append("it holds a merge, so GitHub won't rebase it")
    row["merges_by"] = ("integrator: " + "; ".join(by)) if by else "train"

    bad = checks(head, pr["head"]["ref"])
    if bad:
        held.append("not green: " + "; ".join(bad))
    done = run("python3", "-I", rules, str(number), "--repo", REPO, check=False)
    if done.returncode:
        broken = (done.stdout + done.stderr).strip().splitlines()
        held.append("tools/pr-rules.py: " + " ".join(broken)[:300])

    allow = word(head)
    row["word"] = "yes" if allow is not None else "no"
    row["allowclose"] = allow or []
    issues = closes(number)
    row["closes"] = issues
    hit = [f for f in files if any(g.search(f) for g in gated)]
    row["gated"] = hit
    if hit and allow is None:
        older = [c for c in git("rev-list", "--max-count=10", f"{base}..{head}").stdout.split()[1:]
                 if word(c) is not None]
        where = f"; its word is on {older[0]}, not the head" if older else ""
        held.append(f"touches {', '.join(hit[:5])}{' …' if len(hit) > 5 else ''}: needs "
                    f"{WORD} at {head}{where}")
    if issues and issues != (allow or []):
        held.append(f"closes {', '.join(f'#{i}' for i in issues)}: needs {WORD} at {head} with "
                    f"allowclose={','.join(map(str, issues))}")
    return row, head, issues, not held


def main():
    out, numbers = sys.argv[1], sys.argv[2:]
    rules = os.path.join(out, "pr-rules.py")
    summary, unread = [], []
    try:
        if not REPO:
            raise Unread("GITHUB_REPOSITORY isn't set")
        git("fetch", "-q", "--no-tags", "origin", "+refs/heads/main:refs/remotes/origin/main")
        main_sha = git("rev-parse", "origin/main").stdout.strip()
        text = main_file("tools/pr-rules.py")
        if text is None:
            raise Unread("main has no tools/pr-rules.py")
        with open(rules, "w") as f:
            f.write(text)
        listed = main_file(".github/merge-gated.txt")
        source = "main's"
        if listed is None:
            # Only until this file reaches main: the copy in the commit this workflow runs at,
            # which is a branch here that someone with write access dispatched, never a PR's head.
            listed, source = open(".github/merge-gated.txt").read(), "this run's commit's (main has none yet)"
        gated = [re.compile(l) for l in listed.splitlines() if l.strip() and not l.startswith("#")]
        if not gated:
            raise Unread("the gated list is empty")
        if not numbers:
            found = api(f"repos/{REPO}/issues?labels=queue:ready&state=open&per_page=100", paginate=True)
            numbers = [str(i["number"]) for i in found if "pull_request" in i]
            if not numbers:
                raise Unread("no open pull request has queue:ready, so there's nothing to decide")
    except Unread as e:
        print(f"::error::UNREAD: {e}. Nothing decided.")
        summary.append(f"**UNREAD:** {e}. Nothing decided.")
        write(out, summary, [])
        return 1

    rows = []
    for n in numbers:
        try:
            if not n.isdigit():
                raise Unread(f"{n!r} isn't a pull request number")
            rows.append((queue_order(n), decide(int(n), gated, rules)))
        except Unread as e:
            unread.append(f"#{n}: {e}")
            print(f"::error::UNREAD #{n}: {e}")
    rows.sort(key=lambda r: (r[0] or "~", r[1][0]["pr"]))

    # Each PR that's in merges onto main and the ones before it, as the train would cut them.
    # The commits are local and never pushed; they carry no sign-off, as none lands.
    env = {**os.environ, "GIT_AUTHOR_NAME": "merge-train dry run", "GIT_AUTHOR_EMAIL": "noreply@github.com",
           "GIT_COMMITTER_NAME": "merge-train dry run", "GIT_COMMITTER_EMAIL": "noreply@github.com"}
    tip, train = main_sha, []
    for _, (row, head, issues, ok) in rows:
        if not ok:
            continue
        done = subprocess.run(["git", "merge-tree", "--write-tree", "--name-only", tip, head],
                              capture_output=True, text=True)
        if done.returncode == 1:
            # The tree, then the conflicted files up to a blank line, then git's messages.
            files = []
            for line in done.stdout.splitlines()[1:]:
                if not line:
                    break
                files.append(line)
            row["held"].append("conflicts with main" + (" and the train before it" if train else "")
                               + ": " + ", ".join(sorted(set(files))[:5]))
            continue
        if done.returncode:
            unread.append(f"#{row['pr']}: merge-tree failed: {done.stderr.strip()[:200]}")
            row["held"].append("merge-tree failed")
            continue
        tree = done.stdout.splitlines()[0]
        tip = subprocess.run(["git", "commit-tree", tree, "-p", tip, "-p", head, "-m",
                              f"dry run: #{row['pr']}, never to land"],
                             capture_output=True, text=True, env=env, check=True).stdout.strip()
        train.append(f"{row['pr']} {head}" + (f" {','.join(map(str, issues))}" if issues else ""))

    summary.append(f"Main at {main_sha}. The gated list is {source}. Nothing was pushed, labelled or merged.")
    summary.append("")
    summary.append("| PR | Head | In | Merges by | Word on head | allowclose | Closes | Why held |")
    summary.append("|---|---|---|---|---|---|---|---|")
    for _, (row, head, issues, ok) in rows:
        why = "; ".join(row["held"]) or row.get("note", "")
        summary.append(f"| #{row['pr']} | `{head[:12]}` | {'no' if row['held'] else 'yes'} | "
                       f"{row.get('merges_by', '–').replace('|', '/')} | {row['word']} | "
                       f"{','.join(map(str, row['allowclose'])) or '–'} | "
                       f"{','.join(f'#{i}' for i in row['closes']) or '–'} | {why.replace('|', '/')} |")
    if unread:
        summary.append("")
        summary.append("**UNREAD**, so this run fails: " + "; ".join(unread))
    write(out, summary, train)
    return 1 if unread or not rows else 0


def write(out, summary, train):
    with open(os.path.join(out, "train.txt"), "w") as f:
        f.write("".join(line + "\n" for line in train))
    text = "\n".join(summary) + "\n"
    print(text)
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if path:
        with open(path, "a") as f:
            f.write(text)


if __name__ == "__main__":
    sys.exit(main())
