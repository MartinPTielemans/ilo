"""The core's Mac and Windows compiles, checked in minutes on a hosted runner before an engine build
(#2668, #3105).

pack <src> <out.tar.zst>: run in the Mac engine build's tree once it's built. It packs what a
  compile of //browser_core reads besides the core and the macOS SDK. That's every header in the
  tree and the build's gen/, not only the ones the engine's core includes, so a branch's new include
  is there too. Then Chromium's libc++ and clang, and the core's compile variables from its .ninja
  files: about 200 MB, against the tree's 8 GB.
pack <src> <out.tar.zst> <out dir> <toolchain dir>: the same for the Windows engine's cross build,
  less clang, which a check takes as Chromium's Windows package at the build's revision. Nothing of
  Microsoft's goes in: no file from the toolchain dir, whose compiler and SDK Microsoft's licence
  keeps on the owner's machine (engine/README.md, Windows), nor the Microsoft sources
  ungoogled-chromium-windows downloads into the tree. The pack is public, and the build machine's
  paths in the core's build files are rewritten: the tree's as ../.., where a compile runs, and the
  toolchain's as a path that holds nothing. A packed file that still names the machine's home,
  account or engine directory stops it.
check <pack dir> <checkout> [<engine dir>]: runs clang -fsyntax-only on each core file the engine
  compiled, as the branch has it, with the build's own flags, against the runner's SDK: on a Mac,
  the Mac's, and on Windows, with the clang-cl of Chromium's Windows package put in the pack's
  llvm-build, the one Visual Studio's environment gives in INCLUDE. The engine dir is the engine/
  the packed engine was built from: its series and patches. Given it, the pack's headers are first
  brought to the branch's own patches, as engine/tree.sh brings a tree, so a core change that needs
  a new engine patch is checked with it (#2747).
"""
import concurrent.futures
import getpass
import glob
import io
import os
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import time

HEADER = re.compile(r"\.(h|hh|hpp|inc|def|ipp)$")
CLANG = "third_party/llvm-build/Release+Asserts"
OUT = "out/Release"
# Packed whole, whatever their names: libc++'s headers have none, and clang's are its own.
WHOLE = [
    "third_party/libc++/src/include",
    "third_party/libc++abi/src/include",
    "buildtools/third_party/libc++",
    CLANG + "/lib/clang",
]
# A file the core's flags name.
FLAG_FILES = ["build/config/warning_suppression.txt"]
# The build's outputs other than gen/, and LLVM's own sources. The core, a link to the checkout's,
# isn't walked.
SKIP = {"out", "third_party/llvm-build", "third_party/llvm", ".git"}
# Microsoft's sources in the Windows tree, which ungoogled-chromium-windows downloads (prepare.sh).
MICROSOFT = {
    "third_party/microsoft_dxheaders",
    "third_party/microsoft_webauthn",
    "third_party/microsoft_multimedia_redirection",
}
# What a Windows check reads from Chromium's tree besides headers: clang's revision, whose Windows
# package it compiles with, and the SDK the build was made with, which it names beside the runner's.
WINDOWS_FILES = ["tools/clang/scripts/update.py", "build/vs_toolchain.py"]
# The core's files when the engine was built, in the build's out dir, so a check can tell the ones
# a branch adds.
CORE_FILES = "core-files.txt"
# Where the Windows pack's build files say Microsoft's toolchain was. Nothing is there, and a
# check's compiler takes the runner's own from Visual Studio's environment.
NO_TOOLCHAIN = "/windows-toolchain"
# A compile's error, as clang gives it, file:line:col, or as clang-cl does, file(line,col).
ERROR = re.compile(r"^(\S+?)(?::(\d+):(\d+)|\((\d+),(\d+)\)): (?:fatal )?error: (.*)$", re.M)
# What an engine patch changes that the build turns into headers in gen/, which the pack holds only
# as the engine generated them.
GENERATES = re.compile(r"\.(mojom|proto|pdl|idl|json5|gni?)$|(^|/)BUILD\.gn$")


def files_under(src, top, keep=lambda name: True, skip=()):
    for d, dirs, files in os.walk(os.path.join(src, top)):
        rel = os.path.relpath(d, src)
        dirs[:] = [x for x in dirs if os.path.normpath(os.path.join(rel, x)) not in skip]
        yield from (os.path.normpath(os.path.join(rel, f)) for f in files if keep(f))


