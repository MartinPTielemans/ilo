//! What a task's CLI is started with, and what its output says, as the CLIs write it.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;
use crate::agents::codex;

fn launch(cli: Cli) -> Launch {
    Launch {
        cli,
        program: PathBuf::from("/usr/local/bin/claude"),
        path: Some("/usr/local/bin:/usr/bin".into()),
        dir: PathBuf::from("/tmp/Tasks/task-1"),
        config: PathBuf::from("/tmp/Tasks/task-1.json"),
        server: Server {
            command: "/app/mcp-bridge".into(),
            args: vec!["/profile/agents.sock".into()],
            token: "secret".into(),
        },
        model: Some("haiku".into()),
        instructions: "Work in the browser.".into(),
        tools: vec!["open_tab".into(), "finish".into()],
    }
}

fn read(lines: &[Value]) -> Vec<Out> {
    let mut turn = Turn::default();
    lines.iter().flat_map(|line| claude_read(line, &mut turn)).collect()
}

#[test]
fn claude_gets_the_browser_alone() {
    let args = claude_args(&launch(Cli::Claude));
    let after = |flag: &str| {
        let at = args.iter().position(|arg| arg == flag).unwrap_or_else(|| panic!("no {flag}"));
        args[at + 1].clone()
    };
    for flag in ["-p", "--strict-mcp-config", "--disable-slash-commands", "--no-session-persistence"] {
        assert!(args.iter().any(|arg| arg == flag), "{flag} missing from {args:?}");
    }
    assert_eq!(after("--input-format"), "stream-json");
    assert_eq!(after("--output-format"), "stream-json");
    assert_eq!(after("--tools"), "");
    assert_eq!(after("--setting-sources"), "");
    assert_eq!(after("--permission-mode"), "dontAsk");
    assert_eq!(after("--permission-prompts"), "none");
    assert_eq!(after("--mcp-config"), "/tmp/Tasks/task-1.json");
    assert_eq!(after("--allowedTools"), "mcp__ilo__open_tab,mcp__ilo__finish");
    assert_eq!(after("--model"), "haiku");
    assert_eq!(after("--append-system-prompt"), "Work in the browser.");
    // The token never goes on the command line, which other users can read.
    assert!(!args.iter().any(|arg| arg.contains("secret")));
}

#[test]
fn the_clis_default_model_passes_no_model() {
    let mut launch = launch(Cli::Claude);
    launch.model = Some(String::new());
    assert!(!claude_args(&launch).iter().any(|arg| arg == "--model"));
}

#[test]
fn claudes_config_names_the_browsers_server_alone() {
    let config: Value = serde_json::from_str(&claude_config(&launch(Cli::Claude).server)).unwrap();
    assert_eq!(
        config,
        json!({ "mcpServers": { "ilo": {
            "type": "stdio",
            "command": "/app/mcp-bridge",
            "args": ["/profile/agents.sock"],
            "env": { "BROWSER_AGENT_TOKEN": "secret" },
        } } })
    );
}

#[test]
fn claudes_text_is_said_and_its_tool_calls_are_not() {
    let outs = read(&[
        json!({ "type": "system", "subtype": "init", "tools": ["mcp__ilo__open_tab"] }),
        json!({ "type": "assistant", "message": { "content": [
            { "type": "thinking", "thinking": "Hmm." },
            { "type": "text", "text": "  Looking for flights.  " },
            { "type": "tool_use", "name": "mcp__ilo__open_tab", "input": {} },
        ] } }),
        json!({ "type": "result", "subtype": "success", "is_error": false, "result": "Found one." }),
    ]);
    assert_eq!(outs, vec![Out::Said("Looking for flights.".into()), Out::Ended(None)]);
}

#[test]
fn a_rejected_rate_limit_ends_the_turn_at_its_limit() {
    let outs = read(&[
        json!({ "type": "rate_limit_event", "rate_limit_info": { "status": "allowed_warning", "resetsAt": 1 } }),
        json!({ "type": "rate_limit_event", "rate_limit_info": {
            "status": "rejected", "resetsAt": 1_800_000_000u64, "rateLimitType": "five_hour",
        } }),
        json!({ "type": "result", "subtype": "success", "is_error": true, "result": "You've hit your limit" }),
    ]);
    assert_eq!(outs, vec![Out::Ended(Some(Failure::Limit(Some(1_800_000_000))))]);
}

#[test]
fn claudes_limit_in_its_result_says_when_it_resets() {
    let outs = read(&[json!({
        "type": "result", "subtype": "success", "is_error": true,
        "result": "Claude AI usage limit reached|1800000000",
    })]);
    assert_eq!(outs, vec![Out::Ended(Some(Failure::Limit(Some(1_800_000_000))))]);
}

