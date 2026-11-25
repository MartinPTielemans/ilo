//! What a walled task leaves running (#4101): whatever its program started that outlives it, in
//! its group or out of it. Each wall's rules allow looking up its own mark, a service name that
//! names no service, and refuse the control, which no wall's rules allow. So the system, asked
//! about a process's own rules, tells which wall a process is inside, whatever the process says
//! or does. The walk is here once, for every system, over `System`; a system's file gives only
//! its answers (the Mac's is walk_mac).
//!
//! A task's place is its TMP, `Tasks/.tmp/<name>`, and its mark beside it, `<name>.mark`, which
//! no wall's rules name. The mark is written before the folder is made and the program started,
//! so a folder with no mark started nothing, and a mark left by a run that ended without quitting
//! is swept at the next launch.
//!
//! No log line names a mark, a process or a path: what ends is counted.

use std::ffi::{CStr, CString};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::{Wall, WallRefused};

/// The walks a sweep takes before it says it isn't clean.
pub const CAP: usize = 64;
/// The name no wall's rules allow, which tells a wall's rules from rules that allow every name.
pub const CONTROL: &CStr = c"ilo.mark.control";
/// The places' folder, in `Tasks/`.
pub const ROOT: &str = ".tmp";

const PREFIX: &str = "ilo.mark.";
pub(super) const OUTSIDE: &str = "the program isn't inside its wall";

/// A wall's mark: `ilo.mark.` and 32 hex characters from the system's random numbers, new for each
/// task. It never equals the control, which ends in no hex.
pub struct Mark(CString);

impl Mark {
    fn new() -> Mark {
        Mark::parse(&format!("{PREFIX}{}", hex::<16>())).expect("a mark's shape")
    }

    /// A mark read back from its file: exactly the shape `new` makes, or none.
    fn parse(text: &str) -> Option<Mark> {
        let digits = text.strip_prefix(PREFIX)?;
        let shaped = digits.len() == 32
            && digits
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        shaped.then(|| Mark(CString::new(text).expect("hex holds no zero byte")))
    }

    pub fn as_cstr(&self) -> &CStr {
        &self.0
    }
}

impl std::fmt::Debug for Mark {
    /// Never its value, which nothing outside the wall's rules is to read.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Mark")
    }
}

