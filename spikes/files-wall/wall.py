"""Does an OS sandbox around the assistant's process hold the levels' table on disk (#4101)?

For each level it makes a throwaway tree (a granted folder, attached copies, a place outside, and a
home of stand-ins for the assistant's own folder and the never-grantable ones), derives the
sandbox's rules from the one table, starts probe.py inside them and compares every answer with
what the table says. Then it checks the folder picker's never-grantable rule on the stand-in home.

Linux uses Landlock, the Mac Seatbelt (sandbox-exec). Usage: python3 -I wall.py [WORK]. WORK
(default: a new folder in the system's temp) holds only what it makes. Exit 1 when the sandbox let
through anything the table refuses.
"""

import ctypes
import errno
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.realpath(__file__))
LINUX = sys.platform.startswith("linux")

# The table: what each level lets the assistant do in each place. Nothing else is reachable.
LEVELS = ("look", "work", "full")
OPS = ("read", "search", "make", "change", "delete", "move", "replace")
TABLE = {
    ("look", "attached"): {"read", "search"},
    ("work", "attached"): {"read", "search"},
    ("full", "attached"): {"read", "search"},
    ("look", "folder"): {"read", "search"},
    ("work", "folder"): {"read", "search", "make", "change"},
    ("full", "folder"): {"read", "search", "make", "change", "delete", "move", "replace"},
}


def allows(level, place, op):
    return op in TABLE.get((level, place), set())


# Landlock's filesystem rights (uapi/linux/landlock.h), and the ABI each came in.
EXECUTE, WRITE_FILE, READ_FILE, READ_DIR = 1 << 0, 1 << 1, 1 << 2, 1 << 3
REMOVE_DIR, REMOVE_FILE, MAKE_CHAR, MAKE_DIR = 1 << 4, 1 << 5, 1 << 6, 1 << 7
MAKE_REG, MAKE_SOCK, MAKE_FIFO, MAKE_BLOCK = 1 << 8, 1 << 9, 1 << 10, 1 << 11
MAKE_SYM, REFER, TRUNCATE, IOCTL_DEV = 1 << 12, 1 << 13, 1 << 14, 1 << 15
ABI_RIGHTS = {1: (1 << 13) - 1, 2: (1 << 14) - 1, 3: (1 << 15) - 1, 4: (1 << 15) - 1}
FILE_RIGHTS = EXECUTE | WRITE_FILE | READ_FILE | TRUNCATE | IOCTL_DEV

# Each operation as the rights it needs. Make can't be had without writing the file it makes,
# and moving can't be had without removing the name it moves from: the table keeps those together.
LANDLOCK = {
    "read": READ_FILE,
    "search": READ_FILE | READ_DIR,
    "make": MAKE_REG | MAKE_DIR | WRITE_FILE,
    "change": WRITE_FILE | TRUNCATE,
    "delete": REMOVE_FILE | REMOVE_DIR,
    "move": REFER | REMOVE_FILE | REMOVE_DIR | MAKE_REG | MAKE_DIR,
    "replace": REFER | REMOVE_FILE | MAKE_REG,
}

SEATBELT = {
    "read": ["file-read-data"],
    "search": ["file-read-data"],
    "make": ["file-write-create", "file-write-data"],
    "change": ["file-write-data"],
    "delete": ["file-write-unlink"],
    "move": ["file-write-create", "file-write-unlink"],
    "replace": ["file-write-create", "file-write-unlink"],
}


def landlock_rights(level, place):
    rights = 0
    for op in OPS:
        if allows(level, place, op):
            rights |= LANDLOCK[op]
    return rights


def system_reads():
    """What any process needs to start: the system's programs and libraries, and the probe."""
    paths = ["/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc", sys.prefix, HERE]
    paths.append(os.path.dirname(os.path.realpath(sys.executable)))
    return sorted({os.path.realpath(p) for p in paths if os.path.exists(p)})


