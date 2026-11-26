//! The skill file the command's Add writes for Claude Code and Codex (first run's step 6, and the
//! command's row in Settings › Other apps): one `SKILL.md` in the one folder each maker reads a
//! user's own skills from, which tells the agent when to run the command and to pass its own name.
//! It's written only at the user's click, never over a file the product didn't write, and the
//! command's Remove takes it away only while it's still byte for byte ours.
//!
//! Each step opens the next folder by name from the one before with O_NOFOLLOW and checks it's a
//! directory of the user's own, so no link anywhere under the home is followed, and a folder
//! swapped for a link between two steps is refused. Neither CLAUDE_CONFIG_DIR nor CODEX_HOME
//! moves either maker's folder.

use std::ops::{Deref, DerefMut};
use std::path::Path;
#[cfg(not(unix))]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde_json::{Value, json};

use super::clients::Client;

/// The programs a skill is written for, each into its maker's folder for a user's own skills.
pub const PROGRAMS: [Client; 2] = [Client::ClaudeCode, Client::Codex];

/// The folder in the home that holds the maker's `skills`, which the product never makes.
pub fn folder(program: Client) -> Option<&'static str> {
    match program {
        Client::ClaudeCode => Some(".claude"),
        Client::Codex => Some(".agents"),
        Client::Other | Client::Command => None,
    }
}

const SKILLS: &str = "skills";
const FILE: &str = "SKILL.md";

/// The most of a `SKILL.md` read to learn whether it's ours, far over the text's own size.
const READ_AT_MOST: u64 = 64 * 1024;

/// The file's text, `{command}` standing for the command's name.
const TEXT: &str = "---
name: {command}
description: Use the user's own browser from the command line with the {command} command. Use when a task needs a website the user is signed in to, a page a plain fetch can't read, or something done on a page: open it, read it, click, type.
---

`{command}` drives the user's browser in tabs of your own. The user watches them on a card with your name.

- Pass your own name on every call: `--as \"<the app you are>\"`, such as `--as \"Claude Code\"`. A name only; it changes nothing you may do.
- Run `{command} help` for the commands, and `{command} help <command>` for one.
- You can't type into a password field. At a sign-in, a code or a CAPTCHA, hand the tab to the user as help says and wait.
- If the browser says no to a step, don't try it another way. Say what you were about to do.
- When the task is finished, run `{command} done`.
";

/// The skill's text for the command `name`.
pub fn text(name: &str) -> String {
    TEXT.replace("{command}", name)
}

/// The skill's name and the command's: the link on the path, so the skill always names the
/// command an agent can run.
pub fn name() -> &'static str {
    super::cli::LINK
}

pub fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A name the skill's folder may have: a word of the command's own letters, never a path.
fn valid(name: &str) -> bool {
    !name.is_empty()
        && (name.bytes())
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// What the product keeps of a file it wrote, in its own data folder: which program's, under which
/// name, the file's hash, and whether the product made the `skills` folder too. No path is kept:
/// where the file is comes from the home, the program and the name each time, so an edited entry
/// can't point anywhere else. Which file was written, by its device and inode, is known only to
/// the run that wrote it, for its undo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub program: Client,
    pub name: String,
    pub sha256: String,
    pub made_skills: bool,
    pub written: Option<(u64, u64)>,
}

impl Kept {
    fn json(&self) -> Value {
        json!({
            "program": self.program.id(),
            "name": self.name,
            "sha256": self.sha256,
            "made_skills": self.made_skills,
        })
    }

    /// An entry read from disk, checked as input before any of it reaches a folder: one of the two
    /// programs, a name of the command's letters, a hash of the length one has. Else none.
    fn from_json(value: &Value) -> Option<Kept> {
        let program = Client::from_id(value.get("program")?.as_str()?)?;
        let name: String = value.get("name")?.as_str()?.into();
        let sha256: String = value.get("sha256")?.as_str()?.into();
        let hashed = sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit());
        if !PROGRAMS.contains(&program) || !valid(&name) || !hashed {
            return None;
        }
        Some(Kept {
            program,
            name,
            sha256,
            made_skills: value.get("made_skills")?.as_bool()?,
            written: None,
        })
    }
}

