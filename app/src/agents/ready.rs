//! Whether each task CLI is installed and signed in (Agent task). The browser learns it when
//! Settings › Agents or ⌘T's ask first shows in a run, and after a task fails, on a background
//! thread, and keeps the last answer till then, across runs too, so ⇥ lists the agents at once:
//! never on a timer, and never while ⌘T opens or takes keys.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

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
    /// The models it lists itself, as id and name, for those ilo can't name ahead: Codex's.
    pub models: Vec<(String, String)>,
}

impl Found {
    /// As the agents' settings keep it.
    pub fn saved(&self) -> Value {
        serde_json::json!({
            "ready": self.ready.id(),
            "program": self.program.as_ref().map(|program| program.to_string_lossy()),
            "path": self.path,
            "models": self.models.iter().map(|(id, name)| [id, name]).collect::<Vec<_>>(),
        })
    }

    pub fn from_saved(saved: &Value) -> Found {
        Found {
            ready: Ready::from_id(saved.get("ready").and_then(Value::as_str).unwrap_or_default()),
            program: saved.get("program").and_then(Value::as_str).map(PathBuf::from),
            path: saved.get("path").and_then(Value::as_str).map(String::from),
            models: saved
                .get("models")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|model| {
                    let id = model.get(0)?.as_str()?;
                    let name = model.get(1)?.as_str()?;
                    Some((id.to_string(), name.to_string()))
                })
                .collect(),
        }
    }
}

/// Looks for `cli` and asks it whether the user is signed in. Blocks: runs the CLI.
pub fn look(cli: Cli) -> Found {
    let path = login_path().clone();
    let search = path.clone().or_else(|| std::env::var("PATH").ok()).unwrap_or_default();
    let Some(program) = find(cli.program(), &search) else {
        // check.sh reads this line.
        log!("browser: task agent {} is NotFound", cli.id());
        return Found { ready: Ready::NotFound, program: None, path, models: Vec::new() };
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
    let models = match (cli, ready) {
        (Cli::Codex, Ready::SignedIn) => codex_models(&program, path.as_deref()),
        _ => Vec::new(),
    };
    Found { ready, program: Some(program), path, models }
}

/// The models Codex lists for the signed-in user, which change with their plan and Codex's
/// version: asked of its app-server, which starts no thread for it, and left after 10 s.
fn codex_models(program: &Path, path: Option<&str>) -> Vec<(String, String)> {
    let mut command = Command::new(program);
    command.arg("app-server").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let Ok(mut child) = command.spawn() else {
        return Vec::new();
    };
    let mut stdin = child.stdin.take().expect("piped");
    let stdout = child.stdout.take().expect("piped");
    let (send, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let mut say = |message: Value| writeln!(stdin, "{message}").ok();
    let deadline = Instant::now() + Duration::from_secs(10);
    let models = (|| {
        say(serde_json::json!({
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": { "name": env!("APP_NAME").to_lowercase(), "title": env!("APP_NAME"), "version": env!("CARGO_PKG_VERSION") },
                "capabilities": { "experimentalApi": true },
            },
        }))?;
        loop {
            let line = lines.recv_timeout(deadline.saturating_duration_since(Instant::now())).ok()?;
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match message.get("id").and_then(Value::as_u64) {
                Some(1) => {
                    say(serde_json::json!({ "method": "initialized" }))?;
                    say(serde_json::json!({ "id": 2, "method": "model/list", "params": { "limit": 100 } }))?;
                }
                Some(2) => return Some(codex_listed(&message)),
                _ => {}
            }
        }
    })();
    _ = child.kill();
    _ = child.wait();
    models.unwrap_or_default()
}

/// The models in Codex's answer to model/list, hidden ones left out.
pub fn codex_listed(answer: &Value) -> Vec<(String, String)> {
    let listed = answer.pointer("/result/data").and_then(Value::as_array);
    listed
        .into_iter()
        .flatten()
        .filter(|model| !model.get("hidden").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|model| {
            let id = model.get("id").or_else(|| model.get("model"))?.as_str()?;
            let name = model.get("displayName").and_then(Value::as_str).unwrap_or(id);
            Some((id.to_string(), name.to_string()))
        })
        .collect()
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
