//! Whether each task CLI is installed and signed in (Agent task). The browser learns it when
//! Settings › Agents or ⌘T's ask first shows in a run, and after a task fails, on a background
//! thread, and keeps the last answer till then, across runs too, so ⇥ lists the agents at once:
//! never on a timer, and never while ⌘T opens or takes keys.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use serde_json::Value;

use super::runner::Cli;

/// What the browser last learned of a CLI.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ready {
    /// Not looked yet.
    #[default]
    Unknown,
    NotFound,
    SignedOut,
    SignedIn,
}

impl Ready {
    fn id(self) -> &'static str {
        match self {
            Ready::Unknown => "unknown",
            Ready::NotFound => "not-found",
            Ready::SignedOut => "signed-out",
            Ready::SignedIn => "signed-in",
        }
    }

    fn from_id(id: &str) -> Ready {
        match id {
            "not-found" => Ready::NotFound,
            "signed-out" => Ready::SignedOut,
            "signed-in" => Ready::SignedIn,
            _ => Ready::Unknown,
        }
    }
}

/// A CLI as found: whether it's ready, and what runs it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub ready: Ready,
    pub program: Option<PathBuf>,
    /// The PATH it runs with: the user's login shell's.
    pub path: Option<String>,
}

impl Found {
    /// As the agents' settings keep it.
    pub fn saved(&self) -> Value {
        serde_json::json!({
            "ready": self.ready.id(),
            "program": self.program.as_ref().map(|program| program.to_string_lossy()),
            "path": self.path,
        })
    }

    pub fn from_saved(saved: &Value) -> Found {
        Found {
            ready: Ready::from_id(saved.get("ready").and_then(Value::as_str).unwrap_or_default()),
            program: saved.get("program").and_then(Value::as_str).map(PathBuf::from),
            path: saved.get("path").and_then(Value::as_str).map(String::from),
        }
    }
}

/// Looks for `cli` and asks it whether the user is signed in. Blocks: runs the CLI.
pub fn look(cli: Cli) -> Found {
    let path = login_path().clone();
    let search = path.clone().or_else(|| std::env::var("PATH").ok()).unwrap_or_default();
    let Some(program) = find(cli.program(), &search) else {
        return Found { ready: Ready::NotFound, program: None, path };
    };
    let mut command = Command::new(&program);
    match cli {
        Cli::Claude => command.args(["auth", "status", "--json"]),
        Cli::Codex => command.args(["login", "status"]),
    };
    if let Some(path) = &path {
        command.env("PATH", path);
    }
    let output = command.stdin(Stdio::null()).stderr(Stdio::null()).output();
    let ready = match output {
        Err(error) => {
            log!("browser: {} couldn't run: {error}", cli.id());
            Ready::NotFound
        }
        Ok(output) => match cli {
            Cli::Claude if claude_signed_in(&String::from_utf8_lossy(&output.stdout)) => {
                Ready::SignedIn
            }
            Cli::Codex if output.status.success() => Ready::SignedIn,
            _ => Ready::SignedOut,
        },
    };
    // check.sh reads this line.
    log!("browser: task agent {} is {ready:?} at {}", cli.id(), program.display());
    Found { ready, program: Some(program), path }
}

/// Whether `claude auth status --json` says the user is signed in.
pub fn claude_signed_in(said: &str) -> bool {
    let status: Value = serde_json::from_str(said.trim()).unwrap_or_default();
    status.get("loggedIn").and_then(Value::as_bool).unwrap_or(false)
}

/// The executable `program` in the first directory of `path` that has it.
pub fn find(program: &str, path: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    metadata.is_file()
}

/// The PATH of the user's login shell, which an app opened from the Dock doesn't have: where npm,
/// Homebrew and the CLIs' own installers put them. Asked once a run, of the shell alone: the CLI
/// itself never runs through it, as an alias there could change its flags. Windows apps have the
/// user's PATH already.
fn login_path() -> &'static Option<String> {
    static PATH: OnceLock<Option<String>> = OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(windows) {
            return None;
        }
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let output = Command::new(shell)
            .args(["-l", "-c", r#"printf "%s" "$PATH""#])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!path.is_empty()).then_some(path)
    })
}
