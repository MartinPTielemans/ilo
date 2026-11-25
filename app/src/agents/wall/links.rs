//! The links a task left in its granted folder that lead out of it, and the user's Delete Links
//! (#4101). Both read the folder's directories and its links and nothing else: no file is opened,
//! no link is followed, and nothing outside the folder is read. A link's target is resolved by
//! reading the chain of links it names, from the folder down. What the walk couldn't read it says,
//! so it never answers "nothing found" for a folder it didn't wholly see.
//!
//! A link is the task's by its own change time alone. A folder of the user's that the task moves
//! within the granted folder keeps its links' times, so a relative link in it that now leads out
//! isn't listed: a known gap, and nothing follows such a link out for a task either way.

use std::ffi::{CStr, CString, OsStr, OsString};
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{Granted, LeavingLink, Removed, Stop, Walked, Wall};

#[derive(Clone, Copy, Debug)]
pub(super) struct Limits {
    /// Entries read before the walk stops, so a huge folder can't hold the task's end.
    pub entries: usize,
    pub time: Duration,
    /// Folders held open at once, so a deep folder can't use up the app's handles: one deeper
    /// isn't read, and counts as unread.
    pub depth: usize,
}

pub(super) const LIMITS: Limits = Limits {
    entries: 1_000_000,
    time: Duration::from_secs(30),
    depth: 64,
};

/// How far before the task's start a link's change time still counts as the task's on the folder's
/// filesystem, or None where its change times can't tell the task's links from the user's.
type Trust = fn(&OwnedFd) -> Option<Duration>;

/// A directory's names, with their kinds where the directory says them.
type Names = Vec<(OsString, Option<Kind>)>;

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

/// A folder on the way down, with a handle held to it.
struct Open {
    dir: OwnedFd,
    at: Vec<OsString>,
    names: std::vec::IntoIter<(OsString, Option<Kind>)>,
    /// Already counted as unread, for an entry in it the walk couldn't look at.
    unread: bool,
}

impl Open {
    fn not_wholly_read(&mut self, walked: &mut Walked) {
        if !self.unread {
            self.unread = true;
            count_unread(walked, &self.at);
        }
    }
}

fn count_unread(walked: &mut Walked, at: &[OsString]) {
    walked.unread += 1;
    if walked.first_unread.is_none() {
        walked.first_unread = Some(at.iter().collect());
    }
}

pub(super) fn outside(wall: &Wall, limits: Limits) -> Walked {
    #[cfg(target_os = "macos")]
    let _policy = dataless::left_in_the_cloud();
    outside_with(wall, limits, filesystem_slack)
}

fn outside_with(wall: &Wall, limits: Limits, trust: Trust) -> Walked {
    let mut walked = Walked::default();
    let Some(granted) = wall.folder.as_ref() else {
        return walked;
    };
    let root_path = granted.root();
    let opened = (|| -> io::Result<_> {
        let (root, walk_root) = (open_granted(granted)?, open_granted(granted)?);
        Ok((
            stat_of(&root)?,
            entries(&walk_root, limits.entries)?,
            root,
            walk_root,
        ))
    })();
    let Ok((root_stat, (names, mut more), root, walk_root)) = opened else {
        walked.root_unread = true;
        return walked;
    };
    let slack = trust(&root);
    walked.every_leaving = slack.is_none();
    let since = since(wall, slack);
    let began = Instant::now();
    let mut seen = 0;
    // Names read so far, each within the budget: no directory is read past what's left of it.
    let mut read = names.len();

    // Depth first, holding a handle only to each directory on the way down.
    let mut open = vec![Open {
        dir: walk_root,
        at: Vec::new(),
        names: names.into_iter(),
        unread: false,
    }];
    while let Some(top) = open.last_mut() {
        let Some((name, kind)) = top.names.next() else {
            open.pop();
            continue;
        };
        seen += 1;
        if seen % 256 == 0 && began.elapsed() >= limits.time {
            walked.stopped = Some(Stop::Time);
            break;
        }
        let Ok(c_name) = c_string(&name) else {
            top.not_wholly_read(&mut walked);
            continue;
        };
        let dir = top.dir.as_raw_fd();
        let kind = match kind
            .map(Ok)
            .unwrap_or_else(|| lstat_at(dir, &c_name).map(|s| kind_of(&s)))
        {
            Ok(kind) => kind,
            Err(error) if gone(&error) => continue,
            Err(_) => {
                top.not_wholly_read(&mut walked);
                continue;
            }
        };
        match kind {
            Kind::Dir => {
                let mut at = top.at.clone();
                at.push(name);
                if open.len() >= limits.depth {
                    count_unread(&mut walked, &at);
                    continue;
                }
                let sub = open_dir(dir, &c_name).and_then(|sub| Ok((stat_of(&sub)?, sub)));
                let names = match sub {
                    Err(error) if gone(&error) => continue,
                    Ok((stat, sub)) if same_device(&root_stat, &stat) => {
                        entries(&sub, limits.entries.saturating_sub(read)).map(|names| (sub, names))
                    }
                    _ => Err(io::Error::from(io::ErrorKind::Unsupported)),
                };
                let Ok((sub, (names, past))) = names else {
                    count_unread(&mut walked, &at);
                    continue;
                };
                read += names.len();
                more |= past;
                open.push(Open {
                    dir: sub,
                    at,
                    names: names.into_iter(),
                    unread: false,
                });
            }
            Kind::Link => {
                let stat = match lstat_at(dir, &c_name) {
                    Ok(stat) => stat,
                    Err(error) if gone(&error) => continue,
                    Err(_) => {
                        top.not_wholly_read(&mut walked);
                        continue;
                    }
                };
                if kind_of(&stat) != Kind::Link || (!walked.every_leaving && changed(&stat) < since)
                {
                    continue;
                }
                let target = match read_link_at(dir, &c_name) {
                    Ok(target) => target,
                    Err(error) if gone(&error) => continue,
                    Err(_) => {
                        top.not_wholly_read(&mut walked);
                        continue;
                    }
                };
                if leads(&root, root_path, &top.at, &target) == Leads::Outside {
                    let path: PathBuf = top.at.iter().chain([&name]).collect();
                    walked.links.push(LeavingLink { path, target });
                }
            }
            Kind::Other => {}
        }
    }
    // Every name read was walked, and some were left unread past the budget.
    if more && walked.stopped.is_none() {
        walked.stopped = Some(Stop::Entries);
    }
    walked.links.sort_by(|a, b| a.path.cmp(&b.path));
    walked
}

