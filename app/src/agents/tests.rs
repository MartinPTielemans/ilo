//! Settings › Agents and the tools, as the user and an agent use them, on a browser that makes up
//! its Spaces and tabs and records what the agents ask of it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, SystemTime};

use gpui::{App, AppContext, Entity, TestAppContext};
use serde_json::json;

use super::*;

/// The core's refusal of a tab the user holds.
const HELD: &str = "The user has this tab until they hand it back. Ask for it with ask_for_tab.";

/// The user's Space 1 with the tab 10, on Wikipedia, and Claude Code's Space 2 with the tab 20, on
/// example.com, once it has one.
#[derive(Default)]
struct Made {
    agent_spaces: RefCell<HashMap<String, u64>>,
    /// Each tab's Space, title and URL.
    tabs: RefCell<Vec<(u64, u64, &'static str, &'static str)>>,
    made: RefCell<Vec<(String, String)>>,
    reads: RefCell<Vec<(u64, u64)>>,
    disconnected: RefCell<Vec<String>>,
    /// What the agents asked the core to do in a page: the tab, the request, the action, the
    /// element and the text.
    pages: RefCell<Vec<(u64, u64, PageAction, i64, String)>>,
    navigated: RefCell<Vec<(u64, String)>>,
    /// The question each window shows, as the agents last told them.
    asked: RefCell<Option<Question>>,
    /// The tabs the user holds from agents, as the core keeps them.
    held: RefCell<HashSet<u64>>,
    /// Each action the agents asked the core whether they may take (bc_agent_may_act).
    checked: RefCell<Vec<String>>,
    /// The app, which the core asks about the key press_key presses from inside bc_agent_page.
    app: RefCell<Option<gpui::AsyncApp>>,
    /// The tasks' Spaces made, and the Spaces deleted.
    task_spaces: RefCell<Vec<u64>>,
    deleted: RefCell<Vec<u64>>,
}

impl Made {
    fn with_agent_space() -> Rc<Made> {
        let made = Made::default();
        made.agent_spaces
            .borrow_mut()
            .insert("claude-code".into(), 2);
        made.tabs.borrow_mut().extend([
            (
                10,
                1,
                "Arc (web browser)",
                "https://en.wikipedia.org/wiki/Arc_(web_browser)",
            ),
            (20, 2, "Example Domain", "https://www.example.com/"),
        ]);
        Rc::new(made)
    }
}

impl Browser for Made {
    fn agent_space(&self, agent: &str) -> u64 {
        self.agent_spaces.borrow().get(agent).copied().unwrap_or(0)
    }

    fn make_agent_space(&self, agent: &str, name: &str, _: &mut App) -> u64 {
        self.made.borrow_mut().push((agent.into(), name.into()));
        self.agent_spaces.borrow_mut().insert(agent.into(), 2);
        2
    }

    fn space_tabs(&self, space: u64, _: &App) -> Vec<TabInfo> {
        (self.tabs.borrow().iter())
            .filter(|tab| tab.1 == space)
            .map(|&(id, _, title, url)| TabInfo {
                id,
                title: title.into(),
                url: url.into(),
            })
            .collect()
    }

    fn agent_tabs(&self, agent: &str, cx: &App) -> Vec<TabInfo> {
        match self.agent_space(agent) {
            0 => Vec::new(),
            own => self.space_tabs(own, cx),
        }
    }

    fn tab(&self, tab: u64, _: &App) -> TabInfo {
        let tabs = self.tabs.borrow();
        let found = tabs.iter().find(|each| each.0 == tab);
        found.map_or(
            TabInfo {
                id: tab,
                ..Default::default()
            },
            |&(id, _, title, url)| TabInfo {
                id,
                title: title.into(),
                url: url.into(),
            },
        )
    }

    fn may_act(&self, agent: &str, tab: u64, action: &str, _: &mut App) -> Result<(), String> {
        self.checked.borrow_mut().push(action.into());
        if self.held.borrow().contains(&tab) {
            return Err(HELD.into());
        }
        let space = self.agent_space(agent);
        match self
            .tabs
            .borrow()
            .iter()
            .any(|each| each.0 == tab && each.1 == space && space != 0)
        {
            true => Ok(()),
            false => Err("The tab isn't in the agent's own Space.".into()),
        }
    }

    fn read_text(&self, tab: u64, request: u64) {
        self.reads.borrow_mut().push((tab, request));
    }

    fn disconnected(&self, agent: &str, _: &mut App) {
        self.disconnected.borrow_mut().push(agent.into());
    }

    /// The new tab is 30. As the core, none opens for an agent without a Space.
    fn open_tab(&self, agent: &str, url: &str, _: &mut App) -> u64 {
        let space = self.agent_space(agent);
        if space == 0 {
            return 0;
        }
        let url: &'static str = Box::leak(url.to_string().into_boxed_str());
        self.tabs.borrow_mut().push((30, space, "Opened", url));
        30
    }

    fn navigate(&self, _: &str, tab: u64, url: &str, _: &mut App) -> bool {
        self.navigated.borrow_mut().push((tab, url.into()));
        true
    }

    fn page(&self, _: &str, tab: u64, request: u64, action: PageAction, element: i64, text: &str) {
        (self.pages.borrow_mut()).push((tab, request, action, element, text.into()));
        // The page's key hooks ask the app about the key before the page has it, synchronously,
        // and the app's tab_key_before_page answers in an update of its own unless it's an agent's.
        let app = self.app.borrow();
        if let Some(app) = app
            .as_ref()
            .filter(|_| action == PageAction::Key && !pressing_key())
        {
            app.update(|_| ());
        }
    }

    /// A name with a dot is a site's, as the address bar takes it; other words are a search.
    fn url_for(&self, text: &str) -> Option<String> {
        Some(match (text.contains("://"), text.contains('.')) {
            (true, _) => text.into(),
            (false, true) => format!("https://{text}/"),
            (false, false) => format!("https://duckduckgo.com/?q={text}"),
        })
    }

    fn ask(&self, question: Option<Question>, _: &mut App) {
        *self.asked.borrow_mut() = question;
    }

    fn hand_off(&self, _: &str, tab: u64) -> bool {
        self.held.borrow_mut().insert(tab)
    }

    fn take_back(&self, tab: u64, _: &mut App) {
        self.held.borrow_mut().insert(tab);
    }

    fn hand_back(&self, tab: u64) -> bool {
        self.held.borrow_mut().remove(&tab)
    }

    fn close_tab(&self, tab: u64, _: &mut App) {
        self.tabs.borrow_mut().retain(|each| each.0 != tab);
    }

    /// A task's Space is 100 and up, in the order they're made, in the profile of `like`.
    fn make_task_space(&self, agent: &str, name: &str, like: u64, _: &mut App) -> u64 {
        let space = 100 + self.task_spaces.borrow().len() as u64;
        self.made
            .borrow_mut()
            .push((agent.into(), format!("{name} like {like}")));
        self.agent_spaces.borrow_mut().insert(agent.into(), space);
        self.task_spaces.borrow_mut().push(space);
        space
    }

    fn delete_space(&self, space: u64, _: &mut App) {
        self.deleted.borrow_mut().push(space);
        self.agent_spaces
            .borrow_mut()
            .retain(|_, each| *each != space);
        self.tabs.borrow_mut().retain(|tab| tab.1 != space);
    }

    fn task_spaces(&self) -> Vec<u64> {
        let deleted = self.deleted.borrow();
        (self.task_spaces.borrow().iter())
            .filter(|space| !deleted.contains(space))
            .copied()
            .collect()
    }

    /// As the core's: to the top of the Space's Today, which is first in `tabs`.
    fn move_tab(&self, tab: u64, space: u64, _: &mut App) {
        let mut tabs = self.tabs.borrow_mut();
        if let Some(at) = tabs.iter().position(|each| each.0 == tab) {
            let mut moved = tabs.remove(at);
            moved.1 = space;
            tabs.insert(0, moved);
        }
    }

    /// The user's Space is 1.
    fn user_space(&self, _: u64) -> u64 {
        1
    }
}

/// The clients' configs as Connect and Disconnect left them.
#[derive(Default)]
struct Configs {
    connected: Mutex<Vec<(Client, Entry)>>,
    disconnected: Mutex<Vec<(Client, String)>>,
    /// Why the next Connect fails, as when Claude Code isn't installed.
    failing: Mutex<Option<String>>,
    /// The clients on the computer, besides any other MCP client.
    found: Mutex<Vec<Client>>,
}

impl Clients for Configs {
    fn connect(&self, client: Client, entry: &Entry) -> Result<(), String> {
        if let Some(why) = self.failing.lock().unwrap().take() {
            return Err(why);
        }
        self.connected.lock().unwrap().push((client, entry.clone()));
        Ok(())
    }

    fn disconnect(&self, client: Client, name: &str) -> Result<(), String> {
        self.disconnected
            .lock()
            .unwrap()
            .push((client, name.into()));
        Ok(())
    }

    fn found(&self, client: Client) -> bool {
        client == Client::Other || self.found.lock().unwrap().contains(&client)
    }

    /// The tests never run a CLI of this computer's: none is found.
    fn look_for_task_cli(&self, _: crate::agents::runner::Cli) -> crate::agents::ready::Found {
        crate::agents::ready::Found {
            ready: crate::agents::ready::Ready::NotFound,
            ..Default::default()
        }
    }
}

fn agents(
    browser: &Rc<Made>,
    configs: &Arc<Configs>,
    path: Option<PathBuf>,
    cx: &mut TestAppContext,
) -> Entity<Agents> {
    agents_running(vec!["/app/mcp-bridge".into()], browser, configs, path, cx)
}

/// The Agents, whose clients reach the server by running `bridge` with the socket.
fn agents_running(
    bridge: Vec<String>,
    browser: &Rc<Made>,
    configs: &Arc<Configs>,
    path: Option<PathBuf>,
    cx: &mut TestAppContext,
) -> Entity<Agents> {
    let (browser, configs) = (browser.clone(), configs.clone());
    // A socket of the test's own, so the tests run beside a browser and each other.
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let number = NEXT.fetch_add(1, Ordering::SeqCst);
    let folder = std::env::temp_dir().join(format!("agents-ui-{}-{number}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let socket = address(&folder);
    cx.new(|_| Agents::new(browser, configs, path, socket, bridge))
}

/// Claude Code, connected.
fn connected(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, Arc<Configs>) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    (agents, browser, configs)
}

/// Calls `tool` as the agent with the id `agent` would, and gives the answer's receiver.
fn call(
    agents: &Entity<Agents>,
    agent: &str,
    tool: &str,
    arguments: Value,
    cx: &mut TestAppContext,
) -> mpsc::Receiver<ToolResult> {
    let (reply, answer) = mpsc::channel();
    let call = Call {
        id: 0,
        agent: agent.into(),
        tool: tool.into(),
        arguments,
        reply,
        at: std::time::Instant::now(),
    };
    agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
    answer
}

fn log(
    agents: &Entity<Agents>,
    agent: &str,
    cx: &mut TestAppContext,
) -> Vec<(String, String, String)> {
    agents.read_with(cx, |agents, _| {
        (agents.activity(agent))
            .map(|entry| {
                (
                    entry.action.to_string(),
                    entry.site.to_string(),
                    entry.refused.to_string(),
                )
            })
            .collect()
    })
}

/// The switch is off at first, and the server runs only while it's on: on, its socket or pipe is
/// in place for the user alone and takes a connection; off, it's gone.
#[gpui::test]
fn agents_are_off_until_the_user_turns_them_on(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    assert!(!agents.read_with(cx, |agents, _| agents.is_on()));
    assert!(agents.read_with(cx, |agents, _| agents.server.is_none()));
    let socket = agents.read_with(cx, |agents, _| agents.socket.clone());
    assert!(server::connect(&socket).is_err());

    agents.update(cx, |agents, cx| agents.set_on(true, cx));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&socket).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert!(server::connect(&socket).is_ok());

    agents.update(cx, |agents, cx| agents.set_on(false, cx));
    assert!(agents.read_with(cx, |agents, _| agents.server.is_none()));
    #[cfg(unix)]
    assert!(!socket.exists());
    assert!(
        server::connect(&socket).is_err(),
        "the server still takes connections once off"
    );
}

/// Connect writes the bridge, the socket and a token into the client's config, and the server
/// takes that token as the agent's. Connecting again replaces the token, and the old one stops
/// working.
#[gpui::test]
fn connect_gives_the_client_a_token_the_server_takes(cx: &mut TestAppContext) {
    let (agents, _, configs) = connected(cx);
    let (client, entry) = configs.connected.lock().unwrap()[0].clone();
    assert_eq!(client, Client::ClaudeCode);
    assert_eq!(entry.name, server_name());
    assert_eq!(entry.command, "/app/mcp-bridge");
    let socket = agents.read_with(cx, |agents, _| agents.socket.clone());
    assert_eq!(entry.args, [socket.to_str().unwrap()]);
    assert_eq!(entry.token.len(), 64);
    let tokens = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| agents.tokens.lock().unwrap().clone())
    };
    assert_eq!(
        tokens(cx).get(&entry.token).map(String::as_str),
        Some("claude-code")
    );
    assert!(agents.read_with(cx, |agents, _| agents.is_connected(Client::ClaudeCode)));
    assert!(!agents.read_with(cx, |agents, _| agents.is_connected(Client::Codex)));

    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    let again = configs.connected.lock().unwrap()[1].1.token.clone();
    assert_ne!(again, entry.token);
    assert_eq!(tokens(cx).len(), 1);
    assert!(tokens(cx).contains_key(&again));
}

/// Settings › Agents lists the clients on this computer and any other MCP client, finds one
/// installed since it last looked, and keeps one connected after its client has gone, for its
/// Disconnect.
#[gpui::test]
fn settings_lists_the_clients_found_here(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    let look = |cx: &mut TestAppContext| {
        agents.update(cx, |agents, cx| agents.look_for_clients(cx));
        agents.read_with(cx, |agents, _| agents.listed())
    };
    assert_eq!(look(cx), [Client::Other]);
    configs.found.lock().unwrap().push(Client::Codex);
    assert_eq!(look(cx), [Client::Codex, Client::Other]);

    agents.update(cx, |agents, cx| agents.connect(Client::Codex, cx));
    cx.run_until_parked();
    configs.found.lock().unwrap().clear();
    assert_eq!(look(cx), [Client::Codex, Client::Other]);
    agents.update(cx, |agents, cx| agents.disconnect(Client::Codex, cx));
    cx.run_until_parked();
    assert_eq!(look(cx), [Client::Other]);
}

/// A Connect that fails, as when the client isn't installed, connects nothing and says why on
/// the client's row.
#[gpui::test]
fn a_failed_connect_says_why(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    *configs.failing.lock().unwrap() = Some("Claude Code's claude command isn't installed.".into());
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    assert!(agents.read_with(cx, |agents, _| agents.is_connecting(Client::ClaudeCode)));
    cx.run_until_parked();
    assert!(!agents.read_with(cx, |agents, _| agents.is_connected(Client::ClaudeCode)));
    let failure = agents.read_with(cx, |agents, _| agents.failure(Client::ClaudeCode));
    assert_eq!(
        failure.as_deref(),
        Some("Claude Code's claude command isn't installed.")
    );
    assert!(agents.read_with(cx, |agents, _| agents.tokens.lock().unwrap().is_empty()));
}

/// Any other client gets the address and its token to paste, once it's connected.
#[gpui::test]
fn another_client_gets_its_setup_to_paste(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    assert_eq!(
        agents.read_with(cx, |agents, _| agents.setup(Client::Other)),
        None
    );
    agents.update(cx, |agents, cx| agents.connect(Client::Other, cx));
    cx.run_until_parked();
    let setup = agents
        .read_with(cx, |agents, _| agents.setup(Client::Other))
        .unwrap();
    let setup: Value = serde_json::from_str(&setup).unwrap();
    let server = &setup["mcpServers"][server_name()];
    let socket = agents.read_with(cx, |agents, _| agents.socket.clone());
    assert_eq!(server["type"], "stdio");
    assert_eq!(server["command"], "/app/mcp-bridge");
    assert_eq!(server["args"], json!([socket.to_str().unwrap()]));
    let token = configs.connected.lock().unwrap()[0].1.token.clone();
    assert_eq!(server["env"][bridge::TOKEN], token);
}