def rules(level, tree):
    """The sandbox's rules for |level| in |tree|, from the table and the fixed places."""
    t = lambda *p: os.path.join(tree, *p)
    out = [(p, "system") for p in system_reads()]
    out += [
        (t("folder"), ("table", "folder")),
        (t("attached"), ("table", "attached")),
        # The assistant's own folder: its sign-in is read, only its sessions are written.
        (t("home", ".claude"), "own"),
        (t("home", ".claude", "sessions"), "own-sessions"),
        (t("tmp"), "temp"),
        (t("run"), "own"),
    ]
    return out


def landlock_ruleset(level, tree, abi):
    handled = ABI_RIGHTS[min(abi, 4)]
    fixed = {
        "system": READ_FILE | READ_DIR | EXECUTE,
        "own": READ_FILE | READ_DIR,
        "own-sessions": READ_FILE | READ_DIR | MAKE_REG | WRITE_FILE | TRUNCATE,
        "temp": handled & ~(MAKE_CHAR | MAKE_BLOCK | MAKE_SOCK | MAKE_FIFO | MAKE_SYM),
    }
    out = []
    for path, kind in rules(level, tree):
        rights = landlock_rights(level, kind[1]) if isinstance(kind, tuple) else fixed[kind]
        out.append((path, rights & handled))
    # The probe asks the system for its own open files, and Python for randomness.
    out += [("/proc", READ_FILE | READ_DIR), ("/dev/null", READ_FILE | WRITE_FILE),
            ("/dev/urandom", READ_FILE)]
    return handled, out


def enter_landlock(handled, ruleset):
    libc = ctypes.CDLL(None, use_errno=True)
    attr = ctypes.create_string_buffer(struct.pack("=Q", handled))
    fd = libc.syscall(ctypes.c_long(444), attr, ctypes.c_size_t(8), ctypes.c_uint32(0))
    if fd < 0:
        raise OSError(ctypes.get_errno(), "landlock_create_ruleset")
    for path, rights in ruleset:
        if not os.path.exists(path):
            continue
        at = os.open(path, os.O_PATH | os.O_CLOEXEC)
        if not os.path.isdir(path):
            rights &= FILE_RIGHTS
        beneath = ctypes.create_string_buffer(struct.pack("=Qi", rights, at))
        if libc.syscall(ctypes.c_long(445), ctypes.c_int(fd), ctypes.c_int(1), beneath,
                        ctypes.c_uint32(0)) < 0:
            raise OSError(ctypes.get_errno(), "landlock_add_rule " + path)
        os.close(at)
    if libc.prctl(38, 1, 0, 0, 0) < 0:  # PR_SET_NO_NEW_PRIVS
        raise OSError(ctypes.get_errno(), "no_new_privs")
    if libc.syscall(ctypes.c_long(446), ctypes.c_int(fd), ctypes.c_uint32(0)) < 0:
        raise OSError(ctypes.get_errno(), "landlock_restrict_self")
    os.close(fd)


def seatbelt_profile(level, tree):
    quote = lambda p: '"%s"' % p.replace("\\", "\\\\").replace('"', '\\"')
    lines = ["(version 1)", "(deny default)", '(import "system.sb")',
             "(allow process-exec process-fork)", "(allow file-read-metadata)",
             '(allow file-read* file-write-data (literal "/dev/null"))']
    for path, kind in rules(level, tree):
        if isinstance(kind, tuple):
            ops = sorted({a for op in OPS if allows(level, kind[1], op) for a in SEATBELT[op]})
        else:
            ops = {
                "system": ["file-read*", "process-exec"],
                "own": ["file-read*"],
                "own-sessions": ["file-read*", "file-write-create", "file-write-data"],
                "temp": ["file-read*", "file-write*"],
            }[kind]
        if ops:
            lines.append("(allow %s (subpath %s))" % (" ".join(ops), quote(path)))
    return "\n".join(lines)


class NoWall(Exception):
    """The wall can't be put up here, so the assistant doesn't start with a folder."""