#[test]
fn signed_out_claude_says_so() {
    let outs = read(&[json!({
        "type": "result", "subtype": "success", "is_error": true,
        "result": "Not logged in · Please run /login",
    })]);
    assert_eq!(outs, vec![Out::Ended(Some(Failure::SignedOut))]);
}

#[test]
fn another_failure_keeps_claudes_words() {
    let outs = read(&[json!({ "type": "result", "subtype": "error_max_turns", "is_error": true })]);
    assert_eq!(outs, vec![Out::Ended(Some(Failure::Other("It stopped (error_max_turns).".into())))]);
}

#[test]
fn codex_turns_off_the_users_servers_by_name() {
    let config = "model = \"x\"\n[mcp_servers.linear]\ncommand = \"a\"\n[mcp_servers.linear.env]\n\
                  [mcp_servers.\"my.server\"]\n[mcp_servers.ilo]\n[profiles.fast]\n";
    assert_eq!(codex::servers_in(config), vec!["linear".to_string(), "my.server".to_string()]);
}

#[test]
fn codexs_thread_has_the_browser_alone() {
    let params = codex::thread_params(&launch(Cli::Codex));
    let config = &params["config"];
    for feature in ["shell_tool", "unified_exec", "apps", "plugins", "hooks", "memories", "multi_agent"] {
        assert_eq!(config[format!("features.{feature}")], json!(false), "{feature}");
    }
    assert_eq!(config["web_search"], json!("disabled"));
    assert_eq!(config["project_doc_max_bytes"], json!(0));
    assert_eq!(config["mcp_servers.ilo"]["command"], json!("/app/mcp-bridge"));
    assert_eq!(config["mcp_servers.ilo"]["env"]["BROWSER_AGENT_TOKEN"], json!("secret"));
    assert_eq!(params["approvalPolicy"], json!("never"));
    assert_eq!(params["sandbox"], json!("read-only"));
    assert_eq!(params["ephemeral"], json!(true));
    assert_eq!(params["cwd"], json!("/tmp/Tasks/task-1"));
    assert_eq!(params["model"], json!("haiku"));
}

#[test]
fn codexs_messages_limits_and_failures() {
    let state = codex::State::default();
    let stdin = std::sync::Mutex::new(None);
    let launch = launch(Cli::Codex);
    let read = |line: Value| codex::read(&line, &state, &stdin, &launch);
    assert_eq!(read(json!({ "id": 2, "result": { "thread": { "id": "t1" } } })), vec![]);
    assert_eq!(read(json!({ "method": "turn/started", "params": { "turn": { "id": "u1" } } })), vec![]);
    assert_eq!(
        read(json!({ "method": "item/completed", "params": { "item": { "type": "agentMessage", "text": "On it." } } })),
        vec![Out::Said("On it.".into())]
    );
    read(json!({ "method": "account/rateLimits/updated", "params": { "rateLimits": {
        "primary": { "usedPercent": 100, "resetsAt": 1_800_000_000u64 },
        "secondary": { "usedPercent": 40, "resetsAt": 1_900_000_000u64 },
    } } }));
    read(json!({ "method": "error", "params": { "willRetry": true, "error": { "message": "retrying" } } }));
    read(json!({ "method": "error", "params": { "willRetry": false, "error": {
        "message": "You've hit your usage limit.", "codexErrorInfo": "usageLimitExceeded",
    } } }));
    assert_eq!(
        read(json!({ "method": "turn/completed", "params": { "turn": { "id": "u1", "status": "failed" } } })),
        vec![Out::Ended(Some(Failure::Limit(Some(1_800_000_000))))]
    );
    assert_eq!(
        read(json!({ "method": "turn/completed", "params": { "turn": { "id": "u2", "status": "failed",
            "error": { "message": "no", "codexErrorInfo": "unauthorized" } } } })),
        vec![Out::Ended(Some(Failure::SignedOut))]
    );
    assert_eq!(
        read(json!({ "method": "turn/completed", "params": { "turn": { "id": "u3", "status": "interrupted" } } })),
        vec![Out::Ended(None)]
    );
}