/// In a Flatpak, the bridge is a path in the sandbox, so the setup runs it there with flatpak,
/// which a client on the host can.
#[gpui::test]
fn a_flatpaks_setup_runs_the_bridge_in_its_sandbox(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let bridge = bridge::launch(
        std::path::Path::new("/app/ilo/mcp-bridge"),
        Some("app.ilo.browser"),
    );
    let agents = agents_running(bridge, &browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.connect(Client::Other, cx));
    cx.run_until_parked();
    let setup = agents
        .read_with(cx, |agents, _| agents.setup(Client::Other))
        .unwrap();
    let setup: Value = serde_json::from_str(&setup).unwrap();
    let server = &setup["mcpServers"][server_name()];
    let socket = agents.read_with(cx, |agents, _| agents.socket.clone());
    assert_eq!(server["command"], "flatpak");
    assert_eq!(
        server["args"],
        json!([
            "run",
            "--command=/app/ilo/mcp-bridge",
            "app.ilo.browser",
            socket.to_str().unwrap()
        ])
    );
    let command = agents.read_with(cx, |agents, _| agents.command());
    assert!(
        command
            .starts_with(r#""flatpak" "run" "--command=/app/ilo/mcp-bridge" "app.ilo.browser" ""#),
        "{command}"
    );
}

/// The "Any MCP client" row says which clients it didn't find, except in a Flatpak, where the
/// sandbox can't see the host's, so it says that and points to Copy Setup.
#[gpui::test]
fn the_other_clients_row_says_a_flatpak_cant_see_the_hosts_clients(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let plain = agents(&browser, &configs, None, cx);
    let detail = |agents: &Entity<Agents>, missing: &[&str], cx: &TestAppContext| {
        agents.read_with(cx, |agents, _| agents.other_client_detail(missing))
    };
    assert_eq!(detail(&plain, &[], cx), "Any MCP client");
    assert_eq!(
        detail(&plain, &["Codex"], cx),
        "Any MCP client. Codex isn't on this computer."
    );
    assert_eq!(
        detail(&plain, &["Claude Code", "Codex"], cx),
        "Any MCP client. Claude Code and Codex aren't on this computer."
    );

    let bridge = bridge::launch(
        std::path::Path::new("/app/ilo/mcp-bridge"),
        Some("app.ilo.browser"),
    );
    let flatpak = agents_running(bridge, &browser, &configs, None, cx);
    let said = detail(&flatpak, &["Claude Code", "Codex"], cx);
    assert!(
        said.contains("sandbox can't see Claude Code or Codex"),
        "{said}"
    );
    assert!(said.contains("Copy Setup"), "{said}");
    assert!(!said.contains("aren't on this computer"), "{said}");
}

/// list_tabs makes the agent's Space the first time, named as the agent, and lists only its tabs,
/// never the user's.
#[gpui::test]
fn list_tabs_lists_only_the_agents_own_tabs(cx: &mut TestAppContext) {
    let (browser, configs) = (Rc::new(Made::default()), Arc::new(Configs::default()));
    browser
        .tabs
        .borrow_mut()
        .push((10, 1, "Arc (web browser)", "https://en.wikipedia.org/"));
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();

    let answer = call(&agents, "claude-code", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert_eq!(
        *browser.made.borrow(),
        [("claude-code".to_string(), "Claude Code".to_string())]
    );
    assert!(!answer.error);
    assert!(answer.text.starts_with("Your Space has no tabs yet."));

    browser
        .tabs
        .borrow_mut()
        .push((20, 2, "Example Domain", "https://example.com/"));
    let answer = call(&agents, "claude-code", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert_eq!(answer.text, "  20  Example Domain  https://example.com/");
    assert_eq!(
        browser.made.borrow().len(),
        1,
        "the agent got a second Space"
    );
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("list_tabs".into(), "".into(), "".into())
    );
}

/// An agent whose first call is open_tab gets its Space then, and the tab opens there (#1468).
#[gpui::test]
fn open_tab_as_the_first_call_makes_the_agents_space(cx: &mut TestAppContext) {
    let (browser, configs) = (Rc::new(Made::default()), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();

    let answer = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "example.com" }),
        cx,
    );
    assert_eq!(
        *browser.made.borrow(),
        [("claude-code".to_string(), "Claude Code".to_string())]
    );
    let (tab, ..) = browser.pages.borrow()[0].clone();
    assert_eq!(tab, 30);
    let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(opened.text.starts_with("Opened tab 30."), "{}", opened.text);
    let listed = call(&agents, "claude-code", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert!(
        listed.text.contains("30  Opened  https://example.com/"),
        "{}",
        listed.text
    );
    assert_eq!(
        browser.made.borrow().len(),
        1,
        "the agent got a second Space"
    );
}

/// Two agents each list their own Space's tabs, as the core lists the calling agent's, and neither
/// the other's nor the user's.
#[gpui::test]
fn list_tabs_leaves_out_another_spaces_tabs(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    browser.agent_spaces.borrow_mut().insert("codex".into(), 3);
    (browser.tabs.borrow_mut()).push((30, 3, "Rust", "https://www.rust-lang.org/"));
    agents.update(cx, |agents, cx| agents.connect(Client::Codex, cx));
    cx.run_until_parked();

    let answer = call(&agents, "claude-code", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert_eq!(
        answer.text,
        "  20  Example Domain  https://www.example.com/"
    );
    let answer = call(&agents, "codex", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert_eq!(answer.text, "  30  Rust  https://www.rust-lang.org/");
}

/// read_page on a tab of the user's is refused, with the core's reason, which the activity log
/// keeps with the page's site, and the core reads nothing.
#[gpui::test]
fn reading_a_tab_of_the_users_is_refused_and_logged(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 10 }),
        cx,
    )
    .recv()
    .unwrap();
    assert_eq!(
        answer,
        ToolResult::error("The tab isn't in the agent's own Space.")
    );
    assert!(browser.reads.borrow().is_empty());
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        (
            "read_page".into(),
            "en.wikipedia.org".into(),
            "The tab isn't in the agent's own Space.".into()
        )
    );
}

/// read_page on the agent's own tab asks the core for its text, and answers with it once it
/// comes, saying which page it is. Without a tab, it reads the one the agent used last.
#[gpui::test]
fn read_page_reads_the_agents_own_tab(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": "20" }),
        cx,
    );
    let (tab, request) = browser.reads.borrow()[0];
    assert_eq!(tab, 20);
    assert!(
        answer.try_recv().is_err(),
        "it answered before the core read the page"
    );
    agents.update(cx, |agents, cx| {
        agents.text_read(
            request,
            Some("Example Domain\nThis domain is for examples.".into()),
            cx,
        )
    });
    let answer = answer.recv().unwrap();
    assert!(!answer.error);
    assert_eq!(
        answer.text,
        "Tab 20: Example Domain\nURL: https://www.example.com/\n\n\
         Example Domain\nThis domain is for examples."
    );
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("read_page".into(), "example.com".into(), "".into())
    );

    let answer = call(&agents, "claude-code", "read_page", json!({}), cx);
    let (tab, request) = browser.reads.borrow()[1];
    assert_eq!(tab, 20);
    agents.update(cx, |agents, cx| agents.text_read(request, None, cx));
    assert!(
        answer.recv().unwrap().error,
        "a tab without its page read as text"
    );
}

/// A page's text longer than an agent should take in is cut, and says so.
#[test]
fn a_long_page_is_cut() {
    let tab = TabInfo {
        id: 1,
        title: "Long".into(),
        url: "https://long.example/".into(),
    };
    let text = "é".repeat(tools::MAX_TEXT);
    let answer = tools::page_text(&tab, &text);
    assert!(answer.ends_with("[The page goes on: this is its first part.]"));
    assert!(answer.len() < tools::MAX_TEXT + 200);
}

/// An agent the user hasn't connected, or one they disconnected, gets nothing.
#[gpui::test]
fn an_agent_not_connected_gets_nothing(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "codex", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert_eq!(answer, ToolResult::error("This agent isn't connected."));
    let answer = call(&agents, "claude-code", "drag", json!({}), cx)
        .recv()
        .unwrap();
    assert_eq!(answer, ToolResult::error("There's no tool drag."));
    assert!(browser.made.borrow().is_empty());
}

/// Disconnect takes the server out of the client's config, forgets the agent's token, ends its
/// sessions in the core and answers no read it was waiting for. Its Space stays.
#[gpui::test]
fn disconnect_ends_the_agents_sessions_and_forgets_its_token(cx: &mut TestAppContext) {
    let (agents, browser, configs) = connected(cx);
    agents.update(cx, |agents, cx| {
        let hello = Event::Hello {
            agent: "claude-code".into(),
            client: "claude-code".into(),
        };
        agents.handle(hello, cx)
    });
    assert!(agents.read_with(cx, |agents, _| agents.is_live(Client::ClaudeCode)));
    let waiting = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 20 }),
        cx,
    );

    agents.update(cx, |agents, cx| agents.disconnect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
    assert_eq!(
        *configs.disconnected.lock().unwrap(),
        [(Client::ClaudeCode, server_name())]
    );
    assert!(agents.read_with(cx, |agents, _| agents.tokens.lock().unwrap().is_empty()));
    assert!(
        waiting.recv().is_err(),
        "a read went on for a disconnected agent"
    );
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "disconnected");
    let answer = call(&agents, "claude-code", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert!(answer.error);
}

/// Turning agents off ends every live agent's sessions and answers the calls waiting, and keeps
/// the agents connected for when it's on again.
#[gpui::test]
fn turning_agents_off_ends_their_sessions(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.set_on(true, cx));
    let waiting = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 20 }),
        cx,
    );
    agents.update(cx, |agents, cx| agents.set_on(false, cx));
    assert!(waiting.recv().is_err());
    assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
    assert!(agents.read_with(cx, |agents, _| agents.is_connected(Client::ClaudeCode)));
}

/// What's saved: the switch and each agent's token, in a file only the user can read, which a
/// later launch loads, starting the server if it was on.
#[gpui::test]
fn the_switch_and_the_agents_come_back(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("agents-test-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Agents");
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(path.clone()), cx);
    agents.update(cx, |agents, cx| agents.connect(Client::Codex, cx));
    cx.run_until_parked();
    agents.update(cx, |agents, cx| agents.set_on(true, cx));
    let saved = std::fs::read_to_string(&path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    agents.update(cx, |agents, cx| agents.set_on(false, cx));

    let later = self::agents(&browser, &configs, None, cx);
    later.update(cx, |agents, cx| agents.load(&saved, cx));
    assert!(later.read_with(cx, |agents, _| agents.is_on() && agents.server.is_some()));
    assert!(later.read_with(cx, |agents, _| agents.is_connected(Client::Codex)));
    let token = configs.connected.lock().unwrap()[0].1.token.clone();
    let tokens = later.read_with(cx, |agents, _| agents.tokens.lock().unwrap().clone());
    assert_eq!(tokens.get(&token).map(String::as_str), Some("codex"));
    later.update(cx, |agents, cx| agents.load("", cx));
    assert!(
        later.read_with(cx, |agents, _| !agents.is_on()),
        "a lost file left agents on"
    );
    std::fs::remove_dir_all(dir).ok();
}

/// The tree the core gives for tab 20's page: a heading, a field, a password field and a button
/// that orders.
fn shop_tree() -> String {
    json!([
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" },
          "childIds": ["2", "3", "4", "5"] },
        { "nodeId": "2", "role": { "value": "heading" }, "name": { "value": "Basket" } },
        { "nodeId": "3", "role": { "value": "textbox" }, "name": { "value": "Coupon" },
          "backendDOMNodeId": 31 },
        { "nodeId": "4", "role": { "value": "textbox" }, "name": { "value": "Password" },
          "backendDOMNodeId": 41, "password": true },
        { "nodeId": "5", "role": { "value": "button" }, "name": { "value": "Place order" },
          "backendDOMNodeId": 51 },
    ])
    .to_string()
}

/// The core's answer to the agents' last request of it, and the call's answer.
fn page_done(
    agents: &Entity<Agents>,
    browser: &Made,
    answer: mpsc::Receiver<ToolResult>,
    text: String,
    cx: &mut TestAppContext,
) -> ToolResult {
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| agents.page_done(request, text, false, cx));
    answer.recv().unwrap()
}

/// The key press_key presses reaches the app's key hooks from inside the Agents' update, where
/// another update of the app's would abort ilo (#1288), so it goes to the page alone.
#[gpui::test]
fn an_agents_key_goes_to_the_page_without_an_update_of_the_app(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    *browser.app.borrow_mut() = Some(cx.to_async());
    for key in ["Tab", "Enter"] {
        let answer = call(
            &agents,
            "claude-code",
            "press_key",
            json!({ "tab": 20, "key": key }),
            cx,
        );
        assert_eq!(browser.pages.borrow().last().unwrap().4, key);
        assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
    }
    // The user's keys, after it, are the app's again.
    assert!(!pressing_key());
}

/// An agent opens a page in its Space, gets its outline with refs, and acts by ref: each action
/// goes to the core with the element's node, and answers with what it did and the page after.
#[gpui::test]
fn an_agent_opens_a_page_and_acts_by_ref(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "example.com" }),
        cx,
    );
    let (tab, _, action, _, _) = browser.pages.borrow()[0].clone();
    assert_eq!((tab, action), (30, PageAction::Snapshot));
    let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(
        opened
            .text
            .starts_with("Opened tab 30.\n\nTab 30: Shop\nURL: https://example.com/"),
        "{}",
        opened.text
    );
    assert!(opened.text.contains("- textbox \"Coupon\" [e1]"));
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("open_tab".into(), "example.com".into(), "".into())
    );

    // Without a tab, the one it opened.
    let answer = call(
        &agents,
        "claude-code",
        "type",
        json!({ "ref": "e1", "text": "SAVE10" }),
        cx,
    );
    let (tab, _, action, element, text) = browser.pages.borrow().last().cloned().unwrap();
    assert_eq!(
        (tab, action, element, text.as_str()),
        (30, PageAction::Type, 31, "SAVE10")
    );
    let typed = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(typed.text.starts_with("Typed into “Coupon”.\n\nTab 30"));
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "type");

    let answer = call(
        &agents,
        "claude-code",
        "press_key",
        json!({ "key": "Tab" }),
        cx,
    );
    assert_eq!(browser.pages.borrow().last().unwrap().4, "Tab");
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    let answer = call(&agents, "claude-code", "screenshot", json!({}), cx);
    let picture = page_done(&agents, &browser, answer, "/9j/4AAQ".into(), cx);
    assert_eq!(
        picture.to_json()["content"][0],
        json!({ "type": "image", "data": "/9j/4AAQ", "mimeType": "image/jpeg" })
    );

    let answer = call(
        &agents,
        "claude-code",
        "navigate",
        json!({ "url": "back" }),
        cx,
    );
    assert_eq!(browser.pages.borrow().last().unwrap().2, PageAction::Back);
    assert!(
        page_done(&agents, &browser, answer, shop_tree(), cx)
            .text
            .starts_with("Went back.")
    );

    let answer = call(
        &agents,
        "claude-code",
        "navigate",
        json!({ "url": "iana.org" }),
        cx,
    );
    assert_eq!(
        browser.navigated.borrow()[0],
        (30, "https://iana.org/".into())
    );
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    let pages = browser.pages.borrow().len();
    let stale = call(&agents, "claude-code", "click", json!({ "ref": "e9" }), cx);
    assert!(
        stale
            .recv()
            .unwrap()
            .text
            .starts_with("There's no element e9 in tab 30.")
    );
    let key = call(
        &agents,
        "claude-code",
        "press_key",
        json!({ "key": "F5" }),
        cx,
    );
    assert!(key.recv().unwrap().error);
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "a bad call reached the core"
    );

    // The core's error is the call's.
    let answer = call(&agents, "claude-code", "click", json!({ "ref": "e1" }), cx);
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, "It's gone.".into(), true, cx)
    });
    assert_eq!(answer.recv().unwrap(), ToolResult::error("It's gone."));
}

