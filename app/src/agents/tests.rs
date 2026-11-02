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

    /// The new tab is 30.
    fn open_tab(&self, agent: &str, url: &str, _: &mut App) -> u64 {
        let space = self.agent_space(agent);
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
        if let Some(app) = app.as_ref().filter(|_| action == PageAction::Key && !pressing_key()) {
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

    // As the core does: only a tab of the user's, into an agent's Space.
    fn give(&self, tab: u64, space: u64, _: &mut App) -> bool {
        let agents = self.agent_spaces.borrow();
        let mut tabs = self.tabs.borrow_mut();
        let Some(each) = tabs.iter_mut().find(|each| each.0 == tab) else {
            return false;
        };
        if agents.values().any(|&agent| agent == each.1) || !agents.values().any(|&s| s == space) {
            return false;
        }
        each.1 = space;
        self.held.borrow_mut().remove(&tab);
        true
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
        agent: agent.into(),
        tool: tool.into(),
        arguments,
        reply,
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
    let bridge = bridge::launch(std::path::Path::new("/app/ilo/mcp-bridge"), Some("app.ilo.browser"));
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
        json!(["run", "--command=/app/ilo/mcp-bridge", "app.ilo.browser", socket.to_str().unwrap()])
    );
    let command = agents.read_with(cx, |agents, _| agents.command());
    assert!(command.starts_with(r#""flatpak" "run" "--command=/app/ilo/mcp-bridge" "app.ilo.browser" ""#), "{command}");
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
    assert_eq!(answer.text, "  20  Example Domain  https://www.example.com/");
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
        let answer = call(&agents, "claude-code", "press_key", json!({ "tab": 20, "key": key }), cx);
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
    let answer = call(&agents, "claude-code", "open_tab", json!({ "url": "example.com" }), cx);
    let (tab, _, action, _, _) = browser.pages.borrow()[0].clone();
    assert_eq!((tab, action), (30, PageAction::Snapshot));
    let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(
        opened.text.starts_with("Opened tab 30.\n\nTab 30: Shop\nURL: https://example.com/"),
        "{}",
        opened.text
    );
    assert!(opened.text.contains("- textbox \"Coupon\" [e1]"));
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("open_tab".into(), "example.com".into(), "".into())
    );

    // Without a tab, the one it opened.
    let answer = call(&agents, "claude-code", "type", json!({ "ref": "e1", "text": "SAVE10" }), cx);
    let (tab, _, action, element, text) = browser.pages.borrow().last().cloned().unwrap();
    assert_eq!((tab, action, element, text.as_str()), (30, PageAction::Type, 31, "SAVE10"));
    let typed = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(typed.text.starts_with("Typed into “Coupon”.\n\nTab 30"));
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "type");

    let answer = call(&agents, "claude-code", "press_key", json!({ "key": "Tab" }), cx);
    assert_eq!(browser.pages.borrow().last().unwrap().4, "Tab");
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    let answer = call(&agents, "claude-code", "screenshot", json!({}), cx);
    let picture = page_done(&agents, &browser, answer, "/9j/4AAQ".into(), cx);
    assert_eq!(
        picture.to_json()["content"][0],
        json!({ "type": "image", "data": "/9j/4AAQ", "mimeType": "image/jpeg" })
    );

    let answer = call(&agents, "claude-code", "navigate", json!({ "url": "back" }), cx);
    assert_eq!(browser.pages.borrow().last().unwrap().2, PageAction::Back);
    assert!(page_done(&agents, &browser, answer, shop_tree(), cx).text.starts_with("Went back."));

    let answer = call(&agents, "claude-code", "navigate", json!({ "url": "iana.org" }), cx);
    assert_eq!(browser.navigated.borrow()[0], (30, "https://iana.org/".into()));
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    let pages = browser.pages.borrow().len();
    let stale = call(&agents, "claude-code", "click", json!({ "ref": "e9" }), cx);
    assert!(stale.recv().unwrap().text.starts_with("There's no element e9 in tab 30."));
    let key = call(&agents, "claude-code", "press_key", json!({ "key": "F5" }), cx);
    assert!(key.recv().unwrap().error);
    assert_eq!(browser.pages.borrow().len(), pages, "a bad call reached the core");

    // The core's error is the call's.
    let answer = call(&agents, "claude-code", "click", json!({ "ref": "e1" }), cx);
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| agents.page_done(request, "It's gone.".into(), true, cx));
    assert_eq!(answer.recv().unwrap(), ToolResult::error("It's gone."));
}

