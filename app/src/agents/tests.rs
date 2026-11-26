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

use super::runner::Cli;
use super::sign_in::Offer;
use super::tier::{Stop, Tier, Why};
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
    /// Whether the core can't send a tab where navigate asks.
    cant_go: RefCell<bool>,
    /// The tabs the user holds from agents, as the core keeps them.
    held: RefCell<HashSet<u64>>,
    /// Each action the agents asked the core whether they may take (bc_agent_may_act).
    checked: RefCell<Vec<String>>,
    /// The app, which the core asks about the key press_key presses from inside bc_agent_page.
    app: RefCell<Option<gpui::AsyncApp>>,
    /// The tasks' Spaces made, and the Spaces deleted.
    task_spaces: RefCell<Vec<u64>>,
    deleted: RefCell<Vec<u64>>,
    /// The tabs the user pinned, and those whose page has a beforeunload handler, which a close
    /// asks, and how many times the core was asked to close each.
    pinned: RefCell<HashSet<u64>>,
    asks: RefCell<HashSet<u64>>,
    closes: RefCell<Vec<(u64, bool)>>,
    /// The downloads the agents answered for the user, and whether each goes on.
    downloads: RefCell<Vec<(u64, bool)>>,
    /// The HTTP requests the agents sent: the tab, the request and what it sent.
    requests: RefCell<Vec<(u64, u64, request::Request)>>,
    /// The saved reads sent: the profile, the read, the address, its size and its time.
    saved_reads: RefCell<Vec<(u64, u64, String, usize, i64)>>,
    /// The split views, each its tabs left to right.
    splits: RefCell<Vec<Vec<u64>>>,
    /// Each mute or unmute asked of a tab, in order.
    muted: RefCell<Vec<(u64, bool)>>,
    /// Tabs the core holds in a Space that its list of the Space's tabs leaves out.
    unlisted: RefCell<HashSet<u64>>,
    /// The file fields the agents dropped.
    dropped: RefCell<Vec<u64>>,
    /// Words the core refuses with in place of its own, as a later core may word them.
    reworded: RefCell<Option<&'static str>>,
    /// The sign-in pages opened, each with its tab, and the tabs and windows closed; and whether
    /// none can open.
    sign_in_pages: RefCell<Vec<(String, u64)>>,
    sign_in_closed: RefCell<Vec<(u64, u64)>>,
    no_sign_in_page: std::cell::Cell<bool>,
    /// The user's Spaces outside the profile the others are in, "Default", each with the name of
    /// its profile's folder.
    profiles: RefCell<Vec<(u64, &'static str)>>,
    /// The Spaces that are a private window's own, as a test says.
    private: RefCell<Vec<u64>>,
    /// The user's Space in view, where a test moves it from 1.
    user: std::cell::Cell<Option<u64>>,
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
            .filter(|tab| tab.1 == space && !self.unlisted.borrow().contains(&tab.0))
            .map(|&(id, _, title, url)| TabInfo {
                id,
                title: title.into(),
                url: url.into(),
                pinned: self.pinned.borrow().contains(&id),
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
                pinned: self.pinned.borrow().contains(&id),
            },
        )
    }

    fn space_of(&self, tab: u64, _: &App) -> u64 {
        let tabs = self.tabs.borrow();
        tabs.iter()
            .find(|each| each.0 == tab)
            .map_or(0, |each| each.1)
    }

    fn may_act(
        &self,
        agent: &str,
        tab: u64,
        action: &str,
        _: &mut App,
    ) -> Result<(), (Failure, String)> {
        self.checked.borrow_mut().push(action.into());
        let words = |words: &str| (*self.reworded.borrow()).unwrap_or(words).to_string();
        if self.held.borrow().contains(&tab) {
            return Err((Failure::Held, words(HELD)));
        }
        let space = self.agent_space(agent);
        match self
            .tabs
            .borrow()
            .iter()
            .any(|each| each.0 == tab && each.1 == space && space != 0)
        {
            true => Ok(()),
            false => Err((
                Failure::NotItsSpace,
                words("The tab isn't in the agent's own Space."),
            )),
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
        !*self.cant_go.borrow()
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

    /// A name with a dot is a site's, as the address bar takes it; other words are a search. Text
    /// with a NUL in it can't reach the core, which gives no address for it, as the app's does.
    fn url_for(&self, text: &str) -> Option<String> {
        if text.contains('\0') {
            return None;
        }
        Some(match (text.contains("://"), text.contains('.')) {
            (true, _) => text.into(),
            (false, true) => format!("https://{text}/"),
            (false, false) => format!("https://duckduckgo.com/?q={text}"),
        })
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

    // As the core does, without a dialog.
    fn close_user_tab(&self, tab: u64, _: &mut App) {
        self.tabs.borrow_mut().retain(|each| each.0 != tab);
    }

    fn file_field_drop(&self, field: u64) {
        self.dropped.borrow_mut().push(field);
    }

    fn answer_download(&self, download: u64, keep: bool) {
        self.downloads.borrow_mut().push((download, keep));
    }

    fn split(&self, tab: u64, _: &App) -> Vec<u64> {
        let splits = self.splits.borrow();
        match splits.iter().find(|split| split.contains(&tab)) {
            Some(split) => split.clone(),
            None => vec![tab],
        }
    }

    fn url_site(&self, url: &str) -> Option<String> {
        crate::sort_by_site::made_up_site(url)
    }

    fn move_today(&self, tab: u64, index: usize, cx: &mut App) {
        let panes = self.split(tab, cx);
        let mut tabs = self.tabs.borrow_mut();
        let Some(space) = tabs.iter().find(|each| each.0 == tab).map(|each| each.1) else {
            return;
        };
        let moved: Vec<_> = (panes.iter())
            .filter_map(|&pane| tabs.iter().find(|each| each.0 == pane).copied())
            .collect();
        tabs.retain(|each| !panes.contains(&each.0));
        let pinned = self.pinned.borrow();
        let today: Vec<usize> = (0..tabs.len())
            .filter(|&at| tabs[at].1 == space && !pinned.contains(&tabs[at].0))
            .collect();
        let at = match today.get(index) {
            Some(&at) => at,
            None => today.last().map_or(tabs.len(), |&last| last + 1),
        };
        tabs.splice(at..at, moved);
    }

    // As the core does: three tabs at most, of one Space, the tab leaving any split it was in.
    fn split_with(&self, tab: u64, with: u64, cx: &mut App) -> bool {
        let space = |tab| {
            self.tabs
                .borrow()
                .iter()
                .find(|each| each.0 == tab)
                .map(|each| each.1)
        };
        let joined = self.split(with, cx);
        if joined.len() >= tools::SPLIT_TABS || space(tab) != space(with) {
            return false;
        }
        self.separate(tab, cx);
        let mut splits = self.splits.borrow_mut();
        match splits.iter_mut().find(|split| split.contains(&with)) {
            Some(split) => split.push(tab),
            None => splits.push(vec![with, tab]),
        }
        true
    }

    fn separate(&self, tab: u64, _: &mut App) {
        let mut splits = self.splits.borrow_mut();
        splits
            .iter_mut()
            .for_each(|split| split.retain(|&each| each != tab));
        splits.retain(|split| split.len() > 1);
    }

    fn set_pinned(&self, tab: u64, pinned: bool, _: &mut App) {
        match pinned {
            true => self.pinned.borrow_mut().insert(tab),
            false => self.pinned.borrow_mut().remove(&tab),
        };
    }

    // As the core's: the tab closes with its split.
    fn remove_pinned(&self, tab: u64, cx: &mut App) {
        self.set_pinned(tab, false, cx);
        for each in self.split(tab, cx) {
            self.separate(each, cx);
            self.close_user_tab(each, cx);
        }
    }

    fn close_tab(&self, agent: &str, tab: u64, discard: bool, _: &mut App) -> Closed {
        let space = self.agent_space(agent);
        let ours = (self.tabs.borrow().iter()).any(|each| each.0 == tab && each.1 == space);
        if space == 0 || self.held.borrow().contains(&tab) || !ours {
            return Closed::Refused;
        }
        if self.pinned.borrow().contains(&tab) {
            return Closed::Pinned;
        }
        self.closes.borrow_mut().push((tab, discard));
        if !discard && self.asks.borrow().contains(&tab) {
            return Closed::Pending;
        }
        self.tabs.borrow_mut().retain(|each| each.0 != tab);
        Closed::Closed
    }

    fn set_muted(&self, tab: u64, muted: bool) {
        self.muted.borrow_mut().push((tab, muted));
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

    /// The user's Space is 1, or the one a test moved the user to.
    fn user_space(&self, _: u64) -> u64 {
        self.user.get().unwrap_or(1)
    }

    fn space_profile(&self, space: u64) -> String {
        let profiles = self.profiles.borrow();
        let named = profiles.iter().find(|each| each.0 == space);
        named.map_or("Default", |each| each.1).into()
    }

    /// The user's Space in "Default" is 1.
    fn profile_space(&self, profile: &str) -> u64 {
        let profiles = self.profiles.borrow();
        let space = profiles.iter().find(|each| each.1 == profile);
        space.map_or(u64::from(profile == "Default"), |each| each.0)
    }

    /// "Default" is always there; another is while a test lists it, with Space 0 for one that
    /// has no Space of the user's.
    fn has_profile(&self, profile: &str) -> bool {
        profile == "Default" || self.profiles.borrow().iter().any(|each| each.1 == profile)
    }

    fn private_space(&self, space: u64) -> bool {
        self.private.borrow().contains(&space)
    }

    fn request(&self, _: &str, tab: u64, request: u64, sent: &request::Request) {
        self.requests
            .borrow_mut()
            .push((tab, request, sent.clone()));
    }

    fn read(&self, profile: u64, read: u64, url: &str, max_bytes: usize, timeout_ms: i64) {
        self.saved_reads
            .borrow_mut()
            .push((profile, read, url.into(), max_bytes, timeout_ms));
    }

    /// Each page in a tab of its own, 700 on across the tests, which share which tabs are quiet, in
    /// a window numbered as the tab less 600.
    fn open_sign_in(&self, url: &str, _: &mut App) -> Option<(u64, u64)> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(700);
        if self.no_sign_in_page.get() {
            return None;
        }
        let tab = NEXT.fetch_add(1, Ordering::SeqCst);
        self.sign_in_pages.borrow_mut().push((url.into(), tab));
        Some((tab, tab - 600))
    }

    fn close_sign_in(&self, tab: u64, window: u64, _: &mut App) {
        self.sign_in_closed.borrow_mut().push((tab, window));
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
    /// What each task CLI answers when looked for; one not here isn't found.
    looked: Mutex<Vec<(crate::agents::runner::Cli, crate::agents::ready::Found)>>,
    /// How many times a task CLI was looked for, each CLI counted.
    looks: AtomicUsize,
    /// Whether the user's shell leaves out the `ilo` command's folder.
    unsearched: Mutex<bool>,
    /// Each sign-in started, as a stand-in the test says what it hears through; the CLIs whose
    /// sign-in was cancelled; those signed out; and what the next sign-out answers, else Ok.
    sign_ins: Mutex<Vec<(crate::agents::runner::Cli, SignInHeard)>>,
    sign_ins_cancelled: Arc<Mutex<Vec<crate::agents::runner::Cli>>>,
    signed_out: Mutex<Vec<crate::agents::runner::Cli>>,
    sign_out_answers: Mutex<Option<Result<(), Option<String>>>>,
    /// Whether the next sign-in can't start.
    sign_in_fails: Mutex<bool>,
}

type SignInHeard = futures::channel::mpsc::UnboundedSender<crate::agents::sign_in::Heard>;

/// A sign-in in the tests: a cancel is recorded, and ends it as a CLI's does.
struct StandInSignIn {
    cli: crate::agents::runner::Cli,
    heard: SignInHeard,
    cancelled: Arc<Mutex<Vec<crate::agents::runner::Cli>>>,
}

impl crate::agents::sign_in::Session for StandInSignIn {
    fn cancel(&mut self) {
        use crate::agents::sign_in::{Ended, Heard};
        self.cancelled.lock().unwrap().push(self.cli);
        _ = self.heard.unbounded_send(Heard::Ended(Ended::Cancelled));
    }
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

    fn searches(&self, _: &std::path::Path) -> bool {
        !*self.unsearched.lock().unwrap()
    }

    /// The tests never run a CLI of this computer's: each answers as `looked` says.
    fn look_for_task_cli(&self, cli: crate::agents::runner::Cli) -> crate::agents::ready::Found {
        self.looks.fetch_add(1, Ordering::SeqCst);
        let looked = self.looked.lock().unwrap();
        let found = looked.iter().find(|(each, _)| *each == cli);
        found.map_or(
            crate::agents::ready::Found {
                ready: crate::agents::ready::Ready::NotFound,
                ..Default::default()
            },
            |(_, found)| found.clone(),
        )
    }

    /// The tests never start a CLI of this computer's: the test says what each sign-in hears.
    fn start_sign_in(
        &self,
        cli: crate::agents::runner::Cli,
        _: &crate::agents::ready::Found,
        heard: SignInHeard,
    ) -> std::io::Result<Box<dyn crate::agents::sign_in::Session>> {
        if std::mem::take(&mut *self.sign_in_fails.lock().unwrap()) {
            return Err(std::io::ErrorKind::NotFound.into());
        }
        self.sign_ins.lock().unwrap().push((cli, heard.clone()));
        let cancelled = self.sign_ins_cancelled.clone();
        Ok(Box::new(StandInSignIn {
            cli,
            heard,
            cancelled,
        }))
    }

    fn sign_out(
        &self,
        cli: crate::agents::runner::Cli,
        _: &crate::agents::ready::Found,
    ) -> Result<(), Option<String>> {
        self.signed_out.lock().unwrap().push(cli);
        self.sign_out_answers
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Ok(()))
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
    let agents = cx.new(|_| Agents::new(browser, configs, path, socket, bridge));
    // Other apps' agents on, as the agents of most tests are the user's connected apps: a new
    // profile's off is other_apps.rs's.
    agents.update(cx, |agents, _| {
        (agents.switches.others).store(true, std::sync::atomic::Ordering::SeqCst)
    });
    agents
}

/// The app's Agents, on a made-up browser and clients, for another view's tests: each task CLI
/// answers as `looked` says, and the count returned says how many times they were looked for.
pub(crate) fn stand_in(
    looked: Vec<(crate::agents::runner::Cli, crate::agents::ready::Found)>,
    cx: &mut App,
) -> (Entity<Agents>, impl Fn() -> usize + use<>) {
    let configs = Arc::new(Configs::default());
    *configs.looked.lock().unwrap() = looked;
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let number = NEXT.fetch_add(1, Ordering::SeqCst);
    let folder = std::env::temp_dir().join(format!("agents-in-{}-{number}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let (browser, clients) = (Made::with_agent_space(), configs.clone());
    let bridge = vec!["/app/mcp-bridge".into()];
    let agents = cx.new(|_| Agents::new(browser, clients, None, address(&folder), bridge));
    cx.set_global(GlobalAgents(agents.clone()));
    // One look asks each CLI once.
    let looks =
        move || configs.looks.load(Ordering::SeqCst) / crate::agents::runner::Cli::ALL.len();
    (agents, looks)
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
        connection: 1,
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

/// The key of `agent`'s session under way, which its first call that opens or acts started.
fn session_key(agents: &Entity<Agents>, agent: &str, cx: &mut TestAppContext) -> String {
    agents.read_with(cx, |agents, _| {
        (agents.tasks())
            .find(|task| {
                task.ended.is_none()
                    && (task.session.as_ref()).is_some_and(|session| session.agent == agent)
            })
            .map(|task| task.agent.clone())
            .unwrap_or_else(|| panic!("{agent} has no session under way"))
    })
}

/// The user gives Claude Code's session under way the access `tier`, as its line or chip does.
fn give(agents: &Entity<Agents>, tier: Tier, cx: &mut TestAppContext) {
    let key = session_key(agents, "claude-code", cx);
    agents.update(cx, |agents, cx| agents.set_tier(&key, tier, cx));
}

/// The refusals Claude Code's session holds that the user hasn't settled, oldest first.
fn stops(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<Stop> {
    let key = session_key(agents, "claude-code", cx);
    agents.read_with(cx, |agents, _| agents.stops(&key))
}

/// Checks `answer` is a refusal, at once, of the act `what` for `why`, which nothing asked about
/// and Claude Code's session shows as `at`.
fn refused(
    agents: &Entity<Agents>,
    browser: &Made,
    answer: mpsc::Receiver<ToolResult>,
    what: &str,
    why: Why,
    at: &str,
    cx: &mut TestAppContext,
) -> ToolResult {
    let refused = (answer.try_recv()).unwrap_or_else(|_| panic!("{what} wasn't refused at once"));
    assert!(refused.error, "{what} went: {}", refused.text);
    assert!(
        refused.text.contains(what),
        "{what} isn't in: {}",
        refused.text
    );
    assert!(
        refused.text.contains("Don't try another way"),
        "{}",
        refused.text
    );
    assert_eq!(shown(&agents, cx), None, "{what} asked");
    // The same refusal again counts on the line it showed as, so it's found by what it stopped.
    // The line shows the page's words capped, as it shows them.
    let at = super::tier::plain(at);
    let shown = stops(agents, cx);
    let stop = (shown.iter().rev().find(|stop| stop.at == at))
        .unwrap_or_else(|| panic!("{what} shows no refusal at {at}: {shown:?}"));
    assert_eq!(stop.why, why, "{what}");
    refused
}

/// The switch is off at first, and the server runs only while it's on: on, its socket or pipe is
/// in place for the user alone and takes a connection; off, it's gone.
/// The step that waits on the user now, as the task's card says it.
fn shown(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Option<Question> {
    agents.read_with(cx, |agents, _| agents.question())
}

#[gpui::test]
fn agents_are_off_until_the_user_turns_them_on(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.set_other_apps(false, cx));
    assert!(!agents.read_with(cx, |agents, _| agents.listening()));
    assert!(agents.read_with(cx, |agents, _| agents.server.is_none()));
    let socket = agents.read_with(cx, |agents, _| agents.socket.clone());
    assert!(server::connect(&socket).is_err());

    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&socket).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert!(server::connect(&socket).is_ok());

    agents.update(cx, |agents, cx| agents.set_threads(false, cx));
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

/// Any MCP client's line and Your assistant's row can't disagree: a client whose CLI the
/// assistants' look found is listed though its files aren't in the home, so the line never calls
/// it missing beside a row that says it's there, and one the look didn't find is missing only
/// where its row says it isn't installed.
#[gpui::test]
fn the_clients_line_agrees_with_the_assistants_rows(cx: &mut TestAppContext) {
    use crate::agents::ready::{Found, Ready};
    use crate::agents::runner::Cli;
    for ready in [Ready::NotFound, Ready::SignedOut, Ready::SignedIn] {
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let found = Found {
            ready,
            ..Default::default()
        };
        *configs.looked.lock().unwrap() = vec![(Cli::Claude, found)];
        let agents = agents(&browser, &configs, None, cx);
        // The assistants' look runs only with the inside switch on.
        agents.update(cx, |agents, cx| {
            agents.set_threads(true, cx);
            agents.look_for_clients(cx);
            agents.look_for_task_agents(cx);
        });
        cx.run_until_parked();
        let (row, listed) = agents.read_with(cx, |agents, _| {
            (agents.assistant_row(Cli::Claude).line, agents.listed())
        });
        assert_eq!(
            !listed.contains(&Client::ClaudeCode),
            row == "Not installed",
            "{ready:?}: Claude Code's row says {row:?} and Apps that connect lists {listed:?}"
        );
    }
}

/// Settings › Models' assistants with Claude Code found as `ready` and agents off, as a new
/// profile has them: each assistant the app can run, its offer, and whether its model is drawn.
fn providers_found(
    ready: crate::agents::ready::Ready,
    cx: &mut TestAppContext,
) -> Vec<(Cli, Offer, bool)> {
    use crate::agents::ready::Found;
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, _| {
        let found = Found {
            ready,
            ..Default::default()
        };
        agents.tasks.found.insert(Cli::Claude, found);
    });
    agents.read_with(cx, |agents, _| {
        assert!(!agents.threads(), "the page is drawn with threads off");
        let listed = agents.providers().into_iter();
        listed
            .map(|(cli, row, model)| (cli, row.offer, model))
            .collect()
    })
}

/// Overview's loose ends read what the app already found and ask nothing: a sign-in is one only
/// for an assistant known signed out or missing while another is signed in, none while its state
/// is unknown or threads are off, none once put away, and no assistant is looked for to list them.
#[gpui::test]
fn overview_lists_only_what_the_app_already_knows(cx: &mut TestAppContext) {
    use crate::agents::ready::{Found, Ready};
    use crate::settings_overview::{LooseEnd, PutAway, loose_ends};
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    let found = |cli, ready, cx: &mut TestAppContext| {
        agents.update(cx, |agents, _| {
            let found = Found {
                ready,
                ..Default::default()
            };
            agents.tasks.found.insert(cli, found);
        })
    };
    let listed = |away: &PutAway, cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| loose_ends(agents, away))
    };
    let away = PutAway::default();
    agents.update(cx, |agents, _| {
        agents.switches.threads.store(true, Ordering::SeqCst)
    });
    let looks = configs.looks.load(Ordering::SeqCst);
    assert_eq!(listed(&away, cx), [], "nothing known, nothing listed");
    found(Cli::Claude, Ready::SignedIn, cx);
    assert_eq!(listed(&away, cx), [], "ChatGPT's state isn't known");
    found(Cli::Codex, Ready::SignedOut, cx);
    assert_eq!(listed(&away, cx), [LooseEnd::SignIn(Cli::Codex)]);
    found(Cli::Claude, Ready::SignedOut, cx);
    assert_eq!(
        listed(&away, cx),
        [],
        "with none signed in, threads don't run without it"
    );
    found(Cli::Claude, Ready::SignedIn, cx);
    let mut away = PutAway::default();
    away.put_away(&LooseEnd::SignIn(Cli::Codex).key());
    assert_eq!(listed(&away, cx), [], "a put-away loose end came back");
    agents.update(cx, |agents, _| {
        agents.switches.threads.store(false, Ordering::SeqCst)
    });
    assert_eq!(listed(&PutAway::default(), cx), [], "threads are off");
    cx.run_until_parked();
    assert_eq!(
        configs.looks.load(Ordering::SeqCst),
        looks,
        "Overview looked for an assistant"
    );
}

/// Models is never only its title: an assistant not on this computer is listed with what Settings
/// offers for it, and no model under it.
#[gpui::test]
fn models_lists_an_assistant_not_on_this_computer(cx: &mut TestAppContext) {
    let listed = providers_found(crate::agents::ready::Ready::NotFound, cx);
    assert_eq!(listed.len(), Cli::ALL.len(), "every assistant: {listed:?}");
    assert!(
        listed.contains(&(Cli::Claude, Offer::Get, false)),
        "{listed:?}"
    );
}

/// One on this computer and not signed in is listed with Sign In, and no model under it.
#[gpui::test]
fn models_lists_an_assistant_not_signed_in_with_sign_in(cx: &mut TestAppContext) {
    let listed = providers_found(crate::agents::ready::Ready::SignedOut, cx);
    assert_eq!(listed.len(), Cli::ALL.len(), "every assistant: {listed:?}");
    assert!(
        listed.contains(&(Cli::Claude, Offer::SignIn, false)),
        "{listed:?}"
    );
}

/// One signed in is listed signed in, with its model for a new thread under it.
#[gpui::test]
fn models_lists_a_signed_in_assistant_with_its_model(cx: &mut TestAppContext) {
    let listed = providers_found(crate::agents::ready::Ready::SignedIn, cx);
    assert_eq!(listed.len(), Cli::ALL.len(), "every assistant: {listed:?}");
    assert!(
        listed.contains(&(Cli::Claude, Offer::SignedIn, true)),
        "{listed:?}"
    );
}

/// With the inside switch off, a click on a row while a look runs is looked for once it ends, and
/// a look no click asked for waits for none: each assistant clicked ends looked up, and nothing
/// else is.
#[gpui::test]
fn a_click_while_a_look_runs_is_looked_for_after_it(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    let looks = || configs.looks.load(Ordering::SeqCst);
    let found = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| {
            Cli::ALL.map(|cli| agents.tasks.found.contains_key(&cli))
        })
    };
    agents.update(cx, |agents, cx| {
        agents.look_up_assistant(Cli::Claude, cx);
        // While Claude's look runs: Codex's row, and what no click asks for.
        agents.look_up_assistant(Cli::Codex, cx);
        agents.models_shown(cx);
        agents.look_for_task_agents(cx);
    });
    cx.run_until_parked();
    assert_eq!(looks(), 2, "Claude's, then Codex's after it");
    assert_eq!(found(cx), [true, true]);

    // A look for every one asked for with the switch on, which goes off while one runs, waits
    // for nothing; the row clicked meanwhile is still looked for.
    agents.update(cx, |agents, cx| {
        agents.set_threads(true, cx);
        agents.look_up_assistant(Cli::Claude, cx);
        agents.look_for_task_agents(cx);
        agents.look_up_assistant(Cli::Codex, cx);
        agents.set_threads(false, cx);
    });
    cx.run_until_parked();
    assert_eq!(looks(), 4, "Claude's, then the click on Codex's alone");
}

/// With the inside switch off, nothing of an assistant's program runs at launch or as Models
/// opens: each row reads Checking… with nothing offered, and a click on a row looks that one up
/// alone. With the switch on, opening Models looks once for every one and says what it found.
#[gpui::test]
fn models_looks_for_the_assistants_only_as_it_opens(cx: &mut TestAppContext) {
    use crate::agents::ready::{Found, Ready};
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let signed_out = Found {
        ready: Ready::SignedOut,
        ..Default::default()
    };
    configs
        .looked
        .lock()
        .unwrap()
        .push((Cli::Claude, signed_out));
    let agents = agents(&browser, &configs, None, cx);
    cx.run_until_parked();
    assert_eq!(configs.looks.load(Ordering::SeqCst), 0, "nothing looked");
    let before = agents.read_with(cx, |agents, _| {
        assert!(!agents.threads());
        let listed = agents.providers().into_iter();
        listed.map(|(_, row, _)| row.offer).collect::<Vec<_>>()
    });
    assert_eq!(before, vec![Offer::Nothing; Cli::ALL.len()]);
    agents.update(cx, |agents, cx| {
        agents.models_shown(cx);
        agents.look_for_clients(cx);
        agents.task_ask_shown(cx);
    });
    cx.run_until_parked();
    assert_eq!(
        configs.looks.load(Ordering::SeqCst),
        0,
        "Models, Other apps and the bar ran nothing with threads off"
    );
    agents.update(cx, |agents, cx| agents.look_up_assistant(Cli::Claude, cx));
    cx.run_until_parked();
    assert_eq!(configs.looks.load(Ordering::SeqCst), 1, "the row clicked");
    let clicked = agents.read_with(cx, |agents, _| {
        let listed = agents.providers().into_iter();
        listed
            .map(|(cli, row, _)| (cli, row.offer))
            .collect::<Vec<_>>()
    });
    assert!(
        clicked.contains(&(Cli::Claude, Offer::SignIn)),
        "{clicked:?}"
    );
    assert!(
        clicked.contains(&(Cli::Codex, Offer::Nothing)),
        "{clicked:?}"
    );
    agents.update(cx, |agents, cx| {
        agents.set_threads(true, cx);
        agents.models_shown(cx);
    });
    cx.run_until_parked();
    assert_eq!(
        configs.looks.load(Ordering::SeqCst),
        1 + Cli::ALL.len(),
        "one look for every one"
    );
    let after = agents.read_with(cx, |agents, _| {
        let listed = agents.providers().into_iter();
        listed
            .map(|(cli, row, _)| (cli, row.offer))
            .collect::<Vec<_>>()
    });
    assert!(after.contains(&(Cli::Claude, Offer::SignIn)), "{after:?}");
    assert!(after.contains(&(Cli::Codex, Offer::Get)), "{after:?}");
}

/// Sign In on Models, with agents off, starts that assistant's own sign-in and nothing else:
/// agents stay off, and it looks again only once the sign-in ends.
#[gpui::test]
fn models_sign_in_with_agents_off_turns_nothing_on(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.sign_in(Cli::Claude, cx));
    cx.run_until_parked();
    let started: Vec<Cli> = (configs.sign_ins.lock().unwrap().iter())
        .map(|(cli, _)| *cli)
        .collect();
    assert_eq!(started, vec![Cli::Claude]);
    assert_eq!(configs.looks.load(Ordering::SeqCst), 0);
    agents.read_with(cx, |agents, _| {
        assert!(!agents.threads(), "threads stay off");
    });
    let heard = configs.sign_ins.lock().unwrap()[0].1.clone();
    let done = crate::agents::sign_in::Heard::Ended(crate::agents::sign_in::Ended::Done);
    heard.unbounded_send(done).unwrap();
    cx.run_until_parked();
    assert_eq!(
        configs.looks.load(Ordering::SeqCst),
        1,
        "the row signed in to, alone"
    );
    agents.read_with(cx, |agents, _| {
        assert!(!agents.threads(), "threads stay off");
    });
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
    assert!(answer.text.starts_with("You have no tabs yet."));

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

/// An agent whose first call is open_tab gets its Space then, and the tab opens there (#1468),
/// muted, as an agent's tabs are until the user unmutes or keeps one, and so are the tabs its page
/// opens there (#1080).
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
    assert_eq!(
        *browser.muted.borrow(),
        [(30, true)],
        "the agent's tab opened with sound"
    );
    // A tab its page opens, beside it in its Space, is as quiet; one a user's page opens isn't.
    (browser.tabs.borrow_mut()).push((31, 2, "Pop-up", "https://example.com/pop"));
    (browser.tabs.borrow_mut()).push((32, 1, "Mine", "https://example.com/mine"));
    agents.update(cx, |agents, cx| {
        agents.tab_opened(31, 30, cx);
        agents.tab_opened(32, 10, cx);
    });
    assert_eq!(
        *browser.muted.borrow(),
        [(30, true), (31, true)],
        "a tab the agent's page opened had sound, or the user's was muted"
    );
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

/// While the user holds one of the agent's tabs, list_tabs names it by its number and says it's
/// with the user, with nothing of its page, and sorting by site leaves it where it is, as its place
/// would tell its site (#3694). Handed back, it's listed as before.
#[gpui::test]
fn a_tab_the_user_holds_is_listed_without_its_page(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    browser.tabs.borrow_mut().extend([
        (21, 2, "Wiki", "https://en.wikipedia.org/wiki/Rust"),
        (22, 2, "Docs", "https://docs.example.com/"),
        (25, 2, "More", "https://de.wikipedia.org/wiki/Arc"),
    ]);
    let list = |cx: &mut TestAppContext| {
        call(&agents, "claude-code", "list_tabs", json!({}), cx)
            .recv()
            .unwrap()
            .text
    };
    assert!(list(cx).contains("21  Wiki  https://en.wikipedia.org/wiki/Rust"));
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 21, cx));
    let held = list(cx);
    let line = held
        .lines()
        .find(|line| line[2..].starts_with("21  "))
        .unwrap();
    assert!(
        line.contains("With the user until they hand it back"),
        "{held}"
    );
    assert!(
        !held.contains("Wiki") && !held.contains("wikipedia.org/wiki/Rust"),
        "list_tabs named the held tab's page: {held}"
    );
    assert!(
        held.contains("22  Docs  https://docs.example.com/"),
        "{held}"
    );

    let sorted = call(&agents, "claude-code", "sort_tabs_by_site", json!({}), cx)
        .recv()
        .unwrap();
    assert_eq!(
        sorted,
        ToolResult::text("Your Today tabs are together by site already."),
        "the held tab's site moved a tab"
    );
    let order: Vec<u64> = (browser.tabs.borrow().iter())
        .filter(|tab| tab.1 == 2)
        .map(|tab| tab.0)
        .collect();
    assert_eq!(order, [20, 21, 22, 25]);

    agents.update(cx, |agents, cx| agents.hand_back(21, cx));
    assert!(list(cx).contains("21  Wiki  https://en.wikipedia.org/wiki/Rust"));
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

/// Every tool that takes a tab, with what it needs besides, so only the tab can refuse it.
const TAB_TOOLS: &[(&str, &str)] = &[
    ("read_page", "{}"),
    ("snapshot", "{}"),
    ("screenshot", "{}"),
    ("click", r#"{ "ref": "e1" }"#),
    ("type", r#"{ "ref": "e1", "text": "x" }"#),
    ("select", r#"{ "ref": "e1", "option": "a" }"#),
    ("press_key", r#"{ "key": "Enter" }"#),
    ("scroll", r#"{ "direction": "down" }"#),
    ("navigate", r#"{ "url": "https://www.example.org/" }"#),
    ("navigate", r#"{ "url": "back" }"#),
    (
        "run_steps",
        r#"{ "steps": [{ "action": "scroll", "direction": "down" }] }"#,
    ),
    ("request", r#"{ "method": "GET", "path": "/" }"#),
    ("close_tab", "{}"),
    ("remove_tab", "{}"),
    ("pin_tab", "{}"),
    ("unpin_tab", "{}"),
    ("split_tabs", r#"{ "tabs": [20, 21] }"#),
    ("add_to_split", r#"{ "split": 20 }"#),
    ("separate_tab", "{}"),
    ("list_split", "{}"),
    ("hand_off", r#"{ "reason": "To sign in." }"#),
];

/// Puts a tab of the user's, 21 on `url` with the title `title`, first in Claude Code's Space,
/// where a call without a tab would otherwise go, and checks the agent gets nothing of it: list_tabs
/// leaves it out, or for a page off the web names it by its number alone, no call without a tab
/// lands on it, and every tool given it is refused with `refused` before the core reads or does
/// anything in it. The activity log names the page by its kind, `kind`, alone. A page off the web
/// the agent may still close, its way out of a tab that went there.
fn gets_nothing_of(
    url: &'static str,
    title: &'static str,
    kind: &str,
    refused: &str,
    cx: &mut TestAppContext,
) {
    let (agents, browser, _) = connected(cx);
    browser.tabs.borrow_mut().insert(1, (21, 2, title, url));

    let listed = call(&agents, "claude-code", "list_tabs", json!({}), cx)
        .recv()
        .unwrap()
        .text;
    let off_the_web = refused == NOT_WEB_PAGE;
    assert!(listed.contains("20  Example Domain"), "{listed}");
    assert!(
        !listed.contains(title) && !listed.contains(url),
        "list_tabs named {url}: {listed}"
    );
    assert_eq!(
        listed.contains("21  A page that isn't http or https"),
        off_the_web,
        "list_tabs for {url}: {listed}"
    );
    assert_eq!(listed.contains("21 "), off_the_web, "{listed}");

    for &(tool, arguments) in TAB_TOOLS {
        if off_the_web && tool == "close_tab" {
            continue;
        }
        let mut arguments: Value = serde_json::from_str(arguments).unwrap();
        arguments["tab"] = json!(21);
        let answer = call(&agents, "claude-code", tool, arguments.clone(), cx);
        cx.run_until_parked();
        let answer = answer.recv().unwrap();
        assert!(
            answer.error && answer.text.contains(refused),
            "{tool} {arguments} on {url}: {answer:?}"
        );
        assert!(!answer.text.contains(title), "{tool} named the tab's title");
    }
    // ask_for_tab: a tab the user doesn't hold is never the agent's to ask for, and one they took
    // over and then opened the page in is refused as the rest.
    let ask = json!({ "tab": 21, "reason": "To see it." });
    let answer = call(&agents, "claude-code", "ask_for_tab", ask.clone(), cx);
    let answer = answer.recv().unwrap();
    assert!(
        answer.error && !answer.text.contains(title),
        "ask_for_tab on {url}: {answer:?}"
    );
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 21, cx));
    let answer = call(&agents, "claude-code", "ask_for_tab", ask, cx);
    let answer = answer.recv().unwrap();
    assert!(
        answer.error && answer.text.contains(refused),
        "ask_for_tab on {url}, held: {answer:?}"
    );
    assert!(browser.reads.borrow().is_empty(), "the core read {url}");
    assert!(browser.pages.borrow().is_empty(), "the core acted in {url}");
    assert!(
        browser.navigated.borrow().is_empty(),
        "the core navigated {url}"
    );
    assert!(browser.closes.borrow().is_empty(), "the core closed {url}");
    assert!(
        browser.requests.borrow().is_empty(),
        "the core sent a request from {url}"
    );
    assert!(browser.splits.borrow().is_empty(), "the core split {url}");
    let (_, site, why) = log(&agents, "claude-code", cx)[0].clone();
    assert_eq!(why, refused, "the activity log doesn't say why");
    assert!(
        !site.contains(title),
        "the activity log named the tab's title: {site}"
    );
    assert_eq!(site, kind, "the activity log's site for {url}");

    let _ = call(&agents, "claude-code", "read_page", json!({}), cx);
    assert_eq!(
        browser.reads.borrow()[0].0,
        20,
        "read_page without a tab read {url}"
    );

    if off_the_web {
        agents.update(cx, |agents, cx| agents.hand_back(21, cx));
        let answer = call(
            &agents,
            "claude-code",
            "close_tab",
            json!({ "tab": 21 }),
            cx,
        );
        cx.run_until_parked();
        let answer = answer.recv().unwrap();
        assert!(!answer.error, "close_tab on {url}: {answer:?}");
        assert!(
            !answer.text.contains(title),
            "close_tab named the tab's title"
        );
        assert_eq!(browser.closes.borrow().as_slice(), &[(21, false)]);
    }
}

/// A Board the user opens in an agent's Space stays the user's alone, as a Board and what's on it
/// are: the agent hears of it only as of a tab not its own (#3073).
#[gpui::test]
fn an_agent_never_reaches_a_board_in_its_space(cx: &mut TestAppContext) {
    gets_nothing_of(
        "chrome://board/0123456789abcdef0123456789abcdef",
        "Kitchen plans",
        "board",
        "The tab isn't in the agent's own Space.",
        cx,
    );
}

/// A page that isn't http(s), opened by the user in an agent's Space, is one agents never use: no
/// title, address, text or picture of it reaches one.
#[gpui::test]
fn an_agent_gets_nothing_of_a_page_off_the_web_in_its_space(cx: &mut TestAppContext) {
    let refused = "The tab's page isn't an http or https page, the only pages agents may use.";
    gets_nothing_of(
        "chrome://settings/passwords",
        "Settings",
        "settings",
        refused,
        cx,
    );
}

#[gpui::test]
fn an_agent_gets_nothing_of_a_file_in_its_space(cx: &mut TestAppContext) {
    let refused = "The tab's page isn't an http or https page, the only pages agents may use.";
    let file = "file:///Users/someone/notes.txt";
    gets_nothing_of(file, "notes.txt", "file", refused, cx);
}

#[gpui::test]
fn an_agent_gets_nothing_of_an_extensions_page_in_its_space(cx: &mut TestAppContext) {
    let refused = "The tab's page isn't an http or https page, the only pages agents may use.";
    gets_nothing_of(
        "chrome-extension://abcdefghijklmnopabcdefghijklmnop/options.html",
        "Options",
        "extension",
        refused,
        cx,
    );
}

/// A pinned Board not yet loaded has only the URL it's pinned to, which is enough.
#[test]
fn a_tab_pinned_to_a_board_is_off_limits_before_it_loads() {
    let board = "chrome://board/0123456789abcdef0123456789abcdef";
    assert_eq!(off_limits("", board), Some(NOT_ITS_SPACE));
    assert_eq!(
        off_limits("https://www.example.com/", board),
        Some(NOT_ITS_SPACE)
    );
    assert_eq!(off_limits("", "https://www.example.com/"), None);
    assert_eq!(off_limits("https://www.example.com/", ""), None);
}

/// A tab whose first page hasn't committed has no address, or the empty start page's, and holds
/// nothing of anyone's: it's the core's to wait for and check once its page commits, as main's
/// page tools always did. Past that start page, anything but http(s) is off limits.
#[test]
fn a_tab_yet_to_commit_its_first_page_is_the_cores_to_check() {
    assert_eq!(off_limits("", ""), None);
    assert_eq!(off_limits("about:blank", ""), None);
    assert_eq!(off_limits("about:blank#top", ""), None);
    assert_eq!(off_limits("about:blank", "https://www.example.com/"), None);
    assert_eq!(off_limits("about:blankish", ""), Some(NOT_WEB_PAGE));
    assert_eq!(off_limits("about:srcdoc", ""), Some(NOT_WEB_PAGE));
    assert_eq!(off_limits("", "chrome://settings/"), Some(NOT_WEB_PAGE));
    assert_eq!(off_limits("data:text/html,hi", ""), Some(NOT_WEB_PAGE));
}

/// The log names a web page by its site, and any other page by its kind alone, as a file's path,
/// an extension's id or anything else in its address may be the user's.
#[test]
fn the_log_names_a_page_off_the_web_by_its_kind() {
    assert_eq!(site("https://www.example.com/a?b"), "example.com");
    assert_eq!(
        site("chrome://board/0123456789abcdef0123456789abcdef"),
        "board"
    );
    assert_eq!(site("chrome://settings/passwords"), "settings");
    assert_eq!(site("chrome://history/?q=kitchen"), "browser");
    assert_eq!(site("file:///Users/someone/notes.txt"), "file");
    assert_eq!(site("file://server/share/notes.txt"), "file");
    assert_eq!(
        site("chrome-extension://abcdefghijklmnopabcdefghijklmnop/options.html"),
        "extension"
    );
    assert_eq!(site("view-source:https://www.example.com/"), "page");
    assert_eq!(site("about:blank"), "");
}

/// The agent's own tab before its first page commits, as one open_tab just made, one a page opened
/// with a link or window.open, or a blank popup its opener is about to navigate, shows no address
/// or the empty start page's. A call on it goes to the core, which waits for the page as it always
/// has, and list_tabs, run beside it, still lists it.
#[gpui::test]
fn an_agents_tab_yet_to_commit_its_first_page_is_the_cores_to_wait_for(cx: &mut TestAppContext) {
    for url in ["", "about:blank"] {
        let (agents, browser, _) = connected(cx);
        browser.tabs.borrow_mut().push((21, 2, "", url));
        let snapshot = call(&agents, "claude-code", "snapshot", json!({ "tab": 21 }), cx);
        let listed = call(&agents, "claude-code", "list_tabs", json!({}), cx);
        cx.run_until_parked();
        let listed = listed.recv().unwrap().text;
        assert!(
            listed.contains("21  "),
            "list_tabs left out {url:?}: {listed}"
        );
        assert!(
            browser.pages.borrow().iter().any(|page| page.0 == 21),
            "the snapshot of a tab on {url:?} didn't reach the core: {:?}",
            snapshot.try_recv()
        );
        let _ = call(
            &agents,
            "claude-code",
            "read_page",
            json!({ "tab": 21 }),
            cx,
        );
        assert!(
            browser.reads.borrow().iter().any(|read| read.0 == 21),
            "read_page didn't ask the core to read a tab on {url:?}"
        );
    }
}

/// A tab of the user's, in their own Space, gets the core's answer for any tab not the agent's,
/// whatever its page: refusing it for being off the web would tell the agent what the user has
/// open.
#[gpui::test]
fn a_users_tab_off_the_web_is_answered_as_any_tab_not_the_agents(cx: &mut TestAppContext) {
    for url in [
        "chrome://settings/passwords",
        "file:///home/user/plan.txt",
        "data:text/html,words",
    ] {
        let (agents, browser, _) = connected(cx);
        browser.tabs.borrow_mut().push((31, 1, "Passwords", url));
        for (tool, arguments) in [
            ("read_page", json!({ "tab": 31 })),
            ("snapshot", json!({ "tab": 31 })),
            (
                "request",
                json!({ "tab": 31, "url": "https://example.org/" }),
            ),
            ("close_tab", json!({ "tab": 31 })),
        ] {
            let answer = call(&agents, "claude-code", tool, arguments, cx);
            cx.run_until_parked();
            assert_eq!(
                answer.recv().unwrap(),
                ToolResult::error(NOT_ITS_SPACE),
                "{tool} on a user's tab at {url}"
            );
            // The core's answer, which its activity log has.
            assert!(
                browser.checked.borrow_mut().drain(..).next().is_some(),
                "{tool} on a user's tab at {url} didn't ask the core"
            );
        }
    }
}

/// A request from a tab not the agent's, the user's or another agent's, gets the core's answer for
/// the tab alone, before any of its requests is read, as every other tool does: an answer for each
/// request would tell the agent they got as far as the tab's site.
#[gpui::test]
fn a_request_from_a_tab_not_the_agents_is_refused_for_the_tab(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    browser.agent_spaces.borrow_mut().insert("codex".into(), 3);
    (browser.tabs.borrow_mut()).push((30, 3, "Rust", "https://www.rust-lang.org/"));
    browser.checked.borrow_mut().clear();
    for (tab, site) in [(10, "en.wikipedia.org"), (30, "rust-lang.org")] {
        let requests = json!({ "tab": tab, "requests": [{ "method": "GET", "path": "/api/me" }] });
        let answer = call(&agents, "claude-code", "request", requests, cx);
        cx.run_until_parked();
        assert_eq!(
            answer.recv().unwrap(),
            ToolResult::error(NOT_ITS_SPACE),
            "request from tab {tab}"
        );
        assert_eq!(
            browser
                .checked
                .borrow_mut()
                .drain(..)
                .collect::<Vec<String>>(),
            ["request"],
            "the core didn't check tab {tab} once"
        );
        assert_eq!(
            log(&agents, "claude-code", cx)[0],
            ("request".into(), site.into(), NOT_ITS_SPACE.into())
        );
    }
    assert!(browser.requests.borrow().is_empty(), "a request was sent");
}

/// A change goes only with the user's answer, whatever the UI's list of the agent's tabs and the
/// core's check say: a tab the core allows that the UI doesn't list, as a popup the agent's tab
/// opened might be, gets no question, and its change is refused in the core's words for a tab not
/// the agent's and sent nowhere; a read in the same call still goes.
#[gpui::test]
fn a_change_from_a_tab_the_ui_doesnt_list_is_refused_unasked(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    (browser.tabs.borrow_mut()).push((21, 2, "Tracker", "https://tracker.example/"));
    browser.unlisted.borrow_mut().insert(21);
    let requests = json!({ "tab": 21, "requests": [
        { "method": "GET", "path": "/api/me" },
        { "method": "POST", "path": "/api/items", "body": { "title": "Milk" },
          "what": "Add milk to the list." },
    ] });
    let answer = call(&agents, "claude-code", "request", requests, cx);
    cx.run_until_parked();
    assert_eq!(shown(&agents, cx), None, "the change asked");
    let sent = browser.requests.borrow().clone();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].2.method, "GET");
    let (request, response) = (
        sent[0].1,
        request::Response {
            status: 200,
            ..Default::default()
        },
    );
    agents.update(cx, |agents, cx| agents.request_done(request, response, cx));
    cx.run_until_parked();
    let answered = answer.recv().unwrap();
    assert!(
        format!("{answered:?}").contains(&format!("POST /api/items: {NOT_ITS_SPACE}")),
        "{answered:?}"
    );
    assert_eq!(browser.requests.borrow().len(), 1, "the change was sent");
}

/// Once read, a page still on the empty start page is no web page, as the core's page tools find,
/// and a tab that became a Board meanwhile is answered as a Board is everywhere else.
#[gpui::test]
fn read_page_answers_for_the_page_it_read(cx: &mut TestAppContext) {
    for (url, refused) in [
        ("about:blank", NOT_WEB_PAGE),
        (
            "chrome://board/0123456789abcdef0123456789abcdef",
            NOT_ITS_SPACE,
        ),
    ] {
        let (agents, browser, _) = connected(cx);
        let answer = call(
            &agents,
            "claude-code",
            "read_page",
            json!({ "tab": 20 }),
            cx,
        );
        let (_, request) = browser.reads.borrow()[0];
        browser.tabs.borrow_mut()[1] = (20, 2, "Kitchen plans", url);
        agents.update(cx, |agents, cx| {
            agents.text_read(request, Some("Kitchen plans\nshelves".into()), cx)
        });
        assert_eq!(answer.recv().unwrap(), ToolResult::error(refused), "{url}");
    }
}

/// read_page answers only for an http(s) page, as the page tools do: the core reads any page, so
/// one the tab went to while it was read, such as the browser's own, gives the agent nothing.
#[gpui::test]
fn read_page_gives_nothing_of_a_page_off_the_web(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 20 }),
        cx,
    );
    let (_, request) = browser.reads.borrow()[0];
    browser.tabs.borrow_mut()[1] = (20, 2, "Settings", "chrome://settings/");
    agents.update(cx, |agents, cx| {
        agents.text_read(request, Some("Settings\nYou and Google".into()), cx)
    });
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error(
            "The tab's page isn't an http or https page, the only pages agents may use."
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
        ..TabInfo::default()
    };
    let text = format!("{}ab", "é".repeat(tools::MAX_TEXT));
    let answer = tools::page_text(&tab, &text, 0);
    assert!(answer.ends_with("[The page goes on: read_page with from 10000 reads the next part.]"));
    assert!(answer.chars().count() < tools::MAX_TEXT + 200);
    // The next part starts where the last ended, in characters, not bytes.
    let next = tools::page_text(&tab, &text, tools::MAX_TEXT);
    assert!(next.ends_with("\n\nab"), "{next}");
    let past = tools::page_text(&tab, &text, 20_000);
    assert!(past.ends_with("[The page's text is 10002 characters, so none starts at 20000.]"));
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
            connection: 1,
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

/// Turning other apps' agents off ends every live one's sessions and answers the calls waiting,
/// and keeps the apps connected for when it's on again.
#[gpui::test]
fn turning_agents_off_ends_their_sessions(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    let waiting = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 20 }),
        cx,
    );
    agents.update(cx, |agents, cx| agents.set_other_apps(false, cx));
    assert!(waiting.recv().is_err());
    assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
    assert!(agents.read_with(cx, |agents, _| agents.is_connected(Client::ClaudeCode)));
}

/// The two switches are two: flipping one, from either state of both, never moves the other.
#[gpui::test]
fn flipping_one_switch_never_touches_the_other(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    let both = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| (agents.threads(), agents.other_apps()))
    };
    for threads in [false, true] {
        for others in [false, true] {
            agents.update(cx, |agents, cx| {
                agents.set_threads(threads, cx);
                agents.set_other_apps(others, cx);
            });
            agents.update(cx, |agents, cx| agents.set_threads(!threads, cx));
            assert_eq!(
                both(cx),
                (!threads, others),
                "threads flipped from {threads}"
            );
            agents.update(cx, |agents, cx| agents.set_threads(threads, cx));
            agents.update(cx, |agents, cx| agents.set_other_apps(!others, cx));
            assert_eq!(
                both(cx),
                (threads, !others),
                "other apps flipped from {others}"
            );
        }
    }
}

/// Only the user's click sets either switch: no tool, of an outside app's or a thread's, is
/// named for a switch or for Settings.
#[test]
fn no_tool_sets_either_switch() {
    for agent in ["claude-code", "task-1"] {
        for name in tools::names(agent) {
            for word in ["switch", "setting", "other_apps", "threads", "listen"] {
                assert!(!name.contains(word), "{agent} has {name}");
            }
        }
    }
}

/// With other apps' agents off, an app that says hello or calls a tool is never at work, so no
/// row of Apps that connect says one is.
#[gpui::test]
fn with_other_apps_off_no_app_is_at_work(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    agents.update(cx, |agents, cx| {
        agents.set_threads(true, cx);
        agents.set_other_apps(false, cx);
    });
    let hello = Event::Hello {
        agent: "claude-code".into(),
        connection: 1,
        client: "claude-code".into(),
    };
    agents.update(cx, |agents, cx| agents.handle(hello, cx));
    let answer = call(&agents, "claude-code", "list_tabs", json!({}), cx);
    cx.run_until_parked();
    assert!(
        !answer.try_recv().is_ok_and(|answer| !answer.error),
        "a call was done while off"
    );
    for client in Client::ALL {
        let (live, connecting) = agents.read_with(cx, |agents, _| {
            (agents.is_live(client), agents.is_connecting(client))
        });
        assert!(!live && !connecting, "{client:?} shows at work while off");
    }
}

/// What's saved: the switches and each agent's token, in a file only the user can read, which a
/// later launch loads, starting the server if either was on.
#[gpui::test]
fn the_switch_and_the_agents_come_back(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("agents-test-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Agents");
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(path.clone()), cx);
    agents.update(cx, |agents, cx| agents.connect(Client::Codex, cx));
    cx.run_until_parked();
    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    let saved = std::fs::read_to_string(&path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    agents.update(cx, |agents, cx| agents.set_threads(false, cx));

    let later = self::agents(&browser, &configs, None, cx);
    later.update(cx, |agents, cx| agents.load(&saved, cx));
    assert!(later.read_with(cx, |agents, _| agents.threads() && agents.server.is_some()));
    assert!(later.read_with(cx, |agents, _| agents.is_connected(Client::Codex)));
    let token = configs.connected.lock().unwrap()[0].1.token.clone();
    let tokens = later.read_with(cx, |agents, _| agents.tokens.lock().unwrap().clone());
    assert_eq!(tokens.get(&token).map(String::as_str), Some("codex"));
    later.update(cx, |agents, cx| agents.load("", cx));
    assert!(
        later.read_with(cx, |agents, _| !agents.listening()),
        "a lost file left agents on"
    );
    std::fs::remove_dir_all(dir).ok();
}

/// The tree the core gives for tab 20's page: a heading, a field, a password field and a button
/// that orders.
fn shop_tree() -> String {
    shop_tree_named("Place order")
}

/// The shop's tree with its button named `button`.
fn shop_tree_named(button: &str) -> String {
    json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" },
          "childIds": ["2", "3", "4", "5"] },
        { "nodeId": "2", "role": { "value": "heading" }, "name": { "value": "Basket" } },
        { "nodeId": "3", "role": { "value": "textbox" }, "name": { "value": "Coupon" },
          "backendDOMNodeId": 31 },
        { "nodeId": "4", "role": { "value": "textbox" }, "name": { "value": "Password" },
          "backendDOMNodeId": 41, "password": true },
        { "nodeId": "5", "role": { "value": "button" }, "name": { "value": button },
          "backendDOMNodeId": 51 },
    ]})
    .to_string()
}

/// navigate answers with the page's text when the agent asks for it, to read the page, once the
/// core has read it, and the page's refs stay for what the agent does next (#3657). open_tab takes
/// the same answer.
#[gpui::test]
fn navigate_answers_with_the_text_it_asks_for(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let wrong = call(
        &agents,
        "claude-code",
        "navigate",
        json!({ "tab": 20, "url": "iana.org", "answer": "page" }),
        cx,
    );
    assert_eq!(
        wrong.recv().unwrap(),
        ToolResult::error("answer is outline, text or none.")
    );
    let answer = call(
        &agents,
        "claude-code",
        "navigate",
        json!({ "tab": 20, "url": "iana.org", "answer": "text" }),
        cx,
    );
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, shop_tree(), None, cx)
    });
    assert!(
        answer.try_recv().is_err(),
        "it answered before the text came"
    );
    let (tab, read) = *browser.reads.borrow().last().unwrap();
    assert_eq!(tab, 20);
    agents.update(cx, |agents, cx| {
        agents.text_read(read, Some("Basket\nTwo items".into()), cx)
    });
    let text = answer.recv().unwrap();
    assert!(!text.error, "{}", text.text);
    assert!(
        text.text.ends_with("\n\nBasket\nTwo items"),
        "{}",
        text.text
    );
    assert!(!text.text.contains("[e"), "{}", text.text);
    let pages = browser.pages.borrow().len();
    let click = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    assert_eq!(browser.pages.borrow().len(), pages + 1);
    assert!(!page_done(&agents, &browser, click, shop_tree(), cx).error);
}

/// Every answer is 12,000 characters at most, whatever the tool, and says where it's cut.
#[test]
fn an_answer_is_cut_at_its_limit() {
    let long = ToolResult::text("é".repeat(30_000));
    assert_eq!(long.text.chars().count(), tools::MAX_ANSWER);
    assert!(
        long.text
            .ends_with("[The answer is cut here, at its limit of 12,000 characters.]")
    );
    let short = "é".repeat(tools::MAX_ANSWER);
    assert_eq!(ToolResult::text(short.clone()).text, short);
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
    agents.update(cx, |agents, cx| agents.page_done(request, text, None, cx));
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
        // The search field has the keyboard, so Enter there pays for nothing and asks nothing.
        let page = checkout_tree(Some("7"));
        assert!(!page_done(&agents, &browser, answer, page, cx).error);
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
    // It answers once the page has settled, as an action does.
    assert_eq!((tab, action), (30, PageAction::Quiet));
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
        agents.page_done(request, "It's gone.".into(), Some(Failure::Other), cx)
    });
    assert_eq!(answer.recv().unwrap(), ToolResult::error("It's gone."));
}

/// A click that landed on a select, a date or time field or a color field of an agent's page, whose
/// list or picker a click doesn't open there, is told what to do instead: once, after the click's
/// usual answer and before the page. The core names the field; a click it names none for, and a
/// word the app doesn't know, are told nothing more.
#[gpui::test]
fn a_click_is_told_when_it_landed_on_a_list_that_stays_shut(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let form = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Ticket" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "combobox" }, "name": { "value": "Status" },
          "backendDOMNodeId": 61 },
    ]});
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, form.to_string(), cx);
    let mut clicked = |shut: Option<&str>, busy: bool| {
        let mut tree = form.clone();
        if let Some(field) = shut {
            tree["shut"] = json!(field);
        }
        if busy {
            tree["busy"] = json!(true);
        }
        let click = json!({ "tab": 20, "ref": "e1" });
        let answer = call(&agents, "claude-code", "click", click, cx);
        let result = page_done(&agents, &browser, answer, tree.to_string(), cx);
        assert!(!result.error, "{}", result.text);
        result.text
    };
    let usual = "Clicked combobox “Status”.";
    let lines = [
        (
            "select",
            "A click doesn't open this list. Use select with the option's text.",
        ),
        (
            "date_time",
            "A click doesn't open a calendar here. Type the date or time into the field.",
        ),
        (
            "color",
            "This color field can't be set by a click or by typing. If it has to be set, hand the \
             tab to the user with hand_off.",
        ),
    ];
    for (field, line) in lines {
        let text = clicked(Some(field), false);
        assert!(text.starts_with(&format!("{usual} {line}\n\n")), "{text}");
        assert_eq!(text.matches(line).count(), 1, "{text}");
    }
    // After everything the click's answer says of the page, too.
    let text = clicked(Some("select"), true);
    assert!(
        text.starts_with(&format!("{usual} {BUSY} {}\n\n", lines[0].1)),
        "{text}"
    );
    for shut in [None, Some("slider"), Some("")] {
        let text = clicked(shut, false);
        assert!(text.starts_with(&format!("{usual}\n\n")), "{text}");
        for (_, line) in lines {
            assert!(!text.contains(line), "{text}");
        }
    }
}

/// A click on a tab the user holds never reaches the page, so it is told none of the lines for a
/// list that stays shut: there the list opens, for the user.
#[gpui::test]
fn a_held_tabs_click_is_told_nothing_of_a_shut_list(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    let pages = browser.pages.borrow().len();
    let click = json!({ "tab": 20, "ref": "e1" });
    let refused = call(&agents, "claude-code", "click", click, cx)
        .recv()
        .unwrap();
    assert!(refused.error);
    assert_eq!(refused.text, HELD);
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "the click reached the page"
    );
    for field in ["select", "date_time", "color"] {
        assert!(!refused.text.contains(shut_line(field).unwrap()));
    }
}

/// A tab open_tab opened stays open though its page then fails, as one still loading does, so the
/// answer names it for the agent to look at later; another action's failure is the core's alone
/// (#1461).
#[gpui::test]
fn a_tab_opened_still_loading_is_named_in_the_error(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let loading = "example.com is still loading after 15 s. Wait, or try again.";
    let answer = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "example.com" }),
        cx,
    );
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, loading.into(), Some(Failure::Loading), cx)
    });
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::failed(
            tools::Kind::StillLoading,
            format!("Opened tab 30. {loading}")
        )
    );

    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 30 }), cx);
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, loading.into(), Some(Failure::Loading), cx)
    });
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::failed(tools::Kind::StillLoading, loading)
    );
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
    // As the core answers: the tree, at rest.
    json!({ "nodes": nodes, "busy": false }).to_string()
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
             Changed:\n- combobox \"Size\" [e4] (expanded): now expanded"
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
        scrolled.contains("Changed:\n- combobox \"Size\" [e4]: now collapsed\n\nGone:\n- listbox"),
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
        agents.page_done(request, size_picker(false, 2), None, cx)
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

/// select chooses an option in a field by the option's text: the core gets the field's node and
/// the text, and the answer says what the field holds now, as the core names the option, or that
/// it doesn't show it. It asks the core as select_option, is a run's step, and asks the user first
/// as a click would.
#[gpui::test]
fn an_agent_chooses_an_option_by_its_text(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let form = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Ticket" },
          "childIds": ["2", "3"] },
        { "nodeId": "2", "role": { "value": "combobox" }, "name": { "value": "Status" },
          "backendDOMNodeId": 61 },
        { "nodeId": "3", "role": { "value": "combobox" }, "name": { "value": "Payment" },
          "backendDOMNodeId": 71 },
    ]});
    let answered = |chosen: Option<&str>| {
        let mut tree = form.clone();
        if let Some(option) = chosen {
            tree["chosen"] = json!(option);
        }
        tree.to_string()
    };
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    let outline = page_done(&agents, &browser, answer, form.to_string(), cx).text;
    assert!(outline.contains("- combobox \"Status\" [e1]"), "{outline}");

    // The core matches in any case, and names the option as the page does.
    let choose = json!({ "tab": 20, "ref": "e1", "option": " closed " });
    let answer = call(&agents, "claude-code", "select", choose, cx);
    let (tab, _, action, element, text) = browser.pages.borrow().last().cloned().unwrap();
    assert_eq!(
        (tab, action, element, text.as_str()),
        (20, PageAction::Select, 61, "closed")
    );
    let held = page_done(&agents, &browser, answer, answered(Some("Closed")), cx);
    assert!(
        held.text
            .starts_with("The combobox “Status” now holds “Closed”.\n\nTab 20"),
        "{}",
        held.text
    );
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "select_option");

    let choose = json!({ "tab": 20, "ref": "e1", "option": "51" });
    let answer = call(&agents, "claude-code", "select", choose, cx);
    let missed = page_done(&agents, &browser, answer, answered(None), cx);
    assert!(
        missed.text.starts_with(
            "Chose “51” in the combobox “Status”. It doesn't show it as chosen, so the page may \
             have changed it back.\n\nTab 20"
        ),
        "{}",
        missed.text
    );

    // A line break in the option goes to the core as it was asked, and the answer says it on one
    // line.
    let choose = json!({ "tab": 20, "ref": "e1", "option": "5\n1" });
    let answer = call(&agents, "claude-code", "select", choose, cx);
    let (.., text) = browser.pages.borrow().last().cloned().unwrap();
    assert_eq!(text, "5\n1");
    let missed = page_done(&agents, &browser, answer, answered(None), cx);
    assert!(
        missed
            .text
            .starts_with("Chose “5 1” in the combobox “Status”. It doesn't"),
        "{}",
        missed.text
    );

    let pages = browser.pages.borrow().len();
    for wrong in [
        json!({ "tab": 20, "ref": "e1" }),
        json!({ "tab": 20, "ref": "e1", "option": "  " }),
        json!({ "tab": 20, "option": "51" }),
    ] {
        let refused = call(&agents, "claude-code", "select", wrong, cx)
            .recv()
            .unwrap();
        assert_eq!(
            refused,
            ToolResult::error("select needs a ref and an option.")
        );
    }
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "a select without its option reached the core"
    );

    let step = json!({ "action": "select", "ref": "e1", "option": "51" });
    let steps = json!({ "tab": 20, "steps": [step] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    let (_, request, action, ..) = browser.pages.borrow().last().cloned().unwrap();
    assert_eq!(action, PageAction::Select);
    agents.update(cx, |agents, cx| {
        agents.page_done(request, answered(Some("51")), None, cx)
    });
    let ran = run.recv().unwrap();
    assert!(
        ran.text
            .starts_with("Ran 1 steps.\n1. The combobox “Status” now holds “51”.\n\nTab 20"),
        "{}",
        ran.text
    );

    let pages = browser.pages.borrow().len();
    // The refusal says the option on one line, its line break a space.
    let paying = json!({ "tab": 20, "ref": "e2", "option": "Pay\u{2028}now" });
    let answer = call(&agents, "claude-code", "select", paying, cx);
    let what = "choose “Pay now” in “Payment” on example.com";
    let at = "choose “Pay now” in “Payment”";
    refused(&agents, &browser, answer, what, Why::Order, at, cx);
    assert_eq!(browser.pages.borrow().len(), pages, "it chose");
}

/// A checkout's page, with what has the keyboard `focused` by its node's id: a card form with its
/// Pay now button and a box to save the card, and a search form with nothing that pays.
fn checkout_tree(focused: Option<&str>) -> String {
    let mut nodes = vec![
        json!({ "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Checkout" },
          "childIds": ["2", "6"] }),
        json!({ "nodeId": "2", "role": { "value": "form" }, "name": { "value": "" },
          "backendDOMNodeId": 60, "childIds": ["3", "4", "5"] }),
        json!({ "nodeId": "3", "role": { "value": "textbox" }, "name": { "value": "Card number" },
          "backendDOMNodeId": 61 }),
        json!({ "nodeId": "4", "role": { "value": "button" }, "name": { "value": "Pay now" },
          "backendDOMNodeId": 62 }),
        json!({ "nodeId": "5", "role": { "value": "checkbox" },
          "name": { "value": "Save this card" }, "backendDOMNodeId": 63 }),
        json!({ "nodeId": "6", "role": { "value": "form" }, "name": { "value": "" },
          "backendDOMNodeId": 70, "childIds": ["7", "8"] }),
        json!({ "nodeId": "7", "role": { "value": "searchbox" }, "name": { "value": "Search" },
          "backendDOMNodeId": 71 }),
        json!({ "nodeId": "8", "role": { "value": "button" }, "name": { "value": "Go" },
          "backendDOMNodeId": 72 }),
    ];
    focusing(nodes, focused)
}

/// The tree `nodes` with the node `focused` holding the keyboard, if any.
fn focusing(mut nodes: Vec<Value>, focused: Option<&str>) -> String {
    for node in &mut nodes {
        if Some(node["nodeId"].as_str().unwrap()) == focused {
            let focused = json!({ "type": "booleanOrUndefined", "value": true });
            node["properties"] = json!([{ "name": "focused", "value": focused }]);
        }
    }
    json!({ "nodes": nodes }).to_string()
}

/// A checkout whose Pay now button sits outside the card's form, as a sticky footer's does, tied
/// to it by the button's form attribute, which the tree doesn't show.
fn footer_tree(focused: Option<&str>) -> String {
    let nodes = vec![
        json!({ "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Checkout" },
          "childIds": ["2", "4"] }),
        json!({ "nodeId": "2", "role": { "value": "form" }, "name": { "value": "" },
          "backendDOMNodeId": 60, "childIds": ["3"] }),
        json!({ "nodeId": "3", "role": { "value": "textbox" }, "name": { "value": "Card number" },
          "backendDOMNodeId": 61 }),
        json!({ "nodeId": "4", "role": { "value": "button" }, "name": { "value": "Pay now" },
          "backendDOMNodeId": 62 }),
    ];
    focusing(nodes, focused)
}

/// A key press that activates a control is refused as a click on it is, on a site that sends no
/// messages. Enter in a field or on a checkbox submits its form, whose Pay now button a click on
/// would be refused, as typing with submit is, as is a Pay now button outside any form. Space or
/// Enter on the focused Pay now button is a click on it, wherever the last answer left the
/// keyboard. Enter in a form with nothing that pays, Space in a field or on a checkbox and Tab go
/// (GHSA-hqw3-jp9p-3rhj).
#[gpui::test]
fn a_key_press_that_activates_a_control_is_refused_as_a_click_on_it(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let look = |tree: String, cx: &mut TestAppContext| {
        let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
        let outline = page_done(&agents, &browser, answer, tree, cx).text;
        assert!(outline.contains("- button \"Pay now\" [e2]"), "{outline}");
    };
    let snapshot =
        |focused: Option<&str>, cx: &mut TestAppContext| look(checkout_tree(focused), cx);
    // Calls `tool` and checks it's refused as `what` before anything reaches the page.
    let asks = |tool: &str, arguments: Value, what: &str, cx: &mut TestAppContext| {
        let pages = browser.pages.borrow().len();
        let answer = call(&agents, "claude-code", tool, arguments, cx);
        // The line's words are the same without the tab's site, which it shows apart (#3836).
        let at = what.replace(" on example.com", "");
        refused(&agents, &browser, answer, what, Why::Order, &at, cx);
        assert_eq!(
            browser.pages.borrow().len(),
            pages,
            "{what} reached the page"
        );
    };
    // Calls `tool`, which goes to the page without a question, and the page answers with `tree`.
    let answered = |tool: &str, arguments: Value, tree: String, cx: &mut TestAppContext| {
        let answer = call(&agents, "claude-code", tool, arguments.clone(), cx);
        assert_eq!(shown(&agents, cx), None, "{tool} {arguments} asked");
        assert!(!page_done(&agents, &browser, answer, tree, cx).error);
    };
    let goes = |tool: &str, arguments: Value, cx: &mut TestAppContext| {
        answered(tool, arguments, checkout_tree(None), cx)
    };

    snapshot(None, cx);
    let card = json!({ "tab": 20, "ref": "e1", "text": "4242", "submit": true });
    let what = "press Enter, which may click “Pay now” on example.com, after typing “4242” into “Card number”";
    asks("type", card, what, cx);
    goes(
        "type",
        json!({ "tab": 20, "ref": "e4", "text": "socks", "submit": true }),
        cx,
    );
    goes(
        "type",
        json!({ "tab": 20, "ref": "e1", "text": "4242" }),
        cx,
    );

    snapshot(Some("3"), cx);
    let enter = json!({ "tab": 20, "key": "Enter" });
    asks(
        "press_key",
        enter.clone(),
        "press Enter, which may click “Pay now” on example.com",
        cx,
    );
    snapshot(Some("3"), cx);
    goes("press_key", json!({ "tab": 20, "key": "Space" }), cx);
    snapshot(Some("3"), cx);
    goes("press_key", json!({ "tab": 20, "key": "Tab" }), cx);

    snapshot(Some("4"), cx);
    let space = json!({ "tab": 20, "key": "Space" });
    asks(
        "press_key",
        space,
        "press Space to click “Pay now” on example.com",
        cx,
    );
    snapshot(Some("4"), cx);
    asks(
        "press_key",
        enter.clone(),
        "press Enter to click “Pay now” on example.com",
        cx,
    );

    // Enter on a checkbox submits its form too, so it asks as Enter in the form's field does, and
    // Space on it checks it.
    snapshot(Some("5"), cx);
    asks(
        "press_key",
        enter.clone(),
        "press Enter, which may click “Pay now” on example.com",
        cx,
    );
    snapshot(Some("5"), cx);
    goes("press_key", json!({ "tab": 20, "key": "Space" }), cx);

    // The answer to the action before the press says where the keyboard went, with no look at the
    // page between: Tab's, and a look at one element's part, whose page the core outlined whole.
    snapshot(Some("7"), cx);
    let tab = json!({ "tab": 20, "key": "Tab" });
    answered("press_key", tab, checkout_tree(Some("4")), cx);
    asks(
        "press_key",
        enter.clone(),
        "press Enter to click “Pay now” on example.com",
        cx,
    );
    snapshot(Some("7"), cx);
    let part = json!({ "tab": 20, "ref": "e1" });
    answered("snapshot", part, checkout_tree(Some("4")), cx);
    asks(
        "press_key",
        enter.clone(),
        "press Enter to click “Pay now” on example.com",
        cx,
    );

    // A Pay now button outside the card's form may be the one its Enter clicks.
    look(footer_tree(Some("3")), cx);
    asks(
        "press_key",
        enter.clone(),
        "press Enter, which may click “Pay now” on example.com",
        cx,
    );

    // Enter in the search form clicks Go, never the card form's Pay now.
    snapshot(Some("7"), cx);
    goes("press_key", enter.clone(), cx);
    // The page itself has the keyboard: Enter activates nothing.
    snapshot(Some("1"), cx);
    goes("press_key", enter.clone(), cx);
    // The outline doesn't say what has it, so anything on the page that pays asks.
    snapshot(None, cx);
    asks(
        "press_key",
        enter,
        "press Enter, which may click “Pay now” on example.com",
        cx,
    );
}

/// scroll with a direction turns the wheel at the middle of the page, or with a ref as well in
/// that element, such as a panel or a frame, and with a ref alone brings the element into view
/// (#1445).
#[gpui::test]
fn an_agent_scrolls_the_page_in_an_element_or_to_it(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let scrolls = [
        (
            json!({ "direction": "up" }),
            PageAction::ScrollUp,
            0,
            "Scrolled up.",
        ),
        (
            json!({ "ref": "e3", "direction": "down" }),
            PageAction::ScrollDown,
            51,
            "Scrolled down in “Place order”.",
        ),
        (
            json!({ "ref": "e3" }),
            PageAction::ScrollTo,
            51,
            "Scrolled to “Place order”.",
        ),
    ];
    for (mut arguments, action, element, note) in scrolls {
        arguments["tab"] = json!(20);
        let answer = call(&agents, "claude-code", "scroll", arguments, cx);
        let (_, _, sent, to, _) = browser.pages.borrow().last().cloned().unwrap();
        assert_eq!((sent, to), (action, element));
        let scrolled = page_done(&agents, &browser, answer, shop_tree(), cx);
        assert!(scrolled.text.starts_with(note), "{}", scrolled.text);
    }
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
        ToolResult::failed(
            tools::Kind::NotWebPage,
            "Agents may open only http and https pages."
        )
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
    let refused = ToolResult::failed(
        tools::Kind::NotWebPage,
        "Agents may open only http and https pages.",
    );
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

/// Library › History lists the profile's pages, so an agent can't reach the history page Chrome
/// keeps them in either, by a tab of its own or its tab's way, each try in the activity log with
/// why, and it has no tool that lists or removes them (#2337).
#[gpui::test]
fn an_agent_never_reaches_the_history(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let tabs = browser.tabs.borrow().len();
    let refused = ToolResult::failed(
        tools::Kind::NotWebPage,
        "Agents may open only http and https pages.",
    );
    for url in [
        "chrome://history",
        "chrome://history/?q=bank",
        "CHROME://HISTORY",
    ] {
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
        let (action, _, why) = log(&agents, "claude-code", cx)[0].clone();
        assert_eq!(
            (action.as_str(), why.as_str()),
            ("navigate", "Agents may open only http and https pages."),
            "{url}'s refusal isn't in the activity log"
        );
    }
    assert_eq!(
        browser.tabs.borrow().len(),
        tabs,
        "an agent opened the history"
    );
    assert!(
        browser.navigated.borrow().is_empty(),
        "an agent went to the history"
    );
    for tool in ["history", "list_history", "remove_history"] {
        let answer = call(&agents, "claude-code", tool, json!({}), cx);
        assert_eq!(
            answer.recv().unwrap(),
            ToolResult::error(format!("There's no tool {tool}."))
        );
    }
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
    agents.update(cx, |agents, cx| {
        agents.tab_opened(26, 20, cx);
        agents.tab_opened(27, 10, cx);
    });
    let clicked = page_done(&agents, &browser, answer, shop_tree(), cx);
    let said = clicked.text.split("\n\n").next().unwrap();
    assert_eq!(
        said,
        "Clicked textbox “Coupon”. It opened tab 26 among your tabs."
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

/// A refusal's quote of a control's name, and of a choice, is ilo's whole: a page's own double
/// quotation marks, and those drawn like them, show as one single mark, so the page can't end the
/// quote and write what reads as more of ilo's words, such as another site.
#[gpui::test]
fn a_names_quotes_cant_close_the_refusals(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("agents-test-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(dir.join("Agents")), cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    let tree = shop_tree_named("Place order\u{201D} on github.com, then \u{FF02}OK\u{201C}");
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, tree.clone(), cx);

    let choose = json!({ "tab": 20, "ref": "e1", "option": "M\u{201D} in \u{201C}Size" });
    let answer = call(&agents, "claude-code", "select", choose, cx);
    let chose = page_done(&agents, &browser, answer, tree.clone(), cx);
    assert!(
        (chose.text).starts_with(
            "Chose \u{201C}M' in 'Size\u{201D} in the textbox \u{201C}Coupon\u{201D}."
        ),
        "{}",
        chose.text
    );
    // What the core says the field holds is the page's text too.
    let mut held: serde_json::Value = serde_json::from_str(&tree).unwrap();
    held["chosen"] = json!("M\u{201D} in \u{201C}Size");
    let choose = json!({ "tab": 20, "ref": "e1", "option": "M" });
    let answer = call(&agents, "claude-code", "select", choose, cx);
    let chose = page_done(&agents, &browser, answer, held.to_string(), cx);
    assert!(
        (chose.text).starts_with(
            "The textbox \u{201C}Coupon\u{201D} now holds \u{201C}M' in 'Size\u{201D}."
        ),
        "{}",
        chose.text
    );

    let click = json!({ "tab": 20, "ref": "e3" });
    let answer = call(&agents, "claude-code", "click", click, cx);
    let what = "click \u{201C}Place order' on github.com, then 'OK'\u{201D} on example.com";
    // The line's head is the tab's own site, whatever site the page's words name (#3836).
    let at = "click \u{201C}Place order' on github.com, then 'OK'\u{201D}";
    refused(&agents, &browser, answer, what, Why::Order, at, cx);
    assert_eq!(stops(&agents, cx)[0].site, "example.com");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A control's name in a refusal loses its direction overrides and isolates, so the page can't
/// draw ilo's own words after the quote, the site's name among them, in an order of its own.
#[gpui::test]
fn a_names_direction_marks_dont_reach_the_refusals(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("agents-test-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(dir.join("Agents")), cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    let tree = shop_tree_named("Place order\u{202E}moc.buhtig\u{2066} now");
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, tree, cx);

    let click = json!({ "tab": 20, "ref": "e3" });
    let answer = call(&agents, "claude-code", "click", click, cx);
    let what = "click \u{201C}Place ordermoc.buhtig now\u{201D} on example.com";
    let at = "click \u{201C}Place ordermoc.buhtig now\u{201D}";
    let told = refused(&agents, &browser, answer, what, Why::Order, at, cx);
    assert!(
        !told.text.contains(['\u{202E}', '\u{2066}']),
        "{:?}",
        told.text
    );

    // A key press's question names the control, the field and the typed text, none with them.
    let mut tree: Value = serde_json::from_str(&checkout_tree(None)).unwrap();
    for node in tree["nodes"].as_array_mut().unwrap() {
        let name = match node["name"]["value"].as_str().unwrap() {
            "Card number" => "Card \u{202E}\"number\u{2066}",
            "Pay now" => "Pay now \"\u{202E}moc.buhtig\u{2066}",
            _ => continue,
        };
        node["name"]["value"] = json!(name);
    }
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    let outline = page_done(&agents, &browser, answer, tree.to_string(), cx).text;
    let card = outline
        .lines()
        .find(|line| line.contains("textbox") && line.contains("Card"))
        .and_then(|line| {
            line.rsplit_once('[')
                .map(|(_, at)| at.trim_end_matches(']'))
        })
        .unwrap_or_else(|| panic!("no card field in {outline}"))
        .to_string();
    let typed = json!({ "tab": 20, "ref": card, "text": "42\u{202E}42", "submit": true });
    let answer = call(&agents, "claude-code", "type", typed, cx);
    let what = "press Enter, which may click \u{201C}Pay now 'moc.buhtig\u{201D} on example.com, \
                after typing \u{201C}4242\u{201D} into \u{201C}Card 'number\u{201D}";
    let at = what.replace(" on example.com", "");
    let told = refused(&agents, &browser, answer, what, Why::Order, &at, cx);
    assert!(
        !told.text.contains(['\u{202E}', '\u{2066}', '"']),
        "{:?}",
        told.text
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An order is refused at Work on pages, at once and each time, and goes once the user gives Full
/// access, which settles the refusal.
#[gpui::test]
fn an_order_is_refused_until_the_user_gives_full_access(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
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
    let what = "click “Place order” on example.com";
    let told = refused(
        &agents,
        &browser,
        order(cx),
        what,
        Why::Order,
        "click “Place order”",
        cx,
    );
    assert_eq!(
        told.text,
        "ilo stopped this: click “Place order” on example.com. It isn't part of this task's access \
         (Work on pages). Only the user can change a task's access, in ilo. Don't try another way \
         to the same end; if they change it, the same call will go through."
    );
    assert_eq!(browser.pages.borrow().len(), pages, "it clicked");
    assert!(log(&agents, "claude-code", cx).contains(&(
        "click".into(),
        "example.com".into(),
        "Placing an order isn't part of Work on pages.".into()
    )));

    // The same refusal again counts on its line rather than adding one.
    refused(
        &agents,
        &browser,
        order(cx),
        what,
        Why::Order,
        "click “Place order”",
        cx,
    );
    let shown = stops(&agents, cx);
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(shown[0].count, 2);

    give(&agents, Tier::Full, cx);
    assert!(
        stops(&agents, cx).is_empty(),
        "the raise left the refusal unsettled"
    );
    let answer = order(cx);
    assert_eq!(browser.pages.borrow().last().unwrap().3, 51);
    let clicked = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(clicked.text.starts_with("Clicked button “Place order”."));
}

/// No tool reads or sets a task's access, for an agent from outside or a task ilo runs: only the
/// user changes it.
#[test]
fn no_tool_touches_a_tasks_access() {
    for agent in ["claude-code", "task-1"] {
        let listed = tools::list(agent).to_string().to_lowercase();
        for word in [
            "tier",
            "access level",
            "full access",
            "look only",
            "work on pages",
        ] {
            assert!(!listed.contains(word), "{agent}'s tools say {word:?}");
        }
        for name in tools::names(agent) {
            assert!(
                !(name.contains("tier") || name.contains("access") || name.contains("permission")),
                "{agent} has {name}"
            );
        }
    }
}

/// No tool reads a task's card or another task's steps: a card is made only for the windows
/// (`Agents::cards` and `shown_cards`, which the windows' observer alone calls), and no tool's
/// name offers one.
#[test]
fn no_tool_reads_a_card_or_another_tasks_steps() {
    for agent in ["claude-code", "task-1"] {
        for name in tools::names(agent) {
            for word in ["card", "task", "trace", "activity", "history", "recent"] {
                assert!(!name.contains(word), "{agent} has {name}");
            }
        }
    }
}

/// Every tool an agent from outside or a ⌘T task is offered, and every step run_steps takes, is in
/// exactly one class the tier table judges, so a new one fails here until it's placed: one that
/// acts on a page would otherwise run at Look only.
#[test]
fn every_tool_and_step_is_in_one_class() {
    // Opens or reads a page, which every tier allows.
    const READS: [&str; 10] = [
        "list_tabs",
        "list_split",
        "open_tab",
        "navigate",
        "snapshot",
        "read_page",
        "scroll",
        "screenshot",
        "wait",
        "check",
    ];
    // Acts on a page: refused below Work by ACTS, or by its own Why, as a request's method is. A
    // run's steps are each judged as their tool is.
    const ACTS_ON: [&str; 6] = [
        "click",
        "type",
        "select",
        "press_key",
        "request",
        "run_steps",
    ];
    // Arranges the agent's own tabs. A close that would lose what a page holds is judged Discard.
    const ARRANGES: [&str; 8] = [
        "close_tab",
        "split_tabs",
        "add_to_split",
        "separate_tab",
        "pin_tab",
        "unpin_tab",
        "remove_tab",
        "sort_tabs_by_site",
    ];
    // Asks the user, who does the step.
    const ASKS: [&str; 4] = ["hand_off", "upload", "ask_for_tab", "ask"];
    // Ends or names the task.
    const ENDS: [&str; 3] = ["done", "title", "finish"];
    let classes: [&[&str]; 5] = [&READS, &ACTS_ON, &ARRANGES, &ASKS, &ENDS];
    let mut every: Vec<String> = ["claude-code", "task-1"]
        .into_iter()
        .flat_map(tools::names)
        .collect();
    every.extend(tools::STEPS.map(String::from));
    for name in &every {
        let placed = classes
            .iter()
            .filter(|class| class.contains(&name.as_str()));
        assert_eq!(placed.count(), 1, "{name} isn't in exactly one class");
    }
    // The tools that act on a page by themselves are those ACTS refuses, as their calls name them.
    let acts: Vec<&str> = (ACTS_ON[..4].iter())
        .map(|name| {
            if *name == "select" {
                "select_option"
            } else {
                name
            }
        })
        .collect();
    assert_eq!(acts, super::access::ACTS);
}

/// No tool lists or reads the user's downloads: the sidebar's plate of recent downloads and the
/// Library's Downloads are the user's own, and an agent's only word of one is the activity log's
/// line for a file its own page started.
#[test]
fn no_tool_reads_the_downloads() {
    let every: Vec<String> = ["claude-code", "task-1"]
        .into_iter()
        .flat_map(tools::names)
        .chain(tools::STEPS.map(String::from))
        .collect();
    assert!(!every.is_empty());
    for name in &every {
        assert!(
            !name.contains("download") && !name.contains("librar"),
            "{name} would read the downloads"
        );
    }
}

/// Lowered mid-task, the next act above the new tier is refused, the log keeps the change, and the
/// task's refusals are settled by it.
#[gpui::test]
fn a_lowered_task_is_refused_from_its_next_act(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let coupon = |cx: &mut TestAppContext| {
        call(
            &agents,
            "claude-code",
            "click",
            json!({ "tab": 20, "ref": "e1" }),
            cx,
        )
    };
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let clicked = coupon(cx);
    assert!(!page_done(&agents, &browser, clicked, shop_tree(), cx).error);

    give(&agents, Tier::Look, cx);
    let pages = browser.pages.borrow().len();
    let what = "click “Coupon” on example.com";
    refused(
        &agents,
        &browser,
        coupon(cx),
        what,
        Why::Act,
        "click “Coupon”",
        cx,
    );
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "it clicked at Look only"
    );
    // A read still goes.
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    let changes = agents.read_with(cx, |agents, _| {
        (agents.activity("claude-code"))
            .filter(|entry| entry.action == super::access::ACCESS_ACTION)
            .map(|entry| (entry.target.to_string(), entry.tier.to_string()))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        changes,
        [(
            "Work on pages → Look only".to_string(),
            "Look only".to_string()
        )]
    );
    give(&agents, Tier::Work, cx);
    assert!(
        stops(&agents, cx).is_empty(),
        "the change left a refusal unsettled"
    );
}

/// A site the user answered Never for this agent is refused at every tier, in the words of an
/// answer of theirs, while Settings lists it; revoked there, the act goes as the tier allows.
#[gpui::test]
fn a_site_set_to_never_is_refused_at_every_tier_until_revoked(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    agents.update(cx, |agents, _| {
        let sites = agents.grants.entry("claude-code".into()).or_default();
        sites.insert("example.com".into(), false);
    });
    let coupon = |cx: &mut TestAppContext| {
        call(
            &agents,
            "claude-code",
            "click",
            json!({ "tab": 20, "ref": "e1" }),
            cx,
        )
    };
    let what = "click “Coupon” on example.com";
    let pages = browser.pages.borrow().len();
    for tier in Tier::ALL {
        give(&agents, tier, cx);
        let told = refused(
            &agents,
            &browser,
            coupon(cx),
            what,
            Why::Never,
            "click “Coupon”",
            cx,
        );
        assert!(
            told.text
                .contains("set example.com to Never for this agent"),
            "{}",
            told.text
        );
        assert_eq!(
            browser.pages.borrow().len(),
            pages,
            "it clicked at {tier:?}"
        );
    }
    // A read still goes.
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    agents.update(cx, |agents, cx| {
        agents.revoke("claude-code", Some("example.com"), cx)
    });
    give(&agents, Tier::Look, cx);
    refused(
        &agents,
        &browser,
        coupon(cx),
        what,
        Why::Act,
        "click “Coupon”",
        cx,
    );
    give(&agents, Tier::Work, cx);
    let clicked = coupon(cx);
    assert!(!page_done(&agents, &browser, clicked, shop_tree(), cx).error);
}

/// A press on a refusal's line just after it appears is taken as meant for what was there, and does
/// nothing; one after that changes the task's access. A pick in the level's menu, which the user
/// opened, is theirs at once: nothing moved under the pointer. Take over holds the tab and settles
/// the refusal there, and the access stays as it was.
#[gpui::test]
fn a_refusals_buttons_wait_and_take_over_keeps_the_access(cx: &mut TestAppContext) {
    use super::task::TaskAction;
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let key = session_key(&agents, "claude-code", cx);
    let coupon = |cx: &mut TestAppContext| {
        let clicked = call(
            &agents,
            "claude-code",
            "click",
            json!({ "tab": 20, "ref": "e1" }),
            cx,
        );
        let what = "click “Coupon” on example.com";
        refused(
            &agents,
            &browser,
            clicked,
            what,
            Why::Act,
            "click “Coupon”",
            cx,
        );
    };
    let press = |action, cx: &mut TestAppContext| {
        agents.update(cx, |agents, cx| {
            agents.task_action(&key, action, cx);
        })
    };
    let tier =
        |cx: &mut TestAppContext| agents.read_with(cx, |agents, _| agents.task(&key).unwrap().tier);
    give(&agents, Tier::Look, cx);
    coupon(cx);

    press(TaskAction::SetTier(Tier::Work), cx);
    assert_eq!(
        tier(cx),
        Tier::Look,
        "a press as the line appeared changed it"
    );
    assert_eq!(stops(&agents, cx).len(), 1);
    press(TaskAction::PickTier(Tier::Work), cx);
    assert_eq!(tier(cx), Tier::Work, "the menu's pick was dropped");
    assert!(stops(&agents, cx).is_empty());

    give(&agents, Tier::Look, cx);
    coupon(cx);
    press(TaskAction::SetTier(Tier::Work), cx);
    assert_eq!(
        tier(cx),
        Tier::Look,
        "a press as the line appeared changed it"
    );
    agents.update(cx, |agents, _| {
        for stop in &mut agents.task_mut(&key).unwrap().stops {
            stop.appeared -= super::tier::PROTECT;
        }
    });
    press(TaskAction::SetTier(Tier::Work), cx);
    assert_eq!(tier(cx), Tier::Work);
    assert!(stops(&agents, cx).is_empty());

    give(&agents, Tier::Look, cx);
    coupon(cx);
    press(TaskAction::TakeOver(20), cx);
    assert!(stops(&agents, cx).is_empty(), "Take over left the refusal");
    assert_eq!(tier(cx), Tier::Look, "Take over changed the access");
    assert!(agents.read_with(cx, |agents, _| agents.held.contains_key(&20)));
}

/// The live line reads what the card's word reads: a refused step waiting is Refused while the
/// card says Needs you, the page the user took over is theirs, and a step handed to them is
/// Handed, never Working.
#[gpui::test]
fn the_live_line_follows_the_cards_word(cx: &mut TestAppContext) {
    use super::task::{Live, TaskAction, Word};
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let key = session_key(&agents, "claude-code", cx);
    let now = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, cx| {
            let task = agents.task(&key).unwrap();
            let live = agents.conversation(&key, cx).unwrap().live;
            (live, agents.card(task, cx).word)
        })
    };
    assert_eq!(now(cx), (Live::Working, Word::Working));

    give(&agents, Tier::Look, cx);
    let clicked = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    let what = "click “Coupon” on example.com";
    refused(
        &agents,
        &browser,
        clicked,
        what,
        Why::Act,
        "click “Coupon”",
        cx,
    );
    assert_eq!(now(cx), (Live::Refused, Word::NeedsYou));

    agents.update(cx, |agents, cx| {
        agents.task_action(&key, TaskAction::TakeOver(20), cx)
    });
    assert_eq!(
        now(cx).0,
        Live::Yours {
            tab: 20,
            site: None
        },
        "the page the user took over, its task's only one"
    );

    agents.update(cx, |agents, cx| {
        agents.task_action(&key, TaskAction::HandBack(20), cx)
    });
    let reason = json!({ "tab": 20, "reason": "Sign in to example.com" });
    let _handed = call(&agents, "claude-code", "hand_off", reason, cx);
    assert_eq!(now(cx), (Live::Handed, Word::NeedsYou));
}

/// The tree for tab 20's page with a message in its field and a Send, which sends it.
fn post_tree() -> String {
    json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" },
          "childIds": ["2", "3"] },
        { "nodeId": "2", "role": { "value": "textbox" }, "name": { "value": "Message" },
          "backendDOMNodeId": 21, "holds": "Is it in stock?",
          "properties": [{ "name": "multiline", "value": { "type": "boolean", "value": true } }] },
        { "nodeId": "3", "role": { "value": "button" }, "name": { "value": "Send" },
          "backendDOMNodeId": 22 },
    ]})
    .to_string()
}

/// Clicks the button `name` in tab 20, its page outlined as `page` first, by the ref the outline
/// gave it, as refs go on counting across a tab's outlines.
fn click_on(
    agents: &Entity<Agents>,
    browser: &Made,
    page: String,
    name: &str,
    cx: &mut TestAppContext,
) -> mpsc::Receiver<ToolResult> {
    let answer = call(agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    let snapshot = page_done(agents, browser, answer, page, cx).text;
    let named = format!("button \"{name}\" [");
    let at = snapshot.find(&named).expect("no such button") + named.len();
    let reference = &snapshot[at..at + snapshot[at..].find(']').unwrap()];
    let click = json!({ "tab": 20, "ref": reference });
    call(agents, "claude-code", "click", click, cx)
}

/// A check's `--agent-waits` shortens the handoff's wait, its second, so a run sees it end, and the
/// quiet end's when it gives a third; its first, an ask's, is taken unread, as nothing asks; a prod
/// build, or a value that isn't two or three parts, keeps the spec's.
#[test]
fn a_checks_waits_stand_in_for_the_specs_outside_prod() {
    let given = |args: &[&str]| args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    let spec = Waits {
        handoff: HANDOFF_WAIT,
        quiet: task::QUIET,
    };
    assert_eq!(Waits::new(&given(&[]), false), spec);
    assert_eq!(
        Waits::new(&given(&["--agent-waits=2,3"]), false),
        Waits {
            handoff: Duration::from_secs(3),
            quiet: task::QUIET,
        }
    );
    assert_eq!(
        Waits::new(&given(&["--agent-waits=2,3,4"]), false).quiet,
        Duration::from_secs(4)
    );
    assert_eq!(Waits::new(&given(&["--agent-waits=2,3,4,5"]), false), spec);
    assert_eq!(
        Waits::new(&given(&["--agent-waits=2,3"]), true),
        spec,
        "a prod build reads no check's waits"
    );
    assert_eq!(Waits::new(&given(&["--agent-waits=2"]), false), spec);
    assert_eq!(
        Waits::new(&given(&["--agent-waits=x,3"]), false).handoff,
        Duration::from_secs(3),
        "the first part is an ask's, which nothing reads"
    );
}

/// A handoff not handed back in 15 minutes answers the agent's call, so it isn't left waiting.
/// The tab stays the user's, and Hand Back still gives it back.
#[gpui::test]
fn a_handoff_not_handed_back_answers_after_15_minutes(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let reason = json!({ "tab": 20, "reason": "Sign in to example.com" });
    let handed = call(&agents, "claude-code", "hand_off", reason, cx);
    assert!(shown(&agents, cx).is_some(), "the handoff didn't ask");

    cx.executor()
        .advance_clock(HANDOFF_WAIT - Duration::from_secs(1));
    assert!(handed.try_recv().is_err(), "it gave up before 15 minutes");
    cx.executor().advance_clock(Duration::from_secs(1));
    let answer = handed.recv().unwrap();
    assert!(answer.error);
    assert!(
        answer
            .text
            .starts_with("The user hasn't handed tab 20 back in 15 minutes.")
    );
    assert_eq!(shown(&agents, cx), None);
    assert!(browser.held.borrow().contains(&20), "the tab left the user");
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, [(20, "Sign in to example.com".into())]);
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "not handed back");

    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    assert!(browser.held.borrow().is_empty());
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "handed back");
}

/// A hand-off is its tab's alone: two tabs handed off at once are both the user's, and each
/// call is answered when its own tab comes back, handed back in the other order.
#[gpui::test]
fn two_tabs_handed_off_at_once_come_back_in_either_order(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    (browser.tabs.borrow_mut()).push((21, 2, "Example Org", "https://www.example.org/"));
    let first = json!({ "tab": 20, "reason": "Sign in to example.com" });
    let first = call(&agents, "claude-code", "hand_off", first, cx);
    let second = json!({ "tab": 21, "reason": "Sign in to example.org" });
    let second = call(&agents, "claude-code", "hand_off", second, cx);
    if let Ok(answer) = second.try_recv() {
        panic!("tab 21's hand-off was answered before it was handed back: {answer:?}");
    }
    assert_eq!(
        browser
            .held
            .borrow()
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        [20, 21].into(),
        "a second tab's hand-off waited for the first's"
    );

    agents.update(cx, |agents, cx| agents.hand_back(21, cx));
    cx.run_until_parked();
    assert!(
        !second.recv().unwrap().error,
        "tab 21's call wasn't answered"
    );
    assert!(
        first.try_recv().is_err(),
        "tab 20's call was answered by tab 21's hand-back"
    );
    assert!(
        browser.held.borrow().contains(&20),
        "tab 20 left the user with tab 21"
    );

    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    cx.run_until_parked();
    assert!(
        !first.recv().unwrap().error,
        "tab 20's call wasn't answered"
    );
    assert!(browser.held.borrow().is_empty());
}

/// The core's report of a download the page in tab `tab` started, held, as the answer to
/// `request`.
fn download_started(
    agents: &Entity<Agents>,
    tab: u64,
    request: u64,
    id: u64,
    cx: &mut TestAppContext,
) {
    let download = Download {
        agent: "claude-code".into(),
        tab,
        request,
        id,
        name: "report.docx".into(),
        url: "https://files.example.com/report.docx".into(),
        tab_empty: tab == 30,
    };
    agents.update(cx, |agents, cx| agents.download_started(download, cx));
}

/// open_tab at a file's URL downloads it (#1446): at Work on pages it goes unasked, the empty tab
/// closes, and the answer names the file, with no error.
#[gpui::test]
fn an_agents_download_goes_at_work_on_pages_and_answers_with_its_file(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let url = json!({ "url": "https://files.example.com/report.docx" });
    let answer = call(&agents, "claude-code", "open_tab", url, cx);
    let request = browser.pages.borrow().last().unwrap().1;
    download_started(&agents, 30, request, 900, cx);
    assert_eq!(shown(&agents, cx), None, "the download asked");
    assert!(
        browser.tabs.borrow().iter().all(|tab| tab.0 != 30),
        "the empty tab stayed"
    );
    assert_eq!(*browser.downloads.borrow(), [(900, true)]);
    let done = DownloadEnd::Done {
        name: "report.docx".into(),
        bytes: 48_213,
        path: "/Users/someone/Downloads/report.docx".into(),
    };
    agents.update(cx, |agents, cx| agents.download_ended(900, done, cx));
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::text(
            "Downloaded report.docx (48 KB) to Downloads. Agents can't open files; ask the user \
             if you need it."
        )
    );
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        ("download".into(), "files.example.com".into(), "".into())
    );
}
/// A download at Look only is refused and cancelled, which leaves no file. A page's own, with no
/// action to answer, is refused all the same.
#[gpui::test]
fn a_download_at_look_only_is_refused_and_leaves_no_file(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        "screenshot",
        json!({ "tab": 20 }),
        cx,
    );
    give(&agents, Tier::Look, cx);
    let request = browser.pages.borrow().last().unwrap().1;
    download_started(&agents, 20, request, 900, cx);
    assert_eq!(*browser.downloads.borrow(), [(900, false)]);
    let what = "download report.docx from files.example.com";
    let told = refused(
        &agents,
        &browser,
        answer,
        what,
        Why::Download,
        "files.example.com",
        cx,
    );
    assert_eq!(told.kind, Some(tools::Kind::Outside));
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        (
            "download".into(),
            "files.example.com".into(),
            "Downloading a file isn't part of Look only.".into()
        )
    );
    assert!(
        browser.tabs.borrow().iter().any(|tab| tab.0 == 20),
        "a tab with a page closed"
    );

    download_started(&agents, 20, 0, 901, cx);
    assert_eq!(*browser.downloads.borrow(), [(900, false), (901, false)]);
    assert_eq!(shown(&agents, cx), None);
}
/// A bank's site is refused below Full access, at Look only too, each time and logged, and opens
/// once the user gives Full access.
#[gpui::test]
fn a_banks_site_opens_only_at_full_access(cx: &mut TestAppContext) {
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
    refused(
        &agents,
        &browser,
        open(cx),
        "open chase.com",
        Why::Bank,
        "chase.com",
        cx,
    );
    assert_eq!(
        log(&agents, "claude-code", cx)[0],
        (
            "open_tab".into(),
            "chase.com".into(),
            "Using a bank site isn't part of Work on pages.".into()
        )
    );
    give(&agents, Tier::Look, cx);
    refused(
        &agents,
        &browser,
        open(cx),
        "open chase.com",
        Why::Bank,
        "chase.com",
        cx,
    );
    assert!(browser.pages.borrow().is_empty());
    assert!(browser.tabs.borrow().iter().all(|tab| tab.0 != 30));

    give(&agents, Tier::Full, cx);
    let _answer = open(cx);
    assert!(
        browser.tabs.borrow().iter().any(|tab| tab.0 == 30),
        "it didn't open"
    );
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

/// A page whose requests were still open when the core stopped waiting, though what it showed
/// held still, as one that polls, says that rather than that it may change yet (#2355).
#[gpui::test]
fn an_action_says_when_only_requests_were_still_open(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "scroll", json!({ "tab": 20 }), cx);
    let polling = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" } },
    ], "requests": true })
    .to_string();
    let scrolled = page_done(&agents, &browser, answer, polling, cx);
    assert!(!scrolled.error);
    assert!(
        scrolled.text.starts_with(
            "Scrolled down. The page held still, with requests it made still open after 5 s.\n\n\
             Tab 20"
        ),
        "{}",
        scrolled.text
    );
    assert!(
        !scrolled.text.contains("may change yet"),
        "{}",
        scrolled.text
    );
}

/// A wait step until "quiet" ends as soon as the page holds still with only its requests open, as
/// a page that polls keeps one open for good (#2355).
#[gpui::test]
fn a_wait_step_ends_on_a_page_that_holds_still_while_it_polls(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let steps =
        json!({ "tab": 20, "steps": [{ "action": "wait", "until": "quiet", "seconds": 10 }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    let polling = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" } },
    ], "requests": true })
    .to_string();
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, polling, None, cx)
    });
    cx.run_until_parked();
    let ran = run.recv().unwrap();
    assert!(
        ran.text.starts_with(
            "Ran 1 steps.\n1. The page held still, with requests it made still open after 5 s.\n\n\
             Tab 20"
        ),
        "{}",
        ran.text
    );
    assert_eq!(browser.pages.borrow().len(), 1, "it looked again");
}

/// The core answers the agents' last request of it with the shop's tree.
fn shop_done(agents: &Entity<Agents>, browser: &Made, cx: &mut TestAppContext) {
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, shop_tree(), None, cx)
    });
}

/// A page that hadn't settled when the core stopped waiting for it says so, after what the action
/// did, so a run's list of steps says it too.
#[gpui::test]
fn an_action_says_when_its_page_was_still_busy(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "scroll", json!({ "tab": 20 }), cx);
    let busy = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" } },
    ], "busy": true })
    .to_string();
    let scrolled = page_done(&agents, &browser, answer, busy, cx);
    assert!(!scrolled.error);
    assert!(
        scrolled.text.starts_with(
            "Scrolled down. The page was still busy after 5 s, so it may change yet.\n\nTab 20"
        ),
        "{}",
        scrolled.text
    );
}

/// A wait step until "quiet" asks the core for the page once it has settled, again while it was
/// still busy, and once its time is up goes on with the run, as a page that keeps changing never
/// settles.
#[gpui::test]
fn a_wait_step_waits_for_the_page_to_settle(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let busy = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" } },
    ], "busy": true })
    .to_string();
    let look = |tree: String, cx: &mut TestAppContext| {
        let request = browser.pages.borrow().last().unwrap().1;
        agents.update(cx, |agents, cx| agents.page_done(request, tree, None, cx));
        cx.run_until_parked();
    };
    let last = || browser.pages.borrow().last().unwrap().2;
    let looks = || browser.pages.borrow().len();

    let steps =
        json!({ "tab": 20, "steps": [{ "action": "wait", "until": "quiet", "seconds": 2 }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    assert_eq!((looks(), last()), (1, PageAction::Quiet));
    look(busy.clone(), cx);
    assert!(
        run.try_recv().is_err(),
        "a busy page ended the wait before its time"
    );
    assert_eq!((looks(), last()), (2, PageAction::Quiet));
    look(shop_tree(), cx);
    let ran = run.recv().unwrap();
    assert!(
        ran.text
            .starts_with("Ran 1 steps.\n1. The page settled.\n\nTab 20"),
        "{}",
        ran.text
    );
    assert_eq!(browser.checked.borrow().last().unwrap(), "snapshot");
    let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    let steps: Vec<&str> = entry
        .steps
        .iter()
        .map(|step| step.action.as_ref())
        .collect();
    assert_eq!(steps, ["wait"]);

    let steps =
        json!({ "tab": 20, "steps": [{ "action": "wait", "until": "quiet", "seconds": 1 }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    look(busy.clone(), cx);
    cx.executor().advance_clock(Duration::from_secs(1));
    look(busy, cx);
    let ran = run.recv().unwrap();
    assert!(!ran.error, "a page still busy stopped the run");
    assert!(
        ran.text
            .starts_with("Ran 1 steps.\n1. The page was still busy after 1 s.\n\nTab 20"),
        "{}",
        ran.text
    );
}

/// A wait step for gone text looks at the page until the text has gone from it, and fails once its
/// time is up with the text still there.
#[gpui::test]
fn a_wait_step_waits_for_text_to_go(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let look = |tree: String, cx: &mut TestAppContext| {
        let request = browser.pages.borrow().last().unwrap().1;
        agents.update(cx, |agents, cx| agents.page_done(request, tree, None, cx));
        cx.run_until_parked();
    };
    let looks = || browser.pages.borrow().len();
    let thanks = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "heading" }, "name": { "value": "Thanks for your order" } },
    ]})
    .to_string();

    let steps = json!({ "tab": 20, "steps": [{ "action": "wait", "gone": "basket" }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    assert_eq!(
        browser.pages.borrow().last().unwrap().2,
        PageAction::Snapshot
    );
    look(shop_tree(), cx);
    assert!(run.try_recv().is_err());
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(looks(), 2);
    look(thanks, cx);
    let ran = run.recv().unwrap();
    assert!(
        ran.text
            .starts_with("Ran 1 steps.\n1. “basket” went.\n\nTab 20"),
        "{}",
        ran.text
    );

    let steps =
        json!({ "tab": 20, "steps": [{ "action": "wait", "gone": "Basket", "seconds": 1 }] });
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
            .starts_with("Step 1 failed: “Basket” was still there after 1 s."),
        "{}",
        failed.text
    );
}

/// navigate and open_tab with wait_for answer once the page has settled and the text shows in it,
/// looking again as a wait step does, and past their seconds answer that it didn't show (#3658).
#[gpui::test]
fn navigate_and_open_tab_wait_for_their_text(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let look = |tree: String, cx: &mut TestAppContext| {
        let request = browser.pages.borrow().last().unwrap().1;
        agents.update(cx, |agents, cx| agents.page_done(request, tree, None, cx));
        cx.run_until_parked();
    };
    let looks = || browser.pages.borrow().len();
    let splash = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Mail" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "StaticText" }, "name": { "value": "Loading" } },
    ]})
    .to_string();

    let arguments = json!({ "tab": 20, "url": "iana.org", "wait_for": "place order" });
    let answer = call(&agents, "claude-code", "navigate", arguments, cx);
    assert_eq!(browser.pages.borrow().last().unwrap().2, PageAction::Quiet);
    look(splash.clone(), cx);
    assert!(answer.try_recv().is_err(), "it answered with the splash");
    let before = looks();
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(looks(), before + 1);
    assert_eq!(
        browser.pages.borrow().last().unwrap().2,
        PageAction::Snapshot
    );
    look(shop_tree(), cx);
    let answered = answer.recv().unwrap();
    assert!(!answered.error);
    assert!(
        answered.text.starts_with("“place order” showed.\n\n"),
        "{}",
        answered.text
    );

    let arguments = json!({ "tab": 20, "url": "iana.org", "wait_for": "Inbox", "seconds": 1 });
    let answer = call(&agents, "claude-code", "navigate", arguments, cx);
    for _ in 0..2 {
        look(splash.clone(), cx);
        assert!(answer.try_recv().is_err());
        cx.executor().advance_clock(Duration::from_millis(500));
    }
    look(splash.clone(), cx);
    let answered = answer.recv().unwrap();
    assert!(!answered.error);
    assert!(
        answered.text.starts_with("“Inbox” didn't show in 1 s.\n\n"),
        "{}",
        answered.text
    );

    let arguments = json!({ "url": "docs.example", "wait_for": "Basket" });
    let answer = call(&agents, "claude-code", "open_tab", arguments, cx);
    look(splash, cx);
    cx.executor().advance_clock(Duration::from_millis(500));
    look(shop_tree(), cx);
    let answered = answer.recv().unwrap();
    assert!(
        answered.text.contains(". “Basket” showed.\n\n"),
        "{}",
        answered.text
    );

    let arguments = json!({ "tab": 20, "url": "iana.org", "wait_for": "Inbox", "seconds": 60 });
    let refused = call(&agents, "claude-code", "navigate", arguments, cx);
    assert!(refused.recv().unwrap().error);
}

/// answer and wait_for go together: navigate and open_tab wait for their text, then read the page
/// for an answer of text, saying first that it showed (#3658, #3657).
#[gpui::test]
fn the_text_answer_comes_after_wait_for(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let look = |tree: String, cx: &mut TestAppContext| {
        let request = browser.pages.borrow().last().unwrap().1;
        agents.update(cx, |agents, cx| agents.page_done(request, tree, None, cx));
        cx.run_until_parked();
    };
    let splash = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "StaticText" }, "name": { "value": "Loading" } },
    ]})
    .to_string();
    for (tool, arguments) in [
        (
            "navigate",
            json!({ "tab": 20, "url": "iana.org", "answer": "text", "wait_for": "place order" }),
        ),
        (
            "open_tab",
            json!({ "url": "docs.example", "answer": "text", "wait_for": "place order" }),
        ),
    ] {
        let answer = call(&agents, "claude-code", tool, arguments, cx);
        let reads = browser.reads.borrow().len();
        look(splash.clone(), cx);
        assert_eq!(
            browser.reads.borrow().len(),
            reads,
            "{tool} read the splash"
        );
        cx.executor().advance_clock(Duration::from_millis(500));
        look(shop_tree(), cx);
        assert!(
            answer.try_recv().is_err(),
            "{tool} answered before the text came"
        );
        let (_, read) = *browser.reads.borrow().last().unwrap();
        agents.update(cx, |agents, cx| {
            agents.text_read(read, Some("Basket\nTwo items".into()), cx)
        });
        let text = answer.recv().unwrap();
        assert!(!text.error, "{}", text.text);
        assert!(
            text.text.contains("“place order” showed."),
            "{tool}: {}",
            text.text
        );
        assert!(
            text.text.ends_with("\n\nBasket\nTwo items"),
            "{tool}: {}",
            text.text
        );
    }
}

/// A wait's next look at a tab the user took over between two looks reads nothing: navigate's
/// wait_for and run_steps' wait step each end as a call on a tab the user holds does.
#[gpui::test]
fn a_wait_reads_nothing_once_the_user_takes_the_tab(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let look = |tree: String, cx: &mut TestAppContext| {
        let request = browser.pages.borrow().last().unwrap().1;
        agents.update(cx, |agents, cx| agents.page_done(request, tree, None, cx));
        cx.run_until_parked();
    };
    let looks = || browser.pages.borrow().len();

    let arguments = json!({ "tab": 20, "url": "iana.org", "wait_for": "Thanks" });
    let answer = call(&agents, "claude-code", "navigate", arguments, cx);
    look(shop_tree(), cx);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    let before = looks();
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(looks(), before, "navigate's wait looked at the user's tab");
    let refused = answer.try_recv().expect("navigate's wait didn't answer");
    assert!(refused.error);
    assert_eq!(refused.text, HELD);
    let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    assert_eq!(
        (entry.action.as_ref(), entry.refused.as_ref()),
        ("navigate", HELD)
    );
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));

    let wait = json!({ "action": "wait", "text": "Thanks", "seconds": 2 });
    let run = call(
        &agents,
        "claude-code",
        "run_steps",
        json!({ "tab": 20, "steps": [wait] }),
        cx,
    );
    look(shop_tree(), cx);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    let before = looks();
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(looks(), before, "the wait step looked at the user's tab");
    let refused = run.try_recv().expect("the run didn't answer");
    assert!(refused.error);
    assert!(refused.text.contains(HELD), "{}", refused.text);
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
    let question = shown(&agents, cx).expect("the handoff didn't ask");
    assert_eq!(question.tab, 20);
    assert_eq!(question.why, "Sign in to example.com");
    assert!(browser.held.borrow().contains(&20));
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, [(20, "Sign in to example.com".into())]);
    let refused = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert_eq!(
        refused.recv().unwrap(),
        ToolResult::failed(tools::Kind::Yours, HELD)
    );
    assert!(
        handed.try_recv().is_err(),
        "the handoff answered before the tab came back"
    );

    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    let back = handed.recv().unwrap();
    assert!(back.text.starts_with("The user handed tab 20 back."));
    assert_eq!(shown(&agents, cx), None);
    assert!(browser.held.borrow().is_empty());
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "handed back");
}

/// Take Over holds the agent's tab until the user hands it back, with no card; Stop answers what
/// the agent waits on and ends its sessions.
#[gpui::test]
fn the_user_takes_over_and_stops_an_agent(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    assert!(browser.held.borrow().contains(&20));
    assert_eq!(shown(&agents, cx), None);
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, [(20, "".into())]);
    let refused = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(refused.recv().unwrap().error);
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);

    // A click the page hasn't answered yet.
    let click = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    agents.update(cx, |agents, cx| agents.stop_agent("claude-code", cx));
    let stopped = click.recv().unwrap();
    assert!(stopped.error && stopped.text.starts_with("The user stopped you."));
    assert_eq!(shown(&agents, cx), None);
    assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "stopped by the user");
}

/// An agent has a status while its session lasts, for the bar in its Space and for a press in any
/// of its tabs, which takes that tab over. Once its last session ends it has none: its tabs are the
/// user's, with nothing to hand back to (#3503).
#[gpui::test]
fn an_agent_whose_session_ended_has_no_status(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(
        status
            .iter()
            .map(|status| &status.agent)
            .collect::<Vec<_>>(),
        ["claude-code"]
    );

    let closed = Event::Closed {
        agent: "claude-code".into(),
        connection: 1,
    };
    agents.update(cx, |agents, cx| agents.handle(closed, cx));
    let bye = Event::Bye {
        agent: "claude-code".into(),
    };
    agents.update(cx, |agents, cx| agents.handle(bye, cx));
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert!(
        status.is_empty(),
        "an agent whose session ended kept a status: {status:?}"
    );
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
    let question = shown(&agents, cx).expect("asking for the tab didn't ask");
    assert_eq!((question.tab, question.back), (20, true));
    assert_eq!(question.why, "To finish the order");
    let again = call(&agents, "claude-code", "ask_for_tab", reason.clone(), cx);
    assert!(
        again
            .recv()
            .unwrap()
            .text
            .starts_with("You're waiting on the user")
    );
    agents.update(cx, |agents, cx| agents.let_go(20, held::Event::Keep, cx));
    let kept = kept.recv().unwrap();
    assert!(kept.error && kept.text.starts_with("The user keeps tab 20."));
    assert_eq!(shown(&agents, cx), None);
    assert!(
        browser.held.borrow().contains(&20),
        "Keep gave the tab back"
    );
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "kept");

    let back = call(&agents, "claude-code", "ask_for_tab", reason, cx);
    assert!(shown(&agents, cx).is_some(), "asking again didn't ask");
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
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
    assert_eq!(shown(&agents, cx), None);
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

/// The core answers a close that asked `tab`'s page with `closed`, from a task of its own, as the
/// core's agent_close_answered does: the tab goes if it closed.
fn page_answers(
    agents: &Entity<Agents>,
    browser: &Made,
    tab: u64,
    closed: Closed,
    cx: &mut TestAppContext,
) {
    if closed == Closed::Closed {
        browser.tabs.borrow_mut().retain(|each| each.0 != tab);
    }
    agents.update(cx, |agents, cx| agents.close_answered(tab, closed, cx));
}

/// The agent's own tab at docs.example, whose page has a beforeunload handler: 30.
fn opened_with_handler(agents: &Entity<Agents>, browser: &Made, cx: &mut TestAppContext) {
    let opened = call(
        agents,
        "claude-code",
        "open_tab",
        json!({ "url": "docs.example" }),
        cx,
    );
    _ = page_done(agents, browser, opened, shop_tree(), cx);
    browser.asks.borrow_mut().insert(30);
}

/// The page of `tab` shows its own dialog of `kind`, or none any more, as the core reports it.
fn page_dialog(
    agents: &Entity<Agents>,
    tab: u64,
    kind: Option<DialogKind>,
    cx: &mut TestAppContext,
) {
    agents.update(cx, |agents, cx| agents.page_dialog(tab, kind, "", cx));
    cx.run_until_parked();
}

/// Tab `tab`'s page asks for a sign-in in the browser's own dialog, for a request to `from`, as
/// the core reports it, and the core ends the call running there as it does for any dialog.
fn asks_for_sign_in(
    agents: &Entity<Agents>,
    browser: &Made,
    tab: u64,
    from: &str,
    cx: &mut TestAppContext,
) {
    let message = format!("{from}\nYour connection to this site is not private");
    let kind = Some(DialogKind::SignIn);
    agents.update(cx, |agents, cx| agents.page_dialog(tab, kind, &message, cx));
    let request = browser.pages.borrow().last().unwrap().1;
    let ended = "The tab's page showed a dialog asking for a user name and password while the \
                 action ran. Nothing more can be done on the tab while it's up, and no tool \
                 answers it. close_tab closes the tab, and the dialog with it.";
    agents.update(cx, |agents, cx| {
        agents.page_done(request, ended.into(), Some(Failure::Other), cx)
    });
    cx.run_until_parked();
}

/// A call whose page asks for a sign-in in the browser's own dialog, which no agent may answer,
/// becomes a handoff to the user, as hand_off makes, naming the host that asks, not the page's
/// site: the card asks, the tab is held and the log says so, and the call waits. Hand Back
/// answers it, saying why it waited (#3662).
#[gpui::test]
fn a_sign_in_the_browser_asks_for_hands_the_tab_to_the_user(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let opened = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "docs.example" }),
        cx,
    );
    cx.run_until_parked();
    asks_for_sign_in(&agents, &browser, 30, "http://auth.example:8080", cx);
    let question = shown(&agents, cx).expect("the sign-in didn't ask");
    let why = "auth.example asks you to sign in. Hand the tab back when you’re done.";
    assert_eq!((question.tab, question.why.as_ref()), (30, why));
    assert!(browser.held.borrow().contains(&30));
    let status = agents.read_with(cx, |agents, _| agents.status());
    assert_eq!(status[0].held, [(30, why.into())]);
    let entries = log(&agents, "claude-code", cx);
    assert!(
        entries.contains(&("hand_off".into(), "docs.example".into(), "".into())),
        "{entries:?}"
    );
    assert!(
        opened.try_recv().is_err(),
        "the call answered before the tab came back"
    );
    let refused = call(&agents, "claude-code", "snapshot", json!({ "tab": 30 }), cx);
    assert_eq!(
        refused.recv().unwrap(),
        ToolResult::failed(tools::Kind::Yours, HELD)
    );

    agents.update(cx, |agents, cx| agents.hand_back(30, cx));
    let back = opened.recv().unwrap();
    assert!(!back.error);
    assert!(
        back.text.starts_with(SIGNED_IN)
            && back.text.ends_with(
                "The user handed tab 30 back. \
                                                              Take a snapshot to see where it \
                                                              is now."
            ),
        "{}",
        back.text
    );
    assert_eq!(shown(&agents, cx), None);
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "handed back");

    // Handed back with the dialog still up, the agent is told to hand it off.
    let again = call(&agents, "claude-code", "snapshot", json!({ "tab": 30 }), cx);
    let again = again.recv().unwrap();
    assert!(again.error);
    assert!(
        again.text.starts_with(
            "Tab 30 shows a browser sign-in dialog, which only the user can answer. No other \
             calls work on the tab while it is open: use hand_off to let the user sign in, or \
             close_tab to close the tab and its dialog."
        ),
        "{}",
        again.text
    );
}

/// A tab the agent didn't open, its sign-in up, is closed with discard, which may ask the user.
#[test]
fn a_sign_in_on_a_tab_not_opened_says_discard() {
    assert_eq!(
        showing_dialog(7, DialogKind::SignIn, false),
        "Tab 7 shows a browser sign-in dialog, which only the user can answer. No other calls \
         work on the tab while it is open: use hand_off to let the user sign in, or close_tab \
         with discard: true to close the tab and its dialog. You did not open this tab, so the \
         user may be asked first."
    );
}

/// A sign-in's handoff not handed back answers in 15 minutes, as any handoff does, and a proxy's
/// is named as one. Another dialog ending a call answers it at once (#3536).
#[gpui::test]
fn a_sign_in_handoff_answers_after_15_minutes(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let snapshot = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    cx.run_until_parked();
    asks_for_sign_in(
        &agents,
        &browser,
        20,
        "The proxy http://proxy.example:3128 requires a username and password",
        cx,
    );
    let question = shown(&agents, cx).expect("the sign-in didn't ask");
    assert_eq!(
        question.why,
        "A proxy asks you to sign in. Hand the tab back when you’re done."
    );
    cx.executor().advance_clock(HANDOFF_WAIT);
    let answer = snapshot.recv().unwrap();
    assert!(answer.error);
    assert!(
        answer.text.starts_with(&format!(
            "{SIGNED_IN} The user hasn't handed tab 20 back in 15 minutes."
        )),
        "{}",
        answer.text
    );
    assert!(browser.held.borrow().contains(&20), "the tab left the user");

    let opened = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "docs.example" }),
        cx,
    );
    _ = page_done(&agents, &browser, opened, shop_tree(), cx);
    let looked = call(&agents, "claude-code", "snapshot", json!({ "tab": 30 }), cx);
    cx.run_until_parked();
    agents.update(cx, |agents, cx| {
        agents.page_dialog(30, Some(DialogKind::Confirm), "", cx)
    });
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, "ended".into(), Some(Failure::Other), cx)
    });
    assert_eq!(looked.recv().unwrap(), ToolResult::error("ended"));
    assert!(!browser.held.borrow().contains(&30));
}

/// While its page shows its own dialog, a call on the agent's tab answers at once, naming the
/// dialog, and reaches no page, as no tool answers a page's dialog; the log says the dialog paused
/// it, without its words. Once the dialog has gone, calls reach the page again (#3536).
#[gpui::test]
fn a_call_on_a_page_showing_its_dialog_answers_at_once(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let opened = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "docs.example" }),
        cx,
    );
    _ = page_done(&agents, &browser, opened, shop_tree(), cx);
    let pages = browser.pages.borrow().len();
    page_dialog(&agents, 30, Some(DialogKind::Confirm), cx);

    for (tool, arguments) in [
        ("press_key", json!({ "tab": 30, "key": "Escape" })),
        ("click", json!({ "tab": 30, "ref": "e1" })),
        ("navigate", json!({ "tab": 30, "url": "back" })),
    ] {
        let answer = call(&agents, "claude-code", tool, arguments, cx);
        cx.run_until_parked();
        let answer = answer.try_recv().expect("an answer at once");
        assert!(answer.error, "{tool}");
        assert_eq!(
            answer.text,
            "Tab 30's page shows a confirm dialog. Nothing more can be done on the tab while \
             it's up, and no tool answers it. close_tab closes the tab, and the dialog with it."
        );
    }
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "a call reached the page"
    );
    let entries = log(&agents, "claude-code", cx);
    assert!(
        entries.contains(&("page_dialog".into(), "docs.example".into(), "".into())),
        "{entries:?}"
    );
    assert_eq!(
        (entries.iter())
            .filter(|entry| entry.0 == "page_dialog")
            .count(),
        1
    );

    page_dialog(&agents, 30, None, cx);
    let pressed = call(
        &agents,
        "claude-code",
        "press_key",
        json!({ "tab": 30, "key": "Escape" }),
        cx,
    );
    cx.run_until_parked();
    assert!(pressed.try_recv().is_err(), "answered without the page");
    assert_eq!(browser.pages.borrow().len(), pages + 1);
}

/// The agent's own tab whose page shows its dialog closes at once, without asking its page,
/// which can't answer while its dialog is up, nor the user (#3536).
#[gpui::test]
fn a_page_showing_its_dialog_closes_at_once(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    opened_with_handler(&agents, &browser, cx);
    page_dialog(&agents, 30, Some(DialogKind::Alert), cx);
    let closed = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 30 }),
        cx,
    );
    cx.run_until_parked();
    let closed = closed.try_recv().expect("an answer at once");
    assert_eq!(
        (closed.error, closed.text.as_str()),
        (false, "Closed tab 30.")
    );
    assert_eq!(*browser.closes.borrow(), [(30, true)]);
    assert_eq!(shown(&agents, cx), None, "asked the user");
}

/// A tab in the agent's Space that it didn't open, as one its page opened, may hold what the user
/// hasn't saved, and its page can't be asked while its dialog is up: a close answers at once that
/// it stays, without asking the page, and a discard is refused below Full access, which closes it
/// (#3536).
#[gpui::test]
fn a_page_the_user_gave_showing_its_dialog_is_discarded_only_at_full_access(
    cx: &mut TestAppContext,
) {
    let (agents, browser, _) = connected(cx);
    for each in browser
        .tabs
        .borrow_mut()
        .iter_mut()
        .filter(|each| each.0 == 10)
    {
        each.1 = 2;
    }
    browser.asks.borrow_mut().insert(10);
    page_dialog(&agents, 10, Some(DialogKind::Alert), cx);

    let pressed = call(
        &agents,
        "claude-code",
        "press_key",
        json!({ "tab": 10, "key": "Escape" }),
        cx,
    );
    cx.run_until_parked();
    let pressed = pressed.try_recv().expect("an answer at once");
    assert_eq!(
        (pressed.error, pressed.text.as_str()),
        (
            true,
            "Tab 10's page shows an alert. Nothing more can be done on the tab while it's up, and \
             no tool answers it. close_tab with discard: true closes the tab, and the dialog with \
             it, if this task's access allows that for a tab you didn't open."
        )
    );

    let kept = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 10 }),
        cx,
    );
    cx.run_until_parked();
    let kept = kept.try_recv().expect("an answer at once");
    assert_eq!(
        (kept.error, kept.text.as_str()),
        (
            true,
            "Tab 10 stays open: its page shows its own dialog, so it can't be asked whether it \
             has unsaved changes. close_tab with discard: true closes it without saving, if this \
             task's access allows that for a tab you didn't open."
        )
    );
    assert_eq!(*browser.closes.borrow(), [], "asked the page");
    assert_eq!(shown(&agents, cx), None, "asked the user");

    let closed = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 10, "discard": true }),
        cx,
    );
    cx.run_until_parked();
    assert_eq!(*browser.closes.borrow(), [], "asked the page");
    let what = "close the tab on en.wikipedia.org without saving";
    refused(
        &agents,
        &browser,
        closed,
        what,
        Why::Discard,
        "en.wikipedia.org",
        cx,
    );
    give(&agents, Tier::Full, cx);
    let closed = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 10, "discard": true }),
        cx,
    );
    cx.run_until_parked();
    let closed = closed.recv().unwrap();
    assert_eq!(closed.text, "Closed tab 10 without saving.");
    assert_eq!(*browser.closes.borrow(), [(10, true)]);
}

/// A dialog on a page that isn't the agent's says nothing to it: the core's refusal answers, and
/// the agent's log has no pause.
#[gpui::test]
fn another_spaces_dialog_says_nothing(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    _ = call(&agents, "claude-code", "list_tabs", json!({}), cx);
    page_dialog(&agents, 10, Some(DialogKind::Alert), cx);
    let refused = call(
        &agents,
        "claude-code",
        "click",
        json!({ "tab": 10, "ref": "e1" }),
        cx,
    );
    cx.run_until_parked();
    let refused = refused.try_recv().expect("the core's refusal");
    assert!(refused.error);
    assert!(!refused.text.contains("dialog"), "{}", refused.text);
    let entries = log(&agents, "claude-code", cx);
    assert!(!entries.iter().any(|entry| entry.0 == "page_dialog"));
}

/// A page whose handler asks to stay stays open, and the answer, which waits for the page, says
/// why; the log has the close once the page answered. With discard, the agent's own tab is asked
/// again and closes without saving, the log saying so, and the user isn't asked (#1447, #3256).
#[gpui::test]
fn an_agent_discards_only_a_page_it_opened(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    opened_with_handler(&agents, &browser, cx);

    let kept = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 30 }),
        cx,
    );
    assert!(kept.try_recv().is_err(), "answered before the page did");
    assert!(
        log(&agents, "claude-code", cx)
            .iter()
            .all(|entry| entry.0 != "close_tab"),
        "logged the close before the page answered"
    );
    page_answers(&agents, &browser, 30, Closed::Asks, cx);
    let kept = kept.recv().unwrap();
    assert!(kept.error);
    assert_eq!(
        kept.text,
        "Tab 30 stays open: its page asks before it's left, as it may have unsaved changes. \
         close_tab with discard: true closes it without saving."
    );
    assert!(
        browser.tabs.borrow().iter().any(|tab| tab.0 == 30),
        "a refused close closed it"
    );
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "close_tab");

    let discard = json!({ "tab": 30, "discard": true });
    let closed = call(&agents, "claude-code", "close_tab", discard, cx);
    assert!(
        closed.try_recv().is_err(),
        "the discard didn't ask the page"
    );
    page_answers(&agents, &browser, 30, Closed::Asks, cx);
    let closed = closed.recv().unwrap();
    assert!(!closed.error);
    assert_eq!(closed.text, "Closed tab 30 without saving.");
    assert!(browser.tabs.borrow().iter().all(|tab| tab.0 != 30));
    let entry = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(
        entry,
        ("discard_tab".into(), "docs.example".into(), "".into())
    );
    assert_eq!(tools::describe(&entry.0), "Closed a tab without saving");
}

/// A page whose handler doesn't ask closes at the first call, discard or not, with nothing lost
/// and no question to the user, even in a tab the agent didn't open.
#[gpui::test]
fn a_page_that_doesnt_ask_closes_at_the_first_call(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    for each in (browser.tabs.borrow_mut().iter_mut()).filter(|each| each.0 == 10) {
        each.1 = 2;
    }
    browser.asks.borrow_mut().insert(10);
    let discard = json!({ "tab": 10, "discard": true });
    let closed = call(&agents, "claude-code", "close_tab", discard, cx);
    page_answers(&agents, &browser, 10, Closed::Closed, cx);
    let closed = closed.recv().unwrap();
    assert!(!closed.error);
    assert_eq!(closed.text, "Closed tab 10.");
    assert_eq!(*browser.closes.borrow(), [(10, false)]);
    assert_eq!(shown(&agents, cx), None, "asked the user");
    let entry = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(
        entry,
        ("close_tab".into(), "en.wikipedia.org".into(), "".into())
    );
}

/// A second close while the page is asked waits for the same answer, with no second ask of the
/// page's own: each close has its entry in the log. A discard of the agent's own tab meanwhile
/// closes it at once, and the close waiting hears it closed.
#[gpui::test]
fn closes_while_the_page_is_asked_share_its_answer(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    opened_with_handler(&agents, &browser, cx);
    let first = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 30 }),
        cx,
    );
    let second = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 30 }),
        cx,
    );
    page_answers(&agents, &browser, 30, Closed::Asks, cx);
    for answer in [first, second] {
        let answer = answer.recv().unwrap();
        assert!(answer.error);
        assert!(answer.text.starts_with("Tab 30 stays open: its page asks"));
    }
    let closes = log(&agents, "claude-code", cx);
    assert_eq!(closes.iter().filter(|e| e.0 == "close_tab").count(), 2);

    let waiting = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 30 }),
        cx,
    );
    let discard = json!({ "tab": 30, "discard": true });
    let discarded = call(&agents, "claude-code", "close_tab", discard, cx)
        .recv()
        .unwrap();
    assert_eq!(discarded.text, "Closed tab 30 without saving.");
    assert!(waiting.try_recv().is_err(), "answered before the core did");
    page_answers(&agents, &browser, 30, Closed::Closed, cx);
    assert_eq!(waiting.recv().unwrap().text, "Closed tab 30.");
}

/// A tab that went to another page before its page answered stays, and the answer says so.
#[gpui::test]
fn a_tab_that_changed_meanwhile_stays(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    opened_with_handler(&agents, &browser, cx);
    let kept = call(
        &agents,
        "claude-code",
        "close_tab",
        json!({ "tab": 30 }),
        cx,
    );
    page_answers(&agents, &browser, 30, Closed::Stays, cx);
    let kept = kept.recv().unwrap();
    assert!(kept.error);
    assert_eq!(
        kept.text,
        "Tab 30 stays open: before its page answered, it went to another page, was unloaded, \
         or the user took it over or pinned it."
    );
}

/// In a tab the agent didn't open, what's unsaved may be the user's: once the page asked to stay,
/// a discard is refused below Full access, and closes it at Full access.
#[gpui::test]
fn discarding_a_page_the_user_gave_needs_full_access(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    // In the agent's Space, a tab it didn't open, as one its page opened is.
    for each in browser
        .tabs
        .borrow_mut()
        .iter_mut()
        .filter(|each| each.0 == 10)
    {
        each.1 = 2;
    }
    browser.asks.borrow_mut().insert(10);
    let discard = json!({ "tab": 10, "discard": true });

    let answer = call(&agents, "claude-code", "close_tab", discard.clone(), cx);
    assert!(
        answer.try_recv().is_err(),
        "refused before the page answered"
    );
    page_answers(&agents, &browser, 10, Closed::Asks, cx);
    let what = "close the tab on en.wikipedia.org without saving";
    refused(
        &agents,
        &browser,
        answer,
        what,
        Why::Discard,
        "en.wikipedia.org",
        cx,
    );
    assert!(
        browser.tabs.borrow().iter().any(|tab| tab.0 == 10),
        "a refusal closed the tab"
    );

    give(&agents, Tier::Full, cx);
    let closed = call(&agents, "claude-code", "close_tab", discard, cx);
    page_answers(&agents, &browser, 10, Closed::Asks, cx);
    let closed = closed.recv().unwrap();
    assert_eq!(closed.text, "Closed tab 10 without saving.");
    assert!(browser.tabs.borrow().iter().all(|tab| tab.0 != 10));
    let entry = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(
        entry,
        ("discard_tab".into(), "en.wikipedia.org".into(), "".into())
    );
}

/// A tab that leaves the agent's Space while its page answers is refused as the core refuses any
/// tab not the agent's, word for word: no refusal of the task's access shows, and nothing closes.
#[gpui::test]
fn a_tab_moved_away_while_its_page_answers_is_the_cores_to_refuse(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let space = |space: u64| {
        for each in (browser.tabs.borrow_mut().iter_mut()).filter(|each| each.0 == 10) {
            each.1 = space;
        }
    };
    space(2);
    browser.asks.borrow_mut().insert(10);
    let discard = json!({ "tab": 10, "discard": true });
    let answer = call(&agents, "claude-code", "close_tab", discard, cx);
    assert!(
        answer.try_recv().is_err(),
        "refused before the page answered"
    );
    // The user moves it to a Space of their own before the page's answer comes.
    space(1);
    page_answers(&agents, &browser, 10, Closed::Asks, cx);
    let refused = answer.recv().unwrap();
    assert!(refused.error);
    assert_eq!(refused.text, "The tab isn't in the agent's own Space.");
    assert!(stops(&agents, cx).is_empty(), "{:?}", stops(&agents, cx));
    assert!(
        browser.tabs.borrow().iter().any(|tab| tab.0 == 10),
        "it closed the tab"
    );
}

/// A pinned tab is the user's to close, and the answer says so alone.
#[gpui::test]
fn a_pinned_tab_stays_open(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    browser.pinned.borrow_mut().insert(20);
    let discard = json!({ "tab": 20, "discard": true });
    let kept = call(&agents, "claude-code", "close_tab", discard, cx)
        .recv()
        .unwrap();
    assert!(kept.error);
    assert_eq!(kept.text, "Tab 20 stays open: the user pinned it.");
    assert!(browser.tabs.borrow().iter().any(|tab| tab.0 == 20));
    let entry = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(
        (entry.0.as_str(), entry.2.as_str()),
        ("close_tab", kept.text.as_str())
    );
}

/// An agent pins its own tab, unpins it and removes it, each in the log (#1360). A pinned tab it
/// didn't pin is the user's, in its Space or theirs, and stays, as does one whose pin the user
/// changed: list_tabs says whose each pin is. A task has no pins.
#[gpui::test]
fn agents_pin_their_own_tabs_and_leave_the_users_pins(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = |tool: &str, tab: u64, cx: &mut TestAppContext| {
        call(&agents, "claude-code", tool, json!({ "tab": tab }), cx)
            .recv()
            .unwrap()
    };
    let listed = |cx: &mut TestAppContext| {
        call(&agents, "claude-code", "list_tabs", json!({}), cx)
            .recv()
            .unwrap()
            .text
    };
    let pinned = answer("pin_tab", 20, cx);
    assert!(
        pinned
            .text
            .starts_with("Pinned tab 20 at the page it's at."),
        "{}",
        pinned.text
    );
    assert!(browser.pinned.borrow().contains(&20));
    assert_eq!(
        listed(cx),
        "* 20  Example Domain  https://www.example.com/  pinned by you"
    );
    assert_eq!(
        answer("unpin_tab", 20, cx),
        ToolResult::text("Unpinned tab 20. It's at the top of your Today tabs.")
    );
    assert!(browser.pinned.borrow().is_empty());
    assert_eq!(
        answer("unpin_tab", 20, cx),
        ToolResult::error("Tab 20 isn't pinned.")
    );
    answer("pin_tab", 20, cx);
    assert_eq!(
        answer("remove_tab", 20, cx),
        ToolResult::text("Removed tab 20. The user can still find it in the archive.")
    );
    assert!(browser.tabs.borrow().iter().all(|tab| tab.0 != 20));
    assert!(browser.pinned.borrow().is_empty());
    let did: Vec<_> = (log(&agents, "claude-code", cx).into_iter())
        .filter(|(action, _, _)| action != "list_tabs")
        .map(|(action, site, refused)| (action, site, refused.is_empty()))
        .collect();
    let did: Vec<_> = did.into_iter().take(5).rev().collect();
    let expected = [
        ("pin_tab", true),
        ("unpin_tab", true),
        ("unpin_tab", false),
        ("pin_tab", true),
        ("remove_tab", true),
    ];
    assert_eq!(
        did,
        expected.map(|(action, done)| (action.into(), "example.com".into(), done))
    );
    assert_eq!(tools::describe("remove_tab"), "Removed a pinned tab");

    browser
        .tabs
        .borrow_mut()
        .push((21, 2, "Docs", "https://docs.example.com/"));
    browser.pinned.borrow_mut().extend([21, 10]);
    assert!(listed(cx).ends_with("21  Docs  https://docs.example.com/  pinned by the user"));
    let theirs = "The user pinned tab 21, so only they unpin it.";
    assert_eq!(answer("unpin_tab", 21, cx), ToolResult::error(theirs));
    let refused = ("unpin_tab".into(), "docs.example.com".into(), theirs.into());
    assert_eq!(log(&agents, "claude-code", cx)[0], refused);
    let theirs = "The user pinned tab 21, so only they remove it.";
    assert_eq!(answer("remove_tab", 21, cx), ToolResult::error(theirs));
    assert_eq!(
        answer("pin_tab", 21, cx),
        ToolResult::text("Tab 21 is pinned already.")
    );
    let theirs = "The user pinned tab 21, so only they unpin it.";
    let answered = answer("unpin_tab", 21, cx);
    assert_eq!(
        answered,
        ToolResult::error(theirs),
        "pin_tab made it the agent's"
    );
    for tool in ["pin_tab", "unpin_tab", "remove_tab"] {
        let refused = ToolResult::error("The tab isn't in the agent's own Space.");
        assert_eq!(answer(tool, 10, cx), refused, "{tool}");
    }
    assert_eq!(*browser.pinned.borrow(), HashSet::from([21, 10]));

    browser
        .tabs
        .borrow_mut()
        .push((22, 2, "Status", "https://status.example.com/"));
    answer("pin_tab", 22, cx);
    agents.update(cx, |agents, _| agents.users_pin(22));
    let theirs = "The user pinned tab 22, so only they remove it.";
    assert_eq!(answer("remove_tab", 22, cx), ToolResult::error(theirs));
    assert!(browser.pinned.borrow().contains(&22));

    let names = tools::names("task-1");
    let pins = ["pin_tab", "unpin_tab", "remove_tab"];
    assert!(
        names.iter().all(|name| !pins.contains(&name.as_str())),
        "{names:?}"
    );
}

/// An agent puts tabs of its own in a split view, adds one, lists it and separates one, each
/// checked by the core and in the log; a tab of the user's is refused, and nothing changes.
#[gpui::test]
fn an_agent_splits_only_its_own_tabs(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    (browser.tabs.borrow_mut()).extend([
        (21, 2, "Shop", "https://shop.example/"),
        (22, 2, "Docs", "https://docs.example/"),
    ]);
    let answer = |tool: &str, arguments: Value, cx: &mut TestAppContext| {
        call(&agents, "claude-code", tool, arguments, cx)
            .recv()
            .unwrap()
    };

    let refused = answer("split_tabs", json!({ "tabs": [20, 10] }), cx);
    assert!(refused.error);
    assert_eq!(refused.text, "The tab isn't in the agent's own Space.");
    assert!(browser.splits.borrow().is_empty());
    let entry = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(entry.0, "split_tabs");
    assert_eq!(entry.2, "The tab isn't in the agent's own Space.");
    let refused = answer("add_to_split", json!({ "tab": 10, "split": 20 }), cx);
    assert_eq!(refused.text, "The tab isn't in the agent's own Space.");
    let wrong = answer("split_tabs", json!({ "tabs": [20, 20] }), cx);
    assert!(wrong.error && wrong.text.starts_with("tabs is two or three"));

    let split = answer("split_tabs", json!({ "tabs": [20, 21] }), cx);
    assert_eq!(split.text, "Tab 20's split view, left to right: 20, 21.");
    let added = answer("add_to_split", json!({ "tab": 22, "split": 21 }), cx);
    assert_eq!(
        added.text,
        "Tab 22's split view, left to right: 20, 21, 22."
    );
    assert_eq!(*browser.splits.borrow(), [vec![20, 21, 22]]);
    let listed = answer("list_split", json!({ "tab": 21 }), cx);
    assert_eq!(
        listed.text,
        "Tab 21's split view, left to right: 20, 21, 22."
    );
    let separated = answer("separate_tab", json!({ "tab": 21 }), cx);
    assert_eq!(separated.text, "Tab 21 is out of its split view.");
    let listed = answer("list_split", json!({ "tab": 21 }), cx);
    assert_eq!(listed.text, "Tab 21 is in no split view.");
    let alone = answer("separate_tab", json!({ "tab": 21 }), cx);
    assert!(alone.error && alone.text == "Tab 21 is in no split view.");

    let logged: Vec<_> = (log(&agents, "claude-code", cx).into_iter())
        .filter(|entry| entry.2.is_empty())
        .map(|entry| entry.0)
        .collect();
    for tool in ["split_tabs", "add_to_split", "list_split", "separate_tab"] {
        assert!(
            logged.iter().any(|each| each == tool),
            "{tool} isn't in {logged:?}"
        );
        assert_ne!(tools::describe(tool), "Acted");
    }
    let checked = browser.checked.borrow();
    assert!(checked.iter().any(|each| each == "separate_tab"));
}

/// Every tool an agent or a task can call says what it did in the activity log in words of its
/// own, not describe's fallback, so a new tool can't land with no entry beside it (#3392).
#[test]
fn every_tool_has_an_activity_log_entry() {
    let mut names = tools::names("claude-code");
    names.extend(tools::names("task-1"));
    // The log names an action as the core's entry does, which is the tool's name but for select's
    // (agents.rs).
    let logged = |name: &str| match name {
        "select" => "select_option".to_string(),
        name => name.to_string(),
    };
    let missing: Vec<_> = (names.iter())
        .filter(|name| tools::describe(&logged(name)) == "Acted")
        .collect();
    assert!(
        missing.is_empty(),
        "these tools have no words in the activity log; give each its line in tools::describe: \
         {missing:?}"
    );
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
/// entry, and answers with what each did and the last one's page. It stops at a step its task's
/// access refuses, saying why.
#[gpui::test]
fn run_steps_runs_its_steps_in_order(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    let pages = browser.pages.borrow().len();
    let wrong = json!({ "tab": 20, "steps": [{ "action": "screenshot" }, { "action": "scroll" }] });
    assert_eq!(
        call(&agents, "claude-code", "run_steps", wrong, cx)
            .recv()
            .unwrap(),
        ToolResult::error(
            "A screenshot is only the last step, as its picture is the run's answer."
        )
    );
    assert_eq!(browser.pages.borrow().len(), pages, "the run took a step");

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
        stopped.text.starts_with(
            "Step 2 failed: ilo stopped this: click “Place order” on example.com. It isn't part \
             of this task's access"
        ),
        "{}",
        stopped.text
    );
    assert!(
        stopped.text.ends_with("1. Scrolled down."),
        "{}",
        stopped.text
    );
    assert_eq!(shown(&agents, cx), None, "the run asked");
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
        agents.update(cx, |agents, cx| agents.page_done(request, tree, None, cx));
    };
    let looks = || browser.pages.borrow().len();
    let thanks = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "heading" }, "name": { "value": "Thanks for your order" } },
    ]})
    .to_string();
    let wrongs = [
        json!({}),
        json!({ "seconds": 31 }),
        json!({ "text": "Thanks", "seconds": 0 }),
        json!({ "text": "Thanks", "gone": "Basket" }),
        json!({ "until": "idle" }),
        json!({ "until": "quiet", "text": "Thanks" }),
    ];
    for wrong in wrongs {
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

/// A run's last step can be a screenshot, checked as one alone: the run answers with what each
/// step did and the picture, as an agent that wants to see the page it acted on would call
/// screenshot next (#3659).
#[gpui::test]
fn a_run_ends_with_a_screenshot(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let steps = json!({ "tab": 20, "steps": [{ "action": "scroll" }, { "action": "screenshot" }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    shop_done(&agents, &browser, cx);
    assert_eq!(
        browser.pages.borrow().last().unwrap().2,
        PageAction::Screenshot
    );
    assert_eq!(browser.checked.borrow().last().unwrap(), "screenshot");
    let ran = page_done(&agents, &browser, run, "/9j/4AAQ".into(), cx);
    assert!(!ran.error);
    assert_eq!(
        ran.to_json()["content"],
        json!([
            { "type": "text", "text": "Ran 2 steps.\n1. Scrolled down.\n2. Took a screenshot of the page." },
            { "type": "image", "data": "/9j/4AAQ", "mimeType": "image/jpeg" },
        ])
    );
    let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    let steps: Vec<&str> = entry
        .steps
        .iter()
        .map(|step| step.action.as_ref())
        .collect();
    assert_eq!(steps, ["scroll", "screenshot"]);
}

/// A check step looks at the page once, checked as a snapshot and logged as a check: the run
/// answers in its one line if the text is there, else fails with the page's outline, as a wait
/// that ran out does (#3659).
#[gpui::test]
fn a_check_step_looks_once_for_its_text(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let looks = || browser.pages.borrow().len();
    let wrongs = [
        json!({}),
        json!({ "text": "Coupon", "seconds": 5 }),
        json!({ "gone": "Coupon" }),
        json!({ "text": "Coupon", "until": "quiet" }),
    ];
    for wrong in wrongs {
        let mut step = json!({ "action": "check" });
        step.as_object_mut()
            .unwrap()
            .extend(wrong.as_object().unwrap().clone());
        let steps = json!({ "tab": 20, "steps": [step] });
        let refused = call(&agents, "claude-code", "run_steps", steps, cx)
            .recv()
            .unwrap();
        assert!(
            refused.text.starts_with("Step 1 failed: check needs text"),
            "{}",
            refused.text
        );
    }
    assert_eq!(looks(), 0, "a check without its text looked");

    let check = json!({ "action": "check", "text": "coupon" });
    let steps = json!({ "tab": 20, "steps": [{ "action": "scroll" }, check] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    shop_done(&agents, &browser, cx);
    assert_eq!(
        browser.pages.borrow().last().unwrap().2,
        PageAction::Snapshot
    );
    assert_eq!(browser.checked.borrow().last().unwrap(), "snapshot");
    let ran = page_done(&agents, &browser, run, shop_tree(), cx);
    assert_eq!(
        ran,
        ToolResult::text("Ran 2 steps.\n1. Scrolled down.\n2. “coupon” is on the page.")
    );
    let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    let steps: Vec<&str> = entry
        .steps
        .iter()
        .map(|step| step.action.as_ref())
        .collect();
    assert_eq!(steps, ["scroll", "check"]);

    let steps = json!({ "tab": 20, "steps": [{ "action": "check", "text": "Thanks" }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    let before = looks();
    let failed = page_done(&agents, &browser, run, shop_tree(), cx);
    assert!(failed.error);
    assert!(
        failed
            .text
            .starts_with("Step 1 failed: “Thanks” isn't on the page.\n\nTab 20"),
        "{}",
        failed.text
    );
    cx.executor().advance_clock(Duration::from_secs(5));
    assert_eq!(looks(), before, "the check looked again");
}

/// A wait for text can take minutes, as a page's search can, up to MAX_TEXT_WAIT, while a wait for
/// a time or for the page to settle stays within MAX_WAIT. The longer it has waited, the less
/// often it looks, up to every 5 s (#3659).
#[gpui::test]
fn a_wait_for_text_takes_minutes_and_looks_less_often(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let look = |tree: String, cx: &mut TestAppContext| {
        let request = browser.pages.borrow().last().unwrap().1;
        agents.update(cx, |agents, cx| agents.page_done(request, tree, None, cx));
        cx.run_until_parked();
    };
    let looks = || browser.pages.borrow().len();
    let refused = |step: Value, cx: &mut TestAppContext| {
        let steps = json!({ "tab": 20, "steps": [step] });
        call(&agents, "claude-code", "run_steps", steps, cx)
            .recv()
            .unwrap()
            .text
    };
    let most = tools::MAX_TEXT_WAIT;
    for step in [
        json!({ "action": "wait", "text": "Thanks", "seconds": most + 1. }),
        json!({ "action": "wait", "gone": "Basket", "seconds": most + 1. }),
        json!({ "action": "wait", "seconds": tools::MAX_WAIT + 1. }),
        json!({ "action": "wait", "until": "quiet", "seconds": tools::MAX_WAIT + 1. }),
    ] {
        let text = refused(step, cx);
        assert!(text.starts_with("Step 1 failed: wait needs text"), "{text}");
    }
    assert_eq!(looks(), 0);

    let wait = json!({ "action": "wait", "text": "Thanks", "seconds": most });
    let run = call(
        &agents,
        "claude-code",
        "run_steps",
        json!({ "tab": 20, "steps": [wait] }),
        cx,
    );
    assert_eq!(looks(), 1);
    look(shop_tree(), cx);
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(
        looks(),
        2,
        "the wait didn't look again at first as often as it does"
    );
    // The core answers a minute in, as a page that took that long would.
    cx.executor().advance_clock(Duration::from_secs(60));
    look(shop_tree(), cx);
    cx.executor().advance_clock(Duration::from_millis(4900));
    assert_eq!(looks(), 2, "a wait a minute in looked within 5 s");
    cx.executor().advance_clock(Duration::from_millis(100));
    assert_eq!(looks(), 3);
    assert!(run.try_recv().is_err());
    let thanks = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Thanks" } },
    ]})
    .to_string();
    look(thanks, cx);
    let ran = run.recv().unwrap();
    assert!(
        ran.text.starts_with("Ran 1 steps.\n1. “Thanks” showed."),
        "{}",
        ran.text
    );
}

/// A check and a screenshot in a run go through the rules their tools do: on a tab of the user's
/// each is refused and logged, and nothing reaches its page.
#[gpui::test]
fn a_runs_check_and_screenshot_keep_to_the_rules(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    for (step, logged) in [
        (json!({ "action": "check", "text": "Main Page" }), "check"),
        (json!({ "action": "screenshot" }), "screenshot"),
    ] {
        let steps = json!({ "tab": 10, "steps": [step] });
        let refused = call(&agents, "claude-code", "run_steps", steps, cx)
            .recv()
            .unwrap();
        assert_eq!(
            refused,
            ToolResult::error("Step 1 failed: The tab isn't in the agent's own Space.")
        );
        let entry = agents.read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
        assert_eq!(entry.action, "run_steps");
        let steps: Vec<(&str, &str)> = (entry.steps.iter())
            .map(|step| (step.action.as_ref(), step.refused.as_ref()))
            .collect();
        assert_eq!(steps, [(logged, "The tab isn't in the agent's own Space.")]);
    }
    assert!(
        browser.pages.borrow().is_empty(),
        "a refused step reached a page"
    );
}

/// The actions the core lets an agent take in its tabs, agent_session.cc's RefusedAction: each a
/// thing the user does in a page, or handing the tab to them.
const CORE_ALLOWS: [&str; 22] = [
    "add_to_split",
    "back",
    "click",
    "close_tab",
    "forward",
    "hand_off",
    "list_split",
    "navigate",
    "pin_tab",
    "press_key",
    "read_page",
    "remove_tab",
    "request",
    "screenshot",
    "scroll",
    "select_option",
    "separate_tab",
    "snapshot",
    "split_tabs",
    "type",
    "unpin_tab",
    "upload",
];

/// What the descriptions tell an agent (#3664): each tool that may take a tab says, as the others
/// do, to give it once there's more than one; the server's instructions and a task's say so too, and
/// name only tools there are; and ask_for_tab and hand_off say when to use them and what follows.
#[test]
fn the_descriptions_say_how_to_use_tabs() {
    for agent in ["claude-code", "task-1"] {
        let listed = tools::list(agent);
        let listed = listed.as_array().unwrap();
        let names: Vec<&str> = (listed.iter())
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        // Those where it may be left out: add_to_split's is the one tab it adds.
        let optional = |tool: &&Value| {
            let required = tool["inputSchema"]["required"].as_array();
            !required.is_some_and(|required| required.contains(&json!("tab")))
        };
        let tab_said: Vec<&str> = (listed.iter().filter(optional))
            .filter_map(|tool| tool["inputSchema"]["properties"]["tab"]["description"].as_str())
            .collect();
        assert!(tab_said.len() > 10, "{agent}: {tab_said:?}");
        assert!(
            tab_said.iter().all(|said| *said == tab_said[0]),
            "{agent}'s tools say the tab differently: {tab_said:?}"
        );
        assert!(tab_said[0].contains("more than one tab"), "{}", tab_said[0]);
        let instructions = match agent {
            "task-1" => tools::TASK_INSTRUCTIONS,
            _ => tools::INSTRUCTIONS,
        };
        assert!(
            instructions.contains("its tab once you have more than one"),
            "{agent}"
        );
        assert!(
            instructions.contains("Calls on different tabs run at the same time"),
            "{agent}"
        );
        // Every snake_case word in them is a tool the agent has.
        let words = instructions.split(|c: char| !(c.is_ascii_lowercase() || c == '_'));
        for word in words.filter(|word| word.contains('_')) {
            assert!(
                names.contains(&word),
                "{agent}'s instructions name {word}, not a tool"
            );
        }
        let said = |name: &str| {
            let tool = listed.iter().find(|tool| tool["name"] == name);
            tool.map(|tool| tool["description"].as_str().unwrap().to_string())
        };
        if let Some(ask) = said("ask_for_tab") {
            assert!(ask.contains("can't go on without it"), "{ask}");
            assert!(ask.contains("say why"), "{ask}");
        }
        let hand_off = said("hand_off").unwrap();
        assert!(hand_off.contains("take a snapshot"), "{hand_off}");
    }
}

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
            "split_tabs",
            "add_to_split",
            "separate_tab",
            "list_split",
            "navigate",
            "snapshot",
            "read_page",
            "click",
            "type",
            "select",
            "press_key",
            "scroll",
            "screenshot",
            "request",
            "run_steps",
            "upload",
            "hand_off",
            "ask_for_tab",
            "pin_tab",
            "unpin_tab",
            "remove_tab",
            "sort_tabs_by_site",
            "title",
            "done",
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
        ("select", tab(json!({ "ref": "e1", "option": "Express" }))),
        ("press_key", tab(json!({ "key": "Tab" }))),
        ("scroll", tab(json!({}))),
        ("screenshot", tab(json!({}))),
        (
            "request",
            tab(json!({ "requests": [{ "method": "GET", "path": "/" }] })),
        ),
        ("navigate", tab(json!({ "url": "back" }))),
        ("navigate", tab(json!({ "url": "forward" }))),
        ("navigate", tab(json!({ "url": "iana.org" }))),
        (
            "run_steps",
            tab(json!({ "steps": [{ "action": "scroll" }] })),
        ),
        ("upload", tab(json!({ "ref": "e1" }))),
        ("hand_off", tab(json!({ "reason": "Sign in" }))),
        ("ask_for_tab", tab(json!({ "reason": "To go on" }))),
        ("pin_tab", tab(json!({}))),
        ("unpin_tab", tab(json!({}))),
        ("pin_tab", tab(json!({}))),
        ("remove_tab", tab(json!({}))),
        ("sort_tabs_by_site", json!({})),
        ("split_tabs", json!({ "tabs": [20, 30] })),
        ("add_to_split", json!({ "tab": 30, "split": 20 })),
        ("list_split", tab(json!({}))),
        ("separate_tab", tab(json!({}))),
        ("close_tab", json!({ "tab": 30 })),
        ("done", json!({})),
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
        "select_option",
        "press_key",
        "scroll",
    ];
    for action in page_actions.into_iter().chain([
        "screenshot",
        "request",
        "back",
        "hand_off",
        "close_tab",
        "pin_tab",
        "unpin_tab",
        "remove_tab",
    ]) {
        assert!(
            checked.contains(action),
            "{action} didn't ask the core: {checked:?}"
        );
    }
}

/// A Boost and Arrange are the user's own view of their pages (#695, #2347): no tool, for an agent
/// from outside or a task, and no run_steps step names them, and the agents' code reaches neither
/// the Boosts nor an arrangement's file.
#[test]
fn no_agent_path_reaches_arrange_or_a_boost() {
    let named = |text: &str| {
        let text = text.to_lowercase();
        ["arrange", "boost", "zap"]
            .iter()
            .any(|word| text.contains(word))
    };
    for agent in ["claude-code", "task-1"] {
        let tools = tools::list(agent).to_string();
        assert!(!named(&tools), "{agent}'s tools name one");
    }
    assert!(!tools::STEPS.iter().any(|step| named(step)));
    let sources = [
        include_str!("../agents.rs"),
        include_str!("tools.rs"),
        include_str!("task.rs"),
        include_str!("server.rs"),
        include_str!("page.rs"),
        include_str!("request.rs"),
        include_str!("runner.rs"),
    ];
    for source in sources {
        assert!(!source.contains("boosts::"), "the agents reach the Boosts");
        assert!(!source.contains("arrange"), "the agents reach Arrange");
    }
}

/// The request tool (#1439): HTTP requests from a tab's page to its own site.
mod requests {
    use super::*;

    /// Answers the request sent last as the core would, and gives what's sent next, if any.
    fn answered(
        agents: &Entity<Agents>,
        browser: &Made,
        status: u16,
        body: &str,
        cx: &mut TestAppContext,
    ) {
        let request = browser.requests.borrow().last().unwrap().1;
        let response = request::Response {
            status,
            kind: "application/json".into(),
            body: body.as_bytes().to_vec(),
            ..request::Response::default()
        };
        agents.update(cx, |agents, cx| agents.request_done(request, response, cx));
    }

    fn sent(browser: &Made) -> Vec<String> {
        let sent = browser.requests.borrow();
        sent.iter()
            .map(|(_, _, each)| format!("{} {}", each.method, each.url))
            .collect()
    }

    fn targets(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<(String, String)> {
        agents.read_with(cx, |agents, _| {
            (agents.activity("claude-code"))
                .filter(|entry| entry.action == "request")
                .map(|entry| (entry.target.to_string(), entry.refused.to_string()))
                .collect()
        })
    }

    /// On a page the agent's outline saw ask for a password, as a sign-in page does, a request's
    /// answer says the site may have answered as to someone signed out, and names hand_off; on
    /// one that doesn't, it says nothing of it (#3661).
    #[gpui::test]
    fn a_request_on_a_sign_in_page_says_so(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        let read = json!({ "tab": 20, "requests": [{ "method": "GET", "path": "/api/me" }] });
        let snapshot = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, snapshot, shop_tree(), cx);
        let answer = call(&agents, "claude-code", "request", read.clone(), cx);
        answered(&agents, &browser, 401, r#"{"error":"sign in"}"#, cx);
        let text = answer.recv().unwrap().text;
        assert!(
            text.starts_with(
                "Tab 20's page asks for a password, as a sign-in page does, so its site may have \
                 answered as to someone signed out. Signing in is the user's to do: hand them the \
                 tab with hand_off, and send the requests again once they hand it back.\n\n\
                 GET /api/me → 401"
            ),
            "{text}"
        );

        let signed_in = json!({ "nodes": [
            { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" } },
        ]})
        .to_string();
        let snapshot = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, snapshot, signed_in, cx);
        let answer = call(&agents, "claude-code", "request", read, cx);
        answered(&agents, &browser, 200, r#"{"name":"Ada"}"#, cx);
        let text = answer.recv().unwrap().text;
        assert!(text.starts_with("GET /api/me → 200"), "{text}");
    }

    /// A read goes at once, and its answer is the body without what looks like a credential. A
    /// call with changes is refused below Full access, its reads with it, and goes whole at Full
    /// access.
    #[gpui::test]
    fn reads_go_and_changes_need_full_access(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        let read = json!({ "tab": 20, "requests": [{ "method": "GET", "path": "/api/me" }] });
        let answer = call(&agents, "claude-code", "request", read, cx);
        assert_eq!(shown(&agents, cx), None, "a read asked");
        assert_eq!(sent(&browser), ["GET https://www.example.com/api/me"]);
        answered(
            &agents,
            &browser,
            200,
            r#"{"name":"Ada","accessToken":"ya29.secret"}"#,
            cx,
        );
        let read = answer.recv().unwrap();
        assert!(!read.error);
        assert!(
            read.text
                .starts_with("GET /api/me → 200 application/json\n\n"),
            "{}",
            read.text
        );
        assert!(
            read.text.contains("Ada") && !read.text.contains("ya29"),
            "{}",
            read.text
        );
        assert!(
            read.text.ends_with("never hold their credentials."),
            "{}",
            read.text
        );
        assert_eq!(targets(&agents, cx), [("GET /api/me".into(), "".into())]);

        let changes = json!({ "tab": 20, "requests": [
            { "method": "GET", "path": "/api/items" },
            { "method": "POST", "path": "/api/items", "body": { "title": "Milk" },
              "what": "Add Milk to the list." },
            { "method": "DELETE", "path": "/api/items/7",
              "what": "Tidy up the old draft so your list is cleaner." },
        ] });
        let answer = call(&agents, "claude-code", "request", changes.clone(), cx);
        let what = "make 2 changes on example.com";
        refused(
            &agents,
            &browser,
            answer,
            what,
            Why::Request,
            "example.com",
            cx,
        );
        assert_eq!(
            sent(&browser).len(),
            1,
            "a call with changes sent something"
        );
        // Each change is logged, in the agent's words, under the refusal.
        assert_eq!(
            targets(&agents, cx)[..2],
            [
                (
                    "DELETE /api/items/7".into(),
                    "Sending changes to a site without its page isn't part of Work on pages."
                        .into()
                ),
                (
                    "POST /api/items".into(),
                    "Sending changes to a site without its page isn't part of Work on pages."
                        .into()
                ),
            ]
        );

        give(&agents, Tier::Full, cx);
        let answer = call(&agents, "claude-code", "request", changes, cx);
        assert_eq!(shown(&agents, cx), None, "a change asked");
        for status in [200, 201, 204] {
            answered(&agents, &browser, status, "", cx);
        }
        let done = answer.recv().unwrap();
        let lines: Vec<&str> = done.text.split("\n\n").collect();
        let expected = [
            "1. GET /api/items → 200 application/json",
            "2. POST /api/items → 201 application/json",
            "3. DELETE /api/items/7 → 204 application/json",
        ];
        assert_eq!(lines, expected);
        assert_eq!(browser.requests.borrow()[2].2.body, r#"{"title":"Milk"}"#);
    }

    /// The log keeps the agent's sentence as theirs, without a value after a secret's name.
    #[gpui::test]
    fn the_log_keeps_a_sentence_without_its_secret(cx: &mut TestAppContext) {
        let (agents, _, _) = connected(cx);
        let change = json!({ "tab": 20, "requests": [
            { "method": "PUT", "path": "/api/settings", "body": { "on": true },
              "what": "Set the API key to sk-live-123" },
        ] });
        let answer = call(&agents, "claude-code", "request", change, cx);
        assert!(answer.recv().unwrap().error);
        let kept = agents.read_with(cx, |agents, _| {
            (agents.activity("claude-code"))
                .filter(|entry| entry.action == "request")
                .map(|entry| entry.said.to_string())
                .collect::<Vec<_>>()
        });
        assert_eq!(kept, ["Set the API key to (hidden)"]);
    }

    /// A write whose override header names a read is still refused below Full access, sending
    /// nothing, and the log names the method sent (#1439).
    #[gpui::test]
    fn a_write_that_says_its_a_read_is_still_a_change(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        let masked = json!({ "tab": 20, "requests": [
            { "method": "POST", "path": "/api/items",
              "headers": { "X-HTTP-Method-Override": "GET" }, "body": { "title": "Milk" },
              "what": "Add Milk to the list." },
        ] });
        let answer = call(&agents, "claude-code", "request", masked, cx);
        let what = "make a change on example.com";
        refused(
            &agents,
            &browser,
            answer,
            what,
            Why::Request,
            "example.com",
            cx,
        );
        assert_eq!(sent(&browser).len(), 0, "a refused call sent something");
        assert_eq!(
            targets(&agents, cx)
                .last()
                .map(|(target, _)| target.as_str()),
            Some("POST as GET /api/items")
        );
    }

    /// The browser's headers, other sites and the user's tabs are refused, and logged, without a
    /// question and without the user's tab's address in the answer. A request that gets no answer
    /// stops the rest.
    #[gpui::test]
    fn requests_keep_to_the_rules(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        let refused = |requests: Value, tab: u64, cx: &mut TestAppContext| {
            let arguments = json!({ "tab": tab, "requests": requests });
            let answer = call(&agents, "claude-code", "request", arguments, cx);
            let answer = answer.recv().unwrap();
            assert!(answer.error, "{}", answer.text);
            assert_eq!(shown(&agents, cx), None, "a refused call asked");
            answer.text
        };
        let cookie = json!([{ "method": "GET", "path": "/", "headers": { "Cookie": "a=1" } }]);
        assert!(refused(cookie, 20, cx).starts_with("Agents can't set the Cookie header"));
        let elsewhere = json!([{ "method": "POST", "path": "https://example.org/a" }]);
        assert!(refused(elsewhere, 20, cx).contains("only to the tab's own site"));
        let theirs = refused(json!([{ "method": "POST", "path": "/w/edit" }]), 10, cx);
        assert!(!theirs.contains("wikipedia"), "{theirs}");
        assert!(sent(&browser).is_empty());
        let refusals: Vec<String> = targets(&agents, cx)
            .into_iter()
            .map(|(_, why)| why)
            .collect();
        assert!(
            refusals.iter().all(|why| !why.is_empty()) && refusals.len() == 3,
            "{refusals:?}"
        );
        // A change without the agent's sentence isn't asked or sent, and the user wasn't asked,
        // so there's no entry for it.
        let unsaid =
            json!([{ "method": "GET", "path": "/a" }, { "method": "PATCH", "path": "/b" }]);
        let unsaid = refused(unsaid, 20, cx);
        assert!(
            unsaid.starts_with("Request 2 (PATCH /b) may change something"),
            "{unsaid}"
        );
        assert!(unsaid.ends_with("Nothing was sent."), "{unsaid}");
        assert!(sent(&browser).is_empty());
        assert_eq!(
            targets(&agents, cx).len(),
            3,
            "a call that asked nothing was logged"
        );

        let two = json!([{ "method": "GET", "path": "/a" }, { "method": "GET", "path": "/b" }]);
        let answer = call(
            &agents,
            "claude-code",
            "request",
            json!({ "tab": 20, "requests": two }),
            cx,
        );
        let request = browser.requests.borrow().last().unwrap().1;
        let error = Some("No answer came: ERR_CONNECTION_REFUSED".into());
        let response = request::Response {
            error,
            ..request::Response::default()
        };
        agents.update(cx, |agents, cx| agents.request_done(request, response, cx));
        let answer = answer.recv().unwrap();
        assert!(answer.error);
        assert_eq!(
            answer.text,
            "1. GET /a: No answer came: ERR_CONNECTION_REFUSED\n\nThe last request wasn't sent."
        );
        assert_eq!(sent(&browser).len(), 1);
    }
}

/// Agents never read or fill a login, address or card the profile has (principle 3). No tool, a
/// task's included, takes one as an argument. A snapshot of a page Chrome filled them into names
/// its fields but gives none of their values, even where the tree carries them, though the core
/// already leaves them out (agent_page.cc's AgentTree, as page_text.cc does for read_page). And
/// typing into its password field is refused before the core hears of it.
#[gpui::test]
fn agents_never_get_a_login_address_or_card(cx: &mut TestAppContext) {
    // The arguments in a tool list's schemas, at any depth, such as a step's in run_steps.
    fn arguments(value: &Value, names: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                if let Some(Value::Object(properties)) = object.get("properties") {
                    names.extend(properties.keys().cloned());
                }
                object.values().for_each(|each| arguments(each, names));
            }
            Value::Array(array) => array.iter().for_each(|each| arguments(each, names)),
            _ => {}
        }
    }
    let mut names = Vec::new();
    arguments(&tools::list("claude-code"), &mut names);
    arguments(&tools::list("task-1"), &mut names);
    assert!(
        names.iter().any(|name| name == "url"),
        "no arguments found: {names:?}"
    );
    let secrets = [
        "password",
        "cookie",
        "credential",
        "autofill",
        "card",
        "address",
        "login",
    ];
    // By the words of each name, so close_tab's discard isn't a card.
    for name in &names {
        let lower = name.to_lowercase();
        let mut words = lower.split(|c: char| !c.is_ascii_alphanumeric());
        assert!(
            !words.any(|word| secrets
                .iter()
                .any(|secret| word == *secret || word.strip_suffix('s') == Some(secret))),
            "a tool takes {name}"
        );
    }

    // A checkout page Chrome filled in: each field with its value as Chromium's tree has it.
    let values = [
        "ada@example.test",
        "hunter2",
        "1 Check Street",
        "4111 1111 1111 1111",
    ];
    let field = |id: &str, name: &str, backend: i64, value: &str| {
        let text = json!({ "type": "string", "value": value });
        json!({ "nodeId": id, "role": { "value": "textbox" }, "name": { "value": name },
                "backendDOMNodeId": backend, "value": text,
                "properties": [{ "name": "valuetext", "value": text }] })
    };
    let mut password = field("3", "Password", 41, values[1]);
    password["password"] = json!(true);
    let tree = json!([
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Checkout" },
          "childIds": ["2", "3", "4", "5"] },
        field("2", "Email", 31, values[0]),
        password,
        field("4", "Street address", 51, values[2]),
        field("5", "Card number", 61, values[3]),
    ]);
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    // As the core answers, with #1606's busy note (agent_page.cc).
    let tree = json!({ "nodes": tree, "busy": false });
    let outline = page_done(&agents, &browser, answer, tree.to_string(), cx).text;
    let fields = [
        "- textbox \"Email\" [e1]",
        "- textbox \"Password\" [e2] (password: off limits)",
        "- textbox \"Street address\" [e3]",
        "- textbox \"Card number\" [e4]",
    ];
    for line in fields {
        assert!(outline.contains(line), "{outline}");
    }
    for value in values {
        assert!(
            !outline.contains(value),
            "the snapshot gave {value}: {outline}"
        );
    }

    let pages = browser.pages.borrow().len();
    let typing = json!({ "tab": 20, "ref": "e2", "text": values[1] });
    assert!(
        call(&agents, "claude-code", "type", typing, cx)
            .recv()
            .unwrap()
            .error
    );
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "the password reached the core"
    );
}

/// Agent tasks: a task is an agent of its own, `task-<n>`, with a hidden Space in the profile it
/// was asked from, and a card that says what it does.
mod tasks {
    use super::*;

    /// What a user reads calls the user's work a thread (Z0), while what an assistant is told keeps
    /// "task", which the sweep leaves until its own change.
    #[test]
    fn a_user_reads_thread_and_an_assistant_is_still_told_task() {
        let says_task = |text: &str| {
            text.to_lowercase()
                .split(|c: char| !c.is_alphabetic())
                .any(|word| word == "task" || word == "tasks")
        };
        let mut read: Vec<String> = [
            crate::agents::tier::NEVER_GETS,
            crate::agents::notes::SWITCH_LINE,
            crate::agents::notes::SWITCH_OFF_LINE,
            crate::agents::notes::FOOT,
            crate::agents::fence::FOOT,
        ]
        .map(String::from)
        .into();
        for agent in ["task-1", "claude-code"] {
            read.extend(
                tools::names(agent)
                    .iter()
                    .map(|name| tools::describe(name).to_string()),
            );
        }
        let said: Vec<&String> = read.iter().filter(|text| says_task(text)).collect();
        assert!(
            said.is_empty(),
            "these strings a user reads still say task, not thread: {said:#?}"
        );

        let told = tools::list("task-1").to_string();
        for (what, text) in [
            ("the instructions", tools::INSTRUCTIONS),
            ("a task's instructions", tools::TASK_INSTRUCTIONS),
            ("a task's tools", told.as_str()),
        ] {
            assert!(
                says_task(text),
                "{what} an assistant is told no longer say task, which they keep for now: {text}"
            );
        }
    }
    use crate::agents::ready::{Found, Ready};
    use crate::agents::runner::{self, Cli, Failure, Out};
    use crate::agents::sign_in::Offer;
    use crate::agents::task::{AskedOn, Dot, State, TITLE_KEPT, TaskAction};

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
            agents.set_threads(true, cx);
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

    /// What the last look found of Claude Code.
    fn set_found(agents: &Entity<Agents>, ready: Ready, cx: &mut TestAppContext) {
        agents.update(cx, |agents, _| {
            agents.tasks.found.get_mut(&Cli::Claude).unwrap().ready = ready
        });
    }

    fn page() -> Option<AskedOn> {
        Some(AskedOn {
            url: "https://en.wikipedia.org/wiki/Arc_(web_browser)".into(),
            title: "Arc (web browser)".into(),
        })
    }

    /// A run of the browser whose agents keep their settings at `path`, with ⌘T's Claude and Codex
    /// signed in.
    fn run_at(path: &std::path::Path, cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, Some(path.to_path_buf()), cx);
        // As the app reads them at launch.
        let saved = std::fs::read_to_string(path).unwrap_or_default();
        agents.update(cx, |agents, cx| {
            agents.load(&saved, cx);
            agents.set_threads(true, cx);
            for cli in Cli::ALL {
                let found = Found {
                    ready: Ready::SignedIn,
                    ..Default::default()
                };
                agents.tasks.found.insert(cli, found);
            }
            agents.tasks.default = Some(Cli::Claude);
        });
        (agents, browser)
    }

    fn start(agents: &Entity<Agents>, asked: &str, cx: &mut TestAppContext) -> String {
        agents
            .update(cx, |agents, cx| {
                agents.start_task(Cli::Claude, asked, page(), 1, Tier::default(), cx)
            })
            .expect("started")
    }

    fn card_of(agents: &Entity<Agents>, cx: &mut TestAppContext) -> crate::agents::task::Card {
        agents.read_with(cx, |agents, cx| agents.cards(1, cx).remove(0))
    }

    fn said(agents: &Entity<Agents>, agent: &str, out: Out, cx: &mut TestAppContext) {
        let event = runner::Event {
            agent: agent.into(),
            run: agents.read_with(cx, |agents, _| agents.run_of(agent)),
            out,
        };
        agents.update(cx, |agents, cx| agents.task_said(event, cx));
    }

    #[gpui::test]
    fn a_sort_runs_a_signed_in_claude_alone_while_tasks_are_on(cx: &mut TestAppContext) {
        let program = PathBuf::from("/opt/claude");
        let (agents, _) = ready(Some(program.clone()), cx);
        let sort =
            |cx: &mut TestAppContext| agents.read_with(cx, |agents, _| agents.sort_program());
        let found = sort(cx).expect("Claude signed in sorts");
        assert!(agents.read_with(cx, |agents, _| agents.sorts()));
        assert_eq!((found.program, found.model), (program, None));
        set_found(&agents, Ready::SignedOut, cx);
        assert!(sort(cx).is_none(), "signed out");
        assert!(!agents.read_with(cx, |agents, _| agents.sorts()));
        set_found(&agents, Ready::SignedIn, cx);
        agents.update(cx, |agents, cx| agents.set_threads(false, cx));
        assert!(sort(cx).is_none(), "threads off");
        agents.update(cx, |agents, cx| agents.set_threads(true, cx));
        agents.update(cx, |agents, cx| {
            let found = Found {
                ready: Ready::SignedIn,
                program: Some("/opt/codex".into()),
                ..Default::default()
            };
            agents.tasks.found.insert(Cli::Codex, found);
            agents.set_default_task_agent(Cli::Codex, cx);
        });
        assert!(
            sort(cx).is_none(),
            "Codex has no switch keeping a run to the list"
        );
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
            (
                "Find a flight to Lisbon",
                "Thinking · no steps yet",
                Dot::Working
            )
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
        // As the look its failure starts finds it.
        set_found(&agents, Ready::NotFound, cx);
        let card = card_of(&agents, cx);
        assert_eq!(card.status.as_ref(), "Claude isn't installed.");
        // Its row's own Get.
        assert_eq!(card.fix, Some((Cli::Claude, Offer::Get)));
        assert_eq!(card.dot, Dot::Ended);
    }

    /// The card offers the install for the fact that the program isn't there, never for a
    /// failure's words: what a CLI says of something of its own that "isn't installed" is its
    /// own failure, said as it said it, with no fix of the assistant's.
    #[gpui::test]
    fn the_install_is_offered_for_the_program_missing_not_for_a_failures_words(
        cx: &mut TestAppContext,
    ) {
        let (agents, _) = ready(None, cx);
        start(&agents, "Find a flight", cx);
        set_found(&agents, Ready::NotFound, cx);
        let agent = card_of(&agents, cx).agent.clone();
        let fails = |failure: Failure, cx: &mut TestAppContext| {
            agents.update(cx, |agents, cx| agents.fail(&agent, failure, cx));
            card_of(&agents, cx)
        };

        let words = "The plugin it needs isn't installed.";
        let card = fails(Failure::Other(words.into()), cx);
        assert_eq!(card.status.as_ref(), words);
        assert_eq!(card.fix, None, "a failure's words chose the card's form");

        let card = fails(Failure::NotInstalled, cx);
        assert_eq!(card.status.as_ref(), "Claude isn't installed.");
        assert_eq!(card.fix, Some((Cli::Claude, Offer::Get)));
    }

    /// The inside switch off: ⌘T's ask lists no agent, no thread starts, and one at its limit
    /// offers no Retry and takes no reply or retry; other apps' agents work on, and on again a
    /// thread starts.
    #[gpui::test]
    fn with_tasks_off_no_task_starts_and_a_card_asks_nothing_more(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Find a flight", cx);
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.state = State::Limit { resets: None };
        });
        assert!(card_of(&agents, cx).retry);
        agents.update(cx, |agents, cx| {
            agents.set_other_apps(true, cx);
            agents.set_threads(false, cx);
        });
        agents.read_with(cx, |agents, _| {
            assert!(agents.listening(), "threads off turned other apps off");
            assert!(!agents.threads());
            assert!(
                agents.ask_agents().is_empty(),
                "⌘T's ask still lists agents"
            );
        });
        let started = agents.update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, "Another", page(), 1, Tier::default(), cx)
        });
        assert_eq!(started, None, "a task started with tasks off");
        assert!(!card_of(&agents, cx).retry, "the card still offers Retry");
        let messages = |cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().messages.len())
        };
        let before = messages(cx);
        agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::Reply("And a hotel".into()), cx);
            agents.task_action(&agent, TaskAction::Retry, cx);
        });
        assert_eq!(
            messages(cx),
            before,
            "a reply reached the task with tasks off"
        );
        assert!(matches!(
            agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().state.clone()),
            State::Limit { .. }
        ));
        agents.update(cx, |agents, cx| agents.set_threads(true, cx));
        assert!(agents.read_with(cx, |agents, _| !agents.ask_agents().is_empty()));
        assert!(card_of(&agents, cx).retry);
    }

    /// The inside switch off ends a thread as Stop does: its ask under way has its answer, a
    /// reply after reaches nothing, its token goes and its connection closes.
    #[gpui::test]
    fn with_tasks_off_a_reply_still_answers_the_ask(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Move the ten cases to Closed", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let ask = json!({ "question": "Close all ten?", "answers": ["Yes", "No"] });
        let asked = call(&agents, &agent, "ask", ask, cx);
        let token = agents.read_with(cx, |agents, _| {
            agents.task(&agent).unwrap().token().to_string()
        });
        agents.update(cx, |agents, cx| agents.set_threads(false, cx));
        assert!(
            asked.recv().map_or(true, |answer| answer.error),
            "the ask went on"
        );
        agents.read_with(cx, |agents, _| {
            assert!(matches!(agents.task(&agent).unwrap().state, State::Stopped));
            assert!(!agents.tokens.lock().unwrap().contains_key(&token));
        });
        let messages = |cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().messages.len())
        };
        let before = messages(cx);
        agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::Reply("Yes".into()), cx);
        });
        assert_eq!(
            messages(cx),
            before,
            "a reply reached a thread with threads off"
        );
    }

    /// Every tool `dispatch` runs is listed for an agent that may call it, as a call by any other
    /// name is answered as no tool before it gets there: an arm listed for nobody would never run,
    /// and one only a task runs is listed for a task's. A made-up name is no tool, and leaves no
    /// entry.
    #[gpui::test]
    fn every_tool_dispatch_runs_is_listed_and_no_other_is_one(cx: &mut TestAppContext) {
        let source = include_str!("../agents.rs");
        let body = &source[source.find("    fn dispatch(").expect("no dispatch")..];
        let body = &body[..body[1..]
            .find("\n    fn ")
            .map_or(body.len(), |end| end + 1)];
        let (task, outside) = (tools::names("task-1"), tools::names("claude-code"));
        let mut arms = Vec::new();
        for line in body.lines() {
            // An arm of its match, at its depth, by a name or a few.
            let Some(arm) = line.strip_prefix("            \"") else {
                continue;
            };
            let Some((pattern, _)) = arm.split_once("=>") else {
                continue;
            };
            let pattern = format!("\"{pattern}");
            for name in pattern.split('"').skip(1).step_by(2) {
                let listed = match pattern.contains("if is_task(") {
                    true => task.iter().any(|each| each == name),
                    false => task.iter().chain(&outside).any(|each| each == name),
                };
                assert!(
                    listed,
                    "dispatch runs {name}, which no agent that may call it is listed"
                );
                arms.push(name.to_string());
            }
        }
        assert!(arms.len() > 15, "dispatch's arms weren't found: {arms:?}");

        let (agents, _, _) = connected(cx);
        let before = log(&agents, "claude-code", cx);
        let answer = call(&agents, "claude-code", "sum_the_receipts", json!({}), cx);
        assert_eq!(
            answer.recv().unwrap(),
            ToolResult::error("There's no tool sum_the_receipts.")
        );
        assert_eq!(log(&agents, "claude-code", cx), before);
    }

    #[gpui::test]
    fn the_task_tools_are_a_tasks_alone(cx: &mut TestAppContext) {
        let names = tools::names("task-1");
        for tool in ["title", "ask", "finish", "open_tab", "hand_off"] {
            assert!(names.iter().any(|name| name == tool), "{tool}");
        }
        // An agent from outside names its session as a task does (#3690), and finishes nothing.
        let outside = tools::names("claude-code");
        assert!(!outside.iter().any(|name| name == "finish" || name == "ask"));
        assert!(outside.iter().any(|name| name == "title"));
        let (agents, _, _) = connected(cx);
        let answer = call(
            &agents,
            "claude-code",
            "finish",
            json!({ "result": "Flights" }),
            cx,
        );
        assert_eq!(
            answer.recv().unwrap(),
            ToolResult::error("There's no tool finish.")
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
        assert!(asked.try_recv().is_err(), "ask waits for the user");
        agents.update(cx, |agents, cx| agents.reply(&agent, "Saturday", cx));
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
            // The oldest is the start's, which keeps the level it starts at.
            ["finish", "answered", "ask", "title", "access"],
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
        // Open <site> moves the tab to the user's Space, theirs now, with its sound (#1080).
        let shown = agents.update(cx, |agents, cx| agents.open_result(&agent, None, cx));
        assert_eq!(shown, Some(30), "it shows");
        assert_eq!(
            browser.muted.borrow().last(),
            Some(&(30, false)),
            "kept, it's still muted"
        );
        assert!(browser.tabs.borrow().contains(&(
            30,
            1,
            "Checkout",
            "https://shop.example/checkout"
        )));
        assert_eq!(card_of(&agents, cx).open, Vec::new());
    }

    /// Beside a_sessions_title_is_never_where_ilo_or_the_user_speaks: a ⌘T task's title, which
    /// its assistant may set, goes into its notification once, as its card caps it, on one line,
    /// before the middle dot, and nothing else of the task goes with it: not its question, its
    /// result, its site nor the user's ask.
    #[gpui::test]
    fn a_tasks_title_is_in_its_notification_once_and_nothing_else(cx: &mut TestAppContext) {
        use crate::needs_you::{self, Host, Notify};
        #[derive(Default)]
        struct Posted(RefCell<Vec<(String, String)>>);
        impl Host for Posted {
            fn badge(&self, _: usize) {}
            fn active(&self) -> bool {
                false
            }
            fn notify(&self, _: u64, title: &str, body: &str) {
                self.0.borrow_mut().push((title.into(), body.into()));
            }
        }
        let (agents, browser) = ready(None, cx);
        let posted = Rc::new(Posted::default());
        cx.update(|cx| needs_you::open(posted.clone(), cx));
        let agent = start(&agents, "Find a flight to Lisbon on shop.example", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let follow = |cx: &mut TestAppContext| {
            let cards = vec![card_of(&agents, cx)];
            cx.update(|cx| needs_you::follow(&cards, Notify::Away, cx));
        };
        follow(cx);
        let named = "Flights to Lisbon\nand a hotel near the river for the whole week";
        let answer = call(&agents, &agent, "title", json!({ "text": named }), cx);
        assert!(!answer.recv().unwrap().error);
        let title = card_of(&agents, cx).title.to_string();
        assert!(
            title.len() < named.len() && !title.contains('\n'),
            "{title:?}"
        );
        let asked = call(
            &agents,
            &agent,
            "ask",
            json!({ "question": "Which day?", "answers": ["Friday", "Saturday"] }),
            cx,
        );
        follow(cx);
        agents.update(cx, |agents, cx| agents.reply(&agent, "Saturday", cx));
        asked.recv().unwrap();
        follow(cx);
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "Checkout", "https://shop.example/checkout"));
        let done =
            json!({ "result": "Found a flight for €80.", "summary": "€80 flight", "tabs": [30] });
        assert!(
            !call(&agents, &agent, "finish", done, cx)
                .recv()
                .unwrap()
                .error
        );
        follow(cx);

        let posted = posted.0.take();
        assert_eq!(
            posted,
            [
                (format!("{title} · Needs you"), String::new()),
                (format!("{title} · Done"), String::new()),
            ]
        );
        for (words, body) in &posted {
            assert_eq!(words.matches(title.as_str()).count(), 1, "{words}");
            assert!(words.starts_with(&title) && body.is_empty(), "{words}");
            for never in [
                "Which day",
                "Friday",
                "€80",
                "shop.example",
                "Find a flight",
            ] {
                assert!(!words.contains(never), "{never} in {words}");
            }
        }
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
        let theirs = "Tab 50 isn't one of yours: give the ids of your own tabs, from list_tabs.";
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
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("Thinking · no steps yet", Dot::Working)
        );
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
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("Thinking · no steps yet", Dot::Working)
        );
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
        // A reply answers it.
        agents.update(cx, |agents, cx| agents.reply(&agent, "Porto", cx));
        assert_eq!(
            _asked.recv().unwrap(),
            ToolResult::text("The user answered: Porto")
        );
    }

    #[gpui::test]
    fn a_refusal_or_a_handoff_shows_on_the_card(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
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
        assert!(opening.try_recv().expect("refused at once").error);
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("Stopped at chase.com", Dot::Waiting)
        );
        assert!(card.question.is_none(), "a refusal asked");
        // One state, one word: the conversation's header follows the card.
        assert_eq!(
            agents
                .read_with(cx, |agents, cx| agents.conversation(&agent, cx))
                .expect("its conversation")
                .about
                .as_ref(),
            "Claude · Needs you",
            "the header and the card say different things at a refusal"
        );
        assert_eq!(shown(&agents, cx), None);
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
            ("Needs you · Sign in, please", Dot::Waiting)
        );
        assert_eq!(card.question.map(|question| question.tab), Some(31));
    }

    /// An agent's calls on several tabs go at once (#1476), so its prompts can come together: they
    /// show one at a time, in the order they came, the handoff after the ask waiting for it.
    #[gpui::test]
    fn an_agents_prompts_show_one_at_a_time_in_turn(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Move the ten cases to Closed", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let ask = json!({ "question": "Close all ten?", "answers": ["Yes", "No"] });
        let asked = call(&agents, &agent, "ask", ask, cx);
        browser
            .tabs
            .borrow_mut()
            .push((31, 100, "Sign in", "https://example.com/login"));
        let handoff = json!({ "reason": "Sign in, please.", "tab": 31 });
        let _handed = call(&agents, &agent, "hand_off", handoff, cx);
        let card = card_of(&agents, cx);
        assert!(
            card.asks && card.question.is_none(),
            "the handoff went first"
        );
        assert_eq!(card.status.as_ref(), "Close all ten?");
        assert_eq!(shown(&agents, cx), None, "a window asked out of turn");
        agents.update(cx, |agents, cx| agents.reply(&agent, "Yes", cx));
        assert!(!asked.recv().unwrap().error);
        let question = shown(&agents, cx).expect("the handoff never asked");
        assert_eq!(question.from.as_ref(), agent);
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.asks),
            ("Needs you · Sign in, please", false)
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
        // As the look its failure starts finds it: the card offers its row's own Sign in.
        set_found(&agents, Ready::SignedOut, cx);
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.fix),
            (
                "Claude isn't signed in.",
                Some((Cli::Claude, Offer::SignIn))
            )
        );
        // Signed in since, it offers nothing more.
        set_found(&agents, Ready::SignedIn, cx);
        assert_eq!(card_of(&agents, cx).fix, None);
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
        traced_by(
            agents,
            agent,
            tool,
            arguments,
            cx,
            |answer, cx| match page {
                Some(text) => page_done(agents, browser, answer, text, cx),
                None => answer.recv().unwrap(),
            },
        )
    }

    /// `traced`, with `answer` taking the call's reply: for a call the core answers later.
    fn traced_by(
        agents: &Entity<Agents>,
        agent: &str,
        tool: &str,
        arguments: Value,
        cx: &mut TestAppContext,
        answer: impl FnOnce(mpsc::Receiver<ToolResult>, &mut TestAppContext) -> ToolResult,
    ) -> ToolResult {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (reply, receiver) = mpsc::channel();
        let at = std::time::Instant::now();
        let call = Call {
            id,
            agent: agent.into(),
            connection: 1,
            tool: tool.into(),
            arguments,
            reply,
            at,
        };
        agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
        let result = answer(receiver, cx);
        let error = result.error.then(|| result.text.clone());
        let at = std::time::Instant::now();
        let answered = Event::Answered {
            agent: agent.into(),
            id,
            at,
            error,
            kind: result.kind,
            image: result.image.clone(),
            notes: result.notes.map(|note| note.name),
        };
        agents.update(cx, |agents, cx| agents.handle(answered, cx));
        result
    }

    /// A task at work with one step taken on tab 30, a shop's basket, and tab 31 open beside it.
    fn at_work(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, String) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Use the coupon", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let mut tabs = browser.tabs.borrow_mut();
        tabs.push((30, 100, "Shop", "https://shop.example/basket"));
        tabs.push((31, 100, "Sign in", "https://example.com/login"));
        drop(tabs);
        let snapshot = json!({ "tab": 30 });
        traced(
            &agents,
            &browser,
            &agent,
            "snapshot",
            snapshot,
            Some(shop_tree()),
            cx,
        );
        (agents, browser, agent)
    }

    /// The live state, and the head of the last stretch of work with whether it shows as under way.
    fn stretch_head(
        agents: &Entity<Agents>,
        agent: &str,
        cx: &mut TestAppContext,
    ) -> (task::Live, String, bool) {
        agents.read_with(cx, |agents, cx| {
            let conversation = agents.conversation(agent, cx).unwrap();
            let trace = (conversation.lines.iter().rev())
                .find_map(|line| match line {
                    task::Line::Trace(trace) => Some(trace),
                    _ => None,
                })
                .expect("a stretch of work");
            (
                conversation.live,
                trace.summary().0.to_string(),
                trace.running,
            )
        })
    }

    /// The head over a stretch reads the task's state, as the card's word does (set 4, find 5):
    /// while the task waits on the user the stretch before is worked, never "Working".
    fn heads_as_worked(live: task::Live, head: (task::Live, String, bool)) {
        let (now, title, running) = head;
        assert_eq!(now, live);
        assert!(
            title.starts_with("Worked for") && !running,
            "a stretch headed {title:?} (under way: {running}) while the task was {live:?}"
        );
    }

    #[gpui::test]
    fn a_stretch_heads_as_working_while_its_task_works(cx: &mut TestAppContext) {
        let (agents, _browser, agent) = at_work(cx);
        let (live, title, running) = stretch_head(&agents, &agent, cx);
        assert_eq!(
            (live, title.as_str(), running),
            (task::Live::Working, "Working", true)
        );
    }

    #[gpui::test]
    fn a_stretch_that_ended_at_a_refused_step_heads_as_worked(cx: &mut TestAppContext) {
        let (agents, browser, agent) = at_work(cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().tier = Tier::Look
        });
        // Its answer comes back as the app's own refusal, which ends the step.
        let click = json!({ "tab": 30, "ref": "e1" });
        assert!(traced(&agents, &browser, &agent, "click", click, None, cx).error);
        heads_as_worked(task::Live::Refused, stretch_head(&agents, &agent, cx));
    }

    /// What the assistant writes after a refused step shows as it comes while the task waits,
    /// and the head over the stretch still doesn't say it's at work (the lead, 01:53Z).
    #[gpui::test]
    fn words_written_after_a_refused_step_show_while_the_stretch_heads_as_worked(
        cx: &mut TestAppContext,
    ) {
        let (agents, browser, agent) = at_work(cx);
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.tier = Tier::Look;
            // Working, as a task under way is: its refusal's line keeps its buttons.
            task.ended = None;
        });
        let click = json!({ "tab": 30, "ref": "e1" });
        assert!(traced(&agents, &browser, &agent, "click", click, None, cx).error);
        said(
            &agents,
            &agent,
            Out::Writing("I can't apply the".into()),
            cx,
        );
        heads_as_worked(task::Live::Refused, stretch_head(&agents, &agent, cx));
        let lines = agents.read_with(cx, |agents, cx| {
            agents.conversation(&agent, cx).unwrap().lines
        });
        let written = lines
            .iter()
            .position(|line| matches!(line, task::Line::Said(text) if text == "I can't apply the"));
        let stretch = lines
            .iter()
            .rposition(|line| matches!(line, task::Line::Trace(_)));
        assert!(
            written.is_some_and(|written| stretch.is_some_and(|stretch| written > stretch)),
            "the words being written should show under the stretch while it waits: {lines:?}"
        );
        assert!(
            matches!(lines.last(), Some(task::Line::Ilo(_))),
            "the refusal's line, with its buttons, should stay last, the words above it: {lines:?}"
        );
    }

    #[gpui::test]
    fn a_stretch_before_a_handed_step_heads_as_worked(cx: &mut TestAppContext) {
        let (agents, _browser, agent) = at_work(cx);
        let reason = json!({ "reason": "Sign in, please.", "tab": 31 });
        let _handed = call(&agents, &agent, "hand_off", reason, cx);
        heads_as_worked(task::Live::Handed, stretch_head(&agents, &agent, cx));
    }

    #[gpui::test]
    fn a_stretch_before_a_question_heads_as_worked(cx: &mut TestAppContext) {
        let (agents, _browser, agent) = at_work(cx);
        let question = json!({ "question": "Which size?" });
        let _asked = call(&agents, &agent, "ask", question, cx);
        heads_as_worked(task::Live::Asked, stretch_head(&agents, &agent, cx));
    }

    #[gpui::test]
    fn a_stretch_before_the_user_took_its_page_heads_as_worked(cx: &mut TestAppContext) {
        let (agents, _browser, agent) = at_work(cx);
        agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::TakeOver(30), cx)
        });
        let (live, ..) = stretch_head(&agents, &agent, cx);
        assert!(
            matches!(live, task::Live::Yours { tab: 30, .. }),
            "{live:?}"
        );
        let head = stretch_head(&agents, &agent, cx);
        heads_as_worked(live, head);
    }

    /// A card's status says how far its task has got, from its own steps and never a clock (#1080):
    /// Thinking with no steps yet, then "Step N · <step>", "Needs you" ahead of what a handoff
    /// waits for, and "Stopped after N steps" once the user stops it. Only the count and "Needs
    /// you" lead it, which the card sets heavier.
    #[gpui::test]
    fn a_cards_status_says_how_far_its_task_has_got(cx: &mut TestAppContext) {
        use crate::agents::task::status_lead;
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Use the coupon", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let status = |cx: &mut TestAppContext| card_of(&agents, cx).status.to_string();
        assert_eq!(status(cx), "Thinking · no steps yet");
        assert_eq!(status_lead(&status(cx)), 0);
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "Shop", "https://shop.example/basket"));
        let snapshot = json!({ "tab": 30 });
        traced(
            &agents,
            &browser,
            &agent,
            "snapshot",
            snapshot,
            Some(shop_tree()),
            cx,
        );
        assert_eq!(status(cx), "Step 1 · Read shop.example");
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
        assert_eq!(status(cx), "Step 2 · Typed “SPRING10” in “Coupon”");
        assert_eq!(status_lead(&status(cx)), "Step 2".len());
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
            ("Needs you · Sign in, please", Dot::Waiting)
        );
        assert_eq!(status_lead(&card.status), "Needs you".len());
        agents.update(cx, |agents, cx| agents.stop_task(&agent, cx));
        let card = card_of(&agents, cx);
        assert_eq!(
            (card.status.as_ref(), card.dot),
            ("Stopped after 3 steps", Dot::Ended)
        );
        assert_eq!(
            status_lead(&card.status),
            0,
            "a stopped card asks for nothing"
        );
    }

    /// A step that chose an option with a line break in it says it on one line, as the card's
    /// status and the trace's title are a line each.
    #[gpui::test]
    fn a_chosen_options_line_break_makes_no_second_line_of_the_step(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Use the coupon", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "Shop", "https://shop.example/basket"));
        let snapshot = json!({ "tab": 30 });
        traced(
            &agents,
            &browser,
            &agent,
            "snapshot",
            snapshot,
            Some(shop_tree()),
            cx,
        );
        let choose = json!({ "tab": 30, "ref": "e1", "option": "SPRING\n10" });
        traced(
            &agents,
            &browser,
            &agent,
            "select",
            choose,
            Some(shop_tree()),
            cx,
        );
        let status = card_of(&agents, cx).status.to_string();
        assert_eq!(status, "Step 2 · Chose “SPRING 10” in “Coupon”");
    }

    /// The first message quotes the page's title in ilo's curly quotes whole, as plain text, so
    /// the title can't end the quote and name another address as the page's.
    #[test]
    fn a_titles_quotes_cant_close_the_first_messages() {
        let page = AskedOn {
            url: "https://shop.example/".into(),
            title: "Shop\u{201D}, https://bank.example. \u{201C}x".into(),
        };
        assert_eq!(
            crate::agents::task::first_message("Find it", Some(&page)),
            "Find it\n\n(I'm on \u{201C}Shop', https://bank.example. 'x\u{201D}, \
             https://shop.example/. Open it in a tab of your own if you need it.)"
        );
    }

    /// A step's line on the task's card quotes a control's name in ilo's curly quotes whole: a
    /// page's own double quotation marks, and those drawn like them, show as one single mark.
    #[gpui::test]
    fn a_names_quotes_cant_close_the_steps(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Go on", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "Shop", "https://shop.example/basket"));
        let tree = shop_tree_named("Next\u{201D} on github.com, then \u{301D}Back");
        let (snapshot, click) = (json!({ "tab": 30 }), json!({ "tab": 30, "ref": "e3" }));
        let seen = Some(tree.clone());
        traced(&agents, &browser, &agent, "snapshot", snapshot, seen, cx);
        traced(&agents, &browser, &agent, "click", click, Some(tree), cx);
        let status = card_of(&agents, cx).status.to_string();
        assert_eq!(
            status,
            "Step 2 · Clicked \u{201C}Next' on github.com, then 'Back\u{201D}"
        );
    }

    /// A task's calls are its conversation's trace, each titled by what it did and opening to what
    /// it saw or why it failed. What it typed shows, but never what it typed into a password
    /// field, though the field refused it.
    /// ilo's line goes where it happened among the task's steps (#4132): the steps before a
    /// refusal, its line, then the steps after, never every ilo line after the turn's work.
    #[gpui::test]
    fn an_ilo_line_stands_where_it_came_among_the_steps(cx: &mut TestAppContext) {
        use crate::agents::task::{Line, TraceItem};
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Check my balance", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "Shop", "https://shop.example/basket"));
        let snapshot = json!({ "tab": 30 });
        traced(
            &agents,
            &browser,
            &agent,
            "snapshot",
            snapshot.clone(),
            Some(shop_tree()),
            cx,
        );
        let bank = json!({ "url": "https://www.chase.com/" });
        assert!(traced(&agents, &browser, &agent, "open_tab", bank, None, cx).error);
        traced(
            &agents,
            &browser,
            &agent,
            "snapshot",
            snapshot,
            Some(shop_tree()),
            cx,
        );
        let lines = agents.read_with(cx, |agents, cx| {
            agents.conversation(&agent, cx).unwrap().lines
        });
        let tools = |line: &Line| match line {
            Line::Trace(trace) => (trace.items.iter())
                .filter_map(|item| match item {
                    TraceItem::Step(step) => Some(step.title.to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(", "),
            Line::Ilo(_) => "ilo".into(),
            other => format!("{other:?}"),
        };
        let order: Vec<String> = lines.iter().map(tools).collect();
        assert_eq!(
            order.len(),
            3,
            "the steps before the refusal, its line, the steps after: {order:?}"
        );
        assert_eq!(order[1], "ilo", "{order:?}");
        assert!(
            matches!(lines[2], Line::Trace(ref trace) if trace.running),
            "{order:?}"
        );
        // The refused row stands alone in the refusal's red, and the line under it says why in
        // the step's own under-way words (B1).
        let Line::Trace(before) = &lines[0] else {
            panic!("{order:?}")
        };
        let Some(TraceItem::Step(refused)) = before.items.last() else {
            panic!("{order:?}")
        };
        assert!(refused.failed, "{order:?}");
        assert_eq!(
            refused.description.as_ref(),
            "",
            "the refused row keeps a tail"
        );
        let Line::Ilo(line) = &lines[1] else {
            panic!("{order:?}")
        };
        let crate::agents::task::ilo::IloKind::Stopped { doing, .. } = &line.kind else {
            panic!("{line:?}")
        };
        assert_eq!(doing.as_ref(), "opening chase.com");
    }

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
                TraceItem::Said(_) | TraceItem::Helper { .. } => None,
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
            "Step 4 · Looked at shop.example",
            "the card says how far it has got and the step it's on"
        );
        assert_eq!(
            conversation.about.as_ref(),
            "Claude · Working",
            "the conversation's header says where the task is, its pages in the rail beside it"
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

    /// A close the page asked to stay through reads as the tab left open, not as a failure, with
    /// why joined to it, and the discard after it as closed without saving, among a plain close's
    /// line (#3256).
    #[gpui::test]
    fn a_close_the_page_kept_reads_as_kept_not_failed(cx: &mut TestAppContext) {
        use crate::agents::task::{Line, Opens, TraceItem};
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Tidy my tabs", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        // The agent opens the draft, and the basket is another tab in its Space.
        let url = json!({ "url": "https://docs.example/draft" });
        traced(
            &agents,
            &browser,
            &agent,
            "open_tab",
            url,
            Some(shop_tree()),
            cx,
        );
        let (draft, space, ..) = *browser.tabs.borrow().last().unwrap();
        let basket = draft + 1;
        let shop = (basket, space, "Shop", "https://shop.example/basket");
        browser.tabs.borrow_mut().push(shop);
        browser.asks.borrow_mut().insert(draft);
        let basket = json!({ "tab": basket });
        assert!(!traced(&agents, &browser, &agent, "close_tab", basket, None, cx).error);
        // The core asks the draft's page, a discard too, and the page answers from a task of its
        // own that it asks to stay.
        let mut asked = |arguments: Value, cx: &mut TestAppContext| {
            traced_by(&agents, &agent, "close_tab", arguments, cx, |answer, cx| {
                page_answers(&agents, &browser, draft, Closed::Asks, cx);
                answer.recv().unwrap()
            })
        };
        assert!(
            asked(json!({ "tab": draft }), cx).error,
            "the page asked to stay"
        );
        let discard = asked(json!({ "tab": draft, "discard": true }), cx);
        assert_eq!(discard.text, format!("Closed tab {draft} without saving."));

        let conversation =
            agents.read_with(cx, |agents, cx| agents.conversation(&agent, cx).unwrap());
        let [Line::Trace(trace)] = conversation.lines.as_slice() else {
            panic!("{:?}", conversation.lines)
        };
        let closes: Vec<_> = (trace.items.iter())
            .filter_map(|item| match item {
                TraceItem::Step(step)
                    if step.title.starts_with("Closed") || step.title.starts_with("Left") =>
                {
                    Some(step)
                }
                _ => None,
            })
            .collect();
        let lines: Vec<_> = (closes.iter())
            .map(|step| (step.title.as_ref(), step.description.as_ref(), step.failed))
            .collect();
        assert_eq!(
            lines,
            [
                ("Closed shop.example", "", false),
                // The reason after a dot, in the title's ink, as History joins a row's parts.
                (
                    "Left docs.example open · asked to stay: unsaved changes",
                    "",
                    false
                ),
                ("Closed docs.example without saving", "", false),
            ]
        );
        assert!(
            matches!(closes[1].opens, Opens::Nothing),
            "it opened to what the agent was told"
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

    /// A failed step says why when its call's kind gives a reason, and nothing when it doesn't,
    /// as its title already says it couldn't. A failed open opens to its address over the error.
    #[test]
    fn a_failed_step_says_why_only_when_it_knows() {
        use crate::agents::task::{Line, Opens, TraceItem, Traced, fold};
        use crate::agents::tools::Kind;
        let reasons = [
            (None, ""),
            (Some(Kind::Stopped), "stopped"),
            (Some(Kind::SaidNo), "you said no"),
            (Some(Kind::Outside), ""),
            (Some(Kind::Password), "password field"),
            (Some(Kind::NotWebPage), "not a web page"),
            (Some(Kind::NotFound), "not found"),
            (Some(Kind::StillLoading), "still loading"),
            (Some(Kind::DidntShow), "didn't show"),
            (Some(Kind::Yours), "it's yours"),
        ];
        for (kind, reason) in reasons {
            assert_eq!(tools::reason(kind), reason, "{kind:?}");
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
            error: Some("Agents may open only http and https pages.".into()),
            kind: Some(Kind::NotWebPage),
            picture: None,
            close: None,
            notes: None,
        };
        let lines = fold(
            &[],
            &[failed],
            true,
            false,
            None,
            &Default::default(),
            &[],
            Vec::new(),
        );
        let [Line::Trace(trace)] = lines.as_slice() else {
            panic!("{lines:?}")
        };
        let [TraceItem::Step(step)] = trace.items.as_slice() else {
            panic!("{:?}", trace.items)
        };
        assert_eq!(
            (step.title.as_ref(), step.description.as_ref(), step.failed),
            ("Couldn't open maat.pt", "not a web page", true)
        );
        let opens = format!("{url}\nAgents may open only http and https pages.");
        assert!(
            matches!(&step.opens, Opens::Text(text) if *text == opens),
            "{:?}",
            step.opens
        );
    }

    /// A step's card says why it failed from its call's kind alone (#3702): each error the app
    /// once sorted by its words, reworded, says the same, and those words with no kind say
    /// nothing.
    #[test]
    fn a_reworded_error_says_the_same_why() {
        use crate::agents::task::{Line, TraceItem, Traced, fold};
        use crate::agents::tools::Kind;
        let why = |error: &str, kind: Option<Kind>| {
            let now = std::time::Instant::now();
            let failed = Traced {
                call: 1,
                did: tools::Did {
                    tool: "click".into(),
                    site: "example.com".into(),
                    ..Default::default()
                },
                after: 0,
                started: now,
                ended: Some(now),
                error: Some(error.into()),
                kind,
                picture: None,
                close: None,
                notes: None,
            };
            let lines = fold(
                &[],
                &[failed],
                true,
                false,
                None,
                &Default::default(),
                &[],
                Vec::new(),
            );
            let [Line::Trace(trace)] = lines.as_slice() else {
                panic!("{lines:?}")
            };
            let [TraceItem::Step(step)] = trace.items.as_slice() else {
                panic!("{:?}", trace.items)
            };
            assert!(step.failed);
            step.description.to_string()
        };
        // What each error said that its word was read from, and its kind. A stopped step has no
        // line to say it in (a_step_under_way_at_stop_leaves_no_line).
        let once = [
            ("The user said no.", Kind::SaidNo),
            ("That's a password field.", Kind::Password),
            ("Tab 30 isn't an http or https page.", Kind::NotWebPage),
            ("There's no element e12 in tab 30.", Kind::NotFound),
            ("The page is still loading.", Kind::StillLoading),
            ("“Basket” didn't show in 5 s.", Kind::DidntShow),
            ("The tab is the user's already.", Kind::Yours),
        ];
        for (words, kind) in once {
            let said = tools::reason(Some(kind));
            assert!(!said.is_empty(), "{kind:?}");
            assert_eq!(why("Something else went on.", Some(kind)), said, "{words}");
            assert_eq!(why(words, None), "", "{words}");
        }
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
            (
                did("scroll", "maat.pt", "“Document”", "down"),
                Tense::Done,
                "Scrolled down in “Document”",
            ),
            (did("wait", "", "2 s", ""), Tense::Done, "Waited 2 s"),
            (
                did("wait", "", "“Tickets”", ""),
                Tense::Failed,
                "Didn't see “Tickets”",
            ),
            (
                did("wait", "", "“Loading” to go", ""),
                Tense::Now,
                "Waiting for “Loading” to go",
            ),
            (
                did("wait", "", "“Loading” to go", ""),
                Tense::Failed,
                "Didn't see “Loading” go",
            ),
            (
                did("wait", "", tools::SETTLE, ""),
                Tense::Done,
                "Waited for the page to settle",
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
            kind: None,
            picture: None,
            close: None,
            notes: None,
        };
        let mut messages = vec![message(Who::User, "Museum hours", 0)];
        let mut trace = vec![step("open_tab", 1, 1, Some(3))];
        messages.push(message(Who::Agent, "Looking at MAAT first.", 3));
        trace.push(step("read_page", 2, 4, None));

        let lines = fold(
            &messages,
            &trace,
            false,
            true,
            None,
            &Default::default(),
            &[],
            Vec::new(),
        );
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
        let lines = fold(
            &messages,
            &trace,
            true,
            false,
            None,
            &Default::default(),
            &[],
            Vec::new(),
        );
        let [Line::You(_), Line::Trace(worked), Line::Said(said)] = lines.as_slice() else {
            panic!("{lines:?}")
        };
        assert!(!worked.running);
        assert_eq!(
            worked.summary(),
            ("Worked for 40 s".into(), "2 steps".into())
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

    /// Worked for counts work alone: a step handed to the user, from its start to Hand Back, isn't,
    /// unless another step of the stretch ran meanwhile.
    #[test]
    fn worked_for_leaves_out_the_time_a_step_waited_on_the_user() {
        use crate::agents::task::{Line, Message, Traced, Who, fold};
        let start = std::time::Instant::now();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let message = |who, text: &str, seconds| Message {
            who,
            text: text.to_string().into(),
            at: at(seconds),
        };
        let step = |tool: &str, started, ended: u64| Traced {
            call: 1,
            did: tools::Did {
                tool: tool.into(),
                site: "maat.pt".into(),
                ..Default::default()
            },
            after: 1,
            started: at(started),
            ended: Some(at(ended)),
            error: None,
            kind: None,
            picture: None,
            close: None,
            notes: None,
        };
        let took = |trace: &[Traced]| {
            let messages = [
                message(Who::User, "Museum hours", 0),
                message(Who::Agent, "MAAT opens at 10:00.", 200),
            ];
            let lines = fold(
                &messages,
                trace,
                true,
                false,
                None,
                &Default::default(),
                &[],
                Vec::new(),
            );
            let [Line::You(_), Line::Trace(worked), Line::Said(_)] = lines.as_slice() else {
                panic!("{lines:?}")
            };
            worked.summary().0
        };
        // 2 s open, 180 s at the user's sign-in, then 6 s more and its answer at 200 s.
        let handed = [
            step("open_tab", 0, 2),
            step("hand_off", 3, 183),
            step("read_page", 184, 190),
        ];
        assert_eq!(took(&handed), "Worked for 20 s");
        // A step running while the handed one waits is work.
        let mut meanwhile = handed.to_vec();
        meanwhile.push(step("read_page", 10, 40));
        assert_eq!(took(&meanwhile), "Worked for 50 s");
    }

    /// A turn the user stopped has no answer: all it said stays in its stretch of work. Only a
    /// turn that ended by itself puts its last words outside, as its answer.
    #[test]
    fn a_stopped_turn_keeps_what_it_said_in_its_stretch() {
        use crate::agents::task::{Line, Message, TraceItem, Traced, Who, fold};
        let start = std::time::Instant::now();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let message = |who, text: &str, seconds| Message {
            who,
            text: text.to_string().into(),
            at: at(seconds),
        };
        let messages = vec![
            message(Who::User, "A table for two", 0),
            message(Who::Agent, "Looking at Tasca first.", 2),
            message(Who::Agent, "Tasca has 8:00 free.", 4),
        ];
        let trace = vec![Traced {
            call: 1,
            did: tools::Did {
                tool: "open_tab".into(),
                site: "tasca.pt".into(),
                ..Default::default()
            },
            after: 1,
            started: at(1),
            ended: Some(at(2)),
            error: None,
            kind: None,
            picture: None,
            close: None,
            notes: None,
        }];
        let fold_with = |stopped: &[usize]| {
            fold(
                &messages,
                &trace,
                true,
                false,
                None,
                &Default::default(),
                stopped,
                Vec::new(),
            )
        };
        let by_itself = fold_with(&[]);
        let [Line::You(_), Line::Trace(_), Line::Said(answer)] = by_itself.as_slice() else {
            panic!("{by_itself:?}")
        };
        assert_eq!(answer.as_ref(), "Tasca has 8:00 free.");
        let stopped = fold_with(&[1]);
        let [Line::You(_), Line::Trace(worked)] = stopped.as_slice() else {
            panic!("a stopped turn showed an answer: {stopped:?}")
        };
        let said: Vec<&str> = (worked.items.iter())
            .filter_map(|item| match item {
                TraceItem::Said(said) => Some(said.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(said, ["Looking at Tasca first.", "Tasca has 8:00 free."]);
    }

    /// A stretch's count is of steps, not of its rows: a row that's the same step on several
    /// tabs counts each.
    #[test]
    fn a_stretch_counts_steps_not_rows() {
        use crate::agents::task::{Opens, Trace, TraceItem, TraceStep};
        let step = |key, opens| TraceStep {
            key,
            glyph: "",
            title: "Read 2 pages".into(),
            description: "".into(),
            running: false,
            failed: false,
            opens,
            under: "".into(),
        };
        let trace = Trace {
            key: 0,
            running: false,
            took: Some(4),
            items: vec![TraceItem::Step(step(
                0,
                Opens::Steps(vec![step(1, Opens::Nothing), step(2, Opens::Nothing)]),
            ))],
        };
        assert_eq!(trace.summary(), ("Worked for 4 s".into(), "2 steps".into()));
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
            kind: None,
            picture: None,
            close: None,
            notes: None,
        }];
        let mut messages = vec![
            message(Who::User, "Museum hours", 0),
            message(Who::Result, "MAAT opens at 10:00.", 5),
        ];
        let before = fold(
            &messages,
            &trace,
            true,
            false,
            None,
            &Default::default(),
            &[],
            Vec::new(),
        );
        messages.push(message(Who::User, "And on Sunday?", 9));
        let lines = fold(
            &messages,
            &trace,
            false,
            true,
            None,
            &Default::default(),
            &[],
            Vec::new(),
        );
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
            kind: None,
            picture: None,
            close: None,
            notes: None,
        };
        let messages = [Message {
            who: Who::User,
            text: "Close cases 1 to 10".into(),
            at: at(0),
        }];
        let lines = |trace: &[Traced]| {
            let lines = fold(
                &messages,
                trace,
                false,
                true,
                None,
                &Default::default(),
                &[],
                Vec::new(),
            );
            let [Line::You(_), Line::Trace(trace)] = lines.as_slice() else {
                panic!("{lines:?}")
            };
            (trace.items.iter())
                .map(|item| match item {
                    TraceItem::Step(step) => step.clone(),
                    TraceItem::Said(said) => panic!("{said}"),
                    TraceItem::Helper { step, .. } => panic!("{}", step.title),
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
                ("Opened 10 pages", "1 s"),
                ("Waited for “Ready” on 10 pages", ""),
                ("Read 10 pages", "1 s"),
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
        (last.ended, last.error, last.kind) = (
            Some(at(7)),
            Some("There's no element e7 on the page.".into()),
            Some(tools::Kind::NotFound),
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
                ("Clicked “Save” on 2 pages", "3 s"),
                ("Waited for “Saved” on 2 pages", ""),
                ("Clicked “Save”", "1 s"),
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

    /// A change of a task's access is ilo's own turn to the CLI it runs: a lowered task between
    /// turns hears it with the user's next, in ilo's fixed words.
    #[gpui::test]
    fn a_change_of_access_is_ilos_turn_to_its_task(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Check my balance", cx);
        // Between turns: its CLI answered, and the user may write again.
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.ended = None;
            task.state = State::Done {
                result: "Done.".into(),
                summary: None,
                tabs: Vec::new(),
            };
        });
        agents.update(cx, |agents, cx| agents.set_tier(&agent, Tier::Look, cx));
        let told = agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().told.clone());
        assert_eq!(
            told.as_deref(),
            Some(
                "The user changed this task's access to Look only. What it doesn't allow will be refused."
            )
        );
        let tier = agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().tier);
        assert_eq!(tier, Tier::Look);
    }

    /// A task starts at the level it's given, the composer's pick, and the log's entry for its
    /// start names it, as a change's entry does. Without a pick (the window's bar and the
    /// composer, which pass the default) it starts at Work on pages.
    #[gpui::test]
    fn a_task_starts_at_the_level_it_was_given_and_the_log_names_it(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let full = agents
            .update(cx, |agents, cx| {
                agents.start_task(Cli::Claude, "Pay the bill", page(), 1, Tier::Full, cx)
            })
            .expect("started");
        let plain = start(&agents, "Check my balance", cx);
        for (agent, tier) in [(&full, Tier::Full), (&plain, Tier::Work)] {
            let at = agents.read_with(cx, |agents, _| agents.task(agent).unwrap().tier);
            assert_eq!(at, tier);
            let logged: Vec<String> = agents.read_with(cx, |agents, _| {
                (agents.activity(agent))
                    .filter(|entry| entry.action.as_ref() == crate::agents::access::ACCESS_ACTION)
                    .map(|entry| entry.target.to_string())
                    .collect()
            });
            assert_eq!(
                logged,
                vec![crate::agents::tier::started_logged(
                    tier,
                    Default::default()
                )]
            );
        }
    }

    /// The conversation's level is the task's own: what it started at, Look only or Full access,
    /// and after a raise from a refusal's button, the raised level, never a default of its own.
    #[gpui::test]
    fn the_conversation_says_the_tasks_own_level(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let level = |agent: &str, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, cx| {
                agents.conversation(agent, cx).unwrap().tier
            })
        };
        let mut started = Vec::new();
        for tier in [Tier::Look, Tier::Full] {
            let agent = agents
                .update(cx, |agents, cx| {
                    agents.start_task(Cli::Claude, "Check my balance", page(), 1, tier, cx)
                })
                .expect("started");
            assert_eq!(level(&agent, cx), tier, "a task started at {tier:?}");
            started.push(agent);
        }

        let agent = started.remove(0);
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.ended = None;
            task.state = State::Working;
        });
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "Shop", "https://shop.example/basket"));
        let tab = json!({ "tab": 30 });
        traced(
            &agents,
            &browser,
            &agent,
            "snapshot",
            tab,
            Some(shop_tree()),
            cx,
        );
        let click = json!({ "tab": 30, "ref": "e1" });
        assert!(traced(&agents, &browser, &agent, "click", click, None, cx).error);
        assert_eq!(
            level(&agent, cx),
            Tier::Look,
            "the refusal changed the level"
        );
        let raise = agents.read_with(cx, |agents, cx| {
            (agents.conversation(&agent, cx).unwrap().lines.iter())
                .filter_map(|line| match line {
                    crate::agents::task::Line::Ilo(line) => Some(line.clone()),
                    _ => None,
                })
                .flat_map(|line| crate::agents::task::ilo::buttons(&line))
                .find_map(|button| match button {
                    crate::agents::task::ilo::IloButton::Raise(tier) => Some(tier),
                    _ => None,
                })
        });
        let raise = raise.expect("the refusal's line has no raise");
        agents.update(cx, |agents, _| {
            for stop in &mut agents.task_mut(&agent).unwrap().stops {
                stop.appeared -= super::tier::PROTECT;
            }
        });
        agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::SetTier(raise), cx);
        });
        assert_eq!(
            level(&agent, cx),
            raise,
            "the raise didn't reach the conversation"
        );
    }

    /// A task that waits on the user is at its own access still. Asking, At limit and Interrupted
    /// are states of a task ilo runs alone, and such a task is found by its own name in every
    /// state, so a call on any connection of its assistant's, a helper's too, is judged as the
    /// task's: at Look only with its question up, a click on its page from a second connection
    /// is refused as Look only refuses it, the line is the task's and its question waits on. At
    /// its limit, the same.
    #[gpui::test]
    fn a_task_that_waits_is_at_its_own_access_whichever_connection_acts(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = agents
            .update(cx, |agents, cx| {
                agents.start_task(Cli::Claude, "Check my basket", page(), 1, Tier::Look, cx)
            })
            .expect("started");
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.ended = None;
            task.state = State::Working;
        });
        (browser.tabs.borrow_mut()).push((30, 100, "Shop", "https://shop.example/basket"));
        let tab = json!({ "tab": 30 });
        let seen = Some(shop_tree());
        traced(&agents, &browser, &agent, "snapshot", tab, seen, cx);
        let asking = json!({ "question": "Which day?", "answers": ["Friday", "Saturday"] });
        let asked = call(&agents, &agent, "ask", asking, cx);
        let state = |cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().state.clone())
        };
        assert!(matches!(state(cx), State::Asking { .. }));

        let pages = browser.pages.borrow().len();
        let click = |id, cx: &mut TestAppContext| {
            let (reply, answer) = mpsc::channel();
            let call = Call {
                id,
                agent: agent.clone(),
                connection: 2,
                tool: "click".into(),
                arguments: json!({ "tab": 30, "ref": "e1" }),
                reply,
                at: std::time::Instant::now(),
            };
            agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
            answer.try_recv().expect("the click wasn't refused at once")
        };
        let refused = click(9_001, cx);
        assert!(refused.error, "it clicked: {}", refused.text);
        assert!(refused.text.contains("(Look only)"), "{}", refused.text);
        assert!(matches!(state(cx), State::Asking { .. }));
        assert!(
            asked.try_recv().is_err(),
            "the refusal answered the question"
        );

        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Limit { resets: None };
        });
        let refused = click(9_002, cx);
        assert!(refused.error, "it clicked: {}", refused.text);
        assert!(refused.text.contains("(Look only)"), "{}", refused.text);

        assert_eq!(
            browser.pages.borrow().len(),
            pages,
            "a click reached the page"
        );
        let stops = agents.read_with(cx, |agents, _| agents.stops(&agent));
        assert_eq!(stops.len(), 1, "{stops:?}");
        assert_eq!(
            (stops[0].why, stops[0].tier, stops[0].count),
            (Why::Act, Tier::Look, 2)
        );
    }

    /// A task ilo runs that the user stops has the line that says so, as every stopped task has,
    /// with the pages it held then and nothing to press. Its Stop and its reply are as they were:
    /// its call that waited is answered as stopped, and the user's reply comes under the line,
    /// which stays where it came.
    #[gpui::test]
    fn a_task_ilo_runs_says_the_user_stopped_it_with_nothing_to_press(cx: &mut TestAppContext) {
        use crate::agents::task::Line;
        use crate::agents::task::ilo::{self, IloKind};
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Check the basket", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let space = agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().space);
        (browser.tabs.borrow_mut()).push((30, space, "Shop", "https://shop.example/basket"));
        (browser.tabs.borrow_mut()).push((31, space, "Help", "https://shop.example/help"));
        let reading = call(&agents, &agent, "read_page", json!({ "tab": 30 }), cx);
        agents.update(cx, |agents, cx| agents.stop_task(&agent, cx));
        assert_eq!(
            reading.recv().unwrap(),
            ToolResult::failed(tools::Kind::Stopped, STOPPED)
        );
        let stopped = |cx: &mut TestAppContext| {
            let talk = agents
                .read_with(cx, |agents, cx| agents.conversation(&agent, cx))
                .expect("its conversation");
            (talk.lines.into_iter())
                .filter_map(|line| match line {
                    Line::Ilo(line) if matches!(line.kind, IloKind::YouStopped { .. }) => {
                        Some(line)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let lines = stopped(cx);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].kind, IloKind::YouStopped { pages: 2 });
        assert!(ilo::buttons(&lines[0]).is_empty());
        // Nor is there anything to hand back: an outside agent's line alone has that.
        agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::PagesBack, cx);
            assert!(
                !agents
                    .log
                    .iter()
                    .any(|entry| entry.action == tools::PAGES_BACK)
            );
        });
        assert_eq!(stopped(cx), lines);

        // Their reply comes under the line, which stays where it came.
        agents.update(cx, |agents, cx| agents.reply(&agent, "Go on", cx));
        let talk = agents
            .read_with(cx, |agents, cx| agents.conversation(&agent, cx))
            .expect("its conversation");
        let at = |found: &dyn Fn(&Line) -> bool| talk.lines.iter().position(found);
        let line = at(&|line| matches!(line, Line::Ilo(line) if *line == lines[0]));
        let reply = at(&|line| matches!(line, Line::You(said) if said.as_ref() == "Go on"));
        assert!(line.is_some() && line < reply, "{:?}", talk.lines);
        assert_eq!(stopped(cx), lines, "the line of its stop changed");
    }

    /// A task ilo runs is untouched by a level an outside client's task was left at, and leaves
    /// none for one: its level is the one its start gave it, from the composer's menu. With Claude
    /// Code's session ended at Look only, a task started at Full access is at Full access, and with
    /// a task ilo runs at Look only, Claude Code's next session starts where every task does.
    #[gpui::test]
    fn a_task_ilo_runs_takes_and_leaves_no_carried_level(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
        cx.run_until_parked();
        let outside = |connection, cx: &mut TestAppContext| {
            let hello = Event::Hello {
                agent: "claude-code".into(),
                connection,
                client: "claude-code".into(),
            };
            let (reply, _answer) = mpsc::channel();
            let call = Call {
                id: 9_000 + connection,
                agent: "claude-code".into(),
                connection,
                tool: "read_page".into(),
                arguments: json!({ "tab": 20 }),
                reply,
                at: std::time::Instant::now(),
            };
            agents.update(cx, |agents, cx| {
                agents.handle(hello, cx);
                agents.handle(Event::Call(call), cx);
                let session = agents.tasks.tasks.last().expect("its session");
                assert!(session.session.is_some());
                session.agent.clone()
            })
        };
        let tier = |key: &str, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.task(key).expect("its task").tier)
        };
        let first = outside(1, cx);
        agents.update(cx, |agents, cx| {
            agents.set_tier(&first, Tier::Look, cx);
            let closed = Event::Closed {
                agent: "claude-code".into(),
                connection: 1,
            };
            agents.handle(closed, cx);
        });
        for (asked, at) in [
            ("Check my basket", Tier::Full),
            ("Read the news", Tier::Look),
        ] {
            let task = agents
                .update(cx, |agents, cx| {
                    agents.start_task(Cli::Claude, asked, page(), 1, at, cx)
                })
                .expect("started");
            assert_eq!(tier(&task, cx), at);
        }
        // The user picks on the ended session, which passes nothing of its own: only it holds
        // the next, not the task ilo runs at Look only.
        agents.update(cx, |agents, cx| agents.set_tier(&first, Tier::Work, cx));
        let second = outside(2, cx);
        assert_ne!(first, second);
        assert_eq!(tier(&second, cx), Tier::Work);
    }

    /// Pages the user added reach the task as their addresses, http(s) only, and the start's entry
    /// gains their count, never an address. They open as its own tabs and keep it to nothing: it
    /// goes to another site at its level, and a site on the Off limits list is still refused.
    #[gpui::test]
    fn added_pages_keep_the_task_to_no_site_but_off_limits_still_refuses(cx: &mut TestAppContext) {
        use crate::agents::task::Start;
        let (agents, browser) = ready(None, cx);
        agents.update(cx, |agents, cx| {
            assert!(agents.fence("https://www.mybank.example/", cx).is_some());
        });
        let start = Start {
            pages: vec![
                "https://fieldbook.example/lakeside".into(),
                "https://www.atlas.example/parks".into(),
                "file:///etc/passwd".into(),
            ],
            ..Start::default()
        };
        let agent = agents.update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, "Plan a walk", None, 1, start, cx)
        });
        let agent = agent.expect("started");
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.ended = None;
            task.state = State::Working;
        });
        agents.read_with(cx, |agents, _| {
            assert_eq!(
                agents.task(&agent).unwrap().added.len(),
                2,
                "no page off the web is added"
            );
            let logged: Vec<String> = (agents.activity(&agent))
                .filter(|entry| entry.action.as_ref() == crate::agents::access::ACCESS_ACTION)
                .map(|entry| entry.target.to_string())
                .collect();
            assert_eq!(logged, ["Started at Work on pages · 2 pages"]);
        });

        let tabs = browser.tabs.borrow().len();
        let other = json!({ "url": "https://www.trailmap.example/" });
        let opening = call(&agents, &agent, "open_tab", other, cx);
        assert!(
            opening.try_recv().is_err(),
            "an open of another site was answered at once"
        );
        assert_eq!(
            browser.tabs.borrow().len(),
            tabs + 1,
            "another site didn't open"
        );
        let stops =
            |cx: &mut TestAppContext| agents.read_with(cx, |agents, _| agents.stops(&agent));
        assert!(stops(cx).is_empty(), "another site was refused");

        let fenced = json!({ "url": "https://www.mybank.example/" });
        let refused = call(&agents, &agent, "open_tab", fenced, cx)
            .recv()
            .unwrap();
        assert!(refused.error, "{}", refused.text);
        assert_eq!(
            browser.tabs.borrow().len(),
            tabs + 1,
            "the fenced site opened"
        );
        let whys: Vec<Why> = stops(cx).iter().map(|stop| stop.why).collect();
        assert_eq!(whys, [Why::Fenced]);
    }

    /// The pages the user added reach the task each on a line of its own under the sentence, so an
    /// address's comma or bracket is never read as the sentence's.
    #[test]
    fn each_added_address_is_a_line_of_its_own() {
        use crate::agents::task::{added_address, with_added};
        let pages: Vec<String> = ["https://a.example/x,y", "https://b.example/(z).)"]
            .into_iter()
            .map(|url| added_address(url).expect("an address the task may get"))
            .collect();
        let first = with_added("Plan a walk".into(), &pages);
        assert_eq!(
            first,
            "Plan a walk\n\nPages I added, to open in tabs of your own:\n\
             https://a.example/x,y\nhttps://b.example/(z).)"
        );
        for page in &pages {
            assert!(
                first.lines().any(|line| line == page),
                "{page} on a line of its own"
            );
        }
        assert_eq!(with_added("Plan a walk".into(), &[]), "Plan a walk");
    }

    /// An added page's address reaches the task without the user name and password a tab's
    /// address can carry, and one over the cap doesn't reach it at all.
    #[gpui::test]
    fn an_added_address_carries_no_login_and_none_over_the_cap(cx: &mut TestAppContext) {
        use crate::agents::task::{ADDED_MOST, Start, added_address};
        let at_most = format!("https://a.example/{}", "a".repeat(ADDED_MOST - 18));
        assert_eq!(at_most.len(), ADDED_MOST);
        assert_eq!(added_address(&at_most).as_deref(), Some(at_most.as_str()));
        assert_eq!(added_address(&format!("{at_most}a")), None);
        assert_eq!(
            added_address("https://sam@a.example/x").as_deref(),
            Some("https://a.example/x")
        );
        let (agents, _) = ready(None, cx);
        let start = Start {
            pages: vec![
                "https://sam:hunter2@fieldbook.example/lakeside?day=2".into(),
                format!("{at_most}a"),
            ],
            ..Start::default()
        };
        let agent = agents.update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, "Plan a walk", None, 1, start, cx)
        });
        agents.read_with(cx, |agents, _| {
            let task = agents.task(&agent.expect("started")).unwrap();
            assert_eq!(
                task.added,
                ["https://fieldbook.example/lakeside?day=2"],
                "the login is the user's, and a page over the cap isn't added"
            );
        });
    }

    /// A card lists the task's last three steps this turn, oldest first, as the conversation's
    /// trace lines say them, and pictures its last page; before its first step it has neither.
    #[gpui::test]
    fn a_card_lists_the_last_three_steps_and_pictures_its_last_page(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Find two tickets", cx);
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.ended = None;
            task.state = State::Working;
        });
        let card = card_of(&agents, cx);
        assert!(card.steps.is_empty(), "{:?}", card.steps);
        assert_eq!(card.last_tab, None);
        for site in ["a.example", "b.example", "c.example", "d.example"] {
            let _ = call(&agents, &agent, "open_tab", json!({ "url": site }), cx);
        }
        let card = card_of(&agents, cx);
        assert_eq!(
            card.steps,
            // Each is still under way, as the test's pages never finish loading.
            [
                "Opening b.example",
                "Opening c.example",
                "Opening d.example"
            ]
            .map(SharedString::from)
            .to_vec()
        );
        assert!(
            card.last_tab.is_some(),
            "no page pictured of {:?}",
            card.pages
        );
        assert_eq!(card.last_tab, card.pages.last().copied());
    }

    /// Latest tasks' line under a card's title once its task is over: a done task's answer's first
    /// line, and a stopped one's last finished step, never one under way at Stop nor one that
    /// failed, or nothing.
    #[gpui::test]
    fn a_cards_line_is_the_answers_first_or_the_last_finished_step(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Find two tickets", cx);
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.ended = None;
            task.state = State::Working;
        });
        assert_eq!(card_of(&agents, cx).line, None, "a line while it works");
        for site in ["a.example", "b.example", "c.example"] {
            let _ = call(&agents, &agent, "open_tab", json!({ "url": site }), cx);
        }
        let stopped = |agents: &Entity<Agents>, cx: &mut TestAppContext| {
            agents.update(cx, |agents, _| {
                agents.task_mut(&agent).unwrap().state = State::Stopped;
            });
            card_of(agents, cx).line
        };
        // All three under way at Stop: none is a step it finished.
        assert_eq!(stopped(&agents, cx), None);
        agents.update(cx, |agents, _| {
            let trace = &mut agents.task_mut(&agent).unwrap().trace;
            let now = std::time::Instant::now();
            trace[0].ended = Some(now);
            trace[1].ended = Some(now);
            trace[1].error = Some("Not found".into());
        });
        // The third was under way at Stop and the second failed, so the first is the last finished.
        assert_eq!(stopped(&agents, cx).as_deref(), Some("Opened a.example"));
        // The third answered as stopped, as Stop answers what's under way: still not a step it
        // finished (a_step_under_way_at_stop_leaves_no_line).
        agents.update(cx, |agents, _| {
            let third = &mut agents.task_mut(&agent).unwrap().trace[2];
            third.ended = Some(std::time::Instant::now());
            third.error = Some(STOPPED.into());
            third.kind = Some(tools::Kind::Stopped);
        });
        assert_eq!(stopped(&agents, cx).as_deref(), Some("Opened a.example"));

        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Done {
                result: "## **Two seats** on Friday\nRow 12.".into(),
                summary: Some("Found two seats".into()),
                tabs: Vec::new(),
            };
        });
        let card = card_of(&agents, cx);
        assert_eq!(card.line.as_deref(), Some("Two seats on Friday"));
        assert_eq!(card.ended_at, None, "a task of this run says when it ended");
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Done {
                result: "".into(),
                summary: None,
                tabs: Vec::new(),
            };
        });
        let card = card_of(&agents, cx);
        assert_eq!(card.line, None, "a line would say Done twice");
        assert_eq!(card.status.as_ref(), card.word.after_name());
    }

    /// A done card's line is cleaned as a title is: a link shows its label alone, what wouldn't
    /// show or would turn the text around is written out, and past TITLE_KEPT it's clipped, so an
    /// answer can't lay out the card's line as it likes.
    #[gpui::test]
    fn a_done_cards_line_is_plain_and_capped_as_a_title_is(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Find two tickets", cx);
        let answer = |result: &str, cx: &mut TestAppContext| {
            agents.update(cx, |agents, _| {
                agents.task_mut(&agent).unwrap().state = State::Done {
                    result: result.into(),
                    summary: None,
                    tabs: Vec::new(),
                };
            });
            card_of(&agents, cx).line.map(String::from)
        };
        assert_eq!(
            answer(
                "**Booked** [two seats](https://a.example/b) \u{202E}\u{7}",
                cx
            )
            .as_deref(),
            Some("Booked two seats \\u{202E}\\u{0007}")
        );
        let long = format!("[Seats]({}) {}", "https://a.example/", "x".repeat(60));
        let kept = format!("Seats {}…", "x".repeat(TITLE_KEPT - 6));
        assert_eq!(answer(&long, cx), Some(kept));
    }

    /// A Stopped line's step up reaches the task through the user's press, as Stop does.
    #[gpui::test]
    fn the_lines_buttons_change_the_tasks_access(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Check my balance", cx);
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.ended = None;
            task.state = State::Working;
        });
        // The step up tells the task, which here has no CLI to run it again.
        agents.update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::SetTier(Tier::Full), cx);
        });
        let tier = agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().tier);
        assert_eq!(tier, Tier::Full);
    }

    #[gpui::test]
    fn stop_ends_it_and_refuses_what_waits(cx: &mut TestAppContext) {
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Check my balance", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        // A refusal left unsettled, which the stop settles.
        let bank = json!({ "url": "https://www.chase.com/" });
        assert!(
            call(&agents, &agent, "open_tab", bank, cx)
                .recv()
                .unwrap()
                .error
        );
        let opening = call(
            &agents,
            &agent,
            "open_tab",
            json!({ "url": "https://example.org/" }),
            cx,
        );
        agents.update(cx, |agents, cx| agents.stop_task(&agent, cx));
        assert_eq!(
            opening.recv().unwrap(),
            ToolResult::failed(tools::Kind::Stopped, STOPPED)
        );
        let card = card_of(&agents, cx);
        // It says how far it got: the refused open and the open_tab the stop answered.
        assert_eq!(
            (card.status.as_ref(), card.question),
            ("Stopped after 2 steps", None)
        );
        let left = agents.read_with(cx, |agents, _| agents.stops(&agent));
        assert!(
            left.is_empty(),
            "the stop left a refusal unsettled: {left:?}"
        );
        // Its turn ending after the stop leaves it stopped.
        said(&agents, &agent, Out::Ended(None), cx);
        assert_eq!(
            card_of(&agents, cx).status.as_ref(),
            "Stopped after 2 steps"
        );
    }

    /// A step under way when the user presses Stop isn't a step that failed: once its answer comes
    /// back stopped it leaves no line, in the trace or on the card, and no count has it. The step
    /// it finished before stays, the stopped card's last.
    #[gpui::test]
    fn a_step_under_way_at_stop_leaves_no_line(cx: &mut TestAppContext) {
        use crate::agents::task::{Line, TraceItem, Word};
        let (agents, _) = ready(None, cx);
        let agent = start(&agents, "Compare three shops", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let answered = |kind: Option<tools::Kind>, cx: &mut TestAppContext| {
            let error = kind.map(|_| STOPPED.to_string());
            let event = Event::Answered {
                agent: agent.clone(),
                id: 0,
                at: std::time::Instant::now(),
                error,
                kind,
                image: None,
                notes: None,
            };
            agents.update(cx, |agents, cx| agents.handle(event, cx));
        };
        let shop = |url: &str| json!({ "url": url });
        let _opened = call(
            &agents,
            &agent,
            "open_tab",
            shop("https://example.org/"),
            cx,
        );
        answered(None, cx);
        let opening = call(
            &agents,
            &agent,
            "open_tab",
            shop("https://example.net/"),
            cx,
        );
        agents.update(cx, |agents, cx| agents.stop_task(&agent, cx));
        assert_eq!(
            opening.recv().unwrap(),
            ToolResult::failed(tools::Kind::Stopped, STOPPED)
        );
        answered(Some(tools::Kind::Stopped), cx);

        let card = card_of(&agents, cx);
        assert_eq!(card.word, Word::Stopped);
        assert_eq!(card.status.as_ref(), "Stopped after 1 step");
        assert_eq!(card.steps.len(), 1, "the card's steps: {:?}", card.steps);
        let lines = agents.read_with(cx, |agents, cx| {
            agents.conversation(&agent, cx).unwrap().lines
        });
        let titles: Vec<String> = (lines.iter())
            .filter_map(|line| match line {
                Line::Trace(trace) => Some(trace),
                _ => None,
            })
            .flat_map(|trace| trace.items.iter())
            .filter_map(|item| match item {
                TraceItem::Step(step) => Some(step.title.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(titles.len(), 1, "the trace's steps: {titles:?}");
        assert_eq!(
            card.steps[0].as_ref(),
            titles[0],
            "the card's last step is the trace's"
        );
        assert!(
            !titles[0].starts_with("Couldn't"),
            "the stopped step was written as failed: {titles:?}"
        );
    }

    /// After Stop no call of the task's is handled, one that waited its turn behind a slow page
    /// included: each kind that can wait is answered as stopped, and nothing of it is run, shown
    /// or kept. Once the task is at work again its calls go on (#4209).
    #[gpui::test]
    fn a_stopped_tasks_calls_are_answered_as_stopped(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        let agent = start(&agents, "Compare three shops", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        // A page that is slow to open holds its call, and in the server the calls behind it.
        let shop = json!({ "url": "https://example.org/" });
        let opening = call(&agents, &agent, "open_tab", shop.clone(), cx);
        agents.update(cx, |agents, cx| agents.stop_task(&agent, cx));
        let stopped = ToolResult::failed(tools::Kind::Stopped, STOPPED);
        assert_eq!(opening.recv().unwrap(), stopped);

        let seen = |cx: &mut TestAppContext| {
            let card = card_of(&agents, cx);
            let asked = (browser.made.borrow().len(), browser.pages.borrow().len());
            let read = browser.reads.borrow().len();
            let card = (card.title, card.status, card.question.is_some());
            (card, log(&agents, &agent, cx), asked, read)
        };
        let before = seen(cx);
        for (tool, arguments) in [
            ("title", json!({ "text": "Probe" })),
            ("ask", json!({ "question": "Can you see this?" })),
            (
                "finish",
                json!({ "result": "Compared them.", "summary": "Compared" }),
            ),
            ("open_tab", shop),
            // Calls without a tab, which wait for every tab's calls before them.
            ("snapshot", json!({})),
            ("list_tabs", json!({})),
        ] {
            let answer = call(&agents, &agent, tool, arguments, cx);
            assert_eq!(answer.recv().unwrap(), stopped, "{tool}");
        }
        assert_eq!(seen(cx), before);
        let state = agents.read_with(cx, |agents, _| agents.task(&agent).unwrap().state.clone());
        assert_eq!(state, State::Stopped);

        // At work again, as the user's next message sets it, it names itself.
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        let named = call(
            &agents,
            &agent,
            "title",
            json!({ "text": "Three shops" }),
            cx,
        );
        assert!(!named.recv().unwrap().error);
        assert_eq!(card_of(&agents, cx).title.as_ref(), "Three shops");
    }

    #[gpui::test]
    fn a_wall_that_does_not_go_up_fails_a_task_but_not_one_the_user_stopped(
        cx: &mut TestAppContext,
    ) {
        let (agents, _) = ready(None, cx);
        let refused = || {
            let ended = "the launcher ended before the program started";
            Out::Walled(Err(crate::agents::wall::WallRefused::Failed(ended.into())))
        };
        let state = |agents: &Entity<Agents>, agent: &str, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.task(agent).unwrap().state.clone())
        };
        let agent = start(&agents, "Tidy my notes", cx);
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Working
        });
        said(&agents, &agent, refused(), cx);
        assert!(matches!(state(&agents, &agent, cx), State::Failed { .. }));
        // Stop while the wall goes up ends the launcher, which then says nothing started.
        let agent = start(&agents, "Tidy my letters", cx);
        agents.update(cx, |agents, cx| {
            agents.task_mut(&agent).unwrap().state = State::Working;
            agents.stop_task(&agent, cx);
        });
        said(&agents, &agent, refused(), cx);
        assert_eq!(state(&agents, &agent, cx), State::Stopped);
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

    /// Each state has one word, which a task's card, its conversation's line and its Archive row all
    /// say: one whose assistant's program quit or broke is Interrupted, never Failed or Stopped;
    /// one waiting on the user Needs you; one at its assistant's limit says so, with the
    /// assistant's name when the word stands alone and never twice. The conversation's line says
    /// when the limit resets if it knows.
    #[gpui::test]
    fn every_state_has_one_word_and_every_place_says_it(cx: &mut TestAppContext) {
        use crate::agents::task::Word;
        let (agents, _) = ready(None, cx);
        let done = State::Done {
            result: "Did it.".into(),
            summary: None,
            tabs: Vec::new(),
        };
        let asking = State::Asking {
            question: "Which day?".into(),
            answers: Vec::new(),
        };
        let failed = State::Failed {
            why: "Claude quit.".into(),
            assistant: false,
        };
        let states = [
            (State::Working, Word::Working, "Working", "Working"),
            (asking, Word::NeedsYou, "Needs you", "Needs you"),
            (done, Word::Done, "Done", "Done"),
            (State::Stopped, Word::Stopped, "Stopped", "Stopped"),
            (failed, Word::Interrupted, "Interrupted", "Interrupted"),
            (
                State::Limit { resets: None },
                Word::AtLimit,
                "At limit",
                "Claude at limit",
            ),
        ];
        for (state, word, after_name, alone) in states {
            assert_eq!(Word::of(&state, false), word, "{state:?}");
            assert_eq!(
                (word.after_name(), word.alone("Claude").as_ref()),
                (after_name, alone)
            );
            let agent = start(&agents, "Find two seats", cx);
            let set = |agents: &Entity<Agents>, cx: &mut TestAppContext| {
                agents.update(cx, |agents, _| {
                    let task = agents.task_mut(&agent).unwrap();
                    (task.state, task.ended) = (state.clone(), None);
                })
            };
            set(&agents, cx);
            let card = agents.read_with(cx, |agents, cx| {
                let mut cards = agents.cards(1, cx).into_iter();
                cards.find(|card| card.agent.as_ref() == agent).unwrap()
            });
            assert_eq!(card.word, word, "the card of {state:?}");
            let about = agents.read_with(cx, |agents, cx| {
                agents.conversation(&agent, cx).unwrap().about.to_string()
            });
            assert_eq!(about, format!("Claude · {after_name}"), "{state:?}");
            // Dismissing stops a task under way, so its row is read with the state put back.
            agents.update(cx, |agents, cx| agents.dismiss_task(&agent, cx));
            set(&agents, cx);
            let row = agents.read_with(cx, |agents, cx| {
                let mut rows = agents.archived_tasks(cx).into_iter();
                rows.find(|row| row.agent.as_ref() == agent).unwrap()
            });
            // The row has when it left the foot to go on with.
            assert_eq!(row.about, format!("Claude · {after_name}"), "{state:?}");
            assert!(row.ended > 0., "{state:?}");
        }
        assert_eq!(Word::of(&State::Working, true), Word::NeedsYou);

        // With its reset time, the line says when, and the card's own status keeps the longer line.
        let agent = start(&agents, "Find two seats", cx);
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.state = State::Limit {
                resets: Some("18:00".into()),
            };
            task.ended = None;
        });
        let about = agents.read_with(cx, |agents, cx| {
            agents.conversation(&agent, cx).unwrap().about.to_string()
        });
        assert_eq!(about, "Claude · Limit resets at 18:00");
        agents.update(cx, |agents, _| {
            agents.task_mut(&agent).unwrap().state = State::Limit { resets: None };
        });
        let card = agents.read_with(cx, |agents, cx| {
            let mut cards = agents.cards(1, cx).into_iter();
            cards.find(|card| card.agent.as_ref() == agent).unwrap()
        });
        assert_eq!(card.status.as_ref(), "Claude hit its usage limit");
    }

    /// A task that ended today says its time, and one from an earlier day its day, as a time
    /// alone reads as today's.
    #[test]
    fn a_task_from_an_earlier_day_says_its_day() {
        use chrono::{TimeZone, Utc};
        let ended = Utc.with_ymd_and_hms(2026, 11, 21, 14, 5, 0).unwrap();
        let that_night = Utc.with_ymd_and_hms(2026, 11, 21, 23, 59, 0).unwrap();
        let next_day = Utc.with_ymd_and_hms(2026, 11, 22, 0, 1, 0).unwrap();
        assert_eq!(
            crate::agents::time_or_day_on(ended, that_night, false),
            "14:05"
        );
        assert_eq!(
            crate::agents::time_or_day_on(ended, next_day, false),
            "21 Nov"
        );
        // On the system's 12-hour clock it reads as History's page rows do.
        assert_eq!(
            crate::agents::time_or_day_on(ended, that_night, true),
            "2:05 PM"
        );
        assert_eq!(
            crate::agents::time_or_day_on(ended, next_day, true),
            "21 Nov"
        );
    }

    /// The Archive lists the dismissed tasks, the last to leave the foot first, each with how it
    /// ended and when, which History says as it says its pages' times. Delete from Archive takes one out of the list and out of ⇧⌘T's reach, and
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
                        .map(|task| (task.agent.to_string(), task.about_at("then"), task.dot))
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
        assert_eq!(
            now[1],
            (first.clone(), "Claude · Done then".to_string(), Dot::Done)
        );
        // Dismissed while it worked, it stopped then.
        assert_eq!(now[0].1, "Claude · Stopped then");
        assert_eq!(now[0].2, Dot::Ended);
        // The row has its end for History to list it by and say, however long after it left the
        // foot; one that had none has when it left.
        let rows = agents.read_with(cx, |agents, cx| agents.archived_tasks(cx));
        assert_eq!(rows[1].ended, 1_700_000_000_000.);
        assert!(rows[0].ended > rows[1].ended);

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

    /// At launch the Spaces a crash left go, each with its pages: a task's and an outside agent's
    /// were never the user's, so a crash leaves no agent's page among the user's tabs (#4105).
    #[gpui::test]
    fn the_spaces_a_run_before_left_go_at_launch(cx: &mut TestAppContext) {
        let (agents, browser) = ready(None, cx);
        browser.task_spaces.borrow_mut().extend([100, 101]);
        browser
            .tabs
            .borrow_mut()
            .push((30, 100, "https://flights.example/", "Flights"));
        agents.update(cx, |agents, cx| agents.end_old_tasks(cx));
        assert_eq!(*browser.deleted.borrow(), vec![100, 101, 2]);
        let spaces: Vec<_> = (browser.tabs.borrow().iter())
            .map(|each| (each.0, each.1))
            .collect();
        // The agent's tab 20 and the task's tab 30 went with their Spaces: the user's own is all
        // that's left.
        assert_eq!(spaces, [(10, 1)]);
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
            agents.set_threads(false, cx);
        });
        assert!(
            agents
                .read_with(cx, |agents, _| agents.ask_agents())
                .is_empty(),
            "off lists none"
        );
    }

    /// The Model menu as the user sees it.
    fn model_menu(agents: &Entity<Agents>, cli: Cli, cx: &mut TestAppContext) -> String {
        agents.read_with(cx, |agents, _| {
            crate::menu::described(&agents.task_model_menu(cli).1)
        })
    }

    #[gpui::test]
    fn claudes_models_come_from_claude_and_one_it_drops_runs_on_its_default(
        cx: &mut TestAppContext,
    ) {
        use crate::agents::ready::Models;
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let listing = |models: Option<Models>| Found {
            ready: Ready::SignedIn,
            program: Some("/usr/local/bin/claude".into()),
            models,
            ..Default::default()
        };
        let listed = Models {
            listed: [("opus", "Opus"), ("fable", "Fable"), ("sonnet", "Sonnet")]
                .map(|(id, name)| (id.to_string(), name.to_string()))
                .into(),
            default: Some("Opus".into()),
        };
        let agents = agents(&browser, &configs, None, cx);
        agents.update(cx, |agents, cx| {
            agents.set_threads(true, cx);
            agents.set_task_model(Cli::Claude, "haiku", cx);
        });
        // Till Claude lists its own, ilo's names for them, its default naming none.
        assert_eq!(
            model_menu(&agents, Cli::Claude, cx),
            "Claude Code's default, Opus, Sonnet, Haiku (checked)"
        );

        *configs.looked.lock().unwrap() = vec![(Cli::Claude, listing(Some(listed)))];
        agents.update(cx, |agents, cx| agents.look_for_task_agents(cx));
        cx.run_until_parked();
        // Its names in its order, its default saying which it is; Haiku is gone, so that's chosen,
        // for the run too, and comes back if Claude lists it again.
        assert_eq!(
            model_menu(&agents, Cli::Claude, cx),
            "Claude Code's default [Opus] (checked), Opus, Fable, Sonnet"
        );
        agents.read_with(cx, |agents, _| {
            assert_eq!(agents.task_model(Cli::Claude), "");
            assert_eq!(agents.task_model_name(Cli::Claude), None);
        });

        // Claude not answering the next time keeps the list it gave.
        *configs.looked.lock().unwrap() = vec![(Cli::Claude, listing(None))];
        agents.update(cx, |agents, cx| {
            agents.look_for_task_agents(cx);
            agents.set_task_model(Cli::Claude, "fable", cx);
        });
        cx.run_until_parked();
        assert_eq!(
            model_menu(&agents, Cli::Claude, cx),
            "Claude Code's default [Opus], Opus, Fable (checked), Sonnet"
        );
        agents.read_with(cx, |agents, _| {
            assert_eq!(agents.task_model(Cli::Claude), "fable");
            assert_eq!(
                agents.task_model_name(Cli::Claude).as_deref(),
                Some("Fable")
            );
        });

        // An empty list: its default alone.
        *configs.looked.lock().unwrap() = vec![(Cli::Claude, listing(Some(Models::default())))];
        agents.update(cx, |agents, cx| agents.look_for_task_agents(cx));
        cx.run_until_parked();
        assert_eq!(
            model_menu(&agents, Cli::Claude, cx),
            "Claude Code's default (checked)"
        );
    }

    /// The model a task runs with (`task_model`, what its launch passes) is one the Model menu
    /// lists: anything else, typed or from a page, an agent or a file, is never set.
    #[gpui::test]
    fn a_model_is_set_only_from_the_list_its_menu_was_built_from(cx: &mut TestAppContext) {
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, None, cx);
        agents.update(cx, |agents, cx| {
            agents.set_threads(true, cx);
            agents.set_task_model(Cli::Claude, "sonnet", cx);
            for unlisted in ["Sonnet", "opus --verbose", "claude-sonnet", " sonnet"] {
                agents.set_task_model(Cli::Claude, unlisted, cx);
            }
            // Codex lists none till it's asked, so it runs on its own default.
            agents.set_task_model(Cli::Codex, "sonnet", cx);
        });
        agents.read_with(cx, |agents, _| {
            assert_eq!(agents.task_model(Cli::Claude), "sonnet");
            assert_eq!(agents.task_model(Cli::Codex), "");
        });
        assert_eq!(
            model_menu(&agents, Cli::Claude, cx),
            "Claude Code's default, Opus, Sonnet (checked), Haiku"
        );
        // Its default passes no model.
        agents.update(cx, |agents, cx| agents.set_task_model(Cli::Claude, "", cx));
        agents.read_with(cx, |agents, _| {
            assert_eq!(agents.task_model(Cli::Claude), "")
        });
    }

    /// The composer's effort pick is the task's for each of its runs, only as a level its own
    /// program lists: none for one it doesn't, nor for Codex, whose program takes none.
    #[gpui::test]
    fn a_task_keeps_only_an_effort_its_program_lists(cx: &mut TestAppContext) {
        use crate::agents::task::Start;
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, None, cx);
        let start = |effort: Option<&str>| Start {
            effort: effort.map(str::to_string),
            ..Start::default()
        };
        agents.update(cx, |agents, cx| agents.set_threads(true, cx));
        let started = [
            (Cli::Claude, Some("max")),
            (Cli::Claude, None),
            (Cli::Claude, Some("turbo")),
            (Cli::Codex, Some("high")),
        ]
        .map(|(cli, effort)| {
            let agent = agents.update(cx, |agents, cx| {
                agents.start_task(cli, "Effort", None, 1, start(effort), cx)
            });
            agent.expect("a task started")
        });
        agents.read_with(cx, |agents, _| {
            let kept = started
                .clone()
                .map(|agent| agents.task(&agent).unwrap().effort.clone());
            assert_eq!(kept, [Some("max".to_string()), None, None, None]);
        });
    }

    /// The composer's pick on the assistant's chip is the model its task's run passes, and the
    /// task's own name for it; Settings' Model is untouched, and the next task starts on it. A
    /// pick the CLI no longer offers runs on its default, saying nothing.
    #[gpui::test]
    fn a_composer_pick_runs_its_task_alone(cx: &mut TestAppContext) {
        use crate::agents::task::Start;
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, None, cx);
        let start = |model: Option<&str>| Start {
            model: model.map(str::to_string),
            ..Start::default()
        };
        agents.update(cx, |agents, cx| {
            agents.set_threads(true, cx);
            agents.set_task_model(Cli::Claude, "sonnet", cx);
        });
        let picked = agents.update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, "Pick", None, 1, start(Some("haiku")), cx)
        });
        let settings = agents.update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, "Settings", None, 1, Tier::default(), cx)
        });
        let unknown = agents.update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, "Gone", None, 1, start(Some("opus-9")), cx)
        });
        let default = agents.update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, "Default", None, 1, start(Some("")), cx)
        });
        let [picked, settings, unknown, default] =
            [picked, settings, unknown, default].map(|agent| agent.expect("a task started"));
        agents.read_with(cx, |agents, _| {
            let run = |agent: &str| agents.launch_model(agent, Cli::Claude);
            let named = |agent: &str| agents.task(agent).unwrap().model.to_string();
            assert_eq!(
                run(&picked).as_deref(),
                Some("haiku"),
                "the pick runs its task"
            );
            assert_eq!(named(&picked), "Haiku");
            assert_eq!(
                run(&settings).as_deref(),
                Some("sonnet"),
                "the next starts on Settings'"
            );
            assert_eq!(named(&settings), "Sonnet");
            assert_eq!(
                run(&unknown),
                None,
                "a model not offered runs on the default"
            );
            assert_eq!(run(&default), None, "the default picked passes no model");
            assert_eq!(
                agents.task_model(Cli::Claude),
                "sonnet",
                "Settings' Model untouched"
            );
        });
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

    /// The composer opens in a profile with what it last started a thread with there: its level,
    /// except Full access, which opens at the level every thread starts at, and its model. Each
    /// profile keeps its own, with the effort Settings set there, and they're saved.
    #[gpui::test]
    fn the_composer_keeps_its_last_level_and_model_per_profile(cx: &mut TestAppContext) {
        use crate::agents::task::Last;
        let folder = std::env::temp_dir().join(format!("ilo-composer-last-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("Agents");
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        browser.profiles.borrow_mut().push((2, "Profile 2"));
        let agents = agents(&browser, &configs, Some(path.clone()), cx);
        let haiku = Some((Cli::Claude, "haiku".to_string()));
        agents.update(cx, |agents, _| {
            assert_eq!(agents.composer_last(1), Last::default(), "a first composer");
            agents.set_new_thread_effort(1, Some("high"));
            agents.remember_composer(1, Tier::Look, haiku.clone());
            agents.remember_composer(2, Tier::Full, None);
        });
        let opens = |agents: &Entity<Agents>, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| {
                (agents.composer_last(1), agents.composer_last(2))
            })
        };
        // Settings' effort outlasts a composer's start, which never sets it.
        let looked = Last {
            tier: Tier::Look,
            model: haiku.clone(),
            effort: Some("high"),
        };
        assert_eq!(opens(&agents, cx), (looked.clone(), Last::default()));
        let again = super::agents(&browser, &configs, Some(path.clone()), cx);
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("\"full\""), "Full access was kept: {saved}");
        again.update(cx, |agents, cx| agents.load(&saved, cx));
        assert_eq!(opens(&again, cx), (looked, Last::default()));
        std::fs::remove_dir_all(folder).unwrap();
    }
}

/// Scripts are off at first: every Apple Event, reads too, is refused and logged as refused under
/// its sender. On, each event is one entry, its command's, however many reads found its objects.
#[gpui::test]
fn scripts_are_refused_until_let_in_and_each_event_is_one_entry(cx: &mut TestAppContext) {
    use scripts::{AGENT, NEW_TAB, READ, SHOW_SPACE, detail, title};
    let (agents, _, _) = connected(cx);
    let entries = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| {
            (agents.activity(AGENT))
                .map(|entry| (title(entry), detail(entry)))
                .collect::<Vec<_>>()
        })
    };
    let script = |cx: &mut TestAppContext, event, action, site| {
        agents.update(cx, |agents, cx| {
            agents.script(event, "osascript in Terminal", action, site, "", cx)
        })
    };
    assert!(!agents.read_with(cx, |agents, _| agents.scripts_on()));
    assert!(!script(cx, 1, READ, ""));
    assert!(!script(cx, 1, READ, ""), "a second read of the same event");
    assert_eq!(
        entries(cx),
        vec![(
            "Read the windows, Spaces and tabs".to_string(),
            "Refused: scripts are off · osascript in Terminal".to_string()
        )]
    );

    agents.update(cx, |agents, cx| agents.set_scripts_on(true, cx));
    assert!(script(cx, 2, READ, ""));
    assert!(
        script(cx, 2, SHOW_SPACE, "Home"),
        "the command after its reads"
    );
    assert!(script(cx, 2, READ, ""), "a read after the command");
    assert!(script(cx, 3, NEW_TAB, "example.com"));
    let titles: Vec<String> = entries(cx).into_iter().map(|(title, _)| title).collect();
    assert_eq!(
        titles,
        vec![
            "Opened example.com in a new tab",
            "Showed the Space Home",
            "Read the windows, Spaces and tabs",
        ],
        "newest first, one entry for each event"
    );
    // No agent's activity holds them.
    let scripts_in_agents = agents.read_with(cx, |agents, _| {
        (agents.activity("claude-code")).any(|entry| !entry.by.is_empty())
    });
    assert!(!scripts_in_agents);
}

/// The scripts' switch is saved with the agents' settings, and comes back with them.
#[gpui::test]
fn the_scripts_switch_comes_back_with_the_settings(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.load(r#"{"scripts": true}"#, cx));
    assert!(agents.read_with(cx, |agents, _| agents.scripts_on()));
    agents.update(cx, |agents, cx| agents.load(r#"{}"#, cx));
    assert!(
        !agents.read_with(cx, |agents, _| agents.scripts_on()),
        "on with nothing saved"
    );
}

/// An outside agent's session (#1080): one task card for each connection of a client the user
/// connected, from its first call that opens or acts on a tab, until that connection closes, it
/// waits IDLE for a call, or the user stops it. The `ilo` command's spans its connections.
mod sessions {
    use super::*;
    use crate::agents::task::{
        Card, Conversation, Dot, IDLE, Line, NAMED_RECENTLY, Opens, QUIET, RENAMING, TITLE_KEPT,
        TaskAction, TraceStep, Word,
    };

    /// Calls `tool` as Claude Code, as call `id`, which `answered` ends.
    fn call_as(
        agents: &Entity<Agents>,
        id: u64,
        tool: &str,
        arguments: Value,
        cx: &mut TestAppContext,
    ) -> mpsc::Receiver<ToolResult> {
        call_at(agents, id, tool, arguments, std::time::Instant::now(), cx)
    }

    /// call_as, made at `at`, as the server stamps a call when it comes.
    fn call_at(
        agents: &Entity<Agents>,
        id: u64,
        tool: &str,
        arguments: Value,
        at: std::time::Instant,
        cx: &mut TestAppContext,
    ) -> mpsc::Receiver<ToolResult> {
        call_on(agents, 1, id, tool, arguments, at, cx)
    }

    /// call_at, on Claude Code's connection `connection`.
    fn call_on(
        agents: &Entity<Agents>,
        connection: u64,
        id: u64,
        tool: &str,
        arguments: Value,
        at: std::time::Instant,
        cx: &mut TestAppContext,
    ) -> mpsc::Receiver<ToolResult> {
        let (reply, answer) = mpsc::channel();
        let call = Call {
            id,
            agent: "claude-code".into(),
            connection,
            tool: tool.into(),
            arguments,
            reply,
            at,
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
            kind: None,
            image: None,
            notes: None,
        };
        agents.update(cx, |agents, cx| agents.handle(event, cx));
    }

    fn hello(agents: &Entity<Agents>, agent: &str, client: &str, cx: &mut TestAppContext) {
        hello_on(agents, agent, 1, client, cx);
    }

    fn hello_on(
        agents: &Entity<Agents>,
        agent: &str,
        connection: u64,
        client: &str,
        cx: &mut TestAppContext,
    ) {
        let event = Event::Hello {
            agent: agent.into(),
            connection,
            client: client.into(),
        };
        agents.update(cx, |agents, cx| agents.handle(event, cx));
    }

    /// Claude Code's connection `connection` closed, and it has others.
    fn closed(agents: &Entity<Agents>, connection: u64, cx: &mut TestAppContext) {
        let event = Event::Closed {
            agent: "claude-code".into(),
            connection,
        };
        agents.update(cx, |agents, cx| agents.handle(event, cx));
    }

    /// Claude Code's one connection closed, which was its last.
    fn bye(agents: &Entity<Agents>, cx: &mut TestAppContext) {
        closed(agents, 1, cx);
        let event = Event::Bye {
            agent: "claude-code".into(),
        };
        agents.update(cx, |agents, cx| agents.handle(event, cx));
    }

    fn cards(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<Card> {
        agents.read_with(cx, |agents, cx| agents.shown_cards(cx))
    }

    fn card(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) -> Card {
        let shown = cards(agents, cx);
        (shown.into_iter().find(|card| card.agent == key)).expect("its card")
    }

    fn conversation(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) -> Conversation {
        agents
            .read_with(cx, |agents, cx| agents.conversation(key, cx))
            .expect("its conversation")
    }

    /// A conversation's steps, a same step on several tabs each its own.
    fn steps(talk: &Conversation) -> usize {
        (talk.lines.iter())
            .map(|line| match line {
                Line::Step(TraceStep {
                    opens: Opens::Steps(steps),
                    ..
                }) => steps.len(),
                Line::Step(_) => 1,
                _ => 0,
            })
            .sum()
    }

    /// ilo's lines about a page in the task `key`'s conversation, a step handed to the user or an
    /// ask for the tab back: each line's key, its tab, and whether its buttons are gone.
    fn page_lines(
        agents: &Entity<Agents>,
        key: &str,
        cx: &mut TestAppContext,
    ) -> Vec<(usize, u64, bool)> {
        use crate::agents::task::ilo::IloKind;
        (conversation(agents, key, cx).lines.iter())
            .filter_map(|line| match line {
                Line::Ilo(line) => match line.kind {
                    IloKind::Yours { tab, .. } | IloKind::TabBack { tab } => {
                        Some((line.key, tab, line.settled))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// A second connection of Claude Code's, which has done nothing yet, hands tab 20 to the
    /// user, as call `id`: the page is the first connection's task's (`at_work`).
    fn second_hands_off(
        agents: &Entity<Agents>,
        id: u64,
        cx: &mut TestAppContext,
    ) -> mpsc::Receiver<ToolResult> {
        hello_on(agents, "claude-code", 2, "claude-code", cx);
        call_on(
            agents,
            2,
            id,
            "hand_off",
            json!({ "tab": 20, "reason": "Sign in to example.com" }),
            std::time::Instant::now(),
            cx,
        )
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

    /// A session's card has a home as a task's has, the Space it was asked from: the user's Space
    /// in view when its first call acted on a tab, which the browser says and the client doesn't.
    /// The user moving on leaves it there, a later session takes the Space in view then, and no
    /// answer tells the client where its card is.
    #[gpui::test]
    fn a_sessions_card_is_at_home_in_the_users_space_in_view(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        browser.user.set(Some(4242));
        hello(&agents, "claude-code", "claude-code", cx);
        let mut answers = vec![call_as(&agents, 1, "list_tabs", json!({}), cx)];
        answered(&agents, 1, cx);
        answers.push(call_as(&agents, 2, "read_page", json!({ "tab": 20 }), cx));
        let first = cards(&agents, cx)[0].clone();
        assert_eq!(first.home, 4242);
        browser.user.set(Some(4343));
        answers.push(call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx));
        answered(&agents, 2, cx);
        answered(&agents, 3, cx);
        assert_eq!(card(&agents, &first.agent, cx).home, 4242);
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let now = std::time::Instant::now();
        answers.push(call_on(
            &agents,
            2,
            4,
            "read_page",
            json!({ "tab": 20 }),
            now,
            cx,
        ));
        let second = cards(&agents, cx)[0].clone();
        assert_ne!(second.agent, first.agent);
        assert_eq!(second.home, 4343);
        cx.run_until_parked();
        for answer in answers {
            if let Ok(answer) = answer.try_recv() {
                for home in ["4242", "4343"] {
                    assert!(!answer.text.contains(home), "{}", answer.text);
                }
            }
        }
    }

    /// A private window is never a session's home: with one in view, its card lists in no Space.
    #[gpui::test]
    fn a_private_window_is_never_a_sessions_home(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        browser.private.borrow_mut().push(1);
        hello(&agents, "claude-code", "claude-code", cx);
        call_as(&agents, 1, "read_page", json!({ "tab": 20 }), cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].home, 0);
    }

    /// The first call that acts on a tab starts its connection's card, with its name, and a
    /// second connection of the client's is a card of its own, each with only its own steps. Its
    /// conversation is its steps, with no ask, and the user talks to it where they started it. Its
    /// connection closing ends it, with the pages it worked in to open, while the other's works
    /// on, and a call on a new connection is a session of its own.
    #[gpui::test]
    fn each_connection_is_a_card_of_its_own_until_it_closes(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        browser
            .tabs
            .borrow_mut()
            .push((31, 2, "Other", "https://example.org/"));
        // A second connection of Claude Code's.
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        call_on(
            &agents,
            2,
            3,
            "snapshot",
            json!({ "tab": 31 }),
            std::time::Instant::now(),
            cx,
        );
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 2, "one card for two connections");
        let second = shown[0].agent.to_string();
        assert_ne!(first, second);
        for (card, site) in shown.iter().zip(["example.org", "example.com"]) {
            // Unnamed, it's titled by the site it works on, its line by its client (#3690).
            assert_eq!(
                (card.name.as_ref(), card.title.as_ref(), card.dot),
                ("Claude Code", site, Dot::Working)
            );
            assert!(!card.replies);
            assert!(card.agent.starts_with("session-"), "{}", card.agent);
            let talk = conversation(&agents, &card.agent, cx);
            assert!(talk.asked.is_empty());
            assert_eq!(steps(&talk), 1, "{:?}", talk.lines);
            assert_eq!(talk.lines.len(), 1, "{:?}", talk.lines);
            assert_eq!(talk.about.as_ref(), "Claude Code · working");
        }
        // A reply has nowhere to go.
        let replied = agents.update(cx, |agents, cx| {
            agents.task_action(&first, TaskAction::Reply("and then?".into()), cx)
        });
        assert_eq!(replied, (None, None));

        answered(&agents, 2, cx);
        answered(&agents, 3, cx);
        closed(&agents, 1, cx);
        let ended = card(&agents, &first, cx);
        assert_eq!(ended.dot, Dot::Done);
        assert!(ended.status.starts_with("Done "), "{}", ended.status);
        assert_eq!(ended.open, vec![(20, "example.com".into())]);
        assert_eq!(card(&agents, &second, cx).dot, Dot::Working);

        hello_on(&agents, "claude-code", 3, "claude-code", cx);
        call_on(
            &agents,
            3,
            4,
            "read_page",
            json!({ "tab": 20 }),
            std::time::Instant::now(),
            cx,
        );
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 3);
        assert_eq!(shown[0].dot, Dot::Working);
        assert!(![first, second].contains(&shown[0].agent.to_string()));
    }

    /// One connection's calls are one card's steps, those its subagents make beside each other on
    /// other tabs included, as they share their session's connection.
    #[gpui::test]
    fn a_connections_calls_and_its_subagents_are_one_card(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        browser
            .tabs
            .borrow_mut()
            .push((31, 2, "Other", "https://example.org/"));
        // A subagent's call while the first is under way.
        call_as(&agents, 3, "snapshot", json!({ "tab": 31 }), cx);
        answered(&agents, 2, cx);
        call_as(&agents, 4, "read_page", json!({ "tab": 20 }), cx);
        answered(&agents, 3, cx);
        answered(&agents, 4, cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1, "a card for a subagent");
        let talk = conversation(&agents, &shown[0].agent, cx);
        assert_eq!(steps(&talk), 3, "{:?}", talk.lines);
        let pages: Vec<u64> = talk.tabs.iter().map(|tab| tab.0).collect();
        assert_eq!(pages, [20, 31]);
    }

    /// A call with no tab acts on its own connection's last tab, never another connection's. With
    /// none yet, a Space of one tab gives that one, and one of two is answered as a call that must
    /// name its tab is, acting on nothing.
    #[gpui::test]
    fn each_connections_default_tab_is_its_own(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        hello_on(&agents, "claude-code", 1, "claude-code", cx);
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let read = |connection, id, arguments, cx: &mut TestAppContext| {
            let before = browser.reads.borrow().len();
            let answer = call_on(
                &agents,
                connection,
                id,
                "read_page",
                arguments,
                std::time::Instant::now(),
                cx,
            );
            let read = browser.reads.borrow().get(before).map(|read| read.0);
            (read, answer)
        };
        // The Space's one tab.
        assert_eq!(read(1, 1, json!({}), cx).0, Some(20));
        browser
            .tabs
            .borrow_mut()
            .push((31, 2, "Other", "https://example.org/"));
        assert_eq!(read(1, 2, json!({}), cx).0, Some(20), "not its own last");
        // The second connection has none of its own, and the Space has two.
        let (none, answer) = read(2, 3, json!({}), cx);
        assert_eq!(none, None, "it read a tab it didn't name");
        let answer = answer.try_recv().expect("an answer at once");
        assert_eq!(answer, ToolResult::error(crate::agents::tools::NOT_A_TAB));
        assert_eq!(read(2, 4, json!({ "tab": 31 }), cx).0, Some(31));
        assert_eq!(read(2, 5, json!({}), cx).0, Some(31));
        assert_eq!(read(1, 6, json!({}), cx).0, Some(20), "the other's last");
    }

    /// The 30-day record of each connection's card has its own connection's steps alone.
    #[gpui::test]
    fn each_cards_record_has_its_own_connections_steps(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        browser
            .tabs
            .borrow_mut()
            .push((31, 2, "Other", "https://example.org/"));
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let looked = call_on(
            &agents,
            2,
            3,
            "snapshot",
            json!({ "tab": 31 }),
            std::time::Instant::now(),
            cx,
        );
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 2, cx);
        answered(&agents, 3, cx);
        closed(&agents, 2, cx);
        bye(&agents, cx);
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        assert_eq!(kept.len(), 2);
        for (_, past) in &kept {
            let actions: Vec<&str> = past.steps.iter().map(|step| step.action.as_str()).collect();
            let sites: Vec<&str> = past.steps.iter().map(|step| step.site.as_str()).collect();
            if past.id.starts_with(&format!("{first}-")) {
                assert_eq!(actions, ["read_page"], "{:?}", past.steps);
                assert_eq!(sites, ["example.com"]);
            } else {
                assert_eq!(actions, ["snapshot"], "{:?}", past.steps);
                assert_eq!(sites, ["example.org"]);
            }
        }
    }

    /// The disconnect noted after a session ended stays out of its record, though the Mac's clock,
    /// in microseconds, can give the two the very same time (#3967).
    #[gpui::test]
    fn the_disconnect_after_a_sessions_end_stays_out_of_its_record(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        answered(&agents, 2, cx);
        bye(&agents, cx);
        agents.update(cx, |agents, _| {
            let ended = (agents.tasks.tasks.iter()).find_map(|task| task.ended);
            let gone = (agents.log.iter_mut()).find(|entry| entry.action == "disconnected");
            gone.expect("the disconnect, noted").at = ended.expect("the session, ended");
        });
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        let [(_, past)] = kept.as_slice() else {
            panic!("one record: {kept:?}");
        };
        let actions: Vec<&str> = past.steps.iter().map(|step| step.action.as_str()).collect();
        assert_eq!(actions, ["read_page"], "{:?}", past.steps);
    }

    /// A handoff and a refusal each show on the card of the connection that asked or was refused,
    /// and never on the other's.
    #[gpui::test]
    fn a_handoff_and_a_refusal_show_on_their_own_card(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let opening = call_on(
            &agents,
            2,
            3,
            "open_tab",
            json!({ "url": "chase.com" }),
            std::time::Instant::now(),
            cx,
        );
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        let asks = |key: &str, cx: &mut TestAppContext| card(&agents, key, cx).question.is_some();
        let stopped = |key: &str, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.stops(key).len())
        };
        // Refused, which only its own conversation shows and its own record keeps.
        assert!(opening.recv().unwrap().error);
        assert_eq!(stopped(&second, cx), 1, "the bank's open isn't on its card");
        assert_eq!(stopped(&first, cx), 0, "the other's refusal is on its card");
        answered(&agents, 3, cx);
        assert!(!asks(&second, cx));

        let reason = json!({ "tab": 20, "reason": "Sign in to example.com" });
        let _handed = call_as(&agents, 4, "hand_off", reason, cx);
        assert!(asks(&first, cx), "the handoff didn't ask on its card");
        assert!(!asks(&second, cx), "the other's handoff is on its card");

        closed(&agents, 2, cx);
        bye(&agents, cx);
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        let refused = |key: &str| {
            let of = |past: &&crate::agent_history::Past| past.id.starts_with(&format!("{key}-"));
            let past = kept.iter().map(|each| &each.1).find(of).unwrap();
            (past.steps.iter())
                .filter(|step| !step.refused.is_empty())
                .map(|step| step.action.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(refused(&second), ["open_tab"]);
        assert!(refused(&first).is_empty(), "{:?}", refused(&first));
    }

    /// A page is the card's of the connection that opened it: a step on it by another connection
    /// is a step of that one's card, and the page stays its opener's.
    #[gpui::test]
    fn a_page_is_its_openers_card_whoever_steps_on_it(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        let answer = call_as(&agents, 3, "open_tab", json!({ "url": "example.org" }), cx);
        let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
        assert!(opened.text.starts_with("Opened tab 30."), "{}", opened.text);
        answered(&agents, 3, cx);

        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let looked = call_on(
            &agents,
            2,
            4,
            "snapshot",
            json!({ "tab": 30 }),
            std::time::Instant::now(),
            cx,
        );
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 4, cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);

        let opener = conversation(&agents, &first, cx);
        let other = conversation(&agents, &second, cx);
        assert_eq!(steps(&opener), 2, "{:?}", opener.lines);
        assert_eq!(steps(&other), 1, "{:?}", other.lines);
        let pages = |talk: &Conversation| talk.tabs.iter().map(|tab| tab.0).collect::<Vec<_>>();
        assert_eq!(pages(&opener), [20, 30]);
        assert!(pages(&other).is_empty(), "{:?}", other.tabs);

        // Both done, the page is its opener's card's to open.
        closed(&agents, 2, cx);
        bye(&agents, cx);
        let open = card(&agents, &first, cx).open;
        assert_eq!(
            open,
            vec![(20, "example.com".into()), (30, "example.org".into())]
        );
        assert!(card(&agents, &second, cx).open.is_empty());
    }

    /// A task's access holds for its pages whichever connection of its client acts on them. At
    /// Look only, a second connection's click on a page the first task holds is refused as Look
    /// only refuses it, though the user raised its own task to Work on pages, and the refusal is
    /// the line of the task that holds the page, whose access the line's button changes. Raised
    /// there again, the same click goes.
    #[gpui::test]
    fn a_tasks_access_holds_for_its_pages_whichever_connection_acts(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let looked = call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 3, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        agents.update(cx, |agents, cx| agents.set_tier(&first, Tier::Look, cx));

        (browser.tabs.borrow_mut()).push((31, 2, "Other", "https://example.org/"));
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let now = std::time::Instant::now;
        call_on(&agents, 2, 9, "snapshot", json!({ "tab": 31 }), now(), cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        agents.update(cx, |agents, cx| agents.set_tier(&second, Tier::Work, cx));
        let coupon = |id, cx: &mut TestAppContext| {
            let arguments = json!({ "tab": 20, "ref": "e1" });
            call_on(&agents, 2, id, "click", arguments, now(), cx)
        };
        let pages = browser.pages.borrow().len();
        let refused = (coupon(4, cx).try_recv()).expect("the click wasn't refused at once");
        assert!(refused.error, "it clicked: {}", refused.text);
        for said in ["click “Coupon” on example.com", "(Look only)"] {
            assert!(refused.text.contains(said), "{}", refused.text);
        }
        assert_eq!(
            browser.pages.borrow().len(),
            pages,
            "a second connection clicked on a page of a task at Look only"
        );
        answered(&agents, 4, cx);
        let stopped = |key: &str, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.stops(key))
        };
        let held = stopped(&first, cx);
        assert_eq!(held.len(), 1, "{held:?}");
        assert_eq!((held[0].why, held[0].tier), (Why::Act, Tier::Look));
        assert!(stopped(&second, cx).is_empty(), "the line is the caller's");
        assert_eq!(tier(&agents, &second, cx), Tier::Work);

        agents.update(cx, |agents, cx| agents.set_tier(&first, Tier::Work, cx));
        let clicked = coupon(5, cx);
        assert!(!page_done(&agents, &browser, clicked, shop_tree(), cx).error);
    }

    /// A task the user lowered is lowered for everyone who can reach its pages. From Full access
    /// to Work on pages while a second connection of its client is at work, itself at Full access:
    /// an order that connection sends on the first task's page is refused. The refusal is the
    /// first task's line and its card says Needs you, where its page's tile says Paused
    /// (`Shell::page_mark`); the second's has none and works on. The answer is the refusal's own
    /// form, with nothing of the first task's name.
    #[gpui::test]
    fn a_task_lowered_is_lowered_for_a_second_connection(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let looked = call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 3, cx);
        let named = call_as(&agents, 4, "title", json!({ "text": "Refund coat" }), cx);
        assert!(!named.recv().unwrap().error);
        answered(&agents, 4, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        assert_eq!(card(&agents, &first, cx).title, "Refund coat");
        agents.update(cx, |agents, cx| agents.set_tier(&first, Tier::Full, cx));

        (browser.tabs.borrow_mut()).push((31, 2, "Other", "https://example.org/"));
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let now = std::time::Instant::now;
        call_on(&agents, 2, 5, "snapshot", json!({ "tab": 31 }), now(), cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        agents.update(cx, |agents, cx| {
            agents.set_tier(&second, Tier::Full, cx);
            agents.set_tier(&first, Tier::Work, cx);
        });

        let pages = browser.pages.borrow().len();
        let order = json!({ "tab": 20, "ref": "e3" });
        let refused = call_on(&agents, 2, 6, "click", order, now(), cx);
        let refused = refused
            .try_recv()
            .expect("the order wasn't refused at once");
        assert!(refused.error, "it ordered: {}", refused.text);
        assert_eq!(
            refused.text,
            "ilo stopped this: click “Place order” on example.com. It isn't part of this task's \
             access (Work on pages). Only the user can change a task's access, in ilo. Don't try \
             another way to the same end; if they change it, the same call will go through."
        );
        assert!(!refused.text.contains("Refund"), "{}", refused.text);
        assert_eq!(
            browser.pages.borrow().len(),
            pages,
            "a second connection ordered on a page of a task lowered to Work on pages"
        );
        answered(&agents, 6, cx);

        let lines = |key: &str, cx: &mut TestAppContext| {
            let talk = conversation(&agents, key, cx);
            (talk.lines.iter())
                .filter(|line| matches!(line, Line::Ilo(line) if !line.settled))
                .count()
        };
        let held = agents.read_with(cx, |agents, _| agents.stops(&first));
        assert_eq!(held.len(), 1, "{held:?}");
        assert_eq!((held[0].why, held[0].tier), (Why::Order, Tier::Work));
        assert_eq!(lines(&first, cx), 1, "the line isn't the page's task's");
        assert_eq!(card(&agents, &first, cx).word, Word::NeedsYou);
        assert_eq!(lines(&second, cx), 0, "the line is the caller's");
        assert_eq!(card(&agents, &second, cx).word, Word::Working);
        let tier = |key: &str, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.task(key).map(|task| task.tier))
        };
        assert_eq!(tier(&second, cx), Some(Tier::Full));
    }

    /// An entry of the log names the level its act was judged at, never one it wasn't held to. A
    /// second connection of the client's, its own task at Full access, acts on a page of a task
    /// at Work on pages: its act is held to that task's access, so its entry, refused or done,
    /// says Work on pages, and stays that connection's. What it does on its own page says its
    /// own level.
    #[gpui::test]
    fn an_entry_names_the_level_its_act_was_judged_at(cx: &mut TestAppContext) {
        let (agents, browser, first) = lowered(Tier::Full, cx);
        (browser.tabs.borrow_mut()).push((31, 2, "Other", "https://example.org/"));
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let now = std::time::Instant::now;
        let looked = call_on(&agents, 2, 5, "snapshot", json!({ "tab": 31 }), now(), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 5, cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        agents.update(cx, |agents, cx| {
            agents.set_tier(&second, Tier::Full, cx);
            agents.set_tier(&first, Tier::Work, cx);
        });
        // The newest entry about a page of `site`: whose connection's, the level it names, and
        // whether it was refused.
        let named = |site: &str, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| {
                let entry = (agents.activity("claude-code"))
                    .find(|entry| entry.site == site)
                    .expect("no entry about that site");
                let refused = !entry.refused.is_empty();
                (entry.connection, entry.tier.to_string(), refused)
            })
        };
        let work = Tier::Work.label().to_string();

        let order = json!({ "tab": 20, "ref": "e3" });
        let refused = call_on(&agents, 2, 6, "click", order, now(), cx);
        assert!(refused.try_recv().expect("refused at once").error);
        answered(&agents, 6, cx);
        assert_eq!(named("example.com", cx), (2, work.clone(), true));

        let clicked = coupon(&agents, (2, 7, 20), cx);
        assert!(!page_done(&agents, &browser, clicked, shop_tree(), cx).error);
        answered(&agents, 7, cx);
        assert_eq!(named("example.com", cx), (2, work, false));

        let looked = call_on(&agents, 2, 8, "snapshot", json!({ "tab": 31 }), now(), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 8, cx);
        let full = Tier::Full.label().to_string();
        assert_eq!(named("example.org", cx), (2, full, false));
    }

    /// A file a page starts is judged where the page is, as an act there is: a second connection's
    /// read of a page of a task at Look only starts a download, which is refused and leaves no
    /// file, though that connection's own task is at the access a download goes at. The refusal
    /// answers the call that waits and is the line of the task that holds the page.
    #[gpui::test]
    fn a_download_is_judged_where_its_page_is(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        agents.update(cx, |agents, cx| agents.set_tier(&first, Tier::Look, cx));

        (browser.tabs.borrow_mut()).push((31, 2, "Other", "https://example.org/"));
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let now = std::time::Instant::now;
        call_on(&agents, 2, 3, "snapshot", json!({ "tab": 31 }), now(), cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        agents.update(cx, |agents, cx| agents.set_tier(&second, Tier::Work, cx));

        let reading = call_on(&agents, 2, 4, "screenshot", json!({ "tab": 20 }), now(), cx);
        let request = browser.pages.borrow().last().unwrap().1;
        download_started(&agents, 20, request, 900, cx);
        assert_eq!(
            *browser.downloads.borrow(),
            [(900, false)],
            "a second connection's download from a page of a task at Look only went"
        );
        let refused = reading.try_recv().expect("the download wasn't refused");
        assert!(refused.error, "{}", refused.text);
        for said in ["download report.docx from files.example.com", "(Look only)"] {
            assert!(refused.text.contains(said), "{}", refused.text);
        }
        let stopped = |key: &str, cx: &mut TestAppContext| {
            agents.read_with(cx, |agents, _| agents.stops(key))
        };
        let held = stopped(&first, cx);
        assert_eq!(held.len(), 1, "{held:?}");
        assert_eq!((held[0].why, held[0].tier), (Why::Download, Tier::Look));
        assert!(stopped(&second, cx).is_empty(), "the line is the caller's");
    }

    /// The access of the task `key`.
    fn tier(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) -> Tier {
        agents.read_with(cx, |agents, _| agents.task(key).expect("its task").tier)
    }

    /// Whether the task `key`'s conversation says the user changed its access.
    fn says_changed(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) -> bool {
        use crate::agents::task::ilo::IloKind;
        (conversation(agents, key, cx).lines.iter())
            .any(|line| matches!(line, Line::Ilo(line) if matches!(line.kind, IloKind::TierChanged { .. })))
    }

    /// Claude Code at work in its tab 20, whose page it has looked at, its task set to `tier`:
    /// the task's key.
    fn lowered(tier: Tier, cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, String) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let looked = call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 3, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        agents.update(cx, |agents, cx| agents.set_tier(&first, tier, cx));
        (agents, browser, first)
    }

    /// A click on the coupon of `tab`'s page, as call `id` on Claude Code's `connection`.
    fn coupon(
        agents: &Entity<Agents>,
        (connection, id, tab): (u64, u64, u64),
        cx: &mut TestAppContext,
    ) -> mpsc::Receiver<ToolResult> {
        let arguments = json!({ "tab": tab, "ref": "e1" });
        let now = std::time::Instant::now();
        call_on(agents, connection, id, "click", arguments, now, cx)
    }

    /// `answer` refused its click at once, as Look only refuses it.
    #[track_caller]
    fn refused_as_look_only(answer: mpsc::Receiver<ToolResult>) {
        let refused = answer.try_recv().expect("the click wasn't refused at once");
        assert!(refused.error, "it clicked: {}", refused.text);
        assert!(refused.text.contains("(Look only)"), "{}", refused.text);
    }

    /// A level the user lowered outlives its task, for that client, until they pick again. At
    /// Look only the agent says its task is done, and the same click on the same connection is a
    /// new task's, which starts at Look only and is refused as Look only refuses it: its chip
    /// says so, with no line saying the user set it, as they didn't on this task. The way out is
    /// the user's: raised on the new task, the same click goes, and what the ended task was left
    /// at is passed, so the task after starts where every task does.
    #[gpui::test]
    fn a_lowered_level_outlives_its_task_until_the_user_picks_again(cx: &mut TestAppContext) {
        let (agents, browser, first) = lowered(Tier::Look, cx);
        let said = call_as(&agents, 4, "done", json!({}), cx).recv().unwrap();
        assert!(!said.error, "{}", said.text);
        answered(&agents, 4, cx);

        let pages = browser.pages.borrow().len();
        refused_as_look_only(coupon(&agents, (1, 5, 20), cx));
        answered(&agents, 5, cx);
        assert_eq!(browser.pages.borrow().len(), pages, "the click went");
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        assert_eq!(tier(&agents, &second, cx), Tier::Look);
        assert!(!says_changed(&agents, &second, cx));
        let held = agents.read_with(cx, |agents, _| agents.stops(&second));
        assert_eq!(held.len(), 1, "{held:?}");
        assert_eq!((held[0].why, held[0].tier), (Why::Act, Tier::Look));
        // The log's entry of its first call names the level it started at.
        let logged = agents.read_with(cx, |agents, _| {
            let mut entries = agents.activity("claude-code");
            let click = entries.find(|entry| entry.action == "click");
            click.expect("its entry").tier.to_string()
        });
        assert_eq!(logged, "Look only");

        agents.update(cx, |agents, cx| agents.set_tier(&second, Tier::Work, cx));
        let clicked = coupon(&agents, (1, 6, 20), cx);
        assert!(!page_done(&agents, &browser, clicked, shop_tree(), cx).error);
        answered(&agents, 6, cx);

        // The user picked again: the task after it isn't held to the first's Look only.
        let said = call_as(&agents, 7, "done", json!({}), cx).recv().unwrap();
        assert!(!said.error, "{}", said.text);
        answered(&agents, 7, cx);
        let clicked = coupon(&agents, (1, 8, 20), cx);
        assert!(!page_done(&agents, &browser, clicked, shop_tree(), cx).error);
        let third = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(second, third);
        assert_eq!(tier(&agents, &third, cx), Tier::Work);
    }

    /// The level holds on a page no task of the client's held, too: after its done at Look only,
    /// the agent opens a new page, and its click there is refused.
    #[gpui::test]
    fn a_lowered_level_holds_on_a_page_opened_after_done(cx: &mut TestAppContext) {
        let (agents, browser, first) = lowered(Tier::Look, cx);
        let said = call_as(&agents, 4, "done", json!({}), cx).recv().unwrap();
        assert!(!said.error, "{}", said.text);
        answered(&agents, 4, cx);

        let answer = call_as(&agents, 5, "open_tab", json!({ "url": "example.org" }), cx);
        let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
        assert!(opened.text.starts_with("Opened tab 30."), "{}", opened.text);
        answered(&agents, 5, cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        let pages = browser.pages.borrow().len();
        refused_as_look_only(coupon(&agents, (1, 6, 30), cx));
        assert_eq!(browser.pages.borrow().len(), pages, "the click went");
    }

    /// The user's Stop ends the task and closes the client's connections, and nothing bars a new
    /// one. Its click on the stopped task's page is refused as stopped, and starts no task; the
    /// task of a page it opens starts at the level the stopped one was left at, so its click
    /// there is refused as Look only refuses it.
    #[gpui::test]
    fn a_lowered_level_holds_after_stop_on_a_new_connection(cx: &mut TestAppContext) {
        let (agents, browser, first) = lowered(Tier::Look, cx);
        agents.update(cx, |agents, cx| {
            agents.task_action(&first, TaskAction::Stop, cx)
        });
        assert!(card(&agents, &first, cx).status.starts_with("Stopped"));

        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let pages = browser.pages.borrow().len();
        let refused = coupon(&agents, (2, 4, 20), cx).try_recv();
        let refused = refused.expect("the click wasn't refused at once");
        assert!(refused.error, "it clicked: {}", refused.text);
        assert_eq!(refused.text, STOPPED);
        assert_eq!(
            cards(&agents, cx).len(),
            1,
            "the refused click started a task"
        );
        assert_eq!(browser.pages.borrow().len(), pages, "the click went");

        let now = std::time::Instant::now();
        let answer = call_on(
            &agents,
            2,
            5,
            "open_tab",
            json!({ "url": "example.org" }),
            now,
            cx,
        );
        let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
        assert!(opened.text.starts_with("Opened tab 30."), "{}", opened.text);
        answered(&agents, 5, cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        let pages = browser.pages.borrow().len();
        refused_as_look_only(coupon(&agents, (2, 6, 30), cx));
        assert_eq!(browser.pages.borrow().len(), pages, "the click went");
    }

    /// Full access is never carried: a task at Full access ends, and the client's next starts
    /// where every task does.
    #[gpui::test]
    fn full_access_isnt_carried_to_the_next_task(cx: &mut TestAppContext) {
        let (agents, _, first) = lowered(Tier::Full, cx);
        let said = call_as(&agents, 4, "done", json!({}), cx).recv().unwrap();
        assert!(!said.error, "{}", said.text);
        answered(&agents, 4, cx);
        call_as(&agents, 5, "snapshot", json!({ "tab": 20 }), cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        assert_eq!(tier(&agents, &second, cx), Tier::Work);
    }

    /// A task under way that takes over a page an ended task of its client's held goes no higher
    /// than that task's level, from the takeover: at Work on pages, its click on the page of a
    /// task that ended at Look only is refused and it is at Look only, by ilo's hand, so no line
    /// says the user set it. The user raises it on its chip, and the same click goes.
    #[gpui::test]
    fn a_task_takes_an_ended_tasks_page_at_no_more_than_its_level(cx: &mut TestAppContext) {
        let (agents, browser, first) = lowered(Tier::Full, cx);
        (browser.tabs.borrow_mut()).push((31, 2, "Other", "https://example.org/"));
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let now = std::time::Instant::now;
        let looked = call_on(&agents, 2, 9, "snapshot", json!({ "tab": 31 }), now(), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 9, cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        assert_eq!(tier(&agents, &second, cx), Tier::Work);

        agents.update(cx, |agents, cx| agents.set_tier(&first, Tier::Look, cx));
        closed(&agents, 1, cx);
        assert_eq!(card(&agents, &first, cx).dot, Dot::Done);
        // Its own page is still its own, at its own level.
        let own = coupon(&agents, (2, 4, 31), cx);
        assert!(!page_done(&agents, &browser, own, shop_tree(), cx).error);
        answered(&agents, 4, cx);
        assert_eq!(tier(&agents, &second, cx), Tier::Work);

        let pages = browser.pages.borrow().len();
        refused_as_look_only(coupon(&agents, (2, 5, 20), cx));
        answered(&agents, 5, cx);
        assert_eq!(browser.pages.borrow().len(), pages, "the click went");
        assert_eq!(tier(&agents, &second, cx), Tier::Look);
        assert!(!says_changed(&agents, &second, cx));
        let held = agents.read_with(cx, |agents, _| agents.stops(&second));
        assert_eq!(held.len(), 1, "{held:?}");

        agents.update(cx, |agents, cx| agents.set_tier(&second, Tier::Work, cx));
        let clicked = coupon(&agents, (2, 6, 20), cx);
        assert!(!page_done(&agents, &browser, clicked, shop_tree(), cx).error);
        assert_eq!(tier(&agents, &second, cx), Tier::Work);
    }

    /// The level is the client's alone: with Claude Code's task ended at Look only, another
    /// client's new task starts where every task does.
    #[gpui::test]
    fn another_clients_task_starts_where_every_task_does(cx: &mut TestAppContext) {
        let (agents, browser, _) = lowered(Tier::Look, cx);
        closed(&agents, 1, cx);
        agents.update(cx, |agents, cx| agents.connect(Client::Codex, cx));
        cx.run_until_parked();
        (browser.agent_spaces.borrow_mut()).insert("codex".into(), 3);
        (browser.tabs.borrow_mut()).push((40, 3, "Other", "https://example.org/"));
        hello(&agents, "codex", "codex", cx);
        let (reply, _answer) = mpsc::channel();
        let call = Call {
            id: 9,
            agent: "codex".into(),
            connection: 1,
            tool: "read_page".into(),
            arguments: json!({ "tab": 40 }),
            reply,
            at: std::time::Instant::now(),
        };
        agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
        let other = cards(&agents, cx)[0].clone();
        assert_eq!(other.name, "Codex");
        assert_eq!(tier(&agents, &other.agent, cx), Tier::Work);
    }

    /// Each of the client's tasks under way holds the next to its level, whatever the user picked
    /// last: with one lowered to Look only and then the other raised to Full access, a new task
    /// of that client's starts at Look only.
    #[gpui::test]
    fn a_new_task_starts_at_the_lowest_of_its_clients_under_way(cx: &mut TestAppContext) {
        let (agents, browser, first) = lowered(Tier::Look, cx);
        (browser.tabs.borrow_mut()).extend([
            (31, 2, "Other", "https://example.org/"),
            (32, 2, "Third", "https://example.net/"),
        ]);
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let now = std::time::Instant::now;
        call_on(&agents, 2, 4, "snapshot", json!({ "tab": 31 }), now(), cx);
        let second = cards(&agents, cx)[0].agent.to_string();
        assert_ne!(first, second);
        agents.update(cx, |agents, cx| agents.set_tier(&second, Tier::Full, cx));

        hello_on(&agents, "claude-code", 3, "claude-code", cx);
        call_on(&agents, 3, 5, "snapshot", json!({ "tab": 32 }), now(), cx);
        let third = cards(&agents, cx)[0].agent.to_string();
        assert!(third != first && third != second);
        assert_eq!(tier(&agents, &third, cx), Tier::Look);
        assert_eq!(tier(&agents, &second, cx), Tier::Full);
    }

    /// A step handed to the user stands where its page is (#4086). From a second connection of
    /// the client's, its line and Needs you are the task's that holds the page, and no task is
    /// made for the connection that only asked. Hand Back answers the connection that asked, as
    /// any hand-off is answered, with nothing of the task in it, and the task's word is its own
    /// state's again. The connection's task starts with its first work of its own, and the
    /// hand-off is no step of it.
    #[gpui::test]
    fn a_hand_off_stands_where_its_page_is_whichever_connection_asks(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        let titled = json!({ "text": "Refund order 1179" });
        call_as(&agents, 3, "title", titled, cx);
        answered(&agents, 2, cx);
        answered(&agents, 3, cx);
        let first = cards(&agents, cx)[0].agent.to_string();

        let handing = second_hands_off(&agents, 4, cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1, "a task for a connection that only asked");
        assert_eq!(
            (shown[0].dot, shown[0].word),
            (Dot::Waiting, Word::NeedsYou)
        );
        let asked = shown[0].question.as_ref().map(|question| question.tab);
        assert_eq!(asked, Some(20), "the step isn't its page's task's");
        let lines = page_lines(&agents, &first, cx);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!((lines[0].1, lines[0].2), (20, false));
        assert!(matches!(handing.try_recv(), Err(mpsc::TryRecvError::Empty)));

        agents.update(cx, |agents, cx| agents.hand_back(20, cx));
        let answer = handing.recv().unwrap();
        assert!(!answer.error);
        assert_eq!(
            answer.text,
            "The user handed tab 20 back. Take a snapshot to see where it is now."
        );
        answered(&agents, 4, cx);
        let back = cards(&agents, cx);
        assert_eq!(back.len(), 1, "the hand-back made the asker a task");
        assert_eq!((back[0].dot, back[0].word), (Dot::Working, Word::Working));
        assert_eq!(back[0].question, None);
        let lines = page_lines(&agents, &first, cx);
        assert_eq!((lines.len(), lines[0].2), (1, true), "{lines:?}");

        let answer = call_on(
            &agents,
            2,
            5,
            "open_tab",
            json!({ "url": "example.org" }),
            std::time::Instant::now(),
            cx,
        );
        let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
        assert!(opened.text.starts_with("Opened tab 30."), "{}", opened.text);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 2, "its own page made the asker no task");
        let second = shown[0].agent.to_string();
        assert_ne!(first, second);
        let own = conversation(&agents, &second, cx);
        assert_eq!(steps(&own), 1, "{:?}", own.lines);
        let pages = own.tabs.iter().map(|tab| tab.0).collect::<Vec<_>>();
        assert_eq!(pages, [30]);
        assert!(page_lines(&agents, &second, cx).is_empty());
    }

    /// An upload is the user's step as a hand-off is, so it stands where its page is too (#4086):
    /// asked by a second connection of the client's on the first task's page, it makes no task,
    /// and its line and Needs you are the first task's, until Hand Back answers the connection
    /// that asked.
    #[gpui::test]
    fn an_upload_stands_where_its_page_is_whichever_connection_asks(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        // The page's one element is a file field, e1.
        let form = json!({ "nodes": [
            { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Apply" },
              "childIds": ["2"] },
            { "nodeId": "2", "role": { "value": "button" }, "name": { "value": "Résumé" },
              "backendDOMNodeId": 61 },
        ]});
        let outline = call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx);
        assert!(!page_done(&agents, &browser, outline, form.to_string(), cx).error);
        answered(&agents, 3, cx);
        let first = cards(&agents, cx)[0].agent.to_string();

        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let field = json!({ "tab": 20, "ref": "e1" });
        let asking = call_on(
            &agents,
            2,
            4,
            "upload",
            field,
            std::time::Instant::now(),
            cx,
        );
        let (tab, request, action, element, _) = browser.pages.borrow().last().unwrap().clone();
        assert_eq!((tab, action, element), (20, PageAction::FileField, 61));
        // As the core writes it: Chromium's JSON writer gives every double a ".0".
        let described = json!({ "field": 7.0, "label": "Résumé", "site": "example.com" });
        agents.update(cx, |agents, cx| {
            agents.page_done(request, described.to_string(), None, cx)
        });
        assert!(matches!(asking.try_recv(), Err(mpsc::TryRecvError::Empty)));
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1, "a task for a connection that only asked");
        assert_eq!(shown[0].agent.to_string(), first);
        assert_eq!(
            (shown[0].dot, shown[0].word),
            (Dot::Waiting, Word::NeedsYou)
        );
        let lines = page_lines(&agents, &first, cx);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!((lines[0].1, lines[0].2), (20, false));

        agents.update(cx, |agents, cx| agents.hand_back(20, cx));
        let answer = asking.recv().unwrap();
        assert!(!answer.error, "{answer:?}");
        assert!(
            answer.text.contains("handed tab 20 back"),
            "{}",
            answer.text
        );
        answered(&agents, 4, cx);
        let back = cards(&agents, cx);
        assert_eq!(back.len(), 1, "the hand-back made the asker a task");
        assert_eq!((back[0].dot, back[0].word), (Dot::Working, Word::Working));
        let lines = page_lines(&agents, &first, cx);
        assert_eq!((lines.len(), lines[0].2), (1, true), "{lines:?}");
    }

    /// Not Now on the line, in the conversation of the task that holds the page, answers the
    /// connection that asked as any Not Now does, and still makes it no task.
    #[gpui::test]
    fn not_now_on_the_pages_tasks_line_answers_the_connection_that_asked(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        let handing = second_hands_off(&agents, 3, cx);
        let key = page_lines(&agents, &first, cx)[0].0;
        agents.update(cx, |agents, cx| {
            agents.task_action(&first, TaskAction::NotNow(key), cx)
        });
        let answer = handing.recv().unwrap();
        assert!(answer.error);
        let stays = "The user won't do the step on tab 20 now. It stays theirs; ";
        assert!(answer.text.starts_with(stays), "{}", answer.text);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1);
        assert_eq!((shown[0].word, &shown[0].question), (Word::Working, &None));
        assert_eq!(page_lines(&agents, &first, cx)[0].2, true);
    }

    /// Whether a connection may hand a page off is the core's answer, as before: a second
    /// connection's hand-off of a page that isn't its client's, the user's own or one that left
    /// the client's Space though a task of the client's worked in it, is refused in the words its
    /// first connection's is, and no task's line or word changes.
    #[gpui::test]
    fn a_hand_off_of_a_page_not_its_clients_is_refused_as_before(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let mut id = 2;
        let mut refused = |connection, tab: u64, cx: &mut TestAppContext| {
            id += 1;
            let reason = json!({ "tab": tab, "reason": "Sign in to example.com" });
            let now = std::time::Instant::now();
            let answer = call_on(&agents, connection, id, "hand_off", reason, now, cx)
                .try_recv()
                .expect("the hand-off wasn't refused at once");
            answered(&agents, id, cx);
            assert!(answer.error, "tab {tab} was handed off: {}", answer.text);
            answer.text
        };
        // The user's own tab.
        assert_eq!(refused(1, 10, cx), refused(2, 10, cx));
        // The task's page, which the user moved to a Space of their own.
        for tab in browser.tabs.borrow_mut().iter_mut() {
            if tab.0 == 20 {
                tab.1 = 1;
            }
        }
        assert_eq!(refused(1, 20, cx), refused(2, 20, cx));
        assert_eq!(shown(&agents, cx), None);
        assert!(browser.held.borrow().is_empty());
        assert!(page_lines(&agents, &first, cx).is_empty());
        assert_eq!(card(&agents, &first, cx).word, Word::Working);
    }

    /// An ask for a tab back stands where its page is as a step handed to the user does: from a
    /// second connection, it's the line and the Needs you of the task that holds the page, it
    /// makes the connection no task, and Keep on the line answers the connection that asked.
    #[gpui::test]
    fn an_ask_for_a_tab_back_stands_where_its_page_is(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let asking = call_on(
            &agents,
            2,
            3,
            "ask_for_tab",
            json!({ "tab": 20, "reason": "To finish the order" }),
            std::time::Instant::now(),
            cx,
        );
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1, "a task for a connection that only asked");
        assert_eq!(shown[0].word, Word::NeedsYou);
        let lines = page_lines(&agents, &first, cx);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!((lines[0].1, lines[0].2), (20, false));

        agents.update(cx, |agents, cx| {
            agents.task_action(&first, TaskAction::NotNow(lines[0].0), cx)
        });
        let answer = asking.recv().unwrap();
        assert!(answer.text.starts_with("The user keeps tab 20."));
        assert_eq!(cards(&agents, cx).len(), 1);
    }

    /// The connection that asked closing lets its hand-off go, as its own page's would: the line
    /// in the page's task loses its buttons and the task's word is its own again.
    #[gpui::test]
    fn the_asking_connection_closing_lets_its_hand_off_go(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        let _handing = second_hands_off(&agents, 3, cx);
        assert_eq!(card(&agents, &first, cx).word, Word::NeedsYou);

        closed(&agents, 2, cx);
        assert_eq!(shown(&agents, cx), None, "the hand-off outlived its asker");
        let left = card(&agents, &first, cx);
        assert_eq!((left.word, &left.question), (Word::Working, &None));
        assert_eq!(page_lines(&agents, &first, cx)[0].2, true);
    }

    /// One connection closing ends its own card alone. A hand-off another connection of the
    /// client's asked for on its page stays up for the user, though the ended task's line has no
    /// buttons and the asker has no task: its call waits on, and Hand Back from the page still
    /// answers it as any hand-off is answered.
    #[gpui::test]
    fn a_connection_closing_ends_its_card_alone(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        let handing = second_hands_off(&agents, 3, cx);
        assert!(shown(&agents, cx).is_some());

        closed(&agents, 1, cx);
        assert_eq!(card(&agents, &first, cx).dot, Dot::Done);
        assert_eq!(cards(&agents, cx).len(), 1);
        assert_eq!(page_lines(&agents, &first, cx)[0].2, true);
        assert!(shown(&agents, cx).is_some(), "its handoff went");
        assert!(matches!(handing.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(agents.read_with(cx, |agents, _| agents.is_live(Client::ClaudeCode)));
        assert!(browser.disconnected.borrow().is_empty());

        agents.update(cx, |agents, cx| agents.hand_back(20, cx));
        assert_eq!(
            handing.recv().unwrap().text,
            "The user handed tab 20 back. Take a snapshot to see where it is now."
        );
        assert!(browser.held.borrow().is_empty());
    }

    /// Nor is such a hand-off left waiting for good: 15 minutes after it, the connection that
    /// asked hears the tab wasn't handed back, as any hand-off does.
    #[gpui::test]
    fn a_hand_off_whose_pages_task_ended_answers_after_15_minutes(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        answered(&agents, 2, cx);
        let handing = second_hands_off(&agents, 3, cx);
        closed(&agents, 1, cx);
        cx.executor()
            .advance_clock(HANDOFF_WAIT - Duration::from_secs(1));
        assert!(handing.try_recv().is_err(), "it gave up before 15 minutes");
        cx.executor().advance_clock(Duration::from_secs(1));
        let answer = handing.recv().unwrap();
        assert!(answer.error);
        let waited = "The user hasn't handed tab 20 back in 15 minutes.";
        assert!(answer.text.starts_with(waited), "{}", answer.text);
    }

    /// The user stopping the task that holds the page stops the work on it: the hand-off another
    /// connection asked for there is let go at once, and its call is answered as a stopped task's
    /// own waiting call is (#4209).
    #[gpui::test]
    fn stopping_the_pages_task_answers_the_connection_that_asked(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        let handing = second_hands_off(&agents, 3, cx);
        agents.update(cx, |agents, cx| {
            agents.task_action(&first, TaskAction::Stop, cx)
        });
        let answer = handing.try_recv().expect("the hand-off outlived the Stop");
        assert_eq!((answer.error, answer.text.as_str()), (true, STOPPED));
        assert_eq!(answer.kind, Some(tools::Kind::Stopped));
        assert_eq!(shown(&agents, cx), None);
        assert_eq!(page_lines(&agents, &first, cx)[0].2, true);
    }

    /// A client connecting again starts a card of its own at its first acting call, which nothing
    /// ties to the last, and the access a task starts with: Full access ended with the last.
    #[gpui::test]
    fn a_reconnect_is_a_new_card_with_the_access_a_task_starts_with(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let first = cards(&agents, cx)[0].agent.to_string();
        let order = |connection, id, cx: &mut TestAppContext| {
            let arguments = json!({ "tab": 20, "ref": "e3" });
            call_on(
                &agents,
                connection,
                id,
                "click",
                arguments,
                std::time::Instant::now(),
                cx,
            )
        };
        let looked = call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 3, cx);
        give(&agents, Tier::Full, cx);
        let placed = order(1, 4, cx);
        assert!(!page_done(&agents, &browser, placed, shop_tree(), cx).error);
        answered(&agents, 4, cx);
        bye(&agents, cx);
        assert_eq!(card(&agents, &first, cx).dot, Dot::Done);

        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let looked = call_on(
            &agents,
            2,
            5,
            "snapshot",
            json!({ "tab": 20 }),
            std::time::Instant::now(),
            cx,
        );
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 5, cx);
        let shown = cards(&agents, cx);
        assert_eq!(
            shown.len(),
            2,
            "its new connection's calls went on the last card"
        );
        assert_ne!(shown[0].agent.to_string(), first);
        assert_eq!(shown[0].dot, Dot::Working);
        let talk = conversation(&agents, &shown[0].agent, cx);
        assert_eq!(steps(&talk), 1, "{:?}", talk.lines);
        let refused = order(2, 6, cx)
            .try_recv()
            .expect("the order wasn't refused at once");
        assert!(
            refused.error,
            "Full access went on to the new card: {}",
            refused.text
        );
        assert_eq!(stops(&agents, cx).len(), 1);
    }

    /// A client's earlier card offers only the pages none of its sessions works in now. Its Open
    /// moves what it offers out of the client's Space, which would take a tab from under the
    /// agent, whose next call on it is then refused (#3660).
    #[gpui::test]
    fn an_earlier_cards_open_leaves_the_pages_a_session_works_in(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        bye(&agents, cx);
        // It connects again and works on in the same tab.
        hello(&agents, "claude-code", "claude-code", cx);
        call_as(&agents, 3, "read_page", json!({ "tab": 20 }), cx);
        answered(&agents, 3, cx);
        let shown = cards(&agents, cx);
        assert_eq!((shown[0].dot, shown[1].dot), (Dot::Working, Dot::Done));
        assert!(shown[1].open.is_empty(), "{:?}", shown[1].open);
        let earlier = shown[1].agent.to_string();
        let opened = agents.update(cx, |agents, cx| agents.open_result(&earlier, None, cx));
        assert_eq!(opened, None);
        let space = browser
            .tabs
            .borrow()
            .iter()
            .find(|each| each.0 == 20)
            .unwrap()
            .1;
        assert_eq!(space, browser.agent_space("claude-code"));
        let answer = call_as(&agents, 4, "read_page", json!({ "tab": 20 }), cx);
        let refused = answer.try_recv().ok().filter(|answer| answer.error);
        assert!(refused.is_none(), "{refused:?}");

        // Once that session ends, the tab is the user's to open from either card.
        answered(&agents, 4, cx);
        bye(&agents, cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown[1].open, vec![(20, "example.com".into())]);
    }

    /// A call on a page of the agent's that left its Space says why it went, and that open_tab
    /// starts again: the user closed it, moved it to their own Space, or opened it from the card
    /// of the agent's earlier session, or the agent closed it. One that was never the agent's
    /// says no more than that it isn't in the agent's Space (#3660).
    #[gpui::test]
    fn a_call_on_a_page_that_left_says_why(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let refused = |id, tab, cx: &mut TestAppContext| {
            let answer = call_as(&agents, id, "read_page", json!({ "tab": tab }), cx);
            let answer = answer.try_recv().expect("an answer at once");
            assert!(answer.error, "{answer:?}");
            answer.text
        };
        let opened = |cx: &mut TestAppContext| {
            call_as(
                &agents,
                6,
                "open_tab",
                json!({ "url": "https://example.org/" }),
                cx,
            );
        };
        let leave = |tab, space, cx: &mut TestAppContext| {
            cx.update(|cx| browser.move_tab(tab, space, cx));
            agents.update(cx, |agents, cx| agents.space_changed(2, cx));
        };

        // A call on the user's own tab is refused, which doesn't make it a page of the agent's.
        assert_eq!(refused(7, 10, cx), NOT_ITS_SPACE);
        // The user moves it to their Space.
        leave(20, 1, cx);
        assert_eq!(
            refused(3, 20, cx),
            "The user moved tab 20 to one of their own Spaces: it's theirs now. To carry on, \
             open the page again with open_tab."
        );
        // The user closes one the agent opened.
        opened(cx);
        browser.tabs.borrow_mut().retain(|each| each.0 != 30);
        agents.update(cx, |agents, cx| agents.tab_closed(30, cx));
        assert_eq!(
            refused(4, 30, cx),
            "Tab 30 was closed, by the user or by its own page. To carry on, open the page \
             again with open_tab."
        );
        // The user's own tab was never the agent's, though the agent's Space changed since its
        // call on it.
        assert_eq!(refused(5, 10, cx), NOT_ITS_SPACE);
    }

    /// The core's refusals are decided on by their code, whatever their words (#3702): a held
    /// tab is the user's, a page that left the agent's Space still says why it went, and one
    /// that was never the agent's answers the core's words as they are.
    #[gpui::test]
    fn the_cores_refusal_is_decided_by_its_code(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let words = "Something else went on.";
        *browser.reworded.borrow_mut() = Some(words);
        let refused = |id, tab, cx: &mut TestAppContext| {
            let answer = call_as(&agents, id, "read_page", json!({ "tab": tab }), cx);
            let answer = answer.try_recv().expect("an answer at once");
            assert!(answer.error, "{answer:?}");
            answer
        };

        browser.held.borrow_mut().insert(20);
        let held = refused(3, 20, cx);
        assert_eq!(
            (held.text.as_str(), held.kind),
            (words, Some(tools::Kind::Yours))
        );
        browser.held.borrow_mut().remove(&20);

        cx.update(|cx| browser.move_tab(20, 1, cx));
        agents.update(cx, |agents, cx| agents.space_changed(2, cx));
        let moved = refused(4, 20, cx);
        assert_eq!(
            (moved.text.as_str(), moved.kind),
            (
                "The user moved tab 20 to one of their own Spaces: it's theirs now. To carry \
                 on, open the page again with open_tab.",
                None
            )
        );

        let never = refused(5, 10, cx);
        assert_eq!((never.text.as_str(), never.kind), (words, None));
    }

    /// The agent's own remove_tab, the one tool of its that takes a page out of its Space, says
    /// so of the page and of the rest of its split, which close with it, not that the user closed
    /// them, though the core says only that they closed (#3660).
    #[gpui::test]
    fn a_page_the_agent_removed_says_so(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        call_as(
            &agents,
            3,
            "open_tab",
            json!({ "url": "https://example.org/" }),
            cx,
        );
        call_as(&agents, 4, "split_tabs", json!({ "tabs": [20, 30] }), cx);
        call_as(&agents, 5, "pin_tab", json!({ "tab": 20 }), cx);
        let answer = call_as(&agents, 6, "remove_tab", json!({ "tab": 20 }), cx);
        assert!(!answer.try_recv().unwrap().error);
        let open = (browser.tabs.borrow().iter()).any(|tab| tab.0 == 20 || tab.0 == 30);
        assert!(!open, "the tab and its split closed");
        agents.update(cx, |agents, cx| {
            agents.tab_closed(20, cx);
            agents.tab_closed(30, cx);
        });
        let read = |id, tab, cx: &mut TestAppContext| {
            let answer = call_as(&agents, id, "read_page", json!({ "tab": tab }), cx);
            answer.try_recv().unwrap().text
        };
        assert_eq!(
            read(7, 20, cx),
            "The agent removed tab 20 from its pinned tabs, which closed it. To carry on, open \
             the page again with open_tab."
        );
        assert_eq!(
            read(8, 30, cx),
            "Tab 30 closed with its split view, as the agent removed tab 20 from its pinned tabs. \
             To carry on, open the page again with open_tab."
        );
    }

    /// The browser keeps why at most GONE_KEPT pages left, the lowest tab id going first, so a
    /// long run doesn't grow it without end.
    #[gpui::test]
    fn the_reasons_kept_are_bounded(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        let past = GONE_KEPT as u64 + 88;
        agents.update(cx, |agents, _| {
            for tab in 1..=past {
                agents.went(tab, "claude-code", Gone::Closed);
            }
            assert_eq!(agents.gone.len(), GONE_KEPT);
            assert_eq!(agents.gone.keys().min(), Some(&89));
            assert!(agents.gone.contains_key(&past));
        });
    }

    /// The pages of a session that ended are its client's next session's to work in, until the
    /// user opens them from its card, and the agent hears that, as it does of its own close.
    #[gpui::test]
    fn a_call_on_a_page_an_earlier_card_opened_says_so(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        bye(&agents, cx);
        let earlier = cards(&agents, cx)[0].agent.to_string();
        hello(&agents, "claude-code", "claude-code", cx);
        call_as(
            &agents,
            3,
            "open_tab",
            json!({ "url": "https://example.org/" }),
            cx,
        );
        agents.update(cx, |agents, cx| agents.open_result(&earlier, None, cx));
        let space = browser
            .tabs
            .borrow()
            .iter()
            .find(|each| each.0 == 20)
            .unwrap()
            .1;
        assert_eq!(space, 1);
        let answer = call_as(&agents, 4, "read_page", json!({ "tab": 20 }), cx);
        assert_eq!(
            answer.try_recv().unwrap().text,
            "The user opened tab 20 from the card of the agent's earlier session: it's theirs \
             now. To carry on, open the page again with open_tab."
        );
        call_as(&agents, 5, "close_tab", json!({ "tab": 30 }), cx);
        let answer = call_as(&agents, 7, "read_page", json!({ "tab": 30 }), cx);
        assert_eq!(
            answer.try_recv().unwrap().text,
            "The agent closed tab 30. To carry on, open the page again with open_tab."
        );
    }

    /// A client has one name wherever it shows, its card's (Agents::agent_name): the strip under its
    /// page, from its status, its questions and Settings' activity read the same. Another MCP
    /// client's is the name it gave, or with none its kind's, MCP client, and a session keeps the
    /// name it started with while it works, unless it would pass for the browser. Claude Code,
    /// connected by name, is Claude Code whatever it calls itself.
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
        // A name that would pass for the browser, whose words the cards set beside an agent's, has
        // its kind's: the app's name, and the product's as the user reads it, in any case.
        let app = env!("APP_NAME");
        let product = crate::product::PRODUCT;
        let mut capital = product.chars();
        let capital = capital.next().map_or(String::new(), |first| {
            first.to_uppercase().chain(capital).collect()
        });
        for posing in [
            app.to_string(),
            format!("{} agent", app.to_uppercase()),
            product.to_string(),
            format!("{} agent", product.to_uppercase()),
            format!("My {capital} helper"),
            "The browser".into(),
            "Checked by your browser".into(),
            // Another client's name is that client's, however it's spaced, joined or stopped, and
            // so is one holding an app's name.
            " claude code ".into(),
            "Codex".into(),
            "Claude  Code".into(),
            "Claude-Code".into(),
            "Claude Code.".into(),
            "Claude Code 2".into(),
            "My Codex".into(),
            "ilo-CLI".into(),
            // So is one that only reads as one (#3585): a word mixing scripts, letters of one
            // script that look like another's, compatibility forms, marks that don't show or
            // join, and any case.
            "Cl\u{0430}ude Code".into(),
            "\u{0456}lo agent".into(),
            "\u{0421}\u{043E}\u{0501}\u{0435}\u{0445}".into(),
            "\u{0131}lo agent".into(),
            "\u{FF49}\u{FF4C}\u{FF4F} agent".into(),
            "\u{FF23}\u{FF4F}\u{FF44}\u{FF45}\u{FF58}".into(),
            "\u{1D402}\u{1D428}\u{1D41D}\u{1D41E}\u{1D431}".into(),
            "Clau\u{200D}de Code".into(),
            "Co\u{200B}dex".into(),
            "i\u{034F}lo agent".into(),
            "Cla\u{0301}ude Code".into(),
            "cLaUdE cOdE".into(),
            "CODEX".into(),
            "\u{00ED}lo agent".into(),
            "C\u{0049}aude Code".into(),
            "C\u{0031}aude Code".into(),
            "\u{006C}\u{006C}o agent".into(),
            "\u{0031}lo agent".into(),
        ] {
            hello(&agents, "mcp-client", &posing, cx);
            assert_eq!(named(&agents, cx).0, "MCP client", "{posing} passed");
        }
        // A name of its own is shown as given, in any script, accents and all, and without what
        // doesn't show.
        for taken in [
            "Browser Use",
            "Checker",
            "Ren\u{00E9}e",
            "\u{0410}\u{0433}\u{0435}\u{043D}\u{0442}",
            "\u{30A8}\u{30FC}\u{30B8}\u{30A7}\u{30F3}\u{30C8}",
            "\u{81EA}\u{52D5}\u{30C6}\u{30B9}\u{30C8}",
            "\u{039A}\u{03CE}\u{03B4}\u{03B9}\u{03BA}\u{03B1}\u{03C2} 2",
        ] {
            hello(&agents, "mcp-client", taken, cx);
            assert_eq!(named(&agents, cx).0, taken);
        }
        hello(&agents, "mcp-client", "Zed\n\u{202E}Agent", cx);
        assert_eq!(named(&agents, cx).0, "Zed Agent");
        hello(&agents, "mcp-client", "Zed", cx);
        assert_eq!(named(&agents, cx), ("Zed".into(), Some("Zed".into())));

        // Its session's card has that name, and keeps it while it works.
        let (reply, _answer) = mpsc::channel();
        let call = Call {
            id: 1,
            agent: "mcp-client".into(),
            connection: 1,
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

    /// An outside agent's task starts at Work on pages, as it did before the composer had a level
    /// to pick: nothing of the composer reaches it.
    #[gpui::test]
    fn an_outside_agents_task_starts_at_work_on_pages(cx: &mut TestAppContext) {
        let (agents, _, _) = connected(cx);
        hello(&agents, "claude-code", "claude-code", cx);
        let _ = call_as(&agents, 1, "open_tab", json!({ "url": "example.com" }), cx);
        let tier = agents.read_with(cx, |agents, _| {
            (agents.tasks.tasks.iter())
                .find(|task| task.session.is_some())
                .expect("its session")
                .tier
        });
        assert_eq!(tier, Tier::Work);
    }

    /// A session's card, once it ends, is its receipt (#1080): the orders it placed, done, and not
    /// the pages it worked in, which Open's button counts. An order refused, or let go and failed in
    /// the page, isn't one placed. A session with nothing of the kind says when it ended.
    #[gpui::test]
    fn a_sessions_receipt_counts_its_pages_and_the_orders_it_placed(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        hello(&agents, "claude-code", "claude-code", cx);
        let looked = call_as(&agents, 1, "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 1, cx);
        let order = |id: u64, cx: &mut TestAppContext| {
            call_as(&agents, id, "click", json!({ "tab": 20, "ref": "e3" }), cx)
        };
        let refused = order(2, cx);
        assert!(refused.recv().unwrap().error);
        answered(&agents, 2, cx);
        give(&agents, Tier::Full, cx);
        let placed = order(3, cx);
        assert!(!page_done(&agents, &browser, placed, shop_tree(), cx).error);
        answered(&agents, 3, cx);
        let failed = order(4, cx);
        let request = browser.pages.borrow().last().unwrap().1;
        agents.update(cx, |agents, cx| {
            agents.page_done(
                request,
                "The button went away.".into(),
                Some(Failure::Other),
                cx,
            )
        });
        assert!(failed.recv().unwrap().error);
        answered(&agents, 4, cx);
        assert_eq!(
            cards(&agents, cx)[0].dot,
            Dot::Working,
            "the receipt came before the end"
        );

        bye(&agents, cx);
        let card = cards(&agents, cx).remove(0);
        assert_eq!(
            (card.dot, card.status.as_ref()),
            (Dot::Done, "Done · 1 order placed")
        );
        // Library › Agents keeps it once, as its receipt shows it: its title, agent, pages and the
        // order, never what the pages said.
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        assert_eq!(kept.len(), 1);
        let (_, past) = &kept[0];
        assert_eq!(
            (
                past.agent.as_str(),
                past.orders,
                past.messages,
                past.pages.len()
            ),
            ("Claude Code", 1, 0, 1)
        );
        assert!(past.ended >= past.started);
        // With its activity log, for its page there: what it did and where, and the order it was
        // refused, under its client's answers for good, which its page's Revoke takes (#1080).
        assert_eq!(past.answers, "claude-code");
        let actions: Vec<&str> = past.steps.iter().map(|step| step.action.as_str()).collect();
        assert!(actions.contains(&"snapshot"), "{actions:?}");
        // Each step with the access it was judged under.
        assert!(
            (past.steps.iter()).any(|step| step.action == "click"
                && step.refused.is_empty()
                && step.tier == "Full access"),
            "{:?}",
            past.steps
        );
        assert!(
            (past.steps.iter()).any(|step| step.action == "click"
                && !step.refused.is_empty()
                && step.tier == "Work on pages"),
            "the refused order isn't in its log: {:?}",
            past.steps
        );
        assert!((past.steps.iter()).all(|step| step.at >= past.started && step.at <= past.ended));
        assert!(
            agents
                .update(cx, |agents, cx| agents.take_ended(cx))
                .is_empty(),
            "kept twice"
        );

        hello(&agents, "claude-code", "claude-code", cx);
        let looked = call_as(&agents, 5, "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 5, cx);
        bye(&agents, cx);
        let card = cards(&agents, cx).remove(0);
        assert!(card.status.starts_with("Done · "), "{}", card.status);
        assert!(
            !card.status.contains("placed") && !card.status.contains("sent"),
            "a session that only looked has a receipt: {}",
            card.status
        );
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
        // Its time is when its last call ended, not IDLE later (#3652).
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let ended = kept[0].1.ended;
        assert!(
            now - ended >= IDLE.as_secs(),
            "it ended {} s ago, at its end, not at its last call",
            now - ended
        );
    }

    /// Claude Code at work in tab 20, then closing it with call 3: its last page (#3652).
    fn closes_its_last_page(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let closed = call_as(&agents, 3, "close_tab", json!({ "tab": 20 }), cx);
        assert_eq!(closed.recv().unwrap().text, "Closed tab 20.");
        answered(&agents, 3, cx);
        (agents, browser)
    }

    /// An agent that closes its last page and doesn't call again within QUIET is done (#3652):
    /// Claude Code keeps its connection open after its work, so without this its card said
    /// Working for IDLE over nothing. It has nothing to open, and says when it was done.
    #[gpui::test]
    fn an_agent_that_closed_its_last_page_is_done_once_quiet(cx: &mut TestAppContext) {
        let (agents, _) = closes_its_last_page(cx);
        cx.executor().advance_clock(QUIET - Duration::from_secs(1));
        assert_eq!(cards(&agents, cx)[0].dot, Dot::Working, "quiet isn't over");
        cx.executor().advance_clock(Duration::from_secs(1));
        let card = cards(&agents, cx).remove(0);
        assert_eq!(card.dot, Dot::Done, "{}", card.status);
        assert!(card.status.starts_with("Done · "), "{}", card.status);
        assert!(card.open.is_empty());
    }

    /// A call within QUIET of its last close is the same task, on the same card, and one that
    /// closes nothing starts no quiet end: IDLE ends it.
    #[gpui::test]
    fn a_call_within_quiet_keeps_the_task_on_its_card(cx: &mut TestAppContext) {
        let (agents, _) = closes_its_last_page(cx);
        cx.executor().advance_clock(QUIET - Duration::from_secs(1));
        call_as(&agents, 4, "list_tabs", json!({}), cx);
        answered(&agents, 4, cx);
        cx.executor().advance_clock(QUIET);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1, "one task, one card");
        assert_eq!(shown[0].dot, Dot::Working);
        cx.executor().advance_clock(IDLE);
        assert_eq!(cards(&agents, cx)[0].dot, Dot::Done);
    }

    /// A hand-off still open holds off the quiet end, as any call waiting on the user does: the
    /// agent closing its other page doesn't start it, and done names the call under way.
    #[gpui::test]
    fn a_handoff_open_holds_off_the_quiet_end(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        (browser.tabs.borrow_mut()).push((31, 2, "Other", "https://example.org/"));
        let reason = json!({ "tab": 31, "reason": "Sign in to example.org" });
        let _handed = call_as(&agents, 3, "hand_off", reason, cx);
        assert!(shown(&agents, cx).is_some(), "the handoff didn't ask");
        call_as(&agents, 4, "close_tab", json!({ "tab": 20 }), cx);
        answered(&agents, 4, cx);
        cx.executor().advance_clock(QUIET * 2);
        assert_ne!(cards(&agents, cx)[0].dot, Dot::Done);
        let refused = call_as(&agents, 5, "done", json!({}), cx).recv().unwrap();
        assert_eq!(
            refused,
            ToolResult::error(
                "Your hand_off on tab 31 is still under way: call done once it's answered."
            )
        );
    }

    /// The user closing the agent's last page doesn't end its task: it may well open it again,
    /// which would split one task over two cards. IDLE ends it.
    #[gpui::test]
    fn the_users_close_of_its_last_page_leaves_it_at_work(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        agents.update(cx, |agents, cx| {
            browser.close_user_tab(20, cx);
            agents.tab_closed(20, cx);
        });
        cx.executor().advance_clock(QUIET * 2);
        assert_eq!(cards(&agents, cx)[0].dot, Dot::Working);
        cx.executor().advance_clock(IDLE);
        assert_eq!(cards(&agents, cx)[0].dot, Dot::Done);
    }

    /// `done` is the agent's word that its task is done (#3652): its card is, with the pages it
    /// left open to open, and it's in its activity log. Its next call is a new task on a card of
    /// its own, which keeps nothing of the last but its tabs: Full access ended with the last.
    #[gpui::test]
    fn done_ends_the_task_with_its_pages_to_open(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        let looked = call_as(&agents, 3, "snapshot", json!({ "tab": 20 }), cx);
        answered(&agents, 2, cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 3, cx);
        let order = |id: u64, cx: &mut TestAppContext| {
            call_as(&agents, id, "click", json!({ "tab": 20, "ref": "e3" }), cx)
        };
        give(&agents, Tier::Full, cx);
        let placed = order(4, cx);
        assert!(!page_done(&agents, &browser, placed, shop_tree(), cx).error);
        answered(&agents, 4, cx);
        let said = call_as(&agents, 5, "done", json!({}), cx).recv().unwrap();
        assert!(!said.error, "{}", said.text);
        assert!(said.text.starts_with("Your task is done"), "{}", said.text);
        answered(&agents, 5, cx);
        let card = cards(&agents, cx).remove(0);
        assert_eq!(
            (card.dot, card.status.as_ref()),
            (Dot::Done, "Done · 1 order placed")
        );
        assert!(card.fuller, "a session's receipt isn't its word's form");
        assert_eq!(card.open, vec![(20, "example.com".into())]);
        assert!(
            (log(&agents, "claude-code", cx).iter()).any(|entry| entry.0 == "done"),
            "its log has no done"
        );
        // Nothing under way: it says so, and it's no error.
        let again = call_as(&agents, 6, "done", json!({}), cx).recv().unwrap();
        assert_eq!(again, ToolResult::text("No task of yours is under way."));
        assert_eq!(cards(&agents, cx).len(), 1, "done started a card");

        let looked = call_as(&agents, 7, "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 7, cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 2, "its next call is a new task");
        assert_eq!(shown[0].dot, Dot::Working);
        let refused = order(8, cx)
            .try_recv()
            .expect("the order wasn't refused at once");
        assert!(
            refused.error,
            "Full access went on to the next task: {}",
            refused.text
        );
    }

    /// `done` while a call of its own waits on the user, a handoff here, is refused, so it never
    /// leaves a question behind: its card still asks.
    #[gpui::test]
    fn done_waits_for_a_handoff_to_be_answered(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        answered(&agents, 2, cx);
        let reason = json!({ "tab": 20, "reason": "Sign in to example.com" });
        let _handed = call_as(&agents, 3, "hand_off", reason, cx);
        assert!(shown(&agents, cx).is_some(), "the handoff didn't ask");
        let refused = call_as(&agents, 4, "done", json!({}), cx).recv().unwrap();
        assert_eq!(
            refused,
            ToolResult::error(
                "Your hand_off on tab 20 is still under way: call done once it's answered."
            )
        );
        answered(&agents, 4, cx);
        let card = cards(&agents, cx).remove(0);
        assert_eq!(card.dot, Dot::Waiting);
        assert!(card.question.is_some());
    }

    /// What a `done` came to is a fact, which its caller logs by whatever its answer says: one
    /// refused for a call still under way is in the activity log with why, the one that ends the
    /// task is there too, and one with nothing under way is no error and isn't logged.
    #[gpui::test]
    fn done_is_logged_by_what_it_came_to(cx: &mut TestAppContext) {
        use crate::agents::task::SaidDone;
        let (agents, _browser) = at_work(cx);
        let dones = |cx: &mut TestAppContext| -> Vec<String> {
            (log(&agents, "claude-code", cx).into_iter())
                .filter(|entry| entry.0 == "done")
                .map(|entry| entry.2)
                .collect()
        };
        // Its read_page, call 2, is still under way.
        let refused = call_as(&agents, 3, "done", json!({}), cx).recv().unwrap();
        answered(&agents, 3, cx);
        assert!(refused.error, "{}", refused.text);
        assert_eq!(dones(cx), [refused.text.clone()]);
        answered(&agents, 2, cx);
        let said = call_as(&agents, 4, "done", json!({}), cx).recv().unwrap();
        answered(&agents, 4, cx);
        assert_eq!(said, SaidDone::Ended.answer());
        let mut logged = dones(cx);
        logged.sort();
        assert_eq!(logged, [String::new(), refused.text]);
        // Nothing under way: answered as such, with nothing more in the log.
        let nothing = agents.update(cx, |agents, cx| agents.said_done("claude-code", 5, cx));
        assert_eq!(nothing, SaidDone::NoTask);
        let again = call_as(&agents, 5, "done", json!({}), cx).recv().unwrap();
        assert_eq!(again, SaidDone::NoTask.answer());
        assert!(!again.error, "{}", again.text);
        assert_eq!(
            dones(cx).len(),
            2,
            "a done with nothing under way was logged"
        );
    }

    /// Either card's Stop stops the client on every connection, and each connection's card, which
    /// says it stopped: its calls under way are answered that the user stopped it, and its
    /// connections close, so it shows the browser's server as gone, with one "stopped by the user"
    /// in its activity log, which each card's record has once. Reconnected, its
    /// next call on a page of its own is a card of its own, and ⌘. from its page, which stops the
    /// client, stops that card too.
    #[gpui::test]
    fn stop_closes_the_clients_connections_and_their_cards(cx: &mut TestAppContext) {
        let (agents, browser) = at_work(cx);
        agents.update(cx, |agents, cx| agents.set_threads(true, cx));
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
        // Its second connection's card, which the client's Stop stops too.
        hello_on(&agents, "claude-code", 2, "claude-code", cx);
        let other = call_on(
            &agents,
            2,
            9,
            "read_page",
            json!({ "tab": 20 }),
            std::time::Instant::now(),
            cx,
        );
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 2);
        // The second card's Stop, which stops the client and so the first card too.
        let second = (shown.iter().find(|card| card.agent != agent.as_str()))
            .map(|card| card.agent.to_string())
            .expect("the second connection's card");
        agents.update(cx, |agents, cx| {
            agents.task_action(&second, TaskAction::Stop, cx)
        });
        assert_eq!(waiting.recv().unwrap().text, STOPPED);
        assert_eq!(other.recv().unwrap().text, STOPPED);
        assert_eq!(*browser.disconnected.borrow(), ["claude-code"]);
        for card in cards(&agents, cx) {
            assert_eq!(card.dot, Dot::Ended);
            assert!(card.status.starts_with("Stopped"), "{:?}", card.status);
        }
        for reader in &mut connections {
            assert_eq!(server::read_line(reader), None, "a connection stayed open");
        }
        let stops = |actions: Vec<&str>| {
            (actions.into_iter())
                .filter(|action| *action == "stopped by the user")
                .count()
        };
        let logged = log(&agents, "claude-code", cx);
        assert_eq!(
            stops(logged.iter().map(|entry| entry.0.as_str()).collect()),
            1,
            "{logged:?}"
        );
        // Each card's record has the one Stop, as the client's.
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        assert_eq!(kept.len(), 2);
        for (_, past) in &kept {
            let actions = past.steps.iter().map(|step| step.action.as_str()).collect();
            assert_eq!(stops(actions), 1, "{:?}", past.steps);
        }

        // On a page that isn't a stopped task's: those are refused it until the user hands them
        // back (sessions::stopped).
        (browser.tabs.borrow_mut()).push((31, 2, "Other", "https://example.org/"));
        call_as(&agents, 4, "read_page", json!({ "tab": 31 }), cx);
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

    /// The `ilo` command's connection closes after each command, which isn't its end: its card
    /// stays at work over the commands, its activity log says it connected once, and it ends as an
    /// idle session does, after which it has no status and isn't live (#3667).
    #[gpui::test]
    fn the_commands_session_lasts_over_its_connections(cx: &mut TestAppContext) {
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, None, cx);
        agents.update(cx, |agents, cx| agents.connect(Client::Command, cx));
        cx.run_until_parked();
        assert_eq!(
            agents.read_with(cx, |agents, _| agents.command_state()),
            crate::agents::cli::State::On
        );
        let agent = Client::Command.id();
        let command = |id: u64, tool: &str, arguments: Value, cx: &mut TestAppContext| {
            let (reply, answer) = mpsc::channel();
            // Each command comes on a connection of its own, as the `ilo` command makes one a call.
            let call = Call {
                id,
                agent: agent.into(),
                connection: id,
                tool: tool.into(),
                arguments,
                reply,
                at: std::time::Instant::now(),
            };
            agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
            let event = Event::Answered {
                agent: agent.into(),
                id,
                at: std::time::Instant::now(),
                error: None,
                kind: None,
                image: None,
                notes: None,
            };
            agents.update(cx, |agents, cx| agents.handle(event, cx));
            let closed = Event::Closed {
                agent: agent.into(),
                connection: id,
            };
            agents.update(cx, |agents, cx| agents.handle(closed, cx));
            let bye = Event::Bye {
                agent: agent.into(),
            };
            agents.update(cx, |agents, cx| agents.handle(bye, cx));
            answer
        };
        // A command can't pass for an app the user connected.
        for posing in [
            "Claude Code",
            " CODEX",
            "Claude-Code.",
            "My Codex",
            "MCP  client",
            "C\u{043E}dex",
        ] {
            hello(&agents, agent, posing, cx);
            let name = agents.read_with(cx, |agents, _| agents.agent_name(agent).to_string());
            assert_eq!(name, "ilo CLI", "{posing} passed");
        }
        hello(&agents, agent, "Researcher", cx);
        command(1, "read_page", json!({ "tab": 20 }), cx);
        // A later command naming another keeps the card's name.
        hello(&agents, agent, "Someone else", cx);
        command(2, "snapshot", json!({ "tab": 20 }), cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1, "a card for each command: {shown:?}");
        assert_eq!(shown[0].dot, Dot::Working);
        assert_eq!(shown[0].name.as_ref(), "Researcher");
        assert!(agents.read_with(cx, |agents, _| agents.is_live(Client::Command)));
        assert!(browser.disconnected.borrow().is_empty());
        let connected = log(&agents, agent, cx)
            .into_iter()
            .filter(|(action, _, _)| action == "connected")
            .count();
        assert_eq!(connected, 2, "Add's line and the first command's");

        cx.executor().advance_clock(IDLE + Duration::from_secs(1));
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].dot, Dot::Done);
        assert!(!agents.read_with(cx, |agents, _| agents.is_live(Client::Command)));
        assert_eq!(*browser.disconnected.borrow(), [agent]);
        assert!(agents.read_with(cx, |agents, _| agents.status()).is_empty());

        // Its next command is a session of its own, with the name that command gives.
        hello(&agents, agent, "Writer", cx);
        command(3, "snapshot", json!({ "tab": 20 }), cx);
        let shown = cards(&agents, cx);
        assert_eq!(shown.len(), 2, "{shown:?}");
        let working: Vec<_> = (shown.iter())
            .filter(|card| card.dot == Dot::Working)
            .map(|card| card.name.to_string())
            .collect();
        assert_eq!(working, ["Writer"]);
    }

    /// The command's row is Add until it's added, then says whether the user's shell looks in its
    /// folder; Remove takes it away as Disconnect does a client, and it's never under Apps that
    /// connect.
    #[gpui::test]
    fn the_commands_row_follows_add_and_remove(cx: &mut TestAppContext) {
        use crate::agents::cli::State;
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, None, cx);
        let state =
            |cx: &mut TestAppContext| agents.read_with(cx, |agents, _| agents.command_state());
        assert_eq!(state(cx), State::Off);
        *configs.unsearched.lock().unwrap() = true;
        agents.update(cx, |agents, cx| agents.connect(Client::Command, cx));
        cx.run_until_parked();
        // On Windows Add puts the folder on PATH itself, so a folder off it is Add again (#3783).
        let unsearched = if cfg!(windows) {
            State::Off
        } else {
            State::Unsearched
        };
        assert_eq!(state(cx), unsearched);
        assert_eq!(configs.connected.lock().unwrap()[0].0, Client::Command);
        *configs.unsearched.lock().unwrap() = false;
        agents.update(cx, |agents, cx| agents.look_for_clients(cx));
        cx.run_until_parked();
        assert_eq!(state(cx), State::On);
        assert!(
            !agents
                .read_with(cx, |agents, _| agents.listed())
                .contains(&Client::Command)
        );
        agents.update(cx, |agents, cx| agents.disconnect(Client::Command, cx));
        cx.run_until_parked();
        assert_eq!(state(cx), State::Off);
        assert_eq!(configs.disconnected.lock().unwrap()[0].0, Client::Command);
    }

    fn titled(agents: &Entity<Agents>, cx: &mut TestAppContext) -> String {
        cards(agents, cx)
            .iter()
            .find(|card| card.dot == Dot::Working)
            .expect("no session at work")
            .title
            .to_string()
    }

    /// An outside agent names its session with title, as a ⌘T task does: its words, trimmed, are
    /// the card's title, and its card's line leads with its client's name, with no step count.
    /// Past TITLE_KEPT characters they're clipped, counted before they're written out, and the
    /// answer says what was kept; what wouldn't show or would move the text around it is written
    /// out, so the title is one line (#3645, #3690).
    #[gpui::test]
    fn a_sessions_title_is_its_agents_words_on_one_line(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        assert_eq!(titled(&agents, cx), "example.com");
        let empty = call_as(&agents, 3, "title", json!({ "text": "  " }), cx);
        let empty = empty.recv().unwrap();
        assert!(empty.error, "{empty:?}");
        assert_eq!(empty.text, "text is the task's name, in two to four words.");

        let start = std::time::Instant::now();
        let named = call_at(
            &agents,
            4,
            "title",
            json!({ "text": "  Refund order 1179 " }),
            start,
            cx,
        );
        let named = named.recv().unwrap();
        assert!(!named.error);
        assert_eq!(named.text, "The task has its name.");
        let card = cards(&agents, cx).remove(0);
        assert_eq!(card.title, "Refund order 1179");
        assert_eq!(card.name, "Claude Code");
        assert!(
            card.status.starts_with("Claude Code · ") && !card.status.contains("Step"),
            "{}",
            card.status
        );
        // Its header leads with the client too, its state a fact after it, in lower case.
        let agent = card.agent.to_string();
        let talk = agents
            .read_with(cx, |agents, cx| agents.conversation(&agent, cx))
            .unwrap();
        assert_eq!(talk.about, "Claude Code · working");

        let words = format!("Line\none\ttwo\u{202E}{}", "x".repeat(40));
        let kept: String = words.chars().take(TITLE_KEPT).collect::<String>() + "…";
        let clipped = call_at(
            &agents,
            5,
            "title",
            json!({ "text": words }),
            start + RENAMING,
            cx,
        );
        let clipped = clipped.recv().unwrap();
        assert!(!clipped.error, "{clipped:?}");
        assert_eq!(clipped.text, format!("The task has its name: “{kept}”."));
        let title = titled(&agents, cx);
        assert_eq!(
            title,
            format!(
                "Line\\u{{000A}}one\\u{{0009}}two\\u{{202E}}{}…",
                "x".repeat(27)
            )
        );
        assert!(!title.contains(['\n', '\t', '\u{202E}']), "{title:?}");
    }

    /// A new name waits RENAMING after the last change, so a title can't flicker: the same name
    /// again changes nothing and is never refused, and a different one sooner is refused and kept
    /// in the activity log as refused, with the title as it was.
    #[gpui::test]
    fn a_session_is_renamed_no_sooner_than_a_minute_after(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        let start = std::time::Instant::now();
        let name = |id: u64, text: &str, after: Duration, cx: &mut TestAppContext| {
            let answer = call_at(
                &agents,
                id,
                "title",
                json!({ "text": text }),
                start + after,
                cx,
            );
            answer.recv().unwrap()
        };
        assert!(!name(3, "Refund order 1179", Duration::ZERO, cx).error);
        let again = name(4, "Refund order 1179", Duration::from_secs(1), cx);
        assert_eq!(
            (again.error, again.text.as_str()),
            (false, "The task has its name.")
        );
        let soon = name(5, "Pay invoice 88", RENAMING - Duration::from_secs(1), cx);
        assert_eq!((soon.error, soon.text.as_str()), (true, NAMED_RECENTLY));
        assert_eq!(titled(&agents, cx), "Refund order 1179");
        let logged = log(&agents, "claude-code", cx);
        assert!(
            logged
                .iter()
                .any(|(action, _, refused)| action == "title" && refused == NAMED_RECENTLY),
            "{logged:?}"
        );
        assert!(!name(6, "Pay invoice 88", RENAMING, cx).error);
        assert_eq!(titled(&agents, cx), "Pay invoice 88");
    }

    /// A session its agent never names is titled by the sites it acted on: its client's name
    /// before any, its first site alone, then the first and how many more, changing no sooner
    /// than RENAMING after the last change; its Library row keeps them all, however soon it ended.
    #[gpui::test]
    fn an_unnamed_session_is_titled_by_its_sites(cx: &mut TestAppContext) {
        let (agents, _, _) = connected(cx);
        hello(&agents, "claude-code", "claude-code", cx);
        let start = std::time::Instant::now();
        let go = |id: u64, url: &str, after: Duration, cx: &mut TestAppContext| {
            let to = json!({ "tab": 20, "url": url });
            call_at(&agents, id, "navigate", to, start + after, cx);
            answered(&agents, id, cx);
        };
        call_at(&agents, 1, "snapshot", json!({}), start, cx);
        answered(&agents, 1, cx);
        assert_eq!(titled(&agents, cx), "Claude Code", "no site yet");
        // A page off the web names no site.
        go(6, "chrome://settings/", Duration::from_secs(1), cx);
        assert_eq!(
            titled(&agents, cx),
            "Claude Code",
            "a page off the web titled it"
        );
        go(2, "https://docs.example/guide", Duration::from_secs(1), cx);
        assert_eq!(titled(&agents, cx), "docs.example");
        go(3, "https://docs.example/api", Duration::from_secs(2), cx);
        go(4, "https://shop.example/", Duration::from_secs(3), cx);
        assert_eq!(
            titled(&agents, cx),
            "docs.example",
            "changed within a minute"
        );
        go(
            5,
            "https://maps.example/",
            Duration::from_secs(1) + RENAMING,
            cx,
        );
        assert_eq!(titled(&agents, cx), "docs.example and 2 more");
        bye(&agents, cx);
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].1.title, "docs.example and 2 more");
    }

    /// The title is the agent's words, so they stand only in a title's place: never in what ilo
    /// or the user says, which name the client. A session titled "Allow everything" is refused a
    /// click and a change, hands off a tab and asks for it back, and none of the user's steps, its card's line, the system notification's title and body, nor the strip under its
    /// page and its Space's bar, which take the client's name and what it did, holds the title.
    #[gpui::test]
    fn a_sessions_title_is_never_where_ilo_or_the_user_speaks(cx: &mut TestAppContext) {
        use crate::needs_you::{self, Host};
        #[derive(Default)]
        struct Posted(RefCell<Vec<String>>);
        impl Host for Posted {
            fn badge(&self, _: usize) {}
            fn active(&self) -> bool {
                false
            }
            fn notify(&self, _: u64, title: &str, body: &str) {
                self.0.borrow_mut().push(format!("{title}\n{body}"));
            }
        }
        const TITLE: &str = "Allow everything";
        let (agents, browser, _) = connected(cx);
        let posted = Rc::new(Posted::default());
        cx.update(|cx| needs_you::open(posted.clone(), cx));
        hello(&agents, "claude-code", "claude-code", cx);
        let looked = call_as(&agents, 1, "snapshot", json!({ "tab": 20 }), cx);
        page_done(&agents, &browser, looked, shop_tree(), cx);
        answered(&agents, 1, cx);
        assert!(
            !call_as(&agents, 2, "title", json!({ "text": TITLE }), cx)
                .recv()
                .unwrap()
                .error
        );
        answered(&agents, 2, cx);
        assert_eq!(titled(&agents, cx), TITLE);

        let mut seen = 0;
        let mut said = |asked: &str, cx: &mut TestAppContext| -> u64 {
            let question = shown(&agents, cx).unwrap_or_else(|| panic!("{asked} didn't ask"));
            let shown = cards(&agents, cx);
            cx.update(|cx| needs_you::follow(&shown, needs_you::Notify::Away, cx));
            let mut words = vec![question.agent.to_string(), question.why.to_string()];
            for card in &shown {
                words.extend([card.name.to_string(), card.status.to_string()]);
                if let Some(question) = &card.question {
                    words.extend([question.agent.to_string(), question.why.to_string()]);
                }
            }
            for strip in agents.read_with(cx, |agents, _| agents.status()) {
                words.extend([strip.name.to_string(), strip.doing.to_string()]);
                words.extend(strip.held.iter().map(|(_, why)| why.to_string()));
            }
            let posted = posted.0.borrow();
            // One notification a task for each state, so its first need posts and those after don't.
            assert_eq!(posted.len() > seen, seen == 0, "{asked}: {posted:?}");
            words.extend(posted[seen..].iter().cloned());
            seen = posted.len();
            for word in words {
                assert!(!word.contains(TITLE), "{asked}: {word:?}");
            }
            assert_eq!(question.agent, "Claude Code", "{asked}");
            question.id
        };

        // A refusal's words, to the agent and on its line, and the card's, never hold it either.
        let change = json!({ "tab": 20, "requests": [
            { "method": "POST", "path": "/api/items", "what": "Add milk to the list." },
        ] });
        let refusals = [
            (3, "click", json!({ "tab": 20, "ref": "e3" })),
            (4, "request", change),
        ];
        for (id, tool, arguments) in refusals {
            let refused = call_as(&agents, id, tool, arguments, cx).recv().unwrap();
            assert!(refused.error, "{tool} went");
            let mut words = vec![refused.text];
            for stop in stops(&agents, cx) {
                let why = super::super::tier::why_line(stop.why, stop.tier, &stop.site);
                words.extend([stop.at, stop.site, why]);
            }
            for card in cards(&agents, cx) {
                words.extend([card.name.to_string(), card.status.to_string()]);
            }
            for word in words {
                assert!(!word.contains(TITLE), "{tool}: {word:?}");
            }
            answered(&agents, id, cx);
        }

        let reason = json!({ "tab": 20, "reason": "Sign in to example.com" });
        let handed = call_as(&agents, 5, "hand_off", reason, cx);
        said("the handoff", cx);
        agents.update(cx, |agents, cx| agents.hand_back(20, cx));
        assert!(!handed.recv().unwrap().error);
        answered(&agents, 5, cx);

        agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
        let reason = json!({ "tab": 20, "reason": "To finish the order" });
        call_as(&agents, 6, "ask_for_tab", reason, cx);
        said("asking for the tab", cx);
        assert_eq!(cards(&agents, cx)[0].title, TITLE, "it kept its title");
    }

    /// A session's title goes with its Library row: it's in the 30-day file alone, which history
    /// deleted over the session's time takes, title and all, and history deleted at another time
    /// leaves it.
    #[gpui::test]
    fn a_sessions_title_goes_with_the_history_deleted(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        call_as(
            &agents,
            3,
            "title",
            json!({ "text": "Refund order 1179" }),
            cx,
        );
        answered(&agents, 2, cx);
        answered(&agents, 3, cx);
        bye(&agents, cx);
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        let past = kept[0].1.clone();
        assert_eq!(past.title, "Refund order 1179");
        let file = crate::agent_history::write(&[past.clone()]);
        let read = crate::agent_history::read(&file);
        assert_eq!(read[0].title, "Refund order 1179");
        let gone = crate::agent_history::outside(read.clone(), past.started, past.ended + 1);
        assert!(gone.is_empty(), "{gone:?}");
        let later = crate::agent_history::outside(read, past.ended + 60, past.ended + 120);
        assert_eq!(later[0].title, "Refund order 1179");
    }

    /// A title alone opens the session, so its card has its name from its first frame, but takes
    /// no tab and asks nothing; a session that only named itself leaves no Library row, and one
    /// that then worked leaves its row with that name.
    #[gpui::test]
    fn a_title_alone_takes_nothing_and_leaves_no_row(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        hello(&agents, "claude-code", "claude-code", cx);
        let named = call_as(
            &agents,
            1,
            "title",
            json!({ "text": "Refund order 1179" }),
            cx,
        );
        assert!(!named.recv().unwrap().error);
        assert_eq!(titled(&agents, cx), "Refund order 1179");
        assert_eq!(shown(&agents, cx), None, "it asked");
        assert!(browser.checked.borrow().is_empty(), "it acted");
        assert!(browser.pages.borrow().is_empty() && browser.reads.borrow().is_empty());
        assert!(
            agents
                .read_with(cx, |agents, _| agents.grants("claude-code"))
                .is_empty()
        );
        answered(&agents, 1, cx);
        bye(&agents, cx);
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        assert!(kept.is_empty(), "a title alone left a row: {kept:?}");

        hello(&agents, "claude-code", "claude-code", cx);
        call_as(
            &agents,
            2,
            "title",
            json!({ "text": "Refund order 1179" }),
            cx,
        );
        call_as(&agents, 3, "read_page", json!({ "tab": 20 }), cx);
        answered(&agents, 2, cx);
        answered(&agents, 3, cx);
        bye(&agents, cx);
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].1.title, "Refund order 1179");
    }

    /// title is refused as any call is to a client the user hasn't connected, before it reaches a
    /// task: no session and no card.
    #[gpui::test]
    fn a_title_from_a_client_not_connected_is_refused(cx: &mut TestAppContext) {
        let (agents, _, _) = connected(cx);
        let named = call(
            &agents,
            "codex",
            "title",
            json!({ "text": "Refund order 1179" }),
            cx,
        );
        let named = named.recv().unwrap();
        assert_eq!(
            (named.error, named.text.as_str()),
            (true, "This agent isn't connected.")
        );
        assert!(cards(&agents, cx).is_empty());
    }

    /// A title after a session ended, at its client's last connection closing or IDLE without a
    /// call, names the next session, at once; the one that ended keeps the title it had, on its
    /// card and its Library row.
    #[gpui::test]
    fn a_title_after_a_session_ended_names_the_next(cx: &mut TestAppContext) {
        let (agents, _) = at_work(cx);
        call_as(
            &agents,
            3,
            "title",
            json!({ "text": "Refund order 1179" }),
            cx,
        );
        answered(&agents, 2, cx);
        answered(&agents, 3, cx);
        bye(&agents, cx);
        hello(&agents, "claude-code", "claude-code", cx);
        let next = call_as(&agents, 4, "title", json!({ "text": "Pay invoice 88" }), cx);
        assert!(
            !next.recv().unwrap().error,
            "the next session's name waited"
        );
        let titles = |cx: &mut TestAppContext| -> Vec<(String, Dot)> {
            let shown = cards(&agents, cx);
            (shown.iter())
                .map(|card| (card.title.to_string(), card.dot))
                .collect()
        };
        let shown = titles(cx);
        assert!(
            shown.contains(&("Refund order 1179".into(), Dot::Done)),
            "{shown:?}"
        );
        assert!(
            shown.contains(&("Pay invoice 88".into(), Dot::Working)),
            "{shown:?}"
        );
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].1.title, "Refund order 1179");

        call_as(&agents, 5, "read_page", json!({ "tab": 20 }), cx);
        answered(&agents, 4, cx);
        answered(&agents, 5, cx);
        cx.executor().advance_clock(IDLE + Duration::from_secs(1));
        let next = call_as(&agents, 6, "title", json!({ "text": "Book a table" }), cx);
        assert!(!next.recv().unwrap().error);
        let shown = titles(cx);
        assert!(
            shown.contains(&("Pay invoice 88".into(), Dot::Done)),
            "{shown:?}"
        );
        assert!(
            shown.contains(&("Book a table".into(), Dot::Working)),
            "{shown:?}"
        );
        let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].1.title, "Pay invoice 88");
    }

    mod stopped;
}

/// An agent sorts its own Space's Today tabs by site, by the user's command's rule (#2344): its
/// pinned tab stays where it is, its split joins no site, the user's Space isn't touched, and it's one entry
/// in the log. Sorted already, it says so and nothing moves. A task has no such tool.
#[gpui::test]
fn an_agent_sorts_its_own_today_tabs_by_site(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    browser.tabs.borrow_mut().extend([
        (21, 2, "Wiki", "https://en.wikipedia.org/wiki/Rust"),
        (22, 2, "Docs", "https://docs.example.com/"),
        (23, 2, "Left", "https://news.ycombinator.com/"),
        (24, 2, "Right", "https://example.org/"),
        (25, 2, "More", "https://de.wikipedia.org/wiki/Arc"),
        (26, 1, "Theirs", "https://www.example.com/b"),
        (27, 2, "Pinned", "https://en.wikipedia.org/"),
    ]);
    browser.pinned.borrow_mut().insert(27);
    browser.splits.borrow_mut().push(vec![23, 24]);
    let sort = |cx: &mut TestAppContext| {
        call(&agents, "claude-code", "sort_tabs_by_site", json!({}), cx)
            .recv()
            .unwrap()
    };
    let in_space = |space: u64| -> Vec<u64> {
        (browser.tabs.borrow().iter())
            .filter(|tab| tab.1 == space)
            .map(|tab| tab.0)
            .collect()
    };
    let users = in_space(1);
    let answer = sort(cx);
    assert_eq!(
        answer,
        ToolResult::text(
            "Sorted your Today tabs by site, each site's tabs together at its first tab's place: \
             20, 22, 21, 25, 23."
        )
    );
    assert_eq!(in_space(2), [20, 22, 21, 25, 23, 24, 27]);
    assert_eq!(in_space(1), users, "the user's Space stays as it was");
    assert_eq!(
        sort(cx),
        ToolResult::text("Your Today tabs are together by site already.")
    );
    assert_eq!(in_space(2), [20, 22, 21, 25, 23, 24, 27]);
    let logged: Vec<_> = (log(&agents, "claude-code", cx).into_iter())
        .map(|(action, _, refused)| (action, refused))
        .collect();
    let sorted = ("sort_tabs_by_site".to_string(), String::new());
    assert_eq!(logged[..2], [sorted.clone(), sorted]);
    assert_eq!(
        tools::describe("sort_tabs_by_site"),
        "Sorted its tabs by site"
    );
    assert!(tools::names("claude-code").contains(&"sort_tabs_by_site".to_string()));
    assert!(!tools::names("task-1").contains(&"sort_tabs_by_site".to_string()));
}

mod fenced;
#[cfg(unix)]
mod helpers;
mod holds;
pub(crate) mod kept;
#[cfg(unix)]
mod lines;
mod notes;
mod other_apps;
mod permission_stopped;
#[cfg(unix)]
mod rewall;
#[cfg(unix)]
mod searches;
mod sign_ins;
mod switches;
mod uploads;
mod went_to;

/// An agent's http:// is never one the user typed (#2417): open_tab and navigate hand the core the
/// address as the agent wrote it through `open_tab` and `navigate`, bc_agent_open_tab's and
/// bc_agent_navigate's, which take no typed text, so the core never sets
/// url_typed_with_http_scheme for it and HTTPS-First still tries it over https first.
#[gpui::test]
fn an_agents_http_address_is_never_a_typed_one(cx: &mut TestAppContext) {
    let (agents, browser, _configs) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "http://plain.example.com/" }),
        cx,
    );
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
    let (tab, _, _, opened) = *browser.tabs.borrow().last().unwrap();
    assert_eq!((tab, opened), (30, "http://plain.example.com/"));

    let answer = call(
        &agents,
        "claude-code",
        "navigate",
        json!({ "url": "http://other.example.com/" }),
        cx,
    );
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
    assert_eq!(
        *browser.navigated.borrow(),
        [(30, "http://other.example.com/".to_string())]
    );
}

mod saved_reads {
    use super::*;
    use crate::agents::saved_read::{self, Answer};

    #[gpui::test]
    fn a_read_goes_to_the_core_within_its_caps_and_answers_once(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        let answered = Rc::new(RefCell::new(Vec::new()));
        let url = "https://www.example.test/inbox";
        let read = agents.update(cx, |agents, _| {
            let answered = answered.clone();
            agents.read(7, url, 1 << 30, 600_000, move |answer, _| {
                answered.borrow_mut().push(answer)
            })
        });
        assert_eq!(
            *browser.saved_reads.borrow(),
            [(
                7,
                read,
                url.to_string(),
                saved_read::MAX_BYTES,
                saved_read::MAX_MILLISECONDS
            )]
        );
        assert!(answered.borrow().is_empty());

        let answer = Answer {
            status: 200,
            url: url.into(),
            body: b"rows".to_vec(),
            ..Default::default()
        };
        agents.update(cx, |agents, cx| {
            agents.read_done(read, answer.clone(), cx);
            // A second answer, or one for a read nobody sent, reaches no one.
            agents.read_done(read, Answer::default(), cx);
            agents.read_done(read + 100, Answer::default(), cx);
        });
        assert_eq!(*answered.borrow(), [answer]);
    }

    #[gpui::test]
    fn a_read_within_the_caps_goes_as_asked(cx: &mut TestAppContext) {
        let (agents, browser, _) = connected(cx);
        let read = agents.update(cx, |agents, _| {
            agents.read(3, "https://www.example.test/", 1000, 200, |_, _| {})
        });
        assert_eq!(
            *browser.saved_reads.borrow(),
            [(3, read, "https://www.example.test/".to_string(), 1000, 200)]
        );
    }
}