def pack(src, archive, out=OUT, toolchain=None):
    windows = toolchain is not None
    is_header = HEADER.search
    names = list(files_under(src, ".", is_header, (SKIP | MICROSOFT) if windows else SKIP))
    names += files_under(src, out + "/gen", is_header)
    for d in WHOLE:
        # Chromium's Windows package of clang brings its own headers.
        if not (windows and d.startswith(CLANG)):
            names += files_under(src, d)
    names += FLAG_FILES
    if windows:
        names += WINDOWS_FILES
    else:
        # clang++, the link to clang, and the link's binary, clang-<major>.
        clang_bin = os.path.join(CLANG, "bin")
        names += [
            os.path.join(clang_bin, f)
            for f in os.listdir(os.path.join(src, clang_bin))
            if re.fullmatch(r"clang(\+\+|-\d+)?", f)
        ]
    names += files_under(src, out + "/obj/browser_core", lambda f: f.endswith(".ninja"))
    names.append(out + "/args.gn")
    # Under src, as files_under takes it: src joined again found nothing when src was relative, as
    # the engine's build gives it, so every core file read as one a branch adds (#2782). An empty
    # list would do that again, so it stops the pack.
    core_files = [os.path.relpath(p, "browser_core") for p in files_under(src, "browser_core")]
    if not core_files:
        sys.exit("No core files in the tree's browser_core.")
    written = {os.path.join(out, CORE_FILES): "".join(p + "\n" for p in core_files).encode()}
    if windows:
        names = in_tree(src, toolchain, names)
        written.update(scrubbed(src, out, toolchain, names))
    names = sorted(set(names) - set(written))
    zstd = subprocess.Popen(["zstd", "-10", "-T0", "-q", "-o", archive], stdin=subprocess.PIPE)
    # A Windows checker's tar can't always make a link, and each one here leads to a file in the
    # tree (in_tree), so that file goes in in its place.
    with tarfile.open(fileobj=zstd.stdin, mode="w|", dereference=windows) as t:
        for name in names:
            t.add(os.path.join(src, name), arcname="src/" + name, recursive=False)
        for name, data in sorted(written.items()):
            info = tarfile.TarInfo("src/" + name)
            info.size, info.mtime, info.mode = len(data), int(time.time()), 0o644
            t.addfile(info, io.BytesIO(data))
    zstd.stdin.close()
    if zstd.wait():
        sys.exit("zstd failed")
    print(f"Packed {len(names) + len(written)} files in {os.path.getsize(archive) / 1e6:.0f} MB.")


def in_tree(src, toolchain, names):
    """|names| less those whose real path is outside the tree, such as in Microsoft's toolchain:
    those aren't Chromium's to pack. A compile that needs one fails the check, as missing."""
    root, kept = os.path.realpath(src), os.path.realpath(toolchain)
    inside = [
        name
        for name in names
        for real in [os.path.realpath(os.path.join(src, name))]
        if real.startswith(root + os.sep) and not real.startswith(kept + os.sep)
    ]
    if len(inside) < len(names):
        print(f"Left out {len(names) - len(inside)} files that lead out of the tree.")
    return inside


def scrubbed(src, out, toolchain, names):
    """The Windows pack's build files with the build machine's paths rewritten, by name. Stops the
    pack where one still names the machine after the rewrite. Its message names a packed file only,
    as the job's log is public."""
    root = os.path.realpath(src)
    kept = os.path.realpath(toolchain)
    # Longest first, so the tree's path goes before a path that holds it.
    rewrites = sorted(
        {
            os.path.abspath(toolchain): NO_TOOLCHAIN,
            kept: NO_TOOLCHAIN,
            os.path.abspath(src): "../..",
            root: "../..",
        }.items(),
        key=lambda kv: -len(kv[0]),
    )
    # A home of / would be in every file, and names nothing.
    machine = [
        m.encode()
        for m in [
            os.path.expanduser("~"),
            os.path.dirname(os.path.dirname(root)),
            "/" + getpass.getuser() + "/",
        ]
        if m.strip("/")
    ]
    # The core's build files, and any header the build generated that names the tree.
    rewritten = [n for n in names if n == os.path.join(out, "args.gn") or n.endswith(".ninja")]
    rewritten += [n for n in names if n.startswith(out + "/gen/")]
    written, named = {}, []
    for name in rewritten:
        data = open(os.path.join(src, name), "rb").read()
        new = data
        for path, to in rewrites:
            new = new.replace(path.encode(), to.encode())
        if new != data or not name.startswith(out + "/gen/"):
            written[name] = new
        if new != data:
            print(f"Rewrote the build machine's paths in {name}.")
        if any(m in new for m in machine):
            named.append(name)
    if named:
        sys.exit(
            f"{len(named)} packed files name the build machine's home, account or engine "
            f"directory, such as {named[0]}."
        )
    return written


