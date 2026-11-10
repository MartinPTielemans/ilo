"""Where a browser's browser process keeps its memory, with the OS's own tools, for #2138. Not for
merge: the perf run calls it at 20 tabs, for ilo and for Chrome, and its files go in the artifact.

  dissect.py SNAPSHOT OUT LABEL   reads usage.py's snapshot, takes its browser process and its
                                  largest renderer, and writes OUT/LABEL-* for each
"""

import ctypes
import json
import os
import subprocess
import sys


def run(args, path):
    try:
        done = subprocess.run(args, capture_output=True, text=True, timeout=120)
        out = done.stdout + ("\n--- stderr\n" + done.stderr if done.stderr else "")
    except Exception as error:  # noqa: BLE001 - a diagnostic, it says what failed
        out = f"failed: {error}"
    with open(path, "w") as f:
        f.write(out)
    return out


def mac(pid, out):
    for tool, args in (("footprint", ["footprint", "-p", pid]),
                       ("vmmap", ["vmmap", "--summary", pid])):
        text = run(["sudo", "-n"] + args, f"{out}-{tool}.txt")
        if text.startswith("failed") or "sudo:" in text:
            run(args, f"{out}-{tool}.txt")


def linux(pid, out):
    # smaps whole, and grouped by mapping name: Anonymous + Swap, usage.py's RssAnon + VmSwap.
    groups = {}
    sizes = {}
    name = None
    with open(f"/proc/{pid}/smaps") as f, open(f"{out}-smaps.txt", "w") as copy:
        for line in f:
            copy.write(line)
            words = line.split()
            if words and "-" in words[0] and not words[0].endswith(":"):
                name = words[5] if len(words) > 5 else "[anon]"
                start, end = (int(x, 16) for x in words[0].split("-"))
                size = end - start
            elif words and words[0] in ("Anonymous:", "Swap:"):
                kb = int(words[1])
                groups[name] = groups.get(name, 0) + kb
                if name == "[anon]" and words[0] == "Anonymous:" and kb:
                    key = size >> 20
                    count, total = sizes.get(key, (0, 0))
                    sizes[key] = (count + 1, total + kb)
    with open(f"{out}-smaps-grouped.txt", "w") as f:
        for name, kb in sorted(groups.items(), key=lambda item: -item[1]):
            if kb >= 256:
                f.write(f"{kb / 1024:9.1f} MiB  {name}\n")
        f.write("\nunnamed anonymous mappings by size (MiB), the most resident first:\n")
        for key, (count, kb) in sorted(sizes.items(), key=lambda item: -item[1][1])[:40]:
            f.write(f"  {key:6d} MiB mappings: {count:4d}, resident {kb / 1024:9.1f} MiB\n")
    for path in ("status", "smaps_rollup"):
        try:
            with open(f"/proc/{pid}/{path}") as f, open(f"{out}-{path}.txt", "w") as copy:
                copy.write(f.read())
        except OSError:
            pass
    run(["sh", "-c", f"ls -l /proc/{pid}/fd | sed 's/.* -> //' | sort | uniq -c | sort -rn"],
        f"{out}-fds.txt")
    if os.environ.get("DISPLAY"):
        run(["xwininfo", "-root", "-tree"], f"{out}-xwindows.txt")


