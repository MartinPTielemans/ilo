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
use std::time::Instant;

use serde_json::Value;

use super::runner::Cli;

/// What the browser last learned of a CLI.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ready {
    /// Not looked yet.
    #[default]
    Unknown,
    NotFound,
    /// Installed, but older than the one the browser runs (AP27): no thread starts on it.
    NeedsUpdate,
    SignedOut,
    SignedIn,
}

impl Ready {
    fn id(self) -> &'static str {
        match self {
            Ready::Unknown => "unknown",
            Ready::NotFound => "not-found",
            Ready::NeedsUpdate => "needs-update",
            Ready::SignedOut => "signed-out",
            Ready::SignedIn => "signed-in",
        }
    }

    fn from_id(id: &str) -> Ready {
        match id {
            "not-found" => Ready::NotFound,
            "needs-update" => Ready::NeedsUpdate,
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
    /// The plan the account is on, where the CLI says, as its row shows it: never saved.
    pub plan: Option<String>,
}

/// The models a CLI lists for the signed-in account, in the CLI's order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Models {
    /// Each as id and name, its own default left out: the pop-up's Default stands for it.
    pub listed: Vec<(String, String)>,
    /// Which model its default is, by name, where the CLI says.
    pub default: Option<String>,
    /// That model by id, the menu's checked row while nothing else is picked (Z54).
    pub default_id: Option<String>,
    /// The models it lists but keeps out of its own picker, which the menu offers behind
    /// More Models (Z54c), as id and name.
    pub more: Vec<(String, String)>,
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
                "default_id": models.default_id,
                "more": models.more.iter().map(|(id, name)| [id, name]).collect::<Vec<_>>(),
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
            plan: None,
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
                default_id: (listed.get("default_id").and_then(Value::as_str)).map(String::from),
                more: pairs(listed.get("more").unwrap_or(&Value::Null)),
            });
        }
        let listed = pairs(saved.get("models")?);
        (!listed.is_empty()).then_some(Models {
            listed,
            ..Models::default()
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
            ..Default::default()
        };
    };
    let (ready, plan, models) = match cli {
        Cli::Claude => {
            let mut command = command(&program);
            command.args(["auth", "status", "--json"]);
            if let Some(path) = &path {
                command.env("PATH", path);
            }
            match command.stdin(Stdio::null()).stderr(Stdio::null()).output() {
                Err(error) => {
                    log!("browser: {} couldn't run: {error}", cli.id());
                    (Ready::NotFound, None, None)
                }
                Ok(output) if claude_signed_in(&String::from_utf8_lossy(&output.stdout)) => {
                    (Ready::SignedIn, None, None)
                }
                Ok(_) => (Ready::SignedOut, None, None),
            }
        }
        Cli::Codex => codex_look(&program, path.as_deref()),
    };
    // check.sh reads this line.
    log!(
        "browser: task agent {} is {ready:?} at {}",
        cli.id(),
        program.display()
    );
    let models = match (cli, ready) {
        (Cli::Claude, Ready::SignedIn) => boxed_models(&program, path.as_deref()),
        (Cli::Codex, _) => models,
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
        plan,
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
    let installed = || installed_at(cli, &super::clients::home());
    (find(cli.program(), &search).or_else(installed), path)
}

