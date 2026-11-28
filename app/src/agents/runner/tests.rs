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
        effort: None,
        instructions: "Work in the browser.".into(),
        tools: vec!["open_tab".into(), "finish".into()],
        wall: None,
        reads: Vec::new(),
    }
}

/// What a run of `launch(Cli::Claude)`'s says, line by line.
fn read(lines: &[Value]) -> Vec<Out> {
    let mut turn = Turn::of(&launch(Cli::Claude));
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

/// A thread's start sets nothing of its assistant's own helpers, which it has none of: Claude's
/// sets the PATH alone, and Codex's the PATH and its token.
#[test]
fn a_threads_start_sets_none_of_its_assistants_helper_variables() {
    let names = |cli| -> Vec<String> {
        let mut names: Vec<String> = set(&launch(cli))
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        names.sort();
        names
    };
    assert_eq!(names(Cli::Claude), ["PATH"]);
    let mut codex = vec!["PATH".to_string(), crate::agents::bridge::TOKEN.to_string()];
    codex.sort();
    assert_eq!(names(Cli::Codex), codex);
    for cli in [Cli::Claude, Cli::Codex] {
        assert!(
            !kept(cli).iter().any(|name| name.contains("SUBAGENT")),
            "{cli:?}"
        );
    }
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
    // None of its own tools: it searches with the browser's, never its provider's, and never
    // fetches.
    assert_eq!(after("--tools"), "");
    assert!(!args.iter().any(|arg| arg.contains("WebFetch")), "{args:?}");
    assert_eq!(after("--setting-sources"), "");
    assert_eq!(after("--permission-mode"), "dontAsk");
    assert_eq!(after("--permission-prompts"), "none");
    assert_eq!(after("--mcp-config"), "/tmp/Tasks/task-1.json");
    assert_eq!(
        after("--allowedTools"),
        format!("mcp__{SERVER}__open_tab,mcp__{SERVER}__finish")
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

/// A level picked for the thread reaches Claude as its own `--effort`; with none picked, or one
/// its program doesn't list, ilo passes nothing and Claude uses its own default.
#[test]
fn claude_gets_the_picked_effort_and_nothing_else() {
    let mut launch = launch(Cli::Claude);
    let effort = |launch: &Launch| {
        let args = claude_args(launch);
        let at = args.iter().position(|arg| arg == "--effort");
        at.map(|at| args[at + 1].clone())
    };
    assert_eq!(effort(&launch), None, "an effort with nothing picked");
    for level in Cli::Claude.efforts() {
        launch.effort = Some(level.to_string());
        assert_eq!(effort(&launch).as_deref(), Some(*level));
    }
    launch.effort = Some("turbo".into());
    assert_eq!(effort(&launch), None, "a level Claude doesn't list");
}

/// The levels are Claude Code's `--effort` as its help prints them, in its order; Codex's help
/// names no effort option, so it has none.
#[test]
fn only_claude_takes_an_effort() {
    assert_eq!(
        Cli::Claude.efforts(),
        ["low", "medium", "high", "xhigh", "max"]
    );
    assert!(Cli::Codex.efforts().is_empty());
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
        json!({ "mcpServers": { SERVER: {
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
        init(&[&format!("mcp__{SERVER}__open_tab")], &[SERVER]),
        json!({ "type": "assistant", "message": { "content": [
            { "type": "thinking", "thinking": "Hmm." },
            { "type": "text", "text": "  Looking for flights.  " },
            { "type": "tool_use", "name": format!("mcp__{SERVER}__open_tab"), "input": {} },
        ] } }),
        json!({ "type": "result", "subtype": "success", "is_error": false, "result": "Found one." }),
    ]);
    // Its init line, with the browser's server and its tools alone, says nothing.
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
            json!({ "type": "content_block_start", "content_block": { "type": "tool_use", "name": format!("mcp__{SERVER}__open_tab") } }),
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
    let config = &format!(
        "model = \"x\"\n[mcp_servers.linear]\ncommand = \"a\"\n[mcp_servers.linear.env]\n\
         [mcp_servers.\"my.server\"]\n[mcp_servers.{SERVER}]\n[profiles.fast]\n"
    );
    assert_eq!(
        codex::servers_in(config),
        vec!["linear".to_string(), "my.server".to_string()]
    );
}

#[test]
fn codexs_thread_has_the_browser_alone() {
    let params = codex::thread_params(&launch(Cli::Codex), NAME);
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
    // Its provider's search is off whatever the user's config: it searches with the browser's.
    assert_eq!(config["web_search"], json!("disabled"));
    assert!(config.get("tools.web_search").is_none(), "{config}");
    assert_eq!(config["project_doc_max_bytes"], json!(0));
    assert_eq!(
        config[format!("mcp_servers.{SERVER}").as_str()]["command"],
        json!("/app/mcp-bridge")
    );
    assert_eq!(
        config[format!("mcp_servers.{SERVER}").as_str()]["env"]["BROWSER_AGENT_TOKEN"],
        json!("secret")
    );
    assert_eq!(
        config[format!("mcp_servers.{SERVER}").as_str()]["enabled_tools"],
        json!(["open_tab", "finish"])
    );
    assert_eq!(
        config[format!("mcp_servers.{SERVER}").as_str()]["tools"]["finish"]["approval_mode"],
        json!("approve")
    );
    assert_eq!(
        config[format!("mcp_servers.{SERVER}").as_str()]["omit_tools_from"],
        json!(["code_mode"])
    );
    assert_eq!(
        config[format!("mcp_servers.{SERVER}").as_str()]["supports_parallel_tool_calls"],
        json!(true)
    );
    assert_eq!(config["agents.max_depth"], json!(0));
    assert_eq!(config["skills.include_instructions"], json!(false));
    assert_eq!(params["approvalPolicy"], json!("never"));
    assert_eq!(params["permissions"], json!(NAME));
    assert!(params.get("sandbox").is_none(), "{params}");
    assert_eq!(params["ephemeral"], json!(true));
    assert_eq!(params["cwd"], json!("/tmp/Tasks/task-1"));
    assert_eq!(params["model"], json!("haiku"));
}

/// A thread's commands run under ilo's own profile: the root denied, no writes, no network, and
/// for a thread with pages alone no read root at all, so a command reads nothing on this computer.
#[test]
fn a_thread_with_pages_alone_has_no_read_root() {
    let params = codex::thread_params(&launch(Cli::Codex), NAME);
    assert_eq!(
        params["config"][format!("permissions.{NAME}").as_str()],
        json!({ "filesystem": { ":root": "deny" }, "network": { "enabled": false } })
    );
}

/// A thread with files reads those alone, and writes nowhere: ilo's own tools do its file work.
#[test]
fn a_thread_with_files_reads_its_own_alone() {
    let reads = vec![
        PathBuf::from("/Users/a/Granted"),
        PathBuf::from("/tmp/Tasks/task-1-attached"),
    ];
    let params = codex::thread_params(
        &Launch {
            reads,
            ..launch(Cli::Codex)
        },
        NAME,
    );
    assert_eq!(
        params["config"][format!("permissions.{NAME}").as_str()],
        json!({
            "filesystem": {
                ":root": "deny",
                "/Users/a/Granted": "read",
                "/tmp/Tasks/task-1-attached": "read",
            },
            "network": { "enabled": false },
        })
    );
}

/// A thread starts only on a Codex new enough to take the profile, and only once its answer says
/// the profile is the one it runs under. Otherwise it ends as one whose Codex needs an update
/// (AP27), and no turn is asked for.
#[test]
fn no_thread_starts_without_its_profile() {
    for (agent, fits) in [
        (
            Some("ilo/0.172.0 (Mac OS 26.1.0; arm64) iTerm.app/3.6"),
            true,
        ),
        (Some("ilo/0.180.2 (Ubuntu 24.4.0; x86_64) unknown"), true),
        (Some("ilo/1.0.0-alpha.3 (Windows 11; x86_64)"), true),
        (Some("ilo/0.171.9 (Mac OS 26.1.0; arm64)"), false),
        (Some("ilo/0.172 (Mac OS 26.1.0; arm64)"), false),
        (Some("ilo (Mac OS 26.1.0; arm64)"), false),
        (Some(""), false),
        (None, false),
    ] {
        assert_eq!(codex::fits(agent), fits, "{agent:?}");
    }
    // What `codex --version` prints, which Settings › Models' row reads before any thread starts.
    for (printed, fits) in [
        ("codex-cli 0.172.0\n", true),
        ("codex-cli 0.180.2", true),
        ("codex-cli 1.0.0-alpha.3\n", true),
        ("codex-cli 0.171.9\n", false),
        ("codex-cli 0.172\n", false),
        ("codex-cli\n", false),
        ("", false),
        ("codex-cli 0.160.0\ncodex-cli 0.172.0\n", false),
    ] {
        assert_eq!(codex::fits_version(printed), fits, "{printed:?}");
    }
    let stdin = std::sync::Mutex::new(None);
    let launch = launch(Cli::Codex);
    let old = json!({ "id": 1, "result": { "userAgent": "ilo/0.160.0 (Mac OS 26.1.0; arm64)" } });
    assert!(matches!(
        codex::read(&old, &codex::State::default(), &stdin, &launch)[..],
        [Out::Ended(Some(Failure::NeedsUpdate))]
    ));
    // A thread answered before it was asked for names no profile it was asked to start under.
    let unasked = json!({ "id": 2, "result": { "thread": { "id": "t1" },
        "activePermissionProfile": { "id": NAME } } });
    assert!(matches!(
        codex::read(&unasked, &codex::State::default(), &stdin, &launch)[..],
        [Out::Gone(_)]
    ));

    let (one, other) = (codex::State::default(), codex::State::default());
    for state in [&one, &other] {
        assert_eq!(codex::read(&INITIALIZED, state, &stdin, &launch), vec![]);
    }
    let (asked, elsewhere) = (one.profile().unwrap(), other.profile().unwrap());
    for name in [&asked, &elsewhere] {
        let random = name.strip_prefix(codex::PROFILE).expect("ilo's profile");
        assert!(
            random.len() == 16 && random.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{name}"
        );
    }
    assert_ne!(asked, elsewhere, "two starts share a profile's name");
    let answered = |id: Value| json!({ "id": 2, "result": { "thread": { "id": "t1" }, "activePermissionProfile": id } });
    for answer in [
        json!({ "id": 2, "result": { "thread": { "id": "t1" } } }),
        answered(Value::Null),
        answered(json!({ "id": ":read-only" })),
        answered(json!({ "id": ":workspace", "extends": asked })),
        // Another start's, a stale one's, and one a user's config could have named beforehand.
        answered(json!({ "id": elsewhere })),
        answered(json!({ "id": NAME })),
        answered(json!({ "id": codex::PROFILE })),
        answered(json!({ "id": "ilo-thread" })),
    ] {
        assert!(
            matches!(
                codex::read(&answer, &one, &stdin, &launch)[..],
                [Out::Gone(_)]
            ),
            "{answer}"
        );
    }
    assert_eq!(
        codex::read(&answered(json!({ "id": asked })), &one, &stdin, &launch),
        vec![]
    );
}

/// Each start names a profile of its own, so neither the user's config nor another start can have
/// written its table, and one start's answer naming another's profile starts no turn.
#[test]
fn each_start_names_its_own_profile() {
    let names: Vec<String> = (0..2).map(|_| codex::profile_name()).collect();
    assert_ne!(names[0], names[1]);
    let stdin = std::sync::Mutex::new(None);
    let launch = launch(Cli::Codex);
    let (one, other) = (codex::State::default(), codex::State::default());
    for state in [&one, &other] {
        assert_eq!(codex::read(&INITIALIZED, state, &stdin, &launch), vec![]);
    }
    let answer = |state: &codex::State| {
        json!({ "id": 2, "result": { "thread": { "id": "t1" },
            "activePermissionProfile": { "id": state.profile() } } })
    };
    assert_ne!(one.profile(), other.profile());
    assert!(matches!(
        codex::read(&answer(&other), &one, &stdin, &launch)[..],
        [Out::Gone(_)]
    ));
    assert_eq!(codex::read(&answer(&one), &one, &stdin, &launch), vec![]);
}

/// A profile's name for the params' tests, as `codex::profile_name` makes one.
const NAME: &str = "ilo-thread-0123456789abcdef";

/// Codex's answer to initialize, from a version that takes the profile.
static INITIALIZED: std::sync::LazyLock<Value> = std::sync::LazyLock::new(
    || json!({ "id": 1, "result": { "userAgent": "ilo/0.172.0 (Mac OS 26.1.0; arm64)" } }),
);

/// The stand-in row: a stand-in for Codex's app-server, never the real one, keeps what ilo sends
/// it and asks to run a command that reads a file outside the thread's folders. ilo started the
/// thread under the deny-root profile and declines the command. Codex's own sandbox holding the
/// profile is read from its source, not run here.
#[cfg(unix)]
#[test]
fn a_command_reading_outside_the_threads_folders_is_refused() {
    let dir = std::env::temp_dir().join(format!("ilo-codex-reads-{}", std::process::id()));
    _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let outside = dir.join("outside");
    std::fs::write(&outside, "not the thread's").unwrap();
    let run = |name: &str, profile: &str| {
        let stand_in = dir.join(name);
        // It answers thread/start with `profile`, where `$name` is the one it was asked for.
        let script = r#"#!/bin/sh
dir=$(dirname "$0")
while IFS= read -r line; do
printf '%s\n' "$line" >> "$0.heard"
case "$line" in
*'"method":"initialize"'*) echo '{"id":1,"result":{"userAgent":"ilo/0.172.0 (Mac OS 26.1.0; arm64)","codexHome":"/nonexistent","platformFamily":"unix","platformOs":"macos"}}' ;;
*'"method":"thread/start"'*) name=$(printf '%s' "$line" | sed -n 's/.*"permissions":"\([^"]*\)".*/\1/p'); echo '{"id":2,"result":{"thread":{"id":"t1"},"activePermissionProfile":PROFILE}}' ;;
*'"method":"turn/start"'*) echo '{"id":7,"method":"item/commandExecution/requestApproval","params":{"threadId":"t1","turnId":"u1","itemId":"c1","command":"cat '"$dir"'/outside"}}' ;;
*'"id":7'*) echo '{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}' ;;
esac
done
"#
        .replace("PROFILE", profile);
        std::fs::write(&stand_in, script).unwrap();
        std::fs::set_permissions(
            &stand_in,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let mut launch = launch(Cli::Codex);
        (launch.program, launch.path) = (stand_in.clone(), Some("/usr/bin:/bin".into()));
        launch.dir = dir.join(format!("{name}-task"));
        launch.config = dir.join(format!("{name}-task.json"));
        let (events, mut said) = futures::channel::mpsc::unbounded();
        let process = Process::start(&launch, "task-1", "Read the file.", events).unwrap();
        let mut outs = Vec::new();
        while let Some(event) = futures::executor::block_on(futures::StreamExt::next(&mut said)) {
            let ended = matches!(event.out, Out::Ended(_) | Out::Gone(_));
            outs.push(event.out);
            if ended {
                break;
            }
        }
        drop(process);
        let heard = std::fs::read_to_string(stand_in.with_extension("heard")).unwrap_or_default();
        let heard: Vec<Value> = (heard.lines())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        (outs, heard)
    };
    let method = |heard: &[Value], name: &str| {
        (heard.iter())
            .find(|line| line["method"] == json!(name))
            .cloned()
    };

    let (outs, heard) = run("held", r#"{"id":"'"$name"'"}"#);
    let start = method(&heard, "thread/start").expect("a thread was asked for");
    let name = start["params"]["permissions"].as_str().unwrap_or_default();
    assert!(name.starts_with(codex::PROFILE), "{start}");
    assert!(start["params"].get("sandbox").is_none(), "{start}");
    assert_eq!(
        start["params"]["config"][format!("permissions.{name}").as_str()],
        json!({ "filesystem": { ":root": "deny" }, "network": { "enabled": false } })
    );
    let answer = (heard.iter())
        .find(|line| line["id"] == json!(7))
        .expect("the command was answered");
    assert_eq!(answer["result"], json!({ "decision": "decline" }));
    assert_eq!(outs.last(), Some(&Out::Ended(None)));

    // One that doesn't say it took the profile it was asked for gets no turn, so nothing it
    // could run is asked: none, or a stale start's.
    for (name, profile) in [
        ("unheld", "null"),
        ("stale", r#"{"id":"ilo-thread-0000000000000000"}"#),
    ] {
        let (outs, heard) = run(name, profile);
        assert!(method(&heard, "thread/start").is_some(), "{name}");
        assert!(method(&heard, "turn/start").is_none(), "{name}: {heard:?}");
        assert!(
            matches!(outs.last(), Some(Out::Gone(_))),
            "{name}: {outs:?}"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn codexs_messages_limits_and_failures() {
    let state = codex::State::default();
    let stdin = std::sync::Mutex::new(None);
    let launch = launch(Cli::Codex);
    let read = |line: Value| codex::read(&line, &state, &stdin, &launch);
    assert_eq!(read(INITIALIZED.clone()), vec![]);
    assert_eq!(
        read(json!({ "id": 2, "result": { "thread": { "id": "t1" },
            "activePermissionProfile": { "id": state.profile() } } })),
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
fn claudes_models_are_its_own_named_with_their_versions_and_its_default_names_one() {
    use crate::agents::ready::{Models, claude_listed};
    // Claude Code 2.1.322's answer to initialize, cut to its models.
    let answer: Value =
        serde_json::from_str(include_str!("answers/claude-2.1.322-initialize.json")).unwrap();
    assert_eq!(
        claude_listed(&answer),
        Some(Models {
            listed: pairs(&[
                ("opus", "Opus 5.5"),
                ("fable", "Fable 5.2"),
                ("sonnet", "Sonnet 5.5"),
                ("haiku", "Haiku 5.5"),
            ]),
            default: Some("Opus 5.5".into()),
            default_id: Some("opus".into()),
            more: Vec::new(),
        })
    );
    // A model whose version can't be read keeps the name Claude gives it.
    let odd = json!({ "type": "control_response", "response": {
        "subtype": "success", "request_id": "models", "response": { "models": [
            { "value": "big", "resolvedModel": "claude-sonnet-5-5[1m]", "displayName": "Sonnet (1M)" },
            { "value": "bare", "resolvedModel": "claude-next" },
        ] } } });
    let odd = claude_listed(&odd).unwrap();
    assert_eq!(
        odd.listed,
        pairs(&[("big", "Sonnet (1M)"), ("bare", "bare")])
    );
    assert_eq!(
        (odd.default, odd.default_id),
        (None, None),
        "no default entry, none named"
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
fn a_claude_models_version_is_read_from_the_id_it_resolves_to() {
    use crate::agents::ready::claude_version_name;
    let named = |id| claude_version_name(id);
    assert_eq!(named("claude-opus-5-5").as_deref(), Some("Opus 5.5"));
    assert_eq!(named("claude-fable-5-2").as_deref(), Some("Fable 5.2"));
    assert_eq!(named("claude-opus-5").as_deref(), Some("Opus 5"));
    assert_eq!(
        named("claude-haiku-4-5-20251001").as_deref(),
        Some("Haiku 4.5"),
        "date dropped"
    );
    for odd in [
        "claude-sonnet-5-5[1m]",
        "claude-next",
        "claude-3-5-sonnet-20241022",
        "gpt-6-luna",
        "claude-opus-five",
        "",
    ] {
        assert_eq!(named(odd), None, "{odd}");
    }
}

#[test]
fn codexs_models_are_kept_with_the_hidden_ones_apart() {
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
            default_id: Some("old".into()),
            more: pairs(&[("old", "Old")]),
        },
        "a hidden default is still named, and the hidden ones kept for More Models"
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
            ..Models::default()
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

/// A start inside a wall whose listener is up, as put_up answers one.
fn listening(cli: Cli) -> Launch {
    Launch {
        wall: Some(Arc::new(Wall::for_test_listening(cli))),
        ..launch(cli)
    }
}

/// Whether two names are one variable, as the started program reads them: on Windows in any case,
/// where a name keeps the spelling it was first given on the command, so a proxy name cleared in
/// lower case and then set in upper case is spelled in lower case.
fn same_name(one: &str, other: &str) -> bool {
    one == other || (cfg!(windows) && one.eq_ignore_ascii_case(other))
}

/// A walled start of either assistant names the wall's listener as its every proxy and clears
/// what would exempt a host from it or name another; a start with no wall names none and clears
/// nothing, as before.
#[test]
fn only_a_walled_start_names_the_listener_as_its_proxy() {
    let proxies = ["HTTPS_PROXY", "HTTP_PROXY", "ALL_PROXY"];
    let cleared = [
        "NO_PROXY",
        "no_proxy",
        "https_proxy",
        "http_proxy",
        "all_proxy",
    ];
    let removed = |launch: &Launch| -> Vec<String> {
        let mut command = crate::child::command(&launch.program);
        environment(&mut command, launch);
        (command.get_envs())
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect()
    };
    for cli in Cli::ALL {
        let inside = listening(cli);
        let port = (inside.wall.as_ref()).and_then(|wall| wall.port()).unwrap();
        let named = format!("http://127.0.0.1:{port}");
        let has = set(&inside);
        for name in proxies {
            let at = (has.iter())
                .find(|(set, _)| same_name(set, name))
                .map(|(_, value)| value);
            assert_eq!(at, Some(&named), "{cli:?}: {name}");
        }
        let gone = removed(&inside);
        for name in cleared {
            let reads_listener =
                (has.iter()).any(|(set, value)| same_name(set, name) && *value == named);
            assert!(
                gone.iter().any(|gone| same_name(gone, name)) || reads_listener,
                "{cli:?}: {name} isn't cleared"
            );
            assert!(
                !has.iter()
                    .any(|(set, value)| same_name(set, name) && *value != named),
                "{cli:?}: {name} names something else"
            );
        }

        let outside = launch(cli);
        let has = set(&outside);
        for name in proxies.iter().chain(&cleared) {
            assert!(
                !has.iter().any(|(set, _)| same_name(set, name)),
                "{cli:?}: {name} is set"
            );
        }
        assert!(
            removed(&outside).is_empty(),
            "{cli:?}: a start with no wall clears a name"
        );
    }
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
        let starts = [
            launch(cli),
            walled(cli, None),
            walled(cli, Some("/nonexistent/tmp")),
            listening(cli),
        ];
        for start in starts {
            for (name, _) in set(&start) {
                assert!(
                    names.iter().any(|kept| same_name(kept, &name)),
                    "{name} is set and not kept"
                );
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
    let wall = Arc::new(Wall::inside(&rules, &names).unwrap());
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

/// A walled run's reap sweeps its place and answers its task with what the sweep found, by the
/// run's own process; a wall with no place, or a run with no wall, sweeps nothing and says
/// nothing. Off the Mac nothing is inside a wall, so the sweep finds nothing in one walk.
#[test]
fn a_walled_runs_reap_answers_its_task_with_the_sweep() {
    let root = std::env::temp_dir().join(format!("ilo-reaped-{}", std::process::id()));
    let place = leftovers::Place::make(&root).unwrap();
    let wall = Wall::for_test(None).in_place(Arc::new(place));
    let (events, mut heard) = futures::channel::mpsc::unbounded();
    let to = || Some(("task".to_string(), events.clone()));
    reaped(Some(&wall), 7, to());
    let event = heard.try_recv().unwrap();
    let swept = leftovers::Swept { ended: 0, walks: 1 };
    assert_eq!((event.agent.as_str(), event.run), ("task", 7));
    assert_eq!(event.out, Out::Swept(Ok(swept)));
    reaped(Some(&Wall::for_test(None)), 8, to());
    reaped(None, 9, to());
    assert!(heard.try_recv().is_err(), "nothing more was said");
    _ = std::fs::remove_dir_all(&root);
}

/// Claude's start has neither its provider's search nor its fetch, walled or not. The wall's hosts
/// are the ones it had.
#[test]
fn claude_has_neither_its_providers_search_nor_its_fetch() {
    for launch in [launch(Cli::Claude), walled(Cli::Claude, None)] {
        let args = claude_args(&launch);
        let after = |flag: &str| {
            let at =
                (args.iter().position(|arg| arg == flag)).unwrap_or_else(|| panic!("no {flag}"));
            args[at + 1].as_str()
        };
        assert_eq!(after("--tools"), "");
        assert!(
            !args.iter().any(|arg| arg.contains("WebSearch")),
            "{args:?}"
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

/// Claude's init line, listing `tools` and the servers `servers`.
fn init(tools: &[&str], servers: &[&str]) -> Value {
    let servers: Vec<Value> = (servers.iter())
        .map(|name| json!({ "name": name, "status": "connected" }))
        .collect();
    json!({ "type": "system", "subtype": "init", "tools": tools, "mcp_servers": servers })
}

/// Its arguments give Claude none of its own tools and no helper of its own: `--tools ""`, and no
/// search, fetch, helper tool, helper kinds or helper denials, walled or not.
#[test]
fn claudes_tools_are_the_browsers_alone() {
    for launch in [launch(Cli::Claude), walled(Cli::Claude, None)] {
        let args = claude_args(&launch);
        let at = (args.iter().position(|arg| arg == "--tools")).expect("--tools");
        assert_eq!(args[at + 1], "");
        let allowed = (args.iter().position(|arg| arg == "--allowedTools")).expect("allowed");
        let ours = format!("mcp__{SERVER}__");
        assert!(
            args[allowed + 1]
                .split(',')
                .all(|tool| tool.starts_with(&ours))
        );
        for theirs in [
            "Agent",
            "Task",
            "WebSearch",
            "WebFetch",
            "--agents",
            "--disallowedTools",
        ] {
            let named = |arg: &String| arg == theirs || arg.split(',').any(|each| each == theirs);
            assert!(!args.iter().any(named), "{theirs} in {args:?}");
        }
    }
}

/// A user's own servers and settings never reach a thread's Claude, whatever their own config
/// holds (Hole 7): it reads the browser's config alone, which holds the browser's server alone,
/// and no settings file.
#[test]
fn a_users_own_servers_and_settings_never_reach_a_thread() {
    let launch = launch(Cli::Claude);
    let args = claude_args(&launch);
    let after = |flag: &str| {
        let at = (args.iter().position(|arg| arg == flag)).unwrap_or_else(|| panic!("no {flag}"));
        args[at + 1].as_str()
    };
    assert!(
        args.iter().any(|arg| arg == "--strict-mcp-config"),
        "{args:?}"
    );
    assert_eq!(args.iter().filter(|arg| *arg == "--mcp-config").count(), 1);
    assert_eq!(after("--mcp-config"), launch.config.to_string_lossy());
    let config: Value = serde_json::from_str(&claude_config(&launch.server)).unwrap();
    let servers = config["mcpServers"].as_object().expect("servers");
    assert_eq!(servers.keys().collect::<Vec<_>>(), [SERVER]);
    assert_eq!(after("--setting-sources"), "");
}

/// An init line with the browser's server and only its tools says nothing. Any other ends the run
/// there, before its first answer, and nothing it says after is read: a tool of Claude's own, a
/// second server, a tool of the browser's this thread wasn't given, no tools, or a line ilo can't
/// read.
#[test]
fn an_init_line_with_a_tool_of_claudes_own_ends_the_run() {
    let ours = |tool: &str| format!("mcp__{SERVER}__{tool}");
    let (open, finish) = (ours("open_tab"), ours("finish"));
    assert_eq!(read(&[init(&[&open, &finish], &[SERVER])]), []);
    let refused = [
        init(&[&open, "Bash"], &[SERVER]),
        init(&["WebSearch", &finish], &[SERVER]),
        init(&[&open, "Agent"], &[SERVER]),
        init(&[&open], &[SERVER, "the-users-own"]),
        init(
            &[&open, "mcp__the-users-own__run"],
            &[SERVER, "the-users-own"],
        ),
        init(&[&open], &[]),
        init(&[&ours("click")], &[SERVER]),
        init(&[], &[SERVER]),
        json!({ "type": "system", "subtype": "init", "mcp_servers": [{ "name": SERVER }] }),
        json!({ "type": "system", "subtype": "init", "tools": [&open] }),
        json!({ "type": "system", "subtype": "init", "tools": [1], "mcp_servers": [{ "name": SERVER }] }),
    ];
    // What follows at once, words and a call of the browser's own, is nothing: no Said, no
    // Calling, no end of a turn.
    let said = json!({ "type": "assistant", "message": { "content": [
        { "type": "text", "text": "Opening it." },
        { "type": "tool_use", "id": "o1", "name": &open, "input": {} },
    ] } });
    let calls = json!({ "type": "stream_event", "event": {
        "type": "content_block_start",
        "content_block": { "type": "tool_use", "id": "o2", "name": &open, "input": {} },
    } });
    let ended = json!({ "type": "result", "subtype": "success", "is_error": false, "result": "" });
    for line in refused {
        let outs = read(&[line.clone(), calls.clone(), said.clone(), ended.clone()]);
        assert!(matches!(outs[..], [Out::Refused(_)]), "{line}: {outs:?}");
    }
}

/// A call of a tool of Claude's own ends the run as it starts, or once its message comes whole,
/// whatever its init line said, and nothing of the call is read: its words aren't in why.
#[test]
fn a_call_of_claudes_own_tool_ends_the_run() {
    let search = json!({ "type": "assistant", "message": { "content": [
        { "type": "text", "text": "Searching." },
        { "type": "tool_use", "id": "s1", "name": "WebSearch", "input": {
            "query": "night trains to Lisbon",
        } },
    ] } });
    let result = json!({ "type": "user", "message": { "content": [
        { "type": "tool_result", "tool_use_id": "s1",
          "content": "Night train · https://timetables.example/lisbon" },
    ] } });
    let outs = read(&[search, result]);
    let [Out::Refused(why)] = &outs[..] else {
        panic!("{outs:?}")
    };
    assert!(
        !why.contains("Lisbon") && !why.contains("timetables"),
        "{why}"
    );
    let starts = json!({ "type": "stream_event", "event": {
        "type": "content_block_start",
        "content_block": { "type": "tool_use", "id": "b1", "name": "Bash", "input": {} },
    } });
    assert!(matches!(read(&[starts])[..], [Out::Refused(_)]));
    // One of the browser's is a call like any.
    let ours = json!({ "type": "assistant", "message": { "content": [
        { "type": "tool_use", "id": "o1", "name": format!("mcp__{SERVER}__open_tab"), "input": {} },
    ] } });
    assert_eq!(read(&[ours]), [Out::Calling]);
}

/// The stand-in row: a stand-in for Claude Code, never the real one, reads its first message,
/// says it started with Bash, and would act two seconds later. ilo ends it there, before it can:
/// the run is refused, then gone, and it never acted.
#[cfg(unix)]
#[test]
fn a_run_whose_init_lists_another_tool_ends_before_it_acts() {
    let dir = std::env::temp_dir().join(format!("ilo-claude-init-{}", std::process::id()));
    _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let stand_in = dir.join("claude");
    let script = r#"#!/bin/sh
dir=$(dirname "$0")
IFS= read -r line
echo '{"type":"system","subtype":"init","tools":["Bash","mcp__SERVER__open_tab"],"mcp_servers":[{"name":"SERVER","status":"connected"}]}'
sleep 2
touch "$dir/acted"
echo '{"type":"result","subtype":"success","is_error":false,"result":"Done."}'
"#
    .replace("SERVER", SERVER);
    std::fs::write(&stand_in, script).unwrap();
    std::fs::set_permissions(
        &stand_in,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let mut launch = launch(Cli::Claude);
    (launch.program, launch.path) = (stand_in, Some("/usr/bin:/bin".into()));
    launch.dir = dir.join("task");
    launch.config = dir.join("task.json");
    let (events, mut said) = futures::channel::mpsc::unbounded();
    let process = Process::start(&launch, "task-1", "Open it.", events).unwrap();
    let mut outs = Vec::new();
    while let Some(event) = futures::executor::block_on(futures::StreamExt::next(&mut said)) {
        let gone = matches!(event.out, Out::Gone(_));
        outs.push(event.out);
        if gone {
            break;
        }
    }
    assert!(
        matches!(outs[..], [Out::Refused(_), Out::Gone(_)]),
        "{outs:?}"
    );
    drop(process);
    assert!(!dir.join("acted").exists(), "the stand-in acted");
    _ = std::fs::remove_dir_all(&dir);
}

/// Codex's own search item, which its config turns off, says nothing, whatever its action.
#[test]
fn codexs_own_search_says_nothing() {
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
    for action in [
        json!({ "type": "search", "query": "night trains to Lisbon" }),
        json!({ "type": "search", "queries": ["trains", "buses"] }),
        json!({ "type": "openPage", "url": "https://timetables.example/lisbon" }),
        Value::Null,
    ] {
        for method in ["item/started", "item/completed"] {
            assert_eq!(read(item(method, action.clone())), [], "{method} {action}");
        }
    }
}
