"""The core's Mac compile, checked in minutes on a hosted Mac before a Mac engine build (#2668).

pack <src> <out.tar.zst>: run in the Mac engine build's tree once it's built. It packs what a
  compile of //browser_core reads besides the core and the macOS SDK. That's every header in the
  tree and the build's gen/, not only the ones the engine's core includes, so a branch's new include
  is there too. Then Chromium's libc++ and clang, and the core's compile variables from its .ninja
  files: about 200 MB, against the tree's 8 GB.
check <pack dir> <checkout>: runs clang -fsyntax-only on each core file the engine compiled, as the
  branch has it, with the build's own flags, against the runner's SDK.
"""
import concurrent.futures
import os
import re
import shlex
import subprocess
import sys
import tarfile

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
# The core's files when the engine was built, so a check can tell the ones a branch adds.
CORE_FILES = OUT + "/core-files.txt"


def files_under(src, top, keep=lambda name: True, skip=()):
    for d, dirs, files in os.walk(os.path.join(src, top)):
        rel = os.path.relpath(d, src)
        dirs[:] = [x for x in dirs if os.path.normpath(os.path.join(rel, x)) not in skip]
        yield from (os.path.normpath(os.path.join(rel, f)) for f in files if keep(f))


def pack(src, archive):
    is_header = HEADER.search
    names = list(files_under(src, ".", is_header, SKIP))
    names += files_under(src, OUT + "/gen", is_header)
    for d in WHOLE:
        names += files_under(src, d)
    names += FLAG_FILES
    # clang++, the link to clang, and the link's binary, clang-<major>.
    clang_bin = os.path.join(CLANG, "bin")
    names += [
        os.path.join(clang_bin, f)
        for f in os.listdir(os.path.join(src, clang_bin))
        if re.fullmatch(r"clang(\+\+|-\d+)?", f)
    ]
    names += files_under(src, OUT + "/obj/browser_core", lambda f: f.endswith(".ninja"))
    names.append(OUT + "/args.gn")
    core = os.path.join(src, "browser_core")
    with open(os.path.join(src, CORE_FILES), "w") as f:
        f.writelines(os.path.relpath(p, "browser_core") + "\n" for p in files_under(src, core))
    names.append(CORE_FILES)
    zstd = subprocess.Popen(["zstd", "-10", "-T0", "-q", "-o", archive], stdin=subprocess.PIPE)
    with tarfile.open(fileobj=zstd.stdin, mode="w|") as t:
        for name in sorted(set(names)):
            t.add(os.path.join(src, name), arcname="src/" + name, recursive=False)
    zstd.stdin.close()
    if zstd.wait():
        sys.exit("zstd failed")
    print(f"Packed {len(set(names))} files in {os.path.getsize(archive) / 1e6:.0f} MB.")


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
        if not a.startswith(("-fprofile-use", "-fcrash-diagnostics-dir"))
    ]
    return [clang, *args, "-fno-color-diagnostics", "-fsyntax-only", "../../browser_core/" + source]


def xcrun(*args):
    return subprocess.run(
        ["xcrun", "--sdk", "macosx", *args], capture_output=True, text=True, check=True
    ).stdout.strip()


def check(pack_dir, checkout):
    src = os.path.join(pack_dir, "src")
    out_dir = os.path.join(src, OUT)
    core = os.path.join(os.path.abspath(checkout), "core")
    if not os.path.lexists(os.path.join(src, "browser_core")):
        os.symlink(core, os.path.join(src, "browser_core"))
    # The runner's SDK, where the build had its own.
    ninja = open(os.path.join(out_dir, "obj/browser_core/browser_core.ninja")).read()
    sysroot = re.search(r"-isysroot (\S+)", ninja).group(1)
    built_with = re.search(r"MacOSX([\d.]+)\.sdk", sysroot).group(1)
    have = xcrun("--show-sdk-version")
    if have != built_with:
        print(f"::warning::The engine was built with macOS SDK {built_with}, and this has {have}.")
    os.makedirs(os.path.dirname(os.path.join(out_dir, sysroot)), exist_ok=True)
    if not os.path.lexists(os.path.join(out_dir, sysroot)):
        os.symlink(xcrun("--show-sdk-path"), os.path.join(out_dir, sysroot))

    built = compiles(out_dir)
    jobs = [job for job in built if os.path.exists(os.path.join(core, job[0]))]
    gone = [job[0] for job in built if not os.path.exists(os.path.join(core, job[0]))]
    # A file the branch adds is checked with the core's flags if it's the Mac's or portable: a .mm,
    # or a .cc outside platform/, where the core keeps each OS's own. One in platform/ may be
    # another OS's, which Core on Linux and Windows compile.
    before = set(open(os.path.join(src, CORE_FILES)).read().splitlines())
    added = [
        f
        for f in sorted(files_under(core, ".", lambda f: f.endswith((".mm", ".cc"))))
        if f not in before and not f.endswith("_unittest.cc")
    ]
    checked = [f for f in added if f.endswith(".mm") or not f.startswith("platform/")]
    core_flags = next(v for _, _, v, target in built if target == "browser_core")
    jobs += [(f, "objcxx" if f.endswith(".mm") else "cxx", core_flags, "") for f in checked]

    clang = os.path.abspath(os.path.join(src, CLANG, "bin/clang++"))

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
            for m in re.finditer(r"^(\S+?):(\d+):(\d+): (?:fatal )?error: (.*)$", err, re.M):
                path = re.sub(r"^\.\./\.\./browser_core/", "core/", m.group(1))
                print(f"::error file={path},line={m.group(2)},col={m.group(3)}::{m.group(4)}")

    lines = [f"Checked {len(jobs)} core files for the Mac; {len(failed)} failed."]
    if failed:
        lines.append("Failed: " + ", ".join(failed) + ".")
    if checked:
        lines.append("Added by this branch, checked with the core's flags: " + ", ".join(checked))
    skipped = [f for f in added if f not in checked]
    if skipped:
        lines.append("Added in platform/, maybe another OS's, not checked: " + ", ".join(skipped))
    if gone:
        lines.append("In the engine's build, not in this branch: " + ", ".join(gone) + ".")
    text = "\n\n".join(lines)
    print(text)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as s:
            s.write(text + "\n")
    return 1 if failed else 0


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "pack":
        pack(sys.argv[2], sys.argv[3])
    elif len(sys.argv) == 4 and sys.argv[1] == "check":
        sys.exit(check(sys.argv[2], sys.argv[3]))
    else:
        sys.exit(__doc__)
