#!/usr/bin/env python3
"""The summary of a run of the core's tests (.github/workflows/core-tests.yml), from the summary
Chromium's test launcher writes with --test-launcher-summary-output: what passed, failed and was
skipped, by each test's last try, as the launcher retries a failed test.

Usage: test-summary.py <summary.json> <head> <ref> <filter>
Prints Markdown, and fails when no test ran, so a filter that matches nothing is never green.
"""
import json
import sys

FAILED = {"FAILURE", "FAILURE_ON_EXIT", "CRASH", "TIMEOUT", "EXCESSIVE_OUTPUT", "UNKNOWN", "NOTRUN"}


def main():
    path, head, ref, test_filter = sys.argv[1:5]
    with open(path) as summary:
        iterations = json.load(summary).get("per_iteration_data", [])
    tries = {}
    for iteration in iterations:
        for name, results in iteration.items():
            tries.setdefault(name, []).extend(result["status"] for result in results)
    passed = sorted(name for name, statuses in tries.items() if statuses[-1] == "SUCCESS")
    skipped = sorted(name for name, statuses in tries.items() if statuses[-1] == "SKIPPED")
    failed = sorted(name for name, statuses in tries.items() if statuses[-1] in FAILED)
    retried = [name for name in passed if len(tries[name]) > 1]
    print(f"### Core tests on {head[:12]}\n")
    print(f"Head {head} on {ref}, filter " + (f"`{test_filter}`" if test_filter else "none (every test)") + ".\n")
    counts = f"**{len(passed)} passed, {len(failed)} failed, {len(skipped)} skipped**"
    if retried:
        counts += f", {len(retried)} of those passed only on a retry: " + ", ".join(f"`{name}`" for name in retried)
    print(counts + ".")
    for name in failed:
        print(f"- failed: `{name}` ({tries[name][-1]})")
    if not tries:
        print("\nNo test ran" + (": the filter matches none." if test_filter else "."))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
