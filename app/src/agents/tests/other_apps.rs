//! Other apps' agents' switch (#4260): off for a new profile, on once for one that had agents on
//! before it had a switch of its own, and while it's off an app's agent is answered as with agents
//! off and its sessions end, while the command bar's tasks never notice.

use std::path::{Path, PathBuf};

use super::*;
use crate::agents::ready::{Found, Ready};
use crate::agents::runner::Cli;

/// A settings file of the test's own, not there yet.
fn settings() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agents-other-apps-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("Agents")
}

/// A launch of the browser whose agents keep their settings at `path`, as the app reads them.
fn launch(path: &Path, cx: &mut TestAppContext) -> Entity<Agents> {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(path.into()), cx);
    let saved = std::fs::read_to_string(path).unwrap_or_default();
    agents.update(cx, |agents, cx| agents.load(&saved, cx));
    agents
}

fn other_apps(agents: &Entity<Agents>, cx: &mut TestAppContext) -> bool {
    agents.read_with(cx, |agents, _| agents.other_apps())
}

/// What the settings file says of the switch.
fn saved(path: &Path) -> Option<bool> {
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    saved.get("other_apps").and_then(Value::as_bool)
}

/// A new profile has other apps' agents off, agents on or not, and the user's switch comes back.
#[gpui::test]
fn a_new_profile_has_other_apps_off_and_the_switch_comes_back(cx: &mut TestAppContext) {
    let path = settings();
    let agents = launch(&path, cx);
    assert!(!other_apps(&agents, cx));
    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    assert!(!other_apps(&agents, cx), "threads on turned other apps on");
    assert_eq!(saved(&path), Some(false));

    agents.update(cx, |agents, cx| agents.set_other_apps(true, cx));
    assert_eq!(saved(&path), Some(true));
    assert!(other_apps(&launch(&path, cx), cx));
}

/// Settings saved with agents on, before other apps had a switch of their own, keep other apps on
/// at the first launch that has it, which saves it: turned off, it stays off at the next. With
/// agents off they start off, and agents turned on later don't carry them.
#[gpui::test]
fn agents_on_carries_other_apps_on_once(cx: &mut TestAppContext) {
    let path = settings();
    std::fs::write(&path, json!({ "on": true }).to_string()).unwrap();
    let first = launch(&path, cx);
    assert!(other_apps(&first, cx));
    assert_eq!(saved(&path), Some(true), "the carry-over wasn't saved");
    first.update(cx, |agents, cx| agents.set_other_apps(false, cx));
    assert!(!other_apps(&launch(&path, cx), cx), "carried over again");

    let path = settings();
    std::fs::write(&path, json!({ "on": false }).to_string()).unwrap();
    let first = launch(&path, cx);
    assert!(!other_apps(&first, cx));
    first.update(cx, |agents, cx| agents.set_threads(true, cx));
    assert!(
        !other_apps(&launch(&path, cx), cx),
        "carried over a later on"
    );
}

