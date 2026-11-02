//! A task's CLI: the user's own `claude` or `codex`, run as a child process with the browser's MCP
//! server as its only tools (Agent task). Nothing of the user's setup reaches it: no shell, files,
//! web search, MCP servers, plugins, hooks, skills, memory or settings files, and it runs in an
//! empty directory of its own. Its output is read on a thread that waits on the pipe, never polled.
//!
//! Claude Code runs in print mode with stream-json both ways, so a reply is one more line on its
//! stdin. Codex runs as `codex app-server`, which speaks newline JSON-RPC (see codex.rs).

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use futures::channel::mpsc::UnboundedSender;
use serde_json::{Value, json};

/// A CLI that runs tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cli {
    Claude,
    Codex,
}

impl Cli {
    pub const ALL: [Cli; 2] = [Cli::Claude, Cli::Codex];

    pub fn id(self) -> &'static str {
        match self {
            Cli::Claude => "claude",
            Cli::Codex => "codex",
        }
    }

    pub fn from_id(id: &str) -> Option<Cli> {
        Cli::ALL.into_iter().find(|cli| cli.id() == id)
    }

    /// What the user calls it, in text only: agents have no maker's logo or colour.
    pub fn name(self) -> &'static str {
        match self {
            Cli::Claude => "Claude",
            Cli::Codex => "Codex",
        }
    }

    /// Its executable's file name. npm puts a .cmd shim on Windows, as clients.rs runs it.
    pub fn program(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Cli::Claude, false) => "claude",
            (Cli::Claude, true) => "claude.cmd",
            (Cli::Codex, false) => "codex",
            (Cli::Codex, true) => "codex.cmd",
        }
    }

    /// The command that signs the user in, which ilo never runs itself.
    pub fn sign_in(self) -> &'static str {
        match self {
            Cli::Claude => "claude auth login",
            Cli::Codex => "codex login",
        }
    }

    /// The command that installs it.
    pub fn install(self) -> &'static str {
        match self {
            Cli::Claude => "npm install -g @anthropic-ai/claude-code",
            Cli::Codex => "npm install -g @openai/codex",
        }
    }

    /// The models its Model pop-up offers after its own default, which is the empty id: ilo runs
    /// the CLI without the user's settings files, so the default is the CLI's, not theirs.
    pub fn models(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Cli::Claude => &[("opus", "Opus"), ("sonnet", "Sonnet"), ("haiku", "Haiku")],
            Cli::Codex => &[],
        }
    }
}

/// The browser's MCP server as the CLI starts it: the bridge, with the task's token in its
/// environment.
#[derive(Clone, Debug)]
pub struct Server {
    pub command: String,
    pub args: Vec<String>,
    pub token: String,
}

/// The name the CLI knows the server by, which its tools' names carry.
pub const SERVER: &str = "ilo";

/// What a task's CLI starts with.
#[derive(Clone, Debug)]
pub struct Launch {
    pub cli: Cli,
    pub program: PathBuf,
    /// The PATH of the user's login shell, which a CLI installed as a script needs for its
    /// interpreter.
    pub path: Option<String>,
    /// An empty directory of its own, which it runs in.
    pub dir: PathBuf,
    /// Where Claude's MCP config goes, readable by the user alone, as it holds the token.
    pub config: PathBuf,
    pub server: Server,
    /// The model, or None for the CLI's default.
    pub model: Option<String>,
    /// What it's told about working in the browser, after its own system prompt.
    pub instructions: String,
    /// The tools it may call, as the server lists them.
    pub tools: Vec<String>,
}

/// What a task's CLI says, as the reader thread hands it to the UI thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// The agent said something to the user.
    Said(String),
    /// Its turn ended, and it waits for the user's next message: well, or with what went wrong.
    Ended(Option<Failure>),
    /// The process has gone: its output closed. What it last said on stderr.
    Gone(String),
}

/// Why a turn or a run ended badly.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// The user's plan is at its limit, until the time given, in seconds since 1970, if the CLI
    /// says.
    Limit(Option<u64>),
    SignedOut,
    /// The CLI said why, in its words.
    Other(String),
}

/// A line of output, for the task it's from.
pub struct Event {
    pub agent: String,
    pub out: Out,
}

/// A task's running CLI.
pub struct Process {
    cli: Cli,
    child: Option<Child>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    id: u32,
    /// Codex's thread and turn, once it has them.
    codex: Option<Arc<super::codex::State>>,
}

