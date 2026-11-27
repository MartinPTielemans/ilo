//! A task's Codex: `codex app-server`, which speaks JSON-RPC, a message a line, without the
//! "jsonrpc" member. initialize, then a thread started with the lockdown as its config, then a turn
//! for each of the user's messages. Every approval it asks for is declined, as it has nothing it
//! may run.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::ChildStdin;
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Map, Value, json};

use super::runner::{Failure, Launch, Out, SERVER, write_line};

const INITIALIZE: u64 = 1;
const THREAD: u64 = 2;
/// Turns' requests count up from here.
const TURNS: u64 = 100;
/// A message steered into the turn under way.
const STEER: u64 = TURNS - 2;

/// What starts the name of the permission profile of ilo's own that a thread's commands run under,
/// never shown. Each start's name ends in its own random part, as Codex merges a table of the
/// user's config.toml into one of the same name and no table written beforehand can name it.
pub const PROFILE: &str = "ilo-thread-";
/// The oldest Codex whose thread/start takes a named profile and answers which one is active: the
/// one they were read from (rust-v0.172.0, app-server-protocol/src/protocol/v2/thread.rs:99-102,
/// 225-229). An older one could take the thread without the profile and never say so.
pub const OLDEST: [u32; 3] = [0, 172, 0];

/// The thread, the turn under way, and the messages that wait for either.
#[derive(Default)]
pub struct State {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    thread: Option<String>,
    turn: Option<String>,
    /// A turn was asked for and hasn't started.
    starting: bool,
    waiting: VecDeque<String>,
    next: u64,
    /// When the window that's used up resets, in seconds since 1970, from the last rate limits.
    resets: Option<u64>,
    /// What went wrong in the turn under way, which its end reports.
    failure: Option<Failure>,
    /// When the reasoning under way started.
    thinking: Option<Instant>,
    /// The model the user picked for its next turns, by id, which each turn names from then on.
    model: Option<String>,
    /// The model the turn under way started on, which a steer into it would run on.
    turn_model: Option<String>,
    /// The name of the profile its thread was asked to start under, once it was asked.
    profile: Option<String>,
    /// A message steered into the turn under way that Codex hasn't taken yet, which becomes a
    /// turn of its own if the turn ended first.
    steering: Option<String>,
}

impl State {
    /// The profile's name its thread was asked to start under, for a test to answer with.
    #[cfg(test)]
    pub fn profile(&self) -> Option<String> {
        self.inner.lock().unwrap().profile.clone()
    }
}

/// A new name for a start's profile: PROFILE and 16 hex digits from the system's random numbers.
pub fn profile_name() -> String {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("the system has no random numbers");
    let random: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{PROFILE}{random}")
}

/// Starts the server: initialize, which `read` follows with the thread and the first turn.
pub fn begin(stdin: &Mutex<Option<ChildStdin>>, state: &State, first: &str) -> std::io::Result<()> {
    state
        .inner
        .lock()
        .unwrap()
        .waiting
        .push_back(first.to_string());
    write_line(
        stdin,
        &json!({
            "id": INITIALIZE,
            "method": "initialize",
            "params": {
                "clientInfo": { "name": env!("APP_NAME").to_lowercase(), "title": env!("APP_NAME"), "version": env!("CARGO_PKG_VERSION") },
                "capabilities": { "experimentalApi": true },
            },
        }),
    )
}

/// The model the next turns run on, from the user's pick.
pub fn set_model(state: &State, model: &str) {
    state.inner.lock().unwrap().model = Some(model.to_string());
}

/// Sends the user's message as a turn, or keeps it for when the thread is there or the turn under
/// way ends.
pub fn say(stdin: &Mutex<Option<ChildStdin>>, state: &State, text: &str) -> std::io::Result<()> {
    let mut inner = state.inner.lock().unwrap();
    inner.waiting.push_back(text.to_string());
    next_turn(stdin, &mut inner)
}

