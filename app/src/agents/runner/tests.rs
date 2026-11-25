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
        helpers: false,
        wall: None,
    }
}

fn read(lines: &[Value]) -> Vec<Out> {
    let mut turn = Turn::default();
    lines
        .iter()
        .flat_map(|line| claude_read(line, &mut turn))
        .collect()
}

/// What ilo sets on a CLI's process, by name.
fn set(launch: &Launch) -> Vec<(String, String)> {
    let mut command = crate::child::command(&launch.program);
    environment(&mut command, launch);
    (command.get_envs())
        .filter_map(|(name, value)| {
            let text = |text: &std::ffi::OsStr| text.to_string_lossy().into_owned();
            Some((text(name), text(value?)))
        })
        .collect()
}

/// A task without helpers is started with none of the helpers' variables, and one with them has
/// each, set on its process, so ilo's value is the one it reads whatever ilo's own environment
/// holds. Codex's start has none either way.
#[test]
fn only_a_task_with_helpers_is_started_with_their_variables() {
    let helpers = |set: &[(String, String)]| -> Vec<(String, String)> {
        let ours = |(name, _): &&(String, String)| name.starts_with("CLAUDE_");
        set.iter().filter(ours).cloned().collect()
    };
    let off = set(&launch(Cli::Claude));
    assert_eq!(helpers(&off), []);
    assert_eq!(off.len(), 1, "more than the PATH is set: {off:?}");
    let mut on = launch(Cli::Claude);
    on.helpers = true;
    let wanted: Vec<(String, String)> = (crate::agents::helpers::ENV.iter())
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    let mut has = helpers(&set(&on));
    has.sort();
    let mut sorted = wanted.clone();
    sorted.sort();
    assert_eq!(has, sorted);
    for name in [
        "CLAUDE_AGENT_SDK_DISABLE_BUILTIN_AGENTS",
        "CLAUDE_CODE_FORK_SUBAGENT",
        "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS",
        "CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH",
    ] {
        assert!(wanted.iter().any(|(each, _)| each == name), "{name}");
    }
    assert_eq!(helpers(&set(&launch(Cli::Codex))), []);
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
    // Of its own tools only its provider's search, which reads no page here; never its fetch.
    assert_eq!(after("--tools"), "WebSearch");
    assert!(!args.iter().any(|arg| arg.contains("WebFetch")), "{args:?}");
    assert_eq!(after("--setting-sources"), "");
    assert_eq!(after("--permission-mode"), "dontAsk");
    assert_eq!(after("--permission-prompts"), "none");
    assert_eq!(after("--mcp-config"), "/tmp/Tasks/task-1.json");
    assert_eq!(
        after("--allowedTools"),
        "mcp__ilo__open_tab,mcp__ilo__finish,WebSearch"
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
    // Its first line lists no kind of helper, which is read as one that isn't ilo's (helpers.rs).
    assert_eq!(outs[0], Out::Kinds { ours: false });
    let Out::Thought { text, .. } = &outs[1] else {
        panic!("{outs:?}")
    };
    assert_eq!(text, "Hmm.");
    assert_eq!(
        outs[2..],
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
    // Its provider's search over its own index, which opens no page, whatever the user's config.
    assert_eq!(config["web_search"], json!("cached"));
    assert!(config.get("tools.web_search").is_none(), "{config}");
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

/// A start inside a wall, as put_up answers one for a task with files.
fn walled(cli: Cli, tmp: Option<&str>) -> Launch {
    Launch {
        wall: Some(Arc::new(Wall::for_test(tmp.map(PathBuf::from)))),
        ..launch(cli)
    }
}

/// A start with no wall is as it was: the PATH alone, and no setting of Claude Code's own.
/// Inside a wall Claude Code is started with each knob that keeps its own files out of the
/// user's folders, its temporary files in the wall's folder for them, and its own sandbox off
/// (#4101). It keeps no conversation there, as on any run that isn't kept.
#[test]
fn only_a_walled_claude_is_started_with_the_walls_knobs() {
    let plain = launch(Cli::Claude);
    let path = ("PATH".to_string(), "/usr/local/bin:/usr/bin".to_string());
    assert_eq!(set(&plain), [path]);
    assert!(!claude_args(&plain).iter().any(|arg| arg == "--settings"));

    let inside = walled(Cli::Claude, Some("/nonexistent/tmp"));
    let knobs = set(&inside);
    for name in [
        "CLAUDE_CODE_SKIP_PROMPT_HISTORY",
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
        "CLAUDE_CODE_DISABLE_FILE_CHECKPOINTING",
        "DISABLE_UPDATES",
        "DISABLE_TELEMETRY",
    ] {
        let on = (name.to_string(), "1".to_string());
        assert!(knobs.contains(&on), "{name} isn't on: {knobs:?}");
    }
    let tmp = ("CLAUDE_CODE_TMPDIR".to_string(), "/nonexistent/tmp".into());
    assert!(knobs.contains(&tmp), "{knobs:?}");
    let args = claude_args(&inside);
    let at = (args.iter().position(|arg| arg == "--settings")).expect("no --settings");
    let settings: Value = serde_json::from_str(&args[at + 1]).unwrap();
    assert_eq!(settings, json!({ "sandbox": { "enabled": false } }));
    assert!(args.iter().any(|arg| arg == "--no-session-persistence"));

    // A wall with no folder for them names none, and Codex's start is as it was.
    let bare = set(&walled(Cli::Claude, None));
    assert!(!bare.iter().any(|(name, _)| name == "CLAUDE_CODE_TMPDIR"));
    assert_eq!(set(&walled(Cli::Codex, None)), set(&launch(Cli::Codex)));
}

/// Every name a start sets on its process is one the wall's launcher is told to keep: the wall
/// drops every other on the way, and nothing would say so. The list itself is one the launcher
/// takes: no name twice, and no more than it reads.
#[test]
fn every_name_a_start_sets_is_in_the_kept_list() {
    for cli in Cli::ALL {
        let names = kept(cli);
        let mut once = names.clone();
        once.sort();
        once.dedup();
        assert_eq!(once.len(), names.len(), "{cli:?} keeps a name twice");
        assert!(names.len() <= crate::agents::wall::launch::KEPT_MAX);
        // Claude Code's alone, as a task's helpers are.
        let with_helpers = Launch {
            helpers: cli == Cli::Claude,
            ..walled(cli, None)
        };
        let starts = [
            launch(cli),
            with_helpers,
            walled(cli, Some("/nonexistent/tmp")),
        ];
        for start in starts {
            for (name, _) in set(&start) {
                assert!(names.contains(&name.as_str()), "{name} is set and not kept");
            }
        }
    }
}

/// Inside a wall told to keep the runner's names, a program has each name the start set and no
/// name outside the list, whatever the browser's own environment holds and whatever else is
/// set on the command.
#[cfg(target_os = "macos")]
#[test]
fn a_walled_start_carries_no_name_outside_the_list() {
    use crate::agents::wall::launch::Rules;
    let rules = Rules {
        text: "(version 1)(allow default)".into(),
        params: Vec::new(),
    };
    let names = kept(Cli::Claude);
    let wall = Arc::new(Wall::inside(None, &rules, &names).unwrap());
    let start = Launch {
        wall: Some(wall.clone()),
        ..launch(Cli::Claude)
    };
    let mut command = wall.command(std::path::Path::new("/usr/bin/env"));
    environment(&mut command, &start);
    command
        .env("ILO_NOT_KEPT", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, rising) = wall.start(&mut command).unwrap();
    drop(child.stdin.take());
    assert_eq!(rising.expect("a walled start").risen(), Ok(()));
    let output = child.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    // Names alone are read: a value is never the test's to print.
    let has: Vec<&str> = (out.lines())
        .filter_map(|line| line.split('=').next())
        .collect();
    for name in &has {
        assert!(names.contains(name), "{name} reached the program");
    }
    for (name, _) in set(&start) {
        assert!(has.contains(&name.as_str()), "{name} was set and lost");
    }
}

/// Claude's start has its provider's search and never its fetch, walled or not. The wall's hosts
/// are the ones it had, as the search rides the assistant's own call to its service.
#[test]
fn claude_searches_by_its_providers_search_and_never_fetches() {
    for launch in [launch(Cli::Claude), walled(Cli::Claude, None)] {
        let args = claude_args(&launch);
        let after = |flag: &str| {
            let at =
                (args.iter().position(|arg| arg == flag)).unwrap_or_else(|| panic!("no {flag}"));
            args[at + 1].as_str()
        };
        assert_eq!(after("--tools"), "WebSearch");
        assert!(
            after("--allowedTools")
                .split(',')
                .any(|tool| tool == "WebSearch")
        );
        assert_eq!(after("--permission-mode"), "dontAsk");
        assert!(!args.iter().any(|arg| arg.contains("WebFetch")), "{args:?}");
    }
    use crate::agents::wall::listener::hosts;
    assert_eq!(
        hosts(Cli::Claude),
        ["api.anthropic.com", "platform.claude.com"]
    );
    assert_eq!(hosts(Cli::Codex), ["chatgpt.com", "auth.openai.com"]);
}

/// Claude's search says its words alone: not the sites it was held to, and nothing of the result
/// that comes back in a message of the user's.
#[test]
fn claudes_search_is_its_words_alone_and_its_result_says_nothing() {
    let mut turn = Turn::default();
    let search = json!({ "type": "assistant", "message": { "content": [
        { "type": "tool_use", "id": "s1", "name": "WebSearch", "input": {
            "query": "night trains to Lisbon",
            "allowed_domains": ["timetables.example"],
            "blocked_domains": ["ads.example"],
        } },
    ] } });
    assert_eq!(
        claude_read(&search, &mut turn),
        [Out::Calling, Out::Searched("night trains to Lisbon".into())]
    );
    let result = json!({ "type": "user", "message": { "content": [
        { "type": "tool_result", "tool_use_id": "s1",
          "content": "Night train · https://timetables.example/lisbon" },
    ] } });
    assert_eq!(claude_read(&result, &mut turn), []);
    // Its start as it's written says only that a call comes, its words still to come.
    let piece = json!({ "type": "stream_event", "event": { "type": "content_block_start",
        "content_block": { "type": "tool_use", "id": "s2", "name": "WebSearch", "input": {} } } });
    assert_eq!(claude_read(&piece, &mut turn), [Out::Calling]);
    // Another tool's query is no search.
    let other = json!({ "type": "assistant", "message": { "content": [
        { "type": "tool_use", "id": "s3", "name": "mcp__ilo__open_tab", "input": { "query": "x" } },
    ] } });
    assert_eq!(claude_read(&other, &mut turn), [Out::Calling]);
}

/// Codex's search says a search action's words alone. A page opened or a word found in one, its
/// other actions, says nothing and is no error, nor are the item's own `query`, which names such
/// a page, and its results ever read.
#[test]
fn codexs_search_is_a_search_actions_words_alone() {
    let state = codex::State::default();
    let stdin = std::sync::Mutex::new(None);
    let launch = launch(Cli::Codex);
    let read = |line: Value| codex::read(&line, &state, &stdin, &launch);
    let item = |method: &str, action: Value| {
        json!({ "method": method, "params": { "item": {
            "type": "webSearch", "id": "w1", "query": "https://timetables.example/lisbon",
            "action": action,
            "results": [{ "title": "Night train", "url": "https://timetables.example/lisbon" }],
        } } })
    };
    let search = json!({ "type": "search", "query": "night trains to Lisbon" });
    assert_eq!(read(item("item/started", search.clone())), []);
    assert_eq!(
        read(item("item/completed", search)),
        [Out::Searched("night trains to Lisbon".into())]
    );
    assert_eq!(
        read(item(
            "item/completed",
            json!({ "type": "search", "queries": ["trains", "buses"] })
        )),
        [
            Out::Searched("trains".into()),
            Out::Searched("buses".into())
        ]
    );
    for action in [
        json!({ "type": "openPage", "url": "https://timetables.example/lisbon" }),
        json!({ "type": "findInPage", "url": "https://timetables.example/lisbon", "pattern": "22:05" }),
        json!({ "type": "other" }),
        Value::Null,
    ] {
        assert_eq!(read(item("item/completed", action.clone())), [], "{action}");
    }
}