/// Tab 20's page with a size picker, open or not, in the document `document`: a menu, a heading,
/// its text, the picker and a button.
fn size_picker(open: bool, document: i64) -> String {
    let picker = json!({ "nodeId": "5", "role": { "value": "combobox" }, "name": { "value": "Size" },
        "backendDOMNodeId": 50, "childIds": if open { vec!["7"] } else { vec![] },
        "properties": [{ "name": "expanded", "value": { "value": open } }] });
    let mut nodes = vec![
        json!({ "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shirt" },
          "backendDOMNodeId": document, "childIds": ["2", "3", "4", "5", "6"] }),
        json!({ "nodeId": "2", "role": { "value": "navigation" }, "name": { "value": "Menu" },
          "childIds": ["21", "22", "23"] }),
        json!({ "nodeId": "3", "role": { "value": "heading" }, "name": { "value": "Linen shirt" } }),
        json!({ "nodeId": "4", "role": { "value": "StaticText" },
          "name": { "value": "Light linen, cut loose, for long summer days." } }),
        picker,
        json!({ "nodeId": "6", "role": { "value": "button" }, "name": { "value": "Add to basket" },
          "backendDOMNodeId": 60 }),
        json!({ "nodeId": "7", "role": { "value": "listbox" }, "name": { "value": "Size" },
          "backendDOMNodeId": 70, "childIds": ["71", "72"] }),
        json!({ "nodeId": "71", "role": { "value": "option" }, "name": { "value": "S" },
          "backendDOMNodeId": 71 }),
        json!({ "nodeId": "72", "role": { "value": "option" }, "name": { "value": "M" },
          "backendDOMNodeId": 72 }),
    ];
    for (id, name) in [("21", "Home"), ("22", "Shirts"), ("23", "Basket")] {
        nodes.push(
            json!({ "nodeId": id, "role": { "value": "link" }, "name": { "value": name },
          "backendDOMNodeId": id.parse::<i64>().unwrap() * 10 }),
        );
    }
    Value::Array(nodes).to_string()
}

/// An action answers with what changed in the page's outline since the last outline the agent
/// had of the tab: a click that opens a dropdown, with its options. A navigation, or a click that
/// opens a new page, answers with the page whole, as does outline full; with outline none, an
/// action answers with what it did, and the next with what changed since the outline it had.
#[gpui::test]
fn an_action_answers_with_what_changed_in_the_page(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let act = |arguments: Value, tree: String, cx: &mut TestAppContext| {
        let tool = arguments["tool"].as_str().unwrap().to_string();
        let answer = call(&agents, "claude-code", &tool, arguments, cx);
        page_done(&agents, &browser, answer, tree, cx).text
    };
    let snapshot = json!({ "tool": "snapshot", "tab": 20 });
    let whole = act(snapshot.clone(), size_picker(false, 1), cx);
    assert!(whole.contains("- link \"Shirts\" [e2]\n"), "{whole}");
    let picker = json!({ "tool": "click", "tab": 20, "ref": "e4" });
    let head = "Tab 20: Shirt\nURL: https://www.example.com/\n\n";
    let since = "What changed since the last outline you had of this tab:\n\n";
    assert_eq!(
        act(picker.clone(), size_picker(true, 1), cx),
        format!(
            "Clicked combobox “Size”.\n\n{head}{since}New:\n\
             - listbox \"Size\" [e6]\n  - option \"S\" [e7]\n  - option \"M\" [e8]\n\n\
             Changed:\n- combobox \"Size\" [e4] (expanded)"
        )
    );

    // With none, what it did and where; the next answer's changes are since the outline it had.
    let option = json!({ "tool": "click", "tab": 20, "ref": "e7", "outline": "none" });
    let chose = act(option, size_picker(false, 1), cx);
    assert_eq!(
        chose,
        "Clicked option “S”.\n\nTab 20: Shirt\nURL: https://www.example.com/"
    );
    let scrolled = act(
        json!({ "tool": "scroll", "tab": 20 }),
        size_picker(false, 1),
        cx,
    );
    assert!(
        scrolled.contains("Changed:\n- combobox \"Size\" [e4]\n\nGone:\n- listbox"),
        "{scrolled}"
    );
    let again = act(
        json!({ "tool": "scroll", "tab": 20 }),
        size_picker(false, 1),
        cx,
    );
    assert_eq!(
        again,
        format!("Scrolled down.\n\n{head}Nothing in the outline changed.")
    );

    // A new page, and outline full, answer whole.
    let link = json!({ "tool": "click", "tab": 20, "ref": "e2" });
    assert_eq!(
        act(link, size_picker(false, 2), cx),
        format!("Clicked link “Shirts”.\n\n{whole}")
    );
    let back = json!({ "tool": "navigate", "tab": 20, "url": "back" });
    assert_eq!(
        act(back, size_picker(false, 2), cx),
        format!("Went back.\n\n{whole}")
    );
    let full = json!({ "tool": "press_key", "tab": 20, "key": "Tab", "outline": "full" });
    assert_eq!(
        act(full, size_picker(false, 2), cx),
        format!("Pressed Tab.\n\n{whole}")
    );
    let wrong = json!({ "tab": 20, "ref": "e4", "outline": "diff" });
    let refused = call(&agents, "claude-code", "click", wrong, cx)
        .recv()
        .unwrap();
    assert_eq!(
        refused,
        ToolResult::error("outline is changes, full or none.")
    );

    // A snapshot of a ref outlines its element alone, and the core outlines the page as ever.
    let part = act(
        json!({ "tool": "snapshot", "tab": 20, "ref": "e4" }),
        size_picker(true, 2),
        cx,
    );
    assert_eq!(
        part,
        format!(
            "{head}- combobox \"Size\" [e4] (expanded)\n  - listbox \"Size\" [e6]\n    \
             - option \"S\" [e7]\n    - option \"M\" [e8]"
        )
    );
    let (_, _, action, element, _) = browser.pages.borrow().last().cloned().unwrap();
    assert_eq!((action, element), (PageAction::Snapshot, 0));
    // It isn't the outline the agent has, which the next changes are since.
    let closed = act(
        json!({ "tool": "scroll", "tab": 20 }),
        size_picker(false, 2),
        cx,
    );
    assert!(
        closed.ends_with("Nothing in the outline changed."),
        "{closed}"
    );

    // A run answers with what each step did, then its last step's changes.
    let steps = json!({ "tool": "run_steps", "tab": 20, "steps": [
        { "action": "scroll" },
        { "action": "click", "ref": "e4" },
    ] });
    let answer = call(&agents, "claude-code", "run_steps", steps, cx);
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, size_picker(false, 2), false, cx)
    });
    let ran = page_done(&agents, &browser, answer, size_picker(true, 2), cx);
    assert!(
        ran.text
            .starts_with("Ran 2 steps.\n1. Scrolled down.\n2. Clicked combobox “Size”.\n\n")
    );
    assert!(
        ran.text.contains(&format!("{head}{since}New:\n- listbox")),
        "{}",
        ran.text
    );
}

/// An agent can't act on the user's tabs, type into a password field or open a page that isn't
/// on the web; each refusal is logged.
#[gpui::test]
fn what_an_agent_may_not_do_is_refused_and_logged(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 10, "ref": "e1" }),
        cx,
    );
    assert!(answer.recv().unwrap().error);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 10 }), cx);
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("The tab isn't in the agent's own Space.")
    );

    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    let outline = page_done(&agents, &browser, answer, shop_tree(), cx).text;
    assert!(outline.contains("- textbox \"Password\" [e2] (password: off limits)"));
    let pages = browser.pages.borrow().len();
    let typing = json!({ "ref": "e2", "text": "hunter2" });
    let answer = call(&agents, "claude-code", "type", typing, cx);
    assert!(
        answer
            .recv()
            .unwrap()
            .text
            .starts_with("Password fields are off limits")
    );
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "the password reached the core"
    );
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "type");
    assert!(
        log(&agents, "claude-code", cx)[0]
            .2
            .starts_with("Password fields are off limits")
    );

    let file = json!({ "url": "file:///etc/passwd" });
    let answer = call(&agents, "claude-code", "open_tab", file, cx);
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("Agents may open only http and https pages.")
    );
    let settings = json!({ "url": "chrome://settings" });
    let answer = call(&agents, "claude-code", "navigate", settings, cx);
    assert!(answer.recv().unwrap().error);
    assert!(browser.navigated.borrow().is_empty());
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "navigate");

    // The account is the user's alone: its secret reads their passwords.
    for tool in ["sign_in", "recovery_words", "sync"] {
        let answer = call(&agents, "claude-code", tool, json!({}), cx);
        assert_eq!(
            answer.recv().unwrap(),
            ToolResult::error(format!("There's no tool {tool}."))
        );
    }
}

/// Chrome's password manager page shows and copies the user's passwords once the device's check
/// passes, so an agent can neither open it nor take its own tab there, by any of its addresses.
#[gpui::test]
fn an_agent_never_reaches_chromes_password_manager(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let tabs = browser.tabs.borrow().len();
    let refused = ToolResult::error("Agents may open only http and https pages.");
    let addresses = [
        "chrome://password-manager/passwords",
        "chrome://settings/passwords",
        "CHROME://PASSWORD-MANAGER",
    ];
    for url in addresses {
        let answer = call(
            &agents,
            "claude-code",
            "open_tab",
            json!({ "url": url }),
            cx,
        );
        assert_eq!(answer.recv().unwrap(), refused, "{url}");
        let going = json!({ "tab": 20, "url": url });
        let answer = call(&agents, "claude-code", "navigate", going, cx);
        assert_eq!(answer.recv().unwrap(), refused, "{url}");
    }
    assert_eq!(
        browser.tabs.borrow().len(),
        tabs,
        "an agent opened Chrome's password manager"
    );
    assert!(
        browser.navigated.borrow().is_empty(),
        "an agent went to Chrome's password manager"
    );
    assert!(browser.pages.borrow().is_empty());
}

/// A tab the page opens as the agent clicks, which the core puts in the agent's Space, behind, is
/// named in the click's answer, so the agent can work in it (#1470). One another tab's page opens
/// isn't.
#[gpui::test]
fn a_tab_the_agents_click_opens_is_named(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);

    let answer = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    agents.update(cx, |agents, _| {
        agents.tab_opened(26, 20);
        agents.tab_opened(27, 10);
    });
    let clicked = page_done(&agents, &browser, answer, shop_tree(), cx);
    let said = clicked.text.split("\n\n").next().unwrap();
    assert_eq!(
        said,
        "Clicked textbox “Coupon”. It opened tab 26 in your Space."
    );

    let answer = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    let clicked = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert_eq!(
        clicked.text.split("\n\n").next().unwrap(),
        "Clicked textbox “Coupon”."
    );
}

/// An order asks the user first, in the card: Allow Once lets it go this time and asks again
/// next time, Always Allow lets it go on the site from then on, and is saved.
#[gpui::test]
fn an_order_asks_the_user_first(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("agents-test-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Agents");
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(path.clone()), cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let pages = browser.pages.borrow().len();

    let order = |cx: &mut TestAppContext| {
        call(
            &agents,
            "claude-code",
            "click",
            json!({ "tab": 20, "ref": "e3" }),
            cx,
        )
    };
    let answer = order(cx);
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("the order didn't ask");
    assert_eq!(question.agent, "Claude Code");
    assert_eq!(question.what, "click “Place order” on example.com");
    assert_eq!(question.why, "It looks like a payment or an order.");
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "it clicked before the user answered"
    );
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert_eq!(*browser.asked.borrow(), None);
    assert_eq!(browser.pages.borrow().last().unwrap().3, 51);
    let clicked = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(clicked.text.starts_with("Clicked button “Place order”."));

    let answer = order(cx);
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("once let it go again");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Always, cx)
    });
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
    let answer = order(cx);
    assert_eq!(*browser.asked.borrow(), None, "always asked again");
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    let saved = std::fs::read_to_string(&path).unwrap();
    let later = self::agents(&browser, &configs, None, cx);
    later.update(cx, |agents, cx| agents.load(&saved, cx));
    let grants = later.read_with(cx, |agents, _| agents.grants.clone());
    assert_eq!(grants["claude-code"]["example.com"], true);
    std::fs::remove_dir_all(dir).ok();
}

/// What the user answered for good is listed, by site, and once revoked, one site or all, what
/// asks first there asks again. Each revoke is logged.
#[gpui::test]
fn a_permission_given_for_good_is_listed_and_revoked(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let order = |cx: &mut TestAppContext| {
        call(
            &agents,
            "claude-code",
            "click",
            json!({ "tab": 20, "ref": "e3" }),
            cx,
        )
    };
    let answer = order(cx);
    let question = browser.asked.borrow().clone().unwrap();
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Always, cx)
    });
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let answer = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "chase.com" }),
        cx,
    );
    let question = browser.asked.borrow().clone().unwrap();
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Never, cx)
    });
    assert!(answer.recv().unwrap().error);
    let grants =
        |cx: &mut TestAppContext| agents.read_with(cx, |agents, _| agents.grants("claude-code"));
    assert_eq!(
        grants(cx),
        [("chase.com".into(), false), ("example.com".into(), true)]
    );

    agents.update(cx, |agents, cx| {
        agents.revoke("claude-code", Some("example.com"), cx)
    });
    assert_eq!(grants(cx), [("chase.com".into(), false)]);
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("revoked".into(), "example.com".into(), "".into())
    );
    let answer = order(cx);
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("a revoked Always didn't ask again");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    agents.update(cx, |agents, cx| agents.revoke("claude-code", None, cx));
    assert!(grants(cx).is_empty());
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("revoked all".into(), "".into(), "".into())
    );
    let entries = log(&agents, "claude-code", cx).len();
    agents.update(cx, |agents, cx| agents.revoke("claude-code", None, cx));
    assert_eq!(
        log(&agents, "claude-code", cx).len(),
        entries,
        "revoking nothing was logged"
    );
    call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "chase.com" }),
        cx,
    );
    assert!(
        browser.asked.borrow().is_some(),
        "a revoked Never didn't ask again"
    );
}

/// A bank's site asks before it opens. No, or the ×, refuses it, and Never refuses it from then
/// on without asking; each is logged.
#[gpui::test]
fn no_to_an_agent_is_kept(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let open = |cx: &mut TestAppContext| {
        call(
            &agents,
            "claude-code",
            "open_tab",
            json!({ "url": "chase.com" }),
            cx,
        )
    };
    let answer = open(cx);
    let question = browser.asked.borrow().clone().unwrap();
    assert_eq!(question.what, "open chase.com");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Dismiss, cx)
    });
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("The user said no: you can't open chase.com. Don't try again.")
    );
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("open_tab".into(), "chase.com".into(), "You said no.".into())
    );

    let answer = open(cx);
    let question = browser.asked.borrow().clone().unwrap();
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Never, cx)
    });
    assert!(answer.recv().unwrap().error);
    let answer = open(cx);
    assert_eq!(*browser.asked.borrow(), None, "never asked again");
    assert!(
        answer
            .recv()
            .unwrap()
            .text
            .starts_with("The user said never on chase.com")
    );
    assert!(browser.pages.borrow().is_empty());
    assert!(browser.tabs.borrow().iter().all(|tab| tab.0 != 30));
}

/// A question goes with its agent's session, and the card with it.
#[gpui::test]
fn a_question_goes_with_its_agent(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "chase.com" }),
        cx,
    );
    assert!(browser.asked.borrow().is_some());
    agents.update(cx, |agents, cx| {
        agents.handle(
            Event::Hello {
                agent: "claude-code".into(),
                client: "check".into(),
            },
            cx,
        );
        agents.handle(
            Event::Bye {
                agent: "claude-code".into(),
            },
            cx,
        );
    });
    assert_eq!(*browser.asked.borrow(), None);
    assert!(answer.recv().is_err());
}

/// The driver's calls, checked by the core outside the tools, come to the log too, and a session
/// the user took back has its entry.
#[gpui::test]
fn the_cores_reports_come_to_the_log(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    agents.update(cx, |agents, cx| {
        agents.checked(
            "claude-code",
            10,
            "act",
            "The tab isn't in the agent's own Space.",
            cx,
        );
        agents.session_ended("claude-code", 20, "taken back", cx);
        agents.session_ended("claude-code", 20, "disconnected", cx);
    });
    let log = log(&agents, "claude-code", cx);
    assert_eq!(
        log[0],
        ("taken back".into(), "example.com".into(), "".into())
    );
    assert_eq!(log[1].1, "en.wikipedia.org");
    assert_eq!(tools::describe(&log[0].0), "You took the tab over");
    assert!(log.iter().all(|entry| entry.0 != "disconnected"));
}