/// Interrupts the turn under way.
pub fn interrupt(stdin: &Mutex<Option<ChildStdin>>, state: &State) -> std::io::Result<()> {
    interrupt_turn(stdin, &state.inner.lock().unwrap())
}

fn interrupt_turn(stdin: &Mutex<Option<ChildStdin>>, inner: &Inner) -> std::io::Result<()> {
    let (Some(thread), Some(turn)) = (&inner.thread, &inner.turn) else {
        return Ok(());
    };
    write_line(
        stdin,
        &json!({
            "id": TURNS - 1,
            "method": "turn/interrupt",
            "params": { "threadId": thread, "turnId": turn },
        }),
    )
}

/// Puts the user's message into the turn under way, as Codex's `turn/steer` does ("add user
/// input to an already in-flight turn", app-server README), or makes it the next turn with none.
/// A turn under way on a model the user has since left is interrupted instead, and the message
/// is the next turn, on the one picked. Says whether a turn ends for it.
pub fn steer(
    stdin: &Mutex<Option<ChildStdin>>,
    state: &State,
    text: &str,
) -> std::io::Result<bool> {
    let mut inner = state.inner.lock().unwrap();
    let (Some(thread), Some(turn)) = (inner.thread.clone(), inner.turn.clone()) else {
        inner.waiting.push_front(text.to_string());
        return next_turn(stdin, &mut inner).map(|_| false);
    };
    if inner.model != inner.turn_model {
        inner.waiting.push_front(text.to_string());
        return interrupt_turn(stdin, &inner).map(|_| true);
    }
    inner.steering = Some(text.to_string());
    let input = json!([{ "type": "text", "text": text }]);
    write_line(
        stdin,
        &json!({
            "id": STEER,
            "method": "turn/steer",
            "params": { "threadId": thread, "input": input, "expectedTurnId": turn },
        }),
    )
    .map(|_| false)
}

/// Starts a turn with the next message waiting, if the thread is free.
fn next_turn(stdin: &Mutex<Option<ChildStdin>>, inner: &mut Inner) -> std::io::Result<()> {
    if inner.turn.is_some() || inner.starting {
        return Ok(());
    }
    let Some(thread) = inner.thread.clone() else {
        return Ok(());
    };
    let Some(text) = inner.waiting.pop_front() else {
        return Ok(());
    };
    inner.starting = true;
    inner.next += 1;
    inner.turn_model = inner.model.clone();
    let params = turn_params(&thread, &text, inner.model.as_deref());
    write_line(
        stdin,
        &json!({ "id": TURNS + inner.next, "method": "turn/start", "params": params }),
    )
}

/// A turn's params: the thread, the message, and the model the user picked for it, if they did,
/// which holds for the turns after it too.
fn turn_params(thread: &str, text: &str, model: Option<&str>) -> Value {
    let mut params = json!({ "threadId": thread, "input": [{ "type": "text", "text": text }] });
    if let Some(model) = model {
        params["model"] = json!(model);
    }
    params
}