def landlock_abi():
    return ctypes.CDLL(None, use_errno=True).syscall(ctypes.c_long(444), None, ctypes.c_size_t(0),
                                                     ctypes.c_uint32(1))


def walled(level, tree, command, abi=None, broken=False):
    """|command| behind the wall for |level|. |abi| stands in for the system's answer and |broken|
    for rules the system rejects, so the spike can show both stop the start."""
    if LINUX:
        abi = landlock_abi() if abi is None else abi
        if abi < 1:
            raise NoWall("this system has no Landlock")
        handled, ruleset = landlock_ruleset(level, tree, abi)
        if broken:
            # A right Landlock doesn't know: adding the rule fails.
            ruleset = [(os.path.join(tree, "folder"), 1 << 40)] + ruleset
        config = os.path.join(tree, "run", "wall.json")
        with open(config, "w") as f:
            json.dump({"handled": handled, "rules": ruleset}, f)
        return [sys.executable, "-I", os.path.realpath(__file__), "--exec", config, "--"] + command
    if not os.path.exists("/usr/bin/sandbox-exec"):
        raise NoWall("this system has no sandbox-exec")
    profile = seatbelt_profile(level, tree) + ("\n(allow no-such-operation)" if broken else "")
    return ["/usr/bin/sandbox-exec", "-p", profile] + command


def launch(level, tree, probe_args):
    """Starts the probe inside the wall: the launcher's one job is to apply it and exec."""
    command = walled(level, tree, [sys.executable, "-I", os.path.join(HERE, "probe.py")]
                     + probe_args)
    run = subprocess.run(command, cwd=os.path.join(tree, "folder"), capture_output=True,
                         text=True, close_fds=True)
    if run.returncode:
        raise SystemExit("the probe didn't run at %s:\n%s" % (level, run.stderr))
    return json.loads(run.stdout)


def fail_closed(work):
    """Where the wall can't be put up, nothing starts: the probe would leave a mark outside."""
    rows = []
    for name, abi, broken in (("the system has no Landlock or sandbox", 0, False),
                              ("the system rejects the wall's rules", None, True)):
        tree = os.path.realpath(os.path.join(work, "fail-closed"))
        if os.path.lexists(tree):
            shutil.rmtree(tree)
        make_tree(tree)
        mark = os.path.join(tree, "outside", "ran.txt")
        with open(os.path.join(tree, "run", "attempts.json"), "w") as f:
            json.dump([{"does": "make", "at": mark}], f)
        probe = [sys.executable, "-I", os.path.join(HERE, "probe.py"),
                 os.path.join(tree, "run", "attempts.json")]
        try:
            command = walled("full", tree, probe, abi=abi if LINUX else None, broken=broken)
            if not LINUX and abi == 0:
                raise NoWall("stood in for")
            started = subprocess.run(command, cwd=os.path.join(tree, "folder"),
                                     capture_output=True, text=True).returncode == 0
        except NoWall:
            started = False
        rows.append((name, started, os.path.exists(mark)))
    return rows


def exec_inside(argv):
    config, command = argv[0], argv[2:]
    with open(config) as f:
        wall = json.load(f)
    enter_landlock(wall["handled"], wall["rules"])
    # Nothing the launcher had open goes on: an inherited handle isn't checked again.
    os.closerange(3, 1 << 16)
    os.execv(command[0], command)


def write(path, text="x"):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


def make_tree(tree):
    t = lambda *p: os.path.join(tree, *p)
    for name in ("f.txt", "chg.txt", "app.txt", "trunc.txt", "ren.txt", "victim.txt", "mv.txt",
                 "mv-out.txt", "rep-a.txt", "rep-b.txt", "sub/g.txt"):
        write(t("folder", name))
    os.makedirs(t("folder", "sub2"))
    for name in ("a.txt", "chg.txt", "victim.txt"):
        write(t("attached", name))
    for name in ("o.txt", "o2.txt", "in.txt", "h.txt"):
        write(t("outside", name))
    write(t("home", ".claude", "creds.json"), "{}")
    write(t("home", ".claude", "sessions", "s.json"), "{}")
    write(t("home", ".ssh", "id"))
    os.makedirs(t("tmp"))
    os.makedirs(t("run"))
    os.symlink(t("outside", "o.txt"), t("folder", "link-out-file"))
    os.symlink(t("outside"), t("folder", "link-out-dir"))
    # A hard link the user made before granting the folder: the same file as one outside.
    os.link(t("outside", "h.txt"), t("folder", "hard-out"))