/// Removed between the directory's listing and the look at it: nothing there to read.
fn gone(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ENOENT)
}

/// Whether the walk goes into a folder it opened: not one on another device, a disk or share
/// mounted inside the folder, whose change times and links are another filesystem's.
fn same_device(root: &libc::stat, folder: &libc::stat) -> bool {
    root.st_dev == folder.st_dev
}

fn since(wall: &Wall, slack: Option<Duration>) -> SystemTime {
    let slack = slack.unwrap_or(Duration::ZERO);
    wall.started().checked_sub(slack).unwrap_or(UNIX_EPOCH)
}

/// Between its checks and the unlink, an entry swapped in under the same name in the same folder
/// would be removed in the link's place: never a folder, and never anything outside the folder. A
/// touch of the user's own link during the task, a change of its mode or times or a hard link to
/// it, makes it the task's by its change time; Delete then removes that link, never its target.
pub(super) fn remove(wall: &Wall, links: &[LeavingLink]) -> Removed {
    #[cfg(target_os = "macos")]
    let _policy = dataless::left_in_the_cloud();
    remove_with(wall, links, filesystem_slack)
}

fn remove_with(wall: &Wall, links: &[LeavingLink], trust: Trust) -> Removed {
    let root = wall.folder.as_ref().and_then(|granted| {
        let (path, root) = (granted.root(), open_granted(granted).ok()?);
        // A list of every leaving link is the user's own links too, so it deletes none.
        let slack = trust(&root)?;
        Some((path, root, since(wall, Some(slack))))
    });
    let mut removed = Removed::default();
    for link in links {
        let gone = root
            .as_ref()
            .is_some_and(|(path, root, since)| remove_one(root, path, *since, link).is_ok());
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

/// The folder's filesystem's slack, by its name on the Mac and its number on Linux; any other
/// system trusts none.
fn filesystem_slack(root: &OwnedFd) -> Option<Duration> {
    let mut stat = MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: an open handle, and room for the answer.
    if unsafe { libc::fstatfs(root.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: fstatfs succeeded, so it filled the answer.
    let stat = unsafe { stat.assume_init() };
    #[cfg(target_os = "macos")]
    {
        // SAFETY: the system ends the name with a NUL.
        let name = unsafe { CStr::from_ptr(stat.f_fstypename.as_ptr()) };
        slack_by_name(name.to_bytes())
    }
    #[cfg(target_os = "linux")]
    {
        #[allow(clippy::unnecessary_cast)]
        slack_by_number(stat.f_type as u32)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = stat;
        None
    }
}

/// The Mac's: APFS keeps a link's change time in nanoseconds by the clock the app reads, so a
/// tenth of a second covers it. HFS+ keeps whole seconds, so a link the user made in the second
/// before the task can't be told from the task's, and it isn't trusted; nor is a shared,
/// removable or user-space filesystem.
#[cfg(any(target_os = "macos", test))]
fn slack_by_name(name: &[u8]) -> Option<Duration> {
    (name == b"apfs").then_some(Duration::from_millis(100))
}

/// Linux's: ext2/3/4, btrfs, xfs, tmpfs, f2fs, zfs, overlay and bcachefs, never a shared or
/// user-space one. An ext4 with small inodes keeps whole seconds and statfs can't tell it from
/// one with large, so a second covers it, and the kernel's coarse clock with it.
#[cfg(target_os = "linux")]
fn slack_by_number(number: u32) -> Option<Duration> {
    // Not in libc.
    const ZFS: u32 = 0x2FC1_2FC1;
    const BCACHEFS: u32 = 0xCA45_1A4E;
    const EXT2_OLD: u32 = 0xEF51;
    #[allow(clippy::unnecessary_cast)]
    const TRUSTED: [u32; 9] = [
        libc::EXT4_SUPER_MAGIC as u32,
        libc::BTRFS_SUPER_MAGIC as u32,
        libc::XFS_SUPER_MAGIC as u32,
        libc::TMPFS_MAGIC as u32,
        libc::F2FS_SUPER_MAGIC as u32,
        libc::OVERLAYFS_SUPER_MAGIC as u32,
        ZFS,
        BCACHEFS,
        EXT2_OLD,
    ];
    TRUSTED.contains(&number).then_some(Duration::from_secs(1))
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

/// The granted folder, opened at the path resolved at the user's pick, as a directory and never
/// through a link, and only if what it opened is the folder they picked: one put in its place since
/// is another folder, unread and untouched.
fn open_granted(granted: &Granted) -> io::Result<OwnedFd> {
    let path = c_string(granted.root().as_os_str())?;
    // SAFETY: a NUL-terminated path; the handle is owned from here.
    let fd = unsafe { libc::open(path.as_ptr(), DIR_FLAGS) };
    let root = owned(fd)?;
    match granted.is_open(root.as_fd()) {
        true => Ok(root),
        false => Err(io::Error::other("not the folder granted")),
    }
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

/// A directory's names from its start: at most `budget` of them, so one directory of tens of
/// millions of names is never read whole, and whether it has more.
fn entries(dir: &OwnedFd, budget: usize) -> io::Result<(Names, bool)> {
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
    // The copy shares the handle's place in the directory, which a read stopped at the budget
    // leaves part way.
    // SAFETY: an open stream.
    unsafe { libc::rewinddir(stream) };
    let mut names = Vec::new();
    let mut more = false;
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
        if names.len() == budget {
            more = true;
            break Ok(());
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
    read.map(|()| (names, more))
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

fn stat_of(fd: &OwnedFd) -> io::Result<libc::stat> {
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: an open handle and room for the answer.
    if unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fstat succeeded, so it filled the answer.
    Ok(unsafe { stat.assume_init() })
}

/// While the walk runs, the Mac doesn't fetch a folder's contents that are only in the cloud just
/// to list them: such a folder won't open then, and counts as unread. Unmeasured: no Mac run has
/// walked one yet.
#[cfg(target_os = "macos")]
mod dataless {
    use libc::c_int;

    // From the SDK's sys/resource.h, which libc doesn't bind.
    const MATERIALIZE_DATALESS_FILES: c_int = 3;
    const THREAD: c_int = 1;
    const OFF: c_int = 1;

    unsafe extern "C" {
        fn getiopolicy_np(iotype: c_int, scope: c_int) -> c_int;
        fn setiopolicy_np(iotype: c_int, scope: c_int, policy: c_int) -> c_int;
    }

    /// The walking thread's policy as it was, put back as the walk ends.
    pub(super) struct LeftInTheCloud(Option<c_int>);

    pub(super) fn left_in_the_cloud() -> LeftInTheCloud {
        // SAFETY: the calling thread's own policy.
        let was = unsafe { getiopolicy_np(MATERIALIZE_DATALESS_FILES, THREAD) };
        // SAFETY: as above.
        let set =
            was >= 0 && unsafe { setiopolicy_np(MATERIALIZE_DATALESS_FILES, THREAD, OFF) } == 0;
        LeftInTheCloud(set.then_some(was))
    }

    impl Drop for LeftInTheCloud {
        fn drop(&mut self) {
            if let Some(was) = self.0 {
                // SAFETY: the calling thread's own policy, as it read before the walk.
                unsafe { setiopolicy_np(MATERIALIZE_DATALESS_FILES, THREAD, was) };
            }
        }
    }
}

#[cfg(test)]
mod tests;