/// Where its maker's installer puts `cli` in `home`, which a login shell's PATH may not have yet, as
/// Claude Code's installer only says to add: ~/.local/bin (install.rs).
#[cfg(not(windows))]
fn installed_at(cli: Cli, home: &Path) -> Option<PathBuf> {
    let program = (cli.installs()).then(|| home.join(".local/bin").join(cli.program()))?;
    is_executable(&program).then_some(program)
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

/// The models Claude lists, asked where Claude Code runs (wall.rs `runs`): in a box of the ask's
/// own, in a folder of its own that goes with it, or outside one where there's no sandbox. Where
/// it doesn't run, none.
fn boxed_models(program: &Path, path: Option<&str>) -> Option<Models> {
    use super::wall::{self, Runs};
    match wall::runs(Cli::Claude, wall::available()) {
        Runs::Unboxed => claude_models(program, path, None),
        Runs::Not => None,
        Runs::Boxed => {
            static ASKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let at = ASKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let name = format!("ilo-models-{}-{at}", std::process::id());
            let folder = std::env::temp_dir().join(name);
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            builder.create(&folder).ok()?;
            let models = (wall::without_tools(Cli::Claude, program, &folder).ok())
                .and_then(|wall| claude_models(program, path, Some((&wall, &folder))));
            _ = std::fs::remove_dir_all(&folder);
            models
        }
    }
}

/// The models Claude lists for the signed-in account, which change with their plan and Claude's
/// version: the answer to its stream-json initialize, which starts no turn and sends nothing to a
/// model (checked on 2.1.322). Its input closes after that one request, so a Claude too old to
/// answer ends at once; it's left after LIST_WAIT either way. In `boxed`, a box and the folder it
/// was made for, if given, whose system's word that it's inside comes before the request.
fn claude_models(
    program: &Path,
    path: Option<&str>,
    boxed: Option<(&super::wall::Wall, &Path)>,
) -> Option<Models> {
    let wall = boxed.map(|(wall, _)| wall);
    let mut command = match boxed {
        Some((wall, folder)) => {
            let mut command = wall.command(program);
            command.current_dir(folder);
            command
        }
        None => command(program),
    };
    if let Some(wall) = wall {
        if let Some(tmp) = wall.tmp() {
            command.env(super::runner::CLAUDE_TMP, tmp);
        }
        if let Some(port) = wall.port() {
            let proxy = format!("http://127.0.0.1:{port}");
            command.envs(super::runner::PROXY.map(|name| (name, proxy.as_str())));
        }
        command.envs(super::runner::CLAUDE_WALLED.map(|name| (name, "1")));
    }
    command
        .args(CLAUDE_ASK)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let (mut child, rising) = match wall {
        Some(wall) => wall.start(&mut command).ok()?,
        None => (command.spawn().ok()?, None),
    };
    if let Some(rising) = rising {
        let id = child.id();
        let inside = |()| wall.map_or(Ok(()), |wall| wall.control(id));
        if rising.risen().and_then(inside).is_err() {
            _ = child.kill();
            _ = child.wait();
            return None;
        }
    }
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
    fn resolved(model: &Value) -> Option<&str> {
        model.get("resolvedModel").and_then(Value::as_str)
    }
    fn name(model: &Value) -> Option<String> {
        (resolved(model).and_then(claude_version_name))
            .or_else(|| model.get("displayName")?.as_str().map(String::from))
            .or_else(|| id(model).map(String::from))
    }
    let models = answer.pointer("/response/response/models")?.as_array()?;
    let (defaults, listed): (Vec<&Value>, Vec<&Value>) = models
        .iter()
        .partition(|model| id(model) == Some("default"));
    let default = (defaults.first().and_then(|model| resolved(model)))
        .and_then(|model| listed.iter().find(|each| resolved(each) == Some(model)));
    let default_id = default.and_then(|model| id(model)).map(String::from);
    let default = default.and_then(|model| name(model));
    let listed = (listed.into_iter())
        .filter_map(|model| Some((id(model)?.to_string(), name(model)?)))
        .collect();
    Some(Models {
        listed,
        default,
        default_id,
        more: Vec::new(),
    })
}

/// A Claude model's name with its version, read from the id it resolves to: `claude-opus-5-5` is
/// `Opus 5.5`, with a release date at its end dropped. None for an id of any other shape, such as
/// one with a suffix in brackets, which then keeps the name Claude gives it: nothing is hand-kept.
pub fn claude_version_name(resolved: &str) -> Option<String> {
    let mut parts = resolved.strip_prefix("claude-")?.split('-');
    let family = parts
        .next()
        .filter(|family| !family.is_empty() && family.chars().all(|c| c.is_ascii_lowercase()))?;
    let mut version: Vec<&str> = parts.collect();
    if version.last().is_some_and(|last| last.len() == 8) {
        version.pop();
    }
    let numbers =
        |part: &&str| (1..=2).contains(&part.len()) && part.chars().all(|c| c.is_ascii_digit());
    if version.is_empty() || !version.iter().all(numbers) {
        return None;
    }
    let mut name = family.to_string();
    name[..1].make_ascii_uppercase();
    Some(format!("{name} {}", version.join(".")))
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

/// Whether Codex is signed in, its plan, and the models it lists for the account, which change with
/// the plan and Codex's version: asked of its app-server, as its own sign-in is, which starts no
/// thread for it, and left after LIST_WAIT. `account/read` says an expired sign-in is out, where
/// `codex login status` said in (#4099).
fn codex_look(program: &Path, path: Option<&str>) -> (Ready, Option<String>, Option<Models>) {
    // One older than the browser runs says so before any thread starts, rather than failing it.
    let mut version = command(program);
    version.arg("--version");
    if let Some(path) = path {
        version.env("PATH", path);
    }
    match version.stdin(Stdio::null()).stderr(Stdio::null()).output() {
        Err(error) => {
            log!("browser: codex couldn't run: {error}");
            return (Ready::NotFound, None, None);
        }
        Ok(output) if !super::codex::fits_version(&String::from_utf8_lossy(&output.stdout)) => {
            return (Ready::NeedsUpdate, None, None);
        }
        Ok(_) => {}
    }
    let mut command = command(program);
    command
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            log!("browser: codex couldn't run: {error}");
            return (Ready::NotFound, None, None);
        }
    };
    let mut stdin = child.stdin.take().expect("piped");
    let lines = lines_of(child.stdout.take().expect("piped"));
    let mut say = |message: Value| writeln!(stdin, "{message}").ok();
    let deadline = Instant::now() + LIST_WAIT;
    let (mut ready, mut plan) = (Ready::SignedOut, None);
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
                    say(serde_json::json!({ "id": 2, "method": "account/read", "params": {} }))?;
                }
                Some(2) => {
                    (ready, plan) = codex_account(&message);
                    if ready != Ready::SignedIn {
                        return None;
                    }
                    say(
                        serde_json::json!({ "id": 3, "method": "model/list", "params": { "limit": 100 } }),
                    )?;
                }
                Some(3) => return Some(codex_listed(&message)),
                _ => {}
            }
        }
    })();
    // Its input closes first: killing an npm shim's cmd.exe leaves the node it started, which
    // ends as its input does.
    drop(stdin);
    _ = child.kill();
    _ = child.wait();
    (ready, plan, models)
}

