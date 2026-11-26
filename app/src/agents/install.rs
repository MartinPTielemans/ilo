//! Installing an assistant's program from its row's Install and Sign In (#4100). The installer is
//! the one the program's maker publishes, run as published: fetched over https to a file of its own,
//! then run with no arguments of ilo's, so it installs what it would from a terminal, where it would
//! (Claude Code in ~/.local/bin), as the user's own copy their terminal and other apps find too. It
//! checks what it downloads against the maker's own checksum itself. Only the user's press starts
//! one, and ilo never removes what it installed.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::channel::mpsc::UnboundedSender;

use super::runner::Cli;
use super::sign_in::{Ended, Session};
use crate::child::command;

/// Anthropic's installer for Claude Code on the Mac and Linux, as its setup page has it.
pub const CLAUDE_SH: &str = "https://claude.ai/install.sh";
/// Anthropic's installer for Claude Code on Windows, as its setup page has it.
pub const CLAUDE_PS1: &str = "https://claude.ai/install.ps1";

/// The reason a failed install's line gives where the installer couldn't be fetched for want of a
/// connection (AS4, in TS2's form).
pub const OFFLINE: &str = "you're offline";

/// The exit codes of curl's fetch that mean no connection: the host couldn't be found or reached.
const CURL_OFFLINE: &[i32] = &[6, 7];

impl Cli {
    /// Whether its row installs it where it's missing: only where its maker's installer is known.
    /// Codex's waits for its road (#4100).
    pub fn installs(self) -> bool {
        matches!(self, Cli::Claude)
    }

    /// Its maker's page that says how to uninstall it, which How to Uninstall opens (AS6).
    pub fn uninstall_page(self) -> Option<&'static str> {
        match self {
            Cli::Claude => Some("https://code.claude.com/docs/en/setup#uninstall-claude-code"),
            Cli::Codex => None,
        }
    }
}

/// One program an install runs, and the exit codes of it that mean the computer is offline.
pub(super) struct Step {
    pub command: Command,
    pub offline: &'static [i32],
}

/// The steps of `cli`'s install, its installer fetched into `folder`: none where it has no road.
pub(super) fn steps(cli: Cli, folder: &Path) -> Option<Vec<Step>> {
    if !cli.installs() {
        return None;
    }
    let (url, file) = match cfg!(windows) {
        true => (CLAUDE_PS1, folder.join("install.ps1")),
        false => (CLAUDE_SH, folder.join("install.sh")),
    };
    // curl is the system's own on the Mac, Linux and Windows 10 and later.
    let mut fetch = command("curl");
    fetch
        .args(["-fsSL", "--proto", "=https", "-o"])
        .arg(&file)
        .arg(url);
    let run = match cfg!(windows) {
        // The file fetched is the one run. A downloaded script runs only past the computer's
        // policy for scripts, as the maker's own command to paste passes it.
        true => {
            let mut run = command("powershell");
            run.args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ]);
            run.arg(&file);
            run
        }
        false => {
            let mut run = command("bash");
            run.arg(&file);
            run
        }
    };
    Some(vec![
        Step {
            command: fetch,
            offline: CURL_OFFLINE,
        },
        Step {
            command: run,
            offline: &[],
        },
    ])
}

/// Starts `cli`'s install, which tells `ended` how it ended.
pub fn start(cli: Cli, ended: UnboundedSender<Ended>) -> std::io::Result<Box<dyn Session>> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let number = NEXT.fetch_add(1, Ordering::SeqCst);
    let folder = std::env::temp_dir().join(format!(
        "ilo-install-{}-{}-{number}",
        cli.id(),
        std::process::id()
    ));
    std::fs::create_dir_all(&folder)?;
    let Some(steps) = steps(cli, &folder) else {
        _ = std::fs::remove_dir_all(&folder);
        return Err(std::io::ErrorKind::Unsupported.into());
    };
    Ok(Box::new(Install::run(steps, Some(folder), ended)))
}

/// An install under way: its steps in turn, on a thread of their own, each waited for.
pub(super) struct Install {
    /// The program running now, while it runs, which Cancel ends.
    running: Arc<Mutex<Option<u32>>>,
    cancelled: Arc<AtomicBool>,
}

impl Install {
    /// Runs `steps` in turn until one fails, then takes `folder` away and says how it ended.
    pub(super) fn run(
        steps: Vec<Step>,
        folder: Option<PathBuf>,
        ended: UnboundedSender<Ended>,
    ) -> Install {
        let running = Arc::new(Mutex::new(None));
        let cancelled = Arc::new(AtomicBool::new(false));
        std::thread::spawn({
            let (running, cancelled) = (running.clone(), cancelled.clone());
            move || {
                let end = run_steps(steps, &running, &cancelled);
                if let Some(folder) = folder {
                    _ = std::fs::remove_dir_all(folder);
                }
                _ = ended.unbounded_send(end);
            }
        });
        Install { running, cancelled }
    }

    fn end(&self) {
        let running = self.running.lock().unwrap();
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(pid) = *running {
            end_group(pid);
        }
    }
}

fn run_steps(steps: Vec<Step>, running: &Mutex<Option<u32>>, cancelled: &AtomicBool) -> Ended {
    for Step {
        mut command,
        offline,
    } in steps
    {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Its own group, so Cancel ends what it started too, as the download its installer runs.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = {
            let mut running = running.lock().unwrap();
            if cancelled.load(Ordering::SeqCst) {
                return Ended::Cancelled;
            }
            match super::sign_in::spawn(&mut command) {
                Ok(child) => {
                    *running = Some(child.id());
                    child
                }
                Err(_) => return Ended::Failed(None),
            }
        };
        let status = child.wait();
        // Its id may be another program's from now on.
        *running.lock().unwrap() = None;
        match status {
            _ if cancelled.load(Ordering::SeqCst) => return Ended::Cancelled,
            Ok(status) if status.success() => {}
            Ok(status) if status.code().is_some_and(|code| offline.contains(&code)) => {
                return Ended::Failed(Some(OFFLINE.into()));
            }
            _ => return Ended::Failed(None),
        }
    }
    Ended::Done
}

impl Session for Install {
    fn cancel(&mut self) {
        self.end();
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        self.end();
    }
}

/// Ends the program `pid` and what it started: on unix its group, on Windows it and its children.
fn end_group(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGTERM);
    }
    #[cfg(windows)]
    super::sign_in::end_process(pid);
}

#[cfg(test)]
mod tests;
