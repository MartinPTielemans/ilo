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

mod access;
pub mod bridge;
pub mod cli;
pub mod clients;
mod codex;
pub mod command;
pub mod fence;
mod held;
pub mod kept;
pub mod notes;
pub mod page;
#[cfg(windows)]
mod pipe;
pub mod ready;
pub mod request;
pub mod runner;
pub mod saved_read;
pub mod scripts;
mod server;
pub mod sign_in;
pub mod signing;
pub mod task;
pub mod tier;
pub mod tools;

pub use server::address;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use std::time::{Instant, SystemTime};

use futures::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global, SharedString, Task};
use serde_json::{Value, json};

use crate::shell::DialogKind;
use crate::sort_by_site;

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
    /// Describes a file field, for the card that asks the user for a file (upload).
    FileField,
}

/// What came of the user's Choose File… for a page's file field (file_field_done). Never a file's
/// name or path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FileFieldEnd {
    /// This many of the user's files went into the field.
    Put(usize),
    /// The user closed the dialog choosing none.
    NoFile,
    /// The field left the page, or the agent's hands, before the files could go in.
    Left,
    /// The dialog closed as its field was dropped, its tab closed or its session ended.
    Closed,
}

/// A page's file field an agent asks the user to fill (upload), as the core described it.
#[derive(Clone, Debug, PartialEq)]
struct FileField {
    id: u64,
    /// The field's accessible name, as the page gives it, or empty.
    label: String,
    many: bool,
}

/// What closing an agent's tab did (bc_agent_close_tab), or came to (agent_close_answered).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Closed {
    Closed,
    /// The tab isn't the agent's, or the user holds it.
    Refused,
    /// The user pinned it, and only they close it.
    Pinned,
    /// Its page asked to stay, as one with unsaved changes does.
    Asks,
    /// The core is asking its page, which answers later (#3256).
    Pending,
    /// It stays for another reason: it went to another page, or its page was unloaded, before its
    /// page answered.
    Stays,
}

/// An agent's close_tab waiting on its page's answer.
struct Closing {
    agent: String,
    connection: u64,
    call: u64,
    discard: bool,
    site: String,
    reply: mpsc::Sender<ToolResult>,
}

/// What an agent asks the user's leave for, as the card in each window asks it.
#[derive(Clone, Debug, Default)]
pub struct Question {
    pub id: u64,
    /// The agent's name, as its Space has it.
    pub agent: SharedString,
    /// The agent's id, by which a task's card asks it rather than the windows' card.
    pub from: SharedString,
    /// What it would do, following "wants to", such as "click “Place order” on shop.example".
    /// Whole wherever it's logged or answered.
    pub what: SharedString,
    /// The same without its site, "click “Place order”", for an action on the tab's page: its
    /// card puts the site, the tab's own, on the line above, apart from the page's words (#3836).
    /// Empty for a question that names no such site.
    pub asks: SharedString,
    /// Why that asks, such as "It looks like a payment or an order.", or for a handoff, what the
    /// agent needs the user to do.
    pub why: SharedString,
    /// The tab an agent handed to the user (hand_off), whose card shows the tab or hands it back.
    pub handoff: Option<u64>,
    /// Whether the agent asks for that tab back (ask_for_tab), which Keep refuses, rather than
    /// handing it to the user.
    pub back: bool,
    /// The requests of a call that may change something (request), each with the agent's own
    /// sentence for it, all of them. They ask every time, so their card has no Always or Never.
    /// Shared, as each window's card and the whole request read the same bodies.
    pub changes: std::rc::Rc<[request::Request]>,
    /// The site they go to, the tab's, which the card's facts name.
    pub site: SharedString,
    /// Whether it asks every time, as a payment does, so its card has no Always or Never.
    pub each_time: bool,
    /// The page's file field the agent asks the user to choose a file for (upload), whose card
    /// chooses one or doesn't allow it.
    pub file: Option<FileAsk>,
    /// Whether it asks on a site the user fenced (fence.rs), so its card has Never and Allow
    /// Once, and no Always.
    pub fenced: bool,
}

/// A file field a card asks the user to choose a file for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FileAsk {
    /// The core's id for it (bc_file_field_choose).
    pub field: u64,
    /// Whether the user may choose several files.
    pub many: bool,
}

/// The windows' cards ask again whenever a question changes, which is on every Agents change, so
/// this compares its small fields and its changes by their pointer: they're shared, never copied,
/// and their bodies may be megabytes.
impl PartialEq for Question {
    fn eq(&self, other: &Question) -> bool {
        (
            self.id,
            &self.agent,
            &self.from,
            &self.what,
            &self.asks,
            &self.why,
        ) == (
            other.id,
            &other.agent,
            &other.from,
            &other.what,
            &other.asks,
            &other.why,
        ) && (self.handoff, self.back, &self.site) == (other.handoff, other.back, &other.site)
            && (self.each_time, self.file, self.fenced)
                == (other.each_time, other.file, other.fenced)
            && std::rc::Rc::ptr_eq(&self.changes, &other.changes)
    }
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
    /// `tab`'s title and URL, as the user sees them.
    fn shown(&self, tab: u64, cx: &App) -> TabInfo;
    /// Whether `tab`'s title and address are withheld from agents (bc_tab_withheld_from_agents):
    /// while the core signs it in, and while it shows a page a sign-in withheld.
    fn withheld(&self, tab: u64) -> bool;
    /// `tab`'s title and URL as agents, and all the app keeps or shows of them, may have them:
    /// neither of a withheld tab, which is said to be withheld in its title's place.
    fn tab(&self, tab: u64, cx: &App) -> TabInfo {
        let shown = self.shown(tab, cx);
        match self.withheld(tab) {
            true => TabInfo {
                title: WITHHELD.into(),
                url: "".into(),
                ..shown
            },
            false => shown,
        }
    }
    /// Why no agent may see or act on `tab`, even in its own Space, or None (`off_limits`). By
    /// its own address, so an agent can still take a withheld tab to another page.
    fn off_limits(&self, tab: u64, cx: &App) -> Option<&'static str> {
        off_limits(&self.shown(tab, cx).url, "")
    }
    /// The Space that has `tab`, or 0 if none does: it closed, or is closing.
    fn space_of(&self, tab: u64, cx: &App) -> u64;
    /// Asks the core whether the agent may do `action` to `tab` (bc_agent_may_act): nothing, or
    /// why not, its code and its words.
    fn may_act(
        &self,
        agent: &str,
        tab: u64,
        action: &str,
        cx: &mut App,
    ) -> Result<(), (Failure, String)>;
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
    /// Whether the core holds `tab` from agents (bc_tab_held), as it does a page that showed what
    /// its sign-in filled.
    fn held(&self, tab: u64) -> bool;
    /// Asks the core to sign the agent's `tab` in with a login the user saved (bc_agent_sign_in).
    /// Its answer comes to `Agents::sign_in_done` with `request`.
    fn sign_in(&self, agent: &str, tab: u64, request: u64);
    /// The user hands the tab back to agents. Whether they held it.
    fn hand_back(&self, tab: u64) -> bool;
    /// Closes the agent's `tab` without a dialog (bc_agent_close_tab): a page with a beforeunload
    /// handler is asked, and Pending is answered later with close_answered. `discard` closes it at
    /// once and loses what it holds unsaved.
    fn close_tab(&self, agent: &str, tab: u64, discard: bool, cx: &mut App) -> Closed;
    /// Closes `tab` as the user would (bc_tab_close): a Today tab goes to the archive. For what the
    /// user does to an agent's tabs, as dropping a done session's.
    fn close_user_tab(&self, tab: u64, cx: &mut App);
    /// Answers for the user about a download an agent's page started, which the core holds
    /// (bc_agent_download_answer): `keep` lets it go on, else it's cancelled and its file gone.
    fn answer_download(&self, download: u64, keep: bool);
    /// Opens the system's file dialog over `window` for the user to choose a file for the page's
    /// file `field` (bc_file_field_choose): false if it can't, else file_field_done answers once.
    fn file_field_choose(&self, field: u64, window: u64) -> bool;
    /// Forgets the page's file `field`, closing its dialog if it's up (bc_file_field_drop).
    fn file_field_drop(&self, field: u64);
    /// Pins `tab` in its Space at the page it's at, or unpins it to the top of its Today tabs, as
    /// the user does (bc_tab_set_pinned).
    fn set_pinned(&self, tab: u64, pinned: bool, cx: &mut App);
    /// Removes the pinned `tab` from its Space, as its row's remove button does: unpinned, then
    /// closed with its split, to the archive.
    fn remove_pinned(&self, tab: u64, cx: &mut App);
    /// The tabs shown with `tab`, left to right: its split view's, else `tab` alone (bc_tab_split).
    fn split(&self, tab: u64, cx: &App) -> Vec<u64>;
    /// Moves the Today `tab`, with its split, to `index` among its Space's Today tabs, counted
    /// without them, as the user's drag does (bc_tab_move).
    fn move_today(&self, tab: u64, index: usize, cx: &mut App);
    /// The site of the page at `url`, the registrable domain the link target's plate emphasizes,
    /// or None off the web (bc_url_site): the same as the sidebar's Sort Today Tabs by Site.
    fn url_site(&self, url: &str) -> Option<String>;
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
    /// Reads `url` with one GET on `profile`'s own session, with no tab (bc_profile_read), with up
    /// to `max_bytes` of its body after up to `timeout_ms`. Its answer comes to
    /// `Agents::read_done` with `read`.
    fn read(&self, profile: u64, read: u64, url: &str, max_bytes: usize, timeout_ms: i64);
    /// Opens a task CLI's sign-in page `url` in a tab of a new private window, of the profile of
    /// the user's window in front, which keeps nothing of it, and shows it: the tab and the
    /// window, or None if it couldn't (#4100). The user's alone: no agent opens one.
    fn open_sign_in(&self, url: &str, cx: &mut App) -> Option<(u64, u64)>;
    /// Closes the sign-in's `tab` in `window`, and the window with it if the tab is all it has.
    fn close_sign_in(&self, tab: u64, window: u64, cx: &mut App);
}

/// What waits on a saved read's answer (Agents::read).
type ReadDone = Box<dyn FnOnce(saved_read::Answer, &mut Context<Agents>)>;

/// What the server's threads hand the UI thread.
pub enum Event {
    /// A client began a session as the agent with the id `agent`, on the server's connection
    /// `connection`. `client` is the name it gave: its title, else its name, or empty.
    Hello {
        agent: String,
        client: String,
        connection: u64,
    },
    /// The agent's session on `connection` ended, as the connection closed.
    Closed {
        agent: String,
        connection: u64,
    },
    /// The agent's last session ended, after its connection's Closed.
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
        kind: Option<tools::Kind>,
        image: Option<String>,
        /// The site whose notes the answer carried, by its note's name.
        notes: Option<&'static str>,
    },
}

/// A tool call, which the server's thread waits on until `reply` has the answer.
pub struct Call {
    /// Its number, which its answer's event names.
    pub id: u64,
    pub agent: String,
    /// The server's connection it came on, which its session goes by (`session_connection`).
    pub connection: u64,
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
    /// Of an error, the kind a task's card says it in, if any.
    pub kind: Option<tools::Kind>,
    /// The site's note the answer comes with (#4122). It's apart from `text`, which may hold a
    /// page's words, here and where it leaves for the agent, so only the app's own note is ever
    /// one: the user's line and the log go by it.
    pub notes: Option<&'static notes::Note>,
}

impl ToolResult {
    /// An answer, cut at `tools::MAX_ANSWER` characters, whatever the tool (#3657).
    pub fn text(text: impl Into<String>) -> Self {
        ToolResult {
            text: tools::capped(text.into()),
            error: false,
            image: None,
            kind: None,
            notes: None,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        ToolResult {
            text: tools::capped(text.into()),
            error: true,
            image: None,
            kind: None,
            notes: None,
        }
    }

    /// The error `text`, of the kind a task's card says it in.
    pub fn failed(kind: tools::Kind, text: impl Into<String>) -> Self {
        ToolResult {
            kind: Some(kind),
            ..ToolResult::error(text)
        }
    }

    pub fn image(jpeg: String) -> Self {
        ToolResult {
            text: String::new(),
            error: false,
            image: Some(jpeg),
            kind: None,
            notes: None,
        }
    }

    /// A page's answer `text` after the site's `note`, the two together within an answer's limit.
    fn noted(text: String, note: Option<&'static notes::Note>) -> Self {
        let before = note.map_or(0, |note| note.framed().chars().count() + 2);
        ToolResult {
            text: tools::capped_after(text, before),
            notes: note,
            ..ToolResult::text(String::new())
        }
    }

    /// The answer whole, as one text: the site's note first, in its frame, then the answer's text.
    /// The agent gets the two as parts of their own (`to_json`).
    pub fn said(&self) -> String {
        match self.notes {
            Some(note) if self.text.is_empty() => note.framed(),
            Some(note) => format!("{}\n\n{}", note.framed(), self.text),
            None => self.text.clone(),
        }
    }

    /// The answer as the agent gets it: its text, then its picture. A site's note is a part of
    /// its own, first, so no words in the part that holds a page's are ilo's note, wherever in it
    /// a page writes them.
    pub fn to_json(&self) -> Value {
        let part = |text: String| json!({ "type": "text", "text": text });
        let mut content: Vec<Value> = self.notes.iter().map(|note| part(note.framed())).collect();
        // An answer of no words is its picture alone, or its note's; with neither, an empty text.
        if !self.text.is_empty() || content.is_empty() && self.image.is_none() {
            content.push(part(self.text.clone()));
        }
        if let Some(jpeg) = &self.image {
            content.push(json!({ "type": "image", "data": jpeg, "mimeType": "image/jpeg" }));
        }
        json!({ "content": content, "isError": self.error })
    }
}

/// An agent's action, from its call until the core answers, and through the question it asks the
/// user first if it does.
struct Act {
    agent: String,
    /// The connection its call came on, or for a page's download the one whose card has the page
    /// (`holder`): its log entries and default tab go by it.
    connection: u64,
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
    /// What it would do, as a question has it, and that without the site (Question::asks).
    what: String,
    asks: String,
    /// The download it lets go on, for "download", else 0.
    download: u64,
    /// The file field it asks the user to choose a file for, for "upload".
    file: Option<FileField>,
    /// The HTTP requests it sends (request).
    requests: Vec<request::Request>,
    /// The text open_tab or navigate waits for in the page once it settles.
    wait: Option<WaitText>,
    /// What it does that asked first, once allowed, for its session's receipt.
    did: Option<Did>,
    /// Its place at a fence the user set.
    fenced: Fenced,
    reply: mpsc::Sender<ToolResult>,
}

impl Act {
    /// Sets what it would do on the tab's page at `host`, whole and without the site, from
    /// `words`, which puts what it's given (" on host", or nothing) where the site goes.
    fn say(&mut self, host: &str, words: impl Fn(&str) -> String) {
        self.what = words(&format!(" on {host}"));
        self.asks = words("");
    }
}

/// An action's place at a site the user fenced (#3724).
enum Fenced {
    No,
    /// An open of a page under the entry, which once allowed lets its tab stay there.
    Open(String),
    /// A call on a tab that came to a page under the entry another way, which waits for the
    /// user to let the tab stay there, and then goes as it came.
    Came(String, Box<Call>),
}

/// A tab the user let stay under a fenced entry (#3724). An open they allowed may not get there:
/// until the tab is under the entry, only the page it was on when they answered keeps their yes.
struct Cleared {
    entry: String,
    /// The tab's address when they answered, and whether it has been under the entry since.
    from: String,
    arrived: bool,
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
    connection: u64,
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

/// A read of a tab's text waiting on the core: read_page's, or a page tool's that answers with the
/// text (`Show::Text`), after what it did, its `note`.
struct Reading {
    agent: String,
    /// The connection its call came on, and its entry in the log, for the site's notes.
    connection: u64,
    entry: u64,
    tab: u64,
    reply: mpsc::Sender<ToolResult>,
    note: String,
    /// The character of the text the answer starts at.
    from: usize,
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
    /// The page's text, as read_page gives it, for a page the agent reads: open_tab's and
    /// navigate's `answer` (#3657).
    Text,
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

    /// As a new page answers: the outline whole, unless none or the text.
    fn whole(self) -> Show {
        match self {
            Show::Nothing | Show::Text => self,
            _ => Show::Full,
        }
    }

