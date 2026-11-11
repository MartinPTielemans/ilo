"""Proof only (#2454): in the Flatpak's sandbox, a child (as Crashpad's handler is the browser's) reads
its parent's memory, before and after the parent names it its ptracer, by ptrace, /proc/<pid>/mem,
process_vm_readv and /proc/<pid>/task/<tid>/syscall."""
import ctypes
import functools
import os

# The child ends with os._exit, which flushes nothing.
print = functools.partial(print, flush=True)

libc = ctypes.CDLL(None, use_errno=True)
libc.ptrace.restype = ctypes.c_long
libc.ptrace.argtypes = [ctypes.c_long, ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p]
PR_SET_PTRACER = 0x59616D61
PTRACE_ATTACH, PTRACE_DETACH = 16, 17


class Iovec(ctypes.Structure):
    _fields_ = [("base", ctypes.c_void_p), ("len", ctypes.c_size_t)]


libc.process_vm_readv.restype = ctypes.c_ssize_t
libc.process_vm_readv.argtypes = [ctypes.c_int, ctypes.POINTER(Iovec), ctypes.c_ulong,
                                  ctypes.POINTER(Iovec), ctypes.c_ulong, ctypes.c_ulong]

for line in open("/proc/self/status"):
    if line.startswith(("Seccomp:", "NoNewPrivs:")):
        print(" ", line.strip())

secret = b"crash-probe-secret"
buffer = ctypes.create_string_buffer(secret)
address = ctypes.addressof(buffer)
parent = os.getpid()
go_r, go_w = os.pipe()
done_r, done_w = os.pipe()


def tries(when):
    ok = libc.ptrace(PTRACE_ATTACH, parent, None, None)
    err = ctypes.get_errno()
    print(f"  {when}: ptrace attach: {'ok' if ok == 0 else os.strerror(err)}")
    if ok == 0:
        os.waitpid(parent, 0x40000000)  # __WALL
        libc.ptrace(PTRACE_DETACH, parent, None, None)
    try:
        with open(f"/proc/{parent}/mem", "rb", buffering=0) as mem:
            mem.seek(address)
            print(f"  {when}: /proc/<pid>/mem: read {mem.read(len(secret)) == secret}")
    except OSError as e:
        print(f"  {when}: /proc/<pid>/mem: {e.strerror}")
    out = ctypes.create_string_buffer(len(secret))
    local = Iovec(ctypes.addressof(out), len(secret))
    remote = Iovec(address, len(secret))
    n = libc.process_vm_readv(parent, ctypes.byref(local), 1, ctypes.byref(remote), 1, 0)
    err = ctypes.get_errno()
    print(f"  {when}: process_vm_readv: "
          f"{'read ' + str(out.raw == secret) if n == len(secret) else os.strerror(err)}")
    try:
        tids = os.listdir(f"/proc/{parent}/task")
        line = open(f"/proc/{parent}/task/{tids[0]}/syscall").read().split()
        # Number, six arguments, then the stack pointer and the program counter.
        print(f"  {when}: /proc/<pid>/task/<tid>/syscall: {len(tids)} threads, "
              f"syscall {line[0]}, sp and pc {'given' if len(line) == 9 else line}")
    except OSError as e:
        print(f"  {when}: /proc/<pid>/task/<tid>/syscall: {e.strerror}")


child = os.fork()
if child == 0:
    tries("not named")
    os.write(done_w, b"1")
    os.read(go_r, 1)
    tries("named its ptracer")
    os._exit(0)
os.read(done_r, 1)
print(f"  PR_SET_PTRACER: {libc.prctl(PR_SET_PTRACER, child, 0, 0, 0)}")
os.write(go_w, b"1")
os.waitpid(child, 0)
