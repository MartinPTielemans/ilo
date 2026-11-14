//! Whether each task CLI is installed and signed in (Agent task). The browser learns it when
//! Settings › Agents or ⌘T's ask first shows in a run, and after a task fails, on a background
//! thread, and keeps the last answer till then, across runs too, so ⇥ lists the agents at once:
//! never on a timer, and never while ⌘T opens or takes keys.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::child::command;
use std::sync::mpsc;
use std::time::Duration;
use web_time::Instant;

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
    /// The models it listed itself when last asked, or None till it answers: a Claude too old to
    /// list them keeps ilo's names for them (`Cli::unlisted_models`).
    pub models: Option<Models>,
}

/// The models a CLI lists for the signed-in account, in the CLI's order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Models {
    /// Each as id and name, its own default left out: the pop-up's Default stands for it.
    pub listed: Vec<(String, String)>,
    /// Which model its default is, by name, where the CLI says.
    pub default: Option<String>,
}

impl Found {
    /// As the agents' settings keep it.
    pub fn saved(&self) -> Value {
        serde_json::json!({
            "ready": self.ready.id(),
            "program": self.program.as_ref().map(|program| program.to_string_lossy()),
            "path": self.path,
            "listed": self.models.as_ref().map(|models| serde_json::json!({
                "models": models.listed.iter().map(|(id, name)| [id, name]).collect::<Vec<_>>(),
                "default": models.default,
            })),
        })
    }

    pub fn from_saved(saved: &Value) -> Found {
        Found {
            ready: Ready::from_id(
                saved
                    .get("ready")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ),
            program: saved
                .get("program")
                .and_then(Value::as_str)
                .map(PathBuf::from),
            path: saved.get("path").and_then(Value::as_str).map(String::from),
            models: Models::from_saved(saved),
        }
    }
}

impl Models {
    /// As `Found::saved` keeps them, or as a browser before #2455 kept Codex's, as a bare list
    /// whose emptiness said nothing.
    fn from_saved(saved: &Value) -> Option<Models> {
        let pairs = |list: &Value| -> Vec<(String, String)> {
            (list.as_array().into_iter().flatten())
                .filter_map(|model| {
                    let id = model.get(0)?.as_str()?;
                    let name = model.get(1)?.as_str()?;
                    Some((id.to_string(), name.to_string()))
                })
                .collect()
        };
        if let Some(listed) = saved.get("listed").filter(|listed| listed.is_object()) {
            return Some(Models {
                listed: pairs(listed.get("models").unwrap_or(&Value::Null)),
                default: (listed.get("default").and_then(Value::as_str)).map(String::from),
            });
        }
        let listed = pairs(saved.get("models")?);
        (!listed.is_empty()).then_some(Models {
            listed,
            default: None,
        })
    }
}

/// Looks for `cli` and asks it whether the user is signed in. Blocks: runs the CLI.
pub fn look(cli: Cli) -> Found {
    let (found, path) = locate(cli);
    let Some(program) = found else {
        // check.sh reads this line.
        log!("browser: task agent {} is NotFound", cli.id());
        return Found {
            ready: Ready::NotFound,
            program: None,
            path,
            models: None,
        };
    };
    let mut command = command(&program);
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
    log!(
        "browser: task agent {} is {ready:?} at {}",
        cli.id(),
        program.display()
    );
    let models = match (cli, ready) {
        (Cli::Claude, Ready::SignedIn) => claude_models(&program, path.as_deref()),
        (Cli::Codex, Ready::SignedIn) => codex_models(&program, path.as_deref()),
        _ => None,
    };
    if let Some(models) = &models {
        log!(
            "browser: task agent {} lists {} models",
            cli.id(),
            models.listed.len()
        );
    }
    Found {
        ready,
        program: Some(program),
        path,
        models,
    }
}

/// Where `cli` is, and the PATH it runs with: the login shell's, or on Windows, where it may be
/// claude.exe, claude.cmd, codex.cmd or codex.exe, the one a terminal opened now has (#1676).
#[cfg(not(windows))]
fn locate(cli: Cli) -> (Option<PathBuf>, Option<String>) {
    let path = login_path().clone();
    let search = path
        .clone()
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();
    (find(cli.program(), &search), path)
}

#[cfg(windows)]
fn locate(cli: Cli) -> (Option<PathBuf>, Option<String>) {
    match super::clients::windows_find(cli.program()) {
        Some((program, path)) => (Some(program), Some(path)),
        None => (None, None),
    }
}

/// How long a CLI has to list its models before it's left.
const LIST_WAIT: Duration = Duration::from_secs(10);

/// Claude Code's arguments for asking its models: a task's lockdown (runner::claude_args), with no
/// MCP server, so its default is the one a task gets and nothing of the user's is loaded.
const CLAUDE_ASK: [&str; 13] = [
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--tools",
    "",
    "--strict-mcp-config",
    "--setting-sources",
    "",
    "--disable-slash-commands",
    "--no-session-persistence",
];

