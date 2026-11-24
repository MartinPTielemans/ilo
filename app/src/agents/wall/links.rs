//! The links a task left in its granted folder that lead out of it, and the user's Delete Links
//! (#4101). Both read the folder's directories and its links and nothing else: no file is opened,
//! no link is followed, and nothing outside the folder is read. A link's target is resolved by
//! reading the chain of links it names, from the folder down.

use std::ffi::{CStr, CString, OsStr, OsString};
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{LeavingLink, Removed, Stop, Walked, Wall};

#[derive(Clone, Copy, Debug)]
pub(super) struct Limits {
    /// Entries read before the walk stops, so a huge folder can't hold the task's end.
    pub entries: usize,
    pub time: Duration,
    /// How far before the task's start a link's change time still counts as the task's: a
    /// filesystem keeps it by a coarse clock, a jiffy behind on Linux and to the second on HFS+.
    pub slack: Duration,
}

pub(super) const LIMITS: Limits = Limits {
    entries: 1_000_000,
    time: Duration::from_secs(30),
    slack: Duration::from_secs(1),
};

/// Links read through before a chain counts as a loop: the kernel's own limit.
const HOPS: usize = 40;

/// Where a link leads, by the chain of links it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leads {
    Inside,
    Outside,
    /// A loop, or a path through something that isn't a directory: it reaches nothing.
    Nowhere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Dir,
    Link,
    Other,
}

pub(super) fn outside(wall: &Wall, limits: Limits) -> Walked {
    let mut walked = Walked::default();
    let Some(root_path) = wall.folder() else {
        return walked;
    };
    let (Ok(root), Ok(walk_root)) = (open_root(root_path), open_root(root_path)) else {
        return walked;
    };
    walked.every_leaving = !change_times_trusted(&root);
    let since = wall
        .started()
        .checked_sub(limits.slack)
        .unwrap_or(UNIX_EPOCH);
    let began = Instant::now();
    let mut seen = 0;

    // Depth first, holding a handle only to each directory on the way down.
    struct Open {
        dir: OwnedFd,
        at: Vec<OsString>,
        names: std::vec::IntoIter<(OsString, Option<Kind>)>,
    }
    let Ok(names) = entries(&walk_root) else {
        return walked;
    };
    let mut open = vec![Open {
        dir: walk_root,
        at: Vec::new(),
        names: names.into_iter(),
    }];
    while let Some(top) = open.last_mut() {
        let Some((name, kind)) = top.names.next() else {
            open.pop();
            continue;
        };
        seen += 1;
        if seen > limits.entries {
            walked.stopped = Some(Stop::Entries);
            break;
        }
        if seen % 256 == 0 && began.elapsed() >= limits.time {
            walked.stopped = Some(Stop::Time);
            break;
        }
        let Ok(c_name) = c_string(&name) else {
            continue;
        };
        let dir = top.dir.as_raw_fd();
        let kind = match kind {
            Some(kind) => kind,
            None => match lstat_at(dir, &c_name) {
                Ok(stat) => kind_of(&stat),
                Err(_) => continue,
            },
        };
        match kind {
            Kind::Dir => {
                let Ok(sub) = open_dir(dir, &c_name) else {
                    continue;
                };
                let Ok(names) = entries(&sub) else {
                    continue;
                };
                let mut at = top.at.clone();
                at.push(name);
                open.push(Open {
                    dir: sub,
                    at,
                    names: names.into_iter(),
                });
            }
            Kind::Link => {
                let Ok(stat) = lstat_at(dir, &c_name) else {
                    continue;
                };
                if kind_of(&stat) != Kind::Link || (!walked.every_leaving && changed(&stat) < since)
                {
                    continue;
                }
                let Ok(target) = read_link_at(dir, &c_name) else {
                    continue;
                };
                if leads(&root, root_path, &top.at, &target) == Leads::Outside {
                    let path: PathBuf = top.at.iter().chain([&name]).collect();
                    walked.links.push(LeavingLink { path, target });
                }
            }
            Kind::Other => {}
        }
    }
    walked.links.sort_by(|a, b| a.path.cmp(&b.path));
    walked
}

pub(super) fn remove(wall: &Wall, links: &[LeavingLink], limits: Limits) -> Removed {
    let root = wall
        .folder()
        .and_then(|path| Some((path, open_root(path).ok()?)))
        // A list of every leaving link is the user's own links too, so it deletes none.
        .filter(|(_, root)| change_times_trusted(root));
    let since = wall
        .started()
        .checked_sub(limits.slack)
        .unwrap_or(UNIX_EPOCH);
    let mut removed = Removed::default();
    for link in links {
        let gone = root
            .as_ref()
            .is_some_and(|(path, root)| remove_one(root, path, since, link).is_ok());
        match gone {
            true => removed.removed += 1,
            false => removed.left += 1,
        }
    }
    removed
}

