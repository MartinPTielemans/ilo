"""What the browser's processes use, for the perf run (#456), the way Ardia's bench measured it
(docs/perf in its repo): CPU and wakeups as the change between two snapshots, memory summed over
every process of the browser.

  usage.py snap PROCS FILE   writes a snapshot of every process whose path has PROCS in it
  usage.py memory FILE       prints the memory summed, in MiB, and how many processes
  usage.py idle FILE...      prints each window's CPU, % of one core, and wakeups a second, from
                             each snapshot to the next, as "cpu wakeups" a line

On the Mac it's proc_pid_rusage's: phys_footprint, user and system time, and package idle and
interrupt wakeups, as Ardia's acperf.swift reads them. Linux has no wakeup count, so it's each
thread's context switches, and memory is RssAnon and VmSwap, Chromium's private footprint there.
On Windows it's NtQuerySystemInformation's processes: PagefileUsage (private bytes, Chromium's
footprint there), user and kernel time, and each thread's context switches. Context switches count
only the threads still running, so a window takes no process's below zero.
"""

import ctypes
import json
import os
import subprocess
import sys
import time


def kind_of(args):
    for word in args.split():
        if word.startswith("--type="):
            return word[len("--type="):]
    return "browser"


def mac(procs):
    lib = ctypes.CDLL("/usr/lib/libSystem.B.dylib")

    class Timebase(ctypes.Structure):
        _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]

    class RusageV0(ctypes.Structure):
        _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [
            (name, ctypes.c_uint64)
            for name in ("user", "system", "idle_wakeups", "interrupt_wakeups", "pageins",
                         "wired", "resident", "footprint", "start", "exit")
        ]

    timebase = Timebase()
    lib.mach_timebase_info(ctypes.byref(timebase))
    seen = {}
    listed = subprocess.run(["ps", "-Axww", "-o", "pid=,args="], capture_output=True, text=True)
    for line in listed.stdout.splitlines():
        pid, _, args = line.strip().partition(" ")
        if procs not in args or int(pid) == os.getpid():
            continue
        info = RusageV0()
        if lib.proc_pid_rusage(int(pid), 0, ctypes.byref(info)) != 0:
            continue
        ns = (info.user + info.system) * timebase.numer / timebase.denom
        seen[pid] = [kind_of(args), ns / 1e9, info.idle_wakeups + info.interrupt_wakeups,
                     info.footprint]
    return seen


def linux(procs):
    hz = os.sysconf("SC_CLK_TCK")
    seen = {}
    for pid in filter(str.isdigit, os.listdir("/proc")):
        try:
            with open(f"/proc/{pid}/cmdline", "rb") as f:
                args = f.read().replace(b"\0", b" ").decode(errors="replace")
            if procs not in args or int(pid) == os.getpid():
                continue
            with open(f"/proc/{pid}/stat") as f:
                fields = f.read().rpartition(")")[2].split()
            with open(f"/proc/{pid}/status") as f:
                status = dict(line.split(":", 1) for line in f if ":" in line)
            switches = 0
            for task in os.listdir(f"/proc/{pid}/task"):
                with open(f"/proc/{pid}/task/{task}/status") as f:
                    for line in f:
                        if "ctxt_switches:" in line:
                            switches += int(line.split()[1])
        except OSError:
            continue
        kb = sum(int(status.get(key, "0 kB").split()[0]) for key in ("RssAnon", "VmSwap"))
        # utime and stime, the 14th and 15th of stat's fields, the 12th and 13th after its name.
        seen[pid] = [kind_of(args), (int(fields[11]) + int(fields[12])) / hz, switches, kb * 1024]
    return seen


def windows(procs):
    from ctypes import wintypes

    ntdll = ctypes.WinDLL("ntdll")
    kernel32 = ctypes.WinDLL("kernel32")
    kernel32.OpenProcess.restype = wintypes.HANDLE
    size = 1 << 20
    while True:
        buffer = ctypes.create_string_buffer(size)
        needed = wintypes.ULONG()
        status = ntdll.NtQuerySystemInformation(5, buffer, size, ctypes.byref(needed))
        if status & 0xFFFFFFFF != 0xC0000004:
            break
        size = max(size * 2, needed.value + 65536)
    if status != 0:
        sys.exit(f"NtQuerySystemInformation failed: {status & 0xFFFFFFFF:#x}")
    want = procs.replace("\\", "/").lower()
    seen = {}
    offset = 0
    raw = buffer.raw
    while True:
        # SYSTEM_PROCESS_INFORMATION on x64: 256 bytes, then a SYSTEM_THREAD_INFORMATION of 80
        # bytes a thread, whose ContextSwitches is at 64.
        entry = raw[offset:offset + 256]
        next_offset, threads = int.from_bytes(entry[0:4], "little"), int.from_bytes(entry[4:8], "little")
        pid = int.from_bytes(entry[80:88], "little")
        handle = kernel32.OpenProcess(0x1000, False, pid) if pid else None
        if handle:
            name = ctypes.create_unicode_buffer(1024)
            length = wintypes.DWORD(1024)
            if kernel32.QueryFullProcessImageNameW(handle, 0, name, ctypes.byref(length)) and \
                    name.value.replace("\\", "/").lower() == want:
                user = int.from_bytes(entry[40:48], "little")
                kernel = int.from_bytes(entry[48:56], "little")
                switches = sum(
                    int.from_bytes(raw[at + 64:at + 68], "little")
                    for at in range(offset + 256, offset + 256 + 80 * threads, 80))
                seen[str(pid)] = ["process", (user + kernel) / 1e7, switches,
                                  int.from_bytes(entry[184:192], "little")]
            kernel32.CloseHandle(handle)
        if not next_offset:
            return seen
        offset += next_offset


def read(path):
    with open(path) as f:
        return json.load(f)


def main():
    command = sys.argv[1]
    if command == "snap":
        procs, path = sys.argv[2], sys.argv[3]
        taker = {"darwin": mac, "win32": windows}.get(sys.platform, linux)
        seen = taker(procs)
        with open(path, "w") as f:
            json.dump({"time": time.time(), "procs": seen}, f)
    elif command == "memory":
        seen = read(sys.argv[2])["procs"]
        print(f"{sum(p[3] for p in seen.values()) / 1048576:.0f} {len(seen)}")
    elif command == "idle":
        snaps = [read(path) for path in sys.argv[2:]]
        for a, b in zip(snaps, snaps[1:]):
            secs = b["time"] - a["time"]
            cpu = wakeups = 0
            for pid, now in b["procs"].items():
                was = a["procs"].get(pid, [now[0], 0, 0, 0])
                cpu += max(now[1] - was[1], 0)
                wakeups += max(now[2] - was[2], 0)
            print(f"{cpu * 100 / secs:.3f} {wakeups / secs:.2f}")
    else:
        sys.exit(f"usage.py: no command {command}")


main()