/// The models Claude lists for the signed-in account, which change with their plan and Claude's
/// version: the answer to its stream-json initialize, which starts no turn and sends nothing to a
/// model (checked on 2.1.322). Its input closes after that one request, so a Claude too old to
/// answer ends at once; it's left after LIST_WAIT either way.
fn claude_models(program: &Path, path: Option<&str>) -> Option<Models> {
    let mut command = command(program);
    command
        .args(CLAUDE_ASK)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let mut child = command.spawn().ok()?;
    let mut stdin = child.stdin.take().expect("piped");
    let lines = lines_of(child.stdout.take().expect("piped"));
    let ask = serde_json::json!({
        "type": "control_request",
        "request_id": "models",
        "request": { "subtype": "initialize" },
    });
    let asked = writeln!(stdin, "{ask}").is_ok();
    drop(stdin);
    let deadline = Instant::now() + LIST_WAIT;
    let models = asked
        .then(|| {
            loop {
                let line = lines
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .ok()?;
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if message
                    .pointer("/response/request_id")
                    .and_then(Value::as_str)
                    == Some("models")
                {
                    return claude_listed(&message);
                }
            }
        })
        .flatten();
    _ = child.kill();
    _ = child.wait();
    models
}

/// The models in Claude's answer to initialize, or None from a Claude too old to list them. Its
/// first is "default", which names no model but resolves to one, as another of the list does.
pub fn claude_listed(answer: &Value) -> Option<Models> {
    fn id(model: &Value) -> Option<&str> {
        model.get("value").and_then(Value::as_str)
    }
    fn name(model: &Value) -> Option<&str> {
        model
            .get("displayName")
            .and_then(Value::as_str)
            .or(id(model))
    }
    fn resolved(model: &Value) -> Option<&str> {
        model.get("resolvedModel").and_then(Value::as_str)
    }
    let models = answer.pointer("/response/response/models")?.as_array()?;
    let (defaults, listed): (Vec<&Value>, Vec<&Value>) = models
        .iter()
        .partition(|model| id(model) == Some("default"));
    let default = (defaults.first().and_then(|model| resolved(model)))
        .and_then(|model| listed.iter().find(|each| resolved(each) == Some(model)))
        .and_then(|model| name(model))
        .map(String::from);
    let listed = (listed.into_iter())
        .filter_map(|model| Some((id(model)?.to_string(), name(model)?.to_string())))
        .collect();
    Some(Models { listed, default })
}

/// `output`'s lines as they come, read on a thread of their own.
fn lines_of(output: impl std::io::Read + Send + 'static) -> mpsc::Receiver<String> {
    let (send, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    lines
}

/// The models Codex lists for the signed-in user, which change with their plan and Codex's
/// version: asked of its app-server, which starts no thread for it, and left after LIST_WAIT.
fn codex_models(program: &Path, path: Option<&str>) -> Option<Models> {
    let mut command = command(program);
    command
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let mut child = command.spawn().ok()?;
    let mut stdin = child.stdin.take().expect("piped");
    let lines = lines_of(child.stdout.take().expect("piped"));
    let mut say = |message: Value| writeln!(stdin, "{message}").ok();
    let deadline = Instant::now() + LIST_WAIT;
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
            let line = lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .ok()?;
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match message.get("id").and_then(Value::as_u64) {
                Some(1) => {
                    say(serde_json::json!({ "method": "initialized" }))?;
                    say(
                        serde_json::json!({ "id": 2, "method": "model/list", "params": { "limit": 100 } }),
                    )?;
                }
                Some(2) => return Some(codex_listed(&message)),
                _ => {}
            }
        }
    })();
    // Its input closes first: killing an npm shim's cmd.exe leaves the node it started, which
    // ends as its input does.
    drop(stdin);
    _ = child.kill();
    _ = child.wait();
    models
}

/// The models in Codex's answer to model/list, hidden ones left out, and the one it marks its
/// default named.
pub fn codex_listed(answer: &Value) -> Models {
    let listed = answer.pointer("/result/data").and_then(Value::as_array);
    let named = |model: &Value| {
        let id = model.get("id").or_else(|| model.get("model"))?.as_str()?;
        let name = model
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(id);
        Some((id.to_string(), name.to_string()))
    };
    let default = (listed.into_iter().flatten())
        .find(|model| model.get("isDefault").and_then(Value::as_bool) == Some(true))
        .and_then(named)
        .map(|(_, name)| name);
    let listed = (listed.into_iter().flatten())
        .filter(|model| {
            !model
                .get("hidden")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(named)
        .collect();
    Models { listed, default }
}

/// Whether `claude auth status --json` says the user is signed in.
pub fn claude_signed_in(said: &str) -> bool {
    let status: Value = serde_json::from_str(said.trim()).unwrap_or_default();
    status
        .get("loggedIn")
        .and_then(Value::as_bool)
        .unwrap_or(false)
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
/// itself never runs through it, as an alias there could change its flags.
#[cfg(not(windows))]
fn login_path() -> &'static Option<String> {
    static PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let output = command(shell)
            .args(["-l", "-c", r#"printf "%s" "$PATH""#])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!path.is_empty()).then_some(path)
    })
}