/// Removes `link` if it's still a link made since `since`, with the target the walk read, still
/// leading out: from its parent's handle, opened from the folder down without following any link.
fn remove_one(
    root: &OwnedFd,
    root_path: &Path,
    since: SystemTime,
    link: &LeavingLink,
) -> io::Result<()> {
    let changed_since = || io::Error::other("changed since the walk");
    let mut parts = Vec::new();
    for part in link.path.components() {
        match part {
            Component::Normal(name) => parts.push(name.to_os_string()),
            _ => return Err(changed_since()),
        }
    }
    let name = c_string(&parts.pop().ok_or_else(changed_since)?)?;
    let mut parent: Option<OwnedFd> = None;
    for part in &parts {
        let at = parent.as_ref().unwrap_or(root).as_raw_fd();
        parent = Some(open_dir(at, &c_string(part)?)?);
    }
    let dir = parent.as_ref().unwrap_or(root).as_raw_fd();
    let stat = lstat_at(dir, &name)?;
    if kind_of(&stat) != Kind::Link
        || changed(&stat) < since
        || read_link_at(dir, &name)? != link.target
        || leads(root, root_path, &parts, &link.target) != Leads::Outside
    {
        return Err(changed_since());
    }
    // SAFETY: a directory handle and a NUL-terminated name.
    if unsafe { libc::unlinkat(dir, name.as_ptr(), 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Where a link in the directory `dir` (its parts under the folder, each a real directory) whose
/// target is `target` leads: each part of the way is looked at with lstat from the folder's handle,
/// and a link on the way is read and taken in its place. The way leaves the folder as soon as it
/// climbs above it or names a place outside it; nothing outside is looked at.
fn leads(root: &OwnedFd, root_path: &Path, dir: &[OsString], target: &Path) -> Leads {
    let mut at = dir.to_vec();
    // The parts still to take, the next one last.
    let mut rest: Vec<OsString> = Vec::new();
    let mut next = target.to_path_buf();
    let mut hops = 0;
    loop {
        if next.is_absolute() {
            match next.strip_prefix(root_path) {
                Ok(inside) => {
                    at.clear();
                    next = inside.to_path_buf();
                }
                Err(_) => return Leads::Outside,
            }
        }
        let parts = next.components().filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_os_string()),
            Component::ParentDir => Some(OsString::from("..")),
            _ => None,
        });
        let parts: Vec<OsString> = parts.collect();
        rest.extend(parts.into_iter().rev());
        loop {
            let Some(part) = rest.pop() else {
                return Leads::Inside;
            };
            if part == ".." {
                if at.pop().is_none() {
                    return Leads::Outside;
                }
                continue;
            }
            at.push(part);
            let Ok(here) = c_string(at.iter().collect::<PathBuf>().as_os_str()) else {
                return Leads::Nowhere;
            };
            match lstat_at(root.as_raw_fd(), &here).map(|stat| kind_of(&stat)) {
                // Nothing there to look at, so the rest of the way is taken by its parts alone.
                Err(_) => return by_parts(at.len(), &rest),
                Ok(Kind::Dir) => {}
                Ok(Kind::Other) if rest.is_empty() => return Leads::Inside,
                Ok(Kind::Other) => return Leads::Nowhere,
                Ok(Kind::Link) => {
                    hops += 1;
                    if hops > HOPS {
                        return Leads::Nowhere;
                    }
                    let Ok(target) = read_link_at(root.as_raw_fd(), &here) else {
                        return Leads::Nowhere;
                    };
                    at.pop();
                    next = target;
                    break;
                }
            }
        }
    }
}

/// The rest of a way that reached a part not there, `depth` parts under the folder.
fn by_parts(mut depth: usize, rest: &[OsString]) -> Leads {
    for part in rest.iter().rev() {
        if part == ".." {
            let Some(up) = depth.checked_sub(1) else {
                return Leads::Outside;
            };
            depth = up;
        } else {
            depth += 1;
        }
    }
    Leads::Inside
}

