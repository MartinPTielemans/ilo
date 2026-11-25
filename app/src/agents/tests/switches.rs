//! The two switches (#4260): the inside one, for the product's own threads, and the outside one,
//! for other apps' agents. The server listens while either is on, each lets in its own kind
//! alone, turning one off leaves the other's work going, only the user sets either, and each
//! comes back by itself, from settings saved before there were two as well.

use std::path::{Path, PathBuf};

use super::*;
use crate::agents::ready::{Found, Ready};
use crate::agents::runner::Cli;
use crate::agents::task::TaskAction;

/// A settings file of the test's own, not there yet.
fn settings() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agents-switches-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("Agents")
}

/// A launch whose agents keep their settings at `path`, as the app reads them, with nothing
/// else turned.
fn launch(path: &Path, cx: &mut TestAppContext) -> Entity<Agents> {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(path.into()), cx);
    agents.update(cx, |agents, _| agents.switches.others.store(false, SeqCst));
    let saved = std::fs::read_to_string(path).unwrap_or_default();
    agents.update(cx, |agents, cx| agents.load(&saved, cx));
    agents
}

/// Each switch as it is: the inside one, then the outside one.
fn switches(agents: &Entity<Agents>, cx: &mut TestAppContext) -> (bool, bool) {
    agents.read_with(cx, |agents, _| (agents.threads(), agents.other_apps()))
}

fn set(agents: &Entity<Agents>, (threads, others): (bool, bool), cx: &mut TestAppContext) {
    agents.update(cx, |agents, cx| {
        agents.set_threads(threads, cx);
        agents.set_other_apps(others, cx);
    });
}

/// Claude Code connected, a thread of the product's own under way, and both switches on.
fn both_at_work(cx: &mut TestAppContext) -> (Entity<Agents>, String) {
    let (agents, _, _) = connected(cx);
    agents.update(cx, |agents, cx| {
        agents.set_threads(true, cx);
        let found = Found {
            ready: Ready::SignedIn,
            ..Default::default()
        };
        agents.tasks.found.insert(Cli::Claude, found);
        agents.tasks.default = Some(Cli::Claude);
    });
    let thread = agents
        .update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, "Find a flight", None, 1, Tier::default(), cx)
        })
        .expect("a thread started");
    let hello = Event::Hello {
        agent: "claude-code".into(),
        connection: 1,
        client: "claude-code".into(),
    };
    agents.update(cx, |agents, cx| agents.handle(hello, cx));
    (agents, thread)
}