/// The most of the kept file read, far over two entries' size.
const KEPT_AT_MOST: u64 = 16 * 1024;

/// The kept entries in `file`. One that's missing, unreadable, larger than any list of ours or
/// not a list reads as none, so nothing is removed or written over for it.
pub fn read_kept(file: &Path) -> Vec<Kept> {
    use std::io::Read;
    let mut text = String::new();
    let read = std::fs::File::open(file)
        .and_then(|file| file.take(KEPT_AT_MOST + 1).read_to_string(&mut text));
    if read.is_err() || text.len() as u64 > KEPT_AT_MOST {
        return Vec::new();
    }
    let value: Value = serde_json::from_str(&text).unwrap_or_default();
    (value
        .get("skills")
        .and_then(Value::as_array)
        .into_iter()
        .flatten())
    .filter_map(Kept::from_json)
    .collect()
}

/// Keeps `kept` in `file`, only the user able to read it, written beside and renamed in; with
/// none, the file goes.
pub fn write_kept(file: &Path, kept: &[Kept]) -> std::io::Result<()> {
    if kept.is_empty() {
        return match std::fs::remove_file(file) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let text = json!({ "skills": kept.iter().map(Kept::json).collect::<Vec<_>>() });
    let beside = file.with_file_name(format!(".{}.{}", FILE_KEPT, random()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = (|| {
        use std::io::Write;
        let mut out = options.open(&beside)?;
        out.write_all(text.to_string().as_bytes())?;
        out.sync_all()?;
        std::fs::rename(&beside, file)
    })();
    if written.is_err() {
        _ = std::fs::remove_file(&beside);
    }
    written
}

/// The kept file's name in the product's data folder.
pub const FILE_KEPT: &str = "skills.json";

/// The kept skills, and whether the command is added, which the runs that bring the files to it
/// share: one at a time, under the lock, each looking at what the row is as it runs.
#[derive(Clone, Default)]
pub struct Shared {
    skills: Arc<Mutex<Skills>>,
    wanted: Arc<AtomicBool>,
}

impl Shared {
    /// The command was added, or taken away, at the user's click.
    pub fn want(&self, added: bool) {
        self.wanted.store(added, SeqCst);
    }
}

/// What was written, and how many runs have held the lock, so the UI shows the latest.
#[derive(Default)]
pub struct Skills {
    pub kept: Vec<Kept>,
    runs: u64,
    read: bool,
}

impl Skills {
    /// Waits for the lock, off the UI thread.
    pub fn lock(shared: &Shared) -> Lock<'_> {
        Lock {
            skills: (shared.skills.lock()).unwrap_or_else(PoisonError::into_inner),
            wanted: &shared.wanted,
        }
    }
}

/// The kept skills while a run holds them.
pub struct Lock<'a> {
    skills: MutexGuard<'a, Skills>,
    wanted: &'a AtomicBool,
}

impl Lock<'_> {
    /// Whether the command is added now.
    pub fn wanted(&self) -> bool {
        self.wanted.load(SeqCst)
    }

    /// Takes the kept list from `file` the first time a run holds the lock, so launch reads
    /// nothing.
    pub fn read_once(&mut self, file: Option<&Path>) {
        if !std::mem::replace(&mut self.skills.read, true) {
            if let Some(file) = file {
                self.skills.kept = read_kept(file);
            }
        }
    }

    /// Where each kept skill's file is by `look`, dropping each that's gone and keeping the list
    /// then; what the UI shows of this run, numbered after the runs before it.
    pub fn settle(
        &mut self,
        look: impl FnOnce(&[Kept]) -> Vec<Place>,
        keep: impl FnOnce(&[Kept]),
    ) -> (u64, Vec<(Kept, Place)>) {
        let places = look(&self.skills.kept);
        let shown: Vec<(Kept, Place)> = (self.skills.kept.iter().cloned())
            .zip(places)
            .filter(|(_, place)| *place != Place::Missing)
            .collect();
        if shown.len() != self.skills.kept.len() {
            self.skills.kept = shown.iter().map(|(kept, _)| kept.clone()).collect();
            keep(&self.skills.kept);
        }
        self.skills.runs += 1;
        (self.skills.runs, shown)
    }
}