/// An agent can't act on the user's tabs, type into a password field or open a page that isn't
/// on the web; each refusal is logged.
#[gpui::test]
fn what_an_agent_may_not_do_is_refused_and_logged(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "click", json!({ "tab": 10, "ref": "e1" }), cx);
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
    assert!(answer.recv().unwrap().text.starts_with("Password fields are off limits"));
    assert_eq!(browser.pages.borrow().len(), pages, "the password reached the core");
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "type");
    assert!(log(&agents, "claude-code", cx)[0].2.starts_with("Password fields are off limits"));

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
        assert_eq!(answer.recv().unwrap(), ToolResult::error(format!("There's no tool {tool}.")));
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
        let answer = call(&agents, "claude-code", "open_tab", json!({ "url": url }), cx);
        assert_eq!(answer.recv().unwrap(), refused, "{url}");
        let going = json!({ "tab": 20, "url": url });
        let answer = call(&agents, "claude-code", "navigate", going, cx);
        assert_eq!(answer.recv().unwrap(), refused, "{url}");
    }
    assert_eq!(browser.tabs.borrow().len(), tabs, "an agent opened Chrome's password manager");
    assert!(browser.navigated.borrow().is_empty(), "an agent went to Chrome's password manager");
    assert!(browser.pages.borrow().is_empty());
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
        call(&agents, "claude-code", "click", json!({ "tab": 20, "ref": "e3" }), cx)
    };
    let answer = order(cx);
    let question = browser.asked.borrow().clone().expect("the order didn't ask");
    assert_eq!(question.agent, "Claude Code");
    assert_eq!(question.what, "click “Place order” on example.com");
    assert_eq!(question.why, "It looks like a payment or an order.");
    assert_eq!(browser.pages.borrow().len(), pages, "it clicked before the user answered");
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Once, cx));
    assert_eq!(*browser.asked.borrow(), None);
    assert_eq!(browser.pages.borrow().last().unwrap().3, 51);
    let clicked = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(clicked.text.starts_with("Clicked button “Place order”."));

    let answer = order(cx);
    let question = browser.asked.borrow().clone().expect("once let it go again");
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Always, cx));
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
        call(&agents, "claude-code", "click", json!({ "tab": 20, "ref": "e3" }), cx)
    };
    let answer = order(cx);
    let question = browser.asked.borrow().clone().unwrap();
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Always, cx));
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let answer = call(&agents, "claude-code", "open_tab", json!({ "url": "chase.com" }), cx);
    let question = browser.asked.borrow().clone().unwrap();
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Never, cx));
    assert!(answer.recv().unwrap().error);
    let grants = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| agents.grants("claude-code"))
    };
    assert_eq!(grants(cx), [("chase.com".into(), false), ("example.com".into(), true)]);

    agents.update(cx, |agents, cx| agents.revoke("claude-code", Some("example.com"), cx));
    assert_eq!(grants(cx), [("chase.com".into(), false)]);
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("revoked".into(), "example.com".into(), "".into())
    );
    let answer = order(cx);
    let question = browser.asked.borrow().clone().expect("a revoked Always didn't ask again");
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Once, cx));
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    agents.update(cx, |agents, cx| agents.revoke("claude-code", None, cx));
    assert!(grants(cx).is_empty());
    assert_eq!(log(&agents, "claude-code", cx)[0], ("revoked all".into(), "".into(), "".into()));
    let entries = log(&agents, "claude-code", cx).len();
    agents.update(cx, |agents, cx| agents.revoke("claude-code", None, cx));
    assert_eq!(log(&agents, "claude-code", cx).len(), entries, "revoking nothing was logged");
    call(&agents, "claude-code", "open_tab", json!({ "url": "chase.com" }), cx);
    assert!(browser.asked.borrow().is_some(), "a revoked Never didn't ask again");
}