/// What a line of Codex's says, answering what it asks on the way.
pub fn read(
    message: &Value,
    state: &State,
    stdin: &Mutex<Option<ChildStdin>>,
    launch: &Launch,
) -> Vec<Out> {
    let mut inner = state.inner.lock().unwrap();
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id");
    // A request of the server's: an approval, which is declined.
    if let (Some(method), Some(id)) = (method, id) {
        _ = write_line(stdin, &json!({ "id": id, "result": decline(method) }));
        return Vec::new();
    }
    if let Some(id) = id.and_then(Value::as_u64) {
        // A steer the turn ended before: the message is the next turn, never lost.
        if id == STEER {
            let refused = message.get("error").is_some();
            if let Some(text) = inner.steering.take().filter(|_| refused) {
                inner.waiting.push_front(text);
                _ = next_turn(stdin, &mut inner);
            }
            return Vec::new();
        }
        if let Some(error) = message.get("error") {
            let text = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default();
            inner.starting = false;
            return vec![match id {
                INITIALIZE | THREAD => Out::Gone(text.to_string()),
                _ => Out::Ended(Some(Failure::Other(super::runner::plain_failure(
                    "Codex", text,
                )))),
            }];
        }
        match id {
            // Older than its floor: no thread starts, and the task ends as one whose Codex needs an
            // update (AP27). Not saying the profile asked for: no thread starts either, and the
            // task ends as one whose program stopped.
            INITIALIZE if !fits(message.pointer("/result/userAgent").and_then(Value::as_str)) => {
                log!("browser: codex is older than its floor");
                inner.starting = false;
                return vec![Out::Ended(Some(Failure::NeedsUpdate))];
            }
            THREAD if !holds(message, inner.profile.as_deref()) => {
                inner.starting = false;
                return vec![Out::Gone("Codex didn't take the thread's profile".into())];
            }
            INITIALIZE => {
                let profile = profile_name();
                let params = thread_params(launch, &profile);
                inner.profile = Some(profile);
                _ = write_line(stdin, &json!({ "method": "initialized" }));
                _ = write_line(
                    stdin,
                    &json!({ "id": THREAD, "method": "thread/start", "params": params }),
                );
            }
            THREAD => {
                inner.thread = message
                    .pointer("/result/thread/id")
                    .and_then(Value::as_str)
                    .map(String::from);
                _ = next_turn(stdin, &mut inner);
            }
            id if id > TURNS => {
                inner.starting = false;
                if let Some(turn) = message.pointer("/result/turn/id").and_then(Value::as_str) {
                    inner.turn = Some(turn.to_string());
                }
            }
            _ => {}
        }
        return Vec::new();
    }
    let params = message.get("params").unwrap_or(&Value::Null);
    match method.unwrap_or_default() {
        "turn/started" => {
            inner.starting = false;
            inner.turn = params
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .map(String::from);
            Vec::new()
        }
        "item/started" => {
            if params.pointer("/item/type").and_then(Value::as_str) == Some("reasoning") {
                inner.thinking = Some(Instant::now());
            }
            Vec::new()
        }
        "item/completed" => {
            let item = params.get("item").unwrap_or(&Value::Null);
            let text = item.get("text").and_then(Value::as_str).map(str::trim);
            match (item.get("type").and_then(Value::as_str), text) {
                (Some("agentMessage"), Some(text)) if !text.is_empty() => {
                    vec![Out::Said(text.to_string())]
                }
                (Some("reasoning"), _) => {
                    let took = inner
                        .thinking
                        .take()
                        .map(|started| started.elapsed())
                        .unwrap_or_default();
                    // Its summary, else its whole reasoning, as the model gives either.
                    let parts = |name: &str| -> String {
                        let parts = item
                            .get(name)
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten();
                        let parts: Vec<&str> =
                            parts.filter_map(Value::as_str).map(str::trim).collect();
                        parts.join("\n\n").trim().to_string()
                    };
                    let text = Some(parts("summary"))
                        .filter(|text| !text.is_empty())
                        .unwrap_or_else(|| parts("content"));
                    match text.is_empty() {
                        true => Vec::new(),
                        false => vec![Out::Thought { text, took }],
                    }
                }
                _ => Vec::new(),
            }
        }
        "account/rateLimits/updated" => {
            let limits = params.get("rateLimits").unwrap_or(&Value::Null);
            let full = ["primary", "secondary"].into_iter().filter_map(|window| {
                let window = limits.get(window)?;
                let used = window.get("usedPercent").and_then(Value::as_f64)?;
                (used >= 100.)
                    .then(|| window.get("resetsAt").and_then(Value::as_u64))
                    .flatten()
            });
            inner.resets = full.max();
            Vec::new()
        }
        "error" => {
            if params.get("willRetry").and_then(Value::as_bool) == Some(true) {
                return Vec::new();
            }
            let error = params.get("error").unwrap_or(&Value::Null);
            inner.failure = Some(codex_failure(error, inner.resets));
            Vec::new()
        }
        "turn/completed" => {
            inner.turn = None;
            inner.starting = false;
            let failed = params.pointer("/turn/status").and_then(Value::as_str) == Some("failed");
            let mut failure = inner.failure.take();
            if failure.is_none() && failed {
                let error = params.pointer("/turn/error").unwrap_or(&Value::Null);
                failure = Some(codex_failure(error, inner.resets));
            }
            let interrupted =
                params.pointer("/turn/status").and_then(Value::as_str) == Some("interrupted");
            _ = next_turn(stdin, &mut inner);
            match interrupted {
                true => vec![Out::Ended(None)],
                false => vec![Out::Ended(failure)],
            }
        }
        _ => Vec::new(),
    }
}