/// Whether Codex's answer to `account/read` says it can run a task, and the ChatGPT plan it's on.
/// Only the account's kind and plan are read, never its email. A provider that needs no OpenAI
/// sign-in runs one signed out.
pub fn codex_account(answer: &Value) -> (Ready, Option<String>) {
    let Some(result) = answer.get("result") else {
        return (Ready::SignedOut, None);
    };
    let account = result.get("account").filter(|account| account.is_object());
    let needs = (result.get("requiresOpenaiAuth").and_then(Value::as_bool)).unwrap_or(true);
    let ready = match account.is_some() || !needs {
        true => Ready::SignedIn,
        false => Ready::SignedOut,
    };
    let plan = (account.and_then(|account| account.get("planType")))
        .and_then(Value::as_str)
        .and_then(plan_name);
    (ready, plan)
}

/// A plan's name as the row says it, for the plans the makers name: none for another.
pub fn plan_name(plan: &str) -> Option<String> {
    let name = match plan.to_ascii_lowercase().as_str() {
        "free" => "Free",
        "go" => "Go",
        "plus" => "Plus",
        "pro" => "Pro",
        "max" => "Max",
        "team" => "Team",
        "business" => "Business",
        "enterprise" => "Enterprise",
        "edu" => "Edu",
        _ => return None,
    };
    Some(name.to_string())
}

/// The models in Codex's answer to model/list, the hidden ones apart for More Models, and the one
/// it marks its default, by id and name.
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
        .and_then(named);
    let hidden = |model: &&Value| model.get("hidden").and_then(Value::as_bool) == Some(true);
    let (more, shown): (Vec<&Value>, Vec<&Value>) =
        (listed.into_iter().flatten()).partition(hidden);
    Models {
        listed: shown.into_iter().filter_map(named).collect(),
        default_id: default.as_ref().map(|(id, _)| id.clone()),
        default: default.map(|(_, name)| name),
        more: more.into_iter().filter_map(named).collect(),
    }
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
