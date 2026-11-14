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
mod codex;
mod held;
pub mod page;
#[cfg(windows)]
mod pipe;
pub mod ready;
pub mod request;
pub mod runner;
pub mod scripts;
mod server;
pub mod task;
pub mod tools;

pub use server::address;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use web_time::{Instant, SystemTime};

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
    /// Whether it's pinned in its Space.
    pub pinned: bool,
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
    ScrollTo,
    Back,
    Forward,
    Screenshot,
    /// The tree once the page has settled, as after an action, without one.
    Quiet,
    Select,
}

/// What closing an agent's tab did (bc_agent_close_tab).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Closed {
    Closed,
    /// The tab isn't the agent's, or the user holds it.
    Refused,
    /// The user pinned it, and only they close it.
    Pinned,
    /// Its page may ask before it's left, as one with unsaved changes does.
    Asks,
}

/// What an agent asks the user's leave for, as the card in each window asks it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Question {
    pub id: u64,
    /// The agent's name, as its Space has it.
    pub agent: SharedString,
    /// The agent's id, by which a task's card asks it rather than the windows' card.
    pub from: SharedString,
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
    /// The changes a call's requests would make (request), the first `request::LISTED`, each
    /// what it does and where, then what it sends, or "". They ask every time, so their card has
    /// no Always or Never.
    pub changes: Vec<(SharedString, SharedString)>,
    /// The changes it doesn't list.
    pub more: usize,
}

/// What an agent is doing, and what the user holds from it, for the bar in its Space.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    /// The agent's id.
    pub agent: SharedString,
    /// Its name, as its card has it (Agents::agent_name).
    pub name: SharedString,
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
        self.space_tabs(space, cx)
            .into_iter()
            .map(|tab| tab.id)
            .collect()
    }
    /// The tabs of `agent`'s own Space, in `space_tabs`' order, as the core lists them
    /// (bc_agent_list_tabs): it finds the Space, so no other Space's tabs are ever listed.
    fn agent_tabs(&self, agent: &str, cx: &App) -> Vec<TabInfo>;
    /// `tab`'s title and URL.
    fn tab(&self, tab: u64, cx: &App) -> TabInfo;
    /// The Space that has `tab`, or 0 if none does: it closed, or is closing.
    fn space_of(&self, tab: u64, cx: &App) -> u64;
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
    /// Closes the agent's `tab` without a dialog (bc_agent_close_tab): a page that may ask before
    /// it's left stays open, unless `discard`, which loses what it holds unsaved.
    fn close_tab(&self, agent: &str, tab: u64, discard: bool, cx: &mut App) -> Closed;
    /// Closes `tab` as the user would (bc_tab_close): a Today tab goes to the archive. For what the
    /// user does to an agent's tabs, as dropping a done session's.
    fn close_user_tab(&self, tab: u64, cx: &mut App);
    /// Answers for the user about a download an agent's page started, which the core holds
    /// (bc_agent_download_answer): `keep` lets it go on, else it's cancelled and its file gone.
    fn answer_download(&self, download: u64, keep: bool);
    /// Pins `tab` in its Space at the page it's at, or unpins it to the top of its Today tabs, as
    /// the user does (bc_tab_set_pinned).
    fn set_pinned(&self, tab: u64, pinned: bool, cx: &mut App);
    /// Removes the pinned `tab` from its Space, as its row's remove button does: unpinned, then
    /// closed with its split, to the archive.
    fn remove_pinned(&self, tab: u64, cx: &mut App);
    /// The tabs shown with `tab`, left to right: its split view's, else `tab` alone (bc_tab_split).
    fn split(&self, tab: u64, cx: &App) -> Vec<u64>;
    /// Puts `tab` in a split view with `with`, at its right end, as the user's New Split View and
    /// Add to Split View do (bc_tab_split_with): false if `with`'s split has three tabs already.
    fn split_with(&self, tab: u64, with: u64, cx: &mut App) -> bool;
    /// Takes `tab` out of its split view, as the user's Separate does (bc_tab_separate).
    fn separate(&self, tab: u64, cx: &mut App);
    /// Mutes or unmutes `tab`'s sound (bc_tab_set_muted).
    fn set_muted(&self, tab: u64, muted: bool);
    /// Makes the hidden Space of the task agent `agent`, named `name`, in the profile of the
    /// user's Space `like` (bc_space_create_for_agent, bc_space_set_profile).
    fn make_task_space(&self, agent: &str, name: &str, like: u64, cx: &mut App) -> u64;
    /// Deletes a task's Space, with its tabs.
    fn delete_space(&self, space: u64, cx: &mut App);
    /// The Spaces of task agents, as a run before may have left them.
    fn task_spaces(&self) -> Vec<u64>;
    /// Moves `tab` out of a task's Space to the top of the user's Space `space`'s Today tabs.
    fn move_tab(&self, tab: u64, space: u64, cx: &mut App);
    /// The user's Space a session's pages open into: the one the front window shows, if it's in
    /// the profile of `like`, an agent's Space, else the first of the user's in that profile.
    fn user_space(&self, like: u64) -> u64;
    /// Sends `sent` from the page in the agent's `tab`, as the page's own script could, to its own
    /// site (bc_agent_request). Its answer comes to `Agents::request_done` with `request`.
    fn request(&self, agent: &str, tab: u64, request: u64, sent: &request::Request);
}

/// What the server's threads hand the UI thread.
pub enum Event {
    /// A client began a session as the agent with the id `agent`. `client` is the name it gave:
    /// its title, else its name, or empty.
    Hello {
        agent: String,
        client: String,
    },
    /// The agent's last session ended.
    Bye {
        agent: String,
    },
    Call(Call),
    /// The call `id` was answered, `at` then: with why it failed, if it did, and its picture.
    Answered {
        agent: String,
        id: u64,
        at: Instant,
        error: Option<String>,
        image: Option<String>,
    },
}

/// A tool call, which the server's thread waits on until `reply` has the answer.
pub struct Call {
    /// Its number, which its answer's event names.
    pub id: u64,
    pub agent: String,
    pub tool: String,
    pub arguments: Value,
    pub reply: mpsc::Sender<ToolResult>,
    /// When it came.
    pub at: Instant,
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
    /// The call it's for, whose session's card says it waits for the user's OK.
    call: u64,
    /// What the activity log names it, as `tools::describe` knows it.
    action: &'static str,
    /// The tab, or 0 for open_tab, which makes it.
    tab: u64,
    /// Where open_tab and navigate go.
    url: String,
    page: PageAction,
    element: i64,
    /// What it types, or a download's file name.
    text: String,
    /// What the answer says it did, before the outline.
    note: String,
    /// What of the outline the answer gives.
    show: Show,
    /// The field it chooses in, as its answer names it: "combobox “Status”".
    field: String,
    /// What it would do, as a question has it.
    what: String,
    /// The download it lets go on, for "download", else 0.
    download: u64,
    /// The HTTP requests it sends (request).
    requests: Vec<request::Request>,
    /// What it does that asked first, once allowed, for its session's receipt.
    did: Option<Did>,
    reply: mpsc::Sender<ToolResult>,
}

/// A download a page in an agent's Space started, which the core holds until the user answers
/// (agent_download).
pub struct Download {
    pub agent: String,
    pub tab: u64,
    /// The action that started it, which it answers, or 0.
    pub request: u64,
    pub id: u64,
    pub name: String,
    pub url: String,
    /// The tab has no page of its own, as one open_tab opened at the file's URL: the download is
    /// all it did.
    pub tab_empty: bool,
}

/// How a download an agent's page started ended (agent_download).
pub enum DownloadEnd {
    /// With its file's name, which may have changed to keep another, its size and where it is.
    Done {
        name: String,
        bytes: i64,
        path: String,
    },
    Failed,
    Cancelled,
}

/// A download the user let an agent's page make, until it ends.
struct Downloading {
    agent: String,
    name: String,
    reply: mpsc::Sender<ToolResult>,
}

/// What an action that asked first did once it was allowed and done, which its session's receipt
/// counts: "1 order placed", "2 messages sent".
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Did {
    Order,
    Message,
}

impl Did {
    fn of(why: &str) -> Option<Did> {
        match why {
            page::ORDER => Some(Did::Order),
            page::SENDS => Some(Did::Message),
            _ => None,
        }
    }
}

/// What of the page's outline an action answers with, as its `outline` argument says.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Show {
    /// What changed since the last outline the agent had of the tab, or all of it on a new page.
    Changes,
    Full,
    /// Which tab and page it is, alone.
    Nothing,
    /// The outline of the element with this DOM node alone: snapshot's ref.
    Element(i64),
}

impl Show {
    /// The `outline` argument: "changes", as without it, "full" or "none".
    fn argument(arguments: &Value) -> Result<Show, String> {
        match arguments.get("outline") {
            None | Some(Value::Null) => Ok(Show::Changes),
            Some(Value::String(show)) if show == "changes" => Ok(Show::Changes),
            Some(Value::String(show)) if show == "full" => Ok(Show::Full),
            Some(Value::String(show)) if show == "none" => Ok(Show::Nothing),
            Some(_) => Err("outline is changes, full or none.".into()),
        }
    }

    /// As a new page answers: the outline whole, unless none.
    fn whole(self) -> Show {
        match self {
            Show::Nothing => Show::Nothing,
            _ => Show::Full,
        }
    }
}

/// An action that waits on the core's answer.
struct Acting {
    agent: String,
    /// The call it's for, and what it does that asked first, which its session counts once done.
    call: u64,
    did: Option<Did>,
    tab: u64,
    /// The element it acts on, or 0.
    element: i64,
    note: String,
    show: Show,
    /// Whether the action opened the tab, which stays open if its page then fails, so the answer
    /// keeps the note that names it.
    opened: bool,
    field: String,
    reply: mpsc::Sender<ToolResult>,
    /// The tabs its page opened meanwhile, in the agent's Space, which its answer names.
    opened_tabs: Vec<u64>,
}

/// A call's requests, one at a time, while the one sent waits on the core's answer.
struct Sending {
    agent: String,
    tab: u64,
    sent: Option<request::Request>,
    /// The requests still to send.
    left: std::collections::VecDeque<request::Request>,
    /// What each answered.
    answers: Vec<String>,
    /// What's left of what the call's answers may show, and how many secrets they hid.
    shown: usize,
    hidden: usize,
    reply: mpsc::Sender<ToolResult>,
}