def attempts(tree):
    """Each attempt: what it does, where, and the place and operation the table judges it by."""
    t = lambda *p: os.path.join(tree, *p)
    o = t("outside", "o.txt")
    out = [
        ("read a file by its full path", "read", t("folder", "f.txt"), "folder", "read"),
        ("read a file by its own name", "read", "f.txt", "folder", "read"),
        ("read through .. back into the folder", "read", "sub/../f.txt", "folder", "read"),
        ("list the folder", "search", ".", "folder", "search"),
        ("list a folder in it", "search", "sub", "folder", "search"),
        ("make a file", "make", "new.txt", "folder", "make"),
        ("make a folder", "make_dir", "newdir", "folder", "make"),
        ("make a hard link to a file beside it", "hard_link", ["f.txt", "hl.txt"], "folder",
         "make"),
        # Across folders, a link needs the right a move needs.
        ("make a hard link to a file in another folder in it", "hard_link",
         ["f.txt", "sub/hl.txt"], "folder", "move"),
        ("change a file", "change", "chg.txt", "folder", "change"),
        ("add to a file", "append", "app.txt", "folder", "change"),
        ("empty a file", "truncate", "trunc.txt", "folder", "change"),
        ("change a file by saving a copy over it", "change_by_rename", "ren.txt", "folder",
         "change"),
        ("delete a file", "delete", "victim.txt", "folder", "delete"),
        ("delete an empty folder", "delete_dir", "sub2", "folder", "delete"),
        ("move a file into a folder in it", "move", ["mv.txt", "sub/mv.txt"], "folder", "move"),
        ("move one file over another", "move", ["rep-a.txt", "rep-b.txt"], "folder", "replace"),
        ("read an attached copy", "read", t("attached", "a.txt"), "attached", "read"),
        ("list the attached copies", "search", t("attached"), "attached", "search"),
        ("make a file beside the attached copies", "make", t("attached", "new.txt"), "attached",
         "make"),
        ("change an attached copy", "change", t("attached", "chg.txt"), "attached", "change"),
        ("delete an attached copy", "delete", t("attached", "victim.txt"), "attached", "delete"),
        ("read outside by its full path", "read", o, "outside", "read"),
        ("read outside through ..", "read", "../outside/o.txt", "outside", "read"),
        ("read through a link in the folder to a file outside", "read", "link-out-file",
         "outside", "read"),
        ("read through a link in the folder to a folder outside", "read", "link-out-dir/o.txt",
         "outside", "read"),
        ("read beneath a handle to the folder outside", "read_beneath_handle",
         [t("outside"), "o.txt"], "outside", "read"),
        ("list outside", "search", t("outside"), "outside", "search"),
        ("list outside through a link in the folder", "search", "link-out-dir", "outside",
         "search"),
        ("make a file outside", "make", t("outside", "new.txt"), "outside", "make"),
        ("make a file outside through a link in the folder", "make", "link-out-dir/new2.txt",
         "outside", "make"),
        ("change a file outside", "change", o, "outside", "change"),
        ("change outside through a link in the folder", "change", "link-out-file", "outside",
         "change"),
        ("delete a file outside", "delete", t("outside", "o2.txt"), "outside", "delete"),
        ("move a file out of the folder", "move", ["mv-out.txt", t("outside", "mv-out.txt")],
         "outside", "move"),
        ("move a file from outside into the folder", "move", [t("outside", "in.txt"), "in.txt"],
         "outside", "move"),
        ("hard-link a file outside into the folder", "hard_link", [o, "hl-in.txt"], "outside",
         "make"),
        ("hard-link a file in the folder to outside", "hard_link",
         ["f.txt", t("outside", "hl-out.txt")], "outside", "make"),
        ("move the granted folder itself", "move", [t("folder"), t("folder-moved")], "outside",
         "move"),
        ("make a link in the folder to a file outside", "sym_link", [o, "made-link"], "never",
         "make"),
        ("read the assistant's sign-in", "read", t("home", ".claude", "creds.json"), "own-read",
         "read"),
        ("change the assistant's sign-in", "change", t("home", ".claude", "creds.json"),
         "own-other", "change"),
        ("make a file in the assistant's sessions", "make",
         t("home", ".claude", "sessions", "new.json"), "own-sessions", "make"),
        ("read a never-grantable folder's file", "read", t("home", ".ssh", "id"), "outside",
         "read"),
        ("list the home folder", "search", t("home"), "outside", "search"),
        ("make a file in its own temp", "make", t("tmp", "x"), "temp", "make"),
        ("read through the hard link the user made", "read", "hard-out", "residual-hardlink",
         "read"),
        ("hold nothing the launcher had open", "handles", None, "launcher", "read"),
    ]
    if LINUX:
        out += [
            ("read outside through its own root in /proc", "read", "/proc/self/root" + o,
             "outside", "read"),
            ("read outside through the parent's root in /proc", "read",
             "/proc/%d/root%s" % (os.getpid(), o), "outside", "read"),
        ]
    return out