#[test]
fn a_sites_host_is_its_name() {
    assert_eq!(site("https://www.example.com/a?b"), "example.com");
    assert_eq!(site("http://user@localhost:8000/"), "localhost");
    assert_eq!(site("https://[::1]:8080/"), "[::1]");
    assert_eq!(site("about:blank"), "");
}

#[test]
fn the_log_says_how_long_ago() {
    let now = SystemTime::now();
    assert_eq!(ago(now, now), "Just now");
    assert_eq!(ago(now - Duration::from_secs(150), now), "2 min ago");
    assert_eq!(ago(now - Duration::from_secs(7200), now), "2 h ago");
    assert_eq!(ago(now + Duration::from_secs(5), now), "Just now");
}

/// The core answers the agents' last request of it with the shop's tree.
fn shop_done(agents: &Entity<Agents>, browser: &Made, cx: &mut TestAppContext) {
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, shop_tree(), false, cx)
    });
}

/// hand_off holds the tab for the user and asks them in the card, and its call waits: the agent's
/// calls on the tab are refused meanwhile. Hand Back gives the tab back and answers it.
#[gpui::test]
fn a_handoff_waits_for_the_user_to_hand_the_tab_back(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "hand_off", json!({ "tab": 20 }), cx);
    assert!(answer.recv().unwrap().text.starts_with("reason is"));
    let reason = json!({ "tab": 20, "reason": "Sign in to example.com" });
    let handed = call(&agents, "claude-code", "hand_off", reason, cx);
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("the handoff didn't ask");
    assert_eq!(question.handoff, Some(20));
    assert_eq!(question.why, "Sign in to example.com");
    assert!(browser.held.borrow().contains(&20));
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, [(20, "Sign in to example.com".into())]);
    let refused = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert_eq!(refused.recv().unwrap(), ToolResult::error(HELD));
    assert!(
        handed.try_recv().is_err(),
        "the handoff answered before the tab came back"
    );

    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    let back = handed.recv().unwrap();
    assert!(back.text.starts_with("The user handed tab 20 back."));
    assert_eq!(*browser.asked.borrow(), None);
    assert!(browser.held.borrow().is_empty());
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "handed back");
}

/// Take Over holds the agent's tab until the user hands it back, with no card; Stop refuses what
/// the agent waits on, takes its question away and ends its sessions.
#[gpui::test]
fn the_user_takes_over_and_stops_an_agent(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    assert!(browser.held.borrow().contains(&20));
    assert_eq!(*browser.asked.borrow(), None);
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, [(20, "".into())]);
    let refused = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(refused.recv().unwrap().error);
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    let order = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 20, "ref": "e3" }),
        cx,
    );
    assert!(browser.asked.borrow().is_some());
    agents.update(cx, |agents, cx| agents.stop_agent("claude-code", cx));
    let stopped = order.recv().unwrap();
    assert!(stopped.error && stopped.text.starts_with("The user stopped you."));
    assert_eq!(*browser.asked.borrow(), None);
    assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "stopped by the user");
}

/// A tab the user took over stays theirs: the agent's calls on it are refused until it asks with
/// ask_for_tab and the user hands it back. Keep refuses, and the tab stays the user's.
#[gpui::test]
fn a_tab_the_user_took_back_stays_theirs_until_they_hand_it_back(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    let refused = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    assert!(refused.recv().unwrap().error);
    let other = json!({ "tab": 21, "reason": "To finish the order" });
    let asked = call(&agents, "claude-code", "ask_for_tab", other, cx);
    assert!(
        asked
            .recv()
            .unwrap()
            .text
            .starts_with("The user doesn't have that tab")
    );
    let asked = call(
        &agents,
        "claude-code",
        "ask_for_tab",
        json!({ "tab": 20 }),
        cx,
    );
    assert!(asked.recv().unwrap().text.starts_with("reason is"));

    let reason = json!({ "tab": 20, "reason": "To finish the order" });
    let kept = call(&agents, "claude-code", "ask_for_tab", reason.clone(), cx);
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("asking for the tab didn't ask");
    assert_eq!((question.handoff, question.back), (Some(20), true));
    assert_eq!(question.why, "To finish the order");
    let again = call(&agents, "claude-code", "ask_for_tab", reason.clone(), cx);
    assert!(
        again
            .recv()
            .unwrap()
            .text
            .starts_with("You're waiting on the user")
    );
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Never, cx)
    });
    let kept = kept.recv().unwrap();
    assert!(kept.error && kept.text.starts_with("The user keeps tab 20."));
    assert_eq!(*browser.asked.borrow(), None);
    assert!(
        browser.held.borrow().contains(&20),
        "Keep gave the tab back"
    );
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "kept");

    let back = call(&agents, "claude-code", "ask_for_tab", reason, cx);
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("asking again didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert!(
        back.recv()
            .unwrap()
            .text
            .starts_with("The user handed tab 20 back.")
    );
    assert!(browser.held.borrow().is_empty());
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
}

/// A page asking for the user's passkey hands its tab to them, as a handoff with no call waiting:
/// no card over Chrome's dialog, the bar says why, the log has it and Hand Back ends it.
#[gpui::test]
fn a_pages_passkey_hands_its_tab_to_the_user(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| {
        agents.session_ended("claude-code", 20, "passkey", cx)
    });
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, vec![(20, SharedString::from(PASSKEY))]);
    assert_eq!(*browser.asked.borrow(), None);
    let entry = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(entry.0, "passkey");
    assert_eq!(
        tools::describe(&entry.0),
        "Handed you a tab for your passkey"
    );

    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert!(status.iter().all(|status| status.held.is_empty()));
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "handed back");
}

/// close_tab closes the agent's own tab, with its entry in the log.
/// The user's tab, and one the user holds, are refused.
#[gpui::test]
fn an_agent_closes_only_its_own_tabs(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let refused = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 10 }),
        cx,
    );
    assert_eq!(
        refused.recv().unwrap().text,
        "The tab isn't in the agent's own Space."
    );
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    let held = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 20 }),
        cx,
    );
    assert_eq!(held.recv().unwrap().text, HELD);
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));

    let closed = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 20 }),
        cx,
    );
    let closed = closed.recv().unwrap();
    assert!(!closed.error);
    assert_eq!(closed.text, "Closed tab 20.");
    assert!(browser.tabs.borrow().iter().all(|tab| tab.0 != 20));
    let entry = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(entry, ("close_tab".into(), "example.com".into(), "".into()));
    assert_eq!(tools::describe(&entry.0), "Closed a tab");
}

/// Take Back All Tabs, in the agent's Space's menu, takes over each tab there the user doesn't
/// hold, which the agent is refused until the user hands it back.
#[gpui::test]
fn the_user_takes_back_all_of_an_agents_tabs(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    browser
        .tabs
        .borrow_mut()
        .push((21, 2, "Shop", "https://shop.example/"));
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 21, cx));
    let to_take = agents.read_with(cx, |agents, cx| agents.tabs_to_take("claude-code", cx));
    assert_eq!(to_take, [20]);

    agents.update(cx, |agents, cx| agents.take_back_all("claude-code", cx));
    assert_eq!(*browser.held.borrow(), HashSet::from([20, 21]));
    assert!(
        agents
            .read_with(cx, |agents, cx| agents.tabs_to_take("claude-code", cx))
            .is_empty()
    );
    let refused = call(&agents, "claude-code", "scroll", json!({ "tab": 20 }), cx);
    assert_eq!(refused.recv().unwrap().text, HELD);
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    let to_take = agents.read_with(cx, |agents, cx| agents.tabs_to_take("claude-code", cx));
    assert_eq!(to_take, [20]);
}

