#!/usr/bin/env python3
"""Tests of merge-train.py's decisions that need no GitHub: the files a head changes, on a local
repository, whether its runs are green, and the gated list. The merge-train workflow runs these
before it decides anything.

Usage: python3 -I .github/scripts/merge-train-test.py
"""
import importlib.util
import os
import re
import subprocess
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("merge_train", os.path.join(HERE, "merge-train.py"))
mt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mt)

with open(os.path.join(HERE, "..", "merge-gated.txt")) as f:
    GATED = [re.compile(l) for l in f.read().splitlines() if l.strip() and not l.startswith("#")]


def gated(path):
    return any(g.search(path) for g in GATED)


def git(cwd, *args):
    env = {**os.environ, "GIT_AUTHOR_NAME": "test", "GIT_AUTHOR_EMAIL": "test@example.com",
           "GIT_COMMITTER_NAME": "test", "GIT_COMMITTER_EMAIL": "test@example.com",
           "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
    return subprocess.run(["git", *args], cwd=cwd, env=env, check=True, capture_output=True,
                          text=True).stdout.strip()


class ChangedFiles(unittest.TestCase):
    def setUp(self):
        self.old = os.getcwd()
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = self.tmp.name
        git(self.repo, "init", "-q", "-b", "main")
        os.makedirs(os.path.join(self.repo, "app", "check"))
        with open(os.path.join(self.repo, "app", "check", "lib.sh"), "w") as f:
            f.write("".join(f"line {i}\n" for i in range(40)))
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", "base")
        self.base = git(self.repo, "rev-parse", "HEAD")
        os.chdir(self.repo)

    def tearDown(self):
        os.chdir(self.old)
        self.tmp.cleanup()

    def test_a_file_moved_out_of_a_gated_path_is_a_change_to_it(self):
        os.makedirs(os.path.join(self.repo, "docs"))
        git(self.repo, "mv", "app/check/lib.sh", "docs/lib.sh")
        git(self.repo, "commit", "-q", "-m", "move")
        head = git(self.repo, "rev-parse", "HEAD")
        # git diff finds this rename unless told not to, and names only docs/lib.sh then.
        self.assertIn("R100", git(self.repo, "diff", "--name-status", "-M", self.base, head))
        files = mt.changed_files(self.base, head)
        self.assertEqual(sorted(files), ["app/check/lib.sh", "docs/lib.sh"])
        self.assertTrue(any(gated(f) for f in files))

    def test_an_edit_is_its_path(self):
        with open(os.path.join(self.repo, "app", "check", "lib.sh"), "a") as f:
            f.write("one more\n")
        git(self.repo, "commit", "-q", "-am", "edit")
        self.assertEqual(mt.changed_files(self.base, git(self.repo, "rev-parse", "HEAD")),
                         ["app/check/lib.sh"])


def run(name, conclusion, status="completed", created="2026-11-23T04:00:00Z", workflow=1, event="push"):
    return {"name": name, "conclusion": conclusion, "status": status, "created_at": created,
            "workflow_id": workflow, "event": event, "id": workflow * 100}


class NotGreen(unittest.TestCase):
    def test_a_head_whose_runs_were_all_skipped_ran_nothing(self):
        runs = [run("CI", "skipped", workflow=1), run("DCO", "neutral", workflow=2)]
        with self.assertRaisesRegex(mt.Unread, "nothing ran on this head"):
            mt.not_green(runs, [], "abc")

    def test_no_runs_is_unread(self):
        with self.assertRaisesRegex(mt.Unread, "nothing ran on this head"):
            mt.not_green([], [], "abc")

    def test_a_review_bot_alone_is_not_a_run(self):
        with self.assertRaises(mt.Unread):
            mt.not_green([run("CodeRabbit", "success")], [], "abc")

    def test_a_success_beside_skipped_runs_is_green(self):
        runs = [run("CI", "success", workflow=1), run("Core Mac syntax", "skipped", workflow=2)]
        self.assertEqual(mt.not_green(runs, [{"context": mt.WORD, "state": "pending"}], "abc"), [])

    def test_an_unfinished_or_failed_run_is_not_green(self):
        runs = [run("CI", "success", workflow=1), run("Check scripts", None, status="in_progress", workflow=2),
                run("DCO", "failure", workflow=3)]
        bad = mt.not_green(runs, [{"context": "ci/other", "state": "failure"}], "abc")
        self.assertEqual(len(bad), 3, bad)

    def test_only_the_newest_run_of_a_workflow_counts(self):
        runs = [run("CI", "failure", created="2026-11-23T04:00:00Z"),
                run("CI", "success", created="2026-11-23T05:00:00Z")]
        self.assertEqual(mt.not_green(runs, [], "abc"), [])


class GatedList(unittest.TestCase):
    def test_agent_code_and_its_binaries_are_gated(self):
        for path in ["app/src/agents.rs", "app/src/agents/tools.rs", "app/src/agents_core.rs",
                     "app/src/agent_events.rs", "app/src/shell/agent_bar.rs",
                     "app/src/shell/agent_pointer.rs", "app/src/shell/agent_question.rs",
                     "app/src/shell/agent_card_keys.rs", "app/src/shell/library/agents_tests.rs",
                     "app/src/bridge.rs", "app/src/ilo_command.rs"]:
            self.assertTrue(gated(path), path)

    def test_a_host_header_is_gated_wherever_it_is(self):
        self.assertTrue(gated("app/platform/mac/host.h"))
        self.assertTrue(gated("core/host.h"))

    def test_other_app_code_is_not(self):
        for path in ["app/src/shell/naming.rs", "app/src/command_bar.rs", "docs/ux-principles.md"]:
            self.assertFalse(gated(path), path)


if __name__ == "__main__":
    unittest.main()
