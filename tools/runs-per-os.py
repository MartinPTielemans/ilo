#!/usr/bin/env python3
"""Prints, for X11, Wayland and Windows, what each of check.sh's default runs is there (#2179).

A run passes, is skipped with the reason its CI workflow gives, or isn't listed. Under each run that
passed, it prints the steps the run skipped and why, from that run's log. The table is what parity
on these systems means: every run and step not proven on a system is in it with a reason. A tick
in docs/ux-parity/ is the Mac's.

Each skip's reason is one of three kinds. One that cites an issue (#N) is open and tracked; one that
says "yet" or "so far" with none is open and untracked, listed again at the end, as each needs an
issue or a better reason; the rest are the system's nature, the only kind that's done.

The lists come from check-linux.yml and check-windows.yml, and the results and logs from one run of
each, such as a nightly, its artifacts downloaded with gh:

  gh run download <check-linux run> -p check-linux -p 'results-*' -D <linux dir>
  gh run download <check-windows run> -p 'check-windows*' -p 'results-*' -D <windows dir>

Usage: python3 -I tools/runs-per-os.py [--linux <linux dir>] [--windows <windows dir>]
Without a directory, a system's listed runs show as listed, with no result.
"""
import argparse
import glob
import os
import re

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
FIRST_LINE = re.compile(r'^# check\.sh runs this at ([0-9]+)(, only when named)?\.$')
ISSUE = re.compile(r'#[0-9]+')
# "So far" says the same as "yet".
YET = re.compile(r'\byet\b|\bso far\b', re.IGNORECASE)


def kind(reason):
    """A skip's reason: tracked when it cites an issue, untracked when it says "yet" without one,
    and otherwise the system's nature. Only the last is done."""
    if ISSUE.search(reason):
        return 'tracked'
    if YET.search(reason):
        return 'untracked'
    return 'nature'


def default_runs():
    """check.sh's default runs, in its order: by number, a tie by name (check.sh's runs_of)."""
    runs = []
    for path in glob.glob(os.path.join(ROOT, 'app', 'check', '*.sh')):
        with open(path, encoding='utf-8') as file:
            match = FIRST_LINE.match(file.readline().rstrip('\n'))
        if match and not match.group(2):
            runs.append((int(match.group(1)), os.path.basename(path)[:-3]))
    return [name for _, name in sorted(runs)]


def env_blocks(workflow):
    """The workflow's env values written as block scalars (KEY: >- or KEY: |), by key."""
    with open(os.path.join(ROOT, '.github', 'workflows', workflow), encoding='utf-8') as file:
        lines = file.read().split('\n')
    blocks = {}
    for i, line in enumerate(lines):
        match = re.match(r'^(\s*)([A-Z_]+): ([>|]-?)\s*$', line)
        if not match:
            continue
        indent = len(match.group(1))
        body = []
        for following in lines[i + 1:]:
            if following.strip() and len(following) - len(following.lstrip()) <= indent:
                break
            body.append(following.strip())
        blocks[match.group(2)] = body
    return blocks


def listed(block):
    return {run for line in block for run in line.split() if not line.startswith('#')}


def skips(block):
    out = {}
    for line in block:
        if ': ' in line and not line.startswith('#'):
            run, why = line.split(': ', 1)
            out[run] = why
    return out


def results(directory, session):
    """Each run's result row in the session's results-<session>-<shard> files, by run."""
    out = {}
    if not directory:
        return out
    for path in sorted(glob.glob(os.path.join(directory, 'results-%s-*' % session, '*.tsv'))):
        with open(path, encoding='utf-8') as file:
            for line in file:
                if '\t' in line:
                    run, result = line.rstrip('\n').split('\t', 1)
                    out[run] = result
    return out


def skip_lines(log):
    """The log's skip lines, each once, in their order."""
    if not log or not os.path.exists(log):
        return []
    seen = []
    with open(log, encoding='utf-8', errors='replace') as file:
        for line in file:
            line = line.rstrip('\n')
            if line.startswith('skip: ') and line not in seen:
                seen.append(line)
    return seen


