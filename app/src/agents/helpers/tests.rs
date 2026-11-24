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
        wall: None,
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
    // Read today, that first line ends the run before its first answer.
    assert_eq!(read().first(), Some(&Out::Kinds { ours: false }));
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
    // The assistant's own kinds are denied by name, the task over again among them.
    let denied = on.pop().expect("the denied");
    assert_eq!(on.pop().as_deref(), Some("--disallowedTools"));
    let denied: Vec<&str> = denied.split(',').collect();
    assert_eq!(denied.len(), NOT_OURS.len());
    assert!(denied.contains(&"Agent(fork)"), "{denied:?}");
    assert!(!denied.iter().any(|each| each.contains(KIND)), "{denied:?}");
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
    assert!(told(Limit::Count, 1).contains("more than 1 helper."));
    assert!(told(Limit::Count, several).contains(&format!("more than {several} helpers.")));
    for most in [1, several] {
        let told = told(Limit::Count, most);
        assert!(told.contains(&format!("at most {most} for each message")));
        assert!(told.starts_with("ilo stopped that turn."), "{told}");
    }
    let deep = told(Limit::Depth, several);
    assert!(
        deep.contains("A helper started a helper of its own."),
        "{deep}"
    );
    assert!(!deep.contains("You started"), "{deep}");
    // A task ilo stopped is told so, and the kind it may start by its name.
    let kind = told(Limit::Kind, several);
    assert!(kind.starts_with("ilo stopped this task."), "{kind}");
    assert!(deep.starts_with("ilo stopped this task."), "{deep}");
    assert!(kind.contains(KIND), "{kind}");
}

/// A helper's lines are never the task's. Of the two helpers the recorded task starts, its output
/// tells who asked for each, and that each started in the background, which read today stops the
/// task at the first; what the helpers were answered doesn't come out, and the task's own words
/// do. Each helper's first call opens its tab, which names none, so neither has a page for its
/// label before the turn ends.
#[test]
fn a_helpers_lines_are_never_the_tasks() {
    let said = read();
    let asked: Vec<(&str, &str)> = (said.iter())
        .filter_map(|out| match out {
            Out::Helper {
                id,
                told: Helper::Asked(name),
            } => Some((id.as_str(), name.as_str())),
            _ => None,
        })
        .collect();
    // Each by the name the task gave it, which its line says.
    let names: Vec<&str> = asked.iter().map(|(_, name)| *name).collect();
    assert_eq!(names, ["Northwick Home price", "Pell & Marrow price"]);
    let asked: Vec<&str> = asked.iter().map(|(id, _)| *id).collect();
    let starts: Vec<&Out> = (said.iter())
        .filter(|out| matches!(out, Out::Started { .. } | Out::NotAllowed(_)))
        .collect();
    let background = Out::NotAllowed(Limit::Kind);
    assert_eq!(starts, [&background, &background]);
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
/// says nothing, and a start of ilo's kind that doesn't say how deep counts as the task's own
/// level. Nothing a helper writes, says or thinks comes out. The lines are made up.
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
    let started = serde_json::json!({ "type": "system", "subtype": "task_started",
        "subagent_type": KIND });
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
            "subagent_type": KIND, "parent_tool_use_id": "toolu_task_a" });
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

/// The lines below are made up: no run has shown the assistant started as `ENV` starts it.
fn made_up(line: Value) -> Vec<Out> {
    claude_read(&line, &mut Turn::default())
}

/// A run's first line says whether each kind of helper it lists is ilo's. One that lists another,
/// or no list ilo can read, isn't; one that lists none has none to start.
#[test]
fn a_first_line_says_whether_its_kinds_are_ilos() {
    let init = |agents: Option<Value>| {
        let mut line = serde_json::json!({ "type": "system", "subtype": "init", "tools": [] });
        if let Some(agents) = agents {
            line["agents"] = agents;
        }
        made_up(line)
    };
    let kinds = |ours| vec![Out::Kinds { ours }];
    assert_eq!(init(Some(serde_json::json!([KIND]))), kinds(true));
    assert_eq!(init(Some(serde_json::json!([]))), kinds(true));
    for other in ["general-purpose", "Explore", "fork", "page-helper-2"] {
        assert_eq!(init(Some(serde_json::json!([KIND, other]))), kinds(false));
    }
    assert_eq!(init(None), kinds(false));
    assert_eq!(init(Some(serde_json::json!("page-helper"))), kinds(false));
    assert_eq!(
        init(Some(serde_json::json!([{ "name": KIND }]))),
        kinds(false)
    );
    // Only a `system` line's `init` is a first line.
    let status = serde_json::json!({ "type": "system", "subtype": "status", "agents": ["x"] });
    assert_eq!(made_up(status), []);
}

