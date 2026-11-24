//! What a task is started with, the switch off and on, and what ilo reads of helpers from Claude's
//! output: the made-up lines of `claude-helpers-written.jsonl`, which no run recorded.

use std::path::PathBuf;

use serde_json::Value;

use super::*;
use crate::agents::runner::{Cli, Launch, Server, Turn, claude_args, claude_read};

/// The helpers' lines, as written.
const LINES: &str = include_str!("../runner/answers/claude-helpers-written.jsonl");

fn launch(helpers: bool) -> Launch {
    Launch {
        cli: Cli::Claude,
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
        instructions: String::new(),
        tools: ["open_tab", "read_page", "title", "ask", "finish"]
            .map(String::from)
            .into(),
        helpers,
    }
}

/// What the lines say, read as the task's output is.
fn read() -> Vec<Out> {
    let mut turn = Turn::default();
    (LINES.lines())
        .map(|line| serde_json::from_str::<Value>(line).expect("a line isn't JSON"))
        .flat_map(|line| claude_read(&line, &mut turn))
        .collect()
}

/// The switch is on only over a recorded run: while the lines say they were written, it's off.
#[test]
fn the_switch_is_off_while_no_run_is_recorded() {
    let written = LINES.lines().next().unwrap().contains("\"_written\"");
    assert!(
        written,
        "the first line no longer says the lines are made up"
    );
    assert!(!ON, "helpers are on over lines no run recorded");
}

/// With the switch off a task's assistant starts with the arguments it had before helpers, each
/// one and their order: none of its own tools, and nothing that names a helper.
#[test]
fn off_claude_starts_with_the_arguments_it_had() {
    let had = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--tools",
        "",
        "--strict-mcp-config",
        "--mcp-config",
        "/tmp/Tasks/task-1.json",
        "--setting-sources",
        "",
        "--permission-mode",
        "dontAsk",
        "--permission-prompts",
        "none",
        "--disable-slash-commands",
        "--no-session-persistence",
        "--allowedTools",
        "mcp__ilo__open_tab,mcp__ilo__read_page,mcp__ilo__title,mcp__ilo__ask,mcp__ilo__finish",
        "--model",
        "haiku",
        "--append-system-prompt-file",
        "/tmp/Tasks/task-1.md",
    ];
    assert_eq!(claude_args(&launch(false)), had);
}

/// On, it gets the one tool that starts a helper and ilo's kind of helper, which has the browser's
/// tools without the task's own and without the tool that starts one. Nothing else changes: what
/// it's allowed stays the browser's tools alone.
#[test]
fn on_claude_gets_the_helper_tool_and_ilos_kind_of_helper() {
    let (off, mut on) = (claude_args(&launch(false)), claude_args(&launch(true)));
    let kinds = on.pop().expect("the kinds");
    assert_eq!(on.pop().as_deref(), Some("--agents"));
    let tools = on.iter().position(|arg| arg == "--tools").unwrap() + 1;
    assert_eq!(on[tools], TOOLS[0]);
    on[tools] = String::new();
    assert_eq!(on, off, "more than the helper tool and the kinds changed");

    let kinds: Value = serde_json::from_str(&kinds).expect("the kinds aren't JSON");
    let kinds = kinds.as_object().unwrap();
    assert_eq!(kinds.keys().collect::<Vec<_>>(), [KIND]);
    let has: Vec<&str> = (kinds[KIND]["tools"].as_array().unwrap().iter())
        .map(|tool| tool.as_str().unwrap())
        .collect();
    assert_eq!(has, ["mcp__ilo__open_tab", "mcp__ilo__read_page"]);
    assert!(!kinds[KIND]["prompt"].as_str().unwrap().is_empty());
    // The task is told the kind by the name it's described under.
    assert!(instructions().contains(KIND));
}

/// A helper's lines are never the task's: of two helpers that write, say, think and read, the
/// task's output tells who asked for each, that each started and how deep, and the tab each
/// calls for. Nothing a helper wrote, said, thought or was answered comes out, and the task's own
/// words do.
#[test]
fn a_helpers_lines_are_never_the_tasks() {
    let said = read();
    let helper = |id: &str, told| Out::Helper {
        id: id.into(),
        told,
    };
    assert_eq!(
        said,
        [
            Out::Said("I'll look at both shops at once.".into()),
            Out::Calling,
            Out::Calling,
            helper("toolu_task_a", Helper::Asked),
            helper("toolu_task_b", Helper::Asked),
            Out::Started { deep: 1 },
            Out::Started { deep: 1 },
            helper("toolu_task_a", Helper::On(41)),
            // A tab's id comes as a number or as its text.
            helper("toolu_task_b", Helper::On(42)),
            Out::Said("Two seats together in row 12 at tickets.example, 38 € each.".into()),
            Out::Ended(None),
        ]
    );
    assert!(
        LINES.matches("HELPER-").count() >= 5,
        "the lines lost the helpers' words"
    );
    assert!(!format!("{said:?}").contains("HELPER-"));
}

/// Only the browser's tools label a helper's page: a call of another tool with a `tab` of its own
/// says nothing, and a start that doesn't say how deep counts as the task's own level.
#[test]
fn only_the_browsers_tools_label_a_page() {
    let other = serde_json::json!({ "type": "assistant", "parent_tool_use_id": "toolu_task_a",
        "message": { "content": [
            { "type": "tool_use", "id": "x", "name": "mcp__other__click", "input": { "tab": 41 } },
            { "type": "tool_use", "id": "y", "name": "mcp__ilo__list_tabs", "input": {} },
        ] } });
    assert_eq!(claude_read(&other, &mut Turn::default()), []);
    let started = serde_json::json!({ "type": "system", "subtype": "task_started" });
    assert_eq!(
        claude_read(&started, &mut Turn::default()),
        [Out::Started { deep: 0 }]
    );
    let progress = serde_json::json!({ "type": "system", "subtype": "task_progress" });
    assert_eq!(claude_read(&progress, &mut Turn::default()), []);
}

/// A helper's start is counted whoever's line says so. One on a helper's own line, with a parent,
/// says how deep it is, or without a depth is taken as a helper's helper, deeper than any may be;
/// nothing else of a helper's `system` lines comes out.
#[test]
fn a_start_on_a_helpers_own_line_is_counted() {
    let line = |depth: Option<u64>, subtype: &str| {
        let mut line = serde_json::json!({ "type": "system", "subtype": subtype,
            "parent_tool_use_id": "toolu_task_a" });
        if let Some(depth) = depth {
            line["spawn_depth"] = depth.into();
        }
        claude_read(&line, &mut Turn::default())
    };
    let deeper = u64::from(DEEP) + 1;
    assert_eq!(line(None, "task_started"), [Out::Started { deep: deeper }]);
    assert_eq!(line(Some(1), "task_started"), [Out::Started { deep: 1 }]);
    assert_eq!(line(Some(1), "task_progress"), []);
    // Only a `system` line starts one, whatever another's fields say.
    let said = serde_json::json!({ "type": "assistant", "subtype": "task_started" });
    assert_eq!(claude_read(&said, &mut Turn::default()), []);
}