    /// open_tab's and navigate's `answer`: "outline", as without it, "text" or "none". Without
    /// it, navigate's `outline`, as before.
    fn answer(arguments: &Value) -> Result<Show, String> {
        match arguments.get("answer") {
            None | Some(Value::Null) => Show::argument(arguments),
            Some(Value::String(show)) if show == "outline" => Ok(Show::Full),
            Some(Value::String(show)) if show == "text" => Ok(Show::Text),
            Some(Value::String(show)) if show == "none" => Ok(Show::Nothing),
            Some(_) => Err("answer is outline, text or none.".into()),
        }
    }
}

/// An action that waits on the core's answer.
struct Acting {
    agent: String,
    connection: u64,
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
    /// The text it waits for once its page settles, looking again until it shows.
    wait: Option<WaitText>,
    /// Its call's entry in the log, which says so if its answer carries a site's notes, or 0.
    entry: u64,
}

/// A text open_tab or navigate waits for in the page's outline once it has settled, as a wait
/// step's does (#3658).
struct WaitText {
    text: String,
    seconds: f64,
    /// When it stops looking, from the first answer on.
    until: Option<Instant>,
}

/// A call's requests, one at a time, while the one sent waits on the core's answer.
struct Sending {
    agent: String,
    connection: u64,
    tab: u64,
    /// Whether the user answered the call's question, the one way its changes go.
    answered: bool,
    sent: Option<request::Request>,
    /// The requests still to send.
    left: std::collections::VecDeque<request::Request>,
    /// What each answered.
    answers: Vec<String>,
    /// What's left of what the call's answers may show, and how many secrets they hid.
    shown: usize,
    hidden: usize,
    /// Whether the tab's page, as the agent's last outline of it saw it, has a password field,
    /// which its answer says, as a site's answers may then be those to someone signed out.
    signs_in: bool,
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

/// An upload whose file dialog the user opened (Agents::choose_file).
struct Choosing {
    site: String,
    act: Act,
    /// Drops the field once HANDOFF_WAIT passes with no file chosen; an end drops it.
    _timer: Task<()>,
}

/// An entry of the activity log: what an agent did, or was refused, where and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    pub at: SystemTime,
    /// The agent's id.
    pub agent: SharedString,
    /// The server's connection whose session it's of, which that session's record takes, or 0 for
    /// none: the client's connecting, the user's revoking, a ⌘T task, the `ilo` command.
    pub connection: u64,
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
    /// What the agent said a request does, in its own words, without their secrets (#3548).
    pub said: SharedString,
    /// The access of the task it's of as it was judged, by its name, or empty for none.
    pub tier: SharedString,
    /// The site whose notes its answer carried (#4122), by its note's name, or empty.
    pub notes: SharedString,
}

static NEXT_ENTRY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A new entry's id: the log's entries are made in more than one place.
pub fn entry_id() -> u64 {
    NEXT_ENTRY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The id the log's next entry will take, so what's noted from now on is told from what was
/// before, however close the clock puts them.
pub fn next_entry_id() -> u64 {
    NEXT_ENTRY.load(std::sync::atomic::Ordering::Relaxed)
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

/// The connection a call of `agent`'s on the server's `connection` goes by, as its session's card,
/// default tab and log entries are each connection's own. The `ilo` command's session spans its
/// connections, one a command, and a ⌘T task is one session, so theirs is 0.
fn session_connection(agent: &str, connection: u64) -> u64 {
    match Client::from_id(agent) {
        Some(Client::Command) | None => 0,
        Some(_) => connection,
    }
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
    /// Whether the user's shell looks in the `ilo` command's folder, as Settings › Agents last
    /// asked it.
    command_searched: bool,
    /// The client whose Connect is writing its config, and the client whose last failed, and why.
    connecting: Option<Client>,
    failed: Option<(Client, SharedString)>,
    /// The agents with a session open.
    live: HashSet<String>,
    log: Vec<Activity>,
    /// The reads waiting on the core, by request.
    reading: HashMap<u64, Reading>,
    next_request: u64,
    /// The tab each agent used last on each of its connections (`session_connection`), which a
    /// call without a tab on that connection uses.
    current: HashMap<(String, u64), u64>,
    /// The tabs agents opened, which one may close past its page's unsaved changes: in any other,
    /// they may be the user's.
    opened: HashSet<u64>,
    /// The closes waiting on their page's answer, by tab: one answer for each tab.
    closing: HashMap<u64, Vec<Closing>>,
    /// Why each session's page that left its agent's Space went, and whose it was, for that
    /// agent's next call on it (#3660).
    gone: HashMap<u64, (String, Gone)>,
    /// The actions waiting on the core, by request.
    acting: HashMap<u64, Acting>,
    /// The calls whose requests wait on the core, by the request sent.
    sending: HashMap<u64, Sending>,
    /// What waits on each saved read under way, by read.
    saved_reading: HashMap<u64, ReadDone>,
    /// Each tab's refs, from its outlines.
    refs: HashMap<u64, page::Refs>,
    /// Each tab's latest outline, and the last one an agent had, which its next answer gives the
    /// changes since.
    outlines: HashMap<u64, page::Outline>,
    had: HashMap<u64, (String, page::Outline)>,
    /// The site notes each task has had (#4122), and Settings › Agents' Site notes turned off.
    noted: notes::Given,
    notes_off: bool,
    /// Settings › Agents' Sign in for agents (#4098), and the sign-ins the core is doing, by their
    /// request.
    sign_in_on: bool,
    signing: HashMap<u64, signing::Signing>,
    /// The log's last entry, by its id.
    logged: u64,
    /// The actions waiting on the user, the first asked now.
    asking: Vec<Asking>,
    /// The uploads waiting on the core's description of their file field, by request.
    describing: HashMap<u64, Act>,
    /// The uploads whose file dialog the user opened, by field, until file_field_done.
    choosing: HashMap<u64, Choosing>,
    /// The downloads the user let agents' pages make, by download.
    downloading: HashMap<u64, Downloading>,
    /// How the downloads that ended while their question waited ended, by download: a small file
    /// can come whole before the core's pause holds it, and its yes then answers at once.
    ended: HashMap<u64, DownloadEnd>,
    /// The sites where each agent may (true) or may never (false) do what asks first.
    grants: HashMap<String, HashMap<String, bool>>,
    /// The hosts the user fenced, where agents always ask first (fence.rs), as they added them.
    fenced: Vec<String>,
    /// The tabs the user let stay under a fence's entry, by tab, until their page is off it.
    cleared: HashMap<u64, Cleared>,
    /// What each agent did last, as the log says it.
    doing: HashMap<String, SharedString>,
    /// The tabs the user holds from agents, by tab.
    held: HashMap<u64, Held>,
    /// The tabs whose page shows its own dialog, such as its alert(), by tab, as the core reports
    /// it (`page_dialog`), with who asks for a sign-in: nothing more is done on one until it's
    /// gone (#3536).
    dialogs: HashMap<u64, (DialogKind, String)>,
    /// The runs of steps under way, each waiting on its step's answer.
    runs: Vec<Run>,
    /// The agent and connection whose run's step is starting, whose log entries go under the run's.
    stepping: Option<(String, u64)>,
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
    connection: u64,
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
    /// A check step's one look, which answers in one line.
    check: bool,
    seconds: f64,
    started: Instant,
    until: Instant,
    /// The next look's answer, for the timer to give the core.
    look: Option<mpsc::Sender<ToolResult>>,
    /// The next look, which goes with the run, as a run stopped or gone looks no more.
    timer: Option<Task<()>>,
    /// Its own entry in the log, by its id, or 0 if it has none.
    entry: u64,
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
/// own settling after an action takes, at first, then a tenth of the time it has waited, up to
/// LOOK_MOST, as a wait of minutes is for something slow.
const LOOK_EVERY: Duration = Duration::from_millis(500);
const LOOK_MOST: Duration = Duration::from_secs(5);

/// What a call without a tab is told when its agent's Space has none.
const NO_TAB: &str = "You have no tab open yet. Open one with open_tab.";

/// What an answer starts with when the core stopped waiting for its page to settle.
const BUSY: &str = "The page was still busy after 5 s, so it may change yet.";
/// What it starts with when only requests the page made were still open then, as a page that polls
/// keeps one, while what it shows held still: no reason to wait for it (#2355).
const REQUESTS: &str = "The page held still, with requests it made still open after 5 s.";

/// A tab the user holds from its agent: handed to them, or taken over.
struct Held {
    agent: String,
    /// The connection of the call that asked for it, else the one whose card has the tab
    /// (`holder`), which its log entries go by.
    connection: u64,
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
    /// What the answer of a call that asked for no handoff starts with, as it waits on one: why
    /// the browser handed the tab off, for its page's sign-in dialog (`sign_in_hand_off`) or a
    /// saved login's sign-in that's the user's step (`sign_in_yours`).
    told: Option<String>,
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
            command_searched: true,
            connecting: None,
            failed: None,
            live: HashSet::new(),
            log: Vec::new(),
            reading: HashMap::new(),
            next_request: 1,
            current: HashMap::new(),
            opened: HashSet::new(),
            closing: HashMap::new(),
            gone: HashMap::new(),
            acting: HashMap::new(),
            describing: HashMap::new(),
            choosing: HashMap::new(),
            sending: HashMap::new(),
            saved_reading: HashMap::new(),
            refs: HashMap::new(),
            outlines: HashMap::new(),
            had: HashMap::new(),
            noted: notes::Given::default(),
            notes_off: false,
            sign_in_on: signing::STARTS_ON,
            signing: HashMap::new(),
            logged: 0,
            asking: Vec::new(),
            downloading: HashMap::new(),
            ended: HashMap::new(),
            grants: HashMap::new(),
            fenced: Vec::new(),
            cleared: HashMap::new(),
            doing: HashMap::new(),
            pins: HashMap::new(),
            held: HashMap::new(),
            dialogs: HashMap::new(),
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
        let fenced = saved.get("fenced").and_then(Value::as_array);
        self.fenced = Vec::new();
        for host in fenced.into_iter().flatten().filter_map(Value::as_str) {
            if let Some(host) = fence::host_of(host).filter(|host| !self.fenced.contains(host)) {
                self.fenced.push(host);
            }
        }
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
        self.tasks.kept.off = tasks.get("keep").and_then(Value::as_bool) == Some(false);
        self.tasks.kept.told = tasks.get("kept_told").and_then(Value::as_bool) == Some(true);
        // The conversations kept for the tasks of a run before go, as its tasks went (#4105): now
        // that the assistants found are known, as each removes its own.
        self.remove_old_kept(cx);
        self.notes_off = saved.get("notes").and_then(Value::as_bool) == Some(false);
        self.sign_in_on = (saved.get("sign_in").and_then(Value::as_bool)).unwrap_or(signing::STARTS_ON);
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

    /// Whether Settings › Agents' Site notes is on, as it is unless the user turned it off.
    pub fn notes_on(&self) -> bool {
        !self.notes_off
    }

    /// Settings › Agents' Site notes. Off, no answer carries a site's notes from then on.
    pub fn set_notes_on(&mut self, on: bool, cx: &mut Context<Self>) {
        if on != self.notes_off {
            return;
        }
        self.notes_off = !on;
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
        self.look_for_command(cx);
        let found: Vec<Client> = (Client::APPS.into_iter())
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
        (Client::APPS.into_iter())
            .filter(|client| {
                *client == Client::Other
                    || self.found.contains(client)
                    || self.is_connected(*client)
            })
            .collect()
    }

    /// Whether Settings › Agents offers the `ilo` command.
    pub fn command_offered(&self) -> bool {
        cfg!(any(unix, windows))
    }

    /// In a Flatpak, the line the `ilo` command's row gives to run on the host, since the sandbox
    /// can't write the user's bin folder (cli::flatpak_line).
    pub fn command_flatpak_line(&self) -> Option<String> {
        cli::flatpak_line(&self.bridge)
    }

    /// The folder the `ilo` command's row names on Windows, where the command is beside the
    /// bridge's and not in ~/.local/bin.
    pub fn command_windows_folder(&self) -> Option<String> {
        if !cfg!(windows) {
            return None;
        }
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let bridge = std::path::Path::new(self.bridge.first()?);
        let install = cli::Install::windows(bridge, &local, Default::default());
        Some(cli::shown(&install.folder, &local))
    }

    /// The `ilo` command's row: not added, or added in a folder the user's shell looks in or not.
    /// On Windows, added in a folder no longer on the user's PATH is Add again (#3783). A
    /// Flatpak's sandbox can't ask the host's shell, so there the row gives its line either way.
    pub fn command_state(&self) -> cli::State {
        let added = self.is_connected(Client::Command);
        let flatpak = self.command_flatpak_line().is_some();
        cli::state(added, self.command_searched || flatpak, cfg!(windows))
    }

    /// Asks the user's shell, in the background, whether it looks in the `ilo` command's folder,
    /// as the row says, once it's added: on Windows, whether Add's entry is still on the user's
    /// PATH.
    fn look_for_command(&mut self, cx: &mut Context<Self>) {
        if !self.is_connected(Client::Command) || self.command_flatpak_line().is_some() {
            return;
        }
        let clients = self.clients.clone();
        cx.spawn(async move |agents, cx| {
            let searched = cx
                .background_spawn(async move { clients.searches(&cli::folder(&clients::home())) })
                .await;
            agents
                .update(cx, |agents, cx| {
                    // check.sh reads this line.
                    let found = if searched { "found" } else { "not found" };
                    log!("browser: agent ilo-command looked: {found}");
                    if agents.command_searched != searched {
                        agents.command_searched = searched;
                        cx.notify();
                    }
                })
                .ok();
        })
        .detach();
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
                self.note(client.id(), 0, "connected", "", "");
                if client == Client::Command {
                    self.look_for_command(cx);
                }
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
        self.reading
            .retain(|_, reading| reading.agent != client.id());
        self.forget_waiting(client.id(), None, cx);
        if let Some((server, _)) = &self.server {
            server.disconnect(client.id());
        }
        if self.live.remove(client.id()) {
            self.browser.disconnected(client.id(), cx);
        }
        self.note(client.id(), 0, "disconnected", "", "");
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
        self.note(agent, self.holder(agent, tab), action, &site, refused);
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
                connection: self.holder(agent, tab),
                call: 0,
                why,
                question,
                back: None,
                reply: None,
                timer: None,
                told: None,
            };
            self.held.insert(tab, held);
            self.carry(tab, held::Event::Passkey, life, actions, cx);
        }
        if why != "disconnected" {
            let site = site(&self.browser.tab(tab, cx).url);
            self.note(agent, self.holder(agent, tab), why, &site, "");
            cx.notify();
            self.fields_left(tab, cx);
        }
    }

    /// The core's answer to the read `request`: the page's text, or None if it had none.
    pub fn text_read(&mut self, request: u64, text: Option<String>, cx: &mut Context<Self>) {
        let Some(Reading {
            agent,
            connection,
            entry,
            tab: id,
            reply,
            note,
            from,
        }) = self.reading.remove(&request)
        else {
            return;
        };
        // Read once it loaded, as the page tools look once they've acted: the tab may have gone
        // somewhere agents may not, and the core reads any page. Loaded, a page still on the empty
        // start page is no web page either, as the core's page tools find.
        let refused = self.off_limits(&agent, id, cx);
        let came = self.came_to(id, cx).map(|site| fence::withheld(id, &site));
        let tab = self.browser.tab(id, cx);
        let refused = refused.or_else(|| (!is_web(&tab.url)).then_some(NOT_WEB_PAGE));
        // The page's start alone, as a read from further on goes on with one the agent has.
        let notes = match (&text, refused, &came, from) {
            (Some(_), None, None, 0) => self.notes_now(&agent, connection, entry, id, cx),
            _ => None,
        };
        _ = reply.send(match (text, refused, came) {
            (Some(_), Some(why), _) => ToolResult::error(why),
            (Some(_), None, Some(came)) => ToolResult::text(match note.is_empty() {
                true => came,
                false => format!("{note} {came}"),
            }),
            (Some(text), None, None) => {
                let text = tools::page_text(&tab, &text, from);
                let text = match note.is_empty() {
                    true => text,
                    false => format!("{note}\n\n{text}"),
                };
                ToolResult::noted(text, notes)
            }
            (None, _, _) => ToolResult::error(
                "The tab has no page loaded: it loads once the user shows it. Try another tab.",
            ),
        });
        self.advance(cx);
    }

    /// What the server's threads hand over.
    pub fn handle(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Hello {
                agent,
                client,
                connection,
            } => {
                log!("browser: agent {agent} hello from {client}");
                self.hello(&agent, session_connection(&agent, connection), &client);
                if self.live.insert(agent.clone()) {
                    self.note(&agent, 0, "connected", "", "");
                    cx.notify();
                }
            }
            // A connection closing ends its own session, and drops what waits for it, alone.
            Event::Closed { agent, connection } => {
                let connection = session_connection(&agent, connection);
                if connection != 0 {
                    self.end_session(&agent, connection, cx);
                    self.forget_waiting(&agent, Some(connection), cx);
                    self.goodbye(&agent, connection);
                }
            }
            // The `ilo` command's connection closes after each command, which isn't its end: its
            // session ends as an idle one does, and it's live till then (Agents::command_gone).
            Event::Bye { agent } if agent == Client::Command.id() => {}
            Event::Bye { agent } => {
                if self.live.remove(&agent) {
                    self.forget_waiting(&agent, None, cx);
                    self.browser.disconnected(&agent, cx);
                    self.note(&agent, 0, "disconnected", "", "");
                    cx.notify();
                }
            }
            Event::Call(mut call) => {
                call.connection = session_connection(&call.agent, call.connection);
                self.call(call, cx)
            }
            Event::Answered {
                agent,
                id,
                at,
                error,
                kind,
                image,
                notes,
            } => self.answered(&agent, id, at, error, kind, image, notes, cx),
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
        // check.sh reads this line.
        log!("browser: agent {} calls {}", call.agent, call.tool);
        self.live.insert(call.agent.clone());
        if let Some(call) = self.fence_call(call, cx) {
            self.dispatch(call, cx);
        }
    }

    /// A call on a tab that came to a site the user put off limits (#3724), or one outside its
    /// task's Sites, is refused, as an open there is. Every other call goes on, as it is.
    fn fence_call(&mut self, call: Call, cx: &mut Context<Self>) -> Option<Call> {
        if !FENCE_ASKS.contains(&call.tool.as_str()) {
            return Some(call);
        }
        let tab = match tools::tab_argument(&call.arguments) {
            Ok(Some(tab)) => tab,
            Ok(None) => match self.default_tab(&call.agent, call.connection, cx) {
                Ok(tab) => tab,
                Err(_) => return Some(call),
            },
            Err(_) => return Some(call),
        };
        // A tab the user holds, one agents may not use, or one not the agent's, is refused as it
        // is, saying nothing.
        let space = self.browser.agent_space(&call.agent);
        if self.held.contains_key(&tab)
            || self.off_limits(&call.agent, tab, cx).is_some()
            || !self.tabs_seen(space, cx).iter().any(|seen| seen.id == tab)
        {
            return Some(call);
        }
        let Some((site, entry)) =
            (self.came_to(tab, cx)).and_then(|site| Some((site.clone(), self.fence_of(&site)?)))
        else {
            return self.sites_call(call, tab, cx);
        };
        let reply = call.reply.clone();
        let mut act = act_of(&call.agent, call.connection, call.id, tab, reply);
        act.action = fence::CAME_ACTION;
        act.what = fence::arrival(&site);
        act.fenced = Fenced::Came(entry, Box::new(call));
        self.check(act, site, Some(FENCED), cx);
        None
    }

    /// Runs the call, past the fence.
    fn dispatch(&mut self, call: Call, cx: &mut Context<Self>) {
        let Call {
            id,
            agent,
            connection,
            tool,
            arguments,
            reply,
            at,
            ..
        } = call;
        match tool.as_str() {
            "list_tabs" => {
                // Its first call makes its Space, which the core then lists.
                self.space(&agent, cx);
                // A tab of its own off the web is listed by its number alone, so it can close it,
                // and one the user holds by its number and theirs, so where they go stays theirs.
                let tabs = self.browser.agent_tabs(&agent, cx);
                // A tab that came to a fenced site is listed without its page's title or address.
                let came: HashSet<u64> = (tabs.iter())
                    .filter(|tab| self.came_to(tab.id, cx).is_some())
                    .map(|tab| tab.id)
                    .collect();
                let tabs: Vec<TabInfo> = (tabs.into_iter())
                    .filter_map(|tab| match self.browser.off_limits(tab.id, cx) {
                        None | Some(NOT_WEB_PAGE) if self.held.contains_key(&tab.id) => {
                            Some(TabInfo {
                                title: WITH_THE_USER.into(),
                                url: "".into(),
                                ..tab
                            })
                        }
                        // Whatever lists it: Browser::tab says it's withheld, but a host's
                        // list of a Space's tabs may not ask it.
                        None if self.browser.withheld(tab.id) => Some(TabInfo {
                            title: WITHHELD.into(),
                            url: "".into(),
                            ..tab
                        }),
                        None if came.contains(&tab.id) => Some(TabInfo {
                            title: fence::LISTED.into(),
                            url: "".into(),
                            ..tab
                        }),
                        None => Some(tab),
                        Some(NOT_WEB_PAGE) => Some(TabInfo {
                            title: OFF_THE_WEB.into(),
                            url: "".into(),
                            ..tab
                        }),
                        Some(_) => None,
                    })
                    .collect();
                self.note(&agent, connection, "list_tabs", "", "");
                cx.notify();
                let mine: Vec<u64> = (self.pins.iter())
                    .filter(|(_, by)| **by == agent)
                    .map(|(&tab, _)| tab)
                    .collect();
                let current = self.current.get(&(agent.clone(), connection)).copied();
                _ = reply.send(ToolResult::text(tools::tab_list(&tabs, current, &mine)));
            }
            "read_page" => {
                let tab = match tools::tab_argument(&arguments) {
                    Ok(Some(tab)) => tab,
                    Ok(None) => match self.default_tab(&agent, connection, cx) {
                        Ok(tab) => tab,
                        // Its one refusal of that kind: the agent has no tab at all.
                        Err(refused) if refused.kind == Some(tools::Kind::NotFound) => {
                            _ = reply.send(ToolResult::text(tools::tab_list(&[], None, &[])));
                            return;
                        }
                        Err(refused) => {
                            _ = reply.send(refused.into());
                            return;
                        }
                    },
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                let from = match arguments.get("from") {
                    None | Some(Value::Null) => 0,
                    Some(from) => match from.as_u64() {
                        Some(from) => from as usize,
                        None => {
                            _ = reply.send(ToolResult::error(
                                "from is a character of the page's text to start at, from 0.",
                            ));
                            return;
                        }
                    },
                };
                let before = next_entry_id();
                if !self.act(&agent, connection, tab, "read_page", &reply, cx) {
                    return;
                }
                let note = String::new();
                self.read_page(Reading {
                    agent,
                    connection,
                    entry: Some(self.logged)
                        .filter(|entry| *entry >= before)
                        .unwrap_or(0),
                    tab,
                    reply,
                    note,
                    from,
                });
            }
            "open_tab" => {
                let Some(url) = self.url_argument(&arguments, &reply) else {
                    return;
                };
                let Some(wait) = wait_text(&arguments, &reply) else {
                    return;
                };
                let show = match Show::answer(&arguments) {
                    Ok(show) => show.whole(),
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                let host = site(&url);
                let mut act = Act {
                    agent: agent.clone(),
                    connection,
                    call: id,
                    action: "open_tab",
                    tab: 0,
                    what: format!("open {host}"),
                    asks: String::new(),
                    did: None,
                    fenced: Fenced::No,
                    url,
                    page: PageAction::Quiet,
                    element: 0,
                    text: String::new(),
                    note: String::new(),
                    show,
                    download: 0,
                    file: None,
                    field: String::new(),
                    requests: Vec::new(),
                    wait,
                    reply,
                };
                if !is_web(&act.url) {
                    let refused = Refused::of(tools::Kind::NotWebPage, WEB_ONLY);
                    return self.refuse(act, &host, refused, cx);
                }
                let risk = self.opening(&mut act, &host);
                self.check(act, host, risk, cx);
            }
            "navigate" | "snapshot" | "click" | "type" | "select" | "press_key" | "scroll"
            | "screenshot" => {
                let tab = match tools::tab_argument(&arguments) {
                    Ok(Some(tab)) => tab,
                    Ok(None) => match self.default_tab(&agent, connection, cx) {
                        Ok(tab) => tab,
                        Err(refused) => {
                            _ = reply.send(refused.into());
                            return;
                        }
                    },
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                let show = match tool.as_str() {
                    "navigate" => Show::answer(&arguments),
                    _ => Show::argument(&arguments),
                };
                let show = match show {
                    Ok(show) => show,
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                self.page_tool(
                    &agent, connection, id, &tool, tab, &arguments, show, reply, cx,
                );
            }
            "run_steps" => {
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                let steps = arguments.get("steps").and_then(Value::as_array).cloned();
                let steps = steps.unwrap_or_default();
                let action = |step: &Value| {
                    step.get("action")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                };
                let wrong = steps.iter().position(|step| {
                    !action(step).is_some_and(|action| tools::STEPS.contains(&action.as_str()))
                });
                if steps.is_empty() || steps.len() > tools::MAX_STEPS || wrong.is_some() {
                    _ = reply.send(ToolResult::error(format!(
                        "steps is 1 to {} steps, each with an action: {}.",
                        tools::MAX_STEPS,
                        tools::STEPS.join(", ")
                    )));
                    return;
                }
                // Its picture is the run's answer, so nothing comes after it.
                let pictures = steps.iter().rev().skip(1);
                if pictures
                    .filter_map(action)
                    .any(|action| action == "screenshot")
                {
                    _ = reply.send(ToolResult::error(
                        "A screenshot is only the last step, as its picture is the run's answer.",
                    ));
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
                self.note(&agent, connection, "run_steps", &host, "");
                let (_, step) = mpsc::channel();
                let (steps, done, wait) = (steps.into(), Vec::new(), None);
                let run = Run {
                    agent,
                    connection,
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
            "upload" => {
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                self.upload(&agent, connection, id, tab, &arguments, reply, cx);
            }
            "hand_off" => {
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
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
                if !self.act(&agent, connection, tab, "hand_off", &reply, cx) {
                    return;
                }
                let (life, actions) = self.held_step(tab, held::Event::HandOff);
                if !self.browser.hand_off(&agent, tab) || life != held::Life::HandedOff {
                    _ = reply.send(ToolResult::failed(
                        tools::Kind::Yours,
                        "The tab is the user's already.",
                    ));
                    return;
                }
                // check.sh reads this line.
                log!("browser: agent {agent} hands off {tab}: {why}");
                let question = self.next_request;
                self.next_request += 1;
                let why = page::clip(why, 200);
                let held = Held {
                    agent,
                    connection,
                    call: id,
                    why,
                    question,
                    back: None,
                    reply: Some(reply),
                    timer: None,
                    told: None,
                };
                self.held.insert(tab, held);
                self.carry(tab, held::Event::HandOff, life, actions, cx);
            }
            "sort_tabs_by_site" if !is_task(&agent) => {
                self.sort_by_site(&agent, connection, reply, cx)
            }
            // The agent's word that its task is done (#3652): it ends only its own card, so it
            // needs no permission, and the activity log says so.
            "done" if !is_task(&agent) => {
                let said = self.said_done(&agent, id, cx);
                match &said {
                    Ok(text) if text.starts_with("Your task") => {
                        self.note(&agent, connection, "done", "", "")
                    }
                    Err(why) => self.note(&agent, connection, "done", "", why),
                    Ok(_) => {}
                }
                cx.notify();
                _ = reply.send(match said {
                    Ok(text) => ToolResult::text(text),
                    Err(why) => ToolResult::error(why),
                });
            }
            "pin_tab" | "unpin_tab" | "remove_tab" if !is_task(&agent) => {
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                self.pin(&agent, connection, &tool, tab, reply, cx);
            }
            "close_tab" => {
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                let discard = arguments.get("discard").and_then(Value::as_bool) == Some(true);
                self.close_tab(&agent, connection, id, tab, discard, reply, cx);
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
                    if !self.act(&agent, connection, tab, "split_tabs", &reply, cx) {
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
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                let Some(split) = arguments.get("split").and_then(tools::tab_id) else {
                    _ = reply.send(ToolResult::error(
                        "split is a tab of the split view, a number from list_tabs.",
                    ));
                    return;
                };
                if !self.act(&agent, connection, split, "add_to_split", &reply, cx)
                    || !self.act(&agent, connection, tab, "add_to_split", &reply, cx)
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
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                if !self.act(&agent, connection, tab, "separate_tab", &reply, cx) {
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
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                if !self.act(&agent, connection, tab, "list_split", &reply, cx) {
                    return;
                }
                let split = self.browser.split(tab, cx);
                _ = reply.send(ToolResult::text(tools::split_list(tab, &split)));
            }
            "ask_for_tab" => {
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                // It asks for the tab back without the core's check, which may_act's would add. A
                // tab the user doesn't hold gets NOT_HELD, which says nothing of its page.
                if let Some(why) = self
                    .held
                    .contains_key(&tab)
                    .then(|| self.off_limits(&agent, tab, cx))
                    .flatten()
                {
                    let site = site(&self.browser.tab(tab, cx).url);
                    self.note(&agent, connection, "ask_for_tab", &site, why);
                    _ = reply.send(off_limits_refused(why).into());
                    return;
                }
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
                let (refused, kind) = if actions.has(held::Actions::NOT_HELD) {
                    (NOT_HELD, Some(tools::Kind::Yours))
                } else if actions.has(held::Actions::WAITING) {
                    ("You're waiting on the user for this tab already.", None)
                } else {
                    ("", None)
                };
                self.note(&agent, connection, "ask_for_tab", &site, refused);
                if !refused.is_empty() {
                    _ = reply.send(ToolResult {
                        kind,
                        ..ToolResult::error(refused)
                    });
                    return;
                }
                // check.sh reads this line.
                log!("browser: agent {agent} asks for {tab} back: {why}");
                let question = self.next_request;
                self.next_request += 1;
                let held = self.held.get_mut(&tab).expect("held, as matched");
                (held.question, held.back) = (question, Some(page::clip(why, 200)));
                (held.connection, held.call, held.reply) = (connection, id, Some(reply));
                self.carry(tab, held::Event::Ask { holder }, life, actions, cx);
            }
            "request" => {
                let Some(tab) = self.tab_for(&agent, connection, &arguments, &reply, cx) else {
                    return;
                };
                let host = site(&self.browser.tab(tab, cx).url);
                // Before the request is read, so its answer says nothing of a page off limits. Nor
                // of a tab not the agent's, which the core answers as any other.
                if let Some(why) = self.off_limits(&agent, tab, cx) {
                    self.note(&agent, connection, "request", &host, why);
                    cx.notify();
                    _ = reply.send(off_limits_refused(why).into());
                    return;
                }
                if !self.owns(&agent, tab, cx) {
                    if let Err(refused) = self.may_act(&agent, tab, "request", cx) {
                        let logged = self.log_words(tab, &refused.why, cx);
                        self.note(&agent, connection, "request", &host, &logged);
                        cx.notify();
                        _ = reply.send(refused.into());
                        return;
                    }
                }
                let requests = match request::parse(&arguments, &self.browser.tab(tab, cx).url) {
                    Ok(requests) => requests,
                    Err(request::Wrong::Refused(why)) => {
                        self.note(&agent, connection, "request", &host, &why);
                        cx.notify();
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                    Err(request::Wrong::Malformed(why)) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                // On a site the user fenced, its reads ask too: each carries the site's cookies.
                let fenced = self.fence_of(&host).is_some();
                let what = match requests.iter().filter(|each| each.changes()).count() {
                    0 if fenced => fence::requests(requests.len(), &host),
                    0 => String::new(),
                    1 => format!("make a change on {host}"),
                    changes => format!("make {changes} changes on {host}"),
                };
                let act = Act {
                    agent,
                    connection,
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
                    asks: String::new(),
                    field: String::new(),
                    download: 0,
                    file: None,
                    requests,
                    wait: None,
                    did: None,
                    fenced: Fenced::No,
                    reply,
                };
                // Reads go; changes go as the task's access allows.
                match (act.what.is_empty(), fenced) {
                    (true, _) => self.run(act, cx),
                    (false, true) => self.check(act, host, Some(FENCED), cx),
                    (false, false) => self.check(act, host, Some(CHANGES_ASK), cx),
                }
            }
            "sign_in" => self.sign_in_call(&agent, connection, id, &arguments, reply, cx),
            "title" => self.name_task(&agent, connection, id, &arguments, at, reply, cx),
            "ask" | "finish" if is_task(&agent) => {
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
            None => (self.given_name(agent)).unwrap_or_else(|| name(agent).to_string().into()),
        }
    }

    /// Starts the run's next step, which answers it through `advance`.
    fn next_step(&mut self, mut run: Run, cx: &mut Context<Self>) {
        let Some(step) = run.steps.pop_front() else {
            return;
        };
        // A step took the tab to a fenced site: the run stops there, saying nothing of its page.
        if let Some(site) = self.came_to(run.tab, cx) {
            let stopped = fence::run_stopped(run.done.len() + 1, run.tab, &site);
            let answer = format!("{stopped}{}", run_so_far(&run.done));
            _ = run.reply.send(ToolResult::error(answer));
            return;
        }
        let tool = step
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let (reply, answer) = mpsc::channel();
        run.step = answer;
        self.trace_step(&run.agent, run.connection, run.call, run.tab, &step, cx);
        if tool == "wait" || tool == "check" {
            return self.wait(run, &step, reply, cx);
        }
        self.stepping = Some((run.agent.clone(), run.connection));
        let show = run.show_now();
        self.page_tool(
            &run.agent.clone(),
            run.connection,
            run.call,
            &tool,
            run.tab,
            &step,
            show,
            reply,
            cx,
        );
        self.stepping = None;
        self.runs.push(run);
    }

    /// Starts a wait step, or a check step, which looks once: checked as a snapshot, which each of
    /// its looks is, but logged as itself.
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
        let check = string("action") == "check";
        let given = step.get("seconds").and_then(Value::as_f64);
        let what = match (string("text"), string("gone"), string("until").as_str()) {
            (text, gone, "") if check => (!text.is_empty() && gone.is_empty() && given.is_none())
                .then_some(WaitFor::Shows(text)),
            (text, gone, "") if text.is_empty() && gone.is_empty() => Some(WaitFor::Time),
            (text, gone, "") if gone.is_empty() => Some(WaitFor::Shows(text)),
            (text, gone, "") if text.is_empty() => Some(WaitFor::Goes(gone)),
            (text, gone, "quiet") if text.is_empty() && gone.is_empty() => Some(WaitFor::Quiet),
            _ => None,
        };
        // Text can take minutes to come, as a search's results; the page settles in seconds.
        let most = match what {
            Some(WaitFor::Shows(_) | WaitFor::Goes(_)) => tools::MAX_TEXT_WAIT,
            _ => tools::MAX_WAIT,
        };
        let seconds = match given {
            _ if check => Some(0.0),
            None if !matches!(what, Some(WaitFor::Time)) => Some(tools::WAIT),
            seconds => seconds.filter(|seconds| *seconds > 0.0 && *seconds <= most),
        };
        let (Some(what), Some(seconds)) = (what, seconds) else {
            let why = match check {
                true => "check needs text to look for, once; to wait for it, use wait.".into(),
                false => format!(
                    "wait needs text to wait for, or gone with text to wait to go, up to seconds, \
                     at most {}; until \"quiet\" to wait for the page to settle, or seconds alone, \
                     at most {}.",
                    tools::MAX_TEXT_WAIT,
                    tools::MAX_WAIT
                ),
            };
            _ = reply.send(ToolResult::error(why));
            return self.runs.push(run);
        };
        self.stepping = Some((run.agent.clone(), run.connection));
        let logged = if check { "check" } else { "wait" };
        let before = next_entry_id();
        let may = self.act_as(
            &run.agent,
            run.connection,
            run.tab,
            "snapshot",
            logged,
            &reply,
            cx,
        );
        self.stepping = None;
        let entry = Some(self.logged).filter(|entry| *entry >= before);
        if may {
            let id = self.next_request;
            self.next_request += 1;
            let time = Duration::from_secs_f64(seconds);
            let started = cx.background_executor().now();
            let until = started + time;
            let mut wait = Wait {
                id,
                what,
                check,
                seconds,
                started,
                until,
                look: None,
                timer: None,
                entry: entry.unwrap_or(0),
            };
            match wait.what {
                WaitFor::Time => {
                    (wait.look, wait.timer) = (Some(reply), Some(self.look_after(id, time, cx)))
                }
                WaitFor::Quiet => self.look(
                    &run.agent,
                    run.connection,
                    run.tab,
                    PageAction::Quiet,
                    reply,
                ),
                _ => self.look(
                    &run.agent,
                    run.connection,
                    run.tab,
                    PageAction::Snapshot,
                    reply,
                ),
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
        let (agent, connection) = (run.agent.clone(), run.connection);
        let (tab, seconds, entry) = (run.tab, wait.seconds, wait.entry);
        // The tab came to a fenced site while it waited: the wait ends with that alone, whatever
        // the page shows or does, so no try tells the agent anything of it (#3724).
        if let Some(site) = self.came_to(tab, cx) {
            return Some(ToolResult::text(fence::withheld(tab, &site)));
        }
        let show = run.show_now();
        let outline = self
            .outlines
            .get(&tab)
            .map(page::Outline::text)
            .unwrap_or_default();
        let now = cx.background_executor().now();
        let late = now >= wait.until;
        // The core waits for the page to settle itself, so a quiet wait looks again at once.
        let every = (now.saturating_duration_since(wait.started) / 10).clamp(LOOK_EVERY, LOOK_MOST);
        let mut next = every.min(wait.until.saturating_duration_since(now));
        match &wait.what {
            WaitFor::Time => {
                let note = format!("Waited {seconds} s.");
                return Some(self.page_answer(&agent, connection, entry, tab, note, show, cx));
            }
            WaitFor::Quiet => match answer.text.starts_with(BUSY) {
                // What it shows held still, though a poll's request may never end.
                false if answer.text.starts_with(REQUESTS) => {
                    let note = REQUESTS.to_string();
                    return Some(self.page_answer(&agent, connection, entry, tab, note, show, cx));
                }
                false => {
                    let note = "The page settled.".to_string();
                    return Some(self.page_answer(&agent, connection, entry, tab, note, show, cx));
                }
                // A page that never settles, as one that polls, is no reason to stop the run.
                true if late => {
                    let note = format!("The page was still busy after {seconds} s.");
                    return Some(self.page_answer(&agent, connection, entry, tab, note, show, cx));
                }
                true => next = Duration::ZERO,
            },
            WaitFor::Shows(text) | WaitFor::Goes(text) => {
                let shows = outline.to_lowercase().contains(&text.to_lowercase());
                let goes = matches!(wait.what, WaitFor::Goes(_));
                let text = page::clip(text, 60);
                // A check answers in its one line, or fails with the outline, as a wait does.
                if wait.check {
                    return Some(match shows {
                        true => ToolResult::text(format!("“{text}” is on the page.")),
                        false => {
                            let why = format!("“{text}” isn't on the page.");
                            ToolResult::error(self.answer_with(&agent, tab, why, run.show, cx))
                        }
                    });
                }
                // What it said, and whether it failed, with the kind a card says it in.
                let said = match (goes, shows) {
                    (false, true) => Some((format!("“{text}” showed."), None)),
                    (true, false) => Some((format!("“{text}” went."), None)),
                    (false, false) if late => Some((
                        format!("“{text}” didn't show in {seconds} s."),
                        Some(Some(tools::Kind::DidntShow)),
                    )),
                    (true, true) if late => Some((
                        format!("“{text}” was still there after {seconds} s."),
                        Some(None),
                    )),
                    _ => None,
                };
                match said {
                    // The run ends here, so with the outline as its last step's.
                    Some((why, Some(kind))) => {
                        let show = run.show;
                        let why = self.answer_with(&agent, tab, why, show, cx);
                        return Some(ToolResult {
                            kind,
                            ..ToolResult::error(why)
                        });
                    }
                    Some((note, None)) => {
                        return Some(
                            self.page_answer(&agent, connection, entry, tab, note, show, cx),
                        );
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
            _ = agents.update(cx, |agents, cx| {
                let waiting = agents.runs.iter_mut().find_map(|run| {
                    let wait = run.wait.as_mut().filter(|wait| wait.id == id)?;
                    let action = match wait.what {
                        WaitFor::Quiet => PageAction::Quiet,
                        _ => PageAction::Snapshot,
                    };
                    let (agent, connection) = (run.agent.clone(), run.connection);
                    Some((agent, connection, run.tab, action, wait.look.take()?))
                });
                let Some((agent, connection, tab, action, look)) = waiting else {
                    return;
                };
                // A page on a fenced site the tab came to isn't read for the agent (#3724): the
                // wait ends at its next look, unread.
                if agents.came_to(tab, cx).is_some() {
                    _ = look.send(ToolResult::text(String::new()));
                    return agents.advance(cx);
                }
                agents.stepping = Some((agent.clone(), connection));
                let may = agents.may_look_again(&agent, connection, tab, "wait", &look, cx);
                agents.stepping = None;
                match may {
                    true => agents.look(&agent, connection, tab, action, look),
                    // Its run ends on the refusal, as on any step's.
                    false => agents.advance(cx),
                }
            });
        })
    }

    /// Asks the core for the page's tree, at once or once it has settled, for a wait, which
    /// checked and logged itself.
    fn look(
        &mut self,
        agent: &str,
        connection: u64,
        tab: u64,
        action: PageAction,
        reply: mpsc::Sender<ToolResult>,
    ) {
        let request = self.next_request;
        self.next_request += 1;
        let (note, show) = (String::new(), Show::Nothing);
        let acting = Acting {
            agent: agent.into(),
            connection,
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
            wait: None,
            entry: 0,
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
                        let last = match (&answer.image, answer.text.is_empty()) {
                            (Some(_), true) => "Took a screenshot of the page.",
                            _ => answer.text.as_str(),
                        };
                        let text = format!("Ran {n} steps.\n{done}{n}. {last}");
                        _ = run.reply.send(ToolResult { text, ..answer });
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

    /// The tab a call names, else its default (`default_tab`); or None, having answered why.
    fn tab_for(
        &mut self,
        agent: &str,
        connection: u64,
        arguments: &Value,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let refused = match tools::tab_argument(arguments) {
            Ok(Some(tab)) => return Some(tab),
            Ok(None) => match self.default_tab(agent, connection, cx) {
                Ok(tab) => return Some(tab),
                Err(refused) => refused,
            },
            Err(why) => Refused::new(why),
        };
        _ = reply.send(refused.into());
        None
    }

    /// Hands `tab` to the user for the sign-in its page asks for, as `agent`'s hand_off would,
    /// with `reply`, for its call `call` that met it, waiting as a handoff's call does: its card,
    /// the log's line and Hand Back are a handoff's. No agent can answer the browser's sign-in
    /// (#3662). Gives `reply` back if the page asks for none, or the tab is the user's already.
    fn sign_in_hand_off(
        &mut self,
        agent: &str,
        connection: u64,
        call: u64,
        tab: u64,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> Result<(), mpsc::Sender<ToolResult>> {
        let Some((DialogKind::SignIn, from)) = self.dialogs.get(&tab) else {
            return Err(reply);
        };
        let why = sign_in_why(from);
        let (life, actions) = self.held_step(tab, held::Event::HandOff);
        if life != held::Life::HandedOff || !self.browser.hand_off(agent, tab) {
            return Err(reply);
        }
        let site = site(&self.browser.tab(tab, cx).url);
        self.note(agent, connection, "hand_off", &site, "");
        // check.sh reads this line.
        log!("browser: agent {agent} hands off {tab}: {why}");
        let question = self.next_request;
        self.next_request += 1;
        let held = Held {
            agent: agent.into(),
            connection,
            call,
            why,
            question,
            back: None,
            reply: Some(reply),
            timer: None,
            told: Some(SIGNED_IN.into()),
        };
        self.held.insert(tab, held);
        self.carry(tab, held::Event::HandOff, life, actions, cx);
        Ok(())
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
            connection: self.holder(agent, tab),
            call: 0,
            why,
            question,
            back: None,
            reply: None,
            timer: None,
            told: None,
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

    /// Keeps why `agent`'s page `tab` left its Space, in place of any reason before, as the
    /// agent's own acts know why. Past GONE_KEPT, the lowest tab id, the oldest, goes.
    fn went(&mut self, tab: u64, agent: &str, gone: Gone) {
        self.gone.insert(tab, (agent.into(), gone));
        while self.gone.len() > GONE_KEPT {
            let Some(&oldest) = self.gone.keys().min() else {
                break;
            };
            self.gone.remove(&oldest);
        }
    }

    /// The core closed `tab` (tabs::tab_closed). A hold on it ends. A session's page that the
    /// agent or the browser didn't close, the user or its own page did.
    pub fn tab_closed(&mut self, tab: u64, cx: &mut Context<Self>) {
        self.sign_in_tab_closed(tab, cx);
        self.dialogs.remove(&tab);
        self.cleared.remove(&tab);
        self.left(tab, Gone::Closed);
        self.let_go(tab, held::Event::Closed, cx);
        self.fields_left(tab, cx);
    }

    /// The core dropped `tab`'s file fields, as its tab closed or its agent's session there ended:
    /// the uploads still asking for them end. One whose dialog is up hears its end from the core.
    fn fields_left(&mut self, tab: u64, cx: &mut Context<Self>) {
        let left = |asking: &mut Asking| asking.act.tab == tab && asking.act.file.is_some();
        let left: Vec<Asking> = self.asking.extract_if(.., left).collect();
        if left.is_empty() {
            return;
        }
        for Asking { site, act, .. } in left {
            self.refuse(act, &site, Refused::new(LEFT_PAGE), cx);
        }
        self.asked(cx);
    }

    /// `tab`'s page shows its own dialog of `kind`, or none any more (pages::tab_dialog), with
    /// its `message`, whose first line names who asks for a sign-in. In an agent's tab it pauses
    /// the agent there, which the log says without the dialog's words.
    pub fn page_dialog(
        &mut self,
        tab: u64,
        kind: Option<DialogKind>,
        message: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(kind) = kind else {
            self.dialogs.remove(&tab);
            return;
        };
        let from = match kind {
            DialogKind::SignIn => message.lines().next().unwrap_or_default().into(),
            _ => String::new(),
        };
        if self.dialogs.insert(tab, (kind, from)).is_some() {
            return;
        }
        let space = self.browser.space_of(tab, cx);
        let agent = (self.live.iter())
            .find(|agent| space != 0 && self.browser.agent_space(agent) == space)
            .cloned();
        if let Some(agent) = agent {
            let site = site(&self.browser.tab(tab, cx).url);
            self.note(&agent, self.holder(&agent, tab), "page_dialog", &site, "");
            cx.notify();
        }
    }

    /// `space`'s tabs changed (spaces::space_changed). A held tab that left its agent's Space
    /// closed, or the user moved it to one of theirs, and its hold ends either way.
    pub fn space_changed(&mut self, space: u64, cx: &mut Context<Self>) {
        // A session's page that left its agent's Space and is still open, the user moved.
        let tabs = self.browser.space_tab_ids(space, cx);
        let moved: Vec<u64> = (self.tasks.tasks.iter())
            .filter_map(|task| task.session.as_ref())
            .filter(|session| self.browser.agent_space(&session.agent) == space)
            .flat_map(|session| session.tabs.iter().copied())
            .filter(|tab| !tabs.contains(tab) && self.browser.space_of(*tab, cx) != 0)
            .collect();
        for tab in moved {
            self.left(tab, Gone::Moved);
        }
        if self.held.is_empty() {
            return;
        }
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
        let (agent, connection) = (self.held.get(&tab)).map_or((String::new(), 0), |held| {
            (held.agent.clone(), held.connection)
        });
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
                let mut answer = answer_of(tab, event);
                // The call that met the sign-in asked for no handoff, so it hears why it waited.
                if let Some(told) = &held.told {
                    answer.text = format!("{told} {}", answer.text);
                }
                _ = reply.send(answer);
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
            self.note(&agent, connection, event.way(), &site, "");
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
        let stopped = || ToolResult::failed(tools::Kind::Stopped, STOPPED);
        for (_, reading) in self.reading.extract_if(|_, reading| reading.agent == agent) {
            _ = reading.reply.send(stopped());
        }
        for (_, acting) in self.acting.extract_if(|_, acting| acting.agent == agent) {
            _ = acting.reply.send(stopped());
        }
        for asking in self
            .asking
            .extract_if(.., |asking| asking.act.agent == agent)
        {
            if let Some(file) = &asking.act.file {
                self.browser.file_field_drop(file.id);
            }
            _ = asking.act.reply.send(stopped());
        }
        for (field, choosing) in
            (self.choosing).extract_if(|_, choosing| choosing.act.agent == agent)
        {
            self.browser.file_field_drop(field);
            _ = choosing.act.reply.send(stopped());
        }
        for (_, act) in self.describing.extract_if(|_, act| act.agent == agent) {
            _ = act.reply.send(stopped());
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
                self.note(agent, 0, "stopped by the user", "", "");
                self.session_stopped(agent);
                // No connection of the `ilo` command's is left open to close (Event::Bye).
                if agent == Client::Command.id() {
                    self.live.remove(agent);
                }
            }
            None => self.note(agent, 0, "stopped", "", ""),
        }
        self.advance(cx);
        self.asked(cx);
    }

    /// The tabs the user holds from `agent`.
    fn held_from(&self, agent: &str) -> Vec<u64> {
        let held = self.held.iter().filter(|(_, held)| held.agent == agent);
        held.map(|(&tab, _)| tab).collect()
    }

    /// What each agent did last, and the tabs the user holds from it, for the bar in its Space and
    /// for a press in its pages (Shell::page_pressed). An agent whose session has ended has none:
    /// its tabs are the user's, with nothing to hand back to (#3503).
    pub fn status(&self) -> Vec<Status> {
        let mut agents: Vec<&str> = (self.doing.keys().map(String::as_str))
            .filter(|agent| self.live.contains(*agent))
            .collect();
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
        connection: u64,
        call: u64,
        tool: &str,
        tab: u64,
        arguments: &Value,
        show: Show,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let host = site(&self.browser.tab(tab, cx).url);
        // Before its ref is looked up, so an outline's refs say nothing of a page off limits.
        if let Some(why) = self.off_limits(agent, tab, cx) {
            self.note(agent, connection, tool, &host, why);
            _ = reply.send(off_limits_refused(why).into());
            return;
        }
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
                    _ = reply.send(ToolResult::failed(
                        tools::Kind::NotFound,
                        format!(
                            "There's no element {name} in tab {tab}. Use a ref from its latest \
                             outline."
                        ),
                    ));
                    return;
                }
            },
            None => None,
        };
        let label = |element: &page::Element| match element.name.is_empty() {
            true => element.role.clone(),
            false => page::curly(&element.name, 60),
        };
        let mut act = Act {
            agent: agent.into(),
            connection,
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
            asks: String::new(),
            download: 0,
            file: None,
            requests: Vec::new(),
            wait: None,
            did: None,
            fenced: Fenced::No,
            reply,
        };
        let mut risk = None;
        // Whether it may click, choose or send on the page, which asks on a fenced site.
        let mut sends = false;
        match tool {
            "navigate" => {
                act.show = show.whole();
                let Some(wait) = wait_text(arguments, &act.reply) else {
                    return;
                };
                act.wait = wait;
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
                            let refused = Refused::of(tools::Kind::NotWebPage, WEB_ONLY);
                            return self.refuse(act, &to, refused, cx);
                        }
                        // What asks is the site it goes to.
                        let risk = self.opening(&mut act, &to);
                        return self.check(act, to, risk, cx);
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
                sends = true;
                act.note = format!("Clicked {} {}.", element.role, label(element));
                act.say(&host, |on| format!("click {}{on}", label(element)));
                let outline = self.outlines.get(&tab);
                let message = outline.is_some_and(|outline| outline.sends_message(element.form));
                risk = page::risk(&host, &element.name, message);
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
                    let refused = Refused::of(tools::Kind::Password, PASSWORDS);
                    return self.refuse(act, &host, refused, cx);
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
                act.say(&host, |on| format!("send {}{on}", page::curly(&text, 60)));
                // Its Enter asks as a click on what it submits would, and else as a message sent.
                // A line break in a rich-text editor is that Enter, as a chat's composer sends
                // on it.
                let outline = self.outlines.get(&tab);
                let broken = text.contains(['\n', '\r'])
                    && outline.is_some_and(|outline| outline.is_editor(element.node));
                let enter = submit || broken;
                sends = enter;
                let pressing = (enter
                    .then(|| page::pressing_risk(&host, outline, "Enter", Some(element))))
                .flatten();
                risk = match pressing {
                    Some(pressing) => {
                        // The control and the site come before the typed text, which a
                        // long one can't then push out of the sidebar's row or the strip.
                        let control = label(&pressing.control);
                        let typed = format!("{} into {}", page::curly(&text, 60), label(element));
                        act.say(&host, |on| match pressing.surely {
                            true => {
                                format!("press Enter to click {control}{on} after typing {typed}")
                            }
                            false => format!(
                                "press Enter, which may click {control}{on}, after typing {typed}"
                            ),
                        });
                        Some(pressing.why)
                    }
                    None => enter.then(|| page::risk(&host, "send", false)).flatten(),
                };
                act.text = text;
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
                sends = true;
                act.field = match element.name.is_empty() {
                    true => element.role.clone(),
                    false => format!("{} {}", element.role, label(element)),
                };
                // A line of the question the user may be asked, as of the answer, so a line break
                // in it is a space there.
                let chose = page::curly(&page::one_line(&option), 60);
                act.note = format!("Chose {chose} in the {}.", act.field);
                act.say(&host, |on| {
                    format!("choose {chose} in {}{on}", label(element))
                });
                // As a click on the field or the option would.
                let outline = self.outlines.get(&tab);
                let message = outline.is_some_and(|outline| outline.sends_message(element.form));
                risk = page::risk(&host, &element.name, message)
                    .or_else(|| page::risk(&host, &option, message));
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
                sends = true;
                act.note = format!("Pressed {key}.");
                act.say(&host, |on| {
                    format!("press Enter, which may send a form{on}")
                });
                // What the key activates asks as a click on it would (GHSA-hqw3-jp9p-3rhj), and
                // Enter else as a message sent.
                let outline = self.outlines.get(&tab);
                risk = match page::pressing_risk(&host, outline, &key, None) {
                    Some(pressing) => {
                        let control = label(&pressing.control);
                        act.say(&host, |on| match pressing.surely {
                            true => format!("press {key} to click {control}{on}"),
                            false => format!("press {key}, which may click {control}{on}"),
                        });
                        Some(pressing.why)
                    }
                    None => (key == "Enter")
                        .then(|| page::risk(&host, "send", false))
                        .flatten(),
                };
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
        // On a site the user fenced, each click, key or form sent asks every time, whatever they
        // answered before (#3724); a payment there still asks as one.
        if sends && risk != Some(page::ORDER) && self.fence_of(&host).is_some() {
            risk = Some(FENCED);
        }
        self.check(act, host, risk, cx);
    }

    /// What an open of `site` asks: on a site the user fenced, always, and once allowed its tab
    /// may stay there (#3724); elsewhere as the site's kind says.
    fn opening(&self, act: &mut Act, site: &str) -> Option<&'static str> {
        match self.fence_of(site) {
            Some(entry) => {
                act.fenced = Fenced::Open(entry);
                Some(FENCED)
            }
            None => page::opening_risk(site),
        }
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

    /// Runs the action, unless what it does on `site` (`why`, what once asked the user first) is
    /// above its task's access: then it's refused at once (access.rs).
    fn check(
        &mut self,
        mut act: Act,
        site: String,
        why: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        act.did = why.and_then(Did::of);
        // The task's Sites come first: no tier lifts them, as none lifts a fence.
        let why = match why != Some(FENCED) && self.off_sites(&act, cx) {
            true => Some(OTHER_SITE),
            false => why,
        };
        let tier = self.tier_of(&act.agent, act.connection, act.call);
        // A site the user answered Never for this agent stays refused at every tier until they
        // revoke it, while Settings still lists it (#4086 step 3 moves these to Off limits).
        let why = Self::why_of(&act, &site, why).map(|why| match why {
            tier::Why::Fenced | tier::Why::OtherSite => why,
            _ if self.said_never(&act.agent, &site) => tier::Why::Never,
            _ => why,
        });
        match why {
            Some(why) if !tier.allows(why) => self.stopped(act, site, why, tier, cx),
            // What the tier allowed has its answer: a request's changes go with it.
            _ if act.action == "request" => self.send(act, true, cx),
            _ => self.run(act, cx),
        }
    }

    /// Puts the question to the user, for ASK_WAIT at most.
    fn put_question(&mut self, question: Question, site: String, act: Act, cx: &mut Context<Self>) {
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

    /// An agent asks the user for a file for the page's file field `ref` (upload): the core
    /// describes the field, for the card, and the agent never learns what the user chose. A tab
    /// the agent may not act in is refused with no card.
    fn upload(
        &mut self,
        agent: &str,
        connection: u64,
        call: u64,
        tab: u64,
        arguments: &Value,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let host = site(&self.browser.tab(tab, cx).url);
        if let Err(refused) = self.may_act(agent, tab, "upload", cx) {
            let logged = self.log_words(tab, &refused.why, cx);
            self.note(agent, connection, "upload", &host, &logged);
            cx.notify();
            _ = reply.send(refused.into());
            return;
        }
        let Some(name) = arguments.get("ref").and_then(Value::as_str) else {
            _ = reply.send(ToolResult::error("upload needs a ref."));
            return;
        };
        let Some(element) = self.refs.get(&tab).and_then(|refs| refs.get(name)) else {
            _ = reply.send(ToolResult::error(format!(
                "There's no element {name} in tab {tab}. Use a ref from its latest outline."
            )));
            return;
        };
        let node = element.node;
        self.current.insert((agent.into(), connection), tab);
        let request = self.next_request;
        self.next_request += 1;
        let mut act = act_of(agent, connection, call, tab, reply);
        (act.action, act.page, act.element) = ("upload", PageAction::FileField, node);
        self.describing.insert(request, act);
        self.act_in_page(agent, tab, request, PageAction::FileField, node, "");
    }

    /// The core described the upload's file field, as JSON, or refused it with why: the card asks
    /// the user for a file for it.
    fn file_field_described(
        &mut self,
        mut act: Act,
        text: String,
        error: Option<Failure>,
        cx: &mut Context<Self>,
    ) {
        let host = site(&self.browser.tab(act.tab, cx).url);
        if let Some(failure) = error {
            self.note(&act.agent, act.connection, "upload", &host, &text);
            cx.notify();
            _ = act.reply.send(ToolResult {
                kind: failure.kind(),
                ..ToolResult::error(text)
            });
            return;
        }
        let field: Value = serde_json::from_str(&text).unwrap_or_default();
        // The core writes ids as JSON doubles, which Chromium's writer gives a ".0".
        let id = field.get("field").and_then(Value::as_f64).unwrap_or(0.0) as u64;
        let label = field.get("label").and_then(Value::as_str).unwrap_or("");
        // The page's words, kept inside the quotes they're shown in.
        let label = page::clip(&request::plain(label), 60);
        let many = field.get("many").and_then(Value::as_bool).unwrap_or(false);
        let site = match field.get("site").and_then(Value::as_str) {
            Some(site) if !site.is_empty() => site.to_string(),
            _ => host,
        };
        let files = if many { "files" } else { "a file" };
        // As on the tab's other actions, its card shows the site apart from the label (#3836):
        // the browser's, the field frame's that gets the file, never the page's words.
        act.say(&site, |on| match label.is_empty() {
            true => format!("add {files} to a page{on}"),
            false => format!("add {files} to “{label}”{on}"),
        });
        act.file = Some(FileField { id, label, many });
        let agent = self.agent_name(&act.agent);
        let question = Question {
            id: self.next_request,
            agent: agent.clone(),
            from: act.agent.clone().into(),
            what: act.what.clone().into(),
            asks: act.asks.clone().into(),
            why: format!("You choose the file. {agent} sees only what the page shows of it.")
                .into(),
            site: site.clone().into(),
            each_time: true,
            file: Some(FileAsk { field: id, many }),
            ..Question::default()
        };
        self.next_request += 1;
        // check.sh reads this line.
        log!(
            "browser: agent {} asks {} upload {}",
            act.agent,
            question.id,
            act.tab
        );
        self.put_question(question, site, act, cx);
    }

    /// The user pressed the card's Choose File… for the question `id`: the system's file dialog
    /// opens over `window`, and the agent waits for what came of it, HANDOFF_WAIT at most.
    pub fn choose_file(&mut self, id: u64, window: u64, cx: &mut Context<Self>) {
        let Some(at) = (self.asking.iter())
            .position(|asking| asking.question.id == id && asking.act.file.is_some())
        else {
            return;
        };
        let Asking { site, act, .. } = self.asking.remove(at);
        self.asked(cx);
        let Some(field) = act.file.as_ref().map(|file| file.id) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: agent {} answered {id} Choose", act.agent);
        if !self.browser.file_field_choose(field, window) {
            let refused = Refused::new("The file dialog couldn't open.");
            return self.refuse(act, &site, refused, cx);
        }
        let timer = cx.spawn(async move |agents, cx| {
            cx.background_executor()
                .timer(Waits::launch().handoff)
                .await;
            _ = agents.update(cx, |agents, cx| {
                if let Some(Choosing { site, act, .. }) = agents.choosing.remove(&field) {
                    let why = "The user didn't choose a file in 15 minutes.";
                    agents.refuse(act, &site, Refused::new(why), cx);
                }
            });
        });
        let choosing = Choosing {
            site,
            act,
            _timer: timer,
        };
        self.choosing.insert(field, choosing);
    }

    /// What came of the user's Choose File… for the page's file `field` (file_field_done): the
    /// agent hears how many files went in, never which.
    pub fn file_field_done(&mut self, field: u64, end: FileFieldEnd, cx: &mut Context<Self>) {
        let (site, act) = match self.choosing.remove(&field) {
            Some(Choosing { site, act, .. }) => (site, act),
            None => {
                // The core ends a tab's earlier field as a new one shows there, its card still up.
                let at = (self.asking.iter()).position(|asking| {
                    asking
                        .act
                        .file
                        .as_ref()
                        .is_some_and(|file| file.id == field)
                });
                let Some(at) = at else {
                    return;
                };
                let Asking { site, act, .. } = self.asking.remove(at);
                self.asked(cx);
                (site, act)
            }
        };
        let (said, count) = match end {
            FileFieldEnd::Put(count) => ("put", count),
            FileFieldEnd::NoFile => ("none", 0),
            FileFieldEnd::Left => ("left", 0),
            FileFieldEnd::Closed => ("closed", 0),
        };
        // check.sh reads this line, which names no file.
        log!(
            "browser: agent {} file field {field} {said} {count}",
            act.agent
        );
        let label = act.file.as_ref().map_or("", |file| file.label.as_str());
        let why = match end {
            FileFieldEnd::Put(count) => {
                let files = match count {
                    1 => "a file".to_string(),
                    _ => format!("{count} files"),
                };
                let text = match label.is_empty() {
                    true => format!("The user put {files} in the field."),
                    false => format!("The user put {files} in “{label}”."),
                };
                self.browser.file_field_drop(field);
                self.note(&act.agent, act.connection, "upload", &site, "");
                cx.notify();
                _ = act.reply.send(ToolResult::text(text));
                return;
            }
            FileFieldEnd::NoFile => "The user chose no file.",
            FileFieldEnd::Left => LEFT_PAGE,
            FileFieldEnd::Closed => "The file dialog closed before the user chose a file.",
        };
        self.refuse(act, &site, Refused::new(why), cx);
    }

    /// The question `id` waited ASK_WAIT with no answer: the action is refused this once, as the
    /// × refuses it, so the agent isn't left waiting on a user who went away.
    fn unanswered(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(at) = (self.asking.iter()).position(|asking| asking.question.id == id) else {
            return;
        };
        let Asking { site, act, .. } = self.asking.remove(at);
        log!("browser: agent {} asked {id} with no answer", act.agent);
        self.refuse(act, &site, Refused::new(NO_ANSWER), cx);
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
        // A file is chosen with the card's Choose File… (choose_file), so only a no answers one.
        if self.asking[at].act.file.is_some() && matches!(answer, Answer::Once | Answer::Always) {
            return;
        }
        let Asking {
            site,
            act,
            question,
            ..
        } = self.asking.remove(at);
        // check.sh reads this line.
        log!("browser: agent {} answered {id} {answer:?}", act.agent);
        // A fenced site's card has no Always: the fence would pass none (#3724).
        let answer = match answer {
            Answer::Always if question.fenced => Answer::Once,
            answer => answer,
        };
        // A request's changes and a payment ask every time, whatever answered.
        if act.action != "request"
            && !question.each_time
            && let Answer::Always | Answer::Never = answer
        {
            let sites = self.grants.entry(self.answers_of(&act.agent)).or_default();
            sites.insert(site.clone(), answer == Answer::Always);
            self.save();
        }
        match answer {
            Answer::Once | Answer::Always if act.action == "request" => self.send(act, true, cx),
            Answer::Once | Answer::Always => self.run(act, cx),
            Answer::Never | Answer::Dismiss => {
                self.refuse(act, &site, Refused::of(tools::Kind::SaidNo, SAID_NO), cx)
            }
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

    /// The hosts the user fenced, as they added them. Only Settings reads them: no agent tool
    /// lists, reads or changes them.
    pub fn fenced(&self) -> &[String] {
        &self.fenced
    }

    /// Adds the host `typed` names to the fenced sites, once, and gives it, or None if it names no
    /// web host.
    pub fn fence(&mut self, typed: &str, cx: &mut Context<Self>) -> Option<String> {
        let host = fence::host_of(typed)?;
        if !self.fenced.contains(&host) {
            self.fenced.push(host.clone());
            self.save();
            // check.sh waits for this line.
            log!("browser: agents fenced {host}");
            cx.notify();
        }
        Some(host)
    }

    /// Takes `host` off the fenced sites: the answers the user gave there apply again at once.
    pub fn unfence(&mut self, host: &str, cx: &mut Context<Self>) {
        let before = self.fenced.len();
        self.fenced.retain(|each| each != host);
        if self.fenced.len() != before {
            self.cleared.retain(|_, cleared| cleared.entry != host);
            self.save();
            // check.sh waits for this line.
            log!("browser: agents unfenced {host}");
            cx.notify();
        }
    }

    /// The fenced entry `site`, a host as `site` gives it, is under.
    fn fence_of(&self, site: &str) -> Option<String> {
        fence::entry_for(&self.fenced, site).map(str::to_string)
    }

    /// The site of `tab`'s page if it's under a fenced entry the user hasn't let the tab stay
    /// under: it came there without an open they allowed, so nothing of its page reaches the
    /// agent until they do. The model's address is the page an answer is about: the core reports a
    /// tab's state from the commit, a restore from the back-forward cache included, in a task on
    /// the UI thread posted before the page settles and the action's answer is posted, so the
    /// report reaches the app first (mcp.sh's forward step guards it).
    fn came_to(&self, tab: u64, cx: &App) -> Option<String> {
        let site = site(&self.browser.tab(tab, cx).url);
        let entry = self.fence_of(&site)?;
        (self.cleared.get(&tab).map(|cleared| &cleared.entry) != Some(&entry)).then_some(site)
    }

    /// The log's words for `why`, a refusal of a call on `tab`: the agent's own, but for a tab that
    /// came to a fenced site, whose refusal the user reads by the entry it matched.
    fn log_words(&self, tab: u64, why: &str, cx: &App) -> String {
        let site = site(&self.browser.tab(tab, cx).url);
        match self.fence_of(&site) {
            Some(entry) if why == fence::refused(tab, &site) => fence::logged(&entry),
            _ => why.into(),
        }
    }

    /// Lets `tab` stay under `entry`, as the user said, and gives what it had before.
    fn clear(&mut self, tab: u64, entry: String, cx: &App) -> Option<Cleared> {
        let from = self.browser.tab(tab, cx).url.to_string();
        let arrived = self.fence_of(&site(&from)).as_ref() == Some(&entry);
        let cleared = Cleared {
            entry,
            from,
            arrived,
        };
        self.cleared.insert(tab, cleared)
    }

    /// `tab` is at `url`, from each address the core reports for it, back and forward through
    /// history included: once it's off the entry the user let it stay under, coming back asks
    /// again. Before it gets there, only the page it was on keeps the yes.
    pub fn tab_at(&mut self, tab: u64, url: &str) {
        // Every tab's every report comes here, and almost none has a yes to follow.
        let Some(cleared) = self.cleared.get_mut(&tab) else {
            return;
        };
        if fence::entry_for(&self.fenced, &site(url)) == Some(cleared.entry.as_str()) {
            cleared.arrived = true;
        } else if cleared.arrived || (url != cleared.from && is_web(url)) {
            self.cleared.remove(&tab);
        }
    }

    /// The open the user allowed on `tab` has answered: if it didn't take the tab under its entry,
    /// their yes ends, so a later way there asks.
    fn settle(&mut self, tab: u64, cx: &App) {
        let Some(cleared) = self.cleared.get(&tab).filter(|cleared| !cleared.arrived) else {
            return;
        };
        let url = self.browser.tab(tab, cx).url;
        match self.fence_of(&site(&url)).as_ref() == Some(&cleared.entry) {
            true => self.cleared.get_mut(&tab).unwrap().arrived = true,
            false => _ = self.cleared.remove(&tab),
        }
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
            Some(site) => self.note(agent, 0, "revoked", site, ""),
            None => self.note(agent, 0, "revoked all", "", ""),
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

    /// The agent went, or with `connection`, that connection of its: what waits for it goes
    /// unanswered, its questions with it. Its other connections' wait on.
    fn forget_waiting(&mut self, agent: &str, connection: Option<u64>, cx: &mut Context<Self>) {
        let asked = self.question().map(|question| question.id);
        let theirs =
            |whose: &str, on: u64| whose == agent && connection.is_none_or(|gone| gone == on);
        // A download nobody's left to answer for leaves no file.
        for asking in
            (self.asking.iter()).filter(|asking| theirs(&asking.act.agent, asking.act.connection))
        {
            if asking.act.download != 0 {
                self.browser.answer_download(asking.act.download, false);
                self.ended.remove(&asking.act.download);
            }
            // Nor does a file field: the page gets nothing.
            if let Some(file) = &asking.act.file {
                self.browser.file_field_drop(file.id);
            }
        }
        self.asking
            .retain(|asking| !theirs(&asking.act.agent, asking.act.connection));
        let choosing = (self.choosing)
            .extract_if(|_, choosing| theirs(&choosing.act.agent, choosing.act.connection));
        for (field, _) in choosing {
            self.browser.file_field_drop(field);
        }
        self.describing
            .retain(|_, act| !theirs(&act.agent, act.connection));
        self.downloading
            .retain(|_, downloading| !theirs(&downloading.agent, downloading.connection));
        self.acting
            .retain(|_, acting| !theirs(&acting.agent, acting.connection));
        self.sending
            .retain(|_, sending| !theirs(&sending.agent, sending.connection));
        self.runs.retain(|run| !theirs(&run.agent, run.connection));
        let held = (self.held.iter())
            .filter(|(_, held)| theirs(&held.agent, held.connection))
            .map(|(&tab, _)| tab)
            .collect::<Vec<_>>();
        for tab in held {
            self.let_go(tab, held::Event::Gone { holder: true }, cx);
        }
        if self.question().map(|question| question.id) != asked {
            self.asked(cx);
        }
    }

    /// Answers the action with why it's refused, which the log has.
    fn refuse(&mut self, act: Act, site: &str, refused: Refused, cx: &mut Context<Self>) {
        let Refused { why, kind, told } = refused;
        let why = why.as_str();
        if act.download != 0 {
            self.browser.answer_download(act.download, false);
            self.ended.remove(&act.download);
        }
        // Dropping a field the core ended already is harmless, so every end drops it.
        if let Some(file) = &act.file {
            self.browser.file_field_drop(file.id);
        }
        // A call's requests the user said no to are each listed, as each sent one is.
        match act.requests.as_slice() {
            [] => self.note(&act.agent, act.connection, act.action, site, why),
            requests => {
                for each in requests {
                    self.note_request(&act.agent, act.connection, site, each, why);
                }
            }
        }
        cx.notify();
        let answer = match why {
            _ if told.is_some() => told.unwrap(),
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
        _ = act.reply.send(ToolResult {
            kind,
            ..ToolResult::error(answer)
        });
    }

    /// Does the action: the core checks it and logs it, and answers with the page.
    fn run(&mut self, mut act: Act, cx: &mut Context<Self>) {
        // The user let the tab stay on the fenced site it came to: its call goes as it came.
        let open = match std::mem::replace(&mut act.fenced, Fenced::No) {
            Fenced::Came(entry, call) => {
                let site = site(&self.browser.tab(act.tab, cx).url);
                self.clear(act.tab, entry, cx);
                self.note(&act.agent, act.connection, fence::CAME_ACTION, &site, "");
                return self.dispatch(*call, cx);
            }
            Fenced::Open(entry) => Some(entry),
            Fenced::No => None,
        };
        if act.download != 0 {
            return self.download(act, cx);
        }
        if act.action == "request" {
            return self.send(act, false, cx);
        }
        let Act {
            agent,
            connection,
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
            did,
            reply,
            wait,
            ..
        } = act;
        // The user said yes to closing a page the agent didn't open without saving.
        if action == "discard_tab" {
            let site = site(&self.browser.tab(tab, cx).url);
            let closed = match self.may_act(&agent, tab, "close_tab", cx) {
                Ok(()) => self.browser.close_tab(&agent, tab, true, cx),
                Err(refused) => {
                    let act = act_of(&agent, connection, call, tab, reply);
                    return self.refuse(act, &site, refused, cx);
                }
            };
            return self.closed(
                &agent, connection, call, tab, true, closed, &site, reply, cx,
            );
        }
        // The entry this call logs below, which its answer marks if it carries a site's notes.
        let logged = next_entry_id();
        if action == "open_tab" {
            let site = site(&url);
            // An agent's first call may be this one, and the core opens a tab only in its Space.
            self.space(&agent, cx);
            tab = self.browser.open_tab(&agent, &url, cx);
            if tab == 0 {
                self.note(&agent, connection, action, &site, "The tab couldn't open.");
                cx.notify();
                _ = reply.send(ToolResult::error("The tab couldn't open."));
                return;
            }
            self.note(&agent, connection, action, &site, "");
            cx.notify();
            self.current.insert((agent.clone(), connection), tab);
            self.opened(&agent, call, tab);
            self.opened.insert(tab);
            if let Some(entry) = open {
                self.clear(tab, entry, cx);
            }
            // An agent's tab is quiet until the user wants to hear it (#1080).
            self.browser.set_muted(tab, true);
            note = format!("Opened tab {tab}.");
        } else {
            if !self.act(&agent, connection, tab, action, &reply, cx) {
                return;
            }
            let before =
                (open.filter(|_| action == "navigate")).map(|entry| self.clear(tab, entry, cx));
            if action == "navigate" && !self.browser.navigate(&agent, tab, &url, cx) {
                // It didn't go, so the user's yes for where it was to go goes with it.
                match before {
                    Some(Some(cleared)) => _ = self.cleared.insert(tab, cleared),
                    Some(None) => _ = self.cleared.remove(&tab),
                    None => {}
                }
                _ = reply.send(ToolResult::error("The tab couldn't go there."));
                return;
            }
        }
        let request = self.next_request;
        self.next_request += 1;
        let opened = action == "open_tab";
        let entry = Some(self.logged)
            .filter(|entry| *entry >= logged)
            .unwrap_or(0);
        let acting = Acting {
            agent: agent.clone(),
            connection,
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
            wait,
            entry,
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
        let (connection, mut reply) = match self.acting.remove(&request) {
            Some(acting) => (acting.connection, acting.reply),
            // A page's own download, with no action of the agent's to answer, still asks.
            None => (self.holder(&agent, tab), mpsc::channel().0),
        };
        // From a fenced site the tab wasn't let stay on, the file is the user's to answer, and its
        // name, which the site gave, never reaches the agent, nor how it ends (#3724). An open of
        // it the user allowed is their yes.
        let host = site(&url);
        let fenced = self.came_to(tab, cx).or_else(|| {
            let entry = self.fence_of(&host)?;
            let cleared = self.cleared.get(&tab).map(|cleared| &cleared.entry);
            (cleared != Some(&entry)).then(|| host.clone())
        });
        let why = match fenced {
            Some(site) => {
                _ = reply.send(ToolResult::text(fence::download(&site)));
                reply = mpsc::channel().0;
                FENCED
            }
            None => DOWNLOADS,
        };
        // A download is the open's whole answer, so its yes ends here unless the tab got there.
        self.settle(tab, cx);
        if tab_empty {
            self.left(tab, Gone::Download);
            self.browser.close_user_tab(tab, cx);
        }
        let act = Act {
            agent,
            connection,
            // A page's download comes from no call of the agent's.
            call: 0,
            action: "download",
            // The question is about the file, wherever its tab is.
            tab: 0,
            what: format!("download {name} from {host}"),
            asks: String::new(),
            url,
            page: PageAction::Snapshot,
            element: 0,
            text: name,
            note: String::new(),
            show: Show::Nothing,
            field: String::new(),
            download: id,
            file: None,
            requests: Vec::new(),
            wait: None,
            did: None,
            fenced: Fenced::No,
            reply,
        };
        self.check(act, host, Some(why), cx);
    }

    /// Lets the download the user said yes to go on. Its end answers, or a while if it's slow.
    fn download(&mut self, act: Act, cx: &mut Context<Self>) {
        let Act {
            agent,
            connection,
            url,
            text: name,
            download,
            reply,
            ..
        } = act;
        self.note(&agent, connection, "download", &site(&url), "");
        cx.notify();
        self.browser.answer_download(download, true);
        if let Some(end) = self.ended.remove(&download) {
            _ = reply.send(download_answer(&name, end));
            return self.advance(cx);
        }
        let downloading = Downloading {
            agent,
            connection,
            name,
            reply,
        };
        self.downloading.insert(download, downloading);
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
    /// Sends a call's requests one by one, `answered` if the user allowed its changes.
    fn send(&mut self, act: Act, answered: bool, cx: &mut Context<Self>) {
        let page = self.browser.tab(act.tab, cx).url;
        let signs_in =
            (self.outlines.get(&act.tab)).is_some_and(|outline| outline.signs_in_at(&page));
        let sending = Sending {
            agent: act.agent,
            connection: act.connection,
            tab: act.tab,
            answered,
            sent: None,
            left: act.requests.into(),
            answers: Vec::new(),
            shown: request::ALL_SHOWN,
            hidden: 0,
            signs_in,
            reply: act.reply,
        };
        self.send_next(sending, cx)
    }

    fn send_next(&mut self, mut sending: Sending, cx: &mut Context<Self>) {
        let Some(next) = sending.left.pop_front() else {
            return finish(sending, false, None);
        };
        let (agent, connection, tab) = (sending.agent.clone(), sending.connection, sending.tab);
        // A change goes only with the user's answer. One that comes here without a question, as
        // for a tab the UI doesn't list in the agent's Space (`ask_first`) that the core would
        // still allow, is refused in the core's words for a tab not the agent's, so the agent
        // learns nothing it wouldn't from the core.
        let checked = match next.changes() && !sending.answered {
            true => Err(Refused::new(NOT_ITS_SPACE)),
            false => self.may_act(&agent, tab, "request", cx),
        };
        let site = site(&self.browser.tab(tab, cx).url);
        let refused = checked.as_ref().err().map_or(String::new(), |refused| {
            self.log_words(tab, &refused.why, cx)
        });
        self.note_request(&agent, connection, &site, &next, &refused);
        cx.notify();
        if let Err(refused) = checked {
            sending
                .answers
                .push(format!("{}: {}", next.target(), refused.why));
            return finish(sending, true, refused.kind);
        }
        self.current.insert((agent.clone(), connection), tab);
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

    /// Reads `url` on `profile`'s own session (Browser::read), within the core's caps, then runs
    /// `done` once with what it answered. Gives the read's id.
    pub fn read(
        &mut self,
        profile: u64,
        url: &str,
        max_bytes: usize,
        timeout_ms: i64,
        done: impl FnOnce(saved_read::Answer, &mut Context<Self>) + 'static,
    ) -> u64 {
        let read = self.next_request;
        self.next_request += 1;
        self.saved_reading.insert(read, Box::new(done));
        let (max_bytes, timeout_ms) = (
            max_bytes.min(saved_read::MAX_BYTES),
            timeout_ms.min(saved_read::MAX_MILLISECONDS),
        );
        self.browser.read(profile, read, url, max_bytes, timeout_ms);
        read
    }

    /// The core's answer to the saved read `read`, for what waits on it.
    pub fn read_done(&mut self, read: u64, answer: saved_read::Answer, cx: &mut Context<Self>) {
        if let Some(done) = self.saved_reading.remove(&read) {
            done(answer, cx);
        }
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
            0 => finish(sending, true, None),
            _ => self.send_next(sending, cx),
        }
    }

    /// The ref of the element the action `request` acts on, such as "e12", as the agent named it.
    pub fn ref_of(&self, request: u64) -> Option<String> {
        let acting = self.acting.get(&request)?;
        let number = self.refs.get(&acting.tab)?.number(acting.element)?;
        Some(format!("e{number}"))
    }

    /// The core's answer to the action `request`: with `error`, its code and why it couldn't; else
    /// a screenshot's JPEG in base64, or the page's tree, whose nodes the answer outlines, and
    /// whether the page was still busy when the core stopped waiting for it.
    pub fn page_done(
        &mut self,
        request: u64,
        text: String,
        error: Option<Failure>,
        cx: &mut Context<Self>,
    ) {
        if let Some(act) = self.describing.remove(&request) {
            return self.file_field_described(act, text, error, cx);
        }
        let Some(Acting {
            agent,
            connection,
            call,
            did,
            tab,
            mut note,
            show,
            opened,
            field,
            reply,
            opened_tabs,
            wait,
            entry,
            ..
        }) = self.acting.remove(&request)
        else {
            return;
        };
        // What it did, as a look again for the text it waits for says it too.
        let did_note = note.clone();
        if !opened_tabs.is_empty() {
            let tabs = opened_tabs
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let s = if opened_tabs.len() > 1 { "s" } else { "" };
            note = format!("{note} It opened tab{s} {tabs} among your tabs.")
                .trim_start()
                .into();
        }
        self.settle(tab, cx);
        // What it did took the tab to a fenced site: nothing of that page reaches the agent until
        // the user lets it stay (#3724), the outline an action answers with included, nor whether
        // the page failed it. A sign-in there waits until they do.
        if let Some(site) = self.came_to(tab, cx) {
            if let Some(did) = did {
                self.counted(&agent, call, did);
            }
            let withheld = fence::withheld(tab, &site);
            let text = match note.is_empty() {
                true => withheld,
                false => format!("{note} {withheld}"),
            };
            _ = reply.send(ToolResult::text(text));
            return self.advance(cx);
        }
        if let Some(failure) = error {
            // The call ended as its page asked for a sign-in, which is the user's: it waits on
            // them as a handoff's call does.
            let reply = match self.sign_in_hand_off(&agent, connection, call, tab, reply, cx) {
                Ok(()) => return self.advance(cx),
                Err(reply) => reply,
            };
            // As a page still loading: the agent can look at the tab it opened later (#1461).
            let text = match opened {
                true => format!("{note} {text}"),
                false => text,
            };
            _ = reply.send(ToolResult {
                kind: failure.kind(),
                ..ToolResult::error(text)
            });
            return self.advance(cx);
        }
        if let Some(did) = did {
            self.counted(&agent, call, did);
        }
        if !text.starts_with('{') {
            _ = reply.send(ToolResult::image(text));
            // A run's last step, whose picture is its answer.
            return self.advance(cx);
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
                Some(option) => format!("The {field} now holds {}.", page::curly(option, 60)),
                None => format!("{note} {UNSHOWN}"),
            };
        }
        let info = self.browser.tab(tab, cx);
        let refs = self.refs.entry(tab).or_default();
        // The page's whole outline is stored even when the answer shows a part, since what has
        // the keyboard decides what a key press asks next (GHSA-hqw3-jp9p-3rhj).
        let whole = page::outline(&info, &nodes, refs, None);
        let answer = match show {
            Show::Element(node) => {
                let part = page::outline(&info, &nodes, refs, Some(node)).text();
                self.outlines.insert(tab, whole);
                ToolResult::text(match note.is_empty() {
                    true => part,
                    false => format!("{note}\n\n{part}"),
                })
            }
            show => {
                if let Some(mut wait) = wait {
                    let now = cx.background_executor().now();
                    let until =
                        *(wait.until).get_or_insert(now + Duration::from_secs_f64(wait.seconds));
                    let shows = (whole.text().to_lowercase()).contains(&wait.text.to_lowercase());
                    if !shows && now < until {
                        let acting = Acting {
                            agent,
                            connection,
                            call,
                            did: None,
                            tab,
                            element: 0,
                            note: did_note,
                            show,
                            opened,
                            field,
                            reply,
                            opened_tabs,
                            wait: Some(wait),
                            entry,
                        };
                        let after = LOOK_EVERY.min(until.saturating_duration_since(now));
                        return self.look_again(acting, after, cx);
                    }
                    let (text, seconds) = (page::clip(&wait.text, 60), wait.seconds);
                    let said = match shows {
                        true => format!("“{text}” showed."),
                        false => format!("“{text}” didn't show in {seconds} s."),
                    };
                    note = match note.is_empty() {
                        true => said,
                        false => format!("{note} {said}"),
                    };
                }
                // The refs stay the page's, for an action the agent takes after reading it.
                self.outlines.insert(tab, whole);
                if show == Show::Text {
                    return self.read_page(Reading {
                        agent,
                        connection,
                        entry,
                        tab,
                        reply,
                        note,
                        from: 0,
                    });
                }
                self.page_answer(&agent, connection, entry, tab, note, show, cx)
            }
        };
        _ = reply.send(answer);
        self.advance(cx);
    }

    /// Reads the page of `acting`, whose text hasn't shown yet, again after `after`, unless its
    /// call has gone meanwhile.
    fn look_again(&mut self, acting: Acting, after: Duration, cx: &mut Context<Self>) {
        let request = self.next_request;
        self.next_request += 1;
        self.acting.insert(request, acting);
        cx.spawn(async move |agents, cx| {
            cx.background_executor().timer(after).await;
            _ = agents.update(cx, |agents, cx| {
                let Some(acting) = agents.acting.get(&request) else {
                    return;
                };
                let (agent, connection, tab) =
                    (acting.agent.clone(), acting.connection, acting.tab);
                let action = if acting.opened {
                    "open_tab"
                } else {
                    "navigate"
                };
                let reply = acting.reply.clone();
                if !agents.may_look_again(&agent, connection, tab, action, &reply, cx) {
                    agents.acting.remove(&request);
                    return;
                }
                agents.act_in_page(&agent, tab, request, PageAction::Snapshot, 0, "");
            });
        })
        .detach();
    }

    /// Whether a call under way may look at `tab` again: the user may have taken it between two
    /// looks. If not, the call ends as one on that tab would, refused and logged as `action`. Each
    /// look it may take isn't logged, as the call was once.
    fn may_look_again(
        &mut self,
        agent: &str,
        connection: u64,
        tab: u64,
        action: &str,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Err(refused) = self.may_act(agent, tab, "snapshot", cx) else {
            return true;
        };
        let site = site(&self.browser.tab(tab, cx).url);
        self.note(agent, connection, action, &site, &refused.why);
        cx.notify();
        _ = reply.send(refused.into());
        false
    }

    /// Asks the core for the text of `reading`'s tab, which `text_read` answers with.
    fn read_page(&mut self, reading: Reading) {
        let request = self.next_request;
        self.next_request += 1;
        let tab = reading.tab;
        self.reading.insert(request, reading);
        self.browser.read_text(tab, request);
    }

    /// A call's answer as `answer_with` words it, with the site's notes if it shows the agent the
    /// page and its task hasn't had them (#4122). `entry` is the call's in the log, or 0.
    #[allow(clippy::too_many_arguments)]
    fn page_answer(
        &mut self,
        agent: &str,
        connection: u64,
        entry: u64,
        tab: u64,
        note: String,
        show: Show,
        cx: &App,
    ) -> ToolResult {
        // Before the outline is the agent's, so a tab that came to a fenced site says only that.
        let notes = match show {
            Show::Nothing | Show::Text => None,
            _ => self.notes_now(agent, connection, entry, tab, cx),
        };
        ToolResult::noted(self.answer_with(agent, tab, note, show, cx), notes)
    }

    /// The note for the site `agent`'s `tab` is on, the first time its task is shown a page there,
    /// and never after: only for a tab of its own Space that the user doesn't hold, on a page that
    /// isn't off limits or on a fenced site it came to, with Site notes on. The log's entry for the
    /// call, `entry`, says it carried them, and nothing else is kept.
    fn notes_now(
        &mut self,
        agent: &str,
        connection: u64,
        entry: u64,
        tab: u64,
        cx: &App,
    ) -> Option<&'static notes::Note> {
        if self.notes_off || self.held.contains_key(&tab) || !self.owns(agent, tab, cx) {
            return None;
        }
        if self.off_limits(agent, tab, cx).is_some() || self.came_to(tab, cx).is_some() {
            return None;
        }
        let note = notes::note_for(&self.browser.tab(tab, cx).url)?;
        // The task under way: the user's own by its agent, or the client's session on this
        // connection. A call of no task has no first time, so gets none.
        let task = (self.tasks.tasks.iter()).find(|task| match &task.session {
            None => task.agent == agent,
            Some(session) => {
                task.state == task::State::Working
                    && session.agent == agent
                    && session.connection == connection
            }
        })?;
        let task = task.agent.clone();
        let tasks = &self.tasks.tasks;
        (self.noted).keep(|key| tasks.iter().any(|task| task.agent == key));
        if !self.noted.first(&task, note) {
            return None;
        }
        self.log_notes(entry, note);
        Some(note)
    }

    /// Marks the log's entry `entry`, a call's or a run's step's, as the one whose answer carried
    /// `note`. A call that brought no entry of its own (0) marks none: the log's last entry may
    /// be another call's, as calls on different tabs are under way at once.
    fn log_notes(&mut self, entry: u64, note: &notes::Note) {
        let found = (self.log.iter_mut().rev())
            .flat_map(|each| match each.id == entry {
                true => vec![each],
                false => each.steps.iter_mut().collect(),
            })
            .find(|each| entry != 0 && each.id == entry);
        if let Some(found) = found {
            found.notes = note.name.into();
        }
    }

    /// `note`, what an action did, then the tab's latest outline as `show` says, which is the
    /// outline the agent has of the tab from then on, unless it gets none of it.
    fn answer_with(&mut self, agent: &str, tab: u64, note: String, show: Show, cx: &App) -> String {
        // A wait that saw the tab come to a fenced site says only that (#3724).
        if let Some(site) = self.came_to(tab, cx) {
            let withheld = fence::withheld(tab, &site);
            return match note.is_empty() {
                true => withheld,
                false => format!("{note} {withheld}"),
            };
        }
        let Some(outline) = self.outlines.get(&tab) else {
            return note;
        };
        let had = (self.had.get(&tab))
            .filter(|(whose, _)| whose == agent)
            .map(|(_, had)| had);
        let (page, has) = match (show, had) {
            (Show::Nothing | Show::Text, _) => (outline.head().to_string(), false),
            (Show::Changes, Some(had)) => outline.since(had),
            _ => (outline.text(), true),
        };
        // A line saying most of it changed leaves the agent with the outline it had.
        if has {
            self.had.insert(tab, (agent.to_string(), outline.clone()));
        }
        match note.is_empty() {
            true => page,
            false => format!("{note}\n\n{page}"),
        }
    }

    /// Sort Today Tabs by Site in the agent's own Space, by the user's command's rule
    /// (`crate::sort_by_site`): no tab outside it is ever listed, so none moves. One entry in the
    /// activity log.
    fn sort_by_site(
        &mut self,
        agent: &str,
        connection: u64,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        // A tab the user holds stays where it is, as its place among the sites would tell its site.
        let today = (self.browser.agent_tabs(agent, cx).into_iter()).filter(|tab| {
            !tab.pinned
                && self.browser.off_limits(tab.id, cx).is_none()
                && !self.held.contains_key(&tab.id)
        });
        let mut rows: Vec<(u64, usize, Option<String>)> = Vec::new();
        for tab in today {
            let split = self.browser.split(tab.id, cx);
            // A split's first tab stands for its row; a split joins no site.
            if split.first().is_some_and(|&first| first != tab.id) {
                continue;
            }
            // One on a fenced site it came to goes as a tab of no site, as its place would tell
            // its site (#3724).
            let alone = split.len() <= 1 && self.came_to(tab.id, cx).is_none();
            let site = alone.then(|| self.browser.url_site(&tab.url)).flatten();
            rows.push((tab.id, split.len().max(1), site));
        }
        let sites: Vec<_> = (rows.iter())
            .map(|(row, _, site)| (*row, site.clone()))
            .collect();
        let order = sort_by_site::sorted(&sites);
        let counts: Vec<_> = (rows.iter()).map(|&(row, tabs, _)| (row, tabs)).collect();
        let moves = sort_by_site::moves(&counts, &order);
        self.note(agent, connection, "sort_tabs_by_site", "", "");
        cx.notify();
        for &(row, index) in &moves {
            self.browser.move_today(row, index, cx);
        }
        let answer = match moves.is_empty() {
            true => "Your Today tabs are together by site already.".to_string(),
            false => format!(
                "Sorted your Today tabs by site, each site's tabs together at its first tab's \
                 place: {}.",
                (order.iter().map(u64::to_string))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        _ = reply.send(ToolResult::text(answer));
    }

    /// Pins the agent's tab, or unpins or removes one it pinned (#1360). The core keeps every one
    /// to the agent's own Space; a pinned tab the agent didn't pin is the user's, and stays.
    fn pin(
        &mut self,
        agent: &str,
        connection: u64,
        action: &str,
        tab: u64,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let pinned = self.browser.tab(tab, cx).pinned;
        let theirs = pinned && self.pins.get(&tab).is_none_or(|by| by != agent);
        let checked =
            self.may_act(agent, tab, action, cx)
                .and_then(|()| match (action, pinned, theirs) {
                    ("pin_tab", _, _) => Ok(()),
                    (_, false, _) => Err(Refused::new(format!("Tab {tab} isn't pinned."))),
                    (_, true, true) => Err(Refused::new(format!(
                        "The user pinned tab {tab}, so only they {} it.",
                        action.trim_end_matches("_tab")
                    ))),
                    (_, true, false) => Ok(()),
                });
        let site = site(&self.browser.tab(tab, cx).url);
        let refused = checked.as_ref().err().map_or(String::new(), |refused| {
            self.log_words(tab, &refused.why, cx)
        });
        self.note(agent, connection, action, &site, &refused);
        cx.notify();
        if let Err(refused) = checked {
            _ = reply.send(refused.into());
            return;
        }
        self.current.insert((agent.into(), connection), tab);
        let answer = match (action, pinned) {
            ("pin_tab", true) => format!("Tab {tab} is pinned already."),
            ("pin_tab", false) => {
                self.browser.set_pinned(tab, true, cx);
                self.pins.insert(tab, agent.into());
                format!(
                    "Pinned tab {tab} at the page it's at. It stays in your pinned tabs, even \
                     closed, until you unpin or remove it."
                )
            }
            ("unpin_tab", _) => {
                self.browser.set_pinned(tab, false, cx);
                self.pins.remove(&tab);
                format!("Unpinned tab {tab}. It's at the top of your Today tabs.")
            }
            _ => {
                // Removing closes the tab with its split, so each says why before the core's
                // close would read as the user's, and one whose page kept it open says nothing.
                let split = self.browser.split(tab, cx);
                for &each in &split {
                    self.went(each, agent, Gone::Removed(tab));
                }
                self.browser.remove_pinned(tab, cx);
                self.pins.remove(&tab);
                let space = self.browser.agent_space(agent);
                let still = self.browser.space_tab_ids(space, cx);
                for each in split.iter().filter(|each| still.contains(each)) {
                    self.gone.remove(each);
                }
                let open = still.contains(&tab);
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
        connection: u64,
        tab: u64,
        action: &str,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.act_as(agent, connection, tab, action, action, reply, cx)
    }

    /// The core's answer to whether `agent` may do `action` to `tab` (Browser::may_act), but never
    /// for a tab that's off limits, whatever its Space (`off_limits`).
    /// Closing one of its own off the web reads nothing of it, and is the agent's way out of a tab
    /// that went there.
    fn may_act(&self, agent: &str, tab: u64, action: &str, cx: &mut App) -> Result<(), Refused> {
        match self.off_limits(agent, tab, cx) {
            Some(NOT_WEB_PAGE) if action == "close_tab" => {}
            Some(why) => return Err(off_limits_refused(why)),
            None => {}
        }
        self.browser
            .may_act(agent, tab, action, cx)
            .map_err(|(failure, why)| match (failure, self.gone.get(&tab)) {
                (Failure::NotItsSpace, Some((whose, gone))) if whose == agent => {
                    Refused::new(gone.why(tab))
                }
                _ => Refused {
                    kind: failure.kind(),
                    ..Refused::new(why)
                },
            })?;
        // Only once the tab is known to be the agent's, so another's page says nothing.
        match self.dialogs.get(&tab) {
            // Not even which dialog, on a fenced site the tab came to (#3724).
            Some(&(kind, _)) if action != "close_tab" => match self.came_to(tab, cx) {
                Some(site) => Err(Refused::of(
                    tools::Kind::Outside,
                    fence::refused(tab, &site),
                )),
                None => Err(Refused {
                    kind: (kind == DialogKind::SignIn).then_some(tools::Kind::Password),
                    ..Refused::new(showing_dialog(tab, kind, self.opened.contains(&tab)))
                }),
            },
            _ => Ok(()),
        }
    }

    /// Why `agent` may not see or act on `tab` for its page (`off_limits`), or None. Another
    /// Space's tab off the web is the core's to answer, and log, as any tab not the agent's, so the
    /// answer says nothing of the user's page. A Board's answer is that already.
    fn off_limits(&self, agent: &str, tab: u64, cx: &App) -> Option<&'static str> {
        let why = self.browser.off_limits(tab, cx)?;
        (why == NOT_ITS_SPACE || self.owns(agent, tab, cx)).then_some(why)
    }

    /// Whether `tab` is in `agent`'s own Space.
    fn owns(&self, agent: &str, tab: u64, cx: &App) -> bool {
        let space = self.browser.space_of(tab, cx);
        space != 0 && space == self.browser.agent_space(agent)
    }

    /// The tabs of `space` an agent sees: none off limits, as `may_act` refuses those.
    fn tabs_seen(&self, space: u64, cx: &App) -> Vec<TabInfo> {
        (self.browser.space_tabs(space, cx).into_iter())
            .filter(|tab| self.browser.off_limits(tab.id, cx).is_none())
            .collect()
    }

    /// As `act`, for what the core checks as `action` and the log names `logged`.
    #[allow(clippy::too_many_arguments)]
    fn act_as(
        &mut self,
        agent: &str,
        connection: u64,
        tab: u64,
        action: &str,
        logged: &str,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> bool {
        let checked = self.may_act(agent, tab, action, cx);
        let site = site(&self.browser.tab(tab, cx).url);
        let refused = checked.as_ref().err().map_or(String::new(), |refused| {
            self.log_words(tab, &refused.why, cx)
        });
        self.note(agent, connection, logged, &site, &refused);
        cx.notify();
        match checked {
            Ok(()) => {
                self.current.insert((agent.into(), connection), tab);
                true
            }
            Err(refused) => {
                _ = reply.send(refused.into());
                false
            }
        }
    }

    /// Closes the agent's `tab`, asking its page first as the user's close does, discard or not.
    /// A page that asks to stay stays open, unless `discard`: then one the agent opened closes,
    /// and any other asks the user first, as what it holds unsaved may be theirs.
    #[allow(clippy::too_many_arguments)]
    fn close_tab(
        &mut self,
        agent: &str,
        connection: u64,
        call: u64,
        tab: u64,
        discard: bool,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let site = site(&self.browser.tab(tab, cx).url);
        if let Err(refused) = self.may_act(agent, tab, "close_tab", cx) {
            let act = act_of(agent, connection, call, tab, reply);
            return self.refuse(act, &site, refused, cx);
        }
        // The agent's own tab, discarded while its page is asked, doesn't wait for the page. Nor
        // does one whose page shows its dialog, which can't answer before it's gone (#3536).
        let asked = discard && self.closing.contains_key(&tab);
        let stuck = self.dialogs.contains_key(&tab);
        if self.opened.contains(&tab) && (asked || stuck) {
            let closed = self.browser.close_tab(agent, tab, true, cx);
            return self.closed(
                agent, connection, call, tab, discard, closed, &site, reply, cx,
            );
        }
        let closing = Closing {
            agent: agent.into(),
            connection,
            call,
            discard,
            site,
            reply,
        };
        // In a tab it didn't open, what's unsaved may be the user's, and the page can't say: it
        // goes as one that asks before it's left, so a discard asks the user.
        if stuck {
            self.close_came_to(tab, closing, Closed::Asks, cx);
            return;
        }
        match self.browser.close_tab(agent, tab, false, cx) {
            // A second close while its page is asked waits for the same answer.
            Closed::Pending => self.closing.entry(tab).or_default().push(closing),
            closed => _ = self.close_came_to(tab, closing, closed, cx),
        }
    }

    /// What the core's asking of `tab`'s page came to (agent_close_answered), for each close
    /// waiting on it.
    pub fn close_answered(&mut self, tab: u64, closed: Closed, cx: &mut Context<Self>) {
        let mut closed = closed;
        for closing in self.closing.remove(&tab).unwrap_or_default() {
            // Once one discards it, the rest hear it closed.
            if self.close_came_to(tab, closing, closed, cx) == Closed::Closed {
                closed = Closed::Closed;
            }
        }
    }

    /// Answers `closing` of `tab`, which came to `closed`: a page that asked to stay is discarded
    /// if the agent said to, asking the user first in a tab the agent didn't open. Returns what
    /// it came to then, Pending while the user is asked.
    fn close_came_to(
        &mut self,
        tab: u64,
        closing: Closing,
        closed: Closed,
        cx: &mut Context<Self>,
    ) -> Closed {
        let Closing {
            agent,
            connection,
            call,
            discard,
            site,
            reply,
        } = closing;
        let discarded = closed == Closed::Asks && discard;
        if discarded && !self.opened.contains(&tab) {
            let what = format!("close the tab on {site} without saving");
            let act = Act {
                action: "discard_tab",
                what,
                ..act_of(&agent, connection, call, tab, reply)
            };
            self.check(act, site, Some(UNSAVED), cx);
            return Closed::Pending;
        }
        let closed = match discarded {
            true => self.browser.close_tab(&agent, tab, true, cx),
            false => closed,
        };
        self.closed(
            &agent, connection, call, tab, discarded, closed, &site, reply, cx,
        );
        closed
    }

    /// Answers the agent's close `call` of `tab` on `site` with what happened, and logs it.
    #[allow(clippy::too_many_arguments)]
    fn closed(
        &mut self,
        agent: &str,
        connection: u64,
        call: u64,
        tab: u64,
        discarded: bool,
        closed: Closed,
        site: &str,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        if closed == Closed::Closed {
            self.went(tab, agent, Gone::Itself);
        }
        let answer = match closed {
            Closed::Closed if discarded => Ok(format!("Closed tab {tab} without saving.")),
            Closed::Closed => Ok(format!("Closed tab {tab}.")),
            Closed::Pinned => Err(format!("Tab {tab} stays open: the user pinned it.")),
            Closed::Asks => Err(format!(
                "Tab {tab} stays open: {}. close_tab with discard: true closes it without \
                 saving{}.",
                // Staying open tells that much, and on a fenced site the tab came to no more:
                // not whether a dialog holds it (#3724).
                match (
                    self.dialogs.contains_key(&tab),
                    self.came_to(tab, cx).is_some()
                ) {
                    (_, true) => fence::STAYS_OPEN,
                    (true, false) =>
                        "its page shows its own dialog, so it can't be asked whether it has \
                             unsaved changes",
                    (false, false) =>
                        "its page asks before it's left, as it may have unsaved changes",
                },
                if self.opened.contains(&tab) {
                    ""
                } else {
                    ", if this task's access allows that for a tab you didn't open"
                }
            )),
            Closed::Refused => Err("The tab isn't in the agent's own Space.".into()),
            Closed::Stays | Closed::Pending => Err(format!(
                "Tab {tab} stays open: before its page answered, it went to another page, was \
                 unloaded, or the user took it over or pinned it."
            )),
        };
        let logged = if discarded {
            "discard_tab"
        } else {
            "close_tab"
        };
        self.note(
            agent,
            connection,
            logged,
            site,
            answer.as_ref().err().map_or("", String::as_str),
        );
        if closed == Closed::Closed {
            self.opened.remove(&tab);
        }
        // Its card's line says the page had a say, not that the close failed.
        match closed {
            Closed::Asks => self.close_ended(agent, call, tools::Close::Kept),
            Closed::Closed if discarded => self.close_ended(agent, call, tools::Close::Discarded),
            _ => {}
        }
        cx.notify();
        // A tab that stayed open as the user had a say in it is theirs.
        let yours = matches!(closed, Closed::Stays | Closed::Pending);
        _ = reply.send(match answer {
            Ok(text) => ToolResult::text(text),
            Err(why) => ToolResult {
                kind: yours.then_some(tools::Kind::Yours),
                ..ToolResult::error(why)
            },
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

    /// The tab a call on `connection` without one acts in: the one that connection used last, if
    /// it's still in the agent's Space, else the Space's only tab. Any other may be the page of
    /// another session of the agent's, so the call names one, or with none, opens one. A caller of
    /// no connection, a ⌘T task or the `ilo` command, has its Space to itself and takes its first.
    fn default_tab(
        &mut self,
        agent: &str,
        connection: u64,
        cx: &mut Context<Self>,
    ) -> Result<u64, Refused> {
        let space = self.space(agent, cx);
        let tabs = self.tabs_seen(space, cx);
        let last = self.current.get(&(agent.into(), connection)).copied();
        match (
            last.filter(|last| tabs.iter().any(|tab| tab.id == *last)),
            &tabs[..],
        ) {
            (Some(last), _) => Ok(last),
            (None, [only]) => Ok(only.id),
            (None, []) => Err(Refused::of(tools::Kind::NotFound, NO_TAB)),
            (None, [first, ..]) if connection == 0 => Ok(first.id),
            (None, _) => Err(Refused::new(tools::NOT_A_TAB)),
        }
    }

    /// Logs what `agent` did on `connection` (Activity::connection), or 0 for none.
    fn note(&mut self, agent: &str, connection: u64, action: &str, site: &str, refused: &str) {
        self.note_on(agent, connection, action, site, "", refused)
    }

    /// As `note`, for a request: where it went and what it sent, or would have.
    fn note_request(
        &mut self,
        agent: &str,
        connection: u64,
        site: &str,
        sent: &request::Request,
        refused: &str,
    ) {
        // The agent's words are kept as its, as plain text, without what reads as a secret in
        // them (`hide_said`). They're the activity log's, in memory, not the past task's file.
        let said = request::hide_said(&sent.what).0;
        let entry = (sent.url.as_str(), sent.sent(), said.as_str());
        let target = sent.target();
        self.note_entry(agent, connection, "request", site, &target, refused, entry);
    }

    /// As `note`, for what on the page, `target`.
    fn note_on(
        &mut self,
        agent: &str,
        connection: u64,
        action: &str,
        site: &str,
        target: &str,
        refused: &str,
    ) {
        let entry = ("", Vec::new(), "");
        self.note_entry(agent, connection, action, site, target, refused, entry)
    }

    #[allow(clippy::too_many_arguments)]
    fn note_entry(
        &mut self,
        agent: &str,
        connection: u64,
        action: &str,
        site: &str,
        target: &str,
        refused: &str,
        (url, sent, said): (&str, Vec<request::Sent>, &str),
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
            connection,
            action: SharedString::from(action.to_string()),
            site: SharedString::from(site.to_string()),
            target: SharedString::from(target.to_string()),
            refused: SharedString::from(refused.to_string()),
            steps: Vec::new(),
            by: SharedString::default(),
            id: entry_id(),
            url: SharedString::from(url.to_string()),
            sent,
            said: SharedString::from(said.to_string()),
            tier: self.tier_of(agent, connection, 0).label().into(),
            notes: SharedString::default(),
        };
        self.logged = entry.id;
        if self.stepping == Some((agent.into(), connection)) {
            let run = (self.log.iter_mut().rev()).find(|each| {
                each.agent == agent && each.connection == connection && each.action == "run_steps"
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
        for tab in self.held.keys().copied().collect::<Vec<_>>() {
            self.let_go(tab, held::Event::Gone { holder: true }, cx);
        }
        // A handoff's card goes too, the user still holding its tab, and the page's file fields.
        let asked = self.asking.iter().map(|asking| &asking.act);
        let fields = asked.filter_map(|act| act.file.as_ref().map(|file| file.id));
        let fields = fields
            .chain(self.choosing.keys().copied())
            .collect::<Vec<_>>();
        for field in fields {
            self.browser.file_field_drop(field);
        }
        self.asking.clear();
        self.choosing.clear();
        self.describing.clear();
        self.browser.ask(None, cx);
        for agent in std::mem::take(&mut self.live) {
            self.browser.disconnected(&agent, cx);
            self.note(&agent, 0, "disconnected", "", "");
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
            "keep": !self.tasks.kept.off,
            "kept_told": self.tasks.kept.told,
        });
        let saved = json!({
            "on": self.on,
            "agents": agents,
            "grants": grants,
            "fenced": self.fenced,
            "tasks": tasks,
            "scripts": self.scripts,
            "notes": !self.notes_off,
            "sign_in": self.sign_in_on,
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

/// Why the core didn't do an agent's call, as its code says it (bc_agent_failure, #3702): the
/// app decides on this, never on the words that come with it, which may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Failed with nothing for the app to decide on.
    Other,
    Action,
    Restarted,
    NotItsSpace,
    Held,
    Pointing,
    SystemEngine,
    NotWebPage,
    Loading,
    PasswordField,
    NoOption,
    /// The core is signing the tab in for an agent.
    SigningIn,
    /// The step is the user's, such as a sign-in only they may finish.
    UserStep,
    /// The tab shows a page withheld from agents (Browser::withheld): only a navigation to
    /// another page, or closing it.
    WithheldPage,
}

impl Failure {
    /// The kind a task's card says it in, if any.
    pub fn kind(self) -> Option<tools::Kind> {
        match self {
            Failure::Held => Some(tools::Kind::Yours),
            Failure::NotWebPage => Some(tools::Kind::NotWebPage),
            Failure::Loading => Some(tools::Kind::StillLoading),
            Failure::PasswordField => Some(tools::Kind::Password),
            Failure::NoOption => Some(tools::Kind::NotFound),
            Failure::UserStep => Some(tools::Kind::Yours),
            _ => None,
        }
    }
}

/// An agent's call refused: why, for the agent and the user to read, and the kind of it a task's
/// card says, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Refused {
    pub why: String,
    pub kind: Option<tools::Kind>,
    /// What the agent hears, where it isn't `why`, which the log keeps for the user.
    pub told: Option<String>,
}

impl Refused {
    fn new(why: impl Into<String>) -> Self {
        Refused {
            why: why.into(),
            kind: None,
            told: None,
        }
    }

    fn of(kind: tools::Kind, why: impl Into<String>) -> Self {
        Refused {
            kind: Some(kind),
            ..Refused::new(why)
        }
    }
}

impl From<Refused> for ToolResult {
    fn from(refused: Refused) -> Self {
        ToolResult {
            kind: refused.kind,
            ..ToolResult::error(refused.why)
        }
    }
}

/// The refusal for a tab off limits (`off_limits`), whose page off the web a card says so of.
fn off_limits_refused(why: &'static str) -> Refused {
    // Compared with the app's own constant, not a copy of its words, so rewording it can't
    // change the kind.
    Refused {
        kind: (why == NOT_WEB_PAGE).then_some(tools::Kind::NotWebPage),
        ..Refused::new(why)
    }
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
        held::Event::Timer => ToolResult::failed(
            tools::Kind::Yours,
            format!(
                "The user hasn't handed tab {tab} back in 15 minutes. It stays theirs; ask \
                 them where you started the task before you go on there."
            ),
        ),
        held::Event::Closed => {
            ToolResult::error(format!("Tab {tab} closed while the user had it."))
        }
        held::Event::Moved => ToolResult::error(format!(
            "The user moved tab {tab} to a Space of their own. It's theirs now."
        )),
        _ => ToolResult::failed(tools::Kind::Stopped, STOPPED),
    }
}

/// `tab`'s page shows its own dialog of `kind`, with its `message`, or none any more
/// (pages::tab_dialog).
pub fn page_dialog(tab: u64, kind: Option<DialogKind>, message: &str, cx: &mut App) {
    if let Some(agents) = global(cx) {
        agents.update(cx, |agents, cx| agents.page_dialog(tab, kind, message, cx));
    }
}

/// The core closed `tab` (tabs::tab_closed).
pub fn tab_closed(tab: u64, cx: &mut App) {
    if let Some(agents) = global(cx) {
        agents.update(cx, |agents, cx| agents.tab_closed(tab, cx));
    }
}

/// The core says `tab` is at `url`: it loaded it (tabs::tab_loaded), or its address changed, as
/// going back or forward does, a page the back-forward cache restores included, which loads
/// nothing (tabs::tab_state_changed).
pub fn tab_at(tab: u64, url: &str, cx: &mut App) {
    if let Some(agents) = global(cx) {
        agents.update(cx, |agents, _| agents.tab_at(tab, url));
    }
}

/// `space`'s tabs changed (spaces::space_changed).
pub fn space_changed(space: u64, cx: &mut App) {
    if let Some(agents) = global(cx) {
        agents.update(cx, |agents, cx| agents.space_changed(space, cx));
    }
}

/// The answer to a call on `tab` while its page shows its own dialog of `kind`. No agent tool
/// answers a page's dialog (#3536), so it says what the agent can do: close the tab, which in one
/// it didn't `opened` takes the user's yes.
fn showing_dialog(tab: u64, kind: DialogKind, opened: bool) -> String {
    let what = match kind {
        DialogKind::Alert => "an alert",
        DialogKind::Confirm => "a confirm dialog",
        DialogKind::Prompt => "a prompt dialog",
        DialogKind::Leave => "a dialog asking whether to leave it",
        DialogKind::SignIn => "a dialog asking for a user name and password",
        DialogKind::OpenApp => "a dialog asking to open another app",
    };
    // Only the user may sign in (#3662).
    if kind == DialogKind::SignIn {
        let close = match opened {
            true => "close_tab to close the tab and its dialog",
            false => {
                "close_tab with discard: true to close the tab and its dialog. You did not open \
                 this tab, so the user may be asked first"
            }
        };
        return format!(
            "Tab {tab} shows a browser sign-in dialog, which only the user can answer. No other \
             calls work on the tab while it is open: use hand_off to let the user sign in, or \
             {close}."
        );
    }
    let close = match opened {
        true => "close_tab closes the tab, and the dialog with it",
        false => {
            "close_tab with discard: true closes the tab, and the dialog with it, if this task's \
             access allows that for a tab you didn't open"
        }
    };
    format!(
        "Tab {tab}'s page shows {what}. Nothing more can be done on the tab while it's up, and no \
         tool answers it. {close}."
    )
}

/// The text open_tab or navigate is to wait for, from `arguments`, or None having answered why it
/// can't (#3658).
fn wait_text(arguments: &Value, reply: &mpsc::Sender<ToolResult>) -> Option<Option<WaitText>> {
    let text = arguments["wait_for"].as_str().unwrap_or("").trim();
    if text.is_empty() {
        return Some(None);
    }
    let seconds = match arguments["seconds"].as_f64() {
        None => Some(tools::WAIT),
        seconds => seconds.filter(|seconds| *seconds > 0.0 && *seconds <= tools::MAX_WAIT),
    };
    let Some(seconds) = seconds else {
        _ = reply.send(ToolResult::error(format!(
            "seconds is how long wait_for waits, more than 0 and at most {}.",
            tools::MAX_WAIT
        )));
        return None;
    };
    let (text, until) = (text.to_string(), None);
    Some(Some(WaitText {
        text,
        seconds,
        until,
    }))
}

/// An action on `tab` for the call `call` with nothing more to it, as a close is.
fn act_of(
    agent: &str,
    connection: u64,
    call: u64,
    tab: u64,
    reply: mpsc::Sender<ToolResult>,
) -> Act {
    Act {
        agent: agent.into(),
        connection,
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
        asks: String::new(),
        field: String::new(),
        download: 0,
        file: None,
        requests: Vec::new(),
        wait: None,
        did: None,
        fenced: Fenced::No,
        reply,
    }
}

/// Why a request's changes ask: each time, as no name says what an API call does.
const CHANGES_ASK: &str = "Changes ask every time.";

/// Answers a call's requests with what each answered, numbered if there were more than one, and
/// what wasn't sent.
fn finish(sending: Sending, error: bool, kind: Option<tools::Kind>) {
    let Sending {
        tab,
        answers,
        left,
        hidden,
        signs_in,
        reply,
        ..
    } = sending;
    let many = answers.len() + left.len() > 1;
    let answers = answers.iter().enumerate().map(|(at, answer)| match many {
        true => format!("{}. {answer}", at + 1),
        false => answer.clone(),
    });
    let mut text = answers.collect::<Vec<_>>().join("\n\n");
    if signs_in {
        text = format!(
            "Tab {tab}'s page asks for a password, as a sign-in page does, so its site may have \
             answered as to someone signed out. Signing in is the user's to do: hand them the tab \
             with hand_off, and send the requests again once they hand it back.\n\n{text}"
        );
    }
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
    // A page that asks for a password is what its answer says first.
    let kind = if signs_in {
        Some(tools::Kind::Password)
    } else {
        kind
    };
    _ = reply.send(ToolResult {
        text,
        error,
        image: None,
        kind: kind.filter(|_| error),
        notes: None,
    });
}

/// What the log says of an action the user refused, now or for good on its site.
const SAID_NO: &str = "You said no.";
/// What the log says of an action that asked and had no answer in ASK_WAIT.
const NO_ANSWER: &str = "No answer in 5 minutes.";

/// Why an upload ends when its field went before the user's files could go in.
const LEFT_PAGE: &str = "The field left the page before the user chose a file.";
/// The spec's: how long an action asking first waits for the user's answer.
const ASK_WAIT: Duration = Duration::from_secs(5 * 60);
/// The spec's: how long a handoff's call waits for the user to hand the tab back.
const HANDOFF_WAIT: Duration = Duration::from_secs(15 * 60);
/// A check's waits in place of ASK_WAIT, HANDOFF_WAIT and task::QUIET, in seconds, as
/// `--agent-waits=<ask>,<handoff>[,<quiet>]`, so a run sees each end without waiting minutes, or a
/// run whose own work pauses its calls keeps one task. Not read in a prod build.
const WAITS_SWITCH: &str = "--agent-waits";

/// How long an ask and a handoff wait, and a session quiet after its last page: the spec's, or a
/// check's from the launch's arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Waits {
    ask: Duration,
    handoff: Duration,
    quiet: Duration,
}

impl Waits {
    fn new(given: &[String], prod: bool) -> Waits {
        let spec = Waits {
            ask: ASK_WAIT,
            handoff: HANDOFF_WAIT,
            quiet: task::QUIET,
        };
        let prefix = format!("{WAITS_SWITCH}=");
        let Some(value) = (!prod)
            .then(|| (given.iter().rev()).find_map(|arg| arg.strip_prefix(prefix.as_str())))
            .flatten()
        else {
            return spec;
        };
        let seconds = |word: &str| word.parse::<u64>().ok().map(Duration::from_secs);
        match value.split(',').collect::<Vec<_>>()[..] {
            [ask, handoff, ref quiet @ ..] if quiet.len() <= 1 => Waits {
                ask: seconds(ask).unwrap_or(spec.ask),
                handoff: seconds(handoff).unwrap_or(spec.handoff),
                quiet: (quiet.first().and_then(|quiet| seconds(quiet))).unwrap_or(spec.quiet),
            },
            _ => spec,
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
/// Why an action asks on a site the user fenced, which fence.rs words for the card.
const FENCED: &str = "fenced";
/// Why an act is refused on a site outside its task's Sites, as FENCED is a stand-in.
const OTHER_SITE: &str = "other site";
/// The tools whose call on a tab that came to a fenced site asks first: each reads or acts on its
/// page. Leaving it, closing it, handing it to the user and arranging tabs don't.
const FENCE_ASKS: [&str; 10] = [
    "read_page",
    "snapshot",
    "screenshot",
    "click",
    "type",
    "select",
    "press_key",
    "scroll",
    "run_steps",
    "request",
];
/// Why an agent's download asks first.
const DOWNLOADS: &str = "It saves a file to your computer.";
/// What an agent's download answers with, as it can't read the file.
const NO_FILES: &str = "Agents can't open files; ask the user if you need it.";
/// Ours: how long an agent's download waits to answer with its file, as a large one or one Chrome
/// holds as dangerous may take a while.
const DOWNLOAD_WAIT: Duration = Duration::from_secs(30);
/// Why an agent can't open a page that isn't on the web.
const WEB_ONLY: &str = "Agents may open only http and https pages.";
/// Why an agent can't read a page that isn't on the web, in the core's words (AgentPage::NotWebPage).
const NOT_WEB_PAGE: &str =
    "The tab's page isn't an http or https page, the only pages agents may use.";
/// list_tabs' title for a tab of the agent's own off the web, in place of its title and address.
const OFF_THE_WEB: &str = "A page that isn't http or https";
/// list_tabs' words for a tab the user holds, in place of its page's title and address (#3694).
const WITH_THE_USER: &str = "With the user until they hand it back";
/// The words for a tab whose title and address are withheld (Browser::withheld), in their place in
/// everything an agent gets or the app keeps for agents: all it says is that the page has to move
/// on.
pub const WITHHELD: &str = "Withheld until the tab goes to another page";
/// The core's refusal of a tab not in the agent's Space (bc_agent_may_act).
const NOT_ITS_SPACE: &str = "The tab isn't in the agent's own Space.";

/// How many pages' reasons for leaving Agents::gone keeps.
const GONE_KEPT: usize = 512;

/// Why a session's page left its agent's Space (Agents::gone).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Gone {
    /// The user closed it, or its own page did.
    Closed,
    /// The user moved it to a Space of theirs.
    Moved,
    /// The user opened it from the card of the agent's earlier session.
    Opened,
    /// It closed with the agent's earlier session, as the user put its card away.
    PutAway,
    /// The agent closed it.
    Itself,
    /// It closed as its download started, as it showed no page.
    Download,
    /// It closed as the agent removed this tab, it or one of its split, from its pinned tabs.
    Removed(u64),
}

impl Gone {
    /// What a call on `tab` answers, in place of NOT_ITS_SPACE.
    fn why(self, tab: u64) -> String {
        let went = match self {
            Gone::Closed => format!("Tab {tab} was closed, by the user or by its own page."),
            Gone::Moved => {
                format!("The user moved tab {tab} to one of their own Spaces: it's theirs now.")
            }
            Gone::Opened => format!(
                "The user opened tab {tab} from the card of the agent's earlier session: it's \
                 theirs now."
            ),
            Gone::PutAway => format!(
                "Tab {tab} closed with the agent's earlier session, as the user put its card away."
            ),
            Gone::Itself => format!("The agent closed tab {tab}."),
            Gone::Download => {
                format!("Tab {tab} closed as its download started, as it showed no page.")
            }
            Gone::Removed(removed) if removed == tab => {
                format!("The agent removed tab {tab} from its pinned tabs, which closed it.")
            }
            Gone::Removed(removed) => format!(
                "Tab {tab} closed with its split view, as the agent removed tab {removed} from its \
                 pinned tabs."
            ),
        };
        format!("{went} To carry on, open the page again with open_tab.")
    }
}
/// Why an agent can't type into a password field.
const PASSWORDS: &str = "Password fields are off limits to agents: signing in is the user's to do. \
    Hand them the tab with hand_off.";
/// What a choice adds when its field doesn't show the option after.
const UNSHOWN: &str = "It doesn't show it as chosen, so the page may have changed it back.";
/// What the user reads in the bar of a tab whose page asked for their passkey.
const PASSKEY: &str = "The page asks for your passkey. Sign in, then hand the tab back.";

/// What a handoff for a sign-in the browser asks for tells the user, by `from`, the first line of
/// its dialog's message: the address the request it challenged went to, as Chrome shows it, or a
/// proxy's line. Its host names who asks, which can be a frame's or a request's, not the page's.
fn sign_in_why(from: &str) -> String {
    match site(from) {
        host if web_site(&host) => {
            format!("{host} asks you to sign in. Hand the tab back when you’re done.")
        }
        _ => "A proxy asks you to sign in. Hand the tab back when you’re done.".into(),
    }
}

/// What the call that met a sign-in hears first once its wait ends (`sign_in_hand_off`).
const SIGNED_IN: &str = "A browser sign-in dialog opened on the tab, so the browser handed the \
    tab to the user and this call waited. Nothing they typed is shown.";

/// Why no agent may see or act on a tab showing `url`, or pinned to `pinned`, even in its own
/// Space, or None. A Board's tab, by either, is the user's alone, as a Board and what's on it are,
/// so the refusal is the one for a tab not the agent's. A page that isn't http(s), such as the
/// browser's own or a file, is one agents never use: no title, address, text or picture of it
/// reaches one. A tab with no page yet is its pinned URL's. One with no address at all, or only
/// the empty start page's, hasn't committed its first page and holds nothing of anyone's: the core
/// waits for that page and checks it then, as it does for every page action.
pub fn off_limits(url: &str, pinned: &str) -> Option<&'static str> {
    if crate::boards::id_in(url).is_some() || crate::boards::id_in(pinned).is_some() {
        return Some(NOT_ITS_SPACE);
    }
    let url = if url.is_empty() { pinned } else { url };
    if url.is_empty() || is_start_page(url) {
        return None;
    }
    (!is_web(url)).then_some(NOT_WEB_PAGE)
}

/// The empty page a tab or popup starts on before its first page commits.
fn is_start_page(url: &str) -> bool {
    url.split(['#', '?']).next() == Some("about:blank")
}

fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// A new token: 32 random bytes, in hex.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the system has no random numbers");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A URL's host, without "www.", for the log. Only a web page's host names its site: another
/// page's address can hold a file's path or an extension's id, so for one the log says only what
/// kind of page it is.
pub fn site(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return String::new();
    };
    if !matches!(scheme, "http" | "https") {
        let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
        return match scheme {
            "chrome" if matches!(host, "board" | "settings") => host,
            "chrome" => "browser",
            "file" => "file",
            "chrome-extension" => "extension",
            _ => "page",
        }
        .to_string();
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    let host = match host.split_once(']') {
        Some((address, _)) => &host[..address.len() + 1],
        None => host.split(':').next().unwrap_or_default(),
    };
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

/// The kinds of page `site` names for a page off the web, whose address it doesn't give.
const NOT_WEB: [&str; 6] = ["board", "settings", "browser", "file", "extension", "page"];

/// Whether `site`, as `site` gives it, is a web page's host rather than a kind of page.
pub fn web_site(site: &str) -> bool {
    !site.is_empty() && !NOT_WEB.contains(&site)
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

/// An agent's sentence for a request as an opened log entry shows it, marked as its words:
/// "Claude Code said: “Close item 4127.”". It's plain already, and plain again for one kept
/// before it was made so.
pub fn said(agent: &str, words: &str) -> String {
    format!(
        "{} said: “{}”",
        request::plain(agent),
        request::plain(words)
    )
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
