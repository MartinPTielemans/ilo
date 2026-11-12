//! A task's Codex: `codex app-server`, which speaks JSON-RPC, a message a line, without the
//! "jsonrpc" member. initialize, then a thread started with the lockdown as its config, then a turn
//! for each of the user's messages. Every approval it asks for is declined, as it has nothing it
//! may run.

use std::collections::VecDeque;
use std::process::ChildStdin;
use std::sync::Mutex;
use web_time::Instant;

use serde_json::{Map, Value, json};

use super::runner::{Failure, Launch, Out, SERVER, write_line};

const INITIALIZE: u64 = 1;
const THREAD: u64 = 2;
/// Turns' requests count up from here.
const TURNS: u64 = 100;

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

/// Sends the user's message as a turn, or keeps it for when the thread is there or the turn under
/// way ends.
pub fn say(stdin: &Mutex<Option<ChildStdin>>, state: &State, text: &str) -> std::io::Result<()> {
    let mut inner = state.inner.lock().unwrap();
    inner.waiting.push_back(text.to_string());
    next_turn(stdin, &mut inner)
}

/// Interrupts the turn under way.
pub fn interrupt(stdin: &Mutex<Option<ChildStdin>>, state: &State) -> std::io::Result<()> {
    let inner = state.inner.lock().unwrap();
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
    write_line(
        stdin,
        &json!({
            "id": TURNS + inner.next,
            "method": "turn/start",
            "params": { "threadId": thread, "input": [{ "type": "text", "text": text }] },
        }),
    )
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
            INITIALIZE => {
                _ = write_line(stdin, &json!({ "method": "initialized" }));
                _ = write_line(
                    stdin,
                    &json!({ "id": THREAD, "method": "thread/start", "params": thread_params(launch) }),
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
/// files, network or shell, from which only a patch tool the read-only sandbox refuses and the
/// browser server's resources are reachable; Codex has no setting that removes it.
pub fn thread_params(launch: &Launch) -> Value {
    let mut config = Map::new();
    for feature in FEATURES_OFF {
        config.insert(format!("features.{feature}"), json!(false));
    }
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
        "sandbox": "read-only",
        "ephemeral": true,
        "developerInstructions": launch.instructions,
    });
    if let Some(model) = launch.model.as_ref().filter(|model| !model.is_empty()) {
        params["model"] = json!(model);
    }
    params
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