/// Why a Codex turn failed, from its error.
fn codex_failure(error: &Value, resets: Option<u64>) -> Failure {
    let info = error.get("codexErrorInfo");
    let kind = info
        .and_then(|info| {
            info.as_str()
                .map(String::from)
                .or_else(|| info.as_object()?.keys().next().cloned())
        })
        .unwrap_or_default();
    match kind.as_str() {
        "usageLimitExceeded" | "rateLimitExceeded" => Failure::Limit(resets),
        "unauthorized" => Failure::SignedOut,
        _ => Failure::Other(super::runner::plain_failure(
            "Codex",
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )),
    }
}

/// The answer that declines a request of the server's.
fn decline(method: &str) -> Value {
    match method {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            json!({ "decision": "decline" })
        }
        "applyPatchApproval" | "execCommandApproval" => json!({ "decision": "abort" }),
        "mcpServer/elicitation/request" => json!({ "action": "decline" }),
        "item/permissions/requestApproval" => json!({ "permissions": {} }),
        "item/tool/requestUserInput" => json!({ "answers": {} }),
        _ => json!({}),
    }
}

/// The thread's settings: the browser's server as its tools, every feature of Codex's own off,
/// and none of the user's MCP servers, each turned off by name as config.toml lists them. A model
/// whose catalog entry asks for code mode keeps `exec` and `wait`, a JavaScript isolate with no
/// files, network or shell, from which only a patch tool the profile refuses and the browser
/// server's resources are reachable; Codex has no setting that removes it.
pub fn thread_params(launch: &Launch, profile_name: &str) -> Value {
    let mut config = Map::new();
    for feature in FEATURES_OFF {
        config.insert(format!("features.{feature}"), json!(false));
    }
    // A thread searches with the browser's own search tool, so its provider's is off, set so the
    // user's own config can't turn it on.
    config.insert("web_search".into(), json!("disabled"));
    config.insert("notify".into(), json!([]));
    config.insert("project_doc_max_bytes".into(), json!(0));
    config.insert("tools.update_plan.enabled".into(), json!(false));
    // The model catalog gives some models sub-agents whatever the multi_agent feature says; a
    // depth of 0 drops their tools.
    config.insert("agents.max_depth".into(), json!(0));
    // Otherwise the prompt lists the skills in the user's Codex home.
    config.insert("skills.include_instructions".into(), json!(false));
    for server in user_servers() {
        let bare = server
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        let key = if bare {
            server
        } else {
            format!("\"{server}\"")
        };
        config.insert(format!("mcp_servers.{key}.enabled"), json!(false));
    }
    config.insert(
        format!("permissions.{profile_name}"),
        profile(&launch.reads),
    );
    let mut env = Map::new();
    env.insert(super::bridge::TOKEN.into(), json!(launch.server.token));
    // Under approvalPolicy never Codex refuses an MCP call it would ask about; the browser asks
    // the user itself, so each tool is approved by name (the server-wide default doesn't reach it).
    let tools: Map<String, Value> = launch
        .tools
        .iter()
        .map(|tool| (tool.clone(), json!({ "approval_mode": "approve" })))
        .collect();
    config.insert(
        format!("mcp_servers.{SERVER}"),
        json!({
            "command": launch.server.command,
            "args": launch.server.args,
            "env": env,
            "enabled_tools": launch.tools,
            "tools": tools,
            // A model in code mode would find them only by searching from inside its JavaScript
            // tool; left out of it they're the model's own tools, described in full.
            "omit_tools_from": ["code_mode"],
            // Without the browser's tools a task can't do anything, so the thread fails to start.
            "required": true,
            // ask waits for the user, who may take a while.
            "tool_timeout_sec": 86_400,
            // Otherwise Codex sends a turn's calls one after another; the server runs calls on
            // different tabs at once and keeps each tab's in order.
            "supports_parallel_tool_calls": true,
        }),
    );
    let mut params = json!({
        "config": config,
        "cwd": launch.dir.to_string_lossy(),
        "approvalPolicy": "never",
        "approvalsReviewer": "user",
        // In place of the read-only sandbox, which lets a command read the whole disk; Codex
        // takes a profile or a sandbox, never both.
        "permissions": profile_name,
        "ephemeral": true,
        "developerInstructions": launch.instructions,
    });
    if let Some(model) = launch.model.as_ref().filter(|model| !model.is_empty()) {
        params["model"] = json!(model);
    }
    params
}

