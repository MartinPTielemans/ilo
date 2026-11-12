"""#2439's probe, not to merge: while the app at PID stops, a minidump of it and of its child
processes, with every thread's stack and a line per thread with its name and CPU time, when its
threads haven't stopped AFTER ms after it said it was stopping. Started at "browser: stopping".
Usage: dump-windows.py PID OUT.dmp LOG AFTER"""

import ctypes
import ctypes.wintypes as w
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", newline="\n")
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
dbghelp = ctypes.WinDLL("dbghelp", use_last_error=True)
PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010
PROCESS_DUP_HANDLE = 0x0040
THREAD_QUERY_LIMITED_INFORMATION = 0x0800
TH32CS_SNAPPROCESS = 0x00000002
TH32CS_SNAPTHREAD = 0x00000004
GENERIC_WRITE = 0x40000000
CREATE_ALWAYS = 2
# MiniDumpWithHandleData | WithUnloadedModules | WithProcessThreadData | WithFullMemoryInfo |
# WithThreadInfo: stacks for every thread, which a normal minidump has, and what they wait on.
DUMP_TYPE = 0x4 | 0x20 | 0x100 | 0x800 | 0x1000
STOPPED = "browser: quit: threads stopped"


class THREADENTRY32(ctypes.Structure):
    _fields_ = [("dwSize", w.DWORD), ("cntUsage", w.DWORD), ("th32ThreadID", w.DWORD),
                ("th32OwnerProcessID", w.DWORD), ("tpBasePri", w.LONG), ("tpDeltaPri", w.LONG),
                ("dwFlags", w.DWORD)]


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [("dwSize", w.DWORD), ("cntUsage", w.DWORD), ("th32ProcessID", w.DWORD),
                ("th32DefaultHeapID", ctypes.c_size_t), ("th32ModuleID", w.DWORD),
                ("cntThreads", w.DWORD), ("th32ParentProcessID", w.DWORD),
                ("pcPriClassBase", w.LONG), ("dwFlags", w.DWORD), ("szExeFile", w.WCHAR * 260)]


def snapshot(kind):
    kernel32.CreateToolhelp32Snapshot.restype = w.HANDLE
    return kernel32.CreateToolhelp32Snapshot(kind, 0)


def threads(pid):
    snap = snapshot(TH32CS_SNAPTHREAD)
    entry = THREADENTRY32()
    entry.dwSize = ctypes.sizeof(entry)
    ok = kernel32.Thread32First(snap, ctypes.byref(entry))
    while ok:
        if entry.th32OwnerProcessID == pid:
            yield entry.th32ThreadID
        ok = kernel32.Thread32Next(snap, ctypes.byref(entry))
    kernel32.CloseHandle(snap)


def children(pid):
    """The processes whose parent is PID, from a snapshot, which takes milliseconds."""
    snap = snapshot(TH32CS_SNAPPROCESS)
    entry = PROCESSENTRY32W()
    entry.dwSize = ctypes.sizeof(entry)
    found = []
    ok = kernel32.Process32FirstW(snap, ctypes.byref(entry))
    while ok:
        if entry.th32ParentProcessID == pid:
            found.append(entry.th32ProcessID)
        ok = kernel32.Process32NextW(snap, ctypes.byref(entry))
    kernel32.CloseHandle(snap)
    return found


def describe(tid):
    kernel32.OpenThread.restype = w.HANDLE
    handle = kernel32.OpenThread(THREAD_QUERY_LIMITED_INFORMATION, False, tid)
    if not handle:
        return "%d (can't open: %d)" % (tid, ctypes.get_last_error())
    name = ctypes.c_wchar_p()
    kernel32.GetThreadDescription.argtypes = [w.HANDLE, ctypes.POINTER(ctypes.c_wchar_p)]
    described = kernel32.GetThreadDescription(handle, ctypes.byref(name)) >= 0
    times = [w.FILETIME() for _ in range(4)]
    kernel32.GetThreadTimes(handle, *[ctypes.byref(t) for t in times])
    cpu = sum((t.dwHighDateTime << 32 | t.dwLowDateTime) for t in times[2:]) // 10000
    kernel32.CloseHandle(handle)
    text = name.value if described and name.value else ""
    return "%d %r cpu %d ms" % (tid, text, cpu)


def dump(pid, path):
    for tid in threads(pid):
        print("thread %d: %s" % (pid, describe(tid)))
    kernel32.OpenProcess.restype = w.HANDLE
    process = kernel32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_DUP_HANDLE,
                                   False, pid)
    if not process:
        print("can't open %d: %d" % (pid, ctypes.get_last_error()))
        return
    kernel32.CreateFileW.restype = w.HANDLE
    out = kernel32.CreateFileW(path, GENERIC_WRITE, 0, None, CREATE_ALWAYS, 0, None)
    dbghelp.MiniDumpWriteDump.argtypes = [w.HANDLE, w.DWORD, w.HANDLE, w.DWORD, ctypes.c_void_p,
                                          ctypes.c_void_p, ctypes.c_void_p]
    if not dbghelp.MiniDumpWriteDump(process, pid, out, DUMP_TYPE, None, None, None):
        print("MiniDumpWriteDump of %d failed: %d" % (pid, ctypes.get_last_error()))
    kernel32.CloseHandle(out)
    kernel32.CloseHandle(process)
    print("dumped %d in %s" % (pid, path))


def stopped(log):
    with open(log, encoding="utf-8", errors="replace") as text:
        return any(line.startswith(STOPPED) for line in text)


def main():
    if len(sys.argv) != 5:
        sys.exit(__doc__.splitlines()[-1])
    pid, path, log, after = int(sys.argv[1]), sys.argv[2], sys.argv[3], int(sys.argv[4]) / 1000
    stem = path[:-len(".dmp")] if path.endswith(".dmp") else path
    start = time.monotonic()
    # Each child's command line, for which is the network service, from WMI, which takes a
    # second or two, so asked at once and read after the dumps.
    script = ("Get-CimInstance Win32_Process -Filter 'ParentProcessId=%d' | "
              "ForEach-Object { \"$($_.ProcessId) $($_.CommandLine)\" }" % pid)
    listing = subprocess.Popen(["powershell", "-NoProfile", "-Command", script],
                               stdout=subprocess.PIPE, text=True)
    while time.monotonic() - start < after:
        if stopped(log):
            listing.kill()
            return
        time.sleep(0.05)
    print("threads not stopped %d ms after stopping" % ((time.monotonic() - start) * 1000))
    # Every child: the one the quit waits for may end before WMI says which it is.
    for child in children(pid):
        dump(child, "%s-child-%d.dmp" % (stem, child))
    dump(pid, path)
    print("dumps done %d ms after stopping" % ((time.monotonic() - start) * 1000))
    for line in listing.communicate(timeout=60)[0].splitlines():
        child, _, command = line.strip().partition(" ")
        kind = re.search(r"--type=(\S+)", command)
        sub = re.search(r"--utility-sub-type=(\S+)", command)
        print("child %s %s %s" % (child, kind.group(1) if kind else "?", sub.group(1) if sub else ""))
    while time.monotonic() - start < 15 and not stopped(log):
        time.sleep(0.05)
    print("threads %s %d ms after stopping" % ("stopped" if stopped(log) else "still not stopped",
                                               (time.monotonic() - start) * 1000))


main()