#[test]
fn claudes_sign_in_status() {
    assert!(crate::agents::ready::claude_signed_in(r#"{"loggedIn":true,"authMethod":"claude.ai"}"#));
    assert!(!crate::agents::ready::claude_signed_in(r#"{"loggedIn":false}"#));
    assert!(!crate::agents::ready::claude_signed_in("Not logged in"));
}

#[test]
fn codexs_models_are_kept_without_the_hidden_ones() {
    use crate::agents::ready::{Found, Ready, codex_listed};
    let listed = codex_listed(&json!({ "id": 2, "result": { "data": [
        { "id": "gpt-5-codex", "displayName": "GPT-5 Codex", "hidden": false },
        { "id": "old", "displayName": "Old", "hidden": true },
        { "id": "bare" },
    ], "nextCursor": null } }));
    let models: Vec<(String, String)> =
        [("gpt-5-codex", "GPT-5 Codex"), ("bare", "bare")].map(|(id, name)| (id.into(), name.into())).into();
    assert_eq!(listed, models);
    let found = Found { ready: Ready::SignedIn, models, ..Default::default() };
    assert_eq!(Found::from_saved(&found.saved()), found, "kept across runs");
}

#[cfg(unix)]
#[test]
fn a_cli_is_found_on_the_path_given() {
    let dir = std::env::temp_dir().join(format!("ilo-find-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let program = dir.join("claude");
    std::fs::write(&program, "#!/bin/sh\n").unwrap();
    let path = format!("/nonexistent:{}", dir.display());
    assert_eq!(crate::agents::ready::find("claude", &path), None, "not executable yet");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(crate::agents::ready::find("claude", &path), Some(program));
    std::fs::remove_dir_all(dir).unwrap();
}

/// The real `claude` on this computer, locked down, against a stub MCP server with one tool: what
/// its init lists, and that the stub's tool is all it calls. Spends a cent of the user's plan, so
/// it runs only when asked: `cargo test --release --lib -- --ignored claude_runs_locked_down`.
#[cfg(unix)]
#[test]
#[ignore]
fn claude_runs_locked_down() {
    let found = crate::agents::ready::look(Cli::Claude);
    let program = found.program.expect("claude installed");
    let dir = std::env::temp_dir().join(format!("ilo-claude-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("stub.py");
    std::fs::write(&stub, STUB).unwrap();
    let mut launch = launch(Cli::Claude);
    (launch.program, launch.path) = (program, found.path);
    launch.dir = dir.join("task");
    launch.config = dir.join("task.json");
    launch.server = Server { command: "/usr/bin/python3".into(), args: vec!["-I".into(), stub.display().to_string()], token: "t".into() };
    launch.tools = vec!["probe_echo".into()];
    let (events, mut said) = futures::channel::mpsc::unbounded();
    let mut process = Process::start(&launch, "task-1", "Call probe_echo with the text hi, then say what it answered.", events).unwrap();
    let mut outs = Vec::new();
    while let Some(event) = futures::executor::block_on(futures::StreamExt::next(&mut said)) {
        let ended = matches!(event.out, Out::Ended(_) | Out::Gone(_));
        outs.push(event.out);
        if ended {
            break;
        }
    }
    process.say("Say the word done.").unwrap();
    while let Some(event) = futures::executor::block_on(futures::StreamExt::next(&mut said)) {
        let ended = matches!(event.out, Out::Ended(_) | Out::Gone(_));
        outs.push(event.out);
        if ended {
            break;
        }
    }
    drop(process);
    let calls = std::fs::read_to_string(dir.join("calls.log")).unwrap_or_default();
    eprintln!("{outs:#?}\n{calls}");
    assert!(calls.contains("\"probe_echo\""), "the stub's tool was called");
    assert!(outs.iter().any(|out| matches!(out, Out::Said(text) if text.contains("echo"))));
    assert_eq!(outs.iter().filter(|out| matches!(out, Out::Ended(None))).count(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

/// A stdio MCP server with one tool, which logs each message beside itself.
#[cfg(unix)]
const STUB: &str = r#"
import json, os, sys
log = open(os.path.join(os.path.dirname(__file__), "calls.log"), "a")
for line in sys.stdin:
    if not line.strip():
        continue
    msg = json.loads(line)
    log.write(line); log.flush()
    if "id" not in msg:
        continue
    method = msg.get("method")
    if method == "initialize":
        result = {"protocolVersion": msg["params"].get("protocolVersion"), "capabilities": {"tools": {}}, "serverInfo": {"name": "stub", "version": "0"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "probe_echo", "description": "Echo the text.", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}}]}
    elif method == "tools/call":
        result = {"content": [{"type": "text", "text": "echo: " + msg["params"]["arguments"].get("text", "")}], "isError": False}
    else:
        result = {}
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "result": result}) + "\n"); sys.stdout.flush()
"#;