impl Deref for Lock<'_> {
    type Target = Skills;
    fn deref(&self) -> &Skills {
        &self.skills
    }
}

impl DerefMut for Lock<'_> {
    fn deref_mut(&mut self) -> &mut Skills {
        &mut self.skills
    }
}

fn random() -> String {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("the system has no random numbers");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Why nothing was written. Each leaves the home as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The home isn't a directory of the user's own.
    Home,
    /// The maker's folder or its `skills` folder is missing, a link or not the user's.
    NoFolder,
    /// A skill of that name is there that the product didn't write.
    NotOurs,
    /// The file the product wrote was changed since.
    Changed,
    /// The system refused a step, or the build has no write that can't replace a file.
    Failed,
}

/// What a remove did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removed {
    /// The file and the skill's folder are gone, and `skills` if the product made it and it's
    /// empty.
    Gone,
    /// The file was changed since the product wrote it, so it stays, and its folder.
    Changed,
    /// Nothing of it was there to remove.
    Missing,
}

/// Where a kept skill's file is, as the remover's own walk finds it, read only: for the
/// command's row, when Settings › Other apps opens and after an Add or a Remove.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Byte for byte the file the product wrote.
    Ours,
    /// Changed since, or put in its place.
    Changed,
    /// Not there, or not reached by a walk that takes no link.
    Missing,
}

/// The file a kept skill's reveal shows, made again at the click from the home, the program and
/// the checked name, never from anything stored.
pub fn at(home: &Path, program: Client, name: &str) -> Option<std::path::PathBuf> {
    (home.is_absolute() && valid(name))
        .then(|| {
            Some(
                home.join(folder(program)?)
                    .join(SKILLS)
                    .join(name)
                    .join(FILE),
            )
        })
        .flatten()
}

/// The command's Add, at the user's click: takes away a kept skill under another name than the
/// command's now, as after the command took a new name, then writes the skill for each of
/// `programs` found. What to keep after: each written, and each kept one left as it was.
pub fn add(home: &Path, programs: &[Client], kept: &[Kept]) -> Vec<Kept> {
    let Some(home) = open(home) else {
        return kept.to_vec();
    };
    let home = &home;
    let name = name();
    let (renamed, kept): (Vec<&Kept>, Vec<&Kept>) = kept.iter().partition(|kept| kept.name != name);
    for old in renamed {
        remove_in(home, old);
    }
    let text = text(name);
    (PROGRAMS.into_iter())
        .filter_map(|program| {
            let ours = kept.iter().copied().find(|kept| kept.program == program);
            if !programs.contains(&program) {
                return ours.cloned();
            }
            match write_in(home, program, name, &text, ours) {
                Ok(written) => Some(written),
                // Changed by the user, it's theirs now, and no longer said to be left.
                Err(Refused::Changed) => None,
                Err(_) => ours.cloned(),
            }
        })
        .collect()
}

/// Keeps what an Add left with `keep`; where that fails, takes away with `undo` each file this
/// run wrote, by the inode it wrote, so no file of ours is ever on disk without its entry. What
/// to keep after.
pub fn kept_or_undone(
    after: Vec<Kept>,
    keep: impl FnOnce(&[Kept]) -> std::io::Result<()>,
    undo: impl FnOnce(&[Kept]),
) -> Vec<Kept> {
    if keep(&after).is_ok() {
        return after;
    }
    let (fresh, carried): (Vec<Kept>, Vec<Kept>) =
        after.into_iter().partition(|kept| kept.written.is_some());
    undo(&fresh);
    carried
}

/// The command's Remove: takes away each kept skill still ours, and says what each did. Those
/// changed stay kept, so the row can say so until they're gone or the command is added again.
pub fn remove_all(home: &Path, kept: &[Kept]) -> Vec<(Client, Removed)> {
    let home = open(home);
    (kept.iter())
        .map(|kept| {
            let removed = home
                .as_ref()
                .map_or(Removed::Missing, |home| remove_in(home, kept));
            (kept.program, removed)
        })
        .collect()
}

