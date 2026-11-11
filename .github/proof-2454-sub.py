"""Proof only (#2454): as zypak starts a renderer, a child in a sub-sandbox of the Flatpak's
(flatpak-spawn --sandbox --expose-pids, its own pid namespace), and whether this process, as
Crashpad's handler is, can read its memory: before and after the child allows any ptracer, which it
must, as it can't name the handler's pid from its namespace."""
import ctypes
import functools
import os
import signal
import subprocess

print = functools.partial(print, flush=True)

CHILD = r"""
import ctypes, os, sys
libc = ctypes.CDLL(None, use_errno=True)
secret = b"crash-probe-secret"
buffer = ctypes.create_string_buffer(secret)
ns = [l.split()[1:] for l in open("/proc/self/status") if l.startswith("NSpid:")][0]
print("child", os.getpid(), ctypes.addressof(buffer), " ".join(ns), flush=True)
sys.stdin.readline()
# PR_SET_PTRACER, PR_SET_PTRACER_ANY
print("set", libc.prctl(0x59616D61, ctypes.c_ulong(-1), 0, 0, 0), flush=True)
sys.stdin.readline()
"""

for line in open("/proc/self/status"):
    if line.startswith(("NSpid:", "Seccomp:")):
        print("  parent", line.strip())
# From /, as the sub-sandbox has none of the Flatpak's folders but its own.
child = subprocess.Popen(["flatpak-spawn", "--sandbox", "--expose-pids", "--directory=/", "python3",
                          "-c", CHILD], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
                         cwd="/")
signal.alarm(60)
first = child.stdout.readline().split()
print("  child says:", first)
own_pid, address = int(first[1]), int(first[2])
# The child's pid in this namespace, found by its own pid in the innermost of its namespaces.
pid = None
for entry in os.listdir("/proc"):
    if not entry.isdigit():
        continue
    try:
        status = open(f"/proc/{entry}/status").read()
    except OSError:
        continue
    ns = [l.split()[1:] for l in status.splitlines() if l.startswith("NSpid:")]
    if ns and len(ns[0]) > 1 and ns[0][-1] == str(own_pid) and "python3" in open(f"/proc/{entry}/cmdline").read():
        pid = int(entry)
print(f"  child's pid here: {pid}")


def tries(when):
    if pid is None:
        print(f"  {when}: child not visible in this namespace")
        return
    try:
        with open(f"/proc/{pid}/mem", "rb", buffering=0) as mem:
            mem.seek(address)
            print(f"  {when}: /proc/<pid>/mem: read {mem.read(18) == b'crash-probe-secret'}")
    except OSError as e:
        print(f"  {when}: /proc/<pid>/mem: {e.strerror}")
    try:
        line = open(f"/proc/{pid}/task/{pid}/syscall").read().split()
        print(f"  {when}: /proc/<pid>/task/<tid>/syscall: {'sp and pc given' if len(line) == 9 else line}")
    except OSError as e:
        print(f"  {when}: /proc/<pid>/task/<tid>/syscall: {e.strerror}")


tries("not allowed")
child.stdin.write("go\n")
child.stdin.flush()
print("  child:", child.stdout.readline().strip())
tries("any ptracer allowed")
child.stdin.write("end\n")
child.stdin.flush()
child.wait(timeout=10)
