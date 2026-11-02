//! Agents: Claude Code, Codex or any other MCP client works in the browser, each in a Space of its
//! own, through the browser's MCP server (decision 8). Settings › Agents turns the server on, which
//! it isn't at first, connects the clients and lists what each agent did. The server is a Unix
//! socket in the profile directory, or a named pipe on Windows, which a client reaches through the
//! bridge it launches. Its threads sleep in the kernel between messages, so an idle browser wakes
//! for none of it, and nothing here runs on tab switch, new tab or launch: the saved settings are
//! read after the first window shows.
//!
//! A client proves which agent it is with the token Connect wrote into its config, which its bridge
//! hands the server first. Each tool call reaches the UI thread as an event, and the tools answer
//! through the core, which checks each call on a tab with bc_agent_may_act. That check's answer is
//! the call's activity-log entry. A payment, an order, a message sent or a bank's site asks the
//! user first, in a card in each window: once, always or never on that site, for that agent.
//! Settings › Agents and the agent's Space's menu list what the user answered for good, to revoke.

pub mod bridge;
pub mod clients;
pub mod page;
#[cfg(windows)]
mod pipe;
mod server;
pub mod tools;

pub use server::address;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime};

use futures::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global, SharedString, Task};
use serde_json::{Value, json};

pub use clients::{Client, Clients, Entry, System};

/// The name the clients list the server by: the browser's, a dev build's apart.
pub fn server_name() -> String {
    let name = env!("APP_NAME").to_lowercase();
    match cfg!(feature = "prod") {
        true => name,
        false => format!("{name}-dev"),
    }
}

thread_local! {
    /// An agent's action in a page is under way in the core (`Browser::page`).
    static IN_PAGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a key the core asks the app about is one an agent pressed (press_key), which the core
/// asks about from inside the Agents' call, while their update holds the app. The key goes to the
/// page alone, as an agent never runs ilo's shortcuts, and the app answers without an update.
pub fn pressing_key() -> bool {
    IN_PAGE.get()
}

/// A tab as an agent sees it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabInfo {
    pub id: u64,
    pub title: SharedString,
    pub url: SharedString,
}

/// What the core does in an agent's tab (bc_agent_page).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PageAction {
    Snapshot,
    Click,
    Type,
    TypeSubmit,
    Key,
    ScrollDown,
    ScrollUp,
    Back,
    Forward,
    Screenshot,
}

/// What an agent asks the user's leave for, as the card in each window asks it.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub id: u64,
    /// The agent's name, as its Space has it.
    pub agent: SharedString,
    /// What it would do, following "wants to", such as "click “Place order” on shop.example".
    pub what: SharedString,
    /// Why that asks, such as "It looks like a payment or an order.", or for a handoff, what the
    /// agent needs the user to do.
    pub why: SharedString,
    /// The tab an agent handed to the user (hand_off), whose card shows the tab or hands it back.
    pub handoff: Option<u64>,
    /// Whether the agent asks for that tab back (ask_for_tab), which Keep refuses, rather than
    /// handing it to the user.
    pub back: bool,
}

/// What an agent is doing, and what the user holds from it, for the bar in its Space.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    /// The agent's id.
    pub agent: SharedString,
    pub space: u64,
    /// What it did last, as the log says it, such as "Clicked · example.com".
    pub doing: SharedString,
    /// The tabs the user holds from it, each with what it asked of the user, or empty if the user
    /// took it over.
    pub held: Vec<(u64, SharedString)>,
}

/// The user's answer to an agent's question.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer {
    /// This once.
    Once,
    /// Now and whenever the agent asks on the site again.
    Always,
    /// Not now, nor when it asks on the site again.
    Never,
    /// Not now: the ×.
    Dismiss,
}

/// What the agents ask of the browser: the core, in main.rs, or what a test makes up. Calls into
/// the core take the app, as one may tell the windows something before it returns.
pub trait Browser {
    /// The Space of the agent with the id `agent`, or 0 if it has none.
    fn agent_space(&self, agent: &str) -> u64;
    /// Makes the agent's Space, named as the agent, and returns it.
    fn make_agent_space(&self, agent: &str, name: &str, cx: &mut App) -> u64;
    /// The tabs of `space`, pinned first, then Today's.
    fn space_tabs(&self, space: u64, cx: &App) -> Vec<TabInfo>;
    /// The tabs of `space`, in `space_tabs`' order, without their titles: what a window's menu asks
    /// for while it builds, when the window's titles can't be read.
    fn space_tab_ids(&self, space: u64, cx: &App) -> Vec<u64> {
        self.space_tabs(space, cx).into_iter().map(|tab| tab.id).collect()
    }
    /// The tabs of `agent`'s own Space, in `space_tabs`' order, as the core lists them
    /// (bc_agent_list_tabs): it finds the Space, so no other Space's tabs are ever listed.
    fn agent_tabs(&self, agent: &str, cx: &App) -> Vec<TabInfo>;
    /// `tab`'s title and URL.
    fn tab(&self, tab: u64, cx: &App) -> TabInfo;
    /// Asks the core whether the agent may do `action` to `tab` (bc_agent_may_act): nothing, or
    /// why not.
    fn may_act(&self, agent: &str, tab: u64, action: &str, cx: &mut App) -> Result<(), String>;
    /// Asks the core for the text of `tab`'s page (bc_tab_read_text), which comes to
    /// `Agents::text_read` with `request`.
    fn read_text(&self, tab: u64, request: u64);
    /// Tells the core the agent's client went (bc_agent_disconnected).
    fn disconnected(&self, agent: &str, cx: &mut App);
    /// Opens `url`, http(s) only, in a new tab at the top of the agent's Space, not shown
    /// (bc_agent_open_tab): the tab, or 0 if it couldn't.
    fn open_tab(&self, agent: &str, url: &str, cx: &mut App) -> u64;
    /// Loads `url`, http(s) only, in the agent's `tab` (bc_agent_navigate).
    fn navigate(&self, agent: &str, tab: u64, url: &str, cx: &mut App) -> bool;
    /// Asks the core to do `action` in the agent's `tab` (bc_agent_page), to the element whose
    /// node is `element`, with `text`. Its answer comes to `Agents::page_done` with `request`.
    fn page(
        &self,
        agent: &str,
        tab: u64,
        request: u64,
        action: PageAction,
        element: i64,
        text: &str,
    );
    /// The URL the address bar goes to for `text`: the page it names, else a search for it.
    fn url_for(&self, text: &str) -> Option<String>;
    /// Shows the question for the user to answer, in every window, or takes it away (None).
    fn ask(&self, question: Option<Question>, cx: &mut App);
    /// The agent hands the tab to the user, who holds it from agents until they hand it back
    /// (bc_agent_hand_off). Whether it did.
    fn hand_off(&self, agent: &str, tab: u64) -> bool;
    /// The user takes the tab over, which ends its agent's session there and holds it
    /// (bc_tab_take_back).
    fn take_back(&self, tab: u64, cx: &mut App);
    /// The user hands the tab back to agents. Whether they held it.
    fn hand_back(&self, tab: u64) -> bool;
    /// Closes `tab` as the user would (bc_tab_close): a Today tab goes to the archive.
    fn close_tab(&self, tab: u64, cx: &mut App);
    /// The user gives the agent whose Space is `space` their `tab`, which moves there
    /// (bc_tab_give_to_agent). Whether it did.
    fn give(&self, tab: u64, space: u64, cx: &mut App) -> bool;
}

/// What the server's threads hand the UI thread.
pub enum Event {
    /// A client began a session as the agent with the id `agent`. `client` is the name it gave.
    Hello {
        agent: String,
        client: String,
    },
    /// The agent's last session ended.
    Bye {
        agent: String,
    },
    Call(Call),
}

/// A tool call, which the server's thread waits on until `reply` has the answer.
pub struct Call {
    pub agent: String,
    pub tool: String,
    pub arguments: Value,
    pub reply: mpsc::Sender<ToolResult>,
}

