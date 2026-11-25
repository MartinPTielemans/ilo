//! A task's CLI: the user's own `claude` or `codex`, run as a child process with the browser's MCP
//! server as its only tools (Agent task). Nothing of the user's setup reaches it: no shell, files,
//! web search, MCP servers, plugins, hooks, skills, memory or settings files, and it runs in an
//! empty directory of its own. Its output is read on a thread that waits on the pipe, never polled.
//!
//! Claude Code runs in print mode with stream-json both ways, so a reply is one more line on its
//! stdin. Codex runs as `codex app-server`, which speaks newline JSON-RPC (see codex.rs).

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;

use futures::channel::mpsc::UnboundedSender;
use serde_json::{Value, json};

use super::kept::Keep;
use super::wall::{Wall, WallRefused, leftovers};

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

    /// Its glyph, the same in Settings › Agents and ⌘T's ask: a terminal, as each is a CLI.
    pub fn glyph(self) -> &'static str {
        match self {
            Cli::Claude => "icons/terminal.svg",
            Cli::Codex => "icons/square-terminal.svg",
        }
    }

    /// Its executable's name, which on Windows takes PATHEXT's extensions: claude.exe from Claude
    /// Code's installer, or npm's claude.cmd (ready.rs).
    pub fn program(self) -> &'static str {
        match self {
            Cli::Claude => "claude",
            Cli::Codex => "codex",
        }
    }

    /// The models its Model pop-up offers after its own default while the CLI hasn't listed its
    /// own: Claude's aliases, which every Claude Code takes, for one too old to answer initialize
    /// with its list (ready::claude_models).
    pub fn unlisted_models(self) -> &'static [(&'static str, &'static str)] {
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
    /// Whether it gets its helper tool (#4102), which only Claude's arguments read.
    pub helpers: bool,
    /// The wall it starts inside (#4101), or None for a start that asked for none.
    pub wall: Option<Arc<Wall>>,
}

/// What a task's CLI says, as the reader thread hands it to the UI thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// The agent said something to the user.
    Said(String),
    /// What the agent has written so far of the message it's writing, as it comes: Claude's,
    /// which streams. Empty as a new message starts.
    Writing(String),
    /// The message being written goes on to call a tool, so what it said is said along the way.
    Calling,
    /// The agent thought, for as long as `took`: its reasoning, as its CLI sends it.
    Thought { text: String, took: Duration },
    /// Its turn ended, and it waits for the user's next message: well, or with what went wrong.
    Ended(Option<Failure>),
    /// The process has gone: its output closed. What it last said on stderr.
    Gone(String),
    /// A start inside a wall with files: the assistant is inside it, or nothing started and why.
    /// Before anything else of the run's, and Gone follows a no.
    Walled(Result<(), WallRefused>),
    /// What its output tells of a helper (#4102), by the call of the task's that asked for it.
    Helper { id: String, told: Helper },
    /// Its CLI says a helper started, by the call of the task's that asked for it if it names
    /// one, `deep` levels under the task, or 0 if it doesn't say how deep: what ilo counts for
    /// the limit, and from when the helper shows.
    Started { id: Option<String>, deep: u64 },
    /// Its output shows a helper ilo doesn't allow, which stops the task.
    NotAllowed(super::task::ilo::HelpersLimit),
    /// A run's first line lists its kinds of helper: whether each is ilo's.
    Kinds { ours: bool },
}

/// What a task's output tells of one of its helpers, for its line in the conversation. A label:
/// nothing ilo allows or refuses reads it.
#[derive(Clone, Debug, PartialEq)]
pub enum Helper {
    /// The task asked for it, by the name it gave it: one line of plain text, cut short, or
    /// empty.
    Asked(String),
    /// It calls one of the browser's tools on this tab.
    On(u64),
    /// It calls the browser's tool that opens a tab, which names none: the page is its own once
    /// it's open.
    Opens,
    /// It calls a tool that names no tab.
    Calls,
    /// Its CLI says it ended, however it did.
    Ended,
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
    /// The run it's from, by its process. A task ilo started again hears its run under way
    /// alone, so what the one before still had to say isn't taken for it.
    pub run: u32,
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
    /// Whether it's still the wall's launcher, which reads nothing it's sent.
    rising: Arc<AtomicBool>,
    /// The wall of a walled start, whose group and leftovers its end ends.
    walled: Option<Arc<Wall>>,
}

impl Process {
    /// Starts the CLI, which reads `first` as the user's first message, on a run that keeps
    /// nothing of its conversation.
    pub fn start(
        launch: &Launch,
        agent: &str,
        first: &str,
        events: UnboundedSender<Event>,
    ) -> std::io::Result<Process> {
        Self::start_kept(launch, &Keep::Nothing, agent, first, events)
    }

