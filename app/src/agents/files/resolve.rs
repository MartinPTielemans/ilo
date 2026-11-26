//! The path floor (road B): a path the assistant gave, resolved one name at a time from a handle on
//! `/`, the system never following a link for us, into the real path and handles held on what it
//! names. What's judged, asked about and acted on is that real path and those handles, never the
//! assistant's string, so a link or a name swapped in after the resolve reaches nothing it didn't
//! name.

use std::collections::{HashSet, VecDeque};
use std::ffi::{CStr, CString, OsStr, OsString};
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};

/// Links followed in one resolve before it's refused as a loop, as the systems' own limit is.
pub const MOST_LINKS: usize = 32;

const DIR_FLAGS: libc::c_int =
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;

/// A file's or folder's identity on its disk, the same by any of its names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id {
    device: u64,
    inode: u64,
}

impl Id {
    fn of(stat: &libc::stat) -> Id {
        Id {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Folder,
}

/// What the resolve holds open at the path's end.
#[derive(Debug)]
pub struct Held {
    pub fd: OwnedFd,
    pub id: Id,
    pub kind: Kind,
    /// A file with another name, which is read and never changed.
    pub other_names: bool,
}

/// A path resolved: where it really is, and handles on it and the folder holding it.
pub struct Resolved {
    pub real: PathBuf,
    /// The folder holding it, held open, where a Make makes and a Move moves from.
    pub parent: OwnedFd,
    /// Its name in `parent`; empty for `/`.
    pub name: OsString,
    /// What's there, or None for a name nothing has yet.
    pub target: Option<Held>,
    /// Whether a link was followed on the way, so the ask names where it lands.
    pub through_link: bool,
    /// Whether the walk passed a never root by its identity, whatever name it went by.
    pub never: bool,
}

impl std::fmt::Debug for Resolved {
    /// Never the path (#3656).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolved")
            .field("target", &self.target.as_ref().map(|held| held.kind))
            .field("through_link", &self.through_link)
            .field("never", &self.never)
            .finish_non_exhaustive()
    }
}

impl Resolved {
    /// Whether its name still names what the resolve holds, by the held folder: false once a file
    /// was put in its place (F2), or one was made where nothing was.
    pub fn still_named(&self) -> bool {
        let Ok(name) = c_string(&self.name) else {
            return false;
        };
        match (stat_at(self.parent.as_raw_fd(), &name), &self.target) {
            (Ok(stat), Some(held)) => Id::of(&stat) == held.id,
            (Err(error), None) => error.kind() == io::ErrorKind::NotFound,
            _ => false,
        }
    }
}

/// Why a path didn't resolve.
#[derive(Debug, PartialEq, Eq)]
pub enum Unresolved {
    /// Not from `/`: the caller joins a relative path to the thread's folder first.
    Relative,
    /// More than `MOST_LINKS` links, or a name swapped for a link as often.
    Loop,
    /// A folder on the way isn't there.
    Gone,
    /// A file where the path goes on as if it were a folder.
    NotAFolder,
    /// Neither a file nor a folder: a device, a pipe or a socket, never opened.
    NotAFile,
    Io(io::ErrorKind),
}

impl From<io::Error> for Unresolved {
    fn from(error: io::Error) -> Unresolved {
        Unresolved::Io(error.kind())
    }
}

/// What the end is opened for: reading, or a change written over it in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum For {
    Read,
    Change,
}

/// The never roots' identities, taken once as a thread starts (`Never::by_identity`).
#[derive(Default)]
pub struct NeverIds(HashSet<Id>);

impl NeverIds {
    pub fn of(roots: &[PathBuf]) -> NeverIds {
        let ids = roots.iter().filter_map(|root| {
            let path = c_string(root.as_os_str()).ok()?;
            let mut stat = MaybeUninit::<libc::stat>::uninit();
            // SAFETY: a NUL-terminated path and room for the answer. A root that is a link is
            // known by where it leads, which the walk reaches by its real name.
            let found = unsafe { libc::stat(path.as_ptr(), stat.as_mut_ptr()) } == 0;
            // SAFETY: stat succeeded, so it filled the answer.
            found.then(|| Id::of(&unsafe { stat.assume_init() }))
        });
        NeverIds(ids.collect())
    }
}