/// Where each kept skill's file is, from one opening of the home.
pub fn look_all(home: &Path, kept: &[Kept]) -> Vec<Place> {
    let home = open(home);
    (kept.iter())
        .map(|kept| {
            home.as_ref()
                .map_or(Place::Missing, |home| look_in(home, kept))
        })
        .collect()
}

#[cfg(unix)]
pub use unix::{
    Opened, home_of, look, look_in, open, real_home, remove, remove_in, write, write_in,
};

/// Off Mac and Linux no skill is written: its checks need a walk by open folders the system
/// doesn't give.
#[cfg(not(unix))]
pub fn real_home() -> Option<PathBuf> {
    None
}

#[cfg(not(unix))]
pub fn write(
    _home: &Path,
    _program: Client,
    _name: &str,
    _text: &str,
    _kept: Option<&Kept>,
) -> Result<Kept, Refused> {
    Err(Refused::Failed)
}

#[cfg(not(unix))]
pub fn remove(_home: &Path, _kept: &Kept) -> Removed {
    Removed::Missing
}

#[cfg(not(unix))]
pub fn look(_home: &Path, _kept: &Kept) -> Place {
    Place::Missing
}

#[cfg(not(unix))]
pub struct Opened;

#[cfg(not(unix))]
pub fn open(_home: &Path) -> Option<Opened> {
    None
}

#[cfg(not(unix))]
pub fn write_in(
    _home: &Opened,
    _program: Client,
    _name: &str,
    _text: &str,
    _kept: Option<&Kept>,
) -> Result<Kept, Refused> {
    Err(Refused::Failed)
}

#[cfg(not(unix))]
pub fn remove_in(_home: &Opened, _kept: &Kept) -> Removed {
    Removed::Missing
}

#[cfg(not(unix))]
pub fn look_in(_home: &Opened, _kept: &Kept) -> Place {
    Place::Missing
}

#[cfg(unix)]
mod unix {
    use std::ffi::CString;
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use super::{Client, FILE, Kept, Place, READ_AT_MOST, Refused, Removed, SKILLS, folder};

    /// Why a folder or file couldn't be opened as one of the user's own.
    #[derive(Debug, PartialEq)]
    enum Bad {
        Missing,
        Not,
    }

    /// An open folder of the user's own.
    struct Dir {
        fd: OwnedFd,
        uid: libc::uid_t,
    }

    fn name(name: &str) -> io::Result<CString> {
        CString::new(name).map_err(|_| io::ErrorKind::InvalidInput.into())
    }

    fn checked(result: libc::c_int) -> io::Result<()> {
        match result {
            0 => Ok(()),
            _ => Err(io::Error::last_os_error()),
        }
    }