def compiles(out_dir):
    """Each core source the engine compiled, as (source, rule, its target's variables, target)."""
    found = []
    core = os.path.join(out_dir, "obj/browser_core")
    for name in sorted(os.listdir(core)):
        # The tests' target is in the build files, and the engine doesn't build it.
        if not name.endswith(".ninja") or name.endswith("_unittests.ninja"):
            continue
        variables, builds = {}, []
        for line in open(os.path.join(core, name)):
            m = re.match(r"^(\w+) = (.*)$", line)
            if m:
                variables[m.group(1)] = re.sub(r"\$([$: ])", r"\1", m.group(2))
            m = re.match(r"^build \S+: (cxx|objcxx) \.\./\.\./browser_core/(\S+)", line)
            if m:
                builds.append((m.group(2), m.group(1)))
        target = name[: -len(".ninja")]
        found += [(source, rule, variables, target) for source, rule in builds]
    return found


def command(clang, rule, variables, source):
    keys = ["defines", "include_dirs", "framework_dirs", "cflags"]
    keys.append("cflags_cc" if rule == "cxx" else "cflags_objcc")
    # The PGO profile and the crash folder are for code generation, which a syntax check skips.
    args = [
        a
        for a in shlex.split(" ".join(variables.get(k, "") for k in keys))
        if not a.removeprefix("/clang:").startswith(("-fprofile-use", "-fcrash-diagnostics-dir"))
    ]
    # clang-cl takes cl's /Zs for -fsyntax-only.
    only = "/Zs" if os.path.basename(clang).startswith("clang-cl") else "-fsyntax-only"
    return [clang, *args, "-fno-color-diagnostics", only, "../../browser_core/" + source]


def xcrun(*args):
    return subprocess.run(
        ["xcrun", "--sdk", "macosx", *args], capture_output=True, text=True, check=True
    ).stdout.strip()


def series(engine, major, mac=True):
    """The Mac's series for a Chromium major, or another OS's, as tree.sh's series_for gives it."""
    names = []
    for line in open(os.path.join(engine, "series")):
        if line.startswith("#") or not line.split():
            continue
        name, when = (line.split() + [""])[:2]
        if name.startswith("mac/") and not mac:
            continue
        if when.startswith("<") and major >= int(when[1:]):
            continue
        if when.startswith(">=") and major < int(when[2:]):
            continue
        names.append(name)
    return names


def sections(text):
    """A patch's file sections, as (the file, the section's text)."""
    found, path, lines = [], None, text.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if line.startswith("--- ") and i + 1 < len(lines) and lines[i + 1].startswith("+++ "):
            old, new = line[4:].split()[0], lines[i + 1][4:].split()[0]
            path = re.sub(r"^[ab]/", "", new if new != "/dev/null" else old)
            found.append([path, ""])
        elif line.startswith("diff "):
            path = None
        if path:
            found[-1][1] += line
    return found