def session(name, runs, on, off, found, log_of):
    """Prints the session's table and gives its untracked skips, each as (run, reason)."""
    rows, counts = [], {'pass': 0, 'fail': 0, 'skipped': 0, 'not listed': 0, 'no result': 0,
                    'listed': 0}
    for run in runs:
        steps = []
        if run in off:
            state = 'skipped: %s' % off[run]
            counts['skipped'] += 1
        elif run in on:
            result = found.get(run)
            if result is None:
                state = 'listed, no result in this run' if found else 'listed'
                counts['no result' if found else 'listed'] += 1
            elif result.startswith('passed'):
                # The skip lines follow under it, so the row's own copy of them goes.
                state = 'passes (%s)' % result.split(', skipping: ')[0]
                counts['pass'] += 1
                steps = skip_lines(log_of(run))
            elif result.startswith('skip: '):
                state = 'skipped: %s' % result[6:]
                counts['skipped'] += 1
            else:
                state = result
                counts['fail'] += 1
        else:
            state = 'not listed, no reason given'
            counts['not listed'] += 1
        rows.append((run, state, steps))
    kinds = {'nature': 0, 'tracked': 0, 'untracked': 0}
    untracked = []
    for run, state, steps in rows:
        reasons = [state[9:]] if state.startswith('skipped: ') else []
        reasons += [step[6:] for step in steps]
        for reason in reasons:
            kinds[kind(reason)] += 1
            if kind(reason) == 'untracked':
                untracked.append((run, reason))
    said = ', '.join('%d %s' % (n, what) for what, n in counts.items() if n)
    print('%s: %d default runs: %s' % (name, len(runs), said))
    print('  skips, runs and steps: %d for the system\'s nature, %d open with an issue, %d open with'
          ' none' % (kinds['nature'], kinds['tracked'], kinds['untracked']))
    width = max(len(run) for run in runs)
    for run, state, steps in rows:
        print('  %-*s  %s' % (width, run, state))
        for step in steps:
            print('  %-*s    %s' % (width, '', step))
    print()
    return untracked


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--linux', help="a check-linux run's artifacts, downloaded")
    parser.add_argument('--windows', help="a check-windows run's artifacts, downloaded")
    args = parser.parse_args()
    runs = default_runs()
    linux = env_blocks('check-linux.yml')
    windows = env_blocks('check-windows.yml')
    linux_off = skips(linux['LINUX_SKIPS'])

    def logs(directory, folder):
        if not directory:
            return lambda run: None

        # A sharded run uploads each shard's logs as <folder>-<shard>.
        def log(run):
            found = glob.glob(os.path.join(directory, folder, '%s.log' % run)) + sorted(
                glob.glob(os.path.join(directory, folder + '-*', '%s.log' % run)))
            return found[0] if found else None
        return log

    untracked = []
    untracked += [('X11',) + skip for skip in session('X11 (Xvfb)', runs, listed(linux['LINUX_RUNS']), linux_off,
            results(args.linux, 'xvfb'), logs(args.linux, 'check-linux'))]
    untracked += [('Wayland',) + skip for skip in session('Wayland (sway)', runs, listed(linux['WAYLAND_RUNS']), linux_off,
            results(args.linux, 'wayland'), logs(args.linux, os.path.join('check-linux', 'wayland')))]
    untracked += [('Windows',) + skip for skip in session('Windows', runs, listed(windows['WINDOWS_RUNS']), skips(windows['WINDOWS_SKIPS']),
            results(args.windows, 'windows'), logs(args.windows, 'check-windows'))]
    # Each needs an issue, or a reason that's the system's nature.
    print('Open with no issue, each needing one or a better reason: %d' % len(untracked))
    for system, run, reason in untracked:
        print('  %s, %s: %s' % (system, run, reason))


main()
