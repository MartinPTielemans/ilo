//! What a task is started with, the switch off and on, and what ilo reads of helpers from Claude's
//! output: the first message of the one recorded run, `claude-2.1.331-helpers.jsonl`.

use std::path::PathBuf;

use serde_json::Value;

use super::*;
use crate::agents::runner::{Cli, Launch, Server, Turn, claude_args, claude_read};

/// The recorded run's first message: the task starts two helpers and its turn ends with both at
/// work.
const LINES: &str = include_str!("../runner/answers/claude-2.1.331-helpers.jsonl");

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

/// The recorded lines, each as its JSON.
fn lines() -> Vec<Value> {
    (LINES.lines())
        .map(|line| serde_json::from_str(line).expect("a line isn't JSON"))
        .collect()
}

/// What the lines say, read as the task's output is.
fn read() -> Vec<Out> {
    let mut turn = Turn::default();
    (lines().iter())
        .flat_map(|line| claude_read(line, &mut turn))
        .collect()
}

/// The switch is off while the assistant keeps kinds of helper that aren't ilo's: the recorded
/// run was started as a task with helpers is, and its first line still lists them. One of them
/// has the task's own tools.
#[test]
fn the_switch_is_off_while_the_assistant_keeps_kinds_of_its_own() {
    let lines = lines();
    assert!(
        lines[0].get("_recorded").is_some(),
        "the lines no longer say they were recorded"
    );
    let init = (lines.iter())
        .find(|line| line["subtype"] == "init")
        .expect("no init line");
    let kinds: Vec<&str> = (init["agents"].as_array().unwrap().iter())
        .map(|kind| kind.as_str().unwrap())
        .collect();
    assert!(kinds.contains(&KIND), "{kinds:?}");
    let own = kinds.iter().filter(|kind| **kind != KIND).count();
    assert!(
        own == 0 || !ON,
        "helpers are on with {own} kinds that aren't ilo's"
    );
    // The helper tool is among its tools, under one of the names ilo knows it by.
    let tools = init["tools"].as_array().unwrap();
    assert!(
        tools
            .iter()
            .any(|tool| TOOLS.contains(&tool.as_str().unwrap()))
    );
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

/// What ilo tells a task it stopped counts as a sentence does, one helper or several, with the
/// limit it's held to, and a helper's own helper is said as that and not as the task's count.
#[test]
fn a_stopped_task_is_told_its_limit_as_a_sentence() {
    let several = MOST.max(2);
    assert!(told(false, 1).contains("more than 1 helper."));
    assert!(told(false, several).contains(&format!("more than {several} helpers.")));
    for most in [1, several] {
        assert!(told(false, most).contains(&format!("at most {most} for each message")));
    }
    let deep = told(true, several);
    assert!(
        deep.contains("A helper started a helper of its own."),
        "{deep}"
    );
    assert!(!deep.contains("You started"), "{deep}");
}

/// A helper's lines are never the task's. Of the two helpers the recorded task starts, its output
/// tells who asked for each and that each started, one level under the task; what the helpers
/// were answered doesn't come out, and the task's own words do. Each helper's first call opens its
/// tab, which names none, so neither has a page for its label before the turn ends.
#[test]
fn a_helpers_lines_are_never_the_tasks() {
    let said = read();
    let asked: Vec<&str> = (said.iter())
        .filter_map(|out| match out {
            Out::Helper {
                id,
                told: Helper::Asked,
            } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(asked.len(), 2, "{asked:?}");
    let starts: Vec<&Out> = (said.iter())
        .filter(|out| matches!(out, Out::Started { .. }))
        .collect();
    assert_eq!(
        starts,
        [&Out::Started { deep: 1 }, &Out::Started { deep: 1 }]
    );
    // Each start names the call that asked for it, and each helper's lines name it as their parent.
    let lines = lines();
    for id in &asked {
        let started = lines
            .iter()
            .any(|line| line["subtype"] == "task_started" && line["tool_use_id"] == *id);
        assert!(started, "no start names {id}");
        assert!(lines.iter().any(|line| line["parent_tool_use_id"] == *id));
    }
    let labelled = |out: &&Out| {
        matches!(
            out,
            Out::Helper {
                told: Helper::On(_),
                ..
            }
        )
    };
    assert_eq!(said.iter().filter(labelled).count(), 0);

    // What a helper was answered is in the lines and not in what the task said.
    let answered = "Opened tab 7.";
    assert!(LINES.contains(answered), "the lines lost a helper's answer");
    assert!(!format!("{said:?}").contains(answered));
    let last = "Now I'm waiting for the other two shops.";
    let own = |out: &&Out| matches!(out, Out::Said(text) if text.ends_with(last));
    assert_eq!(said.iter().filter(own).count(), 1);
    assert_eq!(said.last(), Some(&Out::Ended(None)));
}

/// Only the browser's tools label a helper's page: a call of another tool with a `tab` of its own
/// says nothing, and a start that doesn't say how deep counts as the task's own level. Nothing a
/// helper writes, says or thinks comes out.
#[test]
fn only_the_browsers_tools_label_a_page() {
    let helper = |told| Out::Helper {
        id: "toolu_task_a".into(),
        told,
    };
    // A tab's id comes as a number or as its text.
    let calls = serde_json::json!({ "type": "assistant", "parent_tool_use_id": "toolu_task_a",
        "message": { "content": [
            { "type": "text", "text": "The seats page lists row 12." },
            { "type": "thinking", "thinking": "The button is the second one." },
            { "type": "tool_use", "id": "a", "name": "mcp__ilo__read_page", "input": { "tab": 41 } },
            { "type": "tool_use", "id": "b", "name": "mcp__ilo__click",
              "input": { "tab": "42", "ref": "e2" } },
        ] } });
    assert_eq!(
        claude_read(&calls, &mut Turn::default()),
        [helper(Helper::On(41)), helper(Helper::On(42))]
    );
    let writing = serde_json::json!({ "type": "stream_event", "parent_tool_use_id": "toolu_task_a",
        "event": { "type": "content_block_delta",
                   "delta": { "type": "text_delta", "text": "A helper is writing this" } } });
    assert_eq!(claude_read(&writing, &mut Turn::default()), []);
    let answered = serde_json::json!({ "type": "user", "parent_tool_use_id": "toolu_task_a",
        "message": { "content": [
            { "type": "tool_result", "tool_use_id": "a", "content": "Row 12, seats 4 and 5." },
        ] } });
    assert_eq!(claude_read(&answered, &mut Turn::default()), []);
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