/// What a thread's commands may do, as Codex's own sandbox holds them: read `reads` and nothing
/// else, write nowhere, and no network. Codex can't take away its command tool, so this is what
/// bounds it. The root's deny is the default the narrower reads reopen
/// (codex-rs/protocol/src/permissions.rs:1836 at rust-v0.172.0); with no read, a command can't
/// even load its own program. What Codex itself reads, and the browser's server, aren't commands.
fn profile(reads: &[PathBuf]) -> Value {
    let mut filesystem = Map::new();
    filesystem.insert(":root".into(), json!("deny"));
    for read in reads {
        filesystem.insert(read.to_string_lossy().into_owned(), json!("read"));
    }
    json!({ "filesystem": filesystem, "network": { "enabled": false } })
}

/// Whether the `userAgent` Codex answers initialize with, `<originator>/<version> (…)`
/// (codex-rs/login/src/auth/default_client.rs:152-166 at rust-v0.172.0), names OLDEST or newer.
pub fn fits(agent: Option<&str>) -> bool {
    let version = (agent.and_then(|agent| agent.split_whitespace().next()))
        .and_then(|first| first.rsplit_once('/'))
        .map(|(_, version)| version);
    version.is_some_and(at_least_oldest)
}

/// Whether what `codex --version` printed, `codex-cli <version>`, names OLDEST or newer.
pub fn fits_version(printed: &str) -> bool {
    let first = printed.lines().next().unwrap_or_default();
    first.split_whitespace().last().is_some_and(at_least_oldest)
}

/// Whether `version`, `<major>.<minor>.<patch>` and anything after, is OLDEST or newer.
fn at_least_oldest(version: &str) -> bool {
    let parts: Option<Vec<u32>> = (version.split(['.', '-', '+']).take(3))
        .map(|part| part.parse().ok())
        .collect();
    parts.is_some_and(|parts| parts.len() == 3 && parts[..] >= OLDEST[..])
}

/// Whether Codex's answer to thread/start says the thread runs under `asked`, the profile this
/// start named, and not one another start or the user's config named.
pub fn holds(answer: &Value, asked: Option<&str>) -> bool {
    let active = answer.pointer("/result/activePermissionProfile/id");
    asked.is_some() && active.and_then(Value::as_str) == asked
}

/// Codex's features that would give the model anything but the browser's tools.
const FEATURES_OFF: [&str; 19] = [
    "shell_tool",
    "unified_exec",
    "apps",
    "plugins",
    "hooks",
    "computer_use",
    "browser_use",
    "browser_use_external",
    "in_app_browser",
    "view_image",
    "memories",
    "multi_agent",
    "image_generation",
    "chronicle",
    "goals",
    "tool_suggest",
    "skill_search",
    "shell_snapshot",
    "sleep_tool",
];

