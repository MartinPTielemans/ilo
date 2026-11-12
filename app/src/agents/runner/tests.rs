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
    lines
        .iter()
        .flat_map(|line| claude_read(line, &mut turn))
        .collect()
}

#[test]
fn claude_gets_the_browser_alone() {
    let args = claude_args(&launch(Cli::Claude));
    let after = |flag: &str| {
        let at = args
            .iter()
            .position(|arg| arg == flag)
            .unwrap_or_else(|| panic!("no {flag}"));
        args[at + 1].clone()
    };
    for flag in [
        "-p",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--no-session-persistence",
    ] {
        assert!(
            args.iter().any(|arg| arg == flag),
            "{flag} missing from {args:?}"
        );
    }
    assert_eq!(after("--input-format"), "stream-json");
    assert_eq!(after("--output-format"), "stream-json");
    assert_eq!(after("--tools"), "");
    assert_eq!(after("--setting-sources"), "");
    assert_eq!(after("--permission-mode"), "dontAsk");
    assert_eq!(after("--permission-prompts"), "none");
    assert_eq!(after("--mcp-config"), "/tmp/Tasks/task-1.json");
    assert_eq!(
        after("--allowedTools"),
        "mcp__ilo__open_tab,mcp__ilo__finish"
    );
    assert_eq!(after("--model"), "haiku");
    assert_eq!(after("--append-system-prompt-file"), "/tmp/Tasks/task-1.md");
    // The token never goes on the command line, which other users can read.
    assert!(!args.iter().any(|arg| arg.contains("secret")));
}

/// Nothing on Claude's command line has a line break, which Windows can't hand npm's claude.cmd:
/// the instructions, in paragraphs, go in a file (#1676).
#[test]
fn claudes_command_line_has_no_line_break() {
    let mut launch = launch(Cli::Claude);
    launch.instructions = "One.\n\nTwo.\r\n".into();
    let args = claude_args(&launch);
    assert!(
        !args.iter().any(|arg| arg.contains(['\n', '\r'])),
        "{args:?}"
    );
}

