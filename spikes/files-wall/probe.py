"""The stand-in assistant: tries each attempt it's given and says what the system answered.

It runs inside the wall wall.py puts around it, and does nothing but the attempts, so what it
reports is the wall's answer alone. Usage: probe.py ATTEMPTS.json, printing one JSON list.
"""

import errno
import json
import os
import sys


def read(path):
    with open(path, "rb") as f:
        f.read(1)


def search(path):
    os.listdir(path)


def make(path):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    os.write(fd, b"made")
    os.close(fd)


def make_dir(path):
    os.mkdir(path)


def change(path):
    fd = os.open(path, os.O_WRONLY | os.O_TRUNC)
    os.write(fd, b"changed")
    os.close(fd)


def append(path):
    fd = os.open(path, os.O_WRONLY | os.O_APPEND)
    os.write(fd, b"+")
    os.close(fd)


def truncate(path):
    os.truncate(path, 0)


def change_by_rename(path):
    """Writes a new copy beside the file and moves it over, as many editors save."""
    temp = path + ".new"
    make(temp)
    os.rename(temp, path)


def delete(path):
    os.unlink(path)


def delete_dir(path):
    os.rmdir(path)


def move(pair):
    os.rename(pair[0], pair[1])


def hard_link(pair):
    os.link(pair[0], pair[1])


def sym_link(pair):
    os.symlink(pair[0], pair[1])


def read_beneath_handle(pair):
    """Reads a file by name beneath a handle to its folder, opened only for its path."""
    folder = os.open(pair[0], getattr(os, "O_PATH", os.O_RDONLY) | os.O_DIRECTORY)
    try:
        fd = os.open(pair[1], os.O_RDONLY, dir_fd=folder)
        os.read(fd, 1)
        os.close(fd)
    finally:
        os.close(folder)


def handles():
    """The process's open files beyond its own three, as the launcher left them."""
    listed = "/proc/self/fd" if os.path.isdir("/proc/self/fd") else "/dev/fd"
    left = sorted(int(n) for n in os.listdir(listed) if int(n) > 2)
    # listdir's own handle is the last.
    if len(left) > 1:
        raise OSError(errno.EEXIST, "open handles: %s" % left[:-1])


DOES = {
    f.__name__: f
    for f in (read, search, make, make_dir, change, append, truncate, change_by_rename, delete,
              delete_dir, move, hard_link, sym_link, read_beneath_handle, handles)
}


def main():
    with open(sys.argv[1]) as f:
        attempts = json.load(f)
    said = []
    for attempt in attempts:
        try:
            if attempt["does"] == "handles":
                handles()
            else:
                DOES[attempt["does"]](attempt["at"])
            said.append("done")
        except OSError as e:
            said.append(errno.errorcode.get(e.errno, str(e.errno)))
    json.dump(said, sys.stdout)


if __name__ == "__main__":
    main()