/// Connects to the agents' server with `token`, and gives its answer and the connection, which
/// reads None once closed.
fn open(
    agents: &Entity<Agents>,
    token: &str,
    cx: &mut TestAppContext,
) -> (Value, impl FnMut() -> Option<String> + use<>) {
    let socket = agents.read_with(cx, |agents, _| agents.socket.clone());
    let stream = server::connect(&socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    std::io::Write::write_all(
        &mut writer,
        format!("{}\n", json!({ "token": token })).as_bytes(),
    )
    .unwrap();
    let mut reader = std::io::BufReader::new(stream);
    let answer = server::read_line(&mut reader).unwrap_or_default();
    let answer = serde_json::from_str(&answer).unwrap_or_default();
    (answer, move || server::read_line(&mut reader))
}

fn token_of(agents: &Entity<Agents>, agent: &str, cx: &mut TestAppContext) -> String {
    agents.read_with(cx, |agents, _| {
        let tokens = agents.tokens.lock().unwrap();
        let token = tokens.iter().find(|(_, whose)| *whose == agent);
        token.expect("its token").0.clone()
    })
}

/// With agents on and other apps' agents off, Claude Code's connection is answered as with agents
/// off and closed, and makes no task, card or activity entry, while a task from the command bar
/// starts and its own token is taken.
#[gpui::test]
fn with_other_apps_off_an_apps_connection_makes_nothing_and_a_task_starts(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    agents.update(cx, |agents, cx| {
        agents.set_other_apps(false, cx);
        agents.set_threads(true, cx);
    });
    let token = token_of(&agents, "claude-code", cx);
    // What the user's Connect logged, before the connection.
    let logged = log(&agents, "claude-code", cx);
    let (answer, mut read) = open(&agents, &token, cx);
    assert_eq!(answer, json!({ "error": bridge::off() }));
    assert_eq!(read(), None, "a refused connection stayed open");
    cx.run_until_parked();
    agents.read_with(cx, |agents, cx| {
        assert!(agents.tasks.tasks.is_empty(), "a task was made");
        assert!(agents.shown_cards(cx).is_empty(), "a card was made");
        assert!(!agents.live.contains("claude-code"));
    });
    assert_eq!(
        log(&agents, "claude-code", cx),
        logged,
        "the connection was logged"
    );

    agents.update(cx, |agents, _| {
        let found = Found {
            ready: Ready::SignedIn,
            ..Default::default()
        };
        agents.tasks.found.insert(Cli::Claude, found);
        agents.tasks.default = Some(Cli::Claude);
    });
    let started = agents.update(cx, |agents, cx| {
        agents.start_task(Cli::Claude, "Find a flight", None, 1, Tier::default(), cx)
    });
    let task = started.expect("the command bar's task didn't start");
    let token = token_of(&agents, &task, cx);
    assert_eq!(open(&agents, &token, cx).0, json!({ "ok": true }));
}

/// Turning other apps' agents off ends Claude Code's session as agents off does: the call it waits
/// on is dropped, its connection closes, the browser hears it's gone and its log says so, while
/// the server runs on and it stays connected.
#[gpui::test]
fn turning_other_apps_off_ends_their_sessions(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    let token = token_of(&agents, "claude-code", cx);
    let (answer, mut read) = open(&agents, &token, cx);
    assert_eq!(answer, json!({ "ok": true }));
    let hello = Event::Hello {
        agent: "claude-code".into(),
        connection: 1,
        client: "claude-code".into(),
    };
    agents.update(cx, |agents, cx| agents.handle(hello, cx));
    let (reply, waiting) = mpsc::channel();
    let call = Call {
        id: 1,
        agent: "claude-code".into(),
        connection: 1,
        tool: "read_page".into(),
        arguments: json!({ "tab": 20 }),
        reply,
        at: std::time::Instant::now(),
    };
    agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));

    agents.update(cx, |agents, cx| agents.set_other_apps(false, cx));
    assert_eq!(
        waiting.try_recv().unwrap_err(),
        mpsc::TryRecvError::Disconnected,
        "the waiting call is still waiting, or was answered"
    );
    assert_eq!(read(), None, "its connection stayed open");
    assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
    let logged = log(&agents, "claude-code", cx);
    assert_eq!(
        logged.first().map(|entry| entry.0.as_str()),
        Some("disconnected")
    );
    agents.read_with(cx, |agents, _| {
        assert!(agents.server.is_some());
        assert!(agents.is_connected(Client::ClaudeCode));
        assert!(!agents.live.contains("claude-code"));
    });

    // A hello the server read before, and a call, make nothing.
    let hello = Event::Hello {
        agent: "claude-code".into(),
        connection: 2,
        client: "claude-code".into(),
    };
    agents.update(cx, |agents, cx| agents.handle(hello, cx));
    assert!(agents.read_with(cx, |agents, _| !agents.live.contains("claude-code")));
    assert_eq!(log(&agents, "claude-code", cx), logged);
}