/// `path` resolved from `/` by its own names and links, with its end opened `want`.
pub fn resolve(path: &Path, want: For, never: &NeverIds) -> Result<Resolved, Unresolved> {
    if !path.is_absolute() {
        return Err(Unresolved::Relative);
    }
    #[cfg(target_os = "macos")]
    let _policy = dataless::left_in_the_cloud();
    let root = open_root()?;
    let mut never_seen = never.0.contains(&Id::of(&stat_of(&root)?));
    let mut stack: Vec<(OwnedFd, OsString)> = vec![(root, OsString::new())];
    let mut pending: VecDeque<OsString> = names(path).collect();
    let (mut links, mut through_link) = (0, false);
    while let Some(name) = pending.pop_front() {
        if name == ".." {
            if stack.len() > 1 {
                stack.pop();
            }
            continue;
        }
        let last = pending.is_empty();
        let dir = stack.last().expect("the root stays").0.as_raw_fd();
        let c_name = c_string(&name)?;
        let found = match stat_at(dir, &c_name) {
            Ok(stat) => stat,
            Err(error) if error.kind() == io::ErrorKind::NotFound && last => {
                return Ok(ended(stack, name, None, through_link, never_seen));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(Unresolved::Gone);
            }
            Err(error) => return Err(error.into()),
        };
        if found.st_mode & libc::S_IFMT == libc::S_IFLNK {
            links += 1;
            if links > MOST_LINKS {
                return Err(Unresolved::Loop);
            }
            match read_link_at(dir, &c_name) {
                Ok(to) => {
                    let to = PathBuf::from(to);
                    if to.is_absolute() {
                        stack.truncate(1);
                    }
                    for each in names(&to).collect::<Vec<_>>().into_iter().rev() {
                        pending.push_front(each);
                    }
                    through_link = true;
                }
                // No longer a link: looked at again.
                Err(_) => pending.push_front(name),
            }
            continue;
        }
        let kind = kind_of(&found);
        if last && kind.is_none() {
            return Err(Unresolved::NotAFile);
        }
        let fd = match open_at(dir, &c_name, last, want) {
            Ok(fd) => fd,
            // A link was put there since it was looked at: looked at again, as a link counts.
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                links += 1;
                if links > MOST_LINKS {
                    return Err(Unresolved::Loop);
                }
                pending.push_front(name);
                continue;
            }
            Err(error) if error.raw_os_error() == Some(libc::ENOTDIR) => {
                return Err(Unresolved::NotAFolder);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return match last {
                    true => Ok(ended(stack, name, None, through_link, never_seen)),
                    false => Err(Unresolved::Gone),
                };
            }
            Err(error) => return Err(error.into()),
        };
        // What was opened, by its handle: what the name held when looked at may have been swapped.
        let opened = stat_of(&fd)?;
        let id = Id::of(&opened);
        never_seen |= never.0.contains(&id);
        if !last {
            stack.push((fd, name));
            continue;
        }
        let Some(kind) = kind_of(&opened) else {
            return Err(Unresolved::NotAFile);
        };
        let held = Held {
            fd,
            id,
            kind,
            other_names: kind == Kind::File && opened.st_nlink > 1,
        };
        return Ok(ended(stack, name, Some(held), through_link, never_seen));
    }
    // The path ended on a folder already held: `/`, a `..`, or a link to a folder.
    let (top, name) = stack.pop().expect("the root stays");
    let parent = match stack.last() {
        Some((parent, _)) => parent.try_clone()?,
        None => top.try_clone()?,
    };
    let held = Held {
        id: Id::of(&stat_of(&top)?),
        fd: top,
        kind: Kind::Folder,
        other_names: false,
    };
    let real = real_of(&stack, &name);
    Ok(Resolved {
        real,
        parent,
        name,
        target: Some(held),
        through_link,
        never: never_seen,
    })
}