/// `N` random bytes, as lower-case hex.
fn hex<const N: usize>() -> String {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the system has no random numbers");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// What a sweep asks of the system. The real one is `Os`; the tests' is made up.
pub trait System {
    /// Every process there is now.
    fn pids(&self) -> io::Result<Vec<u32>>;
    /// Whether `pid` has rules, they allow `mark` and they refuse `control`, or None where the
    /// system can't be asked.
    fn holds(&self, pid: u32, mark: &CStr, control: &CStr) -> Option<bool>;
    /// Ends the process `pid`.
    fn kill(&self, pid: u32);
    /// Ends every process in the group `leader` started with its walled start.
    fn end_group(&self, leader: u32);
}

/// The counts a clean sweep ends with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    pub ended: usize,
    pub walks: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SweepFailed {
    /// A walk still found one at the cap.
    Cap,
    /// The system's call isn't there, so nothing can be asked.
    Missing,
    /// The system wouldn't list its processes.
    Unlisted,
}

/// Ends every process inside the wall marked `mark`, each the moment the system says it is, and
/// walks again until a walk finds none, up to the cap. Each walk lists the processes anew, as one
/// started since isn't in the last list. Never ilo, 0 or 1. A process that ends and whose id is
/// taken between the question and the kill is the window left: ids come back only on a wrap.
pub fn sweep_by(system: &impl System, mark: &Mark) -> Result<Swept, SweepFailed> {
    let own = std::process::id();
    let mut ended = 0;
    for walks in 1..=CAP {
        let mut found = false;
        for pid in system.pids().map_err(|_| SweepFailed::Unlisted)? {
            if pid <= 1 || pid == own {
                continue;
            }
            match system.holds(pid, mark.as_cstr(), CONTROL) {
                None => return Err(SweepFailed::Missing),
                Some(true) => {
                    system.kill(pid);
                    ended += 1;
                    found = true;
                }
                Some(false) => {}
            }
        }
        if !found {
            return Ok(Swept { ended, walks });
        }
    }
    Err(SweepFailed::Cap)
}

/// The system's answers: the Mac's, and elsewhere none, as no other system walls a task in yet.
pub struct Os;

impl System for Os {
    fn pids(&self) -> io::Result<Vec<u32>> {
        os::pids()
    }

    fn holds(&self, pid: u32, mark: &CStr, control: &CStr) -> Option<bool> {
        os::holds(pid, mark, control)
    }

    fn kill(&self, pid: u32) {
        signal(pid, false);
    }

    fn end_group(&self, leader: u32) {
        signal(leader, true);
    }
}

#[cfg(target_os = "macos")]
mod os {
    pub use super::super::walk_mac::{holds, pids};
}

/// No other system walls a task in yet, so nothing there is inside a wall.
#[cfg(not(target_os = "macos"))]
mod os {
    use std::ffi::CStr;
    use std::io;

    pub fn pids() -> io::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    pub fn holds(_pid: u32, _mark: &CStr, _control: &CStr) -> Option<bool> {
        Some(false)
    }
}

/// Kills `pid`, or the group it leads: never ilo, and never 0, 1 or an id past a pid's, which to a
/// signal are a whole group or every process.
#[cfg(unix)]
fn signal(pid: u32, group: bool) {
    let Ok(pid @ 2..) = libc::pid_t::try_from(pid) else {
        return;
    };
    if pid as u32 == std::process::id() {
        return;
    }
    // SAFETY: a signal to one process, or to the group a walled start made for its program.
    unsafe { libc::kill(if group { -pid } else { pid }, libc::SIGKILL) };
}

/// Windows walls no task in, so nothing walled is started there to end.
#[cfg(not(unix))]
fn signal(_pid: u32, _group: bool) {}

/// Ends what is inside the wall marked `mark`. Elsewhere than the Mac nothing is walled yet, and
/// the one walk finds nothing to end.
pub fn sweep(mark: &Mark) -> Result<Swept, SweepFailed> {
    sweep_by(&Os, mark)
}

/// A task's TMP and the mark beside it.
pub struct Place {
    tmp: PathBuf,
    mark: Mark,
}

impl std::fmt::Debug for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Place").finish_non_exhaustive()
    }
}

impl Place {
    /// A new place under `root`, `Tasks/.tmp`: its mark file first, 0600, then its folder, 0700,
    /// so that nothing starts in a folder with no mark beside it.
    pub fn make(root: &Path) -> io::Result<Place> {
        folder(true).create(root)?;
        let tmp = root.join(hex::<8>());
        let mark = Mark::new();
        let mark_file = tmp.with_extension("mark");
        let mut file = std::fs::OpenOptions::new();
        file.write(true).create_new(true);
        #[cfg(unix)]
        file.mode(0o600);
        let mut file = file.open(&mark_file)?;
        file.write_all(mark.as_cstr().to_bytes())?;
        drop(file);
        if let Err(error) = folder(false).create(&tmp) {
            _ = std::fs::remove_file(&mark_file);
            return Err(error);
        }
        Ok(Place { tmp, mark })
    }

    pub fn tmp(&self) -> &Path {
        &self.tmp
    }

    pub fn mark(&self) -> &Mark {
        &self.mark
    }
}

/// A folder only its user reads, 0700.
fn folder(recursive: bool) -> std::fs::DirBuilder {
    let mut folder = std::fs::DirBuilder::new();
    folder.recursive(recursive);
    #[cfg(unix)]
    folder.mode(0o700);
    folder
}

impl Wall {
    /// This wall, for the task whose place is `place`: its rules allow the place's mark.
    pub fn in_place(mut self, place: std::sync::Arc<Place>) -> Wall {
        self.tmp = Some(place.tmp().to_path_buf());
        self.place = Some(place);
        self
    }