/// The MCP servers the user's config.toml names, which the thread turns off.
fn user_servers() -> Vec<String> {
    servers_in(&std::fs::read_to_string(super::clients::codex_config()).unwrap_or_default())
}

/// The names of the `[mcp_servers.<name>]` tables in a config.toml.
pub fn servers_in(text: &str) -> Vec<String> {
    let mut names: Vec<String> = (text.lines())
        .filter_map(|line| {
            let table = line
                .trim()
                .strip_prefix("[mcp_servers.")?
                .strip_suffix(']')?;
            let name = match table.strip_prefix('"') {
                Some(quoted) => quoted.split('"').next()?,
                None => table.split('.').next()?,
            };
            Some(name.to_string())
        })
        .filter(|name| name != SERVER)
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::Value;

    /// The lines `say` would send through a pipe, written by `send` and read back.
    fn sent(send: impl FnOnce(&Mutex<Option<std::process::ChildStdin>>)) -> Vec<Value> {
        let mut child = crate::child::command(std::path::Path::new("cat"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = Mutex::new(child.stdin.take());
        send(&stdin);
        stdin.lock().unwrap().take();
        let out = child.wait_with_output().unwrap();
        (String::from_utf8(out.stdout).unwrap().lines())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// Send Now on Codex puts the message into the turn under way, by its id, with no new turn;
    /// with none under way it's the next turn.
    #[test]
    fn a_steer_goes_into_the_turn_under_way() {
        let state = super::State::default();
        {
            let mut inner = state.inner.lock().unwrap();
            inner.thread = Some("t1".into());
            inner.turn = Some("turn-7".into());
        }
        let lines = sent(|stdin| assert!(!super::steer(stdin, &state, "Also Porto").unwrap()));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["method"], "turn/steer");
        assert_eq!(lines[0]["params"]["expectedTurnId"], "turn-7");
        assert_eq!(lines[0]["params"]["input"][0]["text"], "Also Porto");

        state.inner.lock().unwrap().turn = None;
        let lines = sent(|stdin| assert!(!super::steer(stdin, &state, "And Faro").unwrap()));
        assert_eq!(lines[0]["method"], "turn/start");
        assert_eq!(lines[0]["params"]["input"][0]["text"], "And Faro");
    }

    /// A steer after another model is picked doesn't go into the turn on the old one: that turn
    /// is interrupted, and the message is the next turn, on the model picked.
    #[test]
    fn a_steer_after_another_model_is_the_next_turn_on_it() {
        let state = super::State::default();
        {
            let mut inner = state.inner.lock().unwrap();
            inner.thread = Some("t1".into());
            inner.turn = Some("turn-7".into());
        }
        super::set_model(&state, "gpt-b");
        let lines = sent(|stdin| assert!(super::steer(stdin, &state, "Also Porto").unwrap()));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["method"], "turn/interrupt");
        // The interrupted turn ends, as `read` takes its end, and the message starts the next on
        // the model picked.
        let lines = sent(|stdin| {
            let mut inner = state.inner.lock().unwrap();
            inner.turn = None;
            super::next_turn(stdin, &mut inner).unwrap();
        });
        assert_eq!(lines[0]["method"], "turn/start");
        assert_eq!(lines[0]["params"]["model"], "gpt-b");
        assert_eq!(lines[0]["params"]["input"][0]["text"], "Also Porto");
    }

    /// A turn names the model only once the user picked one, and then its id.
    #[test]
    fn a_turn_names_the_model_the_user_picked() {
        let plain = super::turn_params("t1", "Go on", None);
        assert_eq!(plain.get("model"), None);
        let picked = super::turn_params("t1", "Go on", Some("gpt-b"));
        assert_eq!(picked["model"], "gpt-b");
        assert_eq!(picked["input"][0]["text"], "Go on");
    }
}