/// A bank's site asks before it opens. No, or the ×, refuses it, and Never refuses it from then
/// on without asking; each is logged.
#[gpui::test]
fn no_to_an_agent_is_kept(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let open = |cx: &mut TestAppContext| {
        call(&agents, "claude-code", "open_tab", json!({ "url": "chase.com" }), cx)
    };
    let answer = open(cx);
    let question = browser.asked.borrow().clone().unwrap();
    assert_eq!(question.what, "open chase.com");
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Dismiss, cx));
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
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Never, cx));
    assert!(answer.recv().unwrap().error);
    let answer = open(cx);
    assert_eq!(*browser.asked.borrow(), None, "never asked again");
    assert!(answer.recv().unwrap().text.starts_with("The user said never on chase.com"));
    assert!(browser.pages.borrow().is_empty());
    assert!(browser.tabs.borrow().iter().all(|tab| tab.0 != 30));
}

/// A question goes with its agent's session, and the card with it.
#[gpui::test]
fn a_question_goes_with_its_agent(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "open_tab", json!({ "url": "chase.com" }), cx);
    assert!(browser.asked.borrow().is_some());
    agents.update(cx, |agents, cx| {
        agents.handle(Event::Hello { agent: "claude-code".into(), client: "check".into() }, cx);
        agents.handle(Event::Bye { agent: "claude-code".into() }, cx);
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
    agents.update(cx, |agents, cx| agents.page_done(request, shop_tree(), false, cx));
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
    let question = browser.asked.borrow().clone().expect("the handoff didn't ask");
    assert_eq!(question.handoff, Some(20));
    assert_eq!(question.why, "Sign in to example.com");
    assert!(browser.held.borrow().contains(&20));
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, [(20, "Sign in to example.com".into())]);
    let refused = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert_eq!(refused.recv().unwrap(), ToolResult::error(HELD));
    assert!(handed.try_recv().is_err(), "the handoff answered before the tab came back");

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

    let order = call(&agents, "claude-code", "click", json!({ "tab": 20, "ref": "e3" }), cx);
    assert!(browser.asked.borrow().is_some());
    agents.update(cx, |agents, cx| agents.stop_agent("claude-code", cx));
    let stopped = order.recv().unwrap();
    assert!(stopped.error && stopped.text.starts_with("The user stopped you."));
    assert_eq!(*browser.asked.borrow(), None);
    assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "stopped");
}

/// A tab the user took over stays theirs: the agent's calls on it are refused until it asks with
/// ask_for_tab and the user hands it back. Keep refuses, and the tab stays the user's.
#[gpui::test]
fn a_tab_the_user_took_back_stays_theirs_until_they_hand_it_back(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    let refused = call(&agents, "claude-code", "click", json!({ "tab": 20, "ref": "e1" }), cx);
    assert!(refused.recv().unwrap().error);
    let other = json!({ "tab": 21, "reason": "To finish the order" });
    let asked = call(&agents, "claude-code", "ask_for_tab", other, cx);
    assert!(asked.recv().unwrap().text.starts_with("The user doesn't have that tab"));
    let asked = call(&agents, "claude-code", "ask_for_tab", json!({ "tab": 20 }), cx);
    assert!(asked.recv().unwrap().text.starts_with("reason is"));

    let reason = json!({ "tab": 20, "reason": "To finish the order" });
    let kept = call(&agents, "claude-code", "ask_for_tab", reason.clone(), cx);
    let question = browser.asked.borrow().clone().expect("asking for the tab didn't ask");
    assert_eq!((question.handoff, question.back), (Some(20), true));
    assert_eq!(question.why, "To finish the order");
    let again = call(&agents, "claude-code", "ask_for_tab", reason.clone(), cx);
    assert!(again.recv().unwrap().text.starts_with("You're waiting on the user"));
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Never, cx));
    let kept = kept.recv().unwrap();
    assert!(kept.error && kept.text.starts_with("The user keeps tab 20."));
    assert_eq!(*browser.asked.borrow(), None);
    assert!(browser.held.borrow().contains(&20), "Keep gave the tab back");
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "kept");

    let back = call(&agents, "claude-code", "ask_for_tab", reason, cx);
    let question = browser.asked.borrow().clone().expect("asking again didn't ask");
    agents.update(cx, |agents, cx| agents.answer(question.id, Answer::Once, cx));
    assert!(back.recv().unwrap().text.starts_with("The user handed tab 20 back."));
    assert!(browser.held.borrow().is_empty());
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
}

