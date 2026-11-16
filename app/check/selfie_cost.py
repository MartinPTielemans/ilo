"""Probe for #2502, never merged: what uBlock Origin's selfie write costs while a user works.

  python3 -I check/selfie_cost.py sample OUT REGEX STOP
  python3 -I check/selfie_cost.py report SAMPLES ACTIONS LAUNCH_MS SELFIE_MS LEG

sample writes a line every 250 ms to OUT, "ms pid kind thread ticks", for each thread of each
process whose command line matches REGEX, until the file STOP exists. kind is browser, extension,
renderer, gpu or utility; thread is main, pool, io or other in the browser, else the process.
report sums them per second and says, for each action in ACTIONS ("ms what took_ms"), the
browser's pool CPU in the 500 ms after it started.
"""

import os
import re
import sys
import time


def kind_of(cmd):
    if "--extension-process" in cmd:
        return "extension"
    m = re.search(r"--type=([a-z-]+)", cmd)
    if not m:
        return "browser"
    return {"gpu-process": "gpu"}.get(m.group(1), m.group(1))


def thread_of(kind, pid, tid, comm):
    if kind != "browser":
        return kind
    if tid == pid:
        return "main"
    if comm.startswith("ThreadPool"):
        return "pool"
    if comm == "Chrome_IOThread":
        return "io"
    return "other"


def sample(out, regex, stop):
    pat = re.compile(regex)
    with open(out, "w") as f:
        while not os.path.exists(stop):
            ms = int(time.time() * 1000)
            for pid in filter(str.isdigit, os.listdir("/proc")):
                try:
                    cmd = open(f"/proc/{pid}/cmdline", "rb").read().replace(b"\0", b" ").decode("utf-8", "replace")
                except OSError:
                    continue
                if int(pid) == os.getpid() or "selfie_cost.py" in cmd or not pat.search(cmd):
                    continue
                kind = kind_of(cmd)
                try:
                    tids = os.listdir(f"/proc/{pid}/task")
                except OSError:
                    continue
                for tid in tids:
                    try:
                        stat = open(f"/proc/{pid}/task/{tid}/stat").read()
                    except OSError:
                        continue
                    comm = stat[stat.index("(") + 1:stat.rindex(")")]
                    fields = stat[stat.rindex(")") + 2:].split()
                    ticks = int(fields[11]) + int(fields[12])
                    f.write(f"{ms} {pid} {kind} {thread_of(kind, pid, tid, comm)}:{tid} {ticks}\n")
            f.flush()
            time.sleep(0.25)


def report(samples, actions, launch, selfie, leg):
    hz = os.sysconf("SC_CLK_TCK")
    last = {}
    per = {}  # (ms, class) -> ticks since the last sample
    times = []
    for line in open(samples):
        ms, pid, kind, thread, ticks = line.split()
        ms, ticks = int(ms), int(ticks)
        key = (pid, thread)
        if key in last:
            cls = f"{kind}/{thread.split(':')[0]}" if kind == "browser" else kind
            per[(ms, cls)] = per.get((ms, cls), 0) + ticks - last[key]
        last[key] = ticks
        if not times or times[-1] != ms:
            times.append(ms)
    classes = sorted({c for _, c in per})
    launch, selfie = int(launch), int(selfie)
    print(f"probe (#2502) {leg}: the selfie in both stores {(selfie - launch) / 1000:.1f} s after launch; CPU ms a second, from 30 s:")
    print(f"probe (#2502) {leg}:   s " + " ".join(classes))
    seconds = {}
    for (ms, c), t in per.items():
        s = (ms - launch) // 1000
        seconds.setdefault(s, {}).setdefault(c, 0)
        seconds[s][c] += t
    for s in sorted(seconds):
        if s < 30:
            continue
        row = seconds[s]
        print(f"probe (#2502) {leg}: {s:3d} " + " ".join(f"{row.get(c, 0) * 1000 // hz:4d}" for c in classes))
    total = {}
    for (ms, c), t in per.items():
        if ms >= launch + 30000:
            total[c] = total.get(c, 0) + t
    print(f"probe (#2502) {leg}: from 30 s to the end, CPU ms: " + ", ".join(f"{c} {t * 1000 // hz}" for c, t in sorted(total.items())))
    def cpu(cls, at, took):
        # The class's CPU in the action's span, as ms a second: the samples after its start
        # until the one after its end.
        t = sum(t for (ms, c), t in per.items() if c == cls and at < ms <= at + took + 250)
        return t * 1000 // hz * 1000 // max(took + 250, 250)
    rows = {"busy": {}, "quiet": {}}
    for line in open(actions):
        at, what, took = line.split()
        at, took = int(at), int(float(took))
        ext, pool, main = cpu("extension", at, took), cpu("browser/pool", at, took), cpu("browser/main", at, took)
        print(f"probe (#2502) {leg}: action at {(at - launch) / 1000:.1f} s: {what} {took} ms; while it waited, CPU ms a second: extension {ext}, browser pool {pool}, browser main {main}")
        side = "busy" if ext >= 500 else "quiet"
        rows[side].setdefault(what, []).append(took)
    for side, by in rows.items():
        for what, v in sorted(by.items()):
            v.sort()
            n = len(v)
            m = v[n // 2] if n % 2 else (v[n // 2 - 1] + v[n // 2]) / 2
            print(f"probe (#2502) {leg}: {what} with the extension {side}: median {m:.0f} ms ({v[0]:.0f}–{v[-1]:.0f}, n={n})")


if __name__ == "__main__":
    if sys.argv[1] == "sample":
        sample(*sys.argv[2:5])
    else:
        report(*sys.argv[2:7])