def windows(pid, out):
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi")
    user32 = ctypes.WinDLL("user32")
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user32.IsWindowVisible.argtypes = [wintypes.HWND]

    class Region(ctypes.Structure):
        _fields_ = [("base", ctypes.c_uint64), ("allocation", ctypes.c_uint64),
                    ("allocation_protect", wintypes.DWORD), ("partition", wintypes.WORD),
                    ("size", ctypes.c_uint64), ("state", wintypes.DWORD),
                    ("protect", wintypes.DWORD), ("type", wintypes.DWORD)]

    kernel32.VirtualQueryEx.argtypes = [wintypes.HANDLE, ctypes.c_uint64, ctypes.POINTER(Region),
                                        ctypes.c_size_t]
    kernel32.VirtualQueryEx.restype = ctypes.c_size_t
    psapi.GetMappedFileNameW.argtypes = [wintypes.HANDLE, ctypes.c_uint64, wintypes.LPWSTR,
                                         wintypes.DWORD]
    handle = kernel32.OpenProcess(0x0410, False, int(pid))
    if not handle:
        with open(f"{out}-regions.txt", "w") as f:
            f.write(f"OpenProcess failed: {ctypes.get_last_error()}\n")
        return

    # NT heaps, by their base: Heap32ListFirst lists them without walking their blocks.
    class HeapList(ctypes.Structure):
        _fields_ = [("size", ctypes.c_size_t), ("pid", wintypes.DWORD),
                    ("heap", ctypes.c_size_t), ("flags", wintypes.DWORD)]

    heaps = set()
    kernel32.Heap32ListFirst.argtypes = [wintypes.HANDLE, ctypes.POINTER(HeapList)]
    kernel32.Heap32ListNext.argtypes = [wintypes.HANDLE, ctypes.POINTER(HeapList)]
    snap = kernel32.CreateToolhelp32Snapshot(0x1, int(pid))
    if snap and snap != ctypes.c_void_p(-1).value:
        entry = HeapList(size=ctypes.sizeof(HeapList))
        ok = kernel32.Heap32ListFirst(snap, ctypes.byref(entry))
        while ok:
            heaps.add(entry.heap)
            ok = kernel32.Heap32ListNext(snap, ctypes.byref(entry))
        kernel32.CloseHandle(snap)

    allocations = {}
    address = 0
    region = Region()
    name = ctypes.create_unicode_buffer(1024)
    with open(f"{out}-regions.tsv", "w") as f:
        f.write("base\tallocation\tsize\tstate\ttype\tprotect\tfile\n")
        while kernel32.VirtualQueryEx(handle, address, ctypes.byref(region), ctypes.sizeof(region)):
            file = ""
            if region.type in (0x40000, 0x1000000) and region.state == 0x1000:
                if psapi.GetMappedFileNameW(handle, region.base, name, 1024):
                    file = name.value
            f.write(f"{region.base:#x}\t{region.allocation:#x}\t{region.size}\t{region.state:#x}\t"
                    f"{region.type:#x}\t{region.protect:#x}\t{file}\n")
            if region.state == 0x1000 and region.type == 0x20000:
                kind = "heap" if region.allocation in heaps else "private"
                count, total = allocations.get((region.allocation, kind), (0, 0))
                allocations[(region.allocation, kind)] = (count + 1, total + region.size)
            address = region.base + region.size
            if address >= 1 << 47:
                break
    kernel32.CloseHandle(handle)
    by_size = {}
    total = sum(size for _, size in allocations.values())
    with open(f"{out}-private.txt", "w") as f:
        f.write(f"committed private: {total / 1048576:.1f} MiB in {len(allocations)} allocations,"
                f" {len(heaps)} NT heaps\n")
        heap_total = sum(s for (_, kind), (_, s) in allocations.items() if kind == "heap")
        f.write(f"of it in NT heaps' first segments: {heap_total / 1048576:.1f} MiB\n\n")
        f.write("largest allocations:\n")
        for (base, kind), (count, size) in sorted(allocations.items(), key=lambda i: -i[1][1])[:60]:
            f.write(f"  {base:#016x} {kind:7s} {size / 1048576:8.2f} MiB in {count} regions\n")
        for (_, kind), (_, size) in allocations.items():
            key = (kind, size >> 20)
            n, t = by_size.get(key, (0, 0))
            by_size[key] = (n + 1, t + size)
        f.write("\ncommitted private by allocation size (MiB), the most first:\n")
        for (kind, mib), (n, t) in sorted(by_size.items(), key=lambda i: -i[1][1])[:40]:
            f.write(f"  {kind:7s} {mib:5d} MiB: {n:5d} allocations, {t / 1048576:8.1f} MiB\n")

    # The process's windows, top-level and child: each GPUI window has its own swap chain.
    lines = []
    WNDENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def describe(hwnd, depth):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value != int(pid):
            return
        cls = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(hwnd, cls, 256)
        rect = wintypes.RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(rect))
        lines.append(f"{'  ' * depth}{hwnd:#x} {cls.value} {rect.right - rect.left}x"
                     f"{rect.bottom - rect.top} visible={bool(user32.IsWindowVisible(hwnd))}")

    def top(hwnd, _):
        describe(hwnd, 0)

        def child(hwnd, _):
            describe(hwnd, 1)
            return True
        user32.EnumChildWindows(hwnd, WNDENUMPROC(child), 0)
        return True
    user32.EnumWindows(WNDENUMPROC(top), 0)
    with open(f"{out}-windows.txt", "w") as f:
        f.write(f"{len(lines)} windows\n" + "\n".join(lines) + "\n")
    vmmap = os.environ.get("DISSECT_VMMAP")
    if vmmap and os.path.exists(vmmap):
        run([vmmap, "-accepteula", "-p", pid, os.path.abspath(f"{out}-vmmap.csv")],
            f"{out}-vmmap-run.txt")


def main():
    snapshot, out_dir, label = sys.argv[1:4]
    with open(snapshot) as f:
        procs = json.load(f)["procs"]
    largest = {}
    for pid, (kind, _, _, footprint) in procs.items():
        if kind in ("browser", "renderer") and footprint > largest.get(kind, (None, -1))[1]:
            largest[kind] = (pid, footprint)
    taker = {"darwin": mac, "win32": windows}.get(sys.platform, linux)
    for kind, (pid, footprint) in largest.items():
        if kind == "renderer" and sys.platform != "darwin":
            continue
        out = os.path.join(out_dir, f"{label}-{kind}")
        with open(f"{out}-pid.txt", "w") as f:
            f.write(f"{pid} {footprint / 1048576:.1f} MiB\n")
        try:
            taker(pid, out)
        except Exception as error:  # noqa: BLE001 - a diagnostic, it says what failed
            with open(f"{out}-error.txt", "w") as f:
                f.write(f"{type(error).__name__}: {error}\n")


main()