/// The user gives the agent a tab of theirs, which it may then read, with an entry in its log.
/// Nothing else moves into its Space: a tab of its own, or one into a Space that isn't an agent's.
#[gpui::test]
fn the_user_gives_an_agent_a_tab_of_theirs(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let read = |agents: &Entity<Agents>, cx: &mut TestAppContext| {
        call(agents, "claude-code", "snapshot", json!({ "tab": 10 }), cx)
    };
    assert!(read(&agents, cx).recv().unwrap().error, "the user's tab was the agent's already");
    agents.update(cx, |agents, cx| {
        agents.give(20, 2, cx);
        agents.give(10, 1, cx);
    });
    assert!(log(&agents, "claude-code", cx).iter().all(|entry| entry.0 != "given"));

    agents.update(cx, |agents, cx| agents.give(10, 2, cx));
    assert_eq!(
        log(&agents, "claude-code", cx)
            .into_iter()
            .find(|entry| entry.0 == "given"),
        Some(("given".into(), "en.wikipedia.org".into(), String::new()))
    );
    let answer = read(&agents, cx);
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
}

/// A page asking for the user's passkey hands its tab to them, as a handoff with no call waiting:
/// no card over Chrome's dialog, the bar says why, the log has it and Hand Back ends it.
#[gpui::test]
fn a_pages_passkey_hands_its_tab_to_the_user(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.session_ended("claude-code", 20, "passkey", cx));
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, vec![(20, SharedString::from(PASSKEY))]);
    assert_eq!(*browser.asked.borrow(), None);
    let entry = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(entry.0, "passkey");
    assert_eq!(tools::describe(&entry.0), "Handed you a tab for your passkey");

    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert!(status.iter().all(|status| status.held.is_empty()));
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "handed back");
}