/// On Windows, a task starts through an npm .cmd shim, as through an .exe: Rust starts no .cmd
/// with a line break in an argument, which was why 0.1.3's tasks couldn't start (#1676). Claude
/// gets its instructions whole in their file.
#[cfg(windows)]
#[test]
fn a_task_starts_through_a_cmd_shim() {
    let dir = std::env::temp_dir().join(format!("ilo-shim-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A shim that reads all it's given, as claude and codex do.
    let shim = dir.join("claude.cmd");
    std::fs::write(&shim, "@findstr \"^\" > nul\r\n").unwrap();
    let refused = crate::child::command(&shim)
        .arg("One.\n\nTwo.")
        .output()
        .unwrap_err();
    assert_eq!(
        refused.kind(),
        std::io::ErrorKind::InvalidInput,
        "{refused}"
    );
    for cli in Cli::ALL {
        let mut launch = launch(cli);
        launch.program = shim.clone();
        launch.path = None;
        launch.dir = dir.join(format!("{}-task", cli.id()));
        launch.config = dir.join(format!("{}-task.json", cli.id()));
        launch.instructions = "One.\n\nTwo.".into();
        let (events, _said) = futures::channel::mpsc::unbounded();
        let process = Process::start(&launch, "task-1", "Find a flight.\nTo Lisbon.", events)
            .unwrap_or_else(|error| panic!("{} didn't start through a .cmd: {error}", cli.id()));
        if cli == Cli::Claude {
            assert_eq!(
                std::fs::read_to_string(instructions_file(&launch)).unwrap(),
                "One.\n\nTwo."
            );
        }
        drop(process);
    }
    _ = std::fs::remove_dir_all(dir);
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

/// Its thinking goes to the trace as a thought, its text is said, and its tool calls are neither,
/// as the browser sees those itself.
#[test]
fn claudes_text_is_said_its_thinking_is_a_thought_and_its_tool_calls_are_neither() {
    let outs = read(&[
        json!({ "type": "system", "subtype": "init", "tools": ["mcp__ilo__open_tab"] }),
        json!({ "type": "assistant", "message": { "content": [
            { "type": "thinking", "thinking": "Hmm." },
            { "type": "text", "text": "  Looking for flights.  " },
            { "type": "tool_use", "name": "mcp__ilo__open_tab", "input": {} },
        ] } }),
        json!({ "type": "result", "subtype": "success", "is_error": false, "result": "Found one." }),
    ]);
    let Out::Thought { text, .. } = &outs[0] else {
        panic!("{outs:?}")
    };
    assert_eq!(text, "Hmm.");
    assert_eq!(
        outs[1..],
        [
            Out::Said("Looking for flights.".into()),
            Out::Calling,
            Out::Ended(None)
        ]
    );
}

#[test]
fn claudes_text_comes_as_it_writes_it_until_it_calls_a_tool() {
    let piece = |event: Value| json!({ "type": "stream_event", "event": event });
    let delta = |text: &str| {
        piece(
            json!({ "type": "content_block_delta", "delta": { "type": "text_delta", "text": text } }),
        )
    };
    let outs = read(&[
        piece(json!({ "type": "message_start", "message": {} })),
        piece(
            json!({ "type": "content_block_start", "content_block": { "type": "text", "text": "" } }),
        ),
        delta("Two"),
        delta(" shows "),
        json!({ "type": "assistant", "message": { "content": [{ "type": "text", "text": "Two shows" }] } }),
        piece(
            json!({ "type": "content_block_start", "content_block": { "type": "text", "text": "" } }),
        ),
        delta("Both"),
        piece(
            json!({ "type": "content_block_start", "content_block": { "type": "tool_use", "name": "mcp__ilo__open_tab" } }),
        ),
        piece(
            json!({ "type": "content_block_delta", "delta": { "type": "input_json_delta", "partial_json": "{" } }),
        ),
    ]);
    assert_eq!(
        outs,
        [
            Out::Writing(String::new()),
            Out::Writing("Two".into()),
            Out::Writing("Two shows".into()),
            Out::Said("Two shows".into()),
            Out::Writing("Both".into()),
            Out::Calling,
        ]
    );
    assert!(
        claude_args(&launch(Cli::Claude))
            .iter()
            .any(|arg| arg == "--include-partial-messages")
    );
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
    assert_eq!(
        outs,
        vec![Out::Ended(Some(Failure::Limit(Some(1_800_000_000))))]
    );
}

#[test]
fn claudes_limit_in_its_result_says_when_it_resets() {
    let outs = read(&[json!({
        "type": "result", "subtype": "success", "is_error": true,
        "result": "Claude AI usage limit reached|1800000000",
    })]);
    assert_eq!(
        outs,
        vec![Out::Ended(Some(Failure::Limit(Some(1_800_000_000))))]
    );
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
fn a_failure_says_why_in_words_never_a_code() {
    let ended = |result: Value| {
        let outs = read(&[result]);
        match outs.as_slice() {
            [Out::Ended(Some(Failure::Other(why)))] => why.clone(),
            outs => panic!("{outs:?}"),
        }
    };
    let failed = |subtype: &str, text: &str| {
        ended(json!({ "type": "result", "subtype": subtype, "is_error": true, "result": text }))
    };
    assert_eq!(
        ended(json!({ "type": "result", "subtype": "error_max_turns", "is_error": true })),
        "Claude hit its turn limit."
    );
    assert_eq!(
        failed("error_during_execution", ""),
        "Claude ran into an error."
    );
    assert_eq!(
        failed("error_during_execution", "error_during_execution"),
        "Claude ran into an error."
    );
    assert_eq!(
        failed("success", "API Error: 500 {\"type\":\"error\"}"),
        "Claude ran into an error."
    );
    assert_eq!(
        failed("success", "The page wouldn't load."),
        "The page wouldn't load."
    );
}

#[test]
fn codex_turns_off_the_users_servers_by_name() {
    let config = "model = \"x\"\n[mcp_servers.linear]\ncommand = \"a\"\n[mcp_servers.linear.env]\n\
                  [mcp_servers.\"my.server\"]\n[mcp_servers.ilo]\n[profiles.fast]\n";
    assert_eq!(
        codex::servers_in(config),
        vec!["linear".to_string(), "my.server".to_string()]
    );
}

#[test]
fn codexs_thread_has_the_browser_alone() {
    let params = codex::thread_params(&launch(Cli::Codex));
    let config = &params["config"];
    for feature in [
        "shell_tool",
        "unified_exec",
        "apps",
        "plugins",
        "hooks",
        "memories",
        "multi_agent",
    ] {
        assert_eq!(
            config[format!("features.{feature}")],
            json!(false),
            "{feature}"
        );
    }
    assert_eq!(config["web_search"], json!("disabled"));
    assert_eq!(config["project_doc_max_bytes"], json!(0));
    assert_eq!(
        config["mcp_servers.ilo"]["command"],
        json!("/app/mcp-bridge")
    );
    assert_eq!(
        config["mcp_servers.ilo"]["env"]["BROWSER_AGENT_TOKEN"],
        json!("secret")
    );
    assert_eq!(
        config["mcp_servers.ilo"]["enabled_tools"],
        json!(["open_tab", "finish"])
    );
    assert_eq!(
        config["mcp_servers.ilo"]["tools"]["finish"]["approval_mode"],
        json!("approve")
    );
    assert_eq!(
        config["mcp_servers.ilo"]["omit_tools_from"],
        json!(["code_mode"])
    );
    assert_eq!(
        config["mcp_servers.ilo"]["supports_parallel_tool_calls"],
        json!(true)
    );
    assert_eq!(config["agents.max_depth"], json!(0));
    assert_eq!(config["skills.include_instructions"], json!(false));
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
    assert_eq!(
        read(json!({ "id": 2, "result": { "thread": { "id": "t1" } } })),
        vec![]
    );
    assert_eq!(
        read(json!({ "method": "turn/started", "params": { "turn": { "id": "u1" } } })),
        vec![]
    );
    assert_eq!(
        read(
            json!({ "method": "item/completed", "params": { "item": { "type": "agentMessage", "text": "On it." } } })
        ),
        vec![Out::Said("On it.".into())]
    );
    read(
        json!({ "method": "account/rateLimits/updated", "params": { "rateLimits": {
        "primary": { "usedPercent": 100, "resetsAt": 1_800_000_000u64 },
        "secondary": { "usedPercent": 40, "resetsAt": 1_900_000_000u64 },
    } } }),
    );
    read(
        json!({ "method": "error", "params": { "willRetry": true, "error": { "message": "retrying" } } }),
    );
    read(
        json!({ "method": "error", "params": { "willRetry": false, "error": {
        "message": "You've hit your usage limit.", "codexErrorInfo": "usageLimitExceeded",
    } } }),
    );
    assert_eq!(
        read(
            json!({ "method": "turn/completed", "params": { "turn": { "id": "u1", "status": "failed" } } })
        ),
        vec![Out::Ended(Some(Failure::Limit(Some(1_800_000_000))))]
    );
    assert_eq!(
        read(
            json!({ "method": "turn/completed", "params": { "turn": { "id": "u2", "status": "failed",
            "error": { "message": "no", "codexErrorInfo": "unauthorized" } } } })
        ),
        vec![Out::Ended(Some(Failure::SignedOut))]
    );
    assert_eq!(
        read(
            json!({ "method": "turn/completed", "params": { "turn": { "id": "u3", "status": "interrupted" } } })
        ),
        vec![Out::Ended(None)]
    );
}

#[test]
fn claudes_sign_in_status() {
    assert!(crate::agents::ready::claude_signed_in(
        r#"{"loggedIn":true,"authMethod":"claude.ai"}"#
    ));
    assert!(!crate::agents::ready::claude_signed_in(
        r#"{"loggedIn":false}"#
    ));
    assert!(!crate::agents::ready::claude_signed_in("Not logged in"));
}

/// Each model as id and name, as the lists keep them.
fn pairs(models: &[(&str, &str)]) -> Vec<(String, String)> {
    (models.iter())
        .map(|(id, name)| (id.to_string(), name.to_string()))
        .collect()
}

#[test]
fn claudes_models_are_its_own_and_its_default_names_one() {
    use crate::agents::ready::{Models, claude_listed};
    // Claude Code 2.1.322's answer to initialize, cut to its models.
    let answer: Value =
        serde_json::from_str(include_str!("answers/claude-2.1.322-initialize.json")).unwrap();
    assert_eq!(
        claude_listed(&answer),
        Some(Models {
            listed: pairs(&[
                ("opus", "Opus"),
                ("fable", "Fable"),
                ("sonnet", "Sonnet"),
                ("haiku", "Haiku"),
            ]),
            default: Some("Opus".into()),
        })
    );
    // A Claude too old to list them answers without, or with an error.
    let error = json!({ "type": "control_response", "response": {
        "subtype": "error", "request_id": "models", "error": "Unknown subtype: initialize" } });
    assert_eq!(claude_listed(&error), None);
    // An empty list is a list: Default alone.
    let empty = json!({ "type": "control_response", "response": {
        "subtype": "success", "request_id": "models", "response": { "models": [] } } });
    assert_eq!(claude_listed(&empty), Some(Models::default()));
}

#[test]
fn codexs_models_are_kept_without_the_hidden_ones() {
    use crate::agents::ready::{Found, Models, Ready, codex_listed};
    // Codex 0.167.1's answer to model/list, cut to what ilo reads.
    let answer: Value =
        serde_json::from_str(include_str!("answers/codex-0.167.1-model-list.json")).unwrap();
    let listed = codex_listed(&answer);
    assert_eq!(listed.default.as_deref(), Some("GPT-6.1-Sol"));
    assert_eq!(
        listed.listed[..3],
        pairs(&[
            ("gpt-6.1-sol", "GPT-6.1-Sol"),
            ("gpt-6-astra", "GPT-6-Astra"),
            ("gpt-6-sol", "GPT-6-Sol"),
        ])
    );
    assert_eq!(listed.listed.len(), 9);
    let hidden = codex_listed(&json!({ "id": 2, "result": { "data": [
        { "id": "gpt-5-codex", "displayName": "GPT-5 Codex", "hidden": false },
        { "id": "old", "displayName": "Old", "hidden": true, "isDefault": true },
        { "id": "bare" },
    ], "nextCursor": null } }));
    assert_eq!(
        hidden,
        Models {
            listed: pairs(&[("gpt-5-codex", "GPT-5 Codex"), ("bare", "bare")]),
            default: Some("Old".into()),
        },
        "a hidden default is still named"
    );
    let found = Found {
        ready: Ready::SignedIn,
        models: Some(hidden),
        ..Default::default()
    };
    assert_eq!(Found::from_saved(&found.saved()), found, "kept across runs");
    let unasked = Found {
        ready: Ready::SignedIn,
        ..Default::default()
    };
    assert_eq!(Found::from_saved(&unasked.saved()), unasked);
}

#[test]
fn a_list_saved_before_the_cli_named_its_default_is_read() {
    use crate::agents::ready::{Found, Models};
    // As a browser before #2455 kept Codex's, and Claude's empty one, which said nothing.
    let codex = json!({ "ready": "signed-in", "models": [["gpt-5.5", "GPT-5.5"]] });
    assert_eq!(
        Found::from_saved(&codex).models,
        Some(Models {
            listed: pairs(&[("gpt-5.5", "GPT-5.5")]),
            default: None,
        })
    );
    let claude = json!({ "ready": "signed-in", "models": [] });
    assert_eq!(Found::from_saved(&claude).models, None);
}

#[cfg(unix)]
#[test]
fn a_cli_is_found_on_the_path_given() {
    let dir = std::env::temp_dir().join(format!("ilo-find-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let program = dir.join("claude");
    std::fs::write(&program, "#!/bin/sh\n").unwrap();
    let path = format!("/nonexistent:{}", dir.display());
    assert_eq!(
        crate::agents::ready::find("claude", &path),
        None,
        "not executable yet"
    );
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
    launch.server = Server {
        command: "/usr/bin/python3".into(),
        args: vec!["-I".into(), stub.display().to_string()],
        token: "t".into(),
    };
    launch.tools = vec!["probe_echo".into()];
    let (events, mut said) = futures::channel::mpsc::unbounded();
    let mut process = Process::start(
        &launch,
        "task-1",
        "Call probe_echo with the text hi, then say what it answered.",
        events,
    )
    .unwrap();
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
    assert!(
        calls.contains("\"probe_echo\""),
        "the stub's tool was called"
    );
    assert!(
        outs.iter()
            .any(|out| matches!(out, Out::Said(text) if text.contains("echo")))
    );
    assert_eq!(
        outs.iter()
            .filter(|out| matches!(out, Out::Ended(None)))
            .count(),
        2
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// The real `codex` on this computer, as the runner starts it, against the stub: the tools the
/// model says it has, and that the stub's tool is all it calls. One turn on the model named by
/// ILO_CODEX_MODEL, else Codex's default, from the user's plan, so it runs only when asked:
/// `cargo test --release --lib -- --ignored codex_runs_locked_down --nocapture`.
#[cfg(unix)]
#[test]
#[ignore]
fn codex_runs_locked_down() {
    let found = crate::agents::ready::look(Cli::Codex);
    let program = found.program.expect("codex installed");
    let dir = std::env::temp_dir().join(format!("ilo-codex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("stub.py");
    std::fs::write(&stub, STUB).unwrap();
    let mut launch = launch(Cli::Codex);
    (launch.program, launch.path) = (program, found.path);
    launch.dir = dir.join("task");
    launch.config = dir.join("task.json");
    launch.model = std::env::var("ILO_CODEX_MODEL").ok();
    launch.server = Server {
        command: "/usr/bin/python3".into(),
        args: vec!["-I".into(), stub.display().to_string()],
        token: "t".into(),
    };
    launch.tools = vec!["probe_echo".into()];
    let (events, mut said) = futures::channel::mpsc::unbounded();
    let first = "First list the exact names of every tool or function you can call, comma separated. \
                 Then call probe_echo with the text hi, and say what it answered.";
    let process = Process::start(&launch, "task-1", first, events).unwrap();
    let mut outs = Vec::new();
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
    assert!(
        calls.contains("\"probe_echo\""),
        "the stub's tool was called"
    );
    assert!(
        outs.iter()
            .any(|out| matches!(out, Out::Said(text) if text.contains("echo")))
    );
    assert!(
        matches!(outs.last(), Some(Out::Ended(None))),
        "{:?}",
        outs.last()
    );
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