/// A tool's answer.
#[derive(Debug, PartialEq)]
pub struct ToolResult {
    pub text: String,
    pub error: bool,
    /// A picture, as a JPEG in base64.
    pub image: Option<String>,
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        ToolResult {
            text: text.into(),
            error: false,
            image: None,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        ToolResult {
            text: text.into(),
            error: true,
            image: None,
        }
    }

    pub fn image(jpeg: String) -> Self {
        ToolResult {
            text: String::new(),
            error: false,
            image: Some(jpeg),
        }
    }

    pub fn to_json(&self) -> Value {
        match &self.image {
            Some(jpeg) => json!({
                "content": [{ "type": "image", "data": jpeg, "mimeType": "image/jpeg" }],
                "isError": self.error,
            }),
            None => {
                json!({ "content": [{ "type": "text", "text": self.text }], "isError": self.error })
            }
        }
    }
}

/// An agent's action, from its call until the core answers, and through the question it asks the
/// user first if it does.
struct Act {
    agent: String,
    /// What the activity log names it, as `tools::describe` knows it.
    action: &'static str,
    /// The tab, or 0 for open_tab, which makes it.
    tab: u64,
    /// Where open_tab and navigate go.
    url: String,
    page: PageAction,
    element: i64,
    text: String,
    /// What the answer says it did, before the outline.
    note: String,
    /// What it would do, as a question has it.
    what: String,
    reply: mpsc::Sender<ToolResult>,
}

/// An action that waits on the core's answer.
struct Acting {
    agent: String,
    tab: u64,
    /// The element it acts on, or 0.
    element: i64,
    note: String,
    reply: mpsc::Sender<ToolResult>,
}

/// An action that waits on the user.
struct Asking {
    question: Question,
    site: String,
    act: Act,
}

/// An entry of the activity log: what an agent did, or was refused, where and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    pub at: SystemTime,
    /// The agent's id.
    pub agent: SharedString,
    /// What it did, as the core's entry names it, such as "read_page" (`tools::describe`).
    pub action: SharedString,
    /// The host of the page it did it to, or empty.
    pub site: SharedString,
    /// Why it was refused, or empty if it wasn't.
    pub refused: SharedString,
    /// A run's steps (run_steps), each an entry of its own.
    pub steps: Vec<Activity>,
}

/// An agent the user connected.
#[derive(Clone, Debug)]
pub struct Agent {
    pub client: Client,
    token: String,
}

/// The entries the log keeps, all agents' together, as they're for this run of the browser.
const LOG_LENGTH: usize = 1000;

pub struct Agents {
    browser: Rc<dyn Browser>,
    clients: Arc<dyn Clients>,
    /// Where the settings are saved: None in the tests.
    path: Option<PathBuf>,
    /// The server's socket, and what a client runs to reach it, before the socket: the bridge, or
    /// in a Flatpak the command that runs it there (bridge::launch).
    socket: PathBuf,
    bridge: Vec<String>,
    on: bool,
    agents: Vec<Agent>,
    /// The agents' tokens, which the server's threads read.
    tokens: server::Tokens,
    server: Option<(server::Server, Task<()>)>,
    /// Why the server isn't running, though on.
    trouble: Option<SharedString>,
    /// The clients on this computer, as Settings › Agents last looked.
    found: Vec<Client>,
    /// The client whose Connect is writing its config, and the client whose last failed, and why.
    connecting: Option<Client>,
    failed: Option<(Client, SharedString)>,
    /// The agents with a session open.
    live: HashSet<String>,
    log: Vec<Activity>,
    /// The reads waiting on the core, by request.
    reading: HashMap<u64, (String, u64, mpsc::Sender<ToolResult>)>,
    next_request: u64,
    /// The tab each agent used last, which a call without a tab uses.
    current: HashMap<String, u64>,
    /// The actions waiting on the core, by request.
    acting: HashMap<u64, Acting>,
    /// Each tab's refs, from its outlines.
    refs: HashMap<u64, page::Refs>,
    /// The actions waiting on the user, the first asked now.
    asking: Vec<Asking>,
    /// The sites where each agent may (true) or may never (false) do what asks first.
    grants: HashMap<String, HashMap<String, bool>>,
    /// What each agent did last, as the log says it.
    doing: HashMap<String, SharedString>,
    /// The tabs the user holds from agents, by tab.
    held: HashMap<u64, Held>,
    /// The runs of steps under way, each waiting on its step's answer.
    runs: Vec<Run>,
    /// The agent whose run's step is starting, whose log entries go under the run's.
    stepping: Option<String>,
}

/// A run of steps (run_steps): one at a time, each as its tool would run alone.
struct Run {
    agent: String,
    tab: u64,
    /// The steps still to take.
    steps: std::collections::VecDeque<Value>,
    /// What each step taken did.
    done: Vec<String>,
    /// The answer of the step under way.
    step: mpsc::Receiver<ToolResult>,
    reply: mpsc::Sender<ToolResult>,
    /// The step under way, if it's a wait.
    wait: Option<Wait>,
}

/// A run's wait step: it looks at the page until its text shows or its time is up, or without
/// text, looks once its time is up.
struct Wait {
    id: u64,
    /// What it waits to see in the page's outline, or empty.
    text: String,
    seconds: f64,
    until: Instant,
    /// The next look's answer, for the timer to give the core.
    look: Option<mpsc::Sender<ToolResult>>,
    /// The next look, which goes with the run, as a run stopped or gone looks no more.
    timer: Option<Task<()>>,
}

/// How often a wait looks at the page for its text, in a run under way: no more than the page's
/// own settling after an action takes.
const LOOK_EVERY: Duration = Duration::from_millis(500);

/// A tab the user holds from its agent: handed to them, or taken over.
struct Held {
    agent: String,
    /// What the agent asked the user to do, or empty if they took it over.
    why: String,
    /// The handoff's question, or the agent's asking for the tab back, for its card.
    question: u64,
    /// Why the agent asks for the tab back (ask_for_tab), while its call waits for the answer.
    back: Option<String>,
    /// The handoff's or ask_for_tab's call, which waits for the tab to come back, while its
    /// agent's session lasts.
    reply: Option<mpsc::Sender<ToolResult>>,
}

/// The app's agents, once main.rs has made them.
pub struct GlobalAgents(pub Entity<Agents>);

impl Global for GlobalAgents {}

pub fn global(cx: &App) -> Option<Entity<Agents>> {
    cx.try_global::<GlobalAgents>()
        .map(|agents| agents.0.clone())
}

impl Agents {
    /// Off, with none connected, until `load` reads what was saved.
    pub fn new(
        browser: Rc<dyn Browser>,
        clients: Arc<dyn Clients>,
        path: Option<PathBuf>,
        socket: PathBuf,
        bridge: Vec<String>,
    ) -> Self {
        Agents {
            browser,
            clients,
            path,
            socket,
            bridge,
            on: false,
            agents: Vec::new(),
            tokens: Arc::default(),
            server: None,
            trouble: None,
            found: Vec::new(),
            connecting: None,
            failed: None,
            live: HashSet::new(),
            log: Vec::new(),
            reading: HashMap::new(),
            next_request: 1,
            current: HashMap::new(),
            acting: HashMap::new(),
            refs: HashMap::new(),
            asking: Vec::new(),
            grants: HashMap::new(),
            doing: HashMap::new(),
            held: HashMap::new(),
            runs: Vec::new(),
            stepping: None,
        }
    }