/// close_tab closes the agent's own tab, which goes to the archive, with its entry in the log.
/// The user's tab, and one the user holds, are refused.
#[gpui::test]
fn an_agent_closes_only_its_own_tabs(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let refused = call(&agents, "claude-code", "close_tab", json!({ "tab": 10 }), cx);
    assert_eq!(refused.recv().unwrap().text, "The tab isn't in the agent's own Space.");
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    let held = call(&agents, "claude-code", "close_tab", json!({ "tab": 20 }), cx);
    assert_eq!(held.recv().unwrap().text, HELD);
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));

    let closed = call(&agents, "claude-code", "close_tab", json!({ "tab": 20 }), cx);
    let closed = closed.recv().unwrap();
    assert!(!closed.error);
    assert_eq!(closed.text, "Closed tab 20. The user can still find it in the archive.");
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
    browser.tabs.borrow_mut().push((21, 2, "Shop", "https://shop.example/"));
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 21, cx));
    let to_take = agents.read_with(cx, |agents, cx| agents.tabs_to_take("claude-code", cx));
    assert_eq!(to_take, [20]);

    agents.update(cx, |agents, cx| agents.take_back_all("claude-code", cx));
    assert_eq!(*browser.held.borrow(), HashSet::from([20, 21]));
    assert!(agents.read_with(cx, |agents, cx| agents.tabs_to_take("claude-code", cx)).is_empty());
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
    assert!(call(&agents, "claude-code", "run_steps", wrong, cx).recv().unwrap().error);

    let steps = json!({ "tab": 20, "steps": [
        { "action": "type", "ref": "e1", "text": "SAVE10" },
        { "action": "scroll" },
    ] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    assert_eq!(browser.pages.borrow().last().unwrap().2, PageAction::Type);
    shop_done(&agents, &browser, cx);
    assert!(run.try_recv().is_err(), "the run answered after its first step");
    assert_eq!(browser.pages.borrow().last().unwrap().2, PageAction::ScrollDown);
    shop_done(&agents, &browser, cx);
    let ran = run.recv().unwrap();
    assert!(!ran.error);
    assert!(ran.text.starts_with("Ran 2 steps.\n1. Typed into “Coupon”.\n2. Scrolled down.\n\n"));
    let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    assert_eq!(entry.action, "run_steps");
    let steps: Vec<&str> = entry.steps.iter().map(|step| step.action.as_ref()).collect();
    assert_eq!(steps, ["type", "scroll"]);

    let order = json!({ "action": "click", "ref": "e3" });
    let steps = json!({ "tab": 20, "steps": [{ "action": "scroll" }, order] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    shop_done(&agents, &browser, cx);
    let stopped = run.recv().unwrap();
    assert!(stopped.error);
    assert!(stopped.text.starts_with("Step 2 would click “Place order” on example.com"));
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
    for wrong in [json!({}), json!({ "seconds": 31 }), json!({ "text": "Thanks", "seconds": 0 })] {
        let mut step = json!({ "action": "wait" });
        step.as_object_mut().unwrap().extend(wrong.as_object().unwrap().clone());
        let steps = json!({ "tab": 20, "steps": [step] });
        let refused = call(&agents, "claude-code", "run_steps", steps, cx).recv().unwrap();
        assert!(refused.text.starts_with("Step 1 failed: wait needs text"), "{}", refused.text);
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
        ran.text.starts_with("Ran 2 steps.\n1. Scrolled down.\n2. “thanks” showed.\n\nTab 20"),
        "{}",
        ran.text
    );
    let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    let steps: Vec<&str> = entry.steps.iter().map(|step| step.action.as_ref()).collect();
    assert_eq!(steps, ["scroll", "wait"]);
    assert_eq!(browser.checked.borrow().last().unwrap(), "snapshot");

    let steps = json!({ "tab": 20, "steps": [{ "action": "wait", "text": "Thanks", "seconds": 1 }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    for _ in 0..2 {
        look(shop_tree(), cx);
        cx.executor().advance_clock(Duration::from_millis(500));
    }
    look(shop_tree(), cx);
    let failed = run.recv().unwrap();
    assert!(failed.error);
    assert!(failed.text.starts_with("Step 1 failed: “Thanks” didn't show in 1 s."), "{}", failed.text);

    let steps = json!({ "tab": 20, "steps": [{ "action": "wait", "seconds": 2 }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    let before = looks();
    cx.executor().advance_clock(Duration::from_millis(1900));
    assert_eq!(looks(), before, "a wait without text looked before its time");
    cx.executor().advance_clock(Duration::from_millis(100));
    look(shop_tree(), cx);
    assert!(run.recv().unwrap().text.starts_with("Ran 1 steps.\n1. Waited 2 s.\n\nTab 20"));
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
    let listed = tools::list();
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
        arguments.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
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
        ("run_steps", tab(json!({ "steps": [{ "action": "scroll" }] }))),
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
            agents.update(cx, |agents, cx| agents.text_read(read.1, Some("Shop".into()), cx));
        }
        cx.run_until_parked();
        _ = answer.try_recv();
    }

    let checked: HashSet<String> = browser.checked.borrow().iter().cloned().collect();
    for action in &checked {
        assert!(CORE_ALLOWS.contains(&action.as_str()), "{action} isn't the core's to allow");
    }
    let page_actions = ["snapshot", "read_page", "click", "type", "press_key", "scroll"];
    for action in page_actions.into_iter().chain(["screenshot", "back", "hand_off", "close_tab"]) {
        assert!(checked.contains(action), "{action} didn't ask the core: {checked:?}");
    }
}