impl Process {
    /// Starts the CLI, which reads `first` as the user's first message.
    pub fn start(
        launch: &Launch,
        agent: &str,
        first: &str,
        events: UnboundedSender<Event>,
    ) -> std::io::Result<Process> {
        std::fs::create_dir_all(&launch.dir)?;
        let mut command = Command::new(&launch.program);
        match launch.cli {
            Cli::Claude => {
                super::clients::write_private(&launch.config, claude_config(&launch.server).as_bytes())?;
                command.args(claude_args(launch));
            }
            Cli::Codex => _ = command.arg("app-server"),
        }
        command
            .current_dir(&launch.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(path) = &launch.path {
            command.env("PATH", path);
        }
        // The token reaches the bridge through Codex's environment, and through Claude's config.
        if launch.cli == Cli::Codex {
            command.env(super::bridge::TOKEN, &launch.server.token);
        }
        let mut child = command.spawn()?;
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let codex = (launch.cli == Cli::Codex).then(|| Arc::new(super::codex::State::default()));
        let id = child.id();
        let process =
            Process { cli: launch.cli, child: Some(child), id, stdin: stdin.clone(), codex: codex.clone() };
        let said = Arc::new(Mutex::new(String::new()));
        let err = std::thread::Builder::new().name("task-stderr".into()).spawn({
            let said = said.clone();
            move || keep_tail(stderr, &said)
        })?;
        let agent = agent.to_string();
        let codex_launch = launch.clone();
        std::thread::Builder::new().name("task-output".into()).spawn(move || {
            let mut turn = Turn::default();
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let outs = match &codex {
                    Some(state) => super::codex::read(&message, state, &stdin, &codex_launch),
                    None => claude_read(&message, &mut turn),
                };
                for out in outs {
                    if events.unbounded_send(Event { agent: agent.clone(), out }).is_err() {
                        return;
                    }
                }
            }
            _ = err.join();
            let said = std::mem::take(&mut *said.lock().unwrap());
            _ = events.unbounded_send(Event { agent, out: Out::Gone(said) });
        })?;
        let mut process = process;
        match launch.cli {
            Cli::Claude => process.say(first)?,
            Cli::Codex => {
                let state = process.codex.as_ref().expect("Codex's");
                super::codex::begin(&process.stdin, state, first)?
            }
        }
        Ok(process)
    }

    /// Sends the user's message, which the agent reads once its turn under way ends.
    pub fn say(&mut self, text: &str) -> std::io::Result<()> {
        match (self.cli, &self.codex) {
            (Cli::Codex, Some(state)) => super::codex::say(&self.stdin, state, text),
            _ => {
                let message = json!({
                    "type": "user",
                    "message": { "role": "user", "content": text },
                });
                write_line(&self.stdin, &message)
            }
        }
    }

    /// Stops the turn under way at once, leaving the process to take the next message.
    pub fn interrupt(&mut self) {
        let sent = match (self.cli, &self.codex) {
            (Cli::Codex, Some(state)) => super::codex::interrupt(&self.stdin, state),
            _ => write_line(
                &self.stdin,
                &json!({
                    "type": "control_request",
                    "request_id": format!("stop-{}", self.id),
                    "request": { "subtype": "interrupt" },
                }),
            ),
        };
        if let Err(error) = sent {
            log!("browser: task couldn't be interrupted: {error}");
        }
    }
}

impl Drop for Process {
    /// Ends the CLI: its input closes, which ends it between turns, and it's killed, for a turn
    /// under way. A thread waits for it, so it leaves no zombie behind.
    fn drop(&mut self) {
        self.stdin.lock().unwrap().take();
        if let Some(mut child) = self.child.take() {
            _ = child.kill();
            _ = std::thread::Builder::new().name("task-reap".into()).spawn(move || child.wait());
        }
    }
}

/// Writes `message` as a line on the CLI's input.
pub(super) fn write_line(stdin: &Mutex<Option<ChildStdin>>, message: &Value) -> std::io::Result<()> {
    let mut stdin = stdin.lock().unwrap();
    let Some(stdin) = stdin.as_mut() else {
        return Err(std::io::Error::other("the task has ended"));
    };
    let mut line = message.to_string();
    line.push('\n');
    stdin.write_all(line.as_bytes())?;
    stdin.flush()
}