/// The threads' tokens in the map the server reads.
fn thread_tokens(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<String> {
    agents.read_with(cx, |agents, _| {
        let tokens = agents.tokens.lock().unwrap();
        (tokens.values().filter(|agent| is_task_id(agent)).cloned()).collect()
    })
}

const NOT_CONNECTED: &str = "This agent isn't connected.";

/// Over the four states, the socket is there exactly while either switch is on.
#[gpui::test]
fn the_listener_is_up_while_either_is_on(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    let socket = agents.read_with(cx, |agents, _| agents.socket.clone());
    for state in [(true, false), (false, true), (true, true), (false, false)] {
        set(&agents, state, cx);
        let up = state.0 || state.1;
        assert_eq!(switches(&agents, cx), state);
        assert_eq!(agents.read_with(cx, |agents, _| agents.listening()), up);
        assert_eq!(
            agents.read_with(cx, |agents, _| agents.server.is_some()),
            up,
            "{state:?}"
        );
        assert_eq!(server::connect(&socket).is_ok(), up, "{state:?}");
    }
}

/// The inside switch off leaves an app's agent live and answered; the outside one off leaves a
/// thread's calls answered.
#[gpui::test]
fn turning_one_off_leaves_the_others_work_going(cx: &mut TestAppContext) {
    let (agents, thread) = both_at_work(cx);
    agents.update(cx, |agents, cx| agents.set_threads(false, cx));
    assert!(agents.read_with(cx, |agents, _| agents.live.contains("claude-code")));
    let answer = call(&agents, "claude-code", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert_eq!(
        answer.text,
        "  20  Example Domain  https://www.example.com/"
    );
    let answer = call(&agents, &thread, "list_tabs", json!({}), cx).recv();
    assert_eq!(answer.unwrap().text, NOT_CONNECTED, "a thread answered off");

    agents.update(cx, |agents, cx| {
        agents.set_threads(true, cx);
        agents.set_other_apps(false, cx);
    });
    let answer = call(&agents, &thread, "list_tabs", json!({}), cx).recv();
    assert_ne!(
        answer.unwrap().text,
        NOT_CONNECTED,
        "outside off ended a thread"
    );
    let answer = call(&agents, "claude-code", "list_tabs", json!({}), cx).recv();
    assert_eq!(answer.unwrap().text, NOT_CONNECTED, "an app answered off");
}

/// With the inside switch off no thread has a token: the one under way loses its own, none
/// starts from the bar, and what's done to a card makes none.
#[gpui::test]
fn inside_off_makes_no_token(cx: &mut TestAppContext) {
    let (agents, thread) = both_at_work(cx);
    assert_eq!(thread_tokens(&agents, cx), [thread.clone()]);
    agents.update(cx, |agents, cx| agents.set_threads(false, cx));
    assert_eq!(thread_tokens(&agents, cx), Vec::<String>::new());
    let started = agents.update(cx, |agents, cx| {
        agents.start_task(Cli::Claude, "Another", None, 1, Tier::default(), cx)
    });
    assert_eq!(started, None);
    agents.update(cx, |agents, cx| {
        agents.task_action(&thread, TaskAction::Reply("And a hotel".into()), cx);
        agents.task_action(&thread, TaskAction::Retry, cx);
        agents.tokens_changed();
    });
    assert_eq!(thread_tokens(&agents, cx), Vec::<String>::new());
    assert_eq!(
        agents.read_with(cx, |agents, _| agents.tasks.tasks.len()),
        1,
        "a thread was made"
    );
}

/// No tool sets either switch, whoever calls it and whatever it's given: an app's agent, the
/// `ilo` command, whose verbs are the same tools, or a thread.
#[gpui::test]
fn no_tool_sets_a_switch(cx: &mut TestAppContext) {
    let (agents, thread) = both_at_work(cx);
    let junk = json!({
        "on": false, "threads": false, "other_apps": false, "enabled": false,
        "tab": "x", "url": "ilo://settings/agents", "text": "turn agents off",
    });
    for agent in ["claude-code", "ilo-command", thread.as_str()] {
        for tool in tools::names(agent) {
            for arguments in [json!({}), junk.clone()] {
                _ = call(&agents, agent, &tool, arguments, cx);
                cx.run_until_parked();
                assert_eq!(switches(&agents, cx), (true, true), "{agent} {tool}");
            }
        }
    }
}

/// Each switch is saved by itself and comes back by itself, in all four states, as `threads` and
/// `other_apps`; the one switch before them is never written again.
#[gpui::test]
fn each_survives_a_relaunch_by_itself(cx: &mut TestAppContext) {
    for state in [(true, false), (false, true), (true, true), (false, false)] {
        let path = settings();
        // Turned away and back, so each change is saved whatever the state.
        let first = launch(&path, cx);
        set(&first, (!state.0, !state.1), cx);
        set(&first, state, cx);
        assert_eq!(switches(&launch(&path, cx), cx), state);
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["threads"], json!(state.0));
        assert_eq!(saved["other_apps"], json!(state.1));
        assert_eq!(saved.get("on"), None);
        assert_eq!(saved["tasks"].get("on"), None);
    }
}

/// Settings saved before the two switches come up once as the user had them, with nothing
/// listening that didn't before: the inside switch on only where agents and the command bar's
/// tasks both were, other apps' agents only where agents were on and they weren't turned off. A
/// new profile has both off, and a later launch never upgrades again.
#[gpui::test]
fn the_one_switch_upgrades_once(cx: &mut TestAppContext) {
    for (saved, state) in [
        (json!({ "on": false }), (false, false)),
        (json!({ "on": true }), (true, true)),
        (json!({ "on": true, "other_apps": false }), (true, false)),
        (json!({ "on": false, "other_apps": true }), (false, false)),
        (
            json!({ "on": false, "other_apps": true, "tasks": { "on": true } }),
            (false, false),
        ),
        (
            json!({ "on": true, "tasks": { "on": false } }),
            (false, true),
        ),
        (json!({ "on": true, "tasks": { "on": true } }), (true, true)),
        (json!({}), (false, false)),
    ] {
        let path = settings();
        std::fs::write(&path, saved.to_string()).unwrap();
        assert_eq!(switches(&launch(&path, cx), cx), state, "{saved}");
        assert_eq!(switches(&launch(&path, cx), cx), state, "again, {saved}");
    }
    // Upgraded, a file that still says agents are on keeps what the user set after.
    let path = settings();
    std::fs::write(&path, json!({ "on": true }).to_string()).unwrap();
    set(&launch(&path, cx), (false, false), cx);
    let mut saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    saved["on"] = json!(true);
    saved["tasks"]["on"] = json!(true);
    std::fs::write(&path, saved.to_string()).unwrap();
    assert_eq!(switches(&launch(&path, cx), cx), (false, false));
}