/// Whether the folder's filesystem keeps a link's own change time, so the task's links can be told
/// from the user's: the system's own disks, never a shared, removable or user-space one.
fn change_times_trusted(root: &OwnedFd) -> bool {
    let mut stat = MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: an open handle, and room for the answer.
    if unsafe { libc::fstatfs(root.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return false;
    }
    // SAFETY: fstatfs succeeded, so it filled the answer.
    let stat = unsafe { stat.assume_init() };
    #[cfg(target_os = "macos")]
    {
        // SAFETY: the system ends the name with a NUL.
        let name = unsafe { CStr::from_ptr(stat.f_fstypename.as_ptr()) };
        matches!(name.to_bytes(), b"apfs" | b"hfs")
    }
    #[cfg(not(target_os = "macos"))]
    {
        // ext2/3/4, btrfs, xfs, tmpfs, f2fs, zfs, overlay, bcachefs.
        const TRUSTED: [u32; 9] = [
            0xEF53,
            0x9123_683E,
            0x5846_5342,
            0x0102_1994,
            0xF2F5_2010,
            0x2FC1_2FC1,
            0x794C_7630,
            0xCA45_1A4E,
            0x0000_EF51,
        ];
        #[allow(clippy::unnecessary_cast)]
        TRUSTED.contains(&(stat.f_type as u32))
    }
}

fn changed(stat: &libc::stat) -> SystemTime {
    let (Ok(secs), Ok(nanos)) = (
        u64::try_from(stat.st_ctime),
        u32::try_from(stat.st_ctime_nsec),
    ) else {
        return UNIX_EPOCH;
    };
    UNIX_EPOCH + Duration::new(secs, nanos)
}

fn kind_of(stat: &libc::stat) -> Kind {
    match stat.st_mode & libc::S_IFMT {
        libc::S_IFDIR => Kind::Dir,
        libc::S_IFLNK => Kind::Link,
        _ => Kind::Other,
    }
}

fn c_string(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

const DIR_FLAGS: libc::c_int =
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;

/// The granted folder, resolved at the user's pick, opened as a directory and never through a link.
fn open_root(path: &Path) -> io::Result<OwnedFd> {
    let path = c_string(path.as_os_str())?;
    // SAFETY: a NUL-terminated path; the handle is owned from here.
    let fd = unsafe { libc::open(path.as_ptr(), DIR_FLAGS) };
    owned(fd)
}

/// The directory `name` in `dir`, opened as one: a link or anything else there is refused.
fn open_dir(dir: RawFd, name: &CStr) -> io::Result<OwnedFd> {
    // SAFETY: a directory handle and a NUL-terminated name; the handle is owned from here.
    let fd = unsafe { libc::openat(dir, name.as_ptr(), DIR_FLAGS) };
    owned(fd)
}

fn owned(fd: RawFd) -> io::Result<OwnedFd> {
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a handle just opened, owned by nothing else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// A directory's names, with their kinds where the directory says them.
fn entries(dir: &OwnedFd) -> io::Result<Vec<(OsString, Option<Kind>)>> {
    // The stream takes a handle of its own, closed with it.
    // SAFETY: an open handle.
    let fd = unsafe { libc::fcntl(dir.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a handle of our own, which the stream owns from here.
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        let error = io::Error::last_os_error();
        // SAFETY: the stream didn't take it.
        unsafe { libc::close(fd) };
        return Err(error);
    }
    let mut names = Vec::new();
    let read = loop {
        clear_errno();
        // SAFETY: an open stream.
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            break match error.raw_os_error() {
                Some(0) | None => Ok(()),
                Some(_) => Err(error),
            };
        }
        // SAFETY: readdir's entry, valid until the next call.
        let entry = unsafe { &*entry };
        // SAFETY: the system ends the name with a NUL.
        let name = unsafe { CStr::from_ptr(entry.d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        let kind = match entry.d_type {
            libc::DT_DIR => Some(Kind::Dir),
            libc::DT_LNK => Some(Kind::Link),
            libc::DT_UNKNOWN => None,
            _ => Some(Kind::Other),
        };
        names.push((OsString::from_vec(name.to_vec()), kind));
    };
    // SAFETY: an open stream, closed once.
    unsafe { libc::closedir(stream) };
    read.map(|()| names)
}

fn clear_errno() {
    // SAFETY: the calling thread's errno.
    #[cfg(target_os = "linux")]
    unsafe {
        *libc::__errno_location() = 0;
    }
    // SAFETY: the calling thread's errno.
    #[cfg(target_os = "macos")]
    unsafe {
        *libc::__error() = 0;
    }
}

fn lstat_at(dir: RawFd, name: &CStr) -> io::Result<libc::stat> {
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: a directory handle, a NUL-terminated name and room for the answer.
    let done = unsafe {
        libc::fstatat(
            dir,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if done != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fstatat succeeded, so it filled the answer.
    Ok(unsafe { stat.assume_init() })
}

fn read_link_at(dir: RawFd, name: &CStr) -> io::Result<PathBuf> {
    let mut target = vec![0u8; libc::PATH_MAX as usize + 1];
    // SAFETY: a directory handle, a NUL-terminated name and a buffer of its length.
    let read =
        unsafe { libc::readlinkat(dir, name.as_ptr(), target.as_mut_ptr().cast(), target.len()) };
    let Ok(read) = usize::try_from(read) else {
        return Err(io::Error::last_os_error());
    };
    if read == target.len() {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    target.truncate(read);
    Ok(PathBuf::from(OsString::from_vec(target)))
}

#[cfg(test)]
mod tests;
