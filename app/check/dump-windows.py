"""#2439's probe, not to merge: a minidump of process PID with every thread's stack, and a line per
thread with its name and CPU time, while the app is stopping its threads.
Usage: dump-windows.py PID OUT.dmp"""

import ctypes
import ctypes.wintypes as w
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", newline="\n")
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
dbghelp = ctypes.WinDLL("dbghelp", use_last_error=True)
PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010
PROCESS_DUP_HANDLE = 0x0040
THREAD_QUERY_LIMITED_INFORMATION = 0x0800
TH32CS_SNAPTHREAD = 0x00000004
GENERIC_WRITE = 0x40000000
CREATE_ALWAYS = 2
# MiniDumpWithHandleData | WithUnloadedModules | WithProcessThreadData | WithFullMemoryInfo |
# WithThreadInfo: stacks for every thread, which a normal minidump has, and what they wait on.
DUMP_TYPE = 0x4 | 0x20 | 0x100 | 0x800 | 0x1000


class THREADENTRY32(ctypes.Structure):
    _fields_ = [("dwSize", w.DWORD), ("cntUsage", w.DWORD), ("th32ThreadID", w.DWORD),
                ("th32OwnerProcessID", w.DWORD), ("tpBasePri", w.LONG), ("tpDeltaPri", w.LONG),
                ("dwFlags", w.DWORD)]


def threads(pid):
    kernel32.CreateToolhelp32Snapshot.restype = w.HANDLE
    snap = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
    entry = THREADENTRY32()
    entry.dwSize = ctypes.sizeof(entry)
    ok = kernel32.Thread32First(snap, ctypes.byref(entry))
    while ok:
        if entry.th32OwnerProcessID == pid:
            yield entry.th32ThreadID
        ok = kernel32.Thread32Next(snap, ctypes.byref(entry))
    kernel32.CloseHandle(snap)


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
        print("thread " + describe(tid))
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


def children(pid):
    """The processes whose parent is PID, each with its command line, from WMI."""
    script = ("Get-CimInstance Win32_Process -Filter 'ParentProcessId=%d' | "
              "ForEach-Object { \"$($_.ProcessId) $($_.CommandLine)\" }" % pid)
    listed = subprocess.run(["powershell", "-NoProfile", "-Command", script], capture_output=True,
                            text=True, timeout=30).stdout
    for line in listed.splitlines():
        child, _, command = line.strip().partition(" ")
        if child.isdigit():
            yield int(child), command


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__.splitlines()[-1])
    pid = int(sys.argv[1])
    stem = sys.argv[2][:-len(".dmp")] if sys.argv[2].endswith(".dmp") else sys.argv[2]
    # The children first, as the browser may end once the one it waits for has.
    for child, command in children(pid):
        kind = re.search(r"--type=(\S+)", command)
        sub = re.search(r"--utility-sub-type=(\S+)", command)
        print("child %d %s %s" % (child, kind.group(1) if kind else "?", sub.group(1) if sub else ""))
        if sub and "NetworkService" in sub.group(1):
            dump(child, "%s-network-%d.dmp" % (stem, child))
    dump(pid, sys.argv[2])


main()