def engine_patches(src, checkout, built, mac=True):
    """Brings the pack's headers from the series the engine was built with to the branch's: the
    headers' part of each patch from the first that differs is taken out, last first, and the
    branch's from there are put in. Returns the summary's lines, and an error when one can't be."""
    pins = open(os.path.join(checkout, "engine/pins.ini")).read()
    major = int(re.search(r"^\s*version\s*=\s*(\d+)\.", pins, re.M).group(1))

    def headers(engine):
        out = []
        for name in series(engine, major, mac):
            parts = sections(open(os.path.join(engine, "patches", name)).read())
            text = "".join(t for path, t in parts if HEADER.search(path))
            generates = [(path, t) for path, t in parts if GENERATES.search(path)]
            out.append((name, text, generates))
        return out

    had, wants = headers(built), headers(os.path.join(checkout, "engine"))
    # What the build generates from, where it differs from the engine's: the pack can't follow it.
    generated = {name: g for name, _, g in had}
    unpacked = [
        f"{name} ({', '.join(path for path, _ in g)})"
        for name, _, g in wants
        if g and generated.get(name) != g
    ]
    had = [(name, text) for name, text, _ in had if text]
    wants = [(name, text) for name, text, _ in wants if text]
    first = 0
    while first < min(len(had), len(wants)) and had[first] == wants[first]:
        first += 1

    def patch(text, *args):
        return subprocess.run(
            ["patch", "-d", src, "-p1", "-F0", "-s", *args],
            input=text, capture_output=True, text=True,
        ).returncode == 0

    lines = []
    for name, text in reversed(had[first:]):
        if not (patch(text, "-R", "--dry-run") and patch(text, "-R")):
            return lines, (
                f"The packed engine's headers don't hold {name} as the commit it was built from "
                "has it, so this can't bring them to the branch's patches."
            )
    for name, text in wants[first:]:
        if not (patch(text, "-N", "--dry-run") and patch(text, "-N")):
            files = ", ".join(path for path, _ in sections(text))
            return lines, (
                f"This branch's core needs an engine with patch {name}: its change to {files} "
                "doesn't apply to the packed engine's headers, so the core can't be checked here "
                f"until a {'Mac' if mac else 'Windows'} engine with it is built."
            )
    # The patches that differ, without those taken out only to go back in after them.
    out = [name for name, text in had[first:] if (name, text) not in wants]
    put = [name for name, text in wants[first:] if (name, text) not in had]
    if out:
        lines.append(
            "Taken out of the packed engine's headers, as this branch doesn't have them so: "
            + ", ".join(out) + "."
        )
    if put:
        lines.append(
            "Put into the packed engine's headers, as this branch has them: " + ", ".join(put) + "."
        )
    if unpacked:
        lines.append(
            "This branch changes what the engine generates headers from, which the pack holds only "
            "as the engine built them, so an error from those needs an engine with the patch: "
            + ", ".join(unpacked) + "."
        )
    return lines, None