    fn stat(fd: &OwnedFd) -> io::Result<libc::stat> {
        let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
        checked(unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) })?;
        Ok(stat)
    }

    fn is(stat: &libc::stat, kind: libc::mode_t) -> bool {
        stat.st_mode & libc::S_IFMT == kind
    }

    impl Dir {
        /// The home, which may itself be a link the system made, as the one open that follows one.
        fn home(home: &Path) -> Result<Dir, Bad> {
            // A relative home would be opened from wherever the app was started.
            if !home.is_absolute() {
                return Err(Bad::Not);
            }
            let path = CString::new(home.as_os_str().as_bytes()).map_err(|_| Bad::Not)?;
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC;
            let fd = unsafe { libc::open(path.as_ptr(), flags) };
            Dir::owned(fd, unsafe { libc::getuid() })
        }

        fn owned(fd: libc::c_int, uid: libc::uid_t) -> Result<Dir, Bad> {
            if fd < 0 {
                return Err(match io::Error::last_os_error().raw_os_error() {
                    Some(libc::ENOENT) => Bad::Missing,
                    _ => Bad::Not,
                });
            }
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            // The user's own, and no one else's to write in; a group may.
            match stat(&fd) {
                Ok(stat)
                    if is(&stat, libc::S_IFDIR)
                        && stat.st_uid == uid
                        && stat.st_mode & libc::S_IWOTH == 0 =>
                {
                    Ok(Dir { fd, uid })
                }
                _ => Err(Bad::Not),
            }
        }

        /// The folder `name` in this one, never through a link.
        fn sub(&self, name: &str) -> Result<Dir, Bad> {
            let name = self::name(name).map_err(|_| Bad::Not)?;
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let fd = unsafe { libc::openat(self.fd.as_raw_fd(), name.as_ptr(), flags) };
            Dir::owned(fd, self.uid)
        }

        fn mkdir(&self, name: &str) -> io::Result<()> {
            let name = self::name(name)?;
            checked(unsafe { libc::mkdirat(self.fd.as_raw_fd(), name.as_ptr(), 0o700) })
        }

        fn rmdir(&self, name: &str) -> io::Result<()> {
            let name = self::name(name)?;
            let at = libc::AT_REMOVEDIR;
            checked(unsafe { libc::unlinkat(self.fd.as_raw_fd(), name.as_ptr(), at) })
        }

        fn unlink(&self, name: &str) -> io::Result<()> {
            let name = self::name(name)?;
            checked(unsafe { libc::unlinkat(self.fd.as_raw_fd(), name.as_ptr(), 0) })
        }

        /// The file `name`'s bytes and which file it is, never through a link, and only a file of
        /// the user's own no larger than any skill of ours.
        fn read(&self, name: &str) -> Result<(Vec<u8>, (u64, u64)), Bad> {
            let name = self::name(name).map_err(|_| Bad::Not)?;
            let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC;
            let fd = unsafe { libc::openat(self.fd.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                return Err(match io::Error::last_os_error().raw_os_error() {
                    Some(libc::ENOENT) => Bad::Missing,
                    _ => Bad::Not,
                });
            }
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            let stat = stat(&fd).map_err(|_| Bad::Not)?;
            if !is(&stat, libc::S_IFREG)
                || stat.st_uid != self.uid
                || stat.st_size as u64 > READ_AT_MOST
            {
                return Err(Bad::Not);
            }
            let mut bytes = Vec::new();
            (File::from(fd).take(READ_AT_MOST + 1))
                .read_to_end(&mut bytes)
                .map_err(|_| Bad::Not)?;
            Ok((bytes, (stat.st_dev as u64, stat.st_ino as u64)))
        }

        /// Which file `name` is now, without following a link.
        fn which(&self, name: &str) -> io::Result<(u64, u64)> {
            let name = self::name(name)?;
            let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
            let at = libc::AT_SYMLINK_NOFOLLOW;
            checked(unsafe { libc::fstatat(self.fd.as_raw_fd(), name.as_ptr(), &mut stat, at) })?;
            Ok((stat.st_dev as u64, stat.st_ino as u64))
        }

        /// A new file `name`, only the user able to read it, that wasn't there before, and which
        /// file it is.
        fn create(&self, name: &str) -> io::Result<(File, (u64, u64))> {
            let name = self::name(name)?;
            let flags =
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let mode = 0o600 as libc::c_uint;
            let fd = unsafe { libc::openat(self.fd.as_raw_fd(), name.as_ptr(), flags, mode) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            checked(unsafe { libc::fchmod(fd.as_raw_fd(), 0o600) })?;
            let stat = stat(&fd)?;
            Ok((File::from(fd), (stat.st_dev as u64, stat.st_ino as u64)))
        }

        /// Renames `from` to `to`, which mustn't be there: a file that appeared meanwhile is never
        /// replaced.
        fn rename_new(&self, from: &str, to: &str) -> io::Result<()> {
            let (from, to, fd) = (self::name(from)?, self::name(to)?, self.fd.as_raw_fd());
            #[cfg(all(target_os = "linux", target_env = "gnu"))]
            return checked(unsafe {
                libc::renameat2(fd, from.as_ptr(), fd, to.as_ptr(), libc::RENAME_NOREPLACE)
            });
            #[cfg(target_os = "macos")]
            return checked(unsafe {
                libc::renameatx_np(fd, from.as_ptr(), fd, to.as_ptr(), libc::RENAME_EXCL)
            });
            #[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "macos")))]
            {
                _ = (from, to, fd);
                Err(io::ErrorKind::Unsupported.into())
            }
        }

        fn rename_over(&self, from: &str, to: &str) -> io::Result<()> {
            let (from, to, fd) = (self::name(from)?, self::name(to)?, self.fd.as_raw_fd());
            checked(unsafe { libc::renameat(fd, from.as_ptr(), fd, to.as_ptr()) })
        }

        fn sync(&self) {
            unsafe { libc::fsync(self.fd.as_raw_fd()) };
        }
    }

    /// The user's home, as the product's other writes there take it (the command's link, Codex's
    /// config): the check runs give the app a made-up one this way, which the system's user list
    /// would get past. No other variable moves it. No test may reach it.
    #[cfg(not(test))]
    pub fn real_home() -> Option<PathBuf> {
        // HOME as clients::home reads it.
        home_of(std::env::var("HOME").ok())
    }

    /// The home `HOME` names, where it's set and absolute: else none, and nothing is written or
    /// removed, with no other place to fall back on.
    pub fn home_of(home: Option<String>) -> Option<PathBuf> {
        home.map(PathBuf::from).filter(|home| home.is_absolute())
    }

    #[cfg(test)]
    pub fn real_home() -> Option<PathBuf> {
        panic!("a test asked for the real home; tests use a made-up one (first run's floor 9)")
    }

    /// Writes `text` as the skill `name` for `program` in `home`, at the user's click alone, over
    /// a file only where `kept` says it's ours and it's unchanged, and says what to keep of it.
    pub fn write(
        home: &Path,
        program: Client,
        name: &str,
        text: &str,
        kept: Option<&Kept>,
    ) -> Result<Kept, Refused> {
        write_in(&open(home).ok_or(Refused::Home)?, program, name, text, kept)
    }

    /// The home, opened once as a directory of the user's own, which each step after goes from.
    pub struct Opened(Dir);

    pub fn open(home: &Path) -> Option<Opened> {
        Dir::home(home).ok().map(Opened)
    }

    pub fn write_in(
        home: &Opened,
        program: Client,
        name: &str,
        text: &str,
        kept: Option<&Kept>,
    ) -> Result<Kept, Refused> {
        let parent = folder(program).ok_or(Refused::NoFolder)?;
        if !super::valid(name) {
            return Err(Refused::Failed);
        }
        let ours = kept.filter(|kept| kept.program == program && kept.name == name);
        let home_dir = &home.0;
        let maker = home_dir.sub(parent).map_err(|_| Refused::NoFolder)?;
        // The one folder the product may make, in a maker's folder of the user's own.
        let (skills, made_skills) = match maker.sub(SKILLS) {
            Ok(skills) => (skills, false),
            Err(Bad::Missing) => {
                maker.mkdir(SKILLS).map_err(|_| Refused::NoFolder)?;
                match maker.sub(SKILLS) {
                    Ok(skills) => (skills, true),
                    Err(_) => {
                        _ = maker.rmdir(SKILLS);
                        return Err(Refused::NoFolder);
                    }
                }
            }
            Err(_) => return Err(Refused::NoFolder),
        };
        let undo_skills = || {
            if made_skills {
                _ = maker.rmdir(SKILLS);
            }
        };
        let made_folder = match skills.mkdir(name) {
            Ok(()) => true,
            Err(error) if error.raw_os_error() == Some(libc::EEXIST) && ours.is_some() => false,
            Err(error) => {
                undo_skills();
                return Err(match error.raw_os_error() {
                    Some(libc::EEXIST) => Refused::NotOurs,
                    _ => Refused::Failed,
                });
            }
        };
        let undo_folder = || {
            if made_folder {
                _ = skills.rmdir(name);
            }
            undo_skills();
        };
        let Ok(skill) = skills.sub(name) else {
            undo_folder();
            return Err(Refused::NotOurs);
        };
        // Over a file only as the one hashed, by its device and inode.
        let replace = match (made_folder, ours) {
            (true, _) => None,
            (false, Some(ours)) => match skill.read(FILE) {
                Ok((bytes, which)) if super::sha256(&bytes) == ours.sha256 => Some(which),
                Err(Bad::Missing) => None,
                _ => return Err(Refused::Changed),
            },
            (false, None) => return Err(Refused::NotOurs),
        };
        let beside = format!(".{FILE}.{}", super::random());
        let written = (|| {
            let (mut out, written) = skill.create(&beside)?;
            out.write_all(text.as_bytes())?;
            out.sync_all()?;
            drop(out);
            match replace {
                Some(which) if skill.which(FILE).ok() != Some(which) => {
                    Err(io::ErrorKind::AlreadyExists.into())
                }
                Some(_) => skill.rename_over(&beside, FILE),
                None => skill.rename_new(&beside, FILE),
            }
            .map(|()| written)
        })();
        let Ok(written) = written else {
            _ = skill.unlink(&beside);
            undo_folder();
            return Err(Refused::Failed);
        };
        skill.sync();
        Ok(Kept {
            program,
            name: name.into(),
            sha256: super::sha256(text.as_bytes()),
            made_skills: made_skills || ours.is_some_and(|ours| ours.made_skills),
            written: Some(written),
        })
    }

    /// Takes away the skill `kept` names, if its file is still byte for byte the one written, and
    /// the very file where the run that wrote it asks, then its folder, and `skills` if the
    /// product made it: each folder only if it's empty, which the system's own remove refuses
    /// otherwise, so nothing lists a folder. Where it is comes from the home, the program and the
    /// name.
    /// The maker's folder, its `skills` and the kept skill's folder, each opened from the one
    /// before as the user's own, by the program and checked name alone.
    fn walk(home: &Opened, kept: &Kept) -> Option<(Dir, Dir, Dir)> {
        let parent = folder(kept.program)?;
        if !super::valid(&kept.name) {
            return None;
        }
        let maker = home.0.sub(parent).ok()?;
        let skills = maker.sub(SKILLS).ok()?;
        let skill = skills.sub(&kept.name).ok()?;
        Some((maker, skills, skill))
    }

    /// Whether the skill's file is byte for byte the one the product wrote, and which file it is.
    fn ours(skill: &Dir, kept: &Kept) -> Result<(u64, u64), Place> {
        match skill.read(FILE) {
            Ok((bytes, which))
                if super::sha256(&bytes) == kept.sha256
                    && kept.written.is_none_or(|written| written == which) =>
            {
                Ok(which)
            }
            Err(Bad::Missing) => Err(Place::Missing),
            _ => Err(Place::Changed),
        }
    }

    pub fn look(home: &Path, kept: &Kept) -> Place {
        open(home).map_or(Place::Missing, |home| look_in(&home, kept))
    }

    pub fn look_in(home: &Opened, kept: &Kept) -> Place {
        match walk(home, kept) {
            Some((_, _, skill)) => match ours(&skill, kept) {
                Ok(_) => Place::Ours,
                Err(place) => place,
            },
            None => Place::Missing,
        }
    }

    pub fn remove(home: &Path, kept: &Kept) -> Removed {
        open(home).map_or(Removed::Missing, |home| remove_in(&home, kept))
    }

    pub fn remove_in(home: &Opened, kept: &Kept) -> Removed {
        let Some((maker, skills, skill)) = walk(home, kept) else {
            return Removed::Missing;
        };
        let removed = match ours(&skill, kept) {
            Ok(which) => {
                // The same file as read, not one put in its place since.
                if skill.which(FILE).ok() != Some(which) || skill.unlink(FILE).is_err() {
                    return Removed::Changed;
                }
                Removed::Gone
            }
            Err(Place::Missing) => Removed::Missing,
            Err(_) => return Removed::Changed,
        };
        drop(skill);
        _ = skills.rmdir(&kept.name);
        if kept.made_skills {
            drop(skills);
            _ = maker.rmdir(SKILLS);
        }
        removed
    }
}

#[cfg(all(test, unix))]
mod tests;
