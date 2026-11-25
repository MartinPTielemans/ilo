//! How an assistant is started inside its wall (#4101). On the Mac ilo starts its own launcher in
//! the assistant's place (platform/mac/wall_launcher.c), hands it the wall's rules as the first
//! bytes of its input, and the launcher puts them on itself and becomes the assistant. Nothing is
//! inside a wall until the system says the launcher became the program: `Rising::risen` waits for
//! that, on the thread that reads the assistant's output, and answers a launcher that ended
//! instead with a sentence that names no folder, no parameter and no code of the system's.
//!
//! What a caller that starts the task again without its files must know: whatever was written to
//! the assistant's input before `risen` answered no went to a launcher that never read it.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::SystemTime;

use super::{Wall, WallRefused};

#[cfg(target_os = "macos")]
#[path = "../../../platform/mac/wall_watch.rs"]
mod watch;

#[cfg(test)]
mod tests;

/// The hand-off's version and caps, which the launcher's own file holds too.
pub const VERSION: u32 = 1;
/// The whole hand-off: what a pipe on the Mac takes at once while nothing reads it, so writing it
/// never waits for the launcher.
pub const HANDOFF_MAX: usize = 16384;
pub const PARAMETERS_MAX: usize = 32;
pub const KEPT_MAX: usize = 64;

const NOT_THERE: &str = "the launcher isn't there";
const TOO_LONG: &str = "the rules don't fit what the launcher reads";
#[cfg(target_os = "macos")]
const NOT_GIVEN: &str = "the launcher couldn't be given its rules";
#[cfg(target_os = "macos")]
const NOT_WATCHED: &str = "the launcher couldn't be watched";
/// What each of the launcher's exit codes says, as its file lists them.
#[cfg(target_os = "macos")]
const CODES: [(i32, &str); 4] = [
    (112, "the launcher couldn't read its rules"),
    (
        113,
        "the launcher couldn't close the handles it was started with",
    ),
    (114, "the system refused the launcher's rules"),
    (115, "the launcher couldn't start the program"),
];
#[cfg(target_os = "macos")]
const ENDED: &str = "the launcher ended before the program started";

/// A wall's rules, as the system's sandbox reads them, and the paths they're filled in with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    pub text: String,
    pub params: Vec<(&'static str, PathBuf)>,
}

/// What a walled task's program is started through: ilo's launcher, and what it's handed.
pub struct Launcher {
    program: PathBuf,
    handoff: Vec<u8>,
}

fn failed(sentence: &str) -> WallRefused {
    WallRefused::Failed(sentence.into())
}

/// ilo's own launcher: beside the app's binary in its bundle, and in a test the one build.rs
/// made. Never from the environment, a setting or PATH, which a task could have a hand in.
#[cfg(all(test, target_os = "macos"))]
fn launcher() -> Option<PathBuf> {
    Some(PathBuf::from(env!("WALL_LAUNCHER")))
}

#[cfg(all(not(test), target_os = "macos"))]
fn launcher() -> Option<PathBuf> {
    Some(
        std::env::current_exe()
            .ok()?
            .parent()?
            .join("wall-launcher"),
    )
}

/// No other system has one yet.
#[cfg(not(target_os = "macos"))]
fn launcher() -> Option<PathBuf> {
    None
}

fn put_text(to: &mut Vec<u8>, text: &[u8]) -> Option<()> {
    if text.contains(&0) {
        return None;
    }
    to.extend((text.len() as u32).to_le_bytes());
    to.extend(text);
    to.push(0);
    Some(())
}

/// The hand-off as the launcher reads it (its file's head says how), or None if it wouldn't: over
/// a cap, a zero byte in a text, or a name that can't be an environment's or comes twice.
fn handoff(rules: &Rules, kept: &[&str]) -> Option<Vec<u8>> {
    if rules.params.len() > PARAMETERS_MAX || kept.len() > KEPT_MAX {
        return None;
    }
    let mut body = Vec::new();
    put_text(&mut body, rules.text.as_bytes())?;
    body.extend((rules.params.len() as u32).to_le_bytes());
    for (key, value) in &rules.params {
        put_text(&mut body, key.as_bytes())?;
        put_text(&mut body, value.as_os_str().as_encoded_bytes())?;
    }
    body.extend((kept.len() as u32).to_le_bytes());
    for (at, name) in kept.iter().enumerate() {
        if name.is_empty() || name.contains('=') || kept[..at].contains(name) {
            return None;
        }
        put_text(&mut body, name.as_bytes())?;
    }
    let mut whole = Vec::with_capacity(12 + body.len());
    whole.extend(b"ILOW");
    whole.extend(VERSION.to_le_bytes());
    whole.extend((body.len() as u32).to_le_bytes());
    whole.extend(body);
    (whole.len() <= HANDOFF_MAX).then_some(whole)
}

