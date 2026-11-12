"""What a profile costs at launch, for #2138's question: does ilo load every profile? Not for merge.

  profile_cost.py spaces FILE N      adds an empty Space in each of N-1 more profiles, "Profile 1"
                                     on, to the Spaces the app saved in FILE
  profile_cost.py loaded SNAP DIR    the profile directories under the user data dir DIR that the
                                     processes in usage.py's snapshot SNAP hold files open in, and
                                     how many each, by kind of process
  profile_cost.py kinds SNAP         the number of processes by kind
  profile_cost.py trace FILE         the profile-loading spans in a JSON startup trace, by profile
"""

import json
import os
import sys


def spaces(path, count):
    with open(path) as f:
        saved = json.load(f)
    first = saved["spaces"][0]
    for n in range(1, count):
        space = {k: v for k, v in first.items() if k not in ("favorites", "shown_favorite",
                                                              "shown_own_favorite", "agent")}
        space.update(name=f"P{n + 1}", pinned=[], today=[], shown=0, profile=f"Profile {n}")
        saved["spaces"].append(space)
    with open(path, "w") as f:
        json.dump(saved, f)


def loaded(snap, data_dir):
    with open(snap) as f:
        procs = json.load(f)["procs"]
    root = os.path.realpath(data_dir) + "/"
    held = {}
    for pid, (kind, *_) in procs.items():
        try:
            fds = os.listdir(f"/proc/{pid}/fd")
        except OSError:
            continue
        for fd in fds:
            try:
                target = os.readlink(f"/proc/{pid}/fd/{fd}")
            except OSError:
                continue
            if not target.startswith(root):
                continue
            rest = target[len(root):]
            if "/" not in rest:
                continue
            profile = rest.split("/", 1)[0]
            held.setdefault(profile, {}).setdefault(kind, set()).add(rest.split("/", 1)[1])
    for profile in sorted(held):
        by_kind = ", ".join(f"{kind} {len(files)}" for kind, files in sorted(held[profile].items()))
        sample = sorted({f.split("/")[0] for files in held[profile].values() for f in files})[:8]
        print(f"{profile}: {by_kind}; {' '.join(sample)}")


def kinds(snap):
    with open(snap) as f:
        procs = json.load(f)["procs"]
    counted = {}
    for kind, *_ in procs.values():
        counted[kind] = counted.get(kind, 0) + 1
    print(f"{len(procs)} processes: " +
          ", ".join(f"{kind} {n}" for kind, n in sorted(counted.items(), key=lambda t: -t[1])))


def trace(path):
    with open(path) as f:
        text = f.read()
    # A trace cut at the quit can lack its closing brackets.
    try:
        data = json.loads(text)
    except ValueError:
        data = json.loads(text.rstrip().rstrip(",") + "]}")
    events = data["traceEvents"] if isinstance(data, dict) else data
    names = ("Profile::CreateProfile", "ProfileImpl::ctor", "ProfileImpl::OnPrefsLoaded",
             "ProfileImpl::DoFinalInit", "ProfileManager::CreateAndInitializeProfile",
             "ProfileManager::OnProfileCreationFinished")
    picked = [e for e in events if e.get("name") in names and e.get("ph") in ("X", "B")]
    if not picked:
        print("no profile spans")
        return
    start = min(e["ts"] for e in events if "ts" in e and e["ts"] > 0)
    for e in sorted(picked, key=lambda e: e["ts"]):
        path = (e.get("args") or {}).get("profile_path", "")
        dur = e.get("dur", 0) / 1000
        print(f"{(e['ts'] - start) / 1000:9.1f} ms  {dur:7.1f} ms  {e['name']} {path}".rstrip())


def main():
    command = sys.argv[1]
    if command == "spaces":
        spaces(sys.argv[2], int(sys.argv[3]))
    elif command == "loaded":
        loaded(sys.argv[2], sys.argv[3])
    elif command == "kinds":
        kinds(sys.argv[2])
    elif command == "trace":
        trace(sys.argv[2])


main()