/// An action that waits on the user.
struct Asking {
    question: Question,
    site: String,
    act: Act,
    /// Refuses the action once ASK_WAIT passes with no answer; an answer drops it.
    _timer: Task<()>,
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
    /// What on the page, as a step's title says it, or empty: a request's method and path.
    pub target: SharedString,
    /// Why it was refused, or empty if it wasn't.
    pub refused: SharedString,
    /// A run's steps (run_steps), each an entry of its own.
    pub steps: Vec<Activity>,
    /// Who sent a script's command, by its app's name (scripts.rs), or empty for an agent's.
    pub by: SharedString,
    /// Its own, which Settings › Agents keeps a row opened by.
    pub id: u64,
    /// A request's whole address, or empty.
    pub url: SharedString,
    /// What a request sent, or would have, a line a field, without its secrets (#2326).
    pub sent: Vec<request::Sent>,
}

/// A new entry's id: the log's entries are made in more than one place.
pub fn entry_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// An agent the user connected.
#[derive(Clone, Debug)]
pub struct Agent {
    pub client: Client,
    token: String,
}

/// Whether a press of the X server's `button` in an agent's page takes it over: a click of any of
/// the mouse's buttons, 1 to 3 and the side ones, but never the wheel's 4 to 7, which scroll (#1080).
pub fn press_takes_over(button: u32) -> bool {
    !(4..=7).contains(&button)
}

/// The key a ⌘T task on `cli` keeps its answers for good under, the CLI's for its tasks, apart
/// from the app of the same name that connects, whose answers are its own: "claude-tasks".
pub fn task_answers(cli: runner::Cli) -> String {
    format!("{}-tasks", cli.id())
}

/// Whether `agent` is a ⌘T task's own id, task-N, which earlier builds kept answers under and no
/// task keeps for good.
fn is_task_id(agent: &str) -> bool {
    agent
        .strip_prefix("task-")
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
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
    /// The tabs agents opened, which one may close past its page's unsaved changes: in any other,
    /// they may be the user's.
    opened: HashSet<u64>,
    /// The actions waiting on the core, by request.
    acting: HashMap<u64, Acting>,
    /// The calls whose requests wait on the core, by the request sent.
    sending: HashMap<u64, Sending>,
    /// Each tab's refs, from its outlines.
    refs: HashMap<u64, page::Refs>,
    /// Each tab's latest outline, and the last one an agent had, which its next answer gives the
    /// changes since.
    outlines: HashMap<u64, page::Outline>,
    had: HashMap<u64, (String, page::Outline)>,
    /// The actions waiting on the user, the first asked now.
    asking: Vec<Asking>,
    /// The downloads the user let agents' pages make, by download.
    downloading: HashMap<u64, Downloading>,
    /// How the downloads that ended while their question waited ended, by download: a small file
    /// can come whole before the core's pause holds it, and its yes then answers at once.
    ended: HashMap<u64, DownloadEnd>,
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
    /// The tasks the user asked from ⌘T, and their CLIs' settings.
    tasks: task::Tasks,
    /// The tabs agents pinned, by tab, with the agent that pinned each, which alone may unpin or
    /// remove it. Every other pinned tab is the user's, as is one whose pin the user changes.
    pins: HashMap<u64, String>,
    /// Whether scripts may control the browser (scripts.rs), and the Apple Event logged last.
    scripts: bool,
    script_event: u64,
    /// Settings › Agents' Tasks from the command bar is off: asking an agent from ⌘T and letting
    /// an app connect are two trust decisions, so tasks have a switch of their own under agents'.
    tasks_off: bool,
}

/// A run of steps (run_steps): one at a time, each as its tool would run alone.
struct Run {
    agent: String,
    /// The call it answers.
    call: u64,
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
    /// What of the outline the run's answer gives, after its last step.
    show: Show,
}

impl Run {
    /// What of the outline the step under way answers with: the run's last, as the run asks, and
    /// those before it none, as the agent gets only what they did.
    fn show_now(&self) -> Show {
        match self.steps.is_empty() {
            true => self.show,
            false => Show::Nothing,
        }
    }
}

/// A run's wait step: it looks at the page until what it waits for comes or its time is up.
struct Wait {
    id: u64,
    what: WaitFor,
    seconds: f64,
    until: Instant,
    /// The next look's answer, for the timer to give the core.
    look: Option<mpsc::Sender<ToolResult>>,
    /// The next look, which goes with the run, as a run stopped or gone looks no more.
    timer: Option<Task<()>>,
}

/// What a wait step waits for.
enum WaitFor {
    /// Its seconds, then it looks once.
    Time,
    /// Text to show in the page's outline.
    Shows(String),
    /// Text to go from it.
    Goes(String),
    /// The page to settle, as an action's answer waits for it.
    Quiet,
}

/// How often a wait looks at the page for its text, in a run under way: no more than the page's
/// own settling after an action takes.
const LOOK_EVERY: Duration = Duration::from_millis(500);

/// What an answer starts with when the core stopped waiting for its page to settle.
const BUSY: &str = "The page was still busy after 5 s, so it may change yet.";
/// What it starts with when only requests the page made were still open then, as a page that polls
/// keeps one, while what it shows held still: no reason to wait for it (#2355).
const REQUESTS: &str = "The page held still, with requests it made still open after 5 s.";

/// A tab the user holds from its agent: handed to them, or taken over.
struct Held {
    agent: String,
    /// The handoff's or ask_for_tab's call, whose session's card says it waits for the user, or 0.
    call: u64,
    /// What the agent asked the user to do, or empty if they took it over.
    why: String,
    /// The handoff's question, or the agent's asking for the tab back, for its card.
    question: u64,
    /// Why the agent asks for the tab back (ask_for_tab), while its call waits for the answer.
    back: Option<String>,
    /// The handoff's or ask_for_tab's call, which waits for the tab to come back, while its
    /// agent's session lasts.
    reply: Option<mpsc::Sender<ToolResult>>,
    /// Answers a handoff's call once HANDOFF_WAIT passes with the tab still the user's.
    timer: Option<Task<()>>,
}

impl Held {
    /// Where the tab is between the user and its agent (held::Life), from what waits on it.
    fn life(&self) -> held::Life {
        match (&self.reply, &self.back) {
            (None, _) => held::Life::Held,
            (Some(_), None) => held::Life::HandedOff,
            (Some(_), Some(_)) => held::Life::Asked,
        }
    }
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
        let dir = path
            .as_ref()
            .and_then(|path| path.parent())
            .map(PathBuf::from);
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
            opened: HashSet::new(),
            acting: HashMap::new(),
            sending: HashMap::new(),
            refs: HashMap::new(),
            outlines: HashMap::new(),
            had: HashMap::new(),
            asking: Vec::new(),
            downloading: HashMap::new(),
            ended: HashMap::new(),
            grants: HashMap::new(),
            doing: HashMap::new(),
            pins: HashMap::new(),
            held: HashMap::new(),
            runs: Vec::new(),
            stepping: None,
            tasks: task::Tasks {
                dir,
                ..Default::default()
            },
            scripts: false,
            script_event: 0,
            tasks_off: false,
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
        let grants = saved
            .get("grants")
            .and_then(Value::as_object)
            .into_iter()
            .flatten();
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
        .filter(|(agent, _)| !is_task_id(agent))
        .collect();
        let tasks = saved.get("tasks").unwrap_or(&Value::Null);
        self.tasks.default = tasks
            .get("default")
            .and_then(Value::as_str)
            .and_then(runner::Cli::from_id);
        let models = tasks
            .get("models")
            .and_then(Value::as_object)
            .into_iter()
            .flatten();
        self.tasks.models = (models.filter_map(|(cli, model)| {
            Some((runner::Cli::from_id(cli)?, model.as_str()?.to_string()))
        }))
        .collect();
        let found = tasks
            .get("found")
            .and_then(Value::as_object)
            .into_iter()
            .flatten();
        self.tasks.found = (found.filter_map(|(cli, found)| {
            Some((runner::Cli::from_id(cli)?, ready::Found::from_saved(found)))
        }))
        .collect();
        self.tasks_off = tasks.get("on").and_then(Value::as_bool) == Some(false);
        self.on = saved.get("on").and_then(Value::as_bool).unwrap_or(false);
        self.scripts = saved
            .get("scripts")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if self.on {
            self.start(cx);
        }
        cx.notify();
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Whether the user may ask an agent from ⌘T: agents are on, and so is Settings › Agents' Tasks
    /// from the command bar, which is on unless they turned it off. Outside agents, through the
    /// server, don't hang on it.
    pub fn tasks_on(&self) -> bool {
        self.on && !self.tasks_off
    }