impl Wall {
    /// The wall around `folder`, the user's pick with its identity, so its watch knows the folder
    /// it holds, that holds `rules`, inside which a program keeps of its environment the names in
    /// `kept` and nothing else. Nothing is started here: rules that the launcher couldn't be handed
    /// are refused now, before anything is.
    pub fn inside(
        folder: Option<super::Granted>,
        rules: &Rules,
        kept: &[&str],
    ) -> Result<Wall, WallRefused> {
        let handoff = handoff(rules, kept).ok_or_else(|| failed(TOO_LONG))?;
        let program = launcher()
            .filter(|launcher| launcher.is_file())
            .ok_or_else(|| failed(NOT_THERE))?;
        Ok(Wall {
            folder,
            started: SystemTime::now(),
            launcher: Some(Launcher { program, handoff }),
            tmp: None,
            place: None,
        })
    }

    /// The command that starts `program` inside this wall: ilo's launcher, told the program it's
    /// to become by its whole path, or for a task with no files the program's own.
    pub fn command(&self, program: &Path) -> Command {
        match &self.launcher {
            None => crate::child::command(program),
            Some(launcher) => {
                let mut command = crate::child::command(&launcher.program);
                command.arg(program);
                // A group of its own, which its end ends whole before it's reaped.
                #[cfg(unix)]
                std::os::unix::process::CommandExt::process_group(&mut command, 0);
                command
            }
        }
    }

    /// Starts `command`, which `Wall::command` made and whose input is a pipe. A walled start
    /// comes back with its `Rising`, the launcher's rules already in its input ahead of whatever
    /// the caller writes there next.
    pub fn start(&self, command: &mut Command) -> io::Result<(Child, Option<Rising>)> {
        let mut child = command.spawn()?;
        let rising = (self.launcher.as_ref()).map(|launcher| give(&mut child, &launcher.handoff));
        Ok((child, rising))
    }
}

/// Ends a launcher that can't go on, and waits for its end, which follows at once.
fn end(child: &mut Child, sentence: &str) -> Rising {
    _ = child.kill();
    _ = child.wait();
    Rising(State::Known(Err(failed(sentence))))
}

/// Watches the launcher just started and hands it its rules, in that order: it reads them before
/// it does anything, so whatever it does next is seen.
#[cfg(target_os = "macos")]
fn give(child: &mut Child, handoff: &[u8]) -> Rising {
    let watch = match watch::watch(child.id()) {
        Ok(Some(watch)) => watch,
        // Gone before it could be watched: its end says how.
        Ok(None) => {
            let code = child.wait().ok().and_then(|status| status.code());
            return Rising(State::Known(Err(ended(code))));
        }
        Err(_) => return end(child, NOT_WATCHED),
    };
    if !child
        .stdin
        .as_ref()
        .is_some_and(|input| hand(input, handoff))
    {
        return end(child, NOT_GIVEN);
    }
    Rising(State::Watched(watch))
}

#[cfg(not(target_os = "macos"))]
fn give(child: &mut Child, _handoff: &[u8]) -> Rising {
    end(child, NOT_THERE)
}

/// Writes the whole hand-off to the launcher's input without waiting for room there, and says
/// whether all of it went: the pipe is empty and takes HANDOFF_MAX at once, so less than all is a
/// failure and never a reason to wait or try again. The pipe then blocks again, as its next
/// writer expects.
#[cfg(target_os = "macos")]
fn hand(input: &std::process::ChildStdin, handoff: &[u8]) -> bool {
    use std::os::fd::AsRawFd;
    let input = input.as_raw_fd();
    // SAFETY: calls on a descriptor the caller holds open, the write reading `handoff` alone.
    unsafe {
        let mode = libc::fcntl(input, libc::F_GETFL);
        if mode < 0 || libc::fcntl(input, libc::F_SETFL, mode | libc::O_NONBLOCK) < 0 {
            return false;
        }
        let wrote = libc::write(input, handoff.as_ptr().cast(), handoff.len());
        let restored = libc::fcntl(input, libc::F_SETFL, mode) == 0;
        restored && wrote == handoff.len() as isize
    }
}

/// Why a launcher that ended with `code`, or with None by a signal, started nothing.
#[cfg(target_os = "macos")]
fn ended(code: Option<i32>) -> WallRefused {
    let said = CODES.iter().find(|(each, _)| Some(*each) == code);
    failed(said.map_or(ENDED, |(_, sentence)| sentence))
}

/// A walled start, until the system says what its launcher did.
pub struct Rising(State);

enum State {
    #[cfg(target_os = "macos")]
    Watched(watch::Watch),
    Known(Result<(), WallRefused>),
}

impl Rising {
    /// Waits for the launcher to become the program, which is then inside the wall, or to end,
    /// which says why nothing started. It waits as long as the launcher lives, so it's for a
    /// thread that may: ending the launcher, as dropping its process does, ends the wait.
    pub fn risen(self) -> Result<(), WallRefused> {
        match self.0 {
            State::Known(known) => known,
            #[cfg(target_os = "macos")]
            State::Watched(watch) => match watch.end() {
                Ok(watch::End::Exec) => Ok(()),
                Ok(watch::End::Exit(code)) => Err(ended(code)),
                Err(_) => Err(failed(NOT_WATCHED)),
            },
        }
    }
}