/// run_steps takes its steps one at a time, each checked and logged as alone, under the run's one
/// entry, and answers with what each did and the last one's page. It stops at a step that asks
/// the user first, leaving no question.
#[gpui::test]
fn run_steps_runs_its_steps_in_order(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let wrong = json!({ "tab": 20, "steps": [{ "action": "screenshot" }] });
    assert!(
        call(&agents, "claude-code", "run_steps", wrong, cx)
            .recv()
            .unwrap()
            .error
    );

    let steps = json!({ "tab": 20, "steps": [
        { "action": "type", "ref": "e1", "text": "SAVE10" },
        { "action": "scroll" },
    ] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    assert_eq!(browser.pages.borrow().last().unwrap().2, PageAction::Type);
    shop_done(&agents, &browser, cx);
    assert!(
        run.try_recv().is_err(),
        "the run answered after its first step"
    );
    assert_eq!(
        browser.pages.borrow().last().unwrap().2,
        PageAction::ScrollDown
    );
    shop_done(&agents, &browser, cx);
    let ran = run.recv().unwrap();
    assert!(!ran.error);
    assert!(
        ran.text
            .starts_with("Ran 2 steps.\n1. Typed into “Coupon”.\n2. Scrolled down.\n\n")
    );
    let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    assert_eq!(entry.action, "run_steps");
    let steps: Vec<&str> = entry
        .steps
        .iter()
        .map(|step| step.action.as_ref())
        .collect();
    assert_eq!(steps, ["type", "scroll"]);

    let order = json!({ "action": "click", "ref": "e3" });
    let steps = json!({ "tab": 20, "steps": [{ "action": "scroll" }, order] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    shop_done(&agents, &browser, cx);
    let stopped = run.recv().unwrap();
    assert!(stopped.error);
    assert!(
        stopped
            .text
            .starts_with("Step 2 would click “Place order” on example.com")
    );
    assert!(stopped.text.ends_with("1. Scrolled down."));
    assert_eq!(*browser.asked.borrow(), None, "the run left its question");
}

/// A run's wait step looks at the page until its text shows, checked as a snapshot and logged once
/// as a wait, and fails once its time is up without it. Without text it waits its time, then looks.
#[gpui::test]
fn a_wait_step_waits_for_its_text_in_the_page(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let look = |tree: String, cx: &mut TestAppContext| {
        let request = browser.pages.borrow().last().unwrap().1;
        agents.update(cx, |agents, cx| agents.page_done(request, tree, false, cx));
    };
    let looks = || browser.pages.borrow().len();
    let thanks = json!([
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "heading" }, "name": { "value": "Thanks for your order" } },
    ])
    .to_string();
    for wrong in [
        json!({}),
        json!({ "seconds": 31 }),
        json!({ "text": "Thanks", "seconds": 0 }),
    ] {
        let mut step = json!({ "action": "wait" });
        step.as_object_mut()
            .unwrap()
            .extend(wrong.as_object().unwrap().clone());
        let steps = json!({ "tab": 20, "steps": [step] });
        let refused = call(&agents, "claude-code", "run_steps", steps, cx)
            .recv()
            .unwrap();
        assert!(
            refused.text.starts_with("Step 1 failed: wait needs text"),
            "{}",
            refused.text
        );
    }

    let wait = json!({ "action": "wait", "text": "thanks", "seconds": 2 });
    let steps = json!({ "tab": 20, "steps": [{ "action": "scroll" }, wait] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    shop_done(&agents, &browser, cx);
    let before = looks();
    look(shop_tree(), cx);
    cx.run_until_parked();
    assert_eq!(looks(), before, "the wait looked again before its time");
    assert!(run.try_recv().is_err());
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(looks(), before + 1);
    look(thanks.clone(), cx);
    let ran = run.recv().unwrap();
    assert!(
        ran.text
            .starts_with("Ran 2 steps.\n1. Scrolled down.\n2. “thanks” showed.\n\nTab 20"),
        "{}",
        ran.text
    );
    let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    let steps: Vec<&str> = entry
        .steps
        .iter()
        .map(|step| step.action.as_ref())
        .collect();
    assert_eq!(steps, ["scroll", "wait"]);
    assert_eq!(browser.checked.borrow().last().unwrap(), "snapshot");

    let steps =
        json!({ "tab": 20, "steps": [{ "action": "wait", "text": "Thanks", "seconds": 1 }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    for _ in 0..2 {
        look(shop_tree(), cx);
        cx.executor().advance_clock(Duration::from_millis(500));
    }
    look(shop_tree(), cx);
    let failed = run.recv().unwrap();
    assert!(failed.error);
    assert!(
        failed
            .text
            .starts_with("Step 1 failed: “Thanks” didn't show in 1 s."),
        "{}",
        failed.text
    );

    let steps = json!({ "tab": 20, "steps": [{ "action": "wait", "seconds": 2 }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    let before = looks();
    cx.executor().advance_clock(Duration::from_millis(1900));
    assert_eq!(
        looks(),
        before,
        "a wait without text looked before its time"
    );
    cx.executor().advance_clock(Duration::from_millis(100));
    look(shop_tree(), cx);
    assert!(
        run.recv()
            .unwrap()
            .text
            .starts_with("Ran 1 steps.\n1. Waited 2 s.\n\nTab 20")
    );
}

/// The actions the core lets an agent take in its tabs, agent_session.cc's RefusedAction: each a
/// thing the user does in a page, or handing the tab to them.
const CORE_ALLOWS: [&str; 13] = [
    "back",
    "click",
    "close_tab",
    "forward",
    "hand_off",
    "navigate",
    "press_key",
    "read_page",
    "screenshot",
    "scroll",
    "select_option",
    "snapshot",
    "type",
];

/// Agents never read or fill a saved password, address, card or passkey (principle 3). The server
/// offers exactly these tools, and every one asks the core only for an action its allowlist has,
/// so a tool or action that could reach them fails here until it's added on purpose, with its rule.
#[gpui::test]
fn agents_get_only_the_allowlisted_tools(cx: &mut TestAppContext) {
    let listed = tools::list("claude-code");
    let names: Vec<&str> = (listed.as_array().unwrap().iter())
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "list_tabs",
            "open_tab",
            "close_tab",
            "navigate",
            "snapshot",
            "read_page",
            "click",
            "type",
            "press_key",
            "scroll",
            "screenshot",
            "run_steps",
            "hand_off",
            "ask_for_tab",
        ]
    );

    let (agents, browser, _) = connected(cx);
    let tab = |more: Value| {
        let mut arguments = json!({ "tab": 20 });
        arguments
            .as_object_mut()
            .unwrap()
            .extend(more.as_object().unwrap().clone());
        arguments
    };
    let calls = [
        ("list_tabs", json!({})),
        ("open_tab", json!({ "url": "example.com" })),
        ("snapshot", tab(json!({}))),
        ("read_page", tab(json!({}))),
        ("click", tab(json!({ "ref": "e1" }))),
        ("type", tab(json!({ "ref": "e1", "text": "SAVE10" }))),
        ("press_key", tab(json!({ "key": "Tab" }))),
        ("scroll", tab(json!({}))),
        ("screenshot", tab(json!({}))),
        ("navigate", tab(json!({ "url": "back" }))),
        ("navigate", tab(json!({ "url": "forward" }))),
        ("navigate", tab(json!({ "url": "iana.org" }))),
        (
            "run_steps",
            tab(json!({ "steps": [{ "action": "scroll" }] })),
        ),
        ("hand_off", tab(json!({ "reason": "Sign in" }))),
        ("ask_for_tab", tab(json!({ "reason": "To go on" }))),
        ("close_tab", json!({ "tab": 30 })),
    ];
    for (tool, arguments) in calls {
        let pages = browser.pages.borrow().len();
        let answer = call(&agents, "claude-code", tool, arguments, cx);
        if browser.pages.borrow().len() > pages {
            shop_done(&agents, &browser, cx);
        }
        if let Some(read) = browser.reads.borrow_mut().pop() {
            agents.update(cx, |agents, cx| {
                agents.text_read(read.1, Some("Shop".into()), cx)
            });
        }
        cx.run_until_parked();
        _ = answer.try_recv();
    }

    let checked: HashSet<String> = browser.checked.borrow().iter().cloned().collect();
    for action in &checked {
        assert!(
            CORE_ALLOWS.contains(&action.as_str()),
            "{action} isn't the core's to allow"
        );
    }
    let page_actions = [
        "snapshot",
        "read_page",
        "click",
        "type",
        "press_key",
        "scroll",
    ];
    for action in page_actions
        .into_iter()
        .chain(["screenshot", "back", "hand_off", "close_tab"])
    {
        assert!(
            checked.contains(action),
            "{action} didn't ask the core: {checked:?}"
        );
    }
}

/// Agent tasks: a task is an agent of its own, `task-<n>`, with a hidden Space in the profile it
/// was asked from, and a card that says what it does.
mod tasks {
    use super::*;
    use crate::agents::ready::{Found, Ready};
    use crate::agents::runner::{self, Cli, Failure, Out};
    use crate::agents::task::{AskedOn, Dot, State, TaskAction};

    /// A stand-in for `claude` that keeps what it's sent beside itself, and never answers. Each
    /// is in a folder of its own: tests running at once that shared one would rewrite the script
    /// under each other's CLI, and each CLI's start would empty the other's `.in`.
    #[cfg(unix)]
    fn fake_cli(name: &str) -> PathBuf {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("ilo-tasks-{}-{name}-{made}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let program = dir.join("claude");
        std::fs::write(&program, "#!/bin/sh\nexec cat > \"$0.in\"\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        run_once_free(&program);
        program
    }

    /// Runs the fake CLI just written, with nothing to read, until it starts. Another test's thread
    /// that forked while the script was open for writing hands its child that handle until the
    /// child runs its own program, and the script won't run meanwhile: "Text file busy" (ETXTBSY).
    /// Once it has run, no such handle is left and none can come, as ours is closed, so the task's
    /// own start can't meet it. A retry, as cargo's and Go's spawns do; writing every fake before
    /// any thread forks isn't possible from inside the test binary.
    #[cfg(unix)]
    fn run_once_free(program: &std::path::Path) {
        for _ in 0..100 {
            match crate::child::command(program)
                .stdin(std::process::Stdio::null())
                .status()
            {
                Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                ran => {
                    ran.expect("the fake CLI didn't run");
                    return;
                }
            }
        }
        panic!("the fake CLI stayed busy for a second");
    }

    /// Agents on, with Claude signed in at `program`, and the user's Space 1.
    fn ready(program: Option<PathBuf>, cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
        // A real CLI's thread says when it ends, whenever that is, which wakes the test's tasks from
        // a thread GPUI doesn't run: unless told the test has real I/O, GPUI fails the test for it.
        if program.is_some() {
            cx.executor().allow_parking();
        }
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, None, cx);
        agents.update(cx, |agents, cx| {
            agents.set_on(true, cx);
            let found = Found {
                ready: Ready::SignedIn,
                program,
                ..Default::default()
            };
            agents.tasks.found.insert(Cli::Claude, found);
            agents.tasks.default = Some(Cli::Claude);
        });
        (agents, browser)
    }

    fn page() -> Option<AskedOn> {
        Some(AskedOn {
            url: "https://en.wikipedia.org/wiki/Arc_(web_browser)".into(),
            title: "Arc (web browser)".into(),
        })
    }

    fn start(agents: &Entity<Agents>, asked: &str, cx: &mut TestAppContext) -> String {
        agents
            .update(cx, |agents, cx| {
                agents.start_task(Cli::Claude, asked, page(), 1, cx)
            })
            .expect("started")
    }

    fn card_of(agents: &Entity<Agents>, cx: &mut TestAppContext) -> crate::agents::task::Card {
        agents.read_with(cx, |agents, cx| agents.cards(1, cx).remove(0))
    }

    fn said(agents: &Entity<Agents>, agent: &str, out: Out, cx: &mut TestAppContext) {
        let event = runner::Event {
            agent: agent.into(),
            out,
        };
        agents.update(cx, |agents, cx| agents.task_said(event, cx));
    }

    #[cfg(unix)]
    #[gpui::test]
    fn a_task_runs_its_cli_with_the_ask_and_the_pages_address_alone(cx: &mut TestAppContext) {
        let program = fake_cli("first");
        let (agents, browser) = ready(Some(program.clone()), cx);
        let agent = start(&agents, "Find a flight to Lisbon", cx);
        assert_eq!(agent, "task-1");
        // Its Space is the agent's own, in the profile of the Space it was asked in.
        assert_eq!(
            browser.made.borrow().last().unwrap(),
            &("task-1".to_string(), "Claude like 1".to_string())
        );
        let token = agents.read_with(cx, |agents, _| {
            agents.task("task-1").unwrap().token().to_string()
        });
        assert_eq!(
            agents.read_with(cx, |agents, _| agents
                .tokens
                .lock()
                .unwrap()
                .get(&token)
                .cloned()),
            Some(agent.clone())
        );
        // The fake CLI has the first message once it reads it.
        let input = program.with_extension("in");
        let mut sent = String::new();
        for _ in 0..200 {
            sent = std::fs::read_to_string(&input).unwrap_or_default();
            if !sent.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let message: Value = serde_json::from_str(sent.lines().next().unwrap()).unwrap();
        assert_eq!(message["type"], json!("user"));
        let text = message["message"]["content"].as_str().unwrap();
        assert!(text.starts_with("Find a flight to Lisbon"));
        assert!(
            text.contains("“Arc (web browser)”, https://en.wikipedia.org/wiki/Arc_(web_browser)")
        );
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.title.as_ref(), card.status.as_ref(), card.dot),
            ("Find a flight to Lisbon", "Thinking", Dot::Working)
        );
        // At quit its CLI ends and its Space goes, and so does Claude Code's, which only its
        // sessions' cards show.
        agents.update(cx, |agents, cx| agents.end_tasks(true, cx));
        assert_eq!(*browser.deleted.borrow(), vec![100, 2]);
        assert!(
            agents.read_with(cx, |agents, _| agents.tokens.lock().unwrap().is_empty()
                || !agents.tokens.lock().unwrap().contains_key(&token))
        );
    }

    /// At a quit with no window left, its CLI ends and its token with it, and its Space stays for
    /// the next launch to delete, as the core can't then (#1676).
    #[cfg(unix)]
    #[gpui::test]
    fn a_task_ended_with_no_window_left_keeps_its_space(cx: &mut TestAppContext) {
        let (agents, browser) = ready(Some(fake_cli("first")), cx);
        start(&agents, "Find a flight to Lisbon", cx);
        agents.update(cx, |agents, cx| agents.end_tasks(false, cx));
        assert!(browser.deleted.borrow().is_empty());
        assert!(agents.read_with(cx, |agents, _| agents.task("task-1").is_none()));
        assert!(agents.read_with(cx, |agents, _| agents.tokens.lock().unwrap().is_empty()));
    }

    #[gpui::test]
    fn a_cli_not_installed_says_how_to_install_it(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        start(&agents, "Find a flight", cx);
        let card = card_of(&agents, cx);
        assert_eq!(card.status.as_ref(), "Claude isn't installed.");
        assert_eq!(
            card.command,
            Some("npm install -g @anthropic-ai/claude-code")
        );
        assert_eq!(card.dot, Dot::Ended);
    }

    #[gpui::test]
    fn the_task_tools_are_a_tasks_alone(cx: &mut TestAppContext) {
        let names = tools::names("task-1");
        for tool in ["title", "ask", "finish", "open_tab", "hand_off"] {
            assert!(names.iter().any(|name| name == tool), "{tool}");
        }
        assert!(
            !tools::names("claude-code")
                .iter()
                .any(|name| name == "finish")
        );
        let (agents, _, _) = connected(cx);
        let answer = call(
            &agents,
            "claude-code",
            "title",
            json!({ "text": "Flights" }),
            cx,
        );
        assert_eq!(
            answer.recv().unwrap(),
            ToolResult::error("There's no tool title.")
        );
        // A task that isn't one the user asked can't call anything.
        let answer = call(&agents, "task-9", "list_tabs", json!({}), cx);
        assert_eq!(
            answer.recv().unwrap(),
            ToolResult::error("This agent isn't connected.")
        );
    }

    #[gpui::test]
    fn title_ask_and_finish_show_on_the_card_and_in_the_log(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Find a flight to Lisbon", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let answer = call(
            &agents,
            &agent,
            "title",
            json!({ "text": "Flights to Lisbon" }),
            cx,
        );
        assert!(!answer.recv().unwrap().error);
        let asked = call(
            &agents,
            &agent,
            "ask",
            json!({ "question": "Which day?", "answers": ["Friday", "Saturday"] }),
            cx,
        );
        let card = card_of(&agents, cx);
        assert_eq!(card.title.as_ref(), "Flights to Lisbon");
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("Which day?", Dot::Waiting)
        );
        assert_eq!(
            card.answers,
            vec![SharedString::from("Friday"), SharedString::from("Saturday")]
        );
        assert!(asked.try_recv().is_err(), "ask waits for the user");
        agents.update(cx, |agents, cx| agents.answer_task(&agent, "Saturday", cx));
        assert_eq!(
            asked.recv().unwrap(),
            ToolResult::text("The user answered: Saturday")
        );
        assert_eq!(card_of(&agents, cx).dot, Dot::Working);
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "Checkout", "https://shop.example/checkout"));
        let done = call(
            &agents,
            &agent,
            "finish",
            json!({ "result": "Found a flight for €80.", "summary": "€80 flight, Saturday", "tabs": [30] }),
            cx,
        );
        assert!(!done.recv().unwrap().error);
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("€80 flight, Saturday", Dot::Done)
        );
        assert_eq!(card.open, vec![(30, "shop.example".into())]);
        let actions: Vec<String> = log(&agents, &agent, cx)
            .into_iter()
            .map(|(action, _, _)| action)
            .collect();
        assert_eq!(
            actions,
            ["finish", "answered", "ask", "title"],
            "newest first"
        );
        // The core's auto-archive taking its page leaves the card saying where it went.
        let kept = browser.tabs.borrow_mut().pop().unwrap();
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.open),
            ("Its page went to the Archive", Vec::new())
        );
        browser.tabs.borrow_mut().push(kept);
        // Open <site> moves the tab to the user's Space.
        let shown = agents.update(cx, |agents, cx| agents.open_result(&agent, None, cx));
        assert_eq!(shown, Some(30), "it shows");
        assert!(browser.tabs.borrow().contains(&(
            30,
            1,
            "Checkout",
            "https://shop.example/checkout"
        )));
        assert_eq!(card_of(&agents, cx).open, Vec::new());
    }

    /// Finish can leave the user several pages, each once in the agent's order. Open moves them
    /// all to the top of the Space it was asked in, in that order, the first to show; the strip's
    /// Open moves the page shown alone, and the card counts what's left.
    #[gpui::test]
    fn finish_leaves_several_pages_and_open_moves_them_in_order(cx: &mut TestAppContext) {
        use crate::agents::task::open_label;
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Tickets for two museums and a garden", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        browser.tabs.borrow_mut().extend([
            (30, 100, "Tickets", "https://gulbenkian.pt/tickets"),
            (31, 100, "Visit", "https://www.maat.pt/en/visit"),
            (32, 100, "Garden", "https://estufafria.pt/"),
        ]);
        let finish = json!({ "result": "Tickets for all three.", "tabs": [31, 30, 31, 32] });
        assert!(
            !call(&agents, &agent, "finish", finish, cx)
                .recv()
                .unwrap()
                .error
        );
        let card = card_of(&agents, cx);
        let sites: Vec<_> = card
            .open
            .iter()
            .map(|(tab, site)| (*tab, site.as_ref()))
            .collect();
        assert_eq!(
            sites,
            [
                (31, "maat.pt"),
                (30, "gulbenkian.pt"),
                (32, "estufafria.pt")
            ]
        );
        assert_eq!(open_label(&card.open).as_deref(), Some("Open 3 pages"));
        assert_eq!(
            log(&agents, &agent, cx)[0].1,
            "3 pages",
            "the activity log's site"
        );
        // The strip's Open, on 30.
        let (shown, _) = agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::OpenTab(30), cx)
        });
        assert_eq!(shown, Some(30));
        assert_eq!(
            open_label(&card_of(&agents, cx).open).as_deref(),
            Some("Open 2 pages")
        );
        // The card's, the rest.
        let (shown, _) = agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::Open, cx)
        });
        assert_eq!(shown, Some(31), "the first shows");
        let home: Vec<u64> = (browser.tabs.borrow().iter())
            .filter(|tab| tab.1 == 1 && tab.0 >= 30)
            .map(|tab| tab.0)
            .collect();
        assert_eq!(
            home,
            [31, 32, 30],
            "at the top of its Today, in the agent's order, over the one opened before"
        );
        assert_eq!(card_of(&agents, cx).open, Vec::new());
        // One page's chip names its site.
        assert_eq!(
            open_label(&[(30, "gulbenkian.pt".into())]).as_deref(),
            Some("Open gulbenkian.pt")
        );
    }

    /// Finish leaves ten pages at most, and only the task's own, and says so when it's asked more,
    /// or when its result has its other arguments written into it.
    #[gpui::test]
    fn finish_refuses_eleven_pages_a_page_not_its_own_or_a_garbled_result(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Every museum in Lisbon", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        browser
            .tabs
            .borrow_mut()
            .extend((30..41).map(|tab| (tab, 100, "Tickets", "https://museum.example/tickets")));
        let eleven: Vec<u64> = (30..41).collect();
        let refused = call(
            &agents,
            &agent,
            "finish",
            json!({ "result": "Eleven museums.", "tabs": eleven }),
            cx,
        );
        let limit = "tabs takes 10 tabs at most: leave the user the 10 that matter most.";
        assert_eq!(refused.recv().unwrap(), ToolResult::error(limit));
        browser
            .tabs
            .borrow_mut()
            .push((50, 1, "Yours", "https://example.com/"));
        let refused = call(
            &agents,
            &agent,
            "finish",
            json!({ "result": "Two museums.", "tabs": [30, 50] }),
            cx,
        );
        let theirs =
            "Tab 50 isn't one of yours: give the ids of tabs in your Space, from list_tabs.";
        assert_eq!(refused.recv().unwrap(), ToolResult::error(theirs));
        // A result with its other arguments written into it, as Haiku wrote one, calls again.
        let garbled = "MAAT opens at 10:00.</result>\n<parameter name=\"summary\">MAAT 10–19h";
        let refused = call(&agents, &agent, "finish", json!({ "result": garbled }), cx);
        assert_eq!(
            refused.recv().unwrap(),
            ToolResult::error(crate::agents::task::FINISH_AGAIN)
        );
        assert_eq!(card_of(&agents, cx).dot, Dot::Working, "it goes on");
    }

    /// The card has an ask's question alone, and the conversation what the user needs to know to
    /// answer over it.
    #[gpui::test]
    fn an_asks_card_has_its_question_and_the_conversation_its_context(cx: &mut TestAppContext) {
        use crate::agents::task::Line;
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Tickets for a museum on Saturday", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let ask = json!({
            "question": "Which would you rather visit?",
            "context": "Gulbenkian opens at 10:00 and MAAT at 11:00.",
            "answers": ["Gulbenkian", "MAAT"],
        });
        let _asked = call(&agents, &agent, "ask", ask, cx);
        assert_eq!(
            card_of(&agents, cx).status.as_ref(),
            "Which would you rather visit?"
        );
        let conversation =
            agents.read_with(cx, |agents, cx| agents.conversation(&agent, cx).unwrap());
        let said = Line::Said(
            "Gulbenkian opens at 10:00 and MAAT at 11:00.\n\nWhich would you rather visit?".into(),
        );
        assert!(
            conversation.lines.contains(&said),
            "{:?}",
            conversation.lines
        );
    }

    /// An agent that puts it all in the question has the last sentence that asks on the card.
    #[test]
    fn a_long_ask_has_its_last_question_on_the_card() {
        use crate::agents::task::card_question;
        let cases = [
            (
                "Gulbenkian opens at 10:00 and MAAT at 11:00. Which would you rather visit? Both sell tickets online.",
                "Which would you rather visit?",
            ),
            (
                "MAAT is by the river, i.e. in Belém. Is that too far?",
                "Is that too far?",
            ),
            (
                "Would you rather see MAAT, i.e. the one in Belém?",
                "Would you rather see MAAT, i.e. the one in Belém?",
            ),
            (
                "Two are open:\n- Gulbenkian\n- MAAT\nWhich one?",
                "Which one?",
            ),
            ("Pick one of the two.", "Pick one of the two."),
        ];
        for (asked, card) in cases {
            assert_eq!(card_question(asked), card, "{asked}");
        }
    }

    /// Finish's result is the answer: the same words written just before it show once, and what
    /// the CLI writes after it doesn't show. Its summary is the card's line, whole: the card's
    /// ellipsis is the only cut.
    #[gpui::test]
    fn finishs_result_is_the_answer_and_its_summary_is_whole(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "When does MAAT open?", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        said(
            &agents,
            &agent,
            Out::Said("MAAT opens at 11:00 on Saturday.".into()),
            cx,
        );
        let summary = "MAAT, Saturday from 11:00 to 19:00";
        let finish = json!({ "result": "MAAT opens at 11:00 on Saturday.", "summary": summary });
        assert!(
            !call(&agents, &agent, "finish", finish, cx)
                .recv()
                .unwrap()
                .error
        );
        said(
            &agents,
            &agent,
            Out::Said("Done: MAAT opens at 11:00 on Saturday.".into()),
            cx,
        );
        let conversation =
            agents.read_with(cx, |agents, cx| agents.conversation(&agent, cx).unwrap());
        let shown = format!("{:?}", conversation.lines);
        assert_eq!(
            shown.matches("MAAT opens at 11:00 on Saturday.").count(),
            1,
            "{shown}"
        );
        assert_eq!(card_of(&agents, cx).status.as_ref(), summary);
    }

    /// What Claude writes shows as it comes, under the trace under way, and once whole, still
    /// once; a tool call takes it into the trace, where it showed.
    #[gpui::test]
    fn what_the_agent_writes_shows_as_it_comes_until_a_call_takes_it_in(cx: &mut TestAppContext) {
        use crate::agents::task::{Line, TraceItem};
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "What's on at MAAT?", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let shown = |cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, cx| agents.conversation(&agent, cx).unwrap())
        };
        said(
            &agents,
            &agent,
            Out::Thought {
                text: "Look at its page.".into(),
                took: Duration::from_secs(1),
            },
            cx,
        );
        said(&agents, &agent, Out::Writing(String::new()), cx);
        said(&agents, &agent, Out::Writing("Two".into()), cx);
        let conversation = shown(cx);
        assert!(conversation.writing);
        assert!(
            matches!(&conversation.lines[..], [Line::Trace(trace), Line::Said(text)] if trace.running && text == "Two"),
            "{:?}",
            conversation.lines
        );
        said(&agents, &agent, Out::Writing("Two shows".into()), cx);
        said(&agents, &agent, Out::Said("Two shows".into()), cx);
        let conversation = shown(cx);
        assert!(!conversation.writing, "it's whole");
        assert_eq!(
            format!("{:?}", conversation.lines)
                .matches("Two shows")
                .count(),
            1,
            "{:?}",
            conversation.lines
        );
        assert_eq!(
            conversation.lines.last(),
            Some(&Line::Said("Two shows".into()))
        );

        said(&agents, &agent, Out::Calling, cx);
        let lines = shown(cx).lines;
        let said_in = |trace: &crate::agents::task::Trace| {
            trace.items.contains(&TraceItem::Said("Two shows".into()))
        };
        assert!(
            matches!(&lines[..], [Line::Trace(trace)] if said_in(trace)),
            "{lines:?}"
        );
    }

    /// A turn the user sends says Thinking at once, under their message and on the card, before
    /// the CLI says anything; the agent's first step or message takes its place, and its result
    /// folds it. Nothing waits on the CLI once it has answered.
    #[cfg(unix)]
    #[gpui::test]
    fn a_sent_turn_thinks_until_the_agent_answers(cx: &mut TestAppContext) {
        use crate::agents::task::{Line, NO_ANSWER};
        let (agents, _) = ready(Some(fake_cli("thinking")), cx);
        let agent = start(&agents, "What is my name?", cx);
        let lines = |cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, cx| {
                agents.conversation(&agent, cx).unwrap().lines
            })
        };
        let waiting = |cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| {
                agents.task(&agent).unwrap().waiting.is_some()
            })
        };
        assert_eq!(
            lines(cx),
            [Line::Thinking {
                key: 0,
                failed: None
            }],
            "Run ↩ shows nothing"
        );
        let card = card_of(&agents, cx);
        assert_eq!((card.status.as_ref(), card.dot), ("Thinking", Dot::Working));
        assert!(waiting(cx), "nothing ends a CLI that never answers");

        // Its first step takes the line's place, as a trace under way.
        said(
            &agents,
            &agent,
            Out::Thought {
                text: "The user asks their name.".into(),
                took: Duration::from_secs(2),
            },
            cx,
        );
        assert!(
            matches!(&lines(cx)[..], [Line::Trace(trace)] if trace.running && trace.steps() == 1),
            "{:?}",
            lines(cx)
        );
        assert!(!waiting(cx), "a timer is out once it answered");
        said(
            &agents,
            &agent,
            Out::Said("I don't know your name.".into()),
            cx,
        );
        said(&agents, &agent, Out::Ended(None), cx);
        assert_eq!(card_of(&agents, cx).dot, Dot::Done);

        // A reply: Thinking again at once, under it, and the card's glyph goes back to working.
        agents.update(cx, |agents, cx| agents.reply(&agent, "It's Ada.", cx));
        let shown = lines(cx);
        assert_eq!(
            shown[shown.len() - 2..],
            [
                Line::You("It's Ada.".into()),
                Line::Thinking {
                    key: 1,
                    failed: None
                }
            ]
        );
        let card = card_of(&agents, cx);
        assert_eq!((card.status.as_ref(), card.dot), ("Thinking", Dot::Working));
        // Its result folds it: the answer shows in its place.
        said(
            &agents,
            &agent,
            Out::Said("Nice to meet you, Ada.".into()),
            cx,
        );
        said(&agents, &agent, Out::Ended(None), cx);
        assert_eq!(
            lines(cx).last(),
            Some(&Line::Said("Nice to meet you, Ada.".into()))
        );
        assert!(!waiting(cx));
        cx.executor().advance_clock(NO_ANSWER * 2);
        cx.run_until_parked();
        assert_eq!(
            card_of(&agents, cx).dot,
            Dot::Done,
            "a timer ended a task that had answered"
        );
    }

    /// A CLI that never answers, or goes before it does, never leaves Thinking going: the line
    /// says why the agent couldn't start, and the card why it stopped. A CLI that ends is a
    /// thread's news, which a test's clock can't wait for, so the task here has none.
    #[gpui::test]
    fn a_turn_the_cli_never_answers_says_why(cx: &mut TestAppContext) {
        use crate::agents::task::{Line, NO_ANSWER};
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "What is my name?", cx);
        // As though its CLI started, and says nothing.
        agents.update(cx, |agents, cx| {
            agents.task_mut(&agent).unwrap().state = State::Working;
            agents.wait_for_answer(&agent, cx);
        });
        let last = |cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, cx| {
                agents
                    .conversation(&agent, cx)
                    .unwrap()
                    .lines
                    .pop()
                    .unwrap()
            })
        };
        cx.executor()
            .advance_clock(NO_ANSWER - Duration::from_secs(1));
        cx.run_until_parked();
        assert_eq!(
            last(cx),
            Line::Thinking {
                key: 0,
                failed: None
            },
            "it gave up early"
        );
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert_eq!(
            last(cx),
            Line::Thinking {
                key: 0,
                failed: Some("Claude didn't answer".into())
            }
        );
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("Claude didn't answer.", Dot::Ended)
        );
        assert!(agents.read_with(cx, |agents, _| {
            agents.task(&agent).unwrap().waiting.is_none()
        }));

        // A reply that can't start its CLI says why under it.
        agents.update(cx, |agents, cx| agents.reply(&agent, "Are you there?", cx));
        assert_eq!(
            last(cx),
            Line::Thinking {
                key: 0,
                failed: Some("Claude isn't installed".into())
            }
        );
        // A CLI that goes before it answers says so.
        agents.update(cx, |agents, cx| {
            agents.task_mut(&agent).unwrap().state = State::Working;
            agents.wait_for_answer(&agent, cx);
        });
        assert_eq!(
            last(cx),
            Line::Thinking {
                key: 0,
                failed: None
            }
        );
        said(&agents, &agent, Out::Gone(String::new()), cx);
        assert_eq!(
            last(cx),
            Line::Thinking {
                key: 0,
                failed: Some("Claude stopped working".into())
            }
        );
        assert!(agents.read_with(cx, |agents, _| {
            agents.task(&agent).unwrap().waiting.is_none()
        }));
    }

    #[gpui::test]
    fn a_long_answer_or_many_ask_in_the_conversation(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Plan a trip", cx);
        let _asked = call(
            &agents,
            &agent,
            "ask",
            json!({
                "question": "Where to?", "answers": ["Lisbon", "Porto", "Faro", "Madeira"],
            }),
            cx,
        );
        assert!(card_of(&agents, cx).answers.is_empty());
        // A reply answers it.
        agents.update(cx, |agents, cx| agents.reply(&agent, "Porto", cx));
        assert_eq!(
            _asked.recv().unwrap(),
            ToolResult::text("The user answered: Porto")
        );
    }

    #[gpui::test]
    fn a_confirmation_or_a_handoff_waits_on_the_card(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Check my balance", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let _opening = call(
            &agents,
            &agent,
            "open_tab",
            json!({ "url": "https://www.chase.com/" }),
            cx,
        );
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("Waiting for your OK", Dot::Waiting)
        );
        let question = card.question.expect("asked on the card");
        assert_eq!(
            (question.agent.as_ref(), question.from.as_ref()),
            ("Claude", "task-1")
        );
        agents.update(cx, |agents, cx| {
            agents.answer(question.id, Answer::Never, cx)
        });
        browser
            .tabs
            .borrow_mut()
            .push((31, 100, "Sign in", "https://example.com/login"));
        let _handed = call(
            &agents,
            &agent,
            "hand_off",
            json!({ "reason": "Sign in, please.", "tab": 31 }),
            cx,
        );
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("Waiting for you", Dot::Waiting)
        );
        assert_eq!(
            card.question.and_then(|question| question.handoff),
            Some(31)
        );
    }

    /// An agent's calls on several tabs go at once (#1476), so its prompts can come together: they
    /// show one at a time, in the order they came, the confirmation after the ask waiting for it.
    #[gpui::test]
    fn an_agents_prompts_show_one_at_a_time_in_turn(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Move the ten cases to Closed", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let ask = json!({ "question": "Close all ten?", "answers": ["Yes", "No"] });
        let asked = call(&agents, &agent, "ask", ask, cx);
        let bank = json!({ "url": "https://www.chase.com/" });
        let _opening = call(&agents, &agent, "open_tab", bank, cx);
        let card = card_of(&agents, cx);
        assert!(
            card.asks && card.question.is_none(),
            "the confirmation went first"
        );
        assert_eq!(
            (card.status.as_ref(), card.answers.len()),
            ("Close all ten?", 2)
        );
        assert_eq!(*browser.asked.borrow(), None, "a window asked out of turn");
        agents.update(cx, |agents, cx| agents.answer_task(&agent, "Yes", cx));
        assert!(!asked.recv().unwrap().error);
        let question = browser
            .asked
            .borrow()
            .clone()
            .expect("the confirmation never asked");
        assert_eq!(question.from.as_ref(), agent);
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.asks),
            ("Waiting for your OK", false)
        );
    }

    #[gpui::test]
    fn limits_and_signing_out_say_what_to_do(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Find a flight", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        said(&agents, &agent, Out::Ended(Some(Failure::Limit(None))), cx);
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.retry),
            ("Claude hit its usage limit", true)
        );
        assert!(!card.finished(), "Retry waits for the reset");
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        said(
            &agents,
            &agent,
            Out::Ended(Some(Failure::Limit(Some(at)))),
            cx,
        );
        let resets = crate::agents::local_time(at);
        assert_eq!(
            card_of(&agents, cx).status.to_string(),
            format!("Limit resets at {resets}")
        );
        let listed = agents.read_with(cx, |agents, _| agents.ask_agents());
        assert_eq!(listed[0].resets.as_deref(), Some(resets.as_str()));
        said(&agents, &agent, Out::Ended(Some(Failure::SignedOut)), cx);
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.command),
            ("Claude isn't signed in.", Some("claude auth login"))
        );
    }

    #[gpui::test]
    fn a_stopped_task_says_stopped_once_its_turn_ends(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Find a flight", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        agents.update(cx, |agents, cx| agents.stop_task(&agent, cx));
        let interrupted = Failure::Other("Claude ran into an error.".into());
        said(&agents, &agent, Out::Ended(Some(interrupted)), cx);
        let card = card_of(&agents, cx);
        assert_eq!((card.status.as_ref(), card.dot), ("Stopped", Dot::Ended));
    }

    /// Calls `tool` as the server does, which stamps when it started and, once answered, when it
    /// ended: by `page` from the core, if it goes to one.
    fn traced(
        agents: &Entity<Agents>,
        browser: &Made,
        agent: &str,
        tool: &str,
        arguments: Value,
        page: Option<String>,
        cx: &mut TestAppContext,
    ) -> ToolResult {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        let at = std::time::Instant::now();
        let call = Call {
            id,
            agent: agent.into(),
            tool: tool.into(),
            arguments,
            reply,
            at,
        };
        agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
        let result = match page {
            Some(text) => page_done(agents, browser, answer, text, cx),
            None => answer.recv().unwrap(),
        };
        let error = result.error.then(|| result.text.clone());
        let at = std::time::Instant::now();
        let answered = Event::Answered {
            agent: agent.into(),
            id,
            at,
            error,
            image: result.image.clone(),
        };
        agents.update(cx, |agents, cx| agents.handle(answered, cx));
        result
    }

    /// A task's calls are its conversation's trace, each titled by what it did and opening to what
    /// it saw or why it failed. What it typed shows, but never what it typed into a password
    /// field, though the field refused it.
    #[gpui::test]
    fn a_tasks_calls_are_its_trace_and_never_show_a_password(cx: &mut TestAppContext) {
        use crate::agents::task::{Line, Opens, TraceItem};
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Use the coupon", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "Shop", "https://shop.example/basket"));
        traced(
            &agents,
            &browser,
            &agent,
            "snapshot",
            json!({ "tab": 30 }),
            Some(shop_tree()),
            cx,
        );
        let coupon = json!({ "tab": 30, "ref": "e1", "text": "SPRING10" });
        traced(
            &agents,
            &browser,
            &agent,
            "type",
            coupon,
            Some(shop_tree()),
            cx,
        );
        let password = json!({ "tab": 30, "ref": "e2", "text": "hunter2" });
        assert!(traced(&agents, &browser, &agent, "type", password, None, cx).error);
        traced(
            &agents,
            &browser,
            &agent,
            "screenshot",
            json!({ "tab": 30 }),
            Some("/9j/4AAQ".into()),
            cx,
        );

        let conversation =
            agents.read_with(cx, |agents, cx| agents.conversation(&agent, cx).unwrap());
        let [Line::Trace(trace)] = conversation.lines.as_slice() else {
            panic!("{:?}", conversation.lines)
        };
        assert!(trace.running, "it's still at work");
        let steps: Vec<_> = (trace.items.iter())
            .filter_map(|item| match item {
                TraceItem::Step(step) => Some(step),
                TraceItem::Said(_) => None,
            })
            .collect();
        let titles: Vec<_> = steps
            .iter()
            .map(|step| (step.title.as_ref(), step.description.as_ref(), step.failed))
            .collect();
        assert_eq!(
            titles,
            [
                ("Read shop.example", "", false),
                ("Typed “SPRING10” in “Coupon”", "", false),
                ("Couldn't type in “Password”", "password field", true),
                ("Looked at shop.example", "", false),
            ]
        );
        assert!(
            matches!(&steps[2].opens, Opens::Text(why) if why.starts_with("Password fields are off limits"))
        );
        assert!(matches!(steps[3].opens, Opens::Picture(_)));
        let card = card_of(&agents, cx);
        assert_eq!(
            card.status.as_ref(),
            "Looked at shop.example",
            "the card says the step it's on"
        );
        assert!(
            !format!("{conversation:?}{card:?}").contains("hunter2"),
            "the password showed"
        );
        let log = format!("{:?}", log(&agents, &agent, cx));
        assert!(
            !log.contains("hunter2"),
            "the password reached the activity log"
        );
    }

    /// An open's line titles its site alone and opens to its whole address.
    #[gpui::test]
    fn an_open_line_titles_its_site_and_opens_to_its_address(cx: &mut TestAppContext) {
        use crate::agents::task::{Line, Opens, TraceItem};
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "When does MAAT open?", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let url = "https://www.maat.pt/en/visit?lang=en";
        traced(
            &agents,
            &browser,
            &agent,
            "open_tab",
            json!({ "url": url }),
            Some(shop_tree()),
            cx,
        );
        let conversation =
            agents.read_with(cx, |agents, cx| agents.conversation(&agent, cx).unwrap());
        let [Line::Trace(trace)] = conversation.lines.as_slice() else {
            panic!("{:?}", conversation.lines)
        };
        let [TraceItem::Step(step)] = trace.items.as_slice() else {
            panic!("{:?}", trace.items)
        };
        assert_eq!(
            (step.title.as_ref(), step.description.as_ref()),
            ("Opened maat.pt", "")
        );
        assert!(
            matches!(&step.opens, Opens::Text(text) if text == url),
            "{:?}",
            step.opens
        );
    }

    /// A failed step says why when its error gives a reason, and nothing when it doesn't, as its
    /// title already says it couldn't. A failed open opens to its address over the error.
    #[test]
    fn a_failed_step_says_why_only_when_it_knows() {
        use crate::agents::task::{Line, Opens, TraceItem, Traced, fold};
        let reasons = [
            ("The tab couldn't open.", ""),
            ("Timed out waiting for the page.", "timed out"),
            ("The page answered HTTP 504.", "504"),
            ("Tab 30 isn't an http or https page.", "not a web page"),
            ("The page is still loading.", "still loading"),
            ("There's no element e12 on the page.", "not found"),
        ];
        for (error, reason) in reasons {
            assert_eq!(tools::reason(error), reason, "{error}");
        }
        let now = std::time::Instant::now();
        let url = "https://www.maat.pt/en/visit";
        let failed = Traced {
            call: 1,
            did: tools::Did {
                tool: "open_tab".into(),
                site: "maat.pt".into(),
                text: url.into(),
                ..Default::default()
            },
            after: 0,
            started: now,
            ended: Some(now),
            error: Some("Timed out waiting for the page.".into()),
            picture: None,
        };
        let lines = fold(&[], &[failed], true, false, None);
        let [Line::Trace(trace)] = lines.as_slice() else {
            panic!("{lines:?}")
        };
        let [TraceItem::Step(step)] = trace.items.as_slice() else {
            panic!("{:?}", trace.items)
        };
        assert_eq!(
            (step.title.as_ref(), step.description.as_ref(), step.failed),
            ("Couldn't open maat.pt", "timed out", true)
        );
        let opens = format!("{url}\nTimed out waiting for the page.");
        assert!(
            matches!(&step.opens, Opens::Text(text) if *text == opens),
            "{:?}",
            step.opens
        );
    }

    /// A step says what it did in the tense it's in: under way, done or failed. A field's text
    /// shows only with the field's name, and a wait says what it waited for or how long.
    #[test]
    fn each_step_says_what_it_did_in_its_tense() {
        use crate::agents::tools::{Did, Tense, step_title};
        let did = |tool: &str, site: &str, target: &str, text: &str| Did {
            tool: tool.into(),
            site: site.into(),
            target: target.into(),
            text: text.into(),
            ..Default::default()
        };
        let titles = [
            (
                did("open_tab", "gulbenkian.pt", "", ""),
                Tense::Now,
                "Opening gulbenkian.pt",
            ),
            (
                did("navigate", "maat.pt", "", ""),
                Tense::Done,
                "Opened maat.pt",
            ),
            (
                did("open_tab", "maat.pt/en/visit", "", ""),
                Tense::Failed,
                "Couldn't open maat.pt/en/visit",
            ),
            (
                did("back", "maat.pt", "", ""),
                Tense::Done,
                "Went back on maat.pt",
            ),
            (
                did("read_page", "maat.pt", "", ""),
                Tense::Now,
                "Reading maat.pt",
            ),
            (
                did("snapshot", "maat.pt", "", ""),
                Tense::Done,
                "Read maat.pt",
            ),
            (
                did("screenshot", "maat.pt", "", ""),
                Tense::Done,
                "Looked at maat.pt",
            ),
            (
                did("click", "gulbenkian.pt", "“Reabertura do Museu”", ""),
                Tense::Done,
                "Clicked “Reabertura do Museu”",
            ),
            (
                did("type", "", "“Search”", "Lisbon"),
                Tense::Done,
                "Typed “Lisbon” in “Search”",
            ),
            (
                did("type", "", "“Search”", ""),
                Tense::Now,
                "Typing in “Search”",
            ),
            (
                did("press_key", "", "Enter", ""),
                Tense::Done,
                "Pressed Enter",
            ),
            (
                did("scroll", "maat.pt", "down", ""),
                Tense::Done,
                "Scrolled down on maat.pt",
            ),
            (
                did("scroll", "maat.pt", "“Opening hours”", ""),
                Tense::Now,
                "Scrolling to “Opening hours”",
            ),
            (did("wait", "", "2s", ""), Tense::Done, "Waited 2s"),
            (
                did("wait", "", "“Tickets”", ""),
                Tense::Failed,
                "Didn't see “Tickets”",
            ),
            (
                did("list_tabs", "", "", ""),
                Tense::Done,
                "Looked over its tabs",
            ),
            (
                did("close_tab", "maat.pt", "", ""),
                Tense::Done,
                "Closed maat.pt",
            ),
            (did("ask", "", "", ""), Tense::Done, "Asked you"),
            (did("thought", "", "", ""), Tense::Now, "Thinking"),
            (did("thought", "", "", ""), Tense::Done, "Thought"),
        ];
        for (did, tense, title) in titles {
            assert_eq!(step_title(&did, tense), title, "{did:?} {tense:?}");
        }
    }

    /// The stretch under way is an open trace with what the agent said along the way in it; its
    /// turn's message folds it to how long it worked, from its first step to that message.
    #[test]
    fn a_stretch_folds_when_its_turns_message_comes() {
        use crate::agents::task::{Line, Message, TraceItem, Traced, Who, fold};
        let start = std::time::Instant::now();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let message = |who, text: &str, seconds| Message {
            who,
            text: text.to_string().into(),
            at: at(seconds),
        };
        let step = |tool: &str, after, started, ended: Option<u64>| Traced {
            call: 1,
            did: tools::Did {
                tool: tool.into(),
                site: "maat.pt".into(),
                ..Default::default()
            },
            after,
            started: at(started),
            ended: ended.map(at),
            error: None,
            picture: None,
        };
        let mut messages = vec![message(Who::User, "Museum hours", 0)];
        let mut trace = vec![step("open_tab", 1, 1, Some(3))];
        messages.push(message(Who::Agent, "Looking at MAAT first.", 3));
        trace.push(step("read_page", 2, 4, None));

        let lines = fold(&messages, &trace, false, true, None);
        let [Line::You(_), Line::Trace(working)] = lines.as_slice() else {
            panic!("{lines:?}")
        };
        assert!(working.running);
        assert_eq!(working.summary(), ("Working".into(), "2 steps".into()));
        assert!(
            matches!(&working.items[1], TraceItem::Said(said) if said == "Looking at MAAT first.")
        );
        let TraceItem::Step(last) = &working.items[2] else {
            panic!()
        };
        assert!(last.running, "the step under way shimmers");

        trace[1].ended = Some(at(6));
        messages.push(message(Who::Agent, "MAAT opens at 10:00.", 41));
        let lines = fold(&messages, &trace, true, false, None);
        let [Line::You(_), Line::Trace(worked), Line::Said(said)] = lines.as_slice() else {
            panic!("{lines:?}")
        };
        assert!(!worked.running);
        assert_eq!(
            worked.summary(),
            ("Worked for 40s".into(), "2 steps".into())
        );
        assert_eq!(said.as_ref(), "MAAT opens at 10:00.");
        assert_eq!(
            worked.key, working.key,
            "the user's opening of it carries across its end"
        );
        assert!(
            worked
                .items
                .iter()
                .all(|item| !matches!(item, TraceItem::Step(step) if step.running))
        );
    }

    /// A reply leaves the turn before it as it was, its answer and how long it worked, as the
    /// next turn gets under way.
    #[test]
    fn a_reply_leaves_the_turn_before_it_as_it_was() {
        use crate::agents::task::{Line, Message, Traced, Who, fold};
        let start = std::time::Instant::now();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let message = |who, text: &str, seconds| Message {
            who,
            text: text.to_string().into(),
            at: at(seconds),
        };
        let trace = vec![Traced {
            call: 1,
            did: tools::Did {
                tool: "open_tab".into(),
                site: "maat.pt".into(),
                ..Default::default()
            },
            after: 1,
            started: at(1),
            ended: Some(at(3)),
            error: None,
            picture: None,
        }];
        let mut messages = vec![
            message(Who::User, "Museum hours", 0),
            message(Who::Result, "MAAT opens at 10:00.", 5),
        ];
        let before = fold(&messages, &trace, true, false, None);
        messages.push(message(Who::User, "And on Sunday?", 9));
        let lines = fold(&messages, &trace, false, true, None);
        assert_eq!(lines[..before.len()], before[..], "{lines:?}");
        assert_eq!(lines.last(), Some(&Line::You("And on Sunday?".into())));
    }

    /// A turn's same step on ten tabs is one line with how many, which opens to each tab's line by
    /// its address, whether the agent's calls came at once, interleaved, or one after another.
    /// Each tab's steps still read in their order (#1476).
    #[test]
    fn a_turns_same_step_on_many_tabs_is_one_line() {
        use crate::agents::task::{Line, Message, Opens, TraceItem, Traced, Who, fold};
        let start = std::time::Instant::now();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let step = |tool: &str, tab: u64, target: &str, seconds: u64| Traced {
            call: 1,
            did: tools::Did {
                tool: tool.into(),
                site: "cases.example".into(),
                target: target.into(),
                // A page it opens makes its tab.
                tab: (tool != "open_tab").then_some(tab),
                page: format!("cases.example/{tab}"),
                ..Default::default()
            },
            after: 1,
            started: at(seconds),
            ended: Some(at(seconds + 1)),
            error: None,
            picture: None,
        };
        let messages = [Message {
            who: Who::User,
            text: "Close cases 1 to 10".into(),
            at: at(0),
        }];
        let lines = |trace: &[Traced]| {
            let lines = fold(&messages, trace, false, true, None);
            let [Line::You(_), Line::Trace(trace)] = lines.as_slice() else {
                panic!("{lines:?}")
            };
            (trace.items.iter())
                .map(|item| match item {
                    TraceItem::Step(step) => step.clone(),
                    TraceItem::Said(said) => panic!("{said}"),
                })
                .collect::<Vec<_>>()
        };
        let titles = |trace: &[Traced]| {
            (lines(trace).iter())
                .map(|step| (step.title.to_string(), step.description.to_string()))
                .collect::<Vec<_>>()
        };
        let mut trace: Vec<Traced> = (1..=10).map(|tab| step("open_tab", tab, "", 1)).collect();
        // A wait then a read on each tab, whose steps come as each tab gets to them.
        for tab in 1..=10 {
            trace.push(step("wait", tab, "“Ready”", 3));
            if tab > 1 {
                trace.push(step("snapshot", tab - 1, "", 4));
            }
        }
        trace.push(step("snapshot", 10, "", 4));
        trace.extend((1..=10).map(|tab| step("click", tab, "“Closed”", 6)));
        trace.last_mut().unwrap().ended = None;
        let owned = |pairs: &[(&str, &str)]| {
            (pairs.iter())
                .map(|(title, description)| (title.to_string(), description.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            titles(&trace),
            owned(&[
                ("Opened 10 pages", "1s"),
                ("Waited for “Ready” on 10 pages", ""),
                ("Read 10 pages", "1s"),
                ("Clicking “Closed” on 10 pages", "9 of 10"),
            ])
        );
        let clicks = lines(&trace).pop().unwrap();
        assert!(clicks.running, "the step under way shimmers");
        let Opens::Steps(tabs) = &clicks.opens else {
            panic!("{:?}", clicks.opens)
        };
        let addresses: Vec<String> = tabs.iter().map(|tab| tab.title.to_string()).collect();
        assert_eq!(
            addresses,
            (1..=10)
                .map(|tab| format!("cases.example/{tab}"))
                .collect::<Vec<_>>()
        );

        // One tab's click failed: the line says how many, and the tab's says why.
        let last = trace.last_mut().unwrap();
        (last.ended, last.error) = (
            Some(at(7)),
            Some("There's no element e7 on the page.".into()),
        );
        let clicks = lines(&trace).pop().unwrap();
        let said = (clicks.title.as_ref(), clicks.description.as_ref());
        assert_eq!(said, ("Clicked “Closed” on 10 pages", "1 failed"));
        let Opens::Steps(tabs) = &clicks.opens else {
            panic!()
        };
        assert!(clicks.failed && tabs[9].failed && !tabs[8].failed);
        assert_eq!(tabs[9].description.as_ref(), "not found");

        // One after another, a step that came on a tab since a line began starts a line of its
        // own, so tab 1's second click reads after its wait.
        let trace = [
            step("click", 1, "“Save”", 10),
            step("wait", 1, "“Saved”", 11),
            step("click", 2, "“Save”", 12),
            step("wait", 2, "“Saved”", 13),
            step("click", 1, "“Save”", 14),
        ];
        assert_eq!(
            titles(&trace),
            owned(&[
                ("Clicked “Save” on 2 pages", "3s"),
                ("Waited for “Saved” on 2 pages", ""),
                ("Clicked “Save”", "1s"),
            ])
        );
    }

    #[gpui::test]
    fn a_turn_ending_without_finish_is_done(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "What's on today?", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        said(
            &agents,
            &agent,
            Out::Said("## Today\n\n**Three meetings**, the first at 9.".into()),
            cx,
        );
        said(&agents, &agent, Out::Ended(None), cx);
        let card = card_of(&agents, cx);
        // The tick says it's done, so the line says what the result does.
        assert_eq!((card.status.as_ref(), card.dot), ("Today", Dot::Done));
        assert!(card.finished());
    }

    #[gpui::test]
    fn stop_ends_it_and_refuses_what_waits(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Check my balance", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let opening = call(
            &agents,
            &agent,
            "open_tab",
            json!({ "url": "https://www.chase.com/" }),
            cx,
        );
        agents.update(cx, |agents, cx| agents.stop_task(&agent, cx));
        assert_eq!(opening.recv().unwrap(), ToolResult::error(STOPPED));
        let card = card_of(&agents, cx);
        assert_eq!((card.status.as_ref(), card.question), ("Stopped", None));
        // Its turn ending after the stop leaves it stopped.
        said(&agents, &agent, Out::Ended(None), cx);
        assert_eq!(card_of(&agents, cx).status.as_ref(), "Stopped");
    }

    #[gpui::test]
    fn dismissed_tasks_come_back(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let first = start(&agents, "One", cx);
        let second = start(&agents, "Two", cx);
        agents.update(cx, |agents, cx| {
            agents.task_mut(&first).unwrap().state = State::Done {
                result: "Did one.".into(),
                summary: None,
                tabs: Vec::new(),
            };
            agents.task_mut(&second).unwrap().state = State::Working;
            let _ = cx;
        });
        let titles = |agents: &Entity<Agents>, cx: &mut TestAppContext| -> Vec<String> {
            agents.read_with(cx, |agents, cx| {
                agents
                    .cards(1, cx)
                    .into_iter()
                    .map(|card| card.title.to_string())
                    .collect()
            })
        };
        assert_eq!(titles(&agents, cx), ["Two", "One"], "newest on top");
        agents.update(cx, |agents, cx| agents.dismiss_task(&first, cx));
        assert_eq!(titles(&agents, cx), ["Two"]);
        agents.update(cx, |agents, cx| agents.dismiss_task(&second, cx));
        assert!(titles(&agents, cx).is_empty());
        assert_eq!(
            agents.update(cx, |agents, cx| agents.restore_task(None, cx)),
            Some(second.clone())
        );
        assert_eq!(
            agents.update(cx, |agents, cx| agents.restore_task(None, cx)),
            Some(first.clone())
        );
        assert_eq!(titles(&agents, cx), ["Two", "One"]);
        // Past the last few dismissed, a task goes for good, with its Space.
        for each in 0..11 {
            let agent = start(&agents, &format!("Task {each}"), cx);
            agents.update(cx, |agents, cx| agents.dismiss_task(&agent, cx));
        }
        assert_eq!(*browser.deleted.borrow(), vec![102]);
    }

    /// The Archive lists the dismissed tasks, the last to leave the foot first, each with how it
    /// ended and when. Delete from Archive takes one out of the list and out of ⇧⌘T's reach, and
    /// ⌘Z's Undelete puts it back.
    #[gpui::test]
    fn the_archive_lists_dismissed_tasks_until_one_is_deleted(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let first = start(&agents, "One", cx);
        let second = start(&agents, "Two", cx);
        let ended = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&first).unwrap();
            task.state = State::Done {
                result: "Did one.".into(),
                summary: None,
                tabs: Vec::new(),
            };
            task.ended = Some(ended);
            agents.task_mut(&second).unwrap().state = State::Working;
        });
        agents.update(cx, |agents, cx| agents.dismiss_task(&first, cx));
        agents.update(cx, |agents, cx| agents.dismiss_task(&second, cx));
        let listed =
            |agents: &Entity<Agents>, cx: &mut TestAppContext| -> Vec<(String, String, Dot)> {
                agents.read_with(cx, |agents, cx| {
                    let tasks = agents.archived_tasks(cx).into_iter();
                    tasks
                        .map(|task| (task.agent.to_string(), task.about.to_string(), task.dot))
                        .collect()
                })
            };
        let now = listed(&agents, cx);
        assert_eq!(
            now.iter()
                .map(|(agent, ..)| agent.clone())
                .collect::<Vec<_>>(),
            [second.clone(), first.clone()]
        );
        let at = crate::agents::local_time(1_700_000_000);
        assert_eq!(
            now[1],
            (first.clone(), format!("Claude · Done {at}"), Dot::Done)
        );
        // Dismissed while it worked, it stopped then.
        assert!(now[0].1.starts_with("Claude · Stopped "), "{}", now[0].1);
        assert_eq!(now[0].2, Dot::Ended);

        agents.update(cx, |agents, cx| {
            agents.task_action(&second, TaskAction::Delete, cx)
        });
        assert_eq!(listed(&agents, cx).len(), 1);
        assert_eq!(
            agents.update(cx, |agents, cx| agents.restore_task(None, cx)),
            Some(first.clone())
        );
        assert_eq!(
            agents.update(cx, |agents, cx| agents.restore_task(None, cx)),
            None,
            "⇧⌘T brought back a deleted task"
        );
        agents.update(cx, |agents, cx| {
            agents.task_action(&second, TaskAction::Undelete, cx)
        });
        assert_eq!(listed(&agents, cx).len(), 1);
        assert_eq!(
            agents.update(cx, |agents, cx| agents.restore_task(None, cx)),
            Some(second.clone())
        );
        assert!(listed(&agents, cx).is_empty());
    }

    /// At launch the tasks' Spaces a run before left go, and an outside agent's, which a run
    /// before #1080 showed as a Space of the user's, gives its tabs to the user's Space first.
    #[gpui::test]
    fn the_spaces_a_run_before_left_go_at_launch(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        browser.task_spaces.borrow_mut().extend([100, 101]);
        agents.update(cx, |agents, cx| agents.end_old_tasks(cx));
        assert_eq!(*browser.deleted.borrow(), vec![100, 101, 2]);
        let spaces: Vec<_> = (browser.tabs.borrow().iter())
            .map(|each| (each.0, each.1))
            .collect();
        assert_eq!(spaces, [(20, 1), (10, 1)]);
    }

    #[gpui::test]
    fn ask_lists_the_signed_in_with_the_default_first(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        agents.update(cx, |agents, cx| {
            let found = Found {
                ready: Ready::SignedIn,
                ..Default::default()
            };
            agents.tasks.found.insert(Cli::Codex, found);
            agents.set_default_task_agent(Cli::Codex, cx);
        });
        let listed: Vec<Cli> = agents.read_with(cx, |agents, _| {
            agents
                .ask_agents()
                .into_iter()
                .map(|agent| agent.cli)
                .collect()
        });
        assert_eq!(listed, [Cli::Codex, Cli::Claude]);
        agents.update(cx, |agents, cx| {
            agents.tasks.found.get_mut(&Cli::Codex).unwrap().ready = Ready::SignedOut;
            agents.set_on(false, cx);
        });
        assert!(
            agents
                .read_with(cx, |agents, _| agents.ask_agents())
                .is_empty(),
            "off lists none"
        );
    }

    #[gpui::test]
    fn the_default_and_models_are_saved(cx: &mut TestAppContext) {
        let folder = std::env::temp_dir().join(format!("ilo-task-settings-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("Agents");
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, Some(path.clone()), cx);
        agents.update(cx, |agents, cx| {
            agents.set_default_task_agent(Cli::Codex, cx);
            agents.set_task_model(Cli::Claude, "haiku", cx);
        });
        let again = super::agents(&browser, &configs, Some(path.clone()), cx);
        let saved = std::fs::read_to_string(&path).unwrap();
        again.update(cx, |agents, cx| agents.load(&saved, cx));
        again.read_with(cx, |agents, _| {
            assert_eq!(agents.tasks.default, Some(Cli::Codex));
            assert_eq!(agents.task_model(Cli::Claude), "haiku");
            assert_eq!(agents.task_model(Cli::Codex), "");
        });
        std::fs::remove_dir_all(folder).unwrap();
    }
}