/// The task's own line that asks for a helper says so before the helper starts: one of ilo's
/// kind is asked for, and one of another kind, of none, or for the background isn't allowed.
#[test]
fn a_helper_asked_for_as_another_kind_isnt_allowed() {
    let asks = |input: Value| {
        made_up(
            serde_json::json!({ "type": "assistant", "message": { "content": [
            { "type": "tool_use", "id": "toolu_a", "name": TOOLS[0], "input": input },
        ] } }),
        )
    };
    let asked = |name: &str| Out::Helper {
        id: "toolu_a".into(),
        told: Helper::Asked(name.into()),
    };
    let ours = serde_json::json!({ "subagent_type": KIND, "prompt": "One shop." });
    assert_eq!(asks(ours), [Out::Calling, asked("")]);
    // The name it gives a helper is one line of plain text, cut short: it stands on a line of
    // the conversation, where it can't run on, break the line or turn its words around.
    let long = format!("Shop\none\u{202e} price {}", "x".repeat(200));
    let named = serde_json::json!({ "subagent_type": KIND, "description": long });
    let [
        _,
        Out::Helper {
            told: Helper::Asked(name),
            ..
        },
    ] = &asks(named)[..]
    else {
        panic!("not asked for");
    };
    assert!(name.starts_with("Shop one price x"), "{name}");
    assert_eq!(name.chars().count(), tier::TEXT_MOST);
    let refused = [Out::Calling, Out::NotAllowed(Limit::Kind)];
    for input in [
        serde_json::json!({ "subagent_type": "general-purpose", "prompt": "One shop." }),
        serde_json::json!({ "subagent_type": "fork", "prompt": "One shop." }),
        serde_json::json!({ "prompt": "One shop." }),
        serde_json::json!({ "subagent_type": KIND, "run_in_background": true }),
    ] {
        assert_eq!(asks(input.clone()), refused, "{input}");
    }
}

/// A start says its kind and whether the task waits for it: another kind than ilo's, none, or
/// one in the background isn't allowed, and one deeper than a helper may be is told as deep
/// whatever its kind, as that's what the task is told of it.
#[test]
fn a_start_of_another_kind_or_in_the_background_isnt_allowed() {
    let start = |fields: Value| {
        let mut line = serde_json::json!({ "type": "system", "subtype": "task_started" });
        for (key, value) in fields.as_object().unwrap() {
            line[key] = value.clone();
        }
        made_up(line)
    };
    let ours = serde_json::json!({ "subagent_type": KIND, "spawn_depth": 1,
        "is_backgrounded": false });
    assert_eq!(start(ours), [Out::Started { deep: 1 }]);
    let refused = [Out::NotAllowed(Limit::Kind)];
    for fields in [
        serde_json::json!({ "subagent_type": "general-purpose", "spawn_depth": 1 }),
        serde_json::json!({ "spawn_depth": 1 }),
        serde_json::json!({ "subagent_type": KIND, "spawn_depth": 1, "is_backgrounded": true }),
    ] {
        assert_eq!(start(fields.clone()), refused, "{fields}");
    }
    let deeper = u64::from(DEEP) + 1;
    for kind in [KIND, "general-purpose"] {
        let deep = serde_json::json!({ "subagent_type": kind, "spawn_depth": deeper });
        assert_eq!(start(deep), [Out::Started { deep: deeper }]);
    }
}

/// A helper's own line that starts a helper, or calls a tool that is the task's alone, says only
/// that: the task's name, its question, its end and a tab handed to the user aren't a helper's.
#[test]
fn a_helpers_line_that_calls_the_tasks_own_tools_isnt_allowed() {
    let calls = |name: &str| {
        made_up(
            serde_json::json!({ "type": "assistant", "parent_tool_use_id": "toolu_a",
            "message": { "content": [
                { "type": "tool_use", "id": "a", "name": "mcp__ilo__read_page",
                  "input": { "tab": 41 } },
                { "type": "tool_use", "id": "b", "name": name, "input": { "tab": 41 } },
            ] } }),
        )
    };
    for tool in TOOLS {
        assert_eq!(calls(tool), [Out::NotAllowed(Limit::Depth)], "{tool}");
    }
    for tool in ["title", "ask", "finish", "done", "hand_off"] {
        let name = format!("mcp__ilo__{tool}");
        assert_eq!(calls(&name), [Out::NotAllowed(Limit::Kind)], "{tool}");
    }
    // The task's own line calls them as it always has.
    let own = serde_json::json!({ "type": "assistant", "message": { "content": [
        { "type": "tool_use", "id": "a", "name": "mcp__ilo__title", "input": { "text": "Shops" } },
    ] } });
    assert_eq!(made_up(own), [Out::Calling]);
    // And ilo's kind is described without any of them.
    let mut launch = launch(true);
    launch.tools.extend(["hand_off", "done"].map(String::from));
    let args = claude_args(&launch);
    let kinds: Value = serde_json::from_str(args.last().unwrap()).unwrap();
    let has = kinds[KIND]["tools"].to_string();
    assert_eq!(has, r#"["mcp__ilo__open_tab","mcp__ilo__read_page"]"#);
}