def check(pack_dir, checkout, built=None):
    windows = sys.platform == "win32"
    os_name = "Windows" if windows else "the Mac"
    src = os.path.join(pack_dir, "src")
    engine_lines, error = (
        engine_patches(src, checkout, built, not windows) if built else ([], None)
    )
    if error:
        print(f"::error::{error}")
        summary("\n\n".join([*engine_lines, error]))
        return 1
    # The Mac's build is out/Release, and the Windows engine's out/Default.
    (out_dir,) = [
        os.path.dirname(os.path.dirname(d))
        for d in glob.glob(os.path.join(src, "out", "*", "obj", "browser_core"))
    ]
    core = os.path.join(os.path.abspath(checkout), "core")
    link = os.path.join(src, "browser_core")
    if not os.path.lexists(link):
        try:
            os.symlink(core, link, target_is_directory=True)
        except OSError:
            # Windows lets only some accounts make links; a copy is the core as the branch has it.
            shutil.copytree(core, link)
    if windows:
        sdk_lines = windows_sdk(src)
    else:
        sdk_lines = []
        # The runner's SDK, where the build had its own.
        ninja = open(os.path.join(out_dir, "obj/browser_core/browser_core.ninja")).read()
        sysroot = re.search(r"-isysroot (\S+)", ninja).group(1)
        built_with = re.search(r"MacOSX([\d.]+)\.sdk", sysroot).group(1)
        have = xcrun("--show-sdk-version")
        if have != built_with:
            print(
                f"::warning::The engine was built with macOS SDK {built_with}, and this has {have}."
            )
        os.makedirs(os.path.dirname(os.path.join(out_dir, sysroot)), exist_ok=True)
        if not os.path.lexists(os.path.join(out_dir, sysroot)):
            os.symlink(xcrun("--show-sdk-path"), os.path.join(out_dir, sysroot))

    built = compiles(out_dir)
    jobs = [job for job in built if os.path.exists(os.path.join(core, job[0]))]
    gone = [job[0] for job in built if not os.path.exists(os.path.join(core, job[0]))]
    # A file the branch adds is checked with the core's flags if it's this OS's or portable: on the
    # Mac a .mm, or a .cc outside platform/, where the core keeps each OS's own; on Windows a .cc
    # in platform/win/ or outside platform/. One in another OS's platform/ is that OS's to compile.
    before = set(open(os.path.join(out_dir, CORE_FILES)).read().splitlines())
    added = [
        f.replace(os.sep, "/")
        for f in sorted(files_under(core, ".", lambda f: f.endswith((".mm", ".cc"))))
        if f.replace(os.sep, "/") not in before and not f.endswith("_unittest.cc")
    ]
    if windows:
        checked = [
            f
            for f in added
            if f.endswith(".cc")
            and (f.startswith("platform/win/") or not f.startswith("platform/"))
        ]
    else:
        checked = [f for f in added if f.endswith(".mm") or not f.startswith("platform/")]
    core_flags = next(v for _, _, v, target in built if target == "browser_core")
    jobs += [(f, "objcxx" if f.endswith(".mm") else "cxx", core_flags, "") for f in checked]

    clang = os.path.abspath(
        os.path.join(src, CLANG, "bin/clang-cl.exe" if windows else "bin/clang++")
    )

    def run(job):
        source, rule, variables, _ = job
        r = subprocess.run(
            command(clang, rule, variables, source), cwd=out_dir, capture_output=True, text=True
        )
        return source, r.returncode, r.stderr

    failed = []
    with concurrent.futures.ThreadPoolExecutor(os.cpu_count()) as pool:
        for source, code, err in pool.map(run, jobs):
            if err:
                print(f"::group::core/{source}{': failed' if code else ''}")
                print(err, end="")
                print("::endgroup::")
            if code:
                failed.append("core/" + source)
            for m in ERROR.finditer(err):
                path = re.sub(r"^\.\./\.\./browser_core/", "core/", m.group(1).replace("\\", "/"))
                line, col = (m.group(2), m.group(3)) if m.group(2) else (m.group(4), m.group(5))
                print(f"::error file={path},line={line},col={col}::{m.group(6)}")

    lines = [
        *engine_lines,
        *sdk_lines,
        f"Checked {len(jobs)} core files for {os_name}; {len(failed)} failed.",
    ]
    if failed:
        lines.append("Failed: " + ", ".join(failed) + ".")
    if checked:
        lines.append("Added by this branch, checked with the core's flags: " + ", ".join(checked))
    skipped = [f for f in added if f not in checked]
    if skipped:
        lines.append("Added for another OS, not checked: " + ", ".join(skipped))
    if gone:
        lines.append("In the engine's build, not in this branch: " + ", ".join(gone) + ".")
    summary("\n\n".join(lines))
    return 1 if failed else 0


def windows_sdk(src):
    """The summary's line on the SDK: the runner's, which Visual Studio's environment names, and the
    one the engine was built with, which may differ (#3105)."""
    tools = open(os.path.join(src, "build/vs_toolchain.py")).read()
    built_with = re.search(r"^SDK_VERSION = '(.*)'$", tools, re.M).group(1)
    have = os.environ.get("WindowsSDKVersion", "").strip("\\/ ")
    if not os.environ.get("INCLUDE"):
        sys.exit("No INCLUDE: this runs in Visual Studio's environment, for the SDK's headers.")
    if have == built_with:
        return []
    return [
        f"Checked against the runner's Windows SDK {have or '(unnamed)'}; the engine is built with "
        f"{built_with}, so an error only one of them gives is missed here, or reported where the "
        "engine builds."
    ]


def summary(text):
    print(text)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as s:
            s.write(text + "\n")


if __name__ == "__main__":
    if len(sys.argv) in (4, 6) and sys.argv[1] == "pack":
        pack(*sys.argv[2:])
    elif len(sys.argv) in (4, 5) and sys.argv[1] == "check":
        sys.exit(check(*sys.argv[2:]))
    else:
        sys.exit(__doc__)