    /// Settings › Agents' Tasks from the command bar. Off, ⇥ makes no task and shows no hint, no
    /// task starts, and a task's card offers no Retry or reply; the tasks running finish, and
    /// their cards stay until seen.
    pub fn set_tasks_on(&mut self, on: bool, cx: &mut Context<Self>) {
        if on != self.tasks_off {
            return;
        }
        self.tasks_off = !on;
        self.save();
        cx.notify();
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

    /// What the "Any MCP client" row says before it's connected. `missing` are the clients it
    /// looked for and didn't find. A Flatpak's sandbox can't see the host's, so there it says so
    /// instead of claiming they're absent, and points to Copy Setup, since reaching the host's
    /// commands would defeat the sandbox.
    pub fn other_client_detail(&self, missing: &[&str]) -> String {
        if self.bridge.first().map(String::as_str) == Some("flatpak") {
            return "Any MCP client. ilo's sandbox can't see Claude Code or Codex on this computer, so Copy Setup gives their setup to paste in.".into();
        }
        match missing {
            [] => "Any MCP client".into(),
            [one] => format!("Any MCP client. {one} isn't on this computer."),
            _ => format!(
                "Any MCP client. {} aren't on this computer.",
                missing.join(" and ")
            ),
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
        let found: Vec<Client> = (Client::ALL.into_iter())
            .filter(|client| self.clients.found(*client))
            .collect();
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
                *client == Client::Other
                    || self.found.contains(client)
                    || self.is_connected(*client)
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
        let (life, actions) = self.held_step(tab, held::Event::Passkey);
        if why == "passkey" && !self.held.contains_key(&tab) && life == held::Life::Held {
            // check.sh reads this line.
            log!("browser: agent {agent} hands off {tab}: passkey");
            let question = self.next_request;
            self.next_request += 1;
            let why = PASSKEY.to_string();
            let held = Held {
                agent: agent.into(),
                call: 0,
                why,
                question,
                back: None,
                reply: None,
                timer: None,
            };
            self.held.insert(tab, held);
            self.carry(tab, held::Event::Passkey, life, actions, cx);
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
                self.hello(&agent, &client);
                if self.live.insert(agent.clone()) {
                    self.note(&agent, "connected", "", "");
                    cx.notify();
                }
            }
            Event::Bye { agent } => {
                self.end_session(&agent, cx);
                if self.live.remove(&agent) {
                    self.forget_waiting(&agent, cx);
                    self.browser.disconnected(&agent, cx);
                    self.note(&agent, "disconnected", "", "");
                    cx.notify();
                }
            }
            Event::Call(call) => self.call(call, cx),
            Event::Answered {
                agent,
                id,
                at,
                error,
                image,
            } => self.answered(&agent, id, at, error, image, cx),
        }
    }

    fn call(&mut self, call: Call, cx: &mut Context<Self>) {
        // A token's agent is always one the user connected, or a task of theirs.
        if !self.may_call(&call.agent) {
            _ = call
                .reply
                .send(ToolResult::error("This agent isn't connected."));
            return;
        }
        self.trace_call(&call, cx);
        let Call {
            id,
            agent,
            tool,
            arguments,
            reply,
            ..
        } = call;
        // check.sh reads this line.
        log!("browser: agent {agent} calls {tool}");
        self.live.insert(agent.clone());
        match tool.as_str() {
            "list_tabs" => {
                // Its first call makes its Space, which the core then lists.
                self.space(&agent, cx);
                let tabs = self.browser.agent_tabs(&agent, cx);
                self.note(&agent, "list_tabs", "", "");
                cx.notify();
                let mine: Vec<u64> = (self.pins.iter())
                    .filter(|(_, by)| **by == agent)
                    .map(|(&tab, _)| tab)
                    .collect();
                let current = self.current.get(&agent).copied();
                _ = reply.send(ToolResult::text(tools::tab_list(&tabs, current, &mine)));
            }
            "read_page" => {
                let tab = match tools::tab_argument(&arguments) {
                    Ok(Some(tab)) => tab,
                    Ok(None) => match self.default_tab(&agent, cx) {
                        Some(tab) => tab,
                        None => {
                            _ = reply.send(ToolResult::text(tools::tab_list(&[], None, &[])));
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
                    call: id,
                    action: "open_tab",
                    tab: 0,
                    what: format!("open {host}"),
                    did: None,
                    url,
                    page: PageAction::Quiet,
                    element: 0,
                    text: String::new(),
                    note: String::new(),
                    show: Show::Full,
                    download: 0,
                    field: String::new(),
                    requests: Vec::new(),
                    reply,
                };
                if !is_web(&act.url) {
                    return self.refuse(act, &host, WEB_ONLY, cx);
                }
                let risk = page::opening_risk(&host);
                self.check(act, host, risk, cx);
            }
            "navigate" | "snapshot" | "click" | "type" | "select" | "press_key" | "scroll"
            | "screenshot" => {
                let tab = match tools::tab_argument(&arguments) {
                    Ok(Some(tab)) => tab,
                    Ok(None) => match self.default_tab(&agent, cx) {
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
                let show = match Show::argument(&arguments) {
                    Ok(show) => show,
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                self.page_tool(&agent, id, &tool, tab, &arguments, show, reply, cx);
            }
            "run_steps" => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
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
                let show = match Show::argument(&arguments) {
                    Ok(show) => show,
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                let host = site(&self.browser.tab(tab, cx).url);
                self.note(&agent, "run_steps", &host, "");
                let (_, step) = mpsc::channel();
                let (steps, done, wait) = (steps.into(), Vec::new(), None);
                let run = Run {
                    agent,
                    call: id,
                    tab,
                    steps,
                    done,
                    step,
                    reply,
                    wait,
                    show,
                };
                self.next_step(run, cx);
                self.advance(cx);
            }
            "hand_off" => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
                    return;
                };
                let why = arguments
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if why.is_empty() {
                    _ = reply.send(ToolResult::error(
                        "reason is what the user is to do, in a short sentence.",
                    ));
                    return;
                }
                if !self.act(&agent, tab, "hand_off", &reply, cx) {
                    return;
                }
                let (life, actions) = self.held_step(tab, held::Event::HandOff);
                if !self.browser.hand_off(&agent, tab) || life != held::Life::HandedOff {
                    _ = reply.send(ToolResult::error("The tab is the user's already."));
                    return;
                }
                // check.sh reads this line.
                log!("browser: agent {agent} hands off {tab}: {why}");
                let question = self.next_request;
                self.next_request += 1;
                let why = page::clip(why, 200);
                let held = Held {
                    agent,
                    call: id,
                    why,
                    question,
                    back: None,
                    reply: Some(reply),
                    timer: None,
                };
                self.held.insert(tab, held);
                self.carry(tab, held::Event::HandOff, life, actions, cx);
            }
            "pin_tab" | "unpin_tab" | "remove_tab" if !is_task(&agent) => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
                    return;
                };
                self.pin(&agent, &tool, tab, reply, cx);
            }
            "close_tab" => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
                    return;
                };
                let discard = arguments.get("discard").and_then(Value::as_bool) == Some(true);
                self.close_tab(&agent, id, tab, discard, reply, cx);
            }
            // Split views, through the core calls the user's own take, each tab checked first.
            "split_tabs" => {
                let tabs = arguments.get("tabs").and_then(Value::as_array);
                let tabs: Option<Vec<u64>> =
                    tabs.and_then(|tabs| tabs.iter().map(tools::tab_id).collect());
                let tabs = tabs.filter(|tabs| {
                    let distinct = tabs.iter().collect::<HashSet<_>>().len();
                    (2..=tools::SPLIT_TABS).contains(&tabs.len()) && distinct == tabs.len()
                });
                let Some(tabs) = tabs else {
                    _ = reply.send(ToolResult::error(
                        "tabs is two or three different tabs' ids, from list_tabs or open_tab.",
                    ));
                    return;
                };
                for &tab in &tabs {
                    if !self.act(&agent, tab, "split_tabs", &reply, cx) {
                        return;
                    }
                }
                let first = tabs[0];
                if let Some(&full) =
                    (tabs[1..].iter()).find(|&&tab| !self.browser.split_with(tab, first, cx))
                {
                    _ = reply.send(ToolResult::error(format!(
                        "Tab {full} couldn't join tab {first}: a split view holds {} tabs at most.",
                        tools::SPLIT_TABS
                    )));
                    return;
                }
                let split = self.browser.split(first, cx);
                _ = reply.send(ToolResult::text(tools::split_list(first, &split)));
            }
            "add_to_split" => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
                    return;
                };
                let Some(split) = arguments.get("split").and_then(tools::tab_id) else {
                    _ = reply.send(ToolResult::error(
                        "split is a tab of the split view, a number from list_tabs.",
                    ));
                    return;
                };
                if !self.act(&agent, split, "add_to_split", &reply, cx)
                    || !self.act(&agent, tab, "add_to_split", &reply, cx)
                {
                    return;
                }
                if self.browser.split(split, cx).len() < 2 {
                    _ = reply.send(ToolResult::error(format!(
                        "Tab {split} is in no split view. Make one with split_tabs."
                    )));
                    return;
                }
                if !self.browser.split(split, cx).contains(&tab)
                    && !self.browser.split_with(tab, split, cx)
                {
                    _ = reply.send(ToolResult::error(format!(
                        "Tab {split}'s split view holds {} tabs already.",
                        tools::SPLIT_TABS
                    )));
                    return;
                }
                let split = self.browser.split(tab, cx);
                _ = reply.send(ToolResult::text(tools::split_list(tab, &split)));
            }
            "separate_tab" => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
                    return;
                };
                if !self.act(&agent, tab, "separate_tab", &reply, cx) {
                    return;
                }
                if self.browser.split(tab, cx).len() < 2 {
                    _ = reply.send(ToolResult::error(format!("Tab {tab} is in no split view.")));
                    return;
                }
                self.browser.separate(tab, cx);
                _ = reply.send(ToolResult::text(format!(
                    "Tab {tab} is out of its split view."
                )));
            }
            "list_split" => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
                    return;
                };
                if !self.act(&agent, tab, "list_split", &reply, cx) {
                    return;
                }
                let split = self.browser.split(tab, cx);
                _ = reply.send(ToolResult::text(tools::split_list(tab, &split)));
            }
            "ask_for_tab" => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
                    return;
                };
                let why = arguments
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if why.is_empty() {
                    _ = reply.send(ToolResult::error(
                        "reason is why you need the tab, in a short sentence.",
                    ));
                    return;
                }
                let site = site(&self.browser.tab(tab, cx).url);
                let holder = self.held.get(&tab).is_some_and(|held| held.agent == agent);
                let (life, actions) = self.held_step(tab, held::Event::Ask { holder });
                let refused = if actions.has(held::Actions::NOT_HELD) {
                    NOT_HELD
                } else if actions.has(held::Actions::WAITING) {
                    "You're waiting on the user for this tab already."
                } else {
                    ""
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
                (held.call, held.reply) = (id, Some(reply));
                self.carry(tab, held::Event::Ask { holder }, life, actions, cx);
            }
            "request" => {
                let Some(tab) = self.tab_for(&agent, &arguments, &reply, cx) else {
                    return;
                };
                let host = site(&self.browser.tab(tab, cx).url);
                let requests = match request::parse(&arguments, &self.browser.tab(tab, cx).url) {
                    Ok(requests) => requests,
                    Err(request::Wrong::Refused(why)) => {
                        self.note(&agent, "request", &host, &why);
                        cx.notify();
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                    Err(request::Wrong::Malformed(why)) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                let what = match requests.iter().filter(|each| each.changes()).count() {
                    0 => String::new(),
                    1 => format!("make a change on {host}"),
                    changes => format!("make {changes} changes on {host}"),
                };
                let act = Act {
                    agent,
                    call: id,
                    action: "request",
                    tab,
                    url: String::new(),
                    page: PageAction::Snapshot,
                    element: 0,
                    text: String::new(),
                    note: String::new(),
                    show: Show::Nothing,
                    what,
                    field: String::new(),
                    download: 0,
                    requests,
                    did: None,
                    reply,
                };
                // Reads go; changes ask every time, never a grant's to answer.
                match act.what.is_empty() {
                    true => self.run(act, cx),
                    false => self.ask_first(act, host, CHANGES_ASK, cx),
                }
            }
            "title" | "ask" | "finish" if is_task(&agent) => {
                self.task_tool(&agent, &tool, &arguments, reply, cx)
            }
            _ => _ = reply.send(ToolResult::error(format!("There's no tool {tool}."))),
        }
    }

    /// Whether the agent may call tools: a client the user connected, or a task not dropped.
    fn may_call(&self, agent: &str) -> bool {
        match Client::from_id(agent) {
            Some(client) => self.is_connected(client),
            None => is_task(agent) && self.task(agent).is_some(),
        }
    }

    /// The agent's name, as the user sees it wherever it shows: its card, the strip under its page,
    /// its pointer, its questions and its activity in Settings (#1080). A task's CLI's; an outside
    /// client's session under way's, which its card has; else the name the client gave last, or for
    /// one that gave none, or Claude Code or Codex, which the user connected by name, its kind's.
    pub fn agent_name(&self, agent: &str) -> SharedString {
        if let Some(task) = self.task(agent) {
            return task.name.clone();
        }
        let session = (self.tasks.tasks.iter()).find(|task| {
            task.state == task::State::Working
                && (task.session.as_ref()).is_some_and(|session| session.agent == agent)
        });
        match session {
            Some(task) => task.name.clone(),
            None => (self.tasks.names.get(agent).cloned())
                .unwrap_or_else(|| name(agent).to_string().into()),
        }
    }

    /// Starts the run's next step, which answers it through `advance`, or answers the run if the
    /// step asks the user first, which a run doesn't wait for.
    fn next_step(&mut self, mut run: Run, cx: &mut Context<Self>) {
        let Some(step) = run.steps.pop_front() else {
            return;
        };
        let tool = step
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let (reply, answer) = mpsc::channel();
        run.step = answer;
        self.trace_step(&run.agent, run.call, run.tab, &step, cx);
        if tool == "wait" {
            return self.wait(run, &step, reply, cx);
        }
        let asked = self.asking.len();
        self.stepping = Some(run.agent.clone());
        let show = run.show_now();
        self.page_tool(
            &run.agent.clone(),
            run.call,
            &tool,
            run.tab,
            &step,
            show,
            reply,
            cx,
        );
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
        let string = |name| {
            step.get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let what = match (string("text"), string("gone"), string("until").as_str()) {
            (text, gone, "") if text.is_empty() && gone.is_empty() => Some(WaitFor::Time),
            (text, gone, "") if gone.is_empty() => Some(WaitFor::Shows(text)),
            (text, gone, "") if text.is_empty() => Some(WaitFor::Goes(gone)),
            (text, gone, "quiet") if text.is_empty() && gone.is_empty() => Some(WaitFor::Quiet),
            _ => None,
        };
        let seconds = match step.get("seconds").and_then(Value::as_f64) {
            None if !matches!(what, Some(WaitFor::Time)) => Some(tools::WAIT),
            seconds => seconds.filter(|seconds| *seconds > 0.0 && *seconds <= tools::MAX_WAIT),
        };
        let (Some(what), Some(seconds)) = (what, seconds) else {
            _ = reply.send(ToolResult::error(format!(
                "wait needs text to wait for, gone with text to wait to go, until \"quiet\" to \
                 wait for the page to settle, or seconds, at most {}.",
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
            let mut wait = Wait {
                id,
                what,
                seconds,
                until,
                look: None,
                timer: None,
            };
            match wait.what {
                WaitFor::Time => {
                    (wait.look, wait.timer) = (Some(reply), Some(self.look_after(id, time, cx)))
                }
                WaitFor::Quiet => self.look(&run.agent, run.tab, PageAction::Quiet, reply),
                _ => self.look(&run.agent, run.tab, PageAction::Snapshot, reply),
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
        let (agent, tab, seconds) = (run.agent.clone(), run.tab, wait.seconds);
        let show = run.show_now();
        let outline = self
            .outlines
            .get(&tab)
            .map(page::Outline::text)
            .unwrap_or_default();
        let now = cx.background_executor().now();
        let late = now >= wait.until;
        // The core waits for the page to settle itself, so a quiet wait looks again at once.
        let mut next = LOOK_EVERY.min(wait.until.saturating_duration_since(now));
        match &wait.what {
            WaitFor::Time => {
                let note = format!("Waited {seconds} s.");
                return Some(ToolResult::text(self.answer_with(&agent, tab, note, show)));
            }
            WaitFor::Quiet => match answer.text.starts_with(BUSY) {
                // What it shows held still, though a poll's request may never end.
                false if answer.text.starts_with(REQUESTS) => {
                    let note = REQUESTS.to_string();
                    return Some(ToolResult::text(self.answer_with(&agent, tab, note, show)));
                }
                false => {
                    let note = "The page settled.".to_string();
                    return Some(ToolResult::text(self.answer_with(&agent, tab, note, show)));
                }
                // A page that never settles, as one that polls, is no reason to stop the run.
                true if late => {
                    let note = format!("The page was still busy after {seconds} s.");
                    return Some(ToolResult::text(self.answer_with(&agent, tab, note, show)));
                }
                true => next = Duration::ZERO,
            },
            WaitFor::Shows(text) | WaitFor::Goes(text) => {
                let shows = outline.to_lowercase().contains(&text.to_lowercase());
                let goes = matches!(wait.what, WaitFor::Goes(_));
                let text = page::clip(text, 60);
                let said = match (goes, shows) {
                    (false, true) => Some((format!("“{text}” showed."), false)),
                    (true, false) => Some((format!("“{text}” went."), false)),
                    (false, false) if late => {
                        Some((format!("“{text}” didn't show in {seconds} s."), true))
                    }
                    (true, true) if late => {
                        Some((format!("“{text}” was still there after {seconds} s."), true))
                    }
                    _ => None,
                };
                match said {
                    // The run ends here, so with the outline as its last step's.
                    Some((why, true)) => {
                        let show = run.show;
                        return Some(ToolResult::error(self.answer_with(&agent, tab, why, show)));
                    }
                    Some((note, false)) => {
                        return Some(ToolResult::text(self.answer_with(&agent, tab, note, show)));
                    }
                    None => {}
                }
            }
        }
        let (look, answer) = mpsc::channel();
        run.step = answer;
        wait.look = Some(look);
        wait.timer = Some(self.look_after(wait.id, next, cx));
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
                    let action = match wait.what {
                        WaitFor::Quiet => PageAction::Quiet,
                        _ => PageAction::Snapshot,
                    };
                    Some((run.agent.clone(), run.tab, action, wait.look.take()?))
                });
                if let Some((agent, tab, action, look)) = waiting {
                    agents.look(&agent, tab, action, look);
                }
            });
        })
    }

    /// Asks the core for the page's tree, at once or once it has settled, for a wait, which
    /// checked and logged itself.
    fn look(&mut self, agent: &str, tab: u64, action: PageAction, reply: mpsc::Sender<ToolResult>) {
        let request = self.next_request;
        self.next_request += 1;
        let (note, show) = (String::new(), Show::Nothing);
        let acting = Acting {
            agent: agent.into(),
            call: 0,
            did: None,
            tab,
            element: 0,
            note,
            show,
            opened: false,
            field: String::new(),
            reply,
            opened_tabs: vec![],
        };
        self.acting.insert(request, acting);
        self.act_in_page(agent, tab, request, action, 0, "");
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
        self.browser
            .page(agent, tab, request, action, element, text);
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
                        Some(wait) if !answer.error => {
                            match self.waited(&mut run, wait, answer, cx) {
                                Some(answer) => answer,
                                None => {
                                    self.runs.push(run);
                                    continue;
                                }
                            }
                        }
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
        agent: &str,
        arguments: &Value,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let why = match tools::tab_argument(arguments) {
            Ok(Some(tab)) => return Some(tab),
            Ok(None) => match self.default_tab(agent, cx) {
                Some(tab) => return Some(tab),
                None => "You have no tab open yet. Open one with open_tab.".into(),
            },
            Err(why) => why,
        };
        _ = reply.send(ToolResult::error(why));
        None
    }

    /// The user takes the tab the agent is at work in over: the agent may not act in it until
    /// they hand it back. A press in its page does, as Take Over did (Shell::page_pressed).
    pub fn take_over(&mut self, agent: &str, tab: u64, cx: &mut Context<Self>) {
        let (life, actions) = self.held_step(tab, held::Event::TakeOver);
        if !actions.has(held::Actions::TAKE_BACK) {
            return;
        }
        // check.sh reads this line.
        log!("browser: agent {agent} taken over in {tab}");
        let question = self.next_request;
        self.next_request += 1;
        let why = String::new();
        let held = Held {
            agent: agent.into(),
            call: 0,
            why,
            question,
            back: None,
            reply: None,
            timer: None,
        };
        self.held.insert(tab, held);
        // The core ends the agent's session there, which logs it.
        self.carry(tab, held::Event::TakeOver, life, actions, cx);
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
        self.let_go(tab, held::Event::HandBack, cx);
        cx.notify();
    }

    /// The core closed `tab` (tabs::tab_closed). A hold on it ends.
    pub fn tab_closed(&mut self, tab: u64, cx: &mut Context<Self>) {
        self.let_go(tab, held::Event::Closed, cx);
    }

    /// `space`'s tabs changed (spaces::space_changed). A held tab that left its agent's Space
    /// closed, or the user moved it to one of theirs, and its hold ends either way.
    pub fn space_changed(&mut self, space: u64, cx: &mut Context<Self>) {
        if self.held.is_empty() {
            return;
        }
        let tabs = self.browser.space_tab_ids(space, cx);
        let left: Vec<u64> = (self.held.iter())
            .filter(|(tab, held)| {
                !tabs.contains(tab) && self.browser.agent_space(&held.agent) == space
            })
            .map(|(&tab, _)| tab)
            .collect();
        for tab in left {
            // Closing posts space_changed before the tab is gone if its page runs unload
            // handlers, so where it is, not whether it's gone yet, says which.
            let event = match self.browser.space_of(tab, cx) {
                0 => held::Event::Closed,
                _ => held::Event::Moved,
            };
            self.let_go(tab, event, cx);
        }
    }

    /// What held::step says `event` does to `tab`'s hold: the life it goes to, and the actions.
    fn held_step(&self, tab: u64, event: held::Event) -> (held::Life, held::Actions) {
        let life = self.held.get(&tab).map_or(held::Life::Free, Held::life);
        held::step(life, event)
    }

    /// Steps `tab`'s hold by `event`, one that needs nothing more of its caller, and carries it
    /// out.
    fn let_go(&mut self, tab: u64, event: held::Event, cx: &mut Context<Self>) {
        let (life, actions) = self.held_step(tab, event);
        self.carry(tab, event, life, actions, cx);
    }

    /// Carries out what held::step returned for `tab`: `life` is the life it goes to. A hold that
    /// begins has its entry by now, made by the caller with what the event brought.
    fn carry(
        &mut self,
        tab: u64,
        event: held::Event,
        life: held::Life,
        actions: held::Actions,
        cx: &mut Context<Self>,
    ) {
        use held::Actions as A;
        let agent = (self.held.get(&tab)).map_or(String::new(), |held| held.agent.clone());
        if actions.has(A::ANSWER) || actions.has(A::DROP) {
            let held = self
                .held
                .get_mut(&tab)
                .expect("a call waits on it, so it's held");
            let reply = held.reply.take();
            (held.back, held.timer) = (None, None);
            if let Some(reply) = reply
                && actions.has(A::ANSWER)
            {
                _ = reply.send(answer_of(tab, event));
            }
        }
        if life == held::Life::Free {
            self.held.remove(&tab);
        }
        if actions.has(A::TIMER) {
            let question = self.held.get(&tab).map_or(0, |held| held.question);
            let timer = self.handoff_timer(tab, question, cx);
            if let Some(held) = self.held.get_mut(&tab) {
                held.timer = Some(timer);
            }
        }
        if actions.has(A::TAKE_BACK) {
            self.browser.take_back(tab, cx);
        }
        if actions.has(A::HAND_BACK) {
            self.browser.hand_back(tab);
        }
        if actions.has(A::NOTE) {
            // check.sh reads the first two.
            match event {
                held::Event::HandBack => log!("browser: agent {agent} handed back {tab}"),
                held::Event::Timer => {
                    log!("browser: agent {agent} handoff {tab} with no hand back")
                }
                _ => log!("browser: agent {agent} held {tab} {}", event.way()),
            }
            let site = site(&self.browser.tab(tab, cx).url);
            self.note(&agent, event.way(), &site, "");
        }
        if actions.has(A::CARD) {
            self.asked(cx);
        }
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
        for asking in self
            .asking
            .extract_if(.., |asking| asking.act.agent == agent)
        {
            _ = asking.act.reply.send(stopped());
        }
        for tab in self.held_from(agent) {
            self.let_go(tab, held::Event::Stop { holder: true }, cx);
        }
        self.browser.disconnected(agent, cx);
        // An outside client stopped stops for good: its connections close once the calls stopped
        // have their answer, so it shows the browser's server as gone until the user reconnects
        // it, a session of its own (#1080). ⌘. from its page stops its card as the card's Stop does.
        match Client::from_id(agent) {
            Some(_) => {
                // check.sh reads this line.
                log!("browser: agent {agent} stopped by the user");
                if let Some((server, _)) = &self.server {
                    server.stop(agent);
                }
                self.note(agent, "stopped by the user", "", "");
                self.session_stopped(agent);
            }
            None => self.note(agent, "stopped", "", ""),
        }
        self.advance(cx);
        self.asked(cx);
    }

    /// The tabs the user holds from `agent`.
    fn held_from(&self, agent: &str) -> Vec<u64> {
        let held = self.held.iter().filter(|(_, held)| held.agent == agent);
        held.map(|(&tab, _)| tab).collect()
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
                    name: self.agent_name(agent),
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
        call: u64,
        tool: &str,
        tab: u64,
        arguments: &Value,
        show: Show,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let host = site(&self.browser.tab(tab, cx).url);
        let string = |name: &str| {
            arguments
                .get(name)
                .and_then(Value::as_str)
                .map(str::to_string)
        };
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
            call,
            action: "snapshot",
            tab,
            url: String::new(),
            page: PageAction::Snapshot,
            element: element.as_ref().map_or(0, |element| element.node),
            text: String::new(),
            note: String::new(),
            show,
            field: String::new(),
            what: String::new(),
            download: 0,
            requests: Vec::new(),
            did: None,
            reply,
        };
        let mut risk = None;
        match tool {
            "navigate" => {
                act.show = show.whole();
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
                        act.page = PageAction::Quiet;
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
            // The core outlines the whole page, which the answer cuts to the element's part.
            "snapshot" => {
                act.show = match (show, &element) {
                    (Show::Nothing, _) => Show::Nothing,
                    (_, Some(element)) => Show::Element(element.node),
                    (_, None) => Show::Full,
                };
                act.element = 0;
            }
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
                    _ = act
                        .reply
                        .send(ToolResult::error("type needs a ref and text."));
                    return;
                };
                act.action = "type";
                if element.password {
                    return self.refuse(act, &host, PASSWORDS, cx);
                }
                let submit = arguments
                    .get("submit")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                act.page = if submit {
                    PageAction::TypeSubmit
                } else {
                    PageAction::Type
                };
                act.note = format!(
                    "Typed into {}{}.",
                    label(element),
                    if submit { " and pressed Enter" } else { "" }
                );
                act.what = format!("send “{}” on {host}", page::clip(&text, 60));
                act.text = text;
                risk = submit.then(|| page::risk(&host, "send")).flatten();
            }
            "select" => {
                let option = string("option").unwrap_or_default().trim().to_string();
                let Some(element) = element.as_ref().filter(|_| !option.is_empty()) else {
                    _ = act
                        .reply
                        .send(ToolResult::error("select needs a ref and an option."));
                    return;
                };
                act.action = "select_option";
                act.page = PageAction::Select;
                act.field = match element.name.is_empty() {
                    true => element.role.clone(),
                    false => format!("{} {}", element.role, label(element)),
                };
                let chose = format!("“{}”", page::clip(&option, 60));
                act.note = format!("Chose {chose} in the {}.", act.field);
                act.what = format!("choose {chose} in {} on {host}", label(element));
                // As a click on the field or the option would.
                risk = page::risk(&host, &element.name).or_else(|| page::risk(&host, &option));
                act.text = option;
            }
            "press_key" => {
                let key = string("key").unwrap_or_default();
                if !page::KEYS.contains(&key.as_str()) {
                    let keys = page::KEYS.join(", ");
                    _ = act
                        .reply
                        .send(ToolResult::error(format!("key is one of {keys}.")));
                    return;
                }
                act.action = "press_key";
                act.page = PageAction::Key;
                act.note = format!("Pressed {key}.");
                act.what = format!("press Enter to send on {host}");
                risk = (key == "Enter")
                    .then(|| page::risk(&host, "send"))
                    .flatten();
                act.text = key;
            }
            _ => {
                let direction = string("direction");
                let up = direction.as_deref() == Some("up");
                let way = if up { "up" } else { "down" };
                act.action = "scroll";
                act.page = if up {
                    PageAction::ScrollUp
                } else {
                    PageAction::ScrollDown
                };
                act.note = match (&element, direction) {
                    (Some(element), None) => {
                        act.page = PageAction::ScrollTo;
                        format!("Scrolled to {}.", label(element))
                    }
                    (Some(element), Some(_)) => format!("Scrolled {way} in {}.", label(element)),
                    (None, _) => format!("Scrolled {way}."),
                };
            }
        }
        self.check(act, host, risk, cx);
    }

    /// The `url` argument as the address bar takes it, or None once the call has its error.
    fn url_argument(&self, arguments: &Value, reply: &mpsc::Sender<ToolResult>) -> Option<String> {
        let typed = arguments
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let url = (!typed.trim().is_empty())
            .then(|| self.browser.url_for(typed))
            .flatten();
        if url.is_none() {
            _ = reply.send(ToolResult::error("url is a URL, or words to search for."));
        }
        url
    }

    /// Runs the action, unless what it does on `site` asks the user first (`why`): then it waits
    /// for the user's answer, or the one they gave for good.
    fn check(
        &mut self,
        mut act: Act,
        site: String,
        why: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        let Some(why) = why else {
            return self.run(act, cx);
        };
        act.did = Did::of(why);
        match self
            .grants
            .get(&self.answers_of(&act.agent))
            .and_then(|sites| sites.get(&site))
        {
            Some(true) => self.run(act, cx),
            Some(false) => self.refuse(act, &site, SAID_NEVER, cx),
            None => self.ask_first(act, site, why, cx),
        }
    }

    /// Asks the user before the action on `site`, `why`, and waits for their answer.
    fn ask_first(&mut self, act: Act, site: String, why: &'static str, cx: &mut Context<Self>) {
        // A tab not the agent's is the core's to refuse, without a question.
        let space = self.browser.agent_space(&act.agent);
        if act.tab != 0
            && !self
                .browser
                .space_tabs(space, cx)
                .iter()
                .any(|tab| tab.id == act.tab)
        {
            return self.run(act, cx);
        }
        let changes: Vec<&request::Request> =
            act.requests.iter().filter(|each| each.changes()).collect();
        let listed = changes.iter().take(request::LISTED);
        let question = Question {
            id: self.next_request,
            agent: self.agent_name(&act.agent),
            from: act.agent.clone().into(),
            what: act.what.clone().into(),
            why: why.into(),
            changes: listed
                .map(|each| (each.line().into(), each.summary().into()))
                .collect(),
            more: changes.len().saturating_sub(request::LISTED),
            ..Question::default()
        };
        self.next_request += 1;
        // check.sh reads this line.
        log!(
            "browser: agent {} asks {}: {}",
            act.agent,
            question.id,
            act.what
        );
        let id = question.id;
        self.asking.push(Asking {
            question,
            site,
            act,
            _timer: cx.spawn(async move |agents, cx| {
                cx.background_executor().timer(Waits::launch().ask).await;
                _ = agents.update(cx, |agents, cx| agents.unanswered(id, cx));
            }),
        });
        self.asked(cx);
    }

    /// The question `id` waited ASK_WAIT with no answer: the action is refused this once, as the
    /// × refuses it, so the agent isn't left waiting on a user who went away.
    fn unanswered(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(at) = (self.asking.iter()).position(|asking| asking.question.id == id) else {
            return;
        };
        let Asking { site, act, .. } = self.asking.remove(at);
        log!("browser: agent {} asked {id} with no answer", act.agent);
        self.refuse(act, &site, NO_ANSWER, cx);
        self.asked(cx);
    }

    /// Answers the handoff `question` of `tab` once HANDOFF_WAIT passes with the tab still the
    /// user's. The tab stays theirs, and Hand Back still gives it back.
    fn handoff_timer(&self, tab: u64, question: u64, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |agents, cx| {
            cx.background_executor()
                .timer(Waits::launch().handoff)
                .await;
            _ = agents.update(cx, |agents, cx| {
                // A timer of an earlier handoff of the tab has gone with its entry, and this
                // checks it's this one's.
                if agents
                    .held
                    .get(&tab)
                    .is_some_and(|held| held.question == question)
                {
                    agents.let_go(tab, held::Event::Timer, cx);
                }
            });
        })
    }

    /// The user's answer to the question `id`.
    pub fn answer(&mut self, id: u64, answer: Answer, cx: &mut Context<Self>) {
        let Some(at) = self
            .asking
            .iter()
            .position(|asking| asking.question.id == id)
        else {
            return self.answer_back(id, answer, cx);
        };
        let Asking { site, act, .. } = self.asking.remove(at);
        // check.sh reads this line.
        log!("browser: agent {} answered {id} {answer:?}", act.agent);
        // A request's changes ask every time, whatever answered.
        if act.action != "request"
            && let Answer::Always | Answer::Never = answer
        {
            let sites = self.grants.entry(self.answers_of(&act.agent)).or_default();
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
        let asked = (self.held.iter())
            .find(|(_, held)| held.question == id && held.life() == held::Life::Asked);
        let Some((&tab, held)) = asked else {
            return;
        };
        // check.sh reads this line.
        log!("browser: agent {} answered {id} {answer:?}", held.agent);
        if let Answer::Once | Answer::Always = answer {
            return self.hand_back(tab, cx);
        }
        self.let_go(tab, held::Event::Keep, cx);
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

    /// Whose answers for good `agent`'s are, by the key they're kept under: a connected app's its
    /// own, and a ⌘T task's its CLI's (task_answers), as a task's own id is new each task and
    /// numbered from 1 again each run, so kept under it an answer was listed nowhere and the next
    /// run's task of that number took it (#1913).
    fn answers_of(&self, agent: &str) -> String {
        match self.task(agent).and_then(|task| task.cli) {
            Some(cli) => task_answers(cli),
            None => agent.to_string(),
        }
    }

    /// Forgets the user's answer for good to the agent on `site`, or on every site with None, so
    /// what asks first there asks again. `agent` is the key the answers are kept under: a connected
    /// app's id, or a ⌘T CLI's task_answers.
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
    /// agent waits on it, of those their agents show (`first_prompt`).
    pub fn question(&self) -> Option<Question> {
        let shown = |agent: &str, id: u64| self.first_prompt(agent) == Some(id);
        let confirmation =
            (self.asking.iter()).find(|asking| shown(&asking.act.agent, asking.question.id));
        if let Some(asking) = confirmation {
            return Some(asking.question.clone());
        }
        let handoff = (self.held.iter())
            .filter(|(_, held)| held.reply.is_some() && shown(&held.agent, held.question))
            .min_by_key(|(_, held)| held.question);
        handoff.map(|(&tab, held)| Question {
            id: held.question,
            agent: self.agent_name(&held.agent),
            from: held.agent.clone().into(),
            what: match held.back {
                Some(_) => "have a tab back".into(),
                None => "hand you a tab".into(),
            },
            why: held.back.as_ref().unwrap_or(&held.why).clone().into(),
            handoff: Some(tab),
            back: held.back.is_some(),
            ..Question::default()
        })
    }

    fn asked(&self, cx: &mut Context<Self>) {
        self.browser.ask(self.question(), cx);
        cx.notify();
    }

    /// The agent went: what waits for it goes unanswered, its questions with it.
    fn forget_waiting(&mut self, agent: &str, cx: &mut Context<Self>) {
        let asked = self.question().map(|question| question.id);
        // A download nobody's left to answer for leaves no file.
        for asking in self
            .asking
            .iter()
            .filter(|asking| asking.act.agent == agent)
        {
            if asking.act.download != 0 {
                self.browser.answer_download(asking.act.download, false);
                self.ended.remove(&asking.act.download);
            }
        }
        self.asking.retain(|asking| asking.act.agent != agent);
        self.downloading
            .retain(|_, downloading| downloading.agent != agent);
        self.acting.retain(|_, acting| acting.agent != agent);
        self.sending.retain(|_, sending| sending.agent != agent);
        self.runs.retain(|run| run.agent != agent);
        for tab in self.held_from(agent) {
            self.let_go(tab, held::Event::Gone { holder: true }, cx);
        }
        if self.question().map(|question| question.id) != asked {
            self.asked(cx);
        }
    }

    /// Answers the action with why it's refused, which the log has.
    fn refuse(&mut self, act: Act, site: &str, why: &str, cx: &mut Context<Self>) {
        if act.download != 0 {
            self.browser.answer_download(act.download, false);
            self.ended.remove(&act.download);
        }
        // A call's requests the user said no to are each listed, as each sent one is.
        match act.requests.as_slice() {
            [] => self.note(&act.agent, act.action, site, why),
            requests => {
                for each in requests {
                    self.note_request(&act.agent, site, each, why);
                }
            }
        }
        cx.notify();
        let answer = match why {
            SAID_NO => format!("The user said no: you can't {}. Don't try again.", act.what),
            NO_ANSWER => format!(
                "The user didn't answer in 5 minutes, so you can't {} now. Ask them where you \
                 started the task before you try again.",
                act.what
            ),
            SAID_NEVER => {
                format!(
                    "The user said never on {site}: you can't {}. Don't try again.",
                    act.what
                )
            }
            _ => why.to_string(),
        };
        _ = act.reply.send(ToolResult::error(answer));
    }

    /// Does the action: the core checks it and logs it, and answers with the page.
    fn run(&mut self, act: Act, cx: &mut Context<Self>) {
        if act.download != 0 {
            return self.download(act, cx);
        }
        let Act {
            agent,
            call,
            action,
            mut tab,
            url,
            page,
            element,
            text,
            mut note,
            show,
            field,
            requests,
            did,
            reply,
            ..
        } = act;
        // The user said yes to closing a page the agent didn't open without saving.
        if action == "discard_tab" {
            let site = site(&self.browser.tab(tab, cx).url);
            let closed = match self.browser.may_act(&agent, tab, "close_tab", cx) {
                Ok(()) => self.browser.close_tab(&agent, tab, true, cx),
                Err(why) => return self.refuse(act_of(&agent, call, tab, reply), &site, &why, cx),
            };
            return self.closed(&agent, tab, true, closed, &site, reply, cx);
        }
        if action == "request" {
            let left = requests.into();
            let (answers, shown, hidden) = (Vec::new(), request::ALL_SHOWN, 0);
            let sending = Sending {
                agent,
                tab,
                sent: None,
                left,
                answers,
                shown,
                hidden,
                reply,
            };
            return self.send_next(sending, cx);
        }
        if action == "open_tab" {
            let site = site(&url);
            // An agent's first call may be this one, and the core opens a tab only in its Space.
            self.space(&agent, cx);
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
            self.opened(&agent, call, tab);
            self.opened.insert(tab);
            // An agent's tab is quiet until the user wants to hear it (#1080).
            self.browser.set_muted(tab, true);
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
        let opened = action == "open_tab";
        let acting = Acting {
            agent: agent.clone(),
            call,
            did,
            tab,
            element,
            note,
            show,
            opened,
            field,
            reply,
            opened_tabs: vec![],
        };
        self.acting.insert(request, acting);
        self.act_in_page(&agent, tab, request, page, element, &text);
    }

    /// The core put `tab`, which `opener`'s page opened, beside it: if an agent is acting there, in
    /// its Space, behind, and its action's answer names it (#1470). In an agent's Space it's quiet,
    /// as the agent's own tabs are, until the user wants to hear it (#1080).
    pub fn tab_opened(&mut self, tab: u64, opener: u64, cx: &App) {
        if let Some(acting) = self.acting.values_mut().find(|acting| acting.tab == opener) {
            acting.opened_tabs.push(tab);
        }
        let tasks = &self.tasks.tasks;
        if tasks
            .iter()
            .any(|task| self.browser.space_tab_ids(task.space, cx).contains(&tab))
        {
            self.browser.set_muted(tab, true);
        }
    }

    /// A download a page in an agent's Space started, held by the core. It puts a file on the
    /// user's disk, so it asks first, as other actions that reach outside the page do, and
    /// answers the action that started it, if one did, once it ends.
    pub fn download_started(&mut self, download: Download, cx: &mut Context<Self>) {
        let Download {
            agent,
            tab,
            request,
            id,
            name,
            url,
            tab_empty,
        } = download;
        let reply = match self.acting.remove(&request) {
            Some(acting) => acting.reply,
            // A page's own download, with no action of the agent's to answer, still asks.
            None => mpsc::channel().0,
        };
        if tab_empty {
            self.browser.close_user_tab(tab, cx);
        }
        let host = site(&url);
        let act = Act {
            agent,
            // A page's download comes from no call of the agent's.
            call: 0,
            action: "download",
            // The question is about the file, wherever its tab is.
            tab: 0,
            what: format!("download {name} from {host}"),
            url,
            page: PageAction::Snapshot,
            element: 0,
            text: name,
            note: String::new(),
            show: Show::Nothing,
            field: String::new(),
            download: id,
            requests: Vec::new(),
            did: None,
            reply,
        };
        self.check(act, host, Some(DOWNLOADS), cx);
    }

    /// Lets the download the user said yes to go on. Its end answers, or a while if it's slow.
    fn download(&mut self, act: Act, cx: &mut Context<Self>) {
        let Act {
            agent,
            url,
            text: name,
            download,
            reply,
            ..
        } = act;
        self.note(&agent, "download", &site(&url), "");
        cx.notify();
        self.browser.answer_download(download, true);
        if let Some(end) = self.ended.remove(&download) {
            _ = reply.send(download_answer(&name, end));
            return self.advance(cx);
        }
        self.downloading
            .insert(download, Downloading { agent, name, reply });
        cx.spawn(async move |agents, cx| {
            cx.background_executor().timer(DOWNLOAD_WAIT).await;
            _ = agents.update(cx, |agents, cx| {
                let Some(Downloading { name, reply, .. }) = agents.downloading.remove(&download)
                else {
                    return;
                };
                _ = reply.send(ToolResult::text(format!(
                    "{name} is still downloading, to the user's downloads. {NO_FILES}"
                )));
                agents.advance(cx);
            });
        })
        .detach();
    }

    /// How a download the user let an agent's page make ended, which answers its action.
    pub fn download_ended(&mut self, download: u64, end: DownloadEnd, cx: &mut Context<Self>) {
        let Some(Downloading { name, reply, .. }) = self.downloading.remove(&download) else {
            // Ended before the user answered for it, which the yes answers with.
            if self
                .asking
                .iter()
                .any(|asking| asking.act.download == download)
            {
                self.ended.insert(download, end);
            }
            return;
        };
        _ = reply.send(download_answer(&name, end));
        self.advance(cx);
    }

    /// Sends the call's next request, which the core checks and logs, or answers the call once
    /// there's none left.
    fn send_next(&mut self, mut sending: Sending, cx: &mut Context<Self>) {
        let Some(next) = sending.left.pop_front() else {
            return finish(sending, false);
        };
        let (agent, tab) = (sending.agent.clone(), sending.tab);
        let checked = self.browser.may_act(&agent, tab, "request", cx);
        let site = site(&self.browser.tab(tab, cx).url);
        let refused = checked.as_ref().err().map_or("", String::as_str);
        self.note_request(&agent, &site, &next, refused);
        cx.notify();
        if let Err(why) = checked {
            sending.answers.push(format!("{}: {why}", next.target()));
            return finish(sending, true);
        }
        self.current.insert(agent.clone(), tab);
        let request = self.next_request;
        self.next_request += 1;
        // check.sh reads this line.
        log!(
            "browser: agent {agent} request {request}: {} {}",
            next.method,
            next.url
        );
        self.browser.request(&agent, tab, request, &next);
        sending.sent = Some(next);
        self.sending.insert(request, sending);
    }

    /// The core's answer to the request `request`: the next goes, unless none came.
    pub fn request_done(
        &mut self,
        request: u64,
        response: request::Response,
        cx: &mut Context<Self>,
    ) {
        let Some(mut sending) = self.sending.remove(&request) else {
            return;
        };
        let Some(sent) = sending.sent.take() else {
            return;
        };
        let (shown, hidden) = (&mut sending.shown, &mut sending.hidden);
        sending
            .answers
            .push(request::answer(&sent, &response, shown, hidden));
        match response.status {
            // Whatever stopped it may stop the rest, which wait for the agent to look again.
            0 => finish(sending, true),
            _ => self.send_next(sending, cx),
        }
    }

    /// The ref of the element the action `request` acts on, such as "e12", as the agent named it.
    pub fn ref_of(&self, request: u64) -> Option<String> {
        let acting = self.acting.get(&request)?;
        let number = self.refs.get(&acting.tab)?.number(acting.element)?;
        Some(format!("e{number}"))
    }

    /// The core's answer to the action `request`: with `error`, why it couldn't; else a
    /// screenshot's JPEG in base64, or the page's tree, whose nodes the answer outlines, and
    /// whether the page was still busy when the core stopped waiting for it.
    pub fn page_done(&mut self, request: u64, text: String, error: bool, cx: &mut Context<Self>) {
        let Some(Acting {
            agent,
            call,
            did,
            tab,
            mut note,
            show,
            opened,
            field,
            reply,
            opened_tabs,
            ..
        }) = self.acting.remove(&request)
        else {
            return;
        };
        if !opened_tabs.is_empty() {
            let tabs = opened_tabs
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let s = if opened_tabs.len() > 1 { "s" } else { "" };
            note = format!("{note} It opened tab{s} {tabs} in your Space.")
                .trim_start()
                .into();
        }
        if error {
            // As a page still loading: the agent can look at the tab it opened later (#1461).
            let text = match opened {
                true => format!("{note} {text}"),
                false => text,
            };
            _ = reply.send(ToolResult::error(text));
            return self.advance(cx);
        }
        if let Some(did) = did {
            self.counted(&agent, call, did);
        }
        if !text.starts_with('{') {
            _ = reply.send(ToolResult::image(text));
            return;
        }
        let tree: Value = serde_json::from_str(&text).unwrap_or_default();
        let nodes = tree["nodes"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let unsettled = match (tree["busy"].as_bool(), tree["requests"].as_bool()) {
            (Some(true), _) => Some(BUSY),
            (_, Some(true)) => Some(REQUESTS),
            _ => None,
        };
        if let Some(unsettled) = unsettled {
            note = match note.is_empty() {
                true => unsettled.into(),
                false => format!("{note} {unsettled}"),
            };
        }
        // A choice answers with what the field holds once it shows the option, as the core
        // names it, and else says the page may have undone it.
        if !field.is_empty() {
            note = match tree["chosen"].as_str() {
                Some(option) => format!("The {field} now holds “{}”.", page::clip(option, 60)),
                None => format!("{note} {UNSHOWN}"),
            };
        }
        let info = self.browser.tab(tab, cx);
        let refs = self.refs.entry(tab).or_default();
        let text = match show {
            Show::Element(node) => {
                let part = page::outline(&info, &nodes, refs, Some(node)).text();
                match note.is_empty() {
                    true => part,
                    false => format!("{note}\n\n{part}"),
                }
            }
            show => {
                self.outlines
                    .insert(tab, page::outline(&info, &nodes, refs, None));
                self.answer_with(&agent, tab, note, show)
            }
        };
        _ = reply.send(ToolResult::text(text));
        self.advance(cx);
    }

    /// `note`, what an action did, then the tab's latest outline as `show` says, which is the
    /// outline the agent has of the tab from then on, unless it gets none of it.
    fn answer_with(&mut self, agent: &str, tab: u64, note: String, show: Show) -> String {
        let Some(outline) = self.outlines.get(&tab) else {
            return note;
        };
        let had = (self.had.get(&tab))
            .filter(|(whose, _)| whose == agent)
            .map(|(_, had)| had);
        let page = match (show, had) {
            (Show::Nothing, _) => outline.head().to_string(),
            (Show::Changes, Some(had)) => outline.since(had),
            _ => outline.text(),
        };
        if show != Show::Nothing {
            self.had.insert(tab, (agent.to_string(), outline.clone()));
        }
        match note.is_empty() {
            true => page,
            false => format!("{note}\n\n{page}"),
        }
    }

    /// Pins the agent's tab, or unpins or removes one it pinned (#1360). The core keeps every one
    /// to the agent's own Space; a pinned tab the agent didn't pin is the user's, and stays.
    fn pin(
        &mut self,
        agent: &str,
        action: &str,
        tab: u64,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let pinned = self.browser.tab(tab, cx).pinned;
        let theirs = pinned && self.pins.get(&tab).is_none_or(|by| by != agent);
        let checked = self.browser.may_act(agent, tab, action, cx).and_then(|()| {
            match (action, pinned, theirs) {
                ("pin_tab", _, _) => Ok(()),
                (_, false, _) => Err(format!("Tab {tab} isn't pinned.")),
                (_, true, true) => Err(format!(
                    "The user pinned tab {tab}, so only they {} it.",
                    action.trim_end_matches("_tab")
                )),
                (_, true, false) => Ok(()),
            }
        });
        let site = site(&self.browser.tab(tab, cx).url);
        self.note(
            agent,
            action,
            &site,
            checked.as_ref().err().map_or("", String::as_str),
        );
        cx.notify();
        if let Err(why) = checked {
            _ = reply.send(ToolResult::error(why));
            return;
        }
        self.current.insert(agent.into(), tab);
        let answer = match (action, pinned) {
            ("pin_tab", true) => format!("Tab {tab} is pinned already."),
            ("pin_tab", false) => {
                self.browser.set_pinned(tab, true, cx);
                self.pins.insert(tab, agent.into());
                format!(
                    "Pinned tab {tab} at the page it's at. It stays in your Space's pinned tabs, \
                     even closed, until you unpin or remove it."
                )
            }
            ("unpin_tab", _) => {
                self.browser.set_pinned(tab, false, cx);
                self.pins.remove(&tab);
                format!("Unpinned tab {tab}. It's at the top of your Today tabs.")
            }
            _ => {
                self.browser.remove_pinned(tab, cx);
                self.pins.remove(&tab);
                let space = self.browser.agent_space(agent);
                let open = (self.browser.space_tabs(space, cx).iter()).any(|each| each.id == tab);
                match open {
                    true => format!(
                        "Tab {tab} is unpinned but stays open: its page asks the user before it's \
                         left."
                    ),
                    false => {
                        format!("Removed tab {tab}. The user can still find it in the archive.")
                    }
                }
            }
        };
        _ = reply.send(ToolResult::text(answer));
    }

    /// The user pinned, unpinned or moved `tab`, which makes its pin theirs.
    pub fn users_pin(&mut self, tab: u64) {
        self.pins.remove(&tab);
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

    /// Closes the agent's `tab`. A page that may ask before it's left stays open, unless `discard`:
    /// then one the agent opened closes, and any other asks the user first, as what it holds
    /// unsaved may be theirs.
    fn close_tab(
        &mut self,
        agent: &str,
        call: u64,
        tab: u64,
        discard: bool,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let site = site(&self.browser.tab(tab, cx).url);
        let mut closed = match self.browser.may_act(agent, tab, "close_tab", cx) {
            Ok(()) => self.browser.close_tab(agent, tab, false, cx),
            Err(why) => return self.refuse(act_of(agent, call, tab, reply), &site, &why, cx),
        };
        let discarded = closed == Closed::Asks && discard;
        if discarded && !self.opened.contains(&tab) {
            let what = format!("close the tab on {site} without saving");
            let act = Act {
                action: "discard_tab",
                what,
                ..act_of(agent, call, tab, reply)
            };
            return self.check(act, site, Some(UNSAVED), cx);
        }
        if discarded {
            closed = self.browser.close_tab(agent, tab, true, cx);
        }
        self.closed(agent, tab, discarded, closed, &site, reply, cx);
    }

    /// Answers the agent's close of `tab` on `site` with what happened, and logs it.
    #[allow(clippy::too_many_arguments)]
    fn closed(
        &mut self,
        agent: &str,
        tab: u64,
        discarded: bool,
        closed: Closed,
        site: &str,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let answer = match closed {
            Closed::Closed if discarded => Ok(format!("Closed tab {tab} without saving.")),
            Closed::Closed => Ok(format!("Closed tab {tab}.")),
            Closed::Pinned => Err(format!("Tab {tab} stays open: the user pinned it.")),
            Closed::Asks => Err(format!(
                "Tab {tab} stays open: its page asks before it's left, as it may have unsaved \
                 changes. close_tab with discard: true closes it without saving{}.",
                if self.opened.contains(&tab) {
                    ""
                } else {
                    ", once the user says yes"
                }
            )),
            Closed::Refused => Err("The tab isn't in the agent's own Space.".into()),
        };
        let logged = if discarded {
            "discard_tab"
        } else {
            "close_tab"
        };
        self.note(
            agent,
            logged,
            site,
            answer.as_ref().err().map_or("", String::as_str),
        );
        if closed == Closed::Closed {
            self.opened.remove(&tab);
        }
        cx.notify();
        _ = reply.send(match answer {
            Ok(text) => ToolResult::text(text),
            Err(why) => ToolResult::error(why),
        });
    }

    /// The agent's Space, hidden, made the first time it calls, where its tabs open.
    fn space(&mut self, agent: &str, cx: &mut Context<Self>) -> u64 {
        match self.browser.agent_space(agent) {
            0 => match self.task(agent) {
                Some(task) => {
                    let (named, home) = (task.name.clone(), task.home);
                    let space = self.browser.make_task_space(agent, &named, home, cx);
                    self.task_space_made(agent, space);
                    space
                }
                None => {
                    let space = self.browser.make_agent_space(agent, name(agent), cx);
                    self.client_space_made(agent, space);
                    space
                }
            },
            space => space,
        }
    }

    /// The tab the agent used last, if it's still in its Space, else its first.
    fn default_tab(&mut self, agent: &str, cx: &mut Context<Self>) -> Option<u64> {
        let space = self.space(agent, cx);
        let tabs = self.browser.space_tabs(space, cx);
        let last = self.current.get(agent).copied();
        (last.filter(|last| tabs.iter().any(|tab| tab.id == *last)))
            .or_else(|| tabs.first().map(|tab| tab.id))
    }

    fn note(&mut self, agent: &str, action: &str, site: &str, refused: &str) {
        self.note_on(agent, action, site, "", refused)
    }

    /// As `note`, for a request: where it went and what it sent, or would have.
    fn note_request(&mut self, agent: &str, site: &str, sent: &request::Request, refused: &str) {
        let entry = (sent.url.as_str(), sent.sent());
        self.note_entry(agent, "request", site, &sent.target(), refused, entry);
    }

    /// As `note`, for what on the page, `target`.
    fn note_on(&mut self, agent: &str, action: &str, site: &str, target: &str, refused: &str) {
        self.note_entry(agent, action, site, target, refused, ("", Vec::new()))
    }

    fn note_entry(
        &mut self,
        agent: &str,
        action: &str,
        site: &str,
        target: &str,
        refused: &str,
        (url, sent): (&str, Vec<request::Sent>),
    ) {
        // The rules run reads each entry as Settings › Agents lists it, the app's own refusals too,
        // which never reach the core's line.
        match refused.is_empty() {
            true => log!("browser: agent logged {agent} {action} {site}"),
            false => log!("browser: agent logged {agent} {action} {site} refused: {refused}"),
        }
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
            target: SharedString::from(target.to_string()),
            refused: SharedString::from(refused.to_string()),
            steps: Vec::new(),
            by: SharedString::default(),
            id: entry_id(),
            url: SharedString::from(url.to_string()),
            sent,
        };
        if self.stepping.as_deref() == Some(agent) {
            let run = self
                .log
                .iter_mut()
                .rev()
                .find(|each| each.agent == agent && each.action == "run_steps");
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
        for tab in self.held.keys().copied().collect::<Vec<_>>() {
            self.let_go(tab, held::Event::Gone { holder: true }, cx);
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
        // A session's calls come with its client's token: it has none of its own.
        let tasks = (self.tasks.tasks.iter())
            .filter(|task| task.session.is_none())
            .map(|task| (task.token().to_string(), task.agent.clone()));
        *self.tokens.lock().unwrap() = tokens.chain(tasks).collect();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let agents: Vec<Value> = (self.agents.iter())
            .map(|agent| json!({ "client": agent.client.id(), "token": agent.token }))
            .collect();
        let grants: serde_json::Map<String, Value> = (self.grants.iter())
            .map(|(agent, sites)| {
                let sites = (sites.iter()).map(|(site, &may)| {
                    (site.clone(), json!(if may { "always" } else { "never" }))
                });
                (agent.clone(), Value::Object(sites.collect()))
            })
            .collect();
        let models: serde_json::Map<String, Value> = (self.tasks.models.iter())
            .map(|(cli, model)| (cli.id().into(), json!(model)))
            .collect();
        let found: serde_json::Map<String, Value> = (self.tasks.found.iter())
            .map(|(cli, found)| (cli.id().into(), found.saved()))
            .collect();
        let tasks = json!({
            "on": !self.tasks_off,
            "default": self.tasks.default.map(runner::Cli::id),
            "models": models,
            "found": found,
        });
        let saved = json!({
            "on": self.on,
            "agents": agents,
            "grants": grants,
            "tasks": tasks,
            "scripts": self.scripts,
        });
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

/// The answer to the call that waited on the held `tab`, as `event` ended its wait.
fn answer_of(tab: u64, event: held::Event) -> ToolResult {
    match event {
        held::Event::HandBack => ToolResult::text(format!(
            "The user handed tab {tab} back. Take a snapshot to see where it is now."
        )),
        held::Event::Keep => ToolResult::error(format!(
            "The user keeps tab {tab}. Don't ask for it again unless they say so."
        )),
        held::Event::Timer => ToolResult::error(format!(
            "The user hasn't handed tab {tab} back in 15 minutes. It stays theirs; ask them \
             where you started the task before you go on there."
        )),
        held::Event::Closed => {
            ToolResult::error(format!("Tab {tab} closed while the user had it."))
        }
        held::Event::Moved => ToolResult::error(format!(
            "The user moved tab {tab} to a Space of their own. It's theirs now."
        )),
        _ => ToolResult::error(STOPPED),
    }
}

/// The core closed `tab` (tabs::tab_closed).
pub fn tab_closed(tab: u64, cx: &mut App) {
    if let Some(agents) = global(cx) {
        agents.update(cx, |agents, cx| agents.tab_closed(tab, cx));
    }
}

/// `space`'s tabs changed (spaces::space_changed).
pub fn space_changed(space: u64, cx: &mut App) {
    if let Some(agents) = global(cx) {
        agents.update(cx, |agents, cx| agents.space_changed(space, cx));
    }
}

/// An action on `tab` for the call `call` with nothing more to it, as a close is.
fn act_of(agent: &str, call: u64, tab: u64, reply: mpsc::Sender<ToolResult>) -> Act {
    Act {
        agent: agent.into(),
        call,
        action: "close_tab",
        tab,
        url: String::new(),
        page: PageAction::Snapshot,
        element: 0,
        text: String::new(),
        note: String::new(),
        show: Show::Nothing,
        what: String::new(),
        field: String::new(),
        download: 0,
        requests: Vec::new(),
        did: None,
        reply,
    }
}

/// Why a request's changes ask: each time, as no name says what an API call does.
const CHANGES_ASK: &str = "Changes ask every time.";

/// Answers a call's requests with what each answered, numbered if there were more than one, and
/// what wasn't sent.
fn finish(sending: Sending, error: bool) {
    let Sending {
        answers,
        left,
        hidden,
        reply,
        ..
    } = sending;
    let many = answers.len() + left.len() > 1;
    let answers = answers.iter().enumerate().map(|(at, answer)| match many {
        true => format!("{}. {answer}", at + 1),
        false => answer.clone(),
    });
    let mut text = answers.collect::<Vec<_>>().join("\n\n");
    match left.len() {
        0 => {}
        1 => text.push_str("\n\nThe last request wasn't sent."),
        unsent => text.push_str(&format!("\n\nThe last {unsent} requests weren't sent.")),
    }
    if hidden > 0 {
        text.push_str(
            "\n\nValues that look like sign-in tokens or secrets are hidden: agents act in the \
             user's sessions, and never hold their credentials.",
        );
    }
    _ = reply.send(ToolResult {
        text,
        error,
        image: None,
    });
}

/// What the log says of an action the user refused, now or for good on its site.
const SAID_NO: &str = "You said no.";
/// What the log says of an action that asked and had no answer in ASK_WAIT.
const NO_ANSWER: &str = "No answer in 5 minutes.";
/// The spec's: how long an action asking first waits for the user's answer.
const ASK_WAIT: Duration = Duration::from_secs(5 * 60);
/// The spec's: how long a handoff's call waits for the user to hand the tab back.
const HANDOFF_WAIT: Duration = Duration::from_secs(15 * 60);
/// A check's waits in place of ASK_WAIT and HANDOFF_WAIT, in seconds, as
/// `--agent-waits=<ask>,<handoff>`, so a run sees each end without waiting minutes. Not read in a
/// prod build.
const WAITS_SWITCH: &str = "--agent-waits";

/// How long an ask and a handoff wait: the spec's, or a check's from the launch's arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Waits {
    ask: Duration,
    handoff: Duration,
}

impl Waits {
    fn new(given: &[String], prod: bool) -> Waits {
        let spec = Waits {
            ask: ASK_WAIT,
            handoff: HANDOFF_WAIT,
        };
        let prefix = format!("{WAITS_SWITCH}=");
        let Some(value) = (!prod)
            .then(|| (given.iter().rev()).find_map(|arg| arg.strip_prefix(prefix.as_str())))
            .flatten()
        else {
            return spec;
        };
        let seconds = |word: &str| word.parse::<u64>().ok().map(Duration::from_secs);
        match value.split_once(',') {
            Some((ask, handoff)) => Waits {
                ask: seconds(ask).unwrap_or(spec.ask),
                handoff: seconds(handoff).unwrap_or(spec.handoff),
            },
            None => spec,
        }
    }

    /// This launch's, read once.
    fn launch() -> Waits {
        static WAITS: std::sync::OnceLock<Waits> = std::sync::OnceLock::new();
        *WAITS.get_or_init(|| {
            let given: Vec<String> = std::env::args().collect();
            Waits::new(&given, cfg!(feature = "prod"))
        })
    }
}
/// Why discarding a page the agent didn't open asks the user first.
const UNSAVED: &str = "Its page may have unsaved changes, which closing it loses.";
const SAID_NEVER: &str = "You said never on this site.";
/// Why an agent's download asks first.
const DOWNLOADS: &str = "It saves a file to your computer.";
/// What an agent's download answers with, as it can't read the file.
const NO_FILES: &str = "Agents can't open files; ask the user if you need it.";
/// Ours: how long an agent's download waits to answer with its file, as a large one or one Chrome
/// holds as dangerous may take a while.
const DOWNLOAD_WAIT: Duration = Duration::from_secs(30);
/// Why an agent can't open a page that isn't on the web.
const WEB_ONLY: &str = "Agents may open only http and https pages.";
/// Why an agent can't type into a password field.
const PASSWORDS: &str = "Password fields are off limits to agents: signing in is the user's to do. \
    Hand them the tab with hand_off.";
/// What a choice adds when its field doesn't show the option after.
const UNSHOWN: &str = "It doesn't show it as chosen, so the page may have changed it back.";
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

/// The local time `at`, in seconds since 1970, reads on the clock here, such as "18:00", with
/// its day if it's a day or more away, such as "Tue 18:00".
pub fn local_time(at: u64) -> String {
    use chrono::{Local, TimeZone};
    let Some(time) = Local.timestamp_opt(at as i64, 0).single() else {
        return String::new();
    };
    let soon = time.signed_duration_since(Local::now()) < chrono::Duration::hours(20);
    time.format(if soon { "%H:%M" } else { "%a %H:%M" })
        .to_string()
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

/// Whether the agent with the id `agent` runs a task the user asked from ⌘T (Agent task).
pub fn is_task(agent: &str) -> bool {
    agent.starts_with("task-")
}

/// The answer of the download `name` the user let an agent's page make, as it ended.
fn download_answer(name: &str, end: DownloadEnd) -> ToolResult {
    match end {
        DownloadEnd::Done { name, bytes, path } => {
            let folder = std::path::Path::new(&path)
                .parent()
                .and_then(|dir| dir.file_name());
            let folder = folder.map(|dir| dir.to_string_lossy());
            let folder = folder.unwrap_or("the user's downloads".into());
            let size = crate::shell::bytes(bytes);
            ToolResult::text(format!(
                "Downloaded {name} ({size}) to {folder}. {NO_FILES}"
            ))
        }
        DownloadEnd::Failed => ToolResult::error(format!("The download of {name} failed.")),
        DownloadEnd::Cancelled => {
            ToolResult::error(format!("The download of {name} was cancelled."))
        }
    }
}