/// Keeps the last part of what `from` says, which a run that fails says why in.
fn keep_tail(mut from: impl Read, said: &Mutex<String>) {
    const KEEP: usize = 2000;
    let mut buffer = [0u8; 4096];
    while let Ok(read) = from.read(&mut buffer) {
        if read == 0 {
            break;
        }
        let mut said = said.lock().unwrap();
        said.push_str(&String::from_utf8_lossy(&buffer[..read]));
        if said.len() > KEEP {
            let mut cut = said.len() - KEEP;
            while !said.is_char_boundary(cut) {
                cut += 1;
            }
            said.drain(..cut);
        }
    }
}

/// Claude's MCP config: the browser's server alone.
pub fn claude_config(server: &Server) -> String {
    let mut env = serde_json::Map::new();
    env.insert(super::bridge::TOKEN.into(), json!(server.token));
    json!({
        "mcpServers": {
            SERVER: {
                "type": "stdio",
                "command": server.command,
                "args": server.args,
                "env": env,
            }
        }
    })
    .to_string()
}

/// Claude Code's arguments, each checked on 2.1.316 with a stub MCP server offering one tool: its
/// init message then listed that tool alone, no slash commands or skills, and only its built-in
/// plugins and agent types, which no tool could reach.
pub fn claude_args(launch: &Launch) -> Vec<String> {
    let allowed: Vec<String> =
        launch.tools.iter().map(|tool| format!("mcp__{SERVER}__{tool}")).collect();
    let mut args: Vec<String> = [
        // Print mode, reading the user's messages as stream-json, so a reply is one more line.
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        // None of its own tools: no shell, files, web fetch or search.
        "--tools",
        "",
        // The browser's server alone, none of the user's.
        "--strict-mcp-config",
        "--mcp-config",
    ]
    .map(String::from)
    .into();
    args.push(launch.config.to_string_lossy().into_owned());
    args.extend(
        [
            // No settings files: the user's hooks, permissions and plugins stay out.
            "--setting-sources",
            "",
            // Anything not allowed here is declined without asking, as no one could answer.
            "--permission-mode",
            "dontAsk",
            "--permission-prompts",
            "none",
            // The user's commands and skills stay out.
            "--disable-slash-commands",
            // The conversation lives as long as the browser runs it.
            "--no-session-persistence",
            "--allowedTools",
        ]
        .map(String::from),
    );
    args.push(allowed.join(","));
    if let Some(model) = launch.model.as_ref().filter(|model| !model.is_empty()) {
        args.extend(["--model".into(), model.clone()]);
    }
    args.extend(["--append-system-prompt".into(), launch.instructions.clone()]);
    args
}

/// What a turn of Claude's has shown so far that its result needs: a limit it hit.
#[derive(Default)]
pub struct Turn {
    limit: Option<Option<u64>>,
}

/// What a line of Claude's stream-json output says to the user, if anything.
pub fn claude_read(message: &Value, turn: &mut Turn) -> Vec<Out> {
    let kind = message.get("type").and_then(Value::as_str).unwrap_or_default();
    match kind {
        "assistant" => {
            let content = message.pointer("/message/content").and_then(Value::as_array);
            (content.into_iter().flatten())
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| Out::Said(text.to_string()))
                .collect()
        }
        "rate_limit_event" => {
            let info = message.get("rate_limit_info").unwrap_or(&Value::Null);
            if info.get("status").and_then(Value::as_str) == Some("rejected") {
                turn.limit = Some(info.get("resetsAt").and_then(Value::as_u64));
            }
            Vec::new()
        }
        "result" => {
            let limit = turn.limit.take();
            let error = message.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            let text = message.get("result").and_then(Value::as_str).unwrap_or_default();
            let failure = match (limit, error) {
                (Some(resets), _) => Some(Failure::Limit(resets)),
                (None, false) => None,
                (None, true) => Some(claude_failure(text, message)),
            };
            vec![Out::Ended(failure)]
        }
        _ => Vec::new(),
    }
}

/// Why Claude's turn failed, from its result's words.
fn claude_failure(text: &str, message: &Value) -> Failure {
    let lower = text.to_lowercase();
    if lower.contains("/login") || lower.contains("not logged in") || lower.contains("invalid api key") {
        return Failure::SignedOut;
    }
    if lower.contains("limit") {
        // Older versions end the text with the reset's time: "…limit reached|1730000000".
        let resets = text.rsplit_once('|').and_then(|(_, at)| at.trim().parse().ok());
        return Failure::Limit(resets);
    }
    let subtype = message.get("subtype").and_then(Value::as_str).unwrap_or_default();
    Failure::Other(match text.is_empty() {
        true => format!("It stopped ({subtype})."),
        false => text.to_string(),
    })
}

#[cfg(test)]
mod tests;
