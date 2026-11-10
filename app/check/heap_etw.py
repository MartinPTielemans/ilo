"""#2317, not for merge: what a browser process holds by allocating stack, from xperf's dump of an
ETW trace with heap tracing and VirtualAlloc events, each with its stack.

heap_etw.py DUMP PID OUT

DUMP is `xperf -i trace.etl -symbols -a dumper`'s text, whose header names each event's fields.
Heap blocks still allocated, and pages still committed, when the trace stopped are summed by the
first frames outside the allocators, for PID alone.
"""

import collections
import gzip
import re
import sys

# Frames that only say an allocation happened, not who wanted it.
ALLOCATORS = re.compile(
    r"^(ntoskrnl\.exe|ntdll\.dll|kernelbase\.dll|kernel32\.dll|ucrtbase\.dll|msvcrt\.dll|"
    r"vcruntime140\.dll|\?)!", re.I)
# Events kept until their stack has had time to follow them.
WINDOW = 50000


def opener(path):
    return gzip.open(path, "rt", errors="replace") if path.endswith(".gz") else open(
        path, errors="replace")


def number(text):
    text = text.strip()
    try:
        return int(text, 16) if text.lower().startswith("0x") else int(text)
    except ValueError:
        return None


def main(dump, pid, out):
    pid = int(pid)
    header = {}
    in_header = False
    counts = collections.Counter()
    seen_names = collections.Counter()
    stacks = {}  # tuple of frames -> id
    frames_of = []
    pending = collections.OrderedDict()  # (ts, tid) -> [name, fields, frames]
    blocks = {}  # address -> (size, stack id)
    commits = {}  # base -> (end, stack id)
    lines = 0

    def field(fields, names, *wanted):
        for want in wanted:
            for i, name in enumerate(names):
                if name.lower().replace(" ", "") == want and i < len(fields):
                    return number(fields[i])
        return None

    def apply(name, fields, frames):
        frames = tuple(frames)
        sid = stacks.get(frames)
        if sid is None:
            sid = stacks[frames] = len(frames_of)
            frames_of.append(frames)
        names = header[name]
        low = name.lower()
        if low == "heapalloc":
            address = field(fields, names, "address")
            size = field(fields, names, "size")
            if address is not None and size is not None:
                blocks[address] = (size, sid)
        elif low == "heapfree":
            blocks.pop(field(fields, names, "address"), None)
        elif low == "heaprealloc":
            blocks.pop(field(fields, names, "oldaddress"), None)
            address = field(fields, names, "newaddress")
            size = field(fields, names, "newsize", "size")
            if address is not None and size is not None:
                blocks[address] = (size, sid)
        elif low in ("virtualalloc", "virtualfree"):
            base = field(fields, names, "baseaddr", "baseaddress")
            end = field(fields, names, "endaddr", "endaddress")
            flags = field(fields, names, "flags") or 0
            if base is None or end is None:
                return
            if low == "virtualalloc" and flags & 0x1000:  # MEM_COMMIT
                commits[base] = (end, sid)
            elif low == "virtualfree" and flags & (0x4000 | 0x8000):  # DECOMMIT, RELEASE
                for start in [b for b, (e, _) in commits.items() if b < end and e > base]:
                    del commits[start]

    tracked = {"heapalloc", "heapfree", "heaprealloc", "virtualalloc", "virtualfree"}
    with opener(dump) as lines_in:
        for line in lines_in:
            lines += 1
            line = line.rstrip("\n")
            if line.startswith("BeginHeader"):
                in_header = True
                continue
            if line.startswith("EndHeader"):
                in_header = False
                continue
            name, _, rest = line.partition(",")
            name = name.strip()
            if in_header:
                header[name] = [f.strip() for f in rest.split(",")]
                continue
            if name == "Stack":
                parts = [p.strip() for p in rest.split(",", 4)]
                if len(parts) == 5:
                    # An event the dump gives no thread takes the stack of its time stamp.
                    for key in ((parts[0], parts[1]), (parts[0], "")):
                        if key in pending:
                            pending[key][2].append(parts[4])
                            break
                continue
            if name not in header:
                continue
            names = header[name]
            fields = [f.strip() for f in rest.split(",", len(names) - 1)]
            proc = next((fields[i] for i, n in enumerate(names) if "PID" in n and i < len(fields)),
                        "")
            match = re.search(r"\(\s*(\d+)\)", proc)
            if not match or int(match.group(1)) != pid:
                continue
            seen_names[name] += 1
            if name.lower() not in tracked:
                continue
            counts[name] += 1
            ts = fields[0] if fields else ""
            tid = next((fields[i] for i, n in enumerate(names) if n == "ThreadID"), "")
            pending[(ts, tid)] = [name, fields, []]
            while len(pending) > WINDOW:
                apply(*pending.popitem(last=False)[1])
    while pending:
        apply(*pending.popitem(last=False)[1])

    with open(out, "w") as report:
        def say(text=""):
            print(text, file=report)

        say(f"{lines} lines; events for pid {pid}: {dict(seen_names.most_common(40))}")
        for name in sorted(n for n in header if n.lower() in tracked or n == "Stack"):
            say(f"  header {name}: {', '.join(header[name])}")
        with_stack = sum(1 for f in frames_of if f)
        say(f"{len(frames_of)} distinct stacks, {with_stack} with frames")

        def caller(frames, depth):
            outside = [f for f in frames if not ALLOCATORS.match(f)]
            return " <- ".join(outside[:depth]) or "(no frames outside the allocators)"

        def table(title, items):
            total = sum(size for size, _ in items)
            say()
            say(f"== {title}: {total / 2**20:.1f} MiB in {len(items)}")
            for depth, top in ((1, 40), (4, 30)):
                by = collections.defaultdict(lambda: [0, 0])
                for size, sid in items:
                    row = by[caller(frames_of[sid], depth)]
                    row[0] += size
                    row[1] += 1
                say(f"-- by the first {depth} frame(s) outside the allocators")
                for key, (size, n) in sorted(by.items(), key=lambda kv: -kv[1][0])[:top]:
                    say(f"{size / 2**20:9.2f} MiB {n:8d}  {key}")
            by_module = collections.defaultdict(int)
            for size, sid in items:
                first = caller(frames_of[sid], 1)
                by_module[first.split("!")[0]] += size
            say("-- by the module of the first frame outside the allocators")
            for module, size in sorted(by_module.items(), key=lambda kv: -kv[1])[:20]:
                say(f"{size / 2**20:9.2f} MiB  {module}")

        table("heap blocks still allocated", list(blocks.values()))
        table("pages still committed by VirtualAlloc",
              [(end - base, sid) for base, (end, sid) in commits.items()])


if __name__ == "__main__":
    main(*sys.argv[1:4])