def expected(level, place, op):
    if place in ("folder", "attached"):
        return allows(level, place, op)
    return place in ("own-read", "own-sessions", "temp", "residual-hardlink", "launcher")


def run_level(level, work):
    tree = os.path.realpath(os.path.join(work, level))
    if os.path.lexists(tree):
        shutil.rmtree(tree)
    make_tree(tree)
    tried = attempts(tree)
    with open(os.path.join(tree, "run", "attempts.json"), "w") as f:
        json.dump([{"does": a[1], "at": a[2]} for a in tried], f)
    said = launch(level, tree, [os.path.join(tree, "run", "attempts.json")])
    rows = []
    for (name, _, _, place, op), got in zip(tried, said):
        want = expected(level, place, op)
        done = got == "done"
        if place == "residual-hardlink":
            verdict = "residual" if done else "as the table"
        elif done == want:
            verdict = "as the table"
        elif done:
            verdict = "LET THROUGH"
        else:
            verdict = "REFUSED"
        rows.append((name, place, op, "yes" if want else "no", got, verdict))
    return rows


# The picker's rule: never these, nothing inside them, no folder holding one, no link to one.
def never_grantable(home, extra=()):
    h = lambda *p: os.path.join(home, *p)
    names = [h(".ssh"), h(".aws"), h(".gnupg"), h(".claude"), h(".claude.json"), h(".codex"),
             h("Library"), h(".config", "google-chrome"), h(".config", "chromium"),
             h(".config", "BraveSoftware"), h(".config", "ilo"), h(".local", "share", "keyrings"),
             h(".mozilla")]
    return [os.path.realpath(p) for p in names + list(extra)]


def cloud_roots(home):
    """A cloud drive's root is never granted, but a folder in one may be."""
    names = ("Dropbox", "Google Drive", "OneDrive", "iCloud Drive")
    return [os.path.realpath(os.path.join(home, n)) for n in names]


def refusal(picked, home, extra=(), walk_limit=20000):
    """Why |picked| can't be granted, or None. Run at the pick and again at each start."""
    real = os.path.realpath(picked)
    home = os.path.realpath(home)
    if real == "/" or real == home:
        return "the whole computer or the home folder"
    inside = lambda child, parent: child == parent or child.startswith(parent.rstrip("/") + "/")
    for never in never_grantable(home, extra):
        if inside(real, never):
            return "inside a folder ilo never grants"
        if inside(never, real):
            return "holds a folder ilo never grants"
    for root in cloud_roots(home):
        if inside(root, real):
            return "a cloud drive's root"
    seen = 0
    for top, dirs, files in os.walk(real, followlinks=False):
        for name in dirs + files:
            seen += 1
            path = os.path.join(top, name)
            if os.path.islink(path):
                target = os.path.realpath(path)
                if target in ("/", home) or any(
                        inside(target, n) or inside(n, target)
                        for n in never_grantable(home, extra)):
                    return "holds a link to a folder ilo never grants"
        if seen > walk_limit:
            return "too many files to check"
    return None