    /// The start's control, once its launcher has become the program: the system must say the
    /// program is inside this wall. If not, its group is ended and nothing started.
    pub fn control(&self, pid: u32) -> Result<(), WallRefused> {
        control_by(&Os, self.place.as_deref(), pid)
    }

    /// After a walled program and its group have ended and it's reaped: ends what it left
    /// running out of its group, logs how many, and answers what it found, for the task. None for
    /// a wall with no place, which nothing can be inside.
    pub fn sweep_after(&self) -> Option<Result<Swept, SweepFailed>> {
        let answer = sweep(self.place.as_ref()?.mark());
        match answer {
            Ok(Swept { ended: 0, .. }) => {}
            Ok(Swept { ended, walks }) => {
                log!("browser: a walled task left {ended} running, ended over {walks} walks");
            }
            Err(failed) => {
                log!("browser: what a walled task left running wasn't ended: {failed:?}")
            }
        }
        Some(answer)
    }
}

fn control_by(system: &impl System, place: Option<&Place>, pid: u32) -> Result<(), WallRefused> {
    let inside = |place: &Place| system.holds(pid, place.mark().as_cstr(), CONTROL) == Some(true);
    if place.is_some_and(inside) {
        return Ok(());
    }
    system.end_group(pid);
    Err(WallRefused::Failed(OUTSIDE.into()))
}

/// A place a run before left: its name, and whether its mark and its folder are there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Left {
    name: String,
    mark: bool,
    folder: bool,
}

/// The places in `root` now, by the names places are made with. Listed at launch before any task
/// can make one, so a task's own is never in it.
pub fn left_in(root: &Path) -> Vec<Left> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut left: Vec<Left> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let (stem, mark) = match name.strip_suffix(".mark") {
            Some(stem) => (stem, true),
            None => (name, false),
        };
        if stem.len() != 16 || !stem.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            continue;
        }
        let at = match left.iter().position(|each| each.name == stem) {
            Some(at) => at,
            None => {
                left.push(Left {
                    name: stem.into(),
                    mark: false,
                    folder: false,
                });
                left.len() - 1
            }
        };
        if mark {
            left[at].mark = true;
        } else {
            left[at].folder = true;
        }
    }
    left
}

/// What the launch's cleanup did, in counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cleaned {
    /// Processes ended.
    pub ended: usize,
    /// Places removed.
    pub removed: usize,
    /// Places kept, as a sweep wasn't clean: the next launch tries again.
    pub kept: usize,
}

/// The launch's cleanup of `left`, off the UI thread: a place with a mark is swept and then both
/// are removed, the folder first; one whose sweep isn't clean is kept. A folder with no mark, or a
/// mark that can't be read, started nothing and is removed.
pub fn clean_up(root: &Path, left: &[Left]) -> Cleaned {
    clean_up_by(&Os, root, left)
}

fn clean_up_by(system: &impl System, root: &Path, left: &[Left]) -> Cleaned {
    let mut cleaned = Cleaned::default();
    for place in left {
        let tmp = root.join(&place.name);
        let mark_file = tmp.with_extension("mark");
        let mark = place.mark.then(|| read_mark(&mark_file)).flatten();
        if let Some(mark) = mark {
            match sweep_by(system, &mark) {
                Ok(swept) => cleaned.ended += swept.ended,
                Err(_) => {
                    cleaned.kept += 1;
                    continue;
                }
            }
        }
        if place.folder {
            remove(&tmp);
        }
        if place.mark {
            remove(&mark_file);
        }
        cleaned.removed += 1;
    }
    cleaned
}

/// The mark in `file`, if it is a file of a mark's length holding one.
fn read_mark(file: &Path) -> Option<Mark> {
    let meta = std::fs::symlink_metadata(file).ok()?;
    if !meta.is_file() || meta.len() > 64 {
        return None;
    }
    Mark::parse(&std::fs::read_to_string(file).ok()?)
}

/// Removes `path`, a folder with all in it or anything else, never following a link.
fn remove(path: &Path) {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => _ = std::fs::remove_dir_all(path),
        Ok(_) => _ = std::fs::remove_file(path),
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests;