/// An outside agent's session (#1080): one task card for each client the user connected, from its
/// first call that opens or acts on a tab, whichever of its connections it comes on, until its last
/// connection closes, it waits IDLE for a call, or the user stops it.
mod sessions {
    use super::*;
    use crate::agents::task::{Card, Dot, IDLE, Line, TaskAction};

    /// Calls `tool` as Claude Code, as call `id`, which `answered` ends.
    fn call_as(
        agents: &Entity<Agents>,
        id: u64,
        tool: &str,
        arguments: Value,
        cx: &mut TestAppContext,
    ) -> mpsc::Receiver<ToolResult> {
        let (reply, answer) = mpsc::channel();
        let call = Call {
            id,
            agent: "claude-code".into(),
            tool: tool.into(),
            arguments,
            reply,
            at: std::time::Instant::now(),
        };
        agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
        answer
    }

    /// The server says call `id` was answered, as it does once it sent the answer.
    fn answered(agents: &Entity<Agents>, id: u64, cx: &mut TestAppContext) {
        let event = Event::Answered {
            agent: "claude-code".into(),
            id,
            at: std::time::Instant::now(),
            error: None,
            image: None,
        };
        agents.update(cx, |agents, cx| agents.handle(event, cx));
    }

    fn hello(agents: &Entity<Agents>, agent: &str, client: &str, cx: &mut TestAppContext) {
        let event = Event::Hello {
            agent: agent.into(),
            client: client.into(),
        };
        agents.update(cx, |agents, cx| agents.handle(event, cx));
    }