    /// Takes what `saved` says, from the settings file, and starts the server if it's on.
    pub fn load(&mut self, saved: &str, cx: &mut Context<Self>) {
        let saved: Value = serde_json::from_str(saved).unwrap_or_default();
        let agents = saved
            .get("agents")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        self.agents = (agents.iter())
            .filter_map(|agent| {
                let client = Client::from_id(agent.get("client")?.as_str()?)?;
                let token = agent.get("token")?.as_str()?.to_string();
                Some(Agent { client, token })
            })
            .collect();
        self.tokens_changed();
        let grants = saved.get("grants").and_then(Value::as_object).into_iter().flatten();
        self.grants = (grants.filter_map(|(agent, sites)| {
            let sites = (sites.as_object()?.iter())
                .filter_map(|(site, grant)| match grant.as_str()? {
                    "always" => Some((site.clone(), true)),
                    "never" => Some((site.clone(), false)),
                    _ => None,
                })
                .collect();
            Some((agent.clone(), sites))
        }))
        .collect();
        self.on = saved.get("on").and_then(Value::as_bool).unwrap_or(false);
        if self.on {
            self.start(cx);
        }
        cx.notify();
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// The switch in Settings › Agents. Off stops the server and ends every agent's session.
    pub fn set_on(&mut self, on: bool, cx: &mut Context<Self>) {
        if on == self.on {
            return;
        }
        self.on = on;
        match on {
            true => self.start(cx),
            false => self.stop(cx),
        }
        self.save();
        cx.notify();
    }

    /// The server as a client's config names it, with the agent's token.
    fn entry(&self, token: &str) -> Entry {
        let mut args = self.bridge.clone();
        args.push(self.socket.to_string_lossy().into_owned());
        let command = args.remove(0);
        Entry {
            name: server_name(),
            command,
            args,
            token: token.into(),
        }
    }

    /// What a client runs to reach the server: the bridge, with the socket.
    pub fn command(&self) -> String {
        let entry = self.entry("");
        std::iter::once(&entry.command)
            .chain(&entry.args)
            .map(|word| format!("\"{word}\""))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Why the server isn't running, though on.
    pub fn trouble(&self) -> Option<SharedString> {
        self.trouble.clone()
    }

    /// Looks again for the clients on this computer, as Settings › Agents shows: one installed
    /// since the last look is listed.
    pub fn look_for_clients(&mut self, cx: &mut Context<Self>) {
        let found: Vec<Client> =
            (Client::ALL.into_iter()).filter(|client| self.clients.found(*client)).collect();
        if found != self.found {
            self.found = found;
            cx.notify();
        }
    }

    /// The clients Settings › Agents lists: those found here, any connected, whose Disconnect
    /// stays though the client has gone, and any other MCP client.
    pub fn listed(&self) -> Vec<Client> {
        (Client::ALL.into_iter())
            .filter(|client| {
                *client == Client::Other || self.found.contains(client) || self.is_connected(*client)
            })
            .collect()
    }

    pub fn is_connected(&self, client: Client) -> bool {
        self.agents.iter().any(|agent| agent.client == client)
    }

    /// Whether the client's Connect is still writing its config.
    pub fn is_connecting(&self, client: Client) -> bool {
        self.connecting == Some(client)
    }

    /// Why the client's last Connect or Disconnect failed.
    pub fn failure(&self, client: Client) -> Option<SharedString> {
        self.failed
            .as_ref()
            .filter(|(each, _)| *each == client)
            .map(|(_, why)| why.clone())
    }

    /// Whether the agent has a session open now.
    pub fn is_live(&self, client: Client) -> bool {
        self.live.contains(client.id())
    }

    /// Makes the client the agent's token, and writes it into the client's config, with the
    /// bridge that reaches the server. A client connected before gets a new token, and the old one stops
    /// working.
    pub fn connect(&mut self, client: Client, cx: &mut Context<Self>) {
        if self.connecting.is_some() {
            return;
        }
        let token = random_token();
        let entry = self.entry(&token);
        self.connecting = Some(client);
        self.failed = None;
        cx.notify();
        let clients = self.clients.clone();
        cx.spawn(async move |agents, cx| {
            let done = cx
                .background_spawn(async move { clients.connect(client, &entry) })
                .await;
            agents
                .update(cx, |agents, cx| agents.connected(client, token, done, cx))
                .ok();
        })
        .detach();
    }

    fn connected(
        &mut self,
        client: Client,
        token: String,
        done: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        self.connecting = None;
        match done {
            Ok(()) => {
                // check.sh reads this line.
                log!("browser: agent {} connected", client.id());
                self.agents.retain(|agent| agent.client != client);
                self.agents.push(Agent { client, token });
                self.tokens_changed();
                self.save();
                self.note(client.id(), "connected", "", "");
            }
            Err(why) => {
                log!("browser: agent {} couldn't connect: {why}", client.id());
                self.failed = Some((client, why.into()));
            }
        }
        cx.notify();
    }

    /// Takes the server out of the client's config, and its token from the agent, which ends its
    /// sessions. Its Space stays the user's to keep or delete.
    pub fn disconnect(&mut self, client: Client, cx: &mut Context<Self>) {
        if !self.is_connected(client) {
            return;
        }
        // check.sh reads this line.
        log!("browser: agent {} disconnected", client.id());
        self.agents.retain(|agent| agent.client != client);
        self.tokens_changed();
        self.save();
        self.reading.retain(|_, (agent, _, _)| agent != client.id());
        self.forget_waiting(client.id(), cx);
        if let Some((server, _)) = &self.server {
            server.disconnect(client.id());
        }
        if self.live.remove(client.id()) {
            self.browser.disconnected(client.id(), cx);
        }
        self.note(client.id(), "disconnected", "", "");
        self.failed = None;
        let clients = self.clients.clone();
        let name = server_name();
        cx.spawn(async move |agents, cx| {
            let done = cx
                .background_spawn(async move { clients.disconnect(client, &name) })
                .await;
            if let Err(why) = done {
                log!("browser: agent {} config kept: {why}", client.id());
                agents
                    .update(cx, |agents, cx| {
                        agents.failed = Some((client, why.into()));
                        cx.notify();
                    })
                    .ok();
            }
        })
        .detach();
        cx.notify();
    }

    /// What an MCP client's config needs, for the user to paste: the bridge, its socket and the
    /// agent's token, as most clients write a stdio server.
    pub fn setup(&self, client: Client) -> Option<String> {
        let agent = self.agents.iter().find(|agent| agent.client == client)?;
        let config = json!({ "mcpServers": { server_name(): self.entry(&agent.token).json() } });
        serde_json::to_string_pretty(&config).ok()
    }

    /// Every agent's activity, newest first.
    pub fn recent(&self) -> impl Iterator<Item = &Activity> {
        self.log.iter().rev()
    }

    /// The agent's activity, newest first.
    pub fn activity(&self, agent: &str) -> impl Iterator<Item = &Activity> {
        self.log
            .iter()
            .rev()
            .filter(move |entry| entry.agent == agent)
    }

    /// The core checked a call of an agent's on `tab` not made here, as the driver's: an entry
    /// for the log. The calls made here log their own, as the core's report comes while they
    /// run.
    pub fn checked(
        &mut self,
        agent: &str,
        tab: u64,
        action: &str,
        refused: &str,
        cx: &mut Context<Self>,
    ) {
        let site = site(&self.browser.tab(tab, cx).url);
        self.note(agent, action, &site, refused);
        cx.notify();
    }

    /// The core ended the agent's session with `tab`: it let go of the tab, or the user took it
    /// back. A disconnect has its own entry.
    pub fn session_ended(&mut self, agent: &str, tab: u64, why: &str, cx: &mut Context<Self>) {
        // The page asked for the user's passkey: a handoff the page made, as no agent may pick
        // one, which the core holds the tab for.
        if why == "passkey" && !self.held.contains_key(&tab) {
            // check.sh reads this line.
            log!("browser: agent {agent} hands off {tab}: passkey");
            let question = self.next_request;
            self.next_request += 1;
            let why = PASSKEY.to_string();
            let held = Held { agent: agent.into(), why, question, back: None, reply: None };
            self.held.insert(tab, held);
        }
        if why != "disconnected" {
            let site = site(&self.browser.tab(tab, cx).url);
            self.note(agent, why, &site, "");
            cx.notify();
        }
    }

    /// The core's answer to the read `request`: the page's text, or None if it had none.
    pub fn text_read(&mut self, request: u64, text: Option<String>, cx: &mut Context<Self>) {
        let Some((_, tab, reply)) = self.reading.remove(&request) else {
            return;
        };
        let tab = self.browser.tab(tab, cx);
        _ = reply.send(match text {
            Some(text) => ToolResult::text(tools::page_text(&tab, &text)),
            None => ToolResult::error(
                "The tab has no page loaded: it loads once the user shows it. Try another tab.",
            ),
        });
    }

    /// What the server's threads hand over.
    pub fn handle(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Hello { agent, client } => {
                log!("browser: agent {agent} hello from {client}");
                if self.live.insert(agent.clone()) {
                    self.note(&agent, "connected", "", "");
                    cx.notify();
                }
            }
            Event::Bye { agent } => {
                if self.live.remove(&agent) {
                    self.forget_waiting(&agent, cx);
                    self.browser.disconnected(&agent, cx);
                    self.note(&agent, "disconnected", "", "");
                    cx.notify();
                }
            }
            Event::Call(call) => self.call(call, cx),
        }
    }

    fn call(&mut self, call: Call, cx: &mut Context<Self>) {
        let Call {
            agent,
            tool,
            arguments,
            reply,
        } = call;
        // A token's agent is always one the user connected.
        let Some(client) = Client::from_id(&agent).filter(|client| self.is_connected(*client))
        else {
            _ = reply.send(ToolResult::error("This agent isn't connected."));
            return;
        };
        // check.sh reads this line.
        log!("browser: agent {agent} calls {tool}");
        self.live.insert(agent.clone());
        match tool.as_str() {
            "list_tabs" => {
                // Its first call makes its Space, which the core then lists.
                self.space(client, cx);
                let tabs = self.browser.agent_tabs(&agent, cx);
                self.note(&agent, "list_tabs", "", "");
                cx.notify();
                _ = reply.send(ToolResult::text(tools::tab_list(
                    &tabs,
                    self.current.get(&agent).copied(),
                )));
            }
            "read_page" => {
                let tab = match tools::tab_argument(&arguments) {
                    Ok(Some(tab)) => tab,
                    Ok(None) => match self.default_tab(client, cx) {
                        Some(tab) => tab,
                        None => {
                            _ = reply.send(ToolResult::text(tools::tab_list(&[], None)));
                            return;
                        }
                    },
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                if !self.act(&agent, tab, "read_page", &reply, cx) {
                    return;
                }
                let request = self.next_request;
                self.next_request += 1;
                self.reading.insert(request, (agent, tab, reply));
                self.browser.read_text(tab, request);
            }
            "open_tab" => {
                let Some(url) = self.url_argument(&arguments, &reply) else {
                    return;
                };
                let host = site(&url);
                let act = Act {
                    agent: agent.clone(),
                    action: "open_tab",
                    tab: 0,
                    what: format!("open {host}"),
                    url,
                    page: PageAction::Snapshot,
                    element: 0,
                    text: String::new(),
                    note: String::new(),
                    reply,
                };
                if !is_web(&act.url) {
                    return self.refuse(act, &host, WEB_ONLY, cx);
                }
                let risk = page::opening_risk(&host);
                self.check(act, host, risk, cx);
            }
            "navigate" | "snapshot" | "click" | "type" | "press_key" | "scroll" | "screenshot" => {
                let tab = match tools::tab_argument(&arguments) {
                    Ok(Some(tab)) => tab,
                    Ok(None) => match self.default_tab(client, cx) {
                        Some(tab) => tab,
                        None => {
                            _ = reply.send(ToolResult::error(
                                "You have no tab open yet. Open one with open_tab.",
                            ));
                            return;
                        }
                    },
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                self.page_tool(&agent, &tool, tab, &arguments, reply, cx);
            }
            "run_steps" => {
                let Some(tab) = self.tab_for(client, &arguments, &reply, cx) else {
                    return;
                };
                let steps = arguments.get("steps").and_then(Value::as_array).cloned();
                let steps = steps.unwrap_or_default();
                let wrong = steps.iter().position(|step| {
                    let action = step.get("action").and_then(Value::as_str);
                    !action.is_some_and(|action| tools::STEPS.contains(&action))
                });
                if steps.is_empty() || steps.len() > tools::MAX_STEPS || wrong.is_some() {
                    _ = reply.send(ToolResult::error(format!(
                        "steps is 1 to {} steps, each with an action: {}.",
                        tools::MAX_STEPS,
                        tools::STEPS.join(", ")
                    )));
                    return;
                }
                let host = site(&self.browser.tab(tab, cx).url);
                self.note(&agent, "run_steps", &host, "");
                let (_, step) = mpsc::channel();
                let steps = steps.into();
                let run = Run { agent, tab, steps, done: Vec::new(), step, reply, wait: None };
                self.next_step(run, cx);
                self.advance(cx);
            }
            "hand_off" => {
                let Some(tab) = self.tab_for(client, &arguments, &reply, cx) else {
                    return;
                };
                let why = arguments.get("reason").and_then(Value::as_str).unwrap_or("").trim();
                if why.is_empty() {
                    _ = reply.send(ToolResult::error(
                        "reason is what the user is to do, in a short sentence.",
                    ));
                    return;
                }
                if !self.act(&agent, tab, "hand_off", &reply, cx) {
                    return;
                }
                if !self.browser.hand_off(&agent, tab) {
                    _ = reply.send(ToolResult::error("The tab is the user's already."));
                    return;
                }
                // check.sh reads this line.
                log!("browser: agent {agent} hands off {tab}: {why}");
                let question = self.next_request;
                self.next_request += 1;
                let why = page::clip(why, 200);
                let held = Held { agent, why, question, back: None, reply: Some(reply) };
                self.held.insert(tab, held);
                self.asked(cx);
            }
            "close_tab" => {
                let Some(tab) = self.tab_for(client, &arguments, &reply, cx) else {
                    return;
                };
                if !self.act(&agent, tab, "close_tab", &reply, cx) {
                    return;
                }
                self.browser.close_tab(tab, cx);
                let space = self.browser.agent_space(&agent);
                let open = (self.browser.space_tabs(space, cx).iter()).any(|each| each.id == tab);
                _ = reply.send(ToolResult::text(match open {
                    true => format!(
                        "Tab {tab} stays open: the user pinned it, or its page asks them before \
                         it's left."
                    ),
                    false => {
                        format!("Closed tab {tab}. The user can still find it in the archive.")
                    }
                }));
            }
            "ask_for_tab" => {
                let Some(tab) = self.tab_for(client, &arguments, &reply, cx) else {
                    return;
                };
                let why = arguments.get("reason").and_then(Value::as_str).unwrap_or("").trim();
                if why.is_empty() {
                    _ = reply.send(ToolResult::error(
                        "reason is why you need the tab, in a short sentence.",
                    ));
                    return;
                }
                let site = site(&self.browser.tab(tab, cx).url);
                let refused = match self.held.get(&tab) {
                    Some(held) if held.agent != agent => NOT_HELD,
                    None => NOT_HELD,
                    Some(held) if held.reply.is_some() => {
                        "You're waiting on the user for this tab already."
                    }
                    Some(_) => "",
                };
                self.note(&agent, "ask_for_tab", &site, refused);
                if !refused.is_empty() {
                    _ = reply.send(ToolResult::error(refused));
                    return;
                }
                // check.sh reads this line.
                log!("browser: agent {agent} asks for {tab} back: {why}");
                let question = self.next_request;
                self.next_request += 1;
                let held = self.held.get_mut(&tab).expect("held, as matched");
                (held.question, held.back) = (question, Some(page::clip(why, 200)));
                held.reply = Some(reply);
                self.asked(cx);
            }
            _ => _ = reply.send(ToolResult::error(format!("There's no tool {tool}."))),
        }
    }

    /// Starts the run's next step, which answers it through `advance`, or answers the run if the
    /// step asks the user first, which a run doesn't wait for.
    fn next_step(&mut self, mut run: Run, cx: &mut Context<Self>) {
        let Some(step) = run.steps.pop_front() else {
            return;
        };
        let tool = step.get("action").and_then(Value::as_str).unwrap_or_default().to_string();
        let (reply, answer) = mpsc::channel();
        run.step = answer;
        if tool == "wait" {
            return self.wait(run, &step, reply, cx);
        }
        let asked = self.asking.len();
        self.stepping = Some(run.agent.clone());
        self.page_tool(&run.agent.clone(), &tool, run.tab, &step, reply, cx);
        self.stepping = None;
        if self.asking.len() > asked {
            let Asking { act, .. } = self.asking.pop().unwrap();
            self.asked(cx);
            let n = run.done.len() + 1;
            let answer = format!(
                "Step {n} would {}, which asks the user first, so the run stopped there. Do it on \
                 its own, as {tool}, for the user to answer.{}",
                act.what,
                run_so_far(&run.done)
            );
            _ = run.reply.send(ToolResult::error(answer));
            return;
        }
        self.runs.push(run);
    }

    /// Starts a wait step: checked as a snapshot, which each of its looks is, but logged as itself.
    fn wait(
        &mut self,
        mut run: Run,
        step: &Value,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let text = step.get("text").and_then(Value::as_str).unwrap_or("").trim().to_string();
        let seconds = match step.get("seconds").and_then(Value::as_f64) {
            None if !text.is_empty() => Some(tools::WAIT),
            seconds => seconds.filter(|seconds| *seconds > 0.0 && *seconds <= tools::MAX_WAIT),
        };
        let Some(seconds) = seconds else {
            _ = reply.send(ToolResult::error(format!(
                "wait needs text to wait for, or seconds, at most {}.",
                tools::MAX_WAIT
            )));
            return self.runs.push(run);
        };
        self.stepping = Some(run.agent.clone());
        let may = self.act_as(&run.agent, run.tab, "snapshot", "wait", &reply, cx);
        self.stepping = None;
        if may {
            let id = self.next_request;
            self.next_request += 1;
            let time = Duration::from_secs_f64(seconds);
            let until = cx.background_executor().now() + time;
            let mut wait = Wait { id, text, seconds, until, look: None, timer: None };
            match wait.text.is_empty() {
                true => (wait.look, wait.timer) = (Some(reply), Some(self.look_after(id, time, cx))),
                false => self.look(&run.agent, run.tab, reply),
            }
            run.wait = Some(wait);
        }
        self.runs.push(run);
    }

    /// The wait's answer, now that it has looked at the page, or None if it looks again.
    fn waited(
        &mut self,
        run: &mut Run,
        mut wait: Wait,
        answer: ToolResult,
        cx: &mut Context<Self>,
    ) -> Option<ToolResult> {
        let outline = answer.text;
        let seconds = wait.seconds;
        if wait.text.is_empty() {
            return Some(ToolResult::text(format!("Waited {seconds} s.\n\n{outline}")));
        }
        let text = page::clip(&wait.text, 60);
        if outline.to_lowercase().contains(&wait.text.to_lowercase()) {
            return Some(ToolResult::text(format!("“{text}” showed.\n\n{outline}")));
        }
        let now = cx.background_executor().now();
        if now >= wait.until {
            let why = format!("“{text}” didn't show in {seconds} s.\n\n{outline}");
            return Some(ToolResult::error(why));
        }
        let (look, answer) = mpsc::channel();
        run.step = answer;
        wait.look = Some(look);
        wait.timer = Some(self.look_after(wait.id, LOOK_EVERY.min(wait.until - now), cx));
        run.wait = Some(wait);
        None
    }

    /// Looks at the page again for the wait `id` after `after`, if its run is still under way.
    fn look_after(&self, id: u64, after: Duration, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |agents, cx| {
            cx.background_executor().timer(after).await;
            _ = agents.update(cx, |agents, _| {
                let waiting = agents.runs.iter_mut().find_map(|run| {
                    let wait = run.wait.as_mut().filter(|wait| wait.id == id)?;
                    Some((run.agent.clone(), run.tab, wait.look.take()?))
                });
                if let Some((agent, tab, look)) = waiting {
                    agents.look(&agent, tab, look);
                }
            });
        })
    }

    /// Asks the core for the page's tree, for a wait, which checked and logged itself.
    fn look(&mut self, agent: &str, tab: u64, reply: mpsc::Sender<ToolResult>) {
        let request = self.next_request;
        self.next_request += 1;
        let note = String::new();
        let acting = Acting { agent: agent.into(), tab, element: 0, note, reply };
        self.acting.insert(request, acting);
        self.act_in_page(agent, tab, request, PageAction::Snapshot, 0, "");
    }

    /// Asks the core to act in the page, which can ask the app about a key from inside the call
    /// (`pressing_key`).
    fn act_in_page(
        &self,
        agent: &str,
        tab: u64,
        request: u64,
        action: PageAction,
        element: i64,
        text: &str,
    ) {
        IN_PAGE.set(true);
        self.browser.page(agent, tab, request, action, element, text);
        IN_PAGE.set(false);
    }

    /// Takes each run's step that has answered: the next step, or the run's answer.
    fn advance(&mut self, cx: &mut Context<Self>) {
        let mut at = 0;
        while at < self.runs.len() {
            match self.runs[at].step.try_recv() {
                Err(mpsc::TryRecvError::Empty) => at += 1,
                // Its agent went, and the run's call with it.
                Err(mpsc::TryRecvError::Disconnected) => _ = self.runs.remove(at),
                Ok(answer) => {
                    let mut run = self.runs.remove(at);
                    let answer = match run.wait.take() {
                        Some(wait) if !answer.error => match self.waited(&mut run, wait, answer, cx) {
                            Some(answer) => answer,
                            None => {
                                self.runs.push(run);
                                continue;
                            }
                        },
                        _ => answer,
                    };
                    let n = run.done.len() + 1;
                    if answer.error {
                        let so_far = run_so_far(&run.done);
                        let why = format!("Step {n} failed: {}{so_far}", answer.text);
                        _ = run.reply.send(ToolResult::error(why));
                    } else if run.steps.is_empty() {
                        let done = run.done.iter().map(|note| format!("{note}\n"));
                        let done: String = done.collect();
                        let text = format!("Ran {n} steps.\n{done}{n}. {}", answer.text);
                        _ = run.reply.send(ToolResult::text(text));
                    } else {
                        let note = answer.text.split("\n\n").next().unwrap_or_default();
                        run.done.push(format!("{n}. {note}"));
                        self.next_step(run, cx);
                    }
                    at = 0;
                }
            }
        }
    }

    /// The tab a call names, else the one its agent used last; or None, having answered why.
    fn tab_for(
        &mut self,
        client: Client,
        arguments: &Value,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let why = match tools::tab_argument(arguments) {
            Ok(Some(tab)) => return Some(tab),
            Ok(None) => match self.default_tab(client, cx) {
                Some(tab) => return Some(tab),
                None => "You have no tab open yet. Open one with open_tab.".into(),
            },
            Err(why) => why,
        };
        _ = reply.send(ToolResult::error(why));
        None
    }

    /// The user takes the tab the agent is at work in over: the agent may not act in it until
    /// they hand it back.
    pub fn take_over(&mut self, agent: &str, tab: u64, cx: &mut Context<Self>) {
        if self.held.contains_key(&tab) {
            return;
        }
        // check.sh reads this line.
        log!("browser: agent {agent} taken over in {tab}");
        let question = self.next_request;
        self.next_request += 1;
        let why = String::new();
        let held = Held { agent: agent.into(), why, question, back: None, reply: None };
        self.held.insert(tab, held);
        // The core ends the agent's session there, which logs it.
        self.browser.take_back(tab, cx);
        cx.notify();
    }

    /// The user gives the agent whose Space is `space` their `tab`, which it may act in from then
    /// on, as in any tab of its own. Moving the tab to a Space of the user's takes it back.
    pub fn give(&mut self, tab: u64, space: u64, cx: &mut Context<Self>) {
        let agent = (Client::ALL.into_iter().map(Client::id))
            .find(|agent| self.browser.agent_space(agent) == space);
        let Some(agent) = agent else {
            return;
        };
        if !self.browser.give(tab, space, cx) {
            return;
        }
        // A hold from before is over, as the core's is.
        if let Some(reply) = self.held.remove(&tab).and_then(|held| held.reply) {
            _ = reply.send(ToolResult::text(format!("The user gave you tab {tab}.")));
        }
        // check.sh reads this line.
        log!("browser: gave {tab} to agent {agent}");
        let site = site(&self.browser.tab(tab, cx).url);
        self.note(agent, "given", &site, "");
        cx.notify();
    }

    /// The tabs of the agent's Space the user doesn't hold.
    pub fn tabs_to_take(&self, agent: &str, cx: &App) -> Vec<u64> {
        let space = self.browser.agent_space(agent);
        if space == 0 {
            return Vec::new();
        }
        let tabs = self.browser.space_tab_ids(space, cx).into_iter();
        tabs.filter(|tab| !self.held.contains_key(tab)).collect()
    }

    /// The user takes each tab of the agent's Space they don't hold over, from its Space's menu.
    pub fn take_back_all(&mut self, agent: &str, cx: &mut Context<Self>) {
        for tab in self.tabs_to_take(agent, cx) {
            self.take_over(agent, tab, cx);
        }
    }

    /// The user hands the tab back to its agent, whose handoff, if it waits, carries on.
    pub fn hand_back(&mut self, tab: u64, cx: &mut Context<Self>) {
        let held = self.held.remove(&tab);
        if !self.browser.hand_back(tab) && held.is_none() {
            return;
        }
        let Some(Held { agent, reply, .. }) = held else {
            return cx.notify();
        };
        // check.sh reads this line.
        log!("browser: agent {agent} handed back {tab}");
        let site = site(&self.browser.tab(tab, cx).url);
        self.note(&agent, "handed back", &site, "");
        if let Some(reply) = reply {
            _ = reply.send(ToolResult::text(format!(
                "The user handed tab {tab} back. Take a snapshot to see where it is now."
            )));
        }
        self.asked(cx);
    }

    /// The user stops the agent: what it waits on is refused, its questions go and its sessions
    /// end. The tabs the user holds stay theirs.
    pub fn stop_agent(&mut self, agent: &str, cx: &mut Context<Self>) {
        // check.sh reads this line.
        log!("browser: agent {agent} stopped");
        let stopped = || ToolResult::error(STOPPED);
        for (_, (_, _, reply)) in self.reading.extract_if(|_, (each, _, _)| each == agent) {
            _ = reply.send(stopped());
        }
        for (_, acting) in self.acting.extract_if(|_, acting| acting.agent == agent) {
            _ = acting.reply.send(stopped());
        }
        for asking in self.asking.extract_if(.., |asking| asking.act.agent == agent) {
            _ = asking.act.reply.send(stopped());
        }
        for held in self.held.values_mut().filter(|held| held.agent == agent) {
            held.back = None;
            if let Some(reply) = held.reply.take() {
                _ = reply.send(stopped());
            }
        }
        self.browser.disconnected(agent, cx);
        self.note(agent, "stopped", "", "");
        self.advance(cx);
        self.asked(cx);
    }

    /// What each agent did last, and the tabs the user holds from it, for the bar in its Space.
    pub fn status(&self) -> Vec<Status> {
        let mut agents: Vec<&str> = self.doing.keys().map(String::as_str).collect();
        agents.extend(self.held.values().map(|held| held.agent.as_str()));
        agents.sort();
        agents.dedup();
        (agents.into_iter())
            .map(|agent| {
                let mut held: Vec<(u64, SharedString)> = (self.held.iter())
                    .filter(|(_, held)| held.agent == agent)
                    .map(|(&tab, held)| (tab, held.why.clone().into()))
                    .collect();
                held.sort();
                Status {
                    agent: agent.to_string().into(),
                    space: self.browser.agent_space(agent),
                    doing: self.doing.get(agent).cloned().unwrap_or_default(),
                    held,
                }
            })
            .collect()
    }

    /// A tool that acts in a tab or looks at it: what it asks of the core, and whether it asks the
    /// user first.
    fn page_tool(
        &mut self,
        agent: &str,
        tool: &str,
        tab: u64,
        arguments: &Value,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let host = site(&self.browser.tab(tab, cx).url);
        let string = |name: &str| arguments.get(name).and_then(Value::as_str).map(str::to_string);
        let element = match string("ref") {
            Some(name) => match self.refs.get(&tab).and_then(|refs| refs.get(&name)) {
                Some(element) => Some(element.clone()),
                None => {
                    _ = reply.send(ToolResult::error(format!(
                        "There's no element {name} in tab {tab}. Use a ref from its latest outline."
                    )));
                    return;
                }
            },
            None => None,
        };
        let label = |element: &page::Element| match element.name.is_empty() {
            true => element.role.clone(),
            false => format!("“{}”", page::clip(&element.name, 60)),
        };
        let mut act = Act {
            agent: agent.into(),
            action: "snapshot",
            tab,
            url: String::new(),
            page: PageAction::Snapshot,
            element: element.as_ref().map_or(0, |element| element.node),
            text: String::new(),
            note: String::new(),
            what: String::new(),
            reply,
        };
        let mut risk = None;
        match tool {
            "navigate" => {
                let url = string("url").unwrap_or_default();
                match url.trim() {
                    "back" => {
                        (act.action, act.page, act.note) =
                            ("back", PageAction::Back, "Went back.".into())
                    }
                    "forward" => {
                        (act.action, act.page, act.note) =
                            ("forward", PageAction::Forward, "Went forward.".into())
                    }
                    _ => {
                        let Some(url) = self.url_argument(arguments, &act.reply) else {
                            return;
                        };
                        let to = site(&url);
                        act.action = "navigate";
                        act.what = format!("open {to}");
                        act.url = url;
                        if !is_web(&act.url) {
                            return self.refuse(act, &to, WEB_ONLY, cx);
                        }
                        // What asks is the site it goes to.
                        return self.check(act, to.clone(), page::opening_risk(&to), cx);
                    }
                }
            }
            "snapshot" => {}
            "screenshot" => (act.action, act.page) = ("screenshot", PageAction::Screenshot),
            "click" => {
                let Some(element) = &element else {
                    _ = act.reply.send(ToolResult::error("click needs a ref."));
                    return;
                };
                act.action = "click";
                act.page = PageAction::Click;
                act.note = format!("Clicked {} {}.", element.role, label(element));
                act.what = format!("click {} on {host}", label(element));
                risk = page::risk(&host, &element.name);
            }
            "type" => {
                let (Some(element), Some(text)) = (&element, string("text")) else {
                    _ = act.reply.send(ToolResult::error("type needs a ref and text."));
                    return;
                };
                act.action = "type";
                if element.password {
                    return self.refuse(act, &host, PASSWORDS, cx);
                }
                let submit = arguments.get("submit").and_then(Value::as_bool).unwrap_or(false);
                act.page = if submit { PageAction::TypeSubmit } else { PageAction::Type };
                act.note = format!(
                    "Typed into {}{}.",
                    label(element),
                    if submit { " and pressed Enter" } else { "" }
                );
                act.what = format!("send “{}” on {host}", page::clip(&text, 60));
                act.text = text;
                risk = submit.then(|| page::risk(&host, "send")).flatten();
            }
            "press_key" => {
                let key = string("key").unwrap_or_default();
                if !page::KEYS.contains(&key.as_str()) {
                    let keys = page::KEYS.join(", ");
                    _ = act.reply.send(ToolResult::error(format!("key is one of {keys}.")));
                    return;
                }
                act.action = "press_key";
                act.page = PageAction::Key;
                act.note = format!("Pressed {key}.");
                act.what = format!("press Enter to send on {host}");
                risk = (key == "Enter").then(|| page::risk(&host, "send")).flatten();
                act.text = key;
            }
            _ => {
                let up = string("direction").as_deref() == Some("up");
                act.action = "scroll";
                act.page = if up { PageAction::ScrollUp } else { PageAction::ScrollDown };
                act.note = match &element {
                    Some(element) => format!("Scrolled to {}.", label(element)),
                    None => format!("Scrolled {}.", if up { "up" } else { "down" }),
                };
            }
        }
        self.check(act, host, risk, cx);
    }

    /// The `url` argument as the address bar takes it, or None once the call has its error.
    fn url_argument(&self, arguments: &Value, reply: &mpsc::Sender<ToolResult>) -> Option<String> {
        let typed = arguments.get("url").and_then(Value::as_str).unwrap_or_default();
        let url = (!typed.trim().is_empty()).then(|| self.browser.url_for(typed)).flatten();
        if url.is_none() {
            _ = reply.send(ToolResult::error("url is a URL, or words to search for."));
        }
        url
    }

    /// Runs the action, unless what it does on `site` asks the user first (`why`): then it waits
    /// for the user's answer, or the one they gave for good.
    fn check(&mut self, act: Act, site: String, why: Option<&'static str>, cx: &mut Context<Self>) {
        let Some(why) = why else {
            return self.run(act, cx);
        };
        match self.grants.get(&act.agent).and_then(|sites| sites.get(&site)) {
            Some(true) => return self.run(act, cx),
            Some(false) => return self.refuse(act, &site, SAID_NEVER, cx),
            None => {}
        }
        // A tab not the agent's is the core's to refuse, without a question.
        let space = self.browser.agent_space(&act.agent);
        if act.tab != 0 && !self.browser.space_tabs(space, cx).iter().any(|tab| tab.id == act.tab) {
            return self.run(act, cx);
        }
        let question = Question {
            id: self.next_request,
            agent: name(&act.agent).into(),
            what: act.what.clone().into(),
            why: why.into(),
            handoff: None,
            back: false,
        };
        self.next_request += 1;
        // check.sh reads this line.
        log!("browser: agent {} asks {}: {}", act.agent, question.id, act.what);
        self.asking.push(Asking {
            question,
            site,
            act,
        });
        self.asked(cx);
    }

    /// The user's answer to the question `id`.
    pub fn answer(&mut self, id: u64, answer: Answer, cx: &mut Context<Self>) {
        let Some(at) = self.asking.iter().position(|asking| asking.question.id == id) else {
            return self.answer_back(id, answer, cx);
        };
        let Asking { site, act, .. } = self.asking.remove(at);
        // check.sh reads this line.
        log!("browser: agent {} answered {id} {answer:?}", act.agent);
        if let Answer::Always | Answer::Never = answer {
            let sites = self.grants.entry(act.agent.clone()).or_default();
            sites.insert(site.clone(), answer == Answer::Always);
            self.save();
        }
        match answer {
            Answer::Once | Answer::Always => self.run(act, cx),
            Answer::Never | Answer::Dismiss => self.refuse(act, &site, SAID_NO, cx),
        }
        self.asked(cx);
    }

    /// The user's answer to an agent asking for a tab back: Hand Back, or Keep, which refuses it.
    fn answer_back(&mut self, id: u64, answer: Answer, cx: &mut Context<Self>) {
        let asked = self.held.iter().find(|(_, held)| {
            held.question == id && held.back.is_some() && held.reply.is_some()
        });
        let Some((&tab, held)) = asked else {
            return;
        };
        // check.sh reads this line.
        log!("browser: agent {} answered {id} {answer:?}", held.agent);
        if let Answer::Once | Answer::Always = answer {
            return self.hand_back(tab, cx);
        }
        let held = self.held.get_mut(&tab).expect("held, as found");
        held.back = None;
        if let Some(reply) = held.reply.take() {
            _ = reply.send(ToolResult::error(format!(
                "The user keeps tab {tab}. Don't ask for it again unless they say so."
            )));
        }
        let (agent, site) = (held.agent.clone(), site(&self.browser.tab(tab, cx).url));
        self.note(&agent, "kept", &site, "");
        self.asked(cx);
    }

    /// The sites where the user answered the agent for good, by name: Always Allow (true) or
    /// Never (false).
    pub fn grants(&self, agent: &str) -> Vec<(SharedString, bool)> {
        let mut grants: Vec<(SharedString, bool)> = (self.grants.get(agent).into_iter())
            .flatten()
            .map(|(site, &may)| (site.clone().into(), may))
            .collect();
        grants.sort();
        grants
    }

    /// Forgets the user's answer for good to the agent on `site`, or on every site with None, so
    /// what asks first there asks again.
    pub fn revoke(&mut self, agent: &str, site: Option<&str>, cx: &mut Context<Self>) {
        let Some(sites) = self.grants.get_mut(agent) else {
            return;
        };
        let revoked = match site {
            Some(site) => sites.remove(site).is_some(),
            None => !std::mem::take(sites).is_empty(),
        };
        if sites.is_empty() {
            self.grants.remove(agent);
        }
        if !revoked {
            return;
        }
        // check.sh reads this line.
        log!("browser: agent {agent} revoked {}", site.unwrap_or("all"));
        self.save();
        match site {
            Some(site) => self.note(agent, "revoked", site, ""),
            None => self.note(agent, "revoked all", "", ""),
        }
        cx.notify();
    }

    /// The question asked now, if any: the first action asked about, else the first handoff whose
    /// agent waits on it.
    pub fn question(&self) -> Option<Question> {
        if let Some(asking) = self.asking.first() {
            return Some(asking.question.clone());
        }
        let handoff = (self.held.iter())
            .filter(|(_, held)| held.reply.is_some())
            .min_by_key(|(_, held)| held.question);
        handoff.map(|(&tab, held)| Question {
            id: held.question,
            agent: name(&held.agent).into(),
            what: match held.back {
                Some(_) => "have a tab back".into(),
                None => "hand you a tab".into(),
            },
            why: held.back.as_ref().unwrap_or(&held.why).clone().into(),
            handoff: Some(tab),
            back: held.back.is_some(),
        })
    }

    fn asked(&self, cx: &mut Context<Self>) {
        self.browser.ask(self.question(), cx);
        cx.notify();
    }

    /// The agent went: what waits for it goes unanswered, its questions with it.
    fn forget_waiting(&mut self, agent: &str, cx: &mut Context<Self>) {
        let asked = self.question().map(|question| question.id);
        self.asking.retain(|asking| asking.act.agent != agent);
        self.acting.retain(|_, acting| acting.agent != agent);
        self.runs.retain(|run| run.agent != agent);
        for held in self.held.values_mut().filter(|held| held.agent == agent) {
            (held.back, held.reply) = (None, None);
        }
        if self.question().map(|question| question.id) != asked {
            self.asked(cx);
        }
    }

    /// Answers the action with why it's refused, which the log has.
    fn refuse(&mut self, act: Act, site: &str, why: &str, cx: &mut Context<Self>) {
        self.note(&act.agent, act.action, site, why);
        cx.notify();
        let answer = match why {
            SAID_NO => format!("The user said no: you can't {}. Don't try again.", act.what),
            SAID_NEVER => {
                format!("The user said never on {site}: you can't {}. Don't try again.", act.what)
            }
            _ => why.to_string(),
        };
        _ = act.reply.send(ToolResult::error(answer));
    }

    /// Does the action: the core checks it and logs it, and answers with the page.
    fn run(&mut self, act: Act, cx: &mut Context<Self>) {
        let Act {
            agent,
            action,
            mut tab,
            url,
            page,
            element,
            text,
            mut note,
            reply,
            ..
        } = act;
        if action == "open_tab" {
            let site = site(&url);
            tab = self.browser.open_tab(&agent, &url, cx);
            if tab == 0 {
                self.note(&agent, action, &site, "The tab couldn't open.");
                cx.notify();
                _ = reply.send(ToolResult::error("The tab couldn't open."));
                return;
            }
            self.note(&agent, action, &site, "");
            cx.notify();
            self.current.insert(agent.clone(), tab);
            note = format!("Opened tab {tab}.");
        } else {
            if !self.act(&agent, tab, action, &reply, cx) {
                return;
            }
            if action == "navigate" && !self.browser.navigate(&agent, tab, &url, cx) {
                _ = reply.send(ToolResult::error("The tab couldn't go there."));
                return;
            }
        }
        let request = self.next_request;
        self.next_request += 1;
        let acting = Acting { agent: agent.clone(), tab, element, note, reply };
        self.acting.insert(request, acting);
        self.act_in_page(&agent, tab, request, page, element, &text);
    }

    /// The ref of the element the action `request` acts on, such as "e12", as the agent named it.
    pub fn ref_of(&self, request: u64) -> Option<String> {
        let acting = self.acting.get(&request)?;
        let number = self.refs.get(&acting.tab)?.number(acting.element)?;
        Some(format!("e{number}"))
    }

    /// The core's answer to the action `request`: with `error`, why it couldn't; else a
    /// screenshot's JPEG in base64, or the nodes of the page's tree, which the answer outlines.
    pub fn page_done(&mut self, request: u64, text: String, error: bool, cx: &mut Context<Self>) {
        let Some(Acting { tab, note, reply, .. }) = self.acting.remove(&request) else {
            return;
        };
        if error {
            _ = reply.send(ToolResult::error(text));
            return self.advance(cx);
        }
        if !text.starts_with('[') {
            _ = reply.send(ToolResult::image(text));
            return;
        }
        let nodes: Vec<Value> = serde_json::from_str(&text).unwrap_or_default();
        let info = self.browser.tab(tab, cx);
        let outline = page::outline(&info, &nodes, self.refs.entry(tab).or_default());
        _ = reply.send(ToolResult::text(match note.is_empty() {
            true => outline,
            false => format!("{note}\n\n{outline}"),
        }));
        self.advance(cx);
    }

    /// Asks the core whether the agent may do `action` to `tab`, and logs the answer. A refusal
    /// answers the call with why.
    fn act(
        &mut self,
        agent: &str,
        tab: u64,
        action: &str,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.act_as(agent, tab, action, action, reply, cx)
    }

    /// As `act`, for what the core checks as `action` and the log names `logged`.
    fn act_as(
        &mut self,
        agent: &str,
        tab: u64,
        action: &str,
        logged: &str,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> bool {
        let checked = self.browser.may_act(agent, tab, action, cx);
        let site = site(&self.browser.tab(tab, cx).url);
        let refused = checked.as_ref().err().map_or("", String::as_str);
        self.note(agent, logged, &site, refused);
        cx.notify();
        match checked {
            Ok(()) => {
                self.current.insert(agent.into(), tab);
                true
            }
            Err(why) => {
                _ = reply.send(ToolResult::error(why));
                false
            }
        }
    }

    /// The agent's Space, made the first time it calls, so it shows in the sidebar for the user
    /// to open pages in.
    fn space(&mut self, client: Client, cx: &mut Context<Self>) -> u64 {
        match self.browser.agent_space(client.id()) {
            0 => self
                .browser
                .make_agent_space(client.id(), client.name(), cx),
            space => space,
        }
    }

    /// The tab the agent used last, if it's still in its Space, else its first.
    fn default_tab(&mut self, client: Client, cx: &mut Context<Self>) -> Option<u64> {
        let space = self.space(client, cx);
        let tabs = self.browser.space_tabs(space, cx);
        let last = self.current.get(client.id()).copied();
        (last.filter(|last| tabs.iter().any(|tab| tab.id == *last)))
            .or_else(|| tabs.first().map(|tab| tab.id))
    }

    fn note(&mut self, agent: &str, action: &str, site: &str, refused: &str) {
        if refused.is_empty() {
            let doing = match site.is_empty() {
                true => tools::describe(action).to_string(),
                false => format!("{} · {site}", tools::describe(action)),
            };
            self.doing.insert(agent.into(), doing.into());
        }
        let entry = Activity {
            at: SystemTime::now(),
            agent: SharedString::from(agent.to_string()),
            action: SharedString::from(action.to_string()),
            site: SharedString::from(site.to_string()),
            refused: SharedString::from(refused.to_string()),
            steps: Vec::new(),
        };
        if self.stepping.as_deref() == Some(agent) {
            let run = self.log.iter_mut().rev().find(|each| {
                each.agent == agent && each.action == "run_steps"
            });
            if let Some(run) = run {
                return run.steps.push(entry);
            }
        }
        if self.log.len() >= LOG_LENGTH {
            self.log.remove(0);
        }
        self.log.push(entry);
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.server.is_some() {
            return;
        }
        let (events, mut handed) = futures::channel::mpsc::unbounded();
        match server::Server::start(&self.socket, self.tokens.clone(), events) {
            Ok(server) => {
                // check.sh reads this line.
                log!("browser: agents server on {}", server.path().display());
                // Wakes only as the server hands something over.
                let task = cx.spawn(async move |agents, cx| {
                    while let Some(event) = handed.next().await {
                        if agents
                            .update(cx, |agents, cx| agents.handle(event, cx))
                            .is_err()
                        {
                            return;
                        }
                    }
                });
                self.server = Some((server, task));
                self.trouble = None;
            }
            Err(error) => {
                log!("browser: agents server couldn't start: {error}");
                self.trouble = Some(format!("The server couldn't start: {error}.").into());
            }
        }
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        // Dropping the server's handover answers each call waiting on it.
        self.server = None;
        self.trouble = None;
        self.reading.clear();
        self.acting.clear();
        self.runs.clear();
        for held in self.held.values_mut() {
            (held.back, held.reply) = (None, None);
        }
        // A handoff's card goes too, the user still holding its tab.
        self.asking.clear();
        self.browser.ask(None, cx);
        for agent in std::mem::take(&mut self.live) {
            self.browser.disconnected(&agent, cx);
            self.note(&agent, "disconnected", "", "");
        }
    }

    fn tokens_changed(&self) {
        let tokens = self
            .agents
            .iter()
            .map(|agent| (agent.token.clone(), agent.client.id().into()));
        *self.tokens.lock().unwrap() = tokens.collect();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let agents: Vec<Value> = (self.agents.iter())
            .map(|agent| json!({ "client": agent.client.id(), "token": agent.token }))
            .collect();
        let grants: serde_json::Map<String, Value> = (self.grants.iter())
            .map(|(agent, sites)| {
                let sites = (sites.iter())
                    .map(|(site, &may)| {
                        (site.clone(), json!(if may { "always" } else { "never" }))
                    });
                (agent.clone(), Value::Object(sites.collect()))
            })
            .collect();
        let saved = json!({ "on": self.on, "agents": agents, "grants": grants });
        if let Err(error) = clients::write_private(path, saved.to_string().as_bytes()) {
            log!("couldn't save the agents' settings: {error}");
        }
    }
}

/// What a run's steps did before it stopped, for its answer.
fn run_so_far(done: &[String]) -> String {
    match done.is_empty() {
        true => String::new(),
        false => format!("\n\nThe steps before it:\n{}", done.join("\n")),
    }
}

/// The agent's name, as its client gives it, else its id.
fn name(agent: &str) -> &str {
    Client::from_id(agent).map_or(agent, |client| client.name())
}

/// What a call of an agent's the user stopped answers.
const STOPPED: &str = "The user stopped you. Don't carry on until they ask you to.";
const NOT_HELD: &str = "The user doesn't have that tab from you, so there's nothing to ask for.";

/// What the log says of an action the user refused, now or for good on its site.
const SAID_NO: &str = "You said no.";
const SAID_NEVER: &str = "You said never on this site.";
/// Why an agent can't open a page that isn't on the web.
const WEB_ONLY: &str = "Agents may open only http and https pages.";
/// Why an agent can't type into a password field.
const PASSWORDS: &str = "Password fields are off limits to agents: signing in is the user's to do. \
    Hand them the tab with hand_off.";
/// What the user reads in the bar of a tab whose page asked for their passkey.
const PASSKEY: &str = "The page asks for your passkey. Sign in, then hand the tab back.";

fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// A new token: 32 random bytes, in hex.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the system has no random numbers");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A URL's host, without "www.", for the log.
pub fn site(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else {
        return String::new();
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    let host = match host.split_once(']') {
        Some((address, _)) => &host[..address.len() + 1],
        None => host.split(':').next().unwrap_or_default(),
    };
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

/// How long ago `at` was, as the log says it.
pub fn ago(at: SystemTime, now: SystemTime) -> String {
    let seconds = now.duration_since(at).map_or(0, |since| since.as_secs());
    match seconds {
        0..60 => "Just now".into(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} d ago", seconds / 86400),
    }
}

#[cfg(test)]
mod tests;