/// The answer for `name` in the folder on top of `stack`.
fn ended(
    mut stack: Vec<(OwnedFd, OsString)>,
    name: OsString,
    target: Option<Held>,
    through_link: bool,
    never: bool,
) -> Resolved {
    let real = real_of(&stack, &name);
    let (parent, _) = stack.pop().expect("the root stays");
    Resolved {
        real,
        parent,
        name,
        target,
        through_link,
        never,
    }
}

fn real_of(stack: &[(OwnedFd, OsString)], name: &OsStr) -> PathBuf {
    let mut real = PathBuf::from("/");
    real.extend(stack.iter().skip(1).map(|(_, each)| each));
    if !name.is_empty() {
        real.push(name);
    }
    real
}

/// A path's names, with `..` kept as itself for the walk and `.` and `/` dropped.
fn names(path: &Path) -> impl Iterator<Item = OsString> + '_ {
    path.components().filter_map(|part| match part {
        Component::Normal(name) => Some(name.to_os_string()),
        Component::ParentDir => Some(OsString::from("..")),
        Component::RootDir | Component::CurDir | Component::Prefix(_) => None,
    })
}

fn kind_of(stat: &libc::stat) -> Option<Kind> {
    match stat.st_mode & libc::S_IFMT {
        libc::S_IFREG => Some(Kind::File),
        libc::S_IFDIR => Some(Kind::Folder),
        _ => None,
    }
}

fn open_root() -> io::Result<OwnedFd> {
    // SAFETY: a NUL-terminated path; the handle is owned from here.
    owned(unsafe { libc::open(c"/".as_ptr(), DIR_FLAGS) })
}

/// `name` in `dir`, never through a link: a folder on the way as a folder, and the end as `want`
/// asks. The end never blocks on opening, and a folder there is opened as one.
fn open_at(dir: RawFd, name: &CStr, last: bool, want: For) -> io::Result<OwnedFd> {
    if !last {
        // SAFETY: a directory handle and a NUL-terminated name; the handle is owned from here.
        return owned(unsafe { libc::openat(dir, name.as_ptr(), DIR_FLAGS) });
    }
    let access = match want {
        For::Read => libc::O_RDONLY,
        For::Change => libc::O_RDWR,
    };
    let flags = access | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOCTTY;
    // SAFETY: as above.
    match owned(unsafe { libc::openat(dir, name.as_ptr(), flags) }) {
        Err(error) if error.raw_os_error() == Some(libc::EISDIR) => {
            // SAFETY: as above.
            owned(unsafe { libc::openat(dir, name.as_ptr(), DIR_FLAGS) })
        }
        opened => opened,
    }
}

fn stat_at(dir: RawFd, name: &CStr) -> io::Result<libc::stat> {
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

fn stat_of(fd: &OwnedFd) -> io::Result<libc::stat> {
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: an open handle and room for the answer.
    if unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fstat succeeded, so it filled the answer.
    Ok(unsafe { stat.assume_init() })
}

fn read_link_at(dir: RawFd, name: &CStr) -> io::Result<OsString> {
    let mut buffer = vec![0u8; libc::PATH_MAX as usize];
    // SAFETY: a directory handle, a NUL-terminated name and a buffer of the length given.
    let read =
        unsafe { libc::readlinkat(dir, name.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len()) };
    if read < 0 {
        return Err(io::Error::last_os_error());
    }
    buffer.truncate(read as usize);
    Ok(OsString::from_vec(buffer))
}

fn c_string(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

fn owned(fd: RawFd) -> io::Result<OwnedFd> {
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a handle just opened, owned by nothing else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// While a walk or a resolve runs, the Mac doesn't fetch a folder's or a file's contents that are
/// only in the cloud: such a folder won't open then, and counts as unread, and a read never
/// downloads a file. Unmeasured: no Mac run has
/// walked one yet.
#[cfg(target_os = "macos")]
pub(crate) mod dataless {
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
    pub(crate) struct LeftInTheCloud(Option<c_int>);

    pub(crate) fn left_in_the_cloud() -> LeftInTheCloud {
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