    fn bye(agents: &Entity<Agents>, cx: &mut TestAppContext) {
        let event = Event::Bye {
            agent: "claude-code".into(),
        };
        agents.update(cx, |agents, cx| agents.handle(event, cx));
    }

    fn cards(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<Card> {
        agents.read_with(cx, |agents, cx| agents.shown_cards(cx))
    }

    /// Claude Code connected and said hello, at work in its tab 20.
    fn at_work(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
        let (agents, browser, _) = connected(cx);
        hello(&agents, "claude-code", "claude-code", cx);
        call_as(&agents, 1, "list_tabs", json!({}), cx);
        answered(&agents, 1, cx);
        assert!(cards(&agents, cx).is_empty(), "list_tabs alone isn't work");
        call_as(&agents, 2, "read_page", json!({ "tab": 20 }), cx);
        (agents, browser)
    }

    /// The first call that acts on a tab starts the client's card, with its name, and the calls of
    /// its other connections are steps of the same card. Its conversation is its steps, with no
    /// ask, and the user talks to it where they started it. Its last connection closing ends it,
    /// with the pages it worked in to open, and its next call is a session of its own.
    #[gpui::test]
    fn a_clients_work_is_one_card_until_it_disconnects(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        // A second connection of Claude Code's.
        hello(&agents, "claude-code", "claude-code", cx);
        call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1, "a card for each connection");
        let card = &shown[0];
        assert_eq!(
            (card.name.as_ref(), card.title.as_ref(), card.dot),
            ("Claude Code", "Claude Code", Dot::Working)
        );
        assert!(!card.replies);
        let agent = card.agent.to_string();
        assert!(agent.starts_with("session-"), "{agent}");
        let talk = agents
            .read_with(cx, |agents, cx| agents.conversation(&agent, cx))
            .unwrap();
        assert!(talk.asked.is_empty());
        assert_eq!(talk.lines.len(), 2, "{:?}", talk.lines);
        assert!(
            talk.lines.iter().all(|line| matches!(line, Line::Step(_))),
            "{:?}",
            talk.lines
        );
        assert!(!talk.about.starts_with("Claude Code"), "{}", talk.about);
        // A reply has nowhere to go.
        let replied = agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::Reply("and then?".into()), cx)
        });
        assert_eq!(replied, (None, None));

        answered(&agents, 2, cx);
        answered(&agents, 3, cx);
        bye(&agents, cx);
        let card = cards(&agents, cx).remove(0);
        assert_eq!(card.dot, Dot::Done);
        assert!(card.status.starts_with("Done "), "{}", card.status);
        assert_eq!(card.open, vec![(20, "example.com".into())]);

        hello(&agents, "claude-code", "claude-code", cx);
        call_as(&agents, 4, "read_page", json!({ "tab": 20 }), cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[0].dot, Dot::Working);
    }

    /// A client has one name wherever it shows, its card's (Agents::agent_name): the strip under its
    /// page, from its status, its questions and Settings' activity read the same. Another MCP
    /// client's is the name it gave, or with none its kind's, MCP client, and a session keeps the
    /// name it started with while it works. Claude Code, connected by name, is Claude Code whatever
    /// it calls itself.
    #[gpui::test]
    fn a_client_has_one_name_the_one_it_gave(cx: &mut TestAppContext) {
        let (agents, _, _) = connected(cx);
        agents.update(cx, |agents, cx| agents.connect(Client::Other, cx));
        cx.run_until_parked();
        let named = |agents: &Entity<Agents>, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| {
                let status = agents.status();
                let strip = status.iter().find(|each| each.agent == "mcp-client");
                (
                    agents.agent_name("mcp-client").to_string(),
                    strip.map(|each| each.name.to_string()),
                )
            })
        };
        hello(&agents, "mcp-client", "", cx);
        agents.update(cx, |agents, cx| agents.take_over("mcp-client", 30, cx));
        assert_eq!(
            named(&agents, cx),
            ("MCP client".into(), Some("MCP client".into())),
            "a client that gave no name has its kind's"
        );
        hello(&agents, "mcp-client", "Zed", cx);
        assert_eq!(named(&agents, cx), ("Zed".into(), Some("Zed".into())));

        // Its session's card has that name, and keeps it while it works.
        let (reply, _answer) = mpsc::channel();
        let call = Call {
            id: 1,
            agent: "mcp-client".into(),
            tool: "read_page".into(),
            arguments: json!({ "tab": 20 }),
            reply,
            at: std::time::Instant::now(),
        };
        agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
        assert_eq!(cards(&agents, cx)[0].name, "Zed");
        hello(&agents, "mcp-client", "Cursor", cx);
        assert_eq!(cards(&agents, cx)[0].name, "Zed");
        assert_eq!(named(&agents, cx), ("Zed".into(), Some("Zed".into())));

        hello(&agents, "claude-code", "claude-code", cx);
        let claude = agents.read_with(cx, |agents, _| agents.agent_name("claude-code"));
        assert_eq!(claude, "Claude Code");
    }

    /// A session with no call under way ends once IDLE passes without another, and a call before
    /// then keeps it going.
    #[gpui::test]
    fn a_session_waiting_idle_for_a_call_is_done(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        answered(&agents, 2, cx);
        cx.executor().advance_clock(IDLE - Duration::from_secs(1));
        call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx);
        cx.executor().advance_clock(Duration::from_secs(2));
        assert_eq!(cards(&agents, cx)[0].dot, Dot::Working);
        // Under way for longer than IDLE, it's still at work.
        cx.executor().advance_clock(IDLE);
        assert_eq!(cards(&agents, cx)[0].dot, Dot::Working);
        answered(&agents, 3, cx);
        cx.executor().advance_clock(IDLE + Duration::from_secs(1));
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].dot, Dot::Done);
    }

    /// The card's Stop stops the client on every connection: its calls under way are answered that
    /// the user stopped it, and its connections close, so it shows the browser's server as gone,
    /// with "stopped by the user" in its activity log. Reconnected, its next call is a card of its
    /// own, and ⌘. from its page, which stops the client, stops that card too.
    #[gpui::test]
    fn stop_closes_the_clients_connections_and_its_card(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        agents.update(cx, |agents, cx| agents.set_on(true, cx));
        let (socket, token) = agents.read_with(cx, |agents, _| {
            let tokens = agents.tokens.lock().unwrap();
            let token = tokens.iter().find(|(_, agent)| *agent == "claude-code");
            (agents.socket.clone(), token.unwrap().0.clone())
        });
        // Two connections of Claude Code's, each past its token.
        let open = || {
            let stream = server::connect(&socket).unwrap();
            let mut writer = stream.try_clone().unwrap();
            std::io::Write::write_all(
                &mut writer,
                format!("{}\n", json!({ "token": token })).as_bytes(),
            )
            .unwrap();
            let mut reader = std::io::BufReader::new(stream);
            assert_eq!(
                server::read_line(&mut reader).as_deref(),
                Some(r#"{"ok":true}"#)
            );
            reader
        };
        let mut connections = [open(), open()];
        let waiting = call_as(&agents, 3, "read_page", json!({ "tab": 20 }), cx);
        let agent = cards(&agents, cx)[0].agent.to_string();
        agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::Stop, cx)
        });
        assert_eq!(waiting.recv().unwrap().text, STOPPED);
        assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
        assert_eq!(cards(&agents, cx)[0].dot, Dot::Ended);
        for reader in &mut connections {
            assert_eq!(server::read_line(reader), None, "a connection stayed open");
        }
        assert!(
            log(&agents, "claude-code", cx)
                .iter()
                .any(|entry| entry.0 == "stopped by the user")
        );

        call_as(&agents, 4, "read_page", json!({ "tab": 20 }), cx);
        assert_eq!(cards(&agents, cx)[0].dot, Dot::Working, "a new session");
        agents.update(cx, |agents, cx| agents.stop_agent("claude-code", cx));
        assert!(cards(&agents, cx).iter().all(|card| card.dot == Dot::Ended));
    }

    /// Open moves a finished session's pages into the user's Space, and archiving one that left
    /// pages closes its own and not those of the client's session under way, which share its
    /// Space.
    #[gpui::test]
    fn open_and_archive_take_a_sessions_own_pages(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        bye(&agents, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        let opened = agents.update(cx, |agents, cx| {
            agents.task_action(&first, TaskAction::Open, cx)
        });
        assert_eq!(opened, (Some(20), None));
        let space = |tab: u64| {
            (browser.tabs.borrow().iter())
                .find(|each| each.0 == tab)
                .map(|each| each.1)
        };
        assert_eq!(space(20), Some(1), "Open moved it to the user's Space");

        // A session that opened 30 and is done, beside one at work in 31 of the same Space.
        browser
            .tabs
            .borrow_mut()
            .push((31, 2, "Other", "https://example.org/"));
        hello(&agents, "claude-code", "claude-code", cx);
        let answer = call_as(&agents, 3, "open_tab", json!({ "url": "example.com" }), cx);
        let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
        assert!(opened.text.starts_with("Opened tab 30."), "{}", opened.text);
        answered(&agents, 3, cx);
        bye(&agents, cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        hello(&agents, "claude-code", "claude-code", cx);
        call_as(&agents, 4, "read_page", json!({ "tab": 31 }), cx);
        agents.update(cx, |agents, cx| {
            agents.task_action(&second, TaskAction::Dismiss, cx);
            agents.task_action(&second, TaskAction::Delete, cx);
        });
        let archived = agents.read_with(cx, |agents, cx| agents.archived_tasks(cx));
        assert!(archived.iter().all(|task| task.agent != second.as_str()));
        // Dropped, as the Archive keeps so many, its 30 closes and 31 stays.
        agents.update(cx, |agents, cx| agents.drop_task(&second, cx));
        assert_eq!(space(30), None);
        assert_eq!(space(31), Some(2));
    }
}