def pick_cases(work):
    home = os.path.realpath(os.path.join(work, "pick-home"))
    if os.path.lexists(home):
        shutil.rmtree(home)
    h = lambda *p: os.path.join(home, *p)
    write(h(".ssh", "keys", "id"))
    write(h(".claude", "creds.json"))
    write(h(".config", "google-chrome", "Default", "Cookies"))
    write(h("Library", "Application Support", "ilo", "Default", "x"))
    write(h("Library", "CloudStorage", "Drive", "doc"))
    write(h("Dropbox", "notes", "n.txt"))
    write(h("projects", "site", "index.html"))
    write(h("holds-link", "readme"))
    os.symlink(h(".ssh"), h("holds-link", "keys"))
    os.symlink(h(".claude"), h("is-link"))
    write(h("later", "readme"))
    bundle = os.path.join(work, "Applications", "ilo.app")
    write(os.path.join(bundle, "Contents", "Info.plist"))
    cases = [
        ("the home folder", home, False),
        ("the whole computer", "/", False),
        (".ssh", h(".ssh"), False),
        ("a folder inside .ssh", h(".ssh", "keys"), False),
        ("the assistant's own folder", h(".claude"), False),
        ("a link to the assistant's own folder", h("is-link"), False),
        ("a folder holding a link to .ssh", h("holds-link"), False),
        (".config, which holds a browser's profile", h(".config"), False),
        ("Library as a whole", h("Library"), False),
        ("ilo's own profile", h("Library", "Application Support", "ilo"), False),
        ("a cloud drive's folder in Library", h("Library", "CloudStorage", "Drive"), False),
        ("a cloud drive's root", h("Dropbox"), False),
        ("the app bundle", bundle, False),
        ("a project folder", h("projects", "site"), True),
        ("a folder in a cloud drive", h("Dropbox", "notes"), True),
    ]
    rows = [(name, refusal(path, home, [bundle]) is None, want) for name, path, want in cases]
    # Granted at the pick, then a link to a never-grantable folder appears before a start.
    picked = refusal(h("later"), home, [bundle]) is None
    os.symlink(h(".ssh"), h("later", "keys"))
    rows.append(("granted, then holding a link to .ssh at the next start",
                 picked and refusal(h("later"), home, [bundle]) is None, False))
    return rows


def main():
    if sys.argv[1:2] == ["--exec"]:
        exec_inside(sys.argv[2:])
        return
    work = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="files-wall-")
    os.makedirs(work, exist_ok=True)
    print("Work folder: %s" % work)
    bad = 0
    for level in LEVELS:
        print("\n## %s\n" % level)
        print("| Attempt | Place | Op | Table | Answer | Verdict |")
        print("|---|---|---|---|---|---|")
        for row in run_level(level, work):
            print("| %s |" % " | ".join(row))
            bad += row[5] == "LET THROUGH"
    print("\n## Picking a folder\n")
    print("| Pick | Granted | Should be |")
    print("|---|---|---|")
    for name, granted, want in pick_cases(work):
        print("| %s | %s | %s |" % (name, "yes" if granted else "no", "yes" if want else "no"))
        bad += granted and not want
    print("\n## Where the wall can't be put up\n")
    print("| Case | Started | Left a mark outside |")
    print("|---|---|---|")
    for name, started, marked in fail_closed(work):
        print("| %s | %s | %s |" % (name, "yes" if started else "no", "yes" if marked else "no"))
        bad += started or marked
    print("\n%s" % ("Nothing got through." if not bad else "%d got through." % bad))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