    /// Starts the CLI on a run that keeps what `keep` says (kept.rs decides), which reads `first`
    /// as the user's first message, or as their next in a conversation it goes on with.
    pub fn start_kept(
        launch: &Launch,
        keep: &Keep,
        agent: &str,
        first: &str,
        events: UnboundedSender<Event>,
    ) -> std::io::Result<Process> {
        std::fs::create_dir_all(&launch.dir)?;
        // A kept run's folder isn't beside its config, so the config's own is made too.
        if let Some(folder) = launch.config.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let mut command = match &launch.wall {
            Some(wall) => wall.command(&launch.program),
            None => crate::child::command(&launch.program),
        };
        match launch.cli {
            Cli::Claude => {
                super::clients::write_private(
                    &launch.config,
                    claude_config(&launch.server).as_bytes(),
                )?;
                super::clients::write_private(
                    &instructions_file(launch),
                    launch.instructions.as_bytes(),
                )?;
                command.args(super::kept::claude_args(claude_args(launch), keep));
            }
            Cli::Codex => _ = command.arg("app-server"),
        }
        command
            .current_dir(&launch.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        environment(&mut command, launch);
        let (mut child, rising) = match &launch.wall {
            Some(wall) => wall.start(&mut command)?,
            None => (command.spawn()?, None),
        };
        let walled = rising.is_some();
        let wall = launch.wall.clone().filter(|_| walled);
        let still_rising = Arc::new(AtomicBool::new(walled));
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let codex = (launch.cli == Cli::Codex).then(|| Arc::new(super::codex::State::default()));
        let id = child.id();
        let process = Process {
            cli: launch.cli,
            child: Some(child),
            id,
            stdin: stdin.clone(),
            codex: codex.clone(),
            rising: still_rising.clone(),
            walled: wall.clone(),
        };
        let said = Arc::new(Mutex::new(String::new()));
        let err = std::thread::Builder::new()
            .name("task-stderr".into())
            .spawn({
                let said = said.clone();
                move || keep_tail(stderr, &said)
            })?;
        let agent = agent.to_string();
        let codex_launch = launch.clone();
        std::thread::Builder::new()
            .name("task-output".into())
            .spawn(move || {
                // The one wait for the wall, here, as this thread may wait and the UI's may not.
                if let Some(rising) = rising {
                    let risen = rising.risen();
                    // The control, before the program is taken to be inside: the system's word.
                    let inside = |()| wall.as_ref().map_or(Ok(()), |wall| wall.control(id));
                    let out = Out::Walled(risen.and_then(inside));
                    still_rising.store(false, Ordering::Release);
                    let agent = agent.clone();
                    let event = Event {
                        agent,
                        run: id,
                        out,
                    };
                    if events.unbounded_send(event).is_err() {
                        return;
                    }
                }
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
                        if events
                            .unbounded_send(Event {
                                agent: agent.clone(),
                                run: id,
                                out,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                _ = err.join();
                let said = std::mem::take(&mut *said.lock().unwrap());
                _ = events.unbounded_send(Event {
                    agent,
                    run: id,
                    out: Out::Gone(said),
                });
            })?;
        let mut process = process;
        let began = match launch.cli {
            Cli::Claude => process.say(first),
            Cli::Codex => {
                let state = process.codex.as_ref().expect("Codex's");
                super::codex::begin(&process.stdin, state, first)
            }
        };
        // A launcher that ended has closed its input, and Walled says why: one account of it.
        if !walled {
            began?;
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

    /// Which run it is, as each of its lines says.
    pub fn run(&self) -> u32 {
        self.id
    }

    /// Stops the turn under way at once, leaving the process to take the next message.
    pub fn interrupt(&mut self) {
        // Still the wall's launcher, which reads no line: Stop ends it, and the wait for it.
        if self.rising.load(Ordering::Acquire) {
            if let Some(child) = &mut self.child {
                _ = child.kill();
            }
            return;
        }
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
            // A walled start's whole group first, while its leader isn't reaped and so no other
            // process can have the group's id. What left the group is the sweep's, after.
            let walled = self.walled.take();
            if walled.is_some() {
                leftovers::System::end_group(&leftovers::Os, self.id);
            }
            _ = child.kill();
            _ = std::thread::Builder::new()
                .name("task-reap".into())
                .spawn(move || {
                    _ = child.wait();
                    if let Some(wall) = walled {
                        wall.sweep_after();
                    }
                });
        }
    }
}

/// Writes `message` as a line on the CLI's input.
pub(super) fn write_line(
    stdin: &Mutex<Option<ChildStdin>>,
    message: &Value,
) -> std::io::Result<()> {
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

/// What the CLI's process is started with beyond the browser's own environment, each set on the
/// process, so ilo's value is the one it reads.
fn environment(command: &mut std::process::Command, launch: &Launch) {
    command.envs(sets(launch));
}

/// Every variable a start of `launch` sets, with its value: the one account of them, which
/// `environment` puts on the process. Each name is in `kept` for its CLI.
pub fn sets(launch: &Launch) -> Vec<(&'static str, OsString)> {
    let mut sets: Vec<(&'static str, OsString)> = Vec::new();
    if let Some(path) = &launch.path {
        sets.push((PATH, path.into()));
    }
    // The token reaches the bridge through Codex's environment, and through Claude's config.
    if launch.cli == Cli::Codex {
        sets.push((super::bridge::TOKEN, (&launch.server.token).into()));
    }
    // With helpers, what leaves the assistant ilo's kind of helper alone (#4102).
    if launch.helpers {
        sets.extend(super::helpers::ENV.map(|(name, value)| (name, value.into())));
    }
    // Inside a wall Claude Code keeps nothing of its own in the user's folders, which the wall
    // denies it: no history of what it was asked, no memory, no copies of the files it changes,
    // no update of itself and no report of its use. Its temporary files go in the wall's folder
    // for them.
    if let (Cli::Claude, Some(wall)) = (launch.cli, &launch.wall) {
        sets.extend(CLAUDE_WALLED.map(|name| (name, "1".into())));
        if let Some(tmp) = wall.tmp() {
            sets.push((CLAUDE_TMP, tmp.into()));
        }
    }
    sets
}

const PATH: &str = "PATH";
/// What a walled Claude Code is started with, each on.
pub(crate) const CLAUDE_WALLED: [&str; 5] = [
    "CLAUDE_CODE_SKIP_PROMPT_HISTORY",
    "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
    "CLAUDE_CODE_DISABLE_FILE_CHECKPOINTING",
    "DISABLE_UPDATES",
    "DISABLE_TELEMETRY",
];
/// Where a walled Claude Code keeps its temporary files: the wall's folder for them.
const CLAUDE_TMP: &str = "CLAUDE_CODE_TMPDIR";

/// Every name a start of `cli` may set on its process, walled or not: what a wall's launcher is
/// told to keep of the runner's, beside the wall's own names. A wall drops every name it isn't
/// told (wall/launch.rs), so one set and not listed here would never arrive, and nothing would
/// say so: `sets` answers from these alone, which a test holds.
pub fn kept(cli: Cli) -> Vec<&'static str> {
    match cli {
        Cli::Claude => {
            let helpers = super::helpers::ENV.map(|(name, _)| name);
            [&[PATH, CLAUDE_TMP][..], &CLAUDE_WALLED[..], &helpers[..]].concat()
        }
        Cli::Codex => vec![PATH, super::bridge::TOKEN],
    }
}

/// Where Claude reads what it's told about working in the browser, beside its MCP config: a file,
/// as the text has paragraphs, and Windows starts no npm claude.cmd with a line break in an
/// argument (#1676).
pub fn instructions_file(launch: &Launch) -> PathBuf {
    launch.config.with_extension("md")
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
    let allowed: Vec<String> = launch
        .tools
        .iter()
        .map(|tool| format!("mcp__{SERVER}__{tool}"))
        .collect();
    let mut args: Vec<String> = [
        // Print mode, reading the user's messages as stream-json, so a reply is one more line.
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        // Its text as it writes it, which the conversation shows as it comes.
        "--include-partial-messages",
        // None of its own tools: no shell, files, web fetch or search. With helpers on, the one
        // that starts a helper.
        "--tools",
        match launch.helpers {
            true => super::helpers::TOOLS[0],
            false => "",
        },
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
    args.extend([
        "--append-system-prompt-file".into(),
        instructions_file(launch).to_string_lossy().into_owned(),
    ]);
    if launch.helpers {
        args.extend(super::helpers::claude_args(&allowed));
    }
    // Inside ilo's wall its own sandbox is off: the wall is what holds it. No settings file is
    // read, so this is its only setting.
    if launch.wall.is_some() {
        args.extend(["--settings".into(), WALLED_SETTINGS.into()]);
    }
    args
}

/// A walled Claude Code's settings.
const WALLED_SETTINGS: &str = r#"{"sandbox":{"enabled":false}}"#;

/// What a turn of Claude's has shown so far that its result needs: a limit it hit.
#[derive(Default)]
pub struct Turn {
    limit: Option<Option<u64>>,
    /// When the last whole line came, which a thought took since.
    heard: Option<Instant>,
    /// The text of the message being written, as its pieces came.
    writing: String,
}

/// What a line of Claude's stream-json output says to the user, if anything.
pub fn claude_read(message: &Value, turn: &mut Turn) -> Vec<Out> {
    let kind = message
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    // A helper's start is counted whoever's line says so, the task's or a helper's own (#4102),
    // its end is told as its start is, and a run's first line says which kinds of helper it has.
    let helpers =
        super::helpers::started(kind, message).or_else(|| super::helpers::ended(kind, message));
    if let Some(told) = helpers.or_else(|| super::helpers::kinds(kind, message)) {
        return vec![told];
    }
    // A line with a parent is a helper's, never the task's: nothing it says, writes or thinks is
    // the task's, shown or kept. It tells only that the helper calls a tool, and on which tab.
    if let Some(parent) = message.get("parent_tool_use_id").and_then(Value::as_str) {
        return super::helpers::claude_read(parent, kind, message);
    }
    if kind == "stream_event" {
        return claude_piece(message.get("event").unwrap_or(&Value::Null), turn);
    }
    let now = Instant::now();
    // A message comes whole, so its thinking took from the whole line before it.
    let took = turn
        .heard
        .replace(now)
        .map_or(Duration::ZERO, |heard| now - heard);
    match kind {
        "assistant" => {
            let content = message
                .pointer("/message/content")
                .and_then(Value::as_array);
            (content.into_iter().flatten())
                .filter_map(|block| {
                    let text = |name| block.get(name).and_then(Value::as_str).map(str::trim);
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => text("text")
                            .filter(|text| !text.is_empty())
                            .map(|text| Out::Said(text.into())),
                        Some("thinking") => {
                            (text("thinking").filter(|text| !text.is_empty())).map(|text| {
                                Out::Thought {
                                    text: text.into(),
                                    took,
                                }
                            })
                        }
                        Some("tool_use") => Some(Out::Calling),
                        _ => None,
                    }
                })
                .chain(super::helpers::asked(content.into_iter().flatten()))
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
            let error = message
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let text = message
                .get("result")
                .and_then(Value::as_str)
                .unwrap_or_default();
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

/// What a piece of the message Claude is writing says: its text so far as a piece of it comes, and
/// that it goes on to call a tool as one starts. Its thinking comes whole, as its message does.
fn claude_piece(event: &Value, turn: &mut Turn) -> Vec<Out> {
    match event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "message_start" => {
            turn.writing.clear();
            vec![Out::Writing(String::new())]
        }
        "content_block_start" => match event.pointer("/content_block/type").and_then(Value::as_str)
        {
            Some("tool_use") => vec![Out::Calling],
            // Each text block comes whole as a message of its own, so the next starts afresh.
            Some("text") => {
                turn.writing.clear();
                Vec::new()
            }
            _ => Vec::new(),
        },
        "content_block_delta"
            if event.pointer("/delta/type").and_then(Value::as_str) == Some("text_delta") =>
        {
            let piece = event
                .pointer("/delta/text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            turn.writing.push_str(piece);
            match turn.writing.trim().is_empty() {
                true => Vec::new(),
                false => vec![Out::Writing(turn.writing.trim().to_string())],
            }
        }
        _ => Vec::new(),
    }
}

/// Why Claude's turn failed, from its result's words.
fn claude_failure(text: &str, message: &Value) -> Failure {
    let lower = text.to_lowercase();
    if lower.contains("/login")
        || lower.contains("not logged in")
        || lower.contains("invalid api key")
    {
        return Failure::SignedOut;
    }
    if lower.contains("limit") {
        // Older versions end the text with the reset's time: "…limit reached|1730000000".
        let resets = text
            .rsplit_once('|')
            .and_then(|(_, at)| at.trim().parse().ok());
        return Failure::Limit(resets);
    }
    // With no words, the result's subtype says why, which the card says in plain words.
    let subtype = message
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Failure::Other(match (text.is_empty(), subtype) {
        (true, "error_max_turns") => "Claude hit its turn limit.".into(),
        (true, "error_max_budget_usd") => "Claude hit its budget.".into(),
        _ => plain_failure("Claude", text),
    })
}

/// A CLI's failure as the card says it: its words, unless they're a code, a status or JSON, which
/// the log keeps and the user can't act on.
pub(super) fn plain_failure(name: &str, text: &str) -> String {
    let text = text.trim();
    let code =
        !text.contains(char::is_whitespace) || text.contains('{') || text.starts_with("API Error");
    match code {
        true => format!("{name} ran into an error."),
        false => text.to_string(),
    }
}

#[cfg(test)]
mod tests;
