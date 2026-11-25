//! Agent tasks: the user asks in ⌘T, and their own `claude` or `codex` does it in the browser
//! (Agent task). Each task is an agent of its own, `task-<n>`, with a token the server knows, so
//! the rules, the activity log, the confirmations and the handoffs are any agent's. Its Space is
//! hidden, in the profile of the Space it was asked in, and it shows as a card at the sidebar's
//! foot. An outside agent's session, from a client the user connected, is a task too,
//! `session-<n>`, with the same card (Agent task › Outside agents): one a client at a time, whose
//! calls, on any of its connections, come as its client's agent, in that agent's hidden Space. Tasks
//! and sessions live as long as the browser runs: their Spaces go when a task is dropped, at quit, and at the next launch for
//! any a crash left.

use std::collections::{HashMap, VecDeque};
use std::iter::Peekable;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use std::time::{Instant, SystemTime};

use futures::StreamExt;
use gpui::{App, AppContext, Context, SharedString};
use serde_json::Value;

use super::files::{self, Given, Names, Never, Picked, never_grantable};
use super::helpers::{self, Helpers};
use super::ready::{Found, Ready};
use super::runner::{self, Cli, Failure, Launch, Out, Process};
use super::sign_in::Offer;
use super::tier::{self, Sites, StartedWith, Stop, Tier};
use super::{Agents, Client, Question, ToolResult, notes, page, site, tools};
use crate::shell::MenuEntry;

pub mod handed;
pub mod ilo;
mod pick_up;
pub(super) mod revoke;
pub(super) mod rewall;
pub use revoke::FolderInUse;

/// The answers an ask shows as buttons on the card, at most, and how long each may be.
pub const ANSWERS: usize = 3;
pub const ANSWER_LENGTH: usize = 24;
/// A done card's line, at most, which finish asks the agent's summary to keep to. The card's own
/// ellipsis cuts one longer.
pub const SUMMARY_LENGTH: usize = 28;
/// The pages a finished task may leave the user, at most.
pub const PAGES: usize = 10;
/// The dismissed tasks ⇧⌘T and ⌘Z can bring back. An older one is dropped, its Space with it.
const KEPT: usize = 10;
/// How long a turn's Thinking waits for the CLI's first step or message before it says the CLI
/// didn't answer: past the longest a model thinks before it does anything.
pub const NO_ANSWER: Duration = Duration::from_secs(180);
/// How long an outside agent's session waits for its next call, with none under way, before it's
/// done (#1080, for now): a client can stay connected long after its user has moved on.
pub const IDLE: Duration = Duration::from_secs(10 * 60);
/// How long a session waits for its next call once the agent closed its last page, before it's
/// done (#3652). It's the end for an agent that doesn't call done, so it's patient: a model can
/// think for most of a minute between closing one page and opening the next.
pub const QUIET: Duration = Duration::from_secs(60);

/// How much of the agent's name for its task is kept: the tool asks for 24 characters, which a
/// model overshoots by a few, and each surface cuts at its own width as any title does (#3690).
pub const TITLE_KEPT: usize = 40;

/// How much of a done task's last answer its card keeps, before the card's three lines cut it
/// (spec-thread-card).
pub const FINDING_KEPT: usize = 160;
/// How much of an answer's first line a screen reader is told: a sentence or two, where a card's
/// line has the width of a title.
pub const TOLD_KEPT: usize = 120;

/// `text` as a title holds it: kept to TITLE_KEPT characters, then written out as a request's text
/// is, on one line (#3645, #3690). An assistant's name for its thread and a tab's title standing in
/// for one are both words ilo didn't write, and both go through here.
fn as_title(text: &str) -> String {
    let kept = page::clip(text, TITLE_KEPT);
    super::request::escaped(&kept, false).0
}

/// How long a task's title stays before it may change, so it can't flicker (#3690).
pub const RENAMING: Duration = Duration::from_secs(60);

/// The answer to a new name sooner than RENAMING after the last.
pub const NAMED_RECENTLY: &str =
    "The task was named under a minute ago; name it again when you move on to another task.";

/// A task's state, as its card shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Working,
    /// The agent asks the user (ask), and waits for the answer.
    Asking {
        question: SharedString,
        answers: Vec<SharedString>,
    },
    /// The agent finished, with its result, the line its card says it in, and the tabs it left the
    /// user to open, in its order, until each is opened.
    Done {
        result: SharedString,
        summary: Option<SharedString>,
        tabs: Vec<u64>,
    },
    /// The user's plan is at its limit, until this time, as the clock reads it.
    Limit {
        resets: Option<SharedString>,
    },
    Stopped,
    /// It couldn't go on: why, and whether its assistant's row fixes it, as it does when the
    /// assistant isn't signed in or isn't installed.
    Failed {
        why: SharedString,
        assistant: bool,
    },
}

/// A line of the conversation, and when it was said, which ends the stretch of work before it.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub who: Who,
    pub text: SharedString,
    pub at: Instant,
}

/// Who said a message, and as what: the conversation folds what the agent says as it works.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Who {
    User,
    Agent,
    /// The agent's question (ask).
    Asked,
    /// The agent's result (finish).
    Result,
}

impl Message {
    fn new(who: Who, text: impl Into<SharedString>) -> Self {
        Self {
            who,
            text: text.into(),
            at: Instant::now(),
        }
    }
}

/// What a task's conversation shows, in the page's place (Agent task): compared with the last, so
/// it redraws only when it changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Conversation {
    pub agent: SharedString,
    /// The agent's name, in text: Claude, Codex, or an outside agent's.
    pub name: SharedString,
    pub title: SharedString,
    /// The agent, its model and where the task is, under the title: "Claude · Haiku · Working".
    pub about: SharedString,
    /// What was asked, and the page it was asked on, by its title.
    pub asked: SharedString,
    pub page: Option<AskedOn>,
    pub lines: Vec<Line>,
    /// Its pages, for their pictures, each by its tab and site.
    pub tabs: Vec<(u64, SharedString)>,
    /// The tab the agent is on while it works, whose picture has the Signal ring.
    pub on: Option<u64>,
    /// Whether it works or waits on the user, which ⌘. stops.
    pub stoppable: bool,
    /// What the live line over the reply field says of it, read where its card's word is, so the
    /// two never disagree.
    pub live: Live,
    /// Open under the result, which moves the tabs it left the user to them: "Open maat.pt", or
    /// "Open 2 pages".
    pub open: Option<SharedString>,
    /// The answers to the question it waits on, as chips under it.
    pub answers: Vec<SharedString>,
    /// Whether its last line is the message the agent is writing, which comes in as it's written.
    pub writing: bool,
    /// Its access, which the chip under the reply field shows and its menu changes.
    pub tier: Tier,
    /// Whether it may go only to the sites of the pages the user added.
    pub only_added: bool,
    /// The folder a live task holds, by its name as the system's panel showed it, which the chip
    /// under the reply field shows with its one act, Revoke.
    pub folder: Option<SharedString>,
}

/// Where a task under way is, as the live line over its reply field says it: working only while
/// nothing of it waits on the user and the user doesn't have its page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Live {
    /// It works: the step under way, else `{assistant} is working`, with Take Over on its page.
    #[default]
    Working,
    /// A step ilo refused waits on the user (its card says Needs you).
    Refused,
    /// The agent handed the user a step on its page, or asks for the page back, and its call
    /// waits (its card says Needs you).
    Handed,
    /// The user took its page `tab` over, and the agent has no step under way on another: `site`
    /// names it while the task has more than one page, as ilo's line of a handed page does.
    Yours {
        tab: u64,
        site: Option<SharedString>,
    },
    /// Its own question waits on the user, which the question's line says with its answers.
    Asked,
}

/// A line of a task's conversation.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    You(SharedString),
    Said(SharedString),
    /// A stretch of work between two messages.
    Trace(Trace),
    /// A step of an outside agent's session, whose stretches and messages are in its terminal.
    Step(TraceStep),
    /// The turn the user sent, from the moment they sent it until the agent's first step or
    /// message takes its place: Thinking, or why the agent couldn't start on it. Its key is the
    /// one the turn's trace takes.
    Thinking {
        key: usize,
        failed: Option<SharedString>,
    },
    /// ilo's own line, which only the app makes (ilo.rs).
    Ilo(ilo::IloLine),
}

/// A stretch of the agent's work between two messages: a summary line, "Working · 5 steps" or
/// "Worked for 41 s · 8 steps", with its steps under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// Its first step's key, which keeps the user's opening or folding of it.
    pub key: usize,
    /// Whether it's the stretch under way, which shows open unless the user folded it.
    pub running: bool,
    /// How long it worked, in seconds, once it's over.
    pub took: Option<u64>,
    /// Its steps, with what the agent said between them.
    pub items: Vec<TraceItem>,
}

impl Trace {
    /// Its steps, not its rows: a row that's the same step on several tabs counts each.
    pub fn steps(&self) -> usize {
        (self.items.iter())
            .map(|item| match item {
                TraceItem::Step(step) => match &step.opens {
                    Opens::Steps(each) => each.len().max(1),
                    _ => 1,
                },
                _ => 0,
            })
            .sum()
    }

    /// The helpers the task started in it (#4102), those that have finished too, as each one's
    /// line stays.
    pub fn helpers(&self) -> usize {
        (self.items.iter())
            .filter(|item| matches!(item, TraceItem::Helper { .. }))
            .count()
    }

    /// What its summary line counts: its helpers if it has any, as each is on the user's plan,
    /// else its steps. Under way or over, so the line counts the same thing as the turn ends.
    pub fn counted(&self) -> (usize, Counted) {
        match self.helpers() {
            0 => (self.steps(), Counted::Steps),
            helpers => (helpers, Counted::Helpers),
        }
    }

    /// Its summary line in one: "Working · 4 helpers", which the task's card says too.
    pub fn line(&self) -> SharedString {
        let (title, counted) = self.summary();
        format!("{title} · {counted}").into()
    }

    /// Its summary line's title and description.
    pub fn summary(&self) -> (SharedString, SharedString) {
        summary(self.took, self.counted())
    }
}

/// A stretch's summary line, its title and description: one that `took` its time, or one under
/// way, and what it counts.
fn summary(took: Option<u64>, counted: (usize, Counted)) -> (SharedString, SharedString) {
    let counted = match counted {
        (1, Counted::Steps) => "1 step".to_string(),
        (count, Counted::Steps) => format!("{count} steps"),
        (1, Counted::Helpers) => "1 helper".to_string(),
        (count, Counted::Helpers) => format!("{count} helpers"),
    };
    let title = match took {
        Some(seconds) => format!("Worked for {}", duration(seconds)),
        None => "Working".to_string(),
    };
    (title.into(), counted.into())
}

/// What a trace's summary line counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counted {
    Steps,
    Helpers,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TraceItem {
    Step(TraceStep),
    /// What the agent said as it worked, which isn't its turn's last message.
    Said(SharedString),
    /// A helper the task started (#4102), as one line. Nothing on it is the helper's own text or
    /// what it found.
    Helper {
        /// The step it's on, in ilo's own words for the page it works on.
        step: TraceStep,
        /// The name its task gave it as it asked for it, the task's own words as its title is:
        /// one line of plain text, cut short, held in memory alone. Empty if it gave none.
        name: SharedString,
        /// The page it works on, by its tab.
        tab: Option<u64>,
    },
}

/// A line of a trace: a step, in one sentence.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceStep {
    /// Its place in the task's trace, which keeps the user's opening of it.
    pub key: usize,
    pub glyph: &'static str,
    pub title: SharedString,
    /// After the title: how long it took, from 1 s, or why it failed.
    pub description: SharedString,
    /// Whether it's the step under way, which shimmers.
    pub running: bool,
    pub failed: bool,
    pub opens: Opens,
    /// The line under it, or empty: "With ilo's notes for GitHub" (#4122).
    pub under: SharedString,
}

/// What a trace's line opens to.
#[derive(Clone, Debug, PartialEq)]
pub enum Opens {
    Nothing,
    /// A thought's text, or a failed step's error.
    Text(SharedString),
    /// What a look at the page saw.
    Picture(Arc<gpui::Image>),
    /// A turn's same step on several tabs: each tab's line (#1476).
    Steps(Vec<TraceStep>),
}

/// A step of a task's work as the browser saw it, timed by the browser's own clock: a tool call,
/// a run's step, or a thought the CLI sent.
#[derive(Clone, Debug, PartialEq)]
pub struct Traced {
    /// The call it's of, whose answer ends it, or 0 for a thought.
    pub call: u64,
    pub did: tools::Did,
    /// How many of the task's messages came before it, which places it among them.
    pub after: usize,
    pub started: Instant,
    pub ended: Option<Instant>,
    /// Why it failed, as its call's answer said.
    pub error: Option<SharedString>,
    /// The kind of its failure, which its card says in a few words.
    pub kind: Option<tools::Kind>,
    pub picture: Option<Arc<gpui::Image>>,
    /// How a close ended when its page had a say.
    pub close: Option<tools::Close>,
    /// The site whose notes its call's answer carried (#4122), by its note's name. From the
    /// answer's own field, which only the app sets, never from the answer's text.
    pub notes: Option<&'static str>,
}

impl Traced {
    /// Whether it was under way when the user pressed Stop, which answered it as stopped: no step
    /// that failed, and it leaves no line, in the trace or on a card, as a stopped turn has no
    /// answer.
    pub fn stopped(&self) -> bool {
        self.kind == Some(tools::Kind::Stopped)
    }
}

/// The page the user was on when they asked, which goes as its address and title only.
#[derive(Clone, Debug, PartialEq)]
pub struct AskedOn {
    pub url: SharedString,
    pub title: SharedString,
}

pub struct Task {
    pub agent: String,
    /// The CLI a task the user asked runs on, or None for an outside agent's session.
    pub cli: Option<Cli>,
    /// Who works on it, as its card says: the CLI's name, or the session's client's.
    pub name: SharedString,
    pub session: Option<Session>,
    /// The user's Space it was asked in, whose Today it shows at the top of.
    pub home: u64,
    /// Its own Space, hidden, where its tabs open.
    pub space: u64,
    pub asked: SharedString,
    pub page: Option<AskedOn>,
    /// The name the agent gives it (title), else what was asked, or for a session, its sites or
    /// its client's name (#3690). Written out as a request's text is, so it's one line.
    pub title: SharedString,
    /// When the agent last named it, which a new name waits RENAMING after.
    pub(super) named: Option<Instant>,
    /// The model it runs on, by name, or empty for its CLI's own default.
    pub model: SharedString,
    /// The model it was started on by the user's pick in the composer, by id, empty for its CLI's
    /// own default; None for Settings' Model, read at each launch.
    pub(super) model_id: Option<String>,
    /// The addresses of the pages the user added to it, which its first message names.
    pub(super) added: Vec<String>,
    pub state: State,
    /// When it finished, done, stopped or failed, for its conversation and the Archive.
    pub ended: Option<SystemTime>,
    /// The id the log's next entry took as it ended (`Task::end`).
    pub(super) end_entry: u64,
    /// When it started, and when it ended as Library › Agents last kept it.
    pub started: SystemTime,
    pub(super) kept: Option<SystemTime>,
    /// The orders it placed and the messages it sent, each once the user allowed it and the page
    /// did it, for Library › Agents. A session counts its own, for its receipt.
    pub(super) orders: usize,
    pub(super) sent: usize,
    pub messages: Vec<Message>,
    /// Its steps, oldest first.
    pub trace: Vec<Traced>,
    pub dismissed: bool,
    /// When it last left the foot for the Archive, which a task with no end is listed by. Kept
    /// in memory only.
    pub left: Option<SystemTime>,
    /// Deleted from the Archive: listed nowhere and out of ⇧⌘T's reach, until ⌘Z brings it back
    /// or it goes for good with the dismissed tasks past the last few.
    pub deleted: bool,
    token: String,
    process: Option<Process>,
    /// The ask's call, which waits for the user's answer, with the number it takes among the
    /// agent's prompts, which show in the order they came.
    asking: Option<(u64, mpsc::Sender<ToolResult>)>,
    /// Its access, which only the user raises and which ends with it: never kept, so Full access
    /// is never remembered. An outside client's starts at what the user left that client's others
    /// at, and comes down to an ended one's as it takes that one's page (`starting_tier`,
    /// `left_at`).
    pub tier: Tier,
    /// The sites it may use.
    pub sites: Sites,
    /// What ilo stopped, as its conversation shows each, oldest first.
    pub stops: Vec<Stop>,
    /// The turns the user stopped, each by how many of their messages came before its answer: a
    /// stopped turn has no answer, so all it said stays in its stretch of work.
    pub stopped_turns: Vec<usize>,
    /// The level it ended at, once the user has picked another in the level's menu since: the
    /// pick is the level its next reply starts at, and this the one its wall was last put up at.
    pub(super) picked_from: Option<Tier>,
    /// ilo's other lines of what happened to it: its steps handed to the user, the tabs its
    /// agent asked back and the changes of its access, oldest first.
    pub(super) placed: Vec<super::lines::Placed>,
    /// What ilo tells its CLI with the user's next turn: a change of tier while it wasn't working.
    pub(super) told: Option<String>,
    /// While it's thinking, what ends the turn if its CLI never answers (NO_ANSWER). It goes as
    /// the CLI answers, so nothing waits once the agent is under way.
    pub(super) waiting: Option<gpui::Task<()>>,
    /// The message the agent is writing at the end of the turn under way, as it comes.
    pub(super) writing: Option<Writing>,
    /// What ilo holds of its helpers (#4102), which it has only if it started with the switch on.
    pub(super) helpers: Helpers,
    /// The files the user gave it (#4101), which its assistant then runs inside a wall for. None
    /// until a grant gives it some.
    pub(in crate::agents) files: Given,
    /// What its files are shown by, in its conversation alone, as long as it has them.
    pub(in crate::agents) names: super::files::Names,
    /// Its start inside a wall, until the system says the wall is up (rewall.rs).
    pub(super) rising: Option<rewall::Rising>,
    /// What came of each wall of its that wasn't put up, in order, each with where it came among
    /// the task's messages and steps, so its line stands where it happened: what its lines about
    /// them are written from, by the conversation's words table.
    pub(in crate::agents) rewalled: Vec<rewall::Came>,
    /// While ilo copies its attached files, before its assistant first starts: nothing starts or
    /// tells a run until the copies end, and what ilo or the user had to say meanwhile is in
    /// `told`, which that one start carries (`attached`).
    pub(in crate::agents) copying: bool,
    /// What its messages of the user's carried of their tabs (handed.rs). In memory alone.
    pub(in crate::agents) sends: handed::Sends,
}

/// The message an agent is writing as the turn's last, which shows under its trace as it comes,
/// until a call it goes on to make takes it into the trace.
#[derive(Clone, Debug, PartialEq)]
pub struct Writing {
    /// Its text so far.
    pub text: SharedString,
    /// Whether it has come whole, as the turn's last message.
    pub said: bool,
}

/// An outside agent's session: a client the user connected at work on one of its connections,
/// from its first call there that opens or acts on a tab until that connection closes, the user
/// stops it, or IDLE passes. A client's sessions share its Space and rules, and each has its own
/// card; the `ilo` command's one spans its connections (`session_connection`).
pub struct Session {
    /// The agent's id, which its rules, its Space and its activity go by.
    pub agent: String,
    /// The server's connection its calls come on, as `session_connection` has it.
    pub connection: u64,
    /// Its pages: the tabs it opened or acted in, in that order, but for one another session
    /// under way has among its own.
    pub tabs: Vec<u64>,
    /// The orders it placed and the messages it sent, each once the user allowed it and it was
    /// done, for its receipt. For this run only, as the session is.
    pub(super) orders: usize,
    pub(super) messages: usize,
    /// Its calls under way, which it waits on.
    pub(super) calls: Vec<u64>,
    /// What ends it once it has waited IDLE for its next call.
    idle: Option<gpui::Task<()>>,
    /// What ends it once it has waited QUIET for its next call after closing its last page.
    quiet: Option<gpui::Task<()>>,
    /// When its last call ended, by the executor's clock: when its work stopped, which its card
    /// says whatever ends it.
    last: Option<Instant>,
    /// When its title from its sites last changed, which the next change waits RENAMING after.
    renamed: Option<Instant>,
    /// The user picked a level on another task of its client's after it ended, so its own is no
    /// longer one that client's tasks are held to.
    passed: bool,
    /// When the user stopped it. A call of its client's that came before then is the stopped
    /// task's own, whatever it would open, and is answered as stopped (#4209).
    pub(super) stopped: Option<Instant>,
    /// Its pages are refused to its client, on every connection and at every level, and no
    /// other task takes one: from the user's Stop until they hand them back on its stopped line.
    pub(super) withheld: bool,
}

impl Task {
    /// The title its card and its panel show: its own, or for a thread started with a tab and no
    /// words, which has none until its assistant names it, what stands in for one. `title` itself
    /// stays empty until then, so what is kept, logged or told of a thread's title never has a
    /// page's words.
    fn shown_title(&self) -> SharedString {
        match self.title.is_empty() {
            true => self.sends.stand_in(),
            false => self.title.clone(),
        }
    }

    /// It ended `at`. Its record keeps the log's entries from before then, and none noted after,
    /// such as its agent's disconnect, which the Mac's clock, in microseconds, can give the very
    /// time it ended (#3967).
    fn end(&mut self, at: SystemTime) {
        self.ended = Some(at);
        self.end_entry = super::next_entry_id();
        // A refusal is passed once its task ends: there's nothing left to raise or take over.
        for stop in &mut self.stops {
            stop.settled = true;
        }
    }

    pub(super) fn token(&self) -> &str {
        &self.token
    }

    /// Stops its turn under way, as Stop's press does, and leaves its state as it is.
    pub(super) fn interrupt(&mut self) {
        if let Some(process) = self.process.as_mut() {
            process.interrupt();
        }
    }

    /// Ends its run: the process goes, and a message to the task starts another.
    pub(super) fn end_run(&mut self) {
        self.process = None;
    }

    /// Where the turn under way starts among its messages: after the user's last.
    fn turn(&self) -> usize {
        self.messages
            .iter()
            .rposition(|message| message.who == Who::User)
            .map_or(0, |at| at + 1)
    }

    /// Whether the turn is sent and its CLI hasn't answered yet: no step and no message since.
    fn heard_nothing(&self) -> bool {
        let turn = self.turn();
        self.messages.len() == turn
            && self.trace.last().is_none_or(|step| step.after < turn)
            && self.writing.is_none()
    }

    fn thinking(&self) -> bool {
        self.state == State::Working && self.heard_nothing()
    }

    /// `text` waits in `told` for the task's next start, after what waits there already.
    pub(in crate::agents) fn hold(&mut self, text: String) {
        self.told = Some(match self.told.take() {
            Some(told) => format!("{told}\n\n{text}"),
            None => text,
        });
    }
}

/// What a task's card shows: compared with the last, so the card redraws only when it changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub agent: SharedString,
    pub home: u64,
    /// The task's own Space, which the page area shows it from.
    pub space: u64,
    /// Who works on it: Claude, Codex, or an outside agent's name as its client gives it.
    pub name: SharedString,
    /// Whether the user talks to it here: a task, not an outside agent's session, whose user talks
    /// to it where they started it.
    pub replies: bool,
    pub title: SharedString,
    /// Whether its title stands in for one it doesn't have yet: a tab's title, a page's words, or
    /// New Thread. Drawn as any title is, and written to no log line.
    pub unnamed: bool,
    /// The step it's on, such as "Opening gulbenkian.pt", what it waits for, or its result's line.
    pub status: SharedString,
    /// Its last steps this turn, at most `CARD_STEPS`, oldest first, each as the conversation's
    /// trace line says it (`tools::step_title`).
    pub steps: Vec<SharedString>,
    /// The page whose tile Latest tasks' plate pictures: of its pages now, the last its steps
    /// acted on, else its last; None with no page.
    pub last_tab: Option<u64>,
    /// Latest tasks' line under the title once it's over: a done task's answer's first line, a
    /// stopped one's last finished step. None for the rest, and for a task a run before left,
    /// whose answer and steps no run keeps.
    pub line: Option<SharedString>,
    /// When a task a run before left ended, which its card's head says after its word; None for
    /// a task of this run.
    pub ended_at: Option<SharedString>,
    /// Whether the status is the agent's question, which may take two lines.
    pub asks: bool,
    pub dot: Dot,
    /// Its state in one word, as every place that names it says it.
    pub word: Word,
    /// Its pages now, in its Space's order: the icons on its card in the sidebar.
    pub pages: Vec<u64>,
    /// Those of `pages` a step of its has been on, which its card shows at full strength.
    pub visited: Vec<u64>,
    /// When it ended in this run, in the user's time ("10:42"), or empty while it hasn't: a done
    /// card's `Done · {time}`.
    pub ended_time: SharedString,
    /// Done, the first paragraph of its last answer as plain text (`finding`), from this run's
    /// conversation alone: empty when it isn't done, ended with no answer, or came back after a
    /// quit, which keeps no conversation.
    pub finding: SharedString,
    /// Done, what a screen reader is told of its finding (`told`): empty with none.
    pub told: SharedString,
    /// Its access, which its group's line leads with at Full access.
    pub tier: Tier,
    /// A confirmation or a handoff of the task's, which the card asks instead of the windows'.
    pub question: Option<Question>,
    /// Done's tabs to open, each with its site, in the agent's order.
    pub open: Vec<(u64, SharedString)>,
    /// Each of its pages' own step of the turn under way, while it works on more than one: what
    /// the strip under a page says, never another page's (#4102).
    pub page_steps: Vec<(u64, SharedString)>,
    /// Limit's Retry.
    pub retry: bool,
    /// A failure's fix, its assistant's row's own button: Sign in to …, Cancel or Get ….
    pub fix: Option<(Cli, Offer)>,
}

impl Card {
    /// Whether the task is over and has nothing left to do: done, stopped or failed. One at its
    /// plan's limit isn't, as its Retry waits for the reset.
    pub fn finished(&self) -> bool {
        self.dot == Dot::Done || (self.dot == Dot::Ended && !self.retry)
    }
}

/// What the card's favicon column shows of the task's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dot {
    /// Signal, breathing, as a tab's while an agent works in it.
    Working,
    /// Ringed and still: it waits for the user.
    Waiting,
    /// A tick.
    Done,
    /// A hollow ring: stopped, at its limit, or it couldn't go on.
    Ended,
}

/// A task's state in one word. Its card, the list of tasks, the Archive's rows and the strip under
/// its page all name it from here, so no two of them say it differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Word {
    Working,
    /// A question, a handoff or a refusal waits on the user.
    NeedsYou,
    Done,
    /// The user stopped it.
    Stopped,
    /// Its assistant's program quit or broke.
    Interrupted,
    /// Its assistant is at its own usage limit.
    AtLimit,
}

impl Word {
    /// The word for `state`; `waits` says whether something of the task's waits on the user.
    pub fn of(state: &State, waits: bool) -> Self {
        match state {
            State::Working if waits => Self::NeedsYou,
            State::Working => Self::Working,
            State::Asking { .. } => Self::NeedsYou,
            State::Done { .. } => Self::Done,
            State::Stopped => Self::Stopped,
            State::Failed { .. } => Self::Interrupted,
            State::Limit { .. } => Self::AtLimit,
        }
    }

    /// After the assistant's name and a middle dot: "Claude · At limit".
    pub fn after_name(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::NeedsYou => NEEDS_YOU,
            Self::Done => "Done",
            Self::Stopped => "Stopped",
            Self::Interrupted => "Interrupted",
            Self::AtLimit => "At limit",
        }
    }

    /// The word of a card drawn without its task, as the tests and the preview draw them, for its
    /// `dot`: an ended one says Stopped.
    pub fn for_dot(dot: Dot) -> Self {
        match dot {
            Dot::Working => Self::Working,
            Dot::Waiting => Self::NeedsYou,
            Dot::Done => Self::Done,
            Dot::Ended => Self::Stopped,
        }
    }

    /// On its own, which names the assistant only when the limit is its: "Claude at limit".
    pub fn alone(self, name: &str) -> SharedString {
        match self {
            Self::AtLimit => format!("{name} at limit").into(),
            word => word.after_name().into(),
        }
    }
}

/// What the user does to a task from its card or its conversation.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskAction {
    Dismiss,
    /// ⇧⌘T or ⌘Z: the task named, or with none named, the last dismissed.
    Restore,
    /// Their reply, with the tabs of theirs picked for it, if any.
    Reply(handed::Reply),
    Retry,
    Stop,
    /// Done's Open <site> or Open 3 pages: every tab it left the user.
    Open,
    /// A page's own Open <site>, from the strip under it.
    OpenTab(u64),
    /// The Archive's Delete from Archive, and ⌘Z's undoing of it.
    Delete,
    Undelete,
    /// A failed card's Sign in to …, and its Cancel while that runs: the assistant's row's own.
    SignIn,
    CancelSignIn,
    /// An ilo line's Take over: the tab is the user's.
    TakeOver(u64),
    /// A Stopped line's step up: only the user's press sets a task's access.
    SetTier(Tier),
    /// A Sites line's Any site.
    AnySite,
    /// The level the user picked in the level's menu, which they opened: nothing moved under the
    /// pointer, so no guard holds it, and on a task that has ended it's the level its next reply
    /// starts at.
    PickTier(Tier),
    /// The level's menu's Any site.
    PickAnySite,
    /// An ilo line's Hand back: the tab goes back to the agent.
    HandBack(u64),
    /// The stopped line's Hand Back, on an outside agent's task: its pages are its client's
    /// again. Only the user's press does it.
    PagesBack,
    /// A step's Not now, by its line's key.
    NotNow(usize),
    /// Revoke on the folder's chip under the reply field: only the user's press takes a folder
    /// back, as on Settings › Agents › Folders in use.
    RevokeFolder,
    /// Return to Your Tabs on a tab the user handed to the thread: it's theirs again, for good.
    /// Only the user's press does it.
    ReturnTab(u64),
}

/// What the composer starts a task with beside what was asked: the user's picks on its line, which
/// hold for this task alone and change nothing in Settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Start {
    pub tier: Tier,
    /// The model picked for it, by id, empty for its CLI's own default; None for Settings' Model.
    pub model: Option<String>,
    /// The addresses of the pages the user added on the `+` menu, which the task opens in tabs of
    /// its own: never a tab of the user's, nor its title or anything of its state.
    pub pages: Vec<String>,
    /// Only the sites you add: the task may go only to the sites of the pages added.
    pub only_added: bool,
    /// The files and the folder picked on the `+` menu (#4101), given at the start alone.
    pub files: super::files::Picked,
    /// The user's own tabs picked on the `+` menu, handed to the thread as it starts.
    pub tabs: handed::Picked,
}

/// What the composer last started a thread with in a profile, which it opens with there next:
/// its level, never Full access, and the model picked on its assistant. Nothing of the words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Last {
    pub tier: Tier,
    /// The assistant, and the model picked on it by id, empty for its own default; None for
    /// Settings' assistant and model.
    pub model: Option<(Cli, String)>,
}

/// The longest address a task gets for a page the user added: a page's over it isn't offered.
pub const ADDED_MOST: usize = 2048;

/// The address a task gets for a page the user added: an http(s) one alone, at most ADDED_MOST
/// long, without the user name or password a tab's address can carry, which a task never gets.
pub fn added_address(url: &str) -> Option<String> {
    if url.len() > ADDED_MOST {
        return None;
    }
    let mut parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    _ = parsed.set_username("");
    _ = parsed.set_password(None);
    Some(parsed.into())
}

impl From<Tier> for Start {
    fn from(tier: Tier) -> Self {
        Start {
            tier,
            ..Start::default()
        }
    }
}

/// A task the user dismissed, as the Library's Archived Tabs lists it among the tabs.
#[derive(Clone, Debug, PartialEq)]
pub struct ArchivedTask {
    pub agent: SharedString,
    pub title: SharedString,
    pub asked: SharedString,
    /// Done's tick, or Stopped's and Failed's grey dot, as its card had.
    pub dot: Dot,
    /// Who worked on it and how it ended, "Claude · Done".
    pub about: SharedString,
    /// When it ended, or left the foot with no end, in milliseconds since the Unix epoch as the
    /// archive dates its tabs, or 0 if nothing says. History lists it by then, under that day,
    /// and says it as it says its pages' times (`about_at`).
    pub ended: f64,
    /// The pages it still holds, "1 page", or nothing.
    pub pages: SharedString,
}

impl ArchivedTask {
    /// What its row says under its title, with `when` it ended as the list it's in says a time:
    /// "Claude · Done 13:31 · 1 page".
    pub fn about_at(&self, when: &str) -> String {
        about_at(&self.about, when, &self.pages)
    }
}

/// A finished task's row under its title: who worked on it and how it ended, when, and the pages
/// it still holds.
fn about_at(about: &str, when: &str, pages: &str) -> String {
    let mut about = about.to_string();
    for (between, part) in [(" ", when), (" · ", pages)] {
        if !part.is_empty() {
            about = format!("{about}{between}{part}");
        }
    }
    about
}

/// Open's words, for the tabs a task left the user: its site for one, and how many for several.
pub fn open_label(open: &[(u64, SharedString)]) -> Option<SharedString> {
    match open {
        [] => None,
        [(_, site)] => Some(format!("Open {site}").into()),
        tabs => Some(format!("Open {} pages", tabs.len()).into()),
    }
}

/// An agent as ⌘T's ask lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct AskAgent {
    pub cli: Cli,
    /// When its plan's limit resets, if it's at its limit, which dims its row.
    pub resets: Option<SharedString>,
}

/// The task CLIs' settings and what the browser last learned of them.
#[derive(Default)]
pub struct Tasks {
    pub tasks: Vec<Task>,
    /// The finished tasks' conversations ilo holds an id for (kept.rs).
    pub kept: super::kept::Kept,
    /// The profiles there are, each by its folder's name, as the core last said them: one that
    /// goes from among them was deleted, and its kept tasks go with it.
    pub(super) profiles: Vec<String>,
    /// The state each task was in when the user's reply took it up, which it goes back to if its
    /// assistant no longer has the conversation.
    pub(super) picked_up: HashMap<String, State>,
    /// The dismissed, newest last, for ⇧⌘T.
    pub(super) dismissed: VecDeque<String>,
    pub(super) next: u64,
    /// The name each outside agent gave on each of its connections (`session_connection`), by its
    /// id and the connection, for that connection's session's card.
    pub(super) names: HashMap<(String, u64), SharedString>,
    pub found: HashMap<Cli, Found>,
    pub(super) looking: bool,
    /// Whether ⌘T's ask has shown in this run, which learns again once.
    pub(super) asked_once: bool,
    /// The default agent, from Settings › Agents.
    pub default: Option<Cli>,
    /// Each CLI's model, or none for its own default.
    pub models: HashMap<Cli, String>,
    /// What the composer last started a thread with, by the folder's name of the profile it
    /// started in.
    pub(super) last: HashMap<String, Last>,
    /// When each CLI's limit resets, as its last task learned.
    pub(super) limits: HashMap<Cli, Option<SharedString>>,
    pub(super) events: Option<(
        futures::channel::mpsc::UnboundedSender<runner::Event>,
        gpui::Task<()>,
    )>,
    /// Where tasks run: a directory of their own each, under the profile's.
    pub dir: Option<PathBuf>,
    /// The sign-ins under way and what each row says of its last.
    pub(super) sign_ins: super::sign_in::SignIns,
    /// Whether a look asked for while one ran waits for it to end, as after a sign-in.
    pub(super) look_again: bool,
    /// The user's tabs the threads hold (handed.rs).
    pub(super) handed: handed::Records,
    /// Helpers for the tasks a test starts, whatever the switch says.
    #[cfg(test)]
    pub(super) helpers: bool,
    /// What answers for `wall::put_up` in a test, which no system does yet.
    #[cfg(test)]
    pub(super) wall: Option<rewall::PutUp>,
    /// The files the tasks a test starts are given, as a grant will give a task its own.
    #[cfg(test)]
    pub(super) files: Given,
    /// Whether a test's system can wall a task in with its files.
    #[cfg(test)]
    pub(super) files_available: bool,
}

impl Agents {
    /// What the composer opens with in `space`'s profile: what it last started a thread with
    /// there, or the level every thread starts at and Settings' assistant and model.
    pub fn composer_last(&self, space: u64) -> Last {
        let profile = self.browser.space_profile(space);
        self.tasks.last.get(&profile).cloned().unwrap_or_default()
    }

    /// Keeps what the user's composer started a thread with in `space`'s profile, for the next
    /// it opens there: its model, and its level unless that was Full access, which is set for one
    /// thread and never remembered. Only the composer's own start calls it.
    pub fn remember_composer(&mut self, space: u64, tier: Tier, model: Option<(Cli, String)>) {
        let profile = self.browser.space_profile(space);
        if profile.is_empty() {
            return;
        }
        let tier = match tier {
            Tier::Full => Tier::default(),
            tier => tier,
        };
        let last = Last { tier, model };
        if self.tasks.last.get(&profile) != Some(&last) {
            self.tasks.last.insert(profile, last);
            self.save();
        }
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.tasks.iter()
    }

    pub fn task(&self, agent: &str) -> Option<&Task> {
        self.tasks.tasks.iter().find(|task| task.agent == agent)
    }

    pub(super) fn task_mut(&mut self, agent: &str) -> Option<&mut Task> {
        self.tasks.tasks.iter_mut().find(|task| task.agent == agent)
    }

    /// `agent`'s run goes before the app hears it end, as a reply can find it: the next reply
    /// starts a run of its own.
    #[cfg(test)]
    pub(super) fn run_of(&self, agent: &str) -> u32 {
        let process = self.task(agent).and_then(|task| task.process.as_ref());
        process.map_or(0, runner::Process::run)
    }

    /// `agent`'s run goes before the app hears it end, as a reply can find it: the next reply
    /// starts a run of its own.
    #[cfg(test)]
    pub(super) fn run_went_unheard(&mut self, agent: &str) {
        if let Some(task) = self.task_mut(agent) {
            task.process = None;
        }
    }

    /// Every Space's task cards, newest first, for the windows to show at the top of each Today.
    pub fn shown_cards(&self, cx: &App) -> Vec<Card> {
        (self.tasks.tasks.iter().rev())
            .filter(|task| !task.dismissed)
            .map(|task| self.card(task, cx))
            .collect()
    }

    /// The tasks in the Archive, the last to leave the foot first, as `dismissed` keeps them.
    pub fn archived_tasks(&self, cx: &App) -> Vec<ArchivedTask> {
        (self.tasks.dismissed.iter().rev())
            .filter_map(|agent| self.task(agent))
            .filter(|task| task.dismissed && !task.deleted)
            .map(|task| {
                let dot = match task.state {
                    State::Done { .. } => Dot::Done,
                    _ => Dot::Ended,
                };
                let (about, pages) = self.about_parts(task, cx);
                let since = |at: Option<SystemTime>| {
                    at.and_then(|at| at.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map_or(0., |since| since.as_secs_f64() * 1000.)
                };
                ArchivedTask {
                    agent: task.agent.clone().into(),
                    title: task.title.clone(),
                    asked: task.asked.clone(),
                    dot,
                    about: about.into(),
                    ended: since(task.ended.or(task.left)),
                    pages: pages.into(),
                }
            })
            .collect()
    }

    /// What a finished task's row says under its title in Settings › Agents' list of the kept:
    /// who worked on it, how it ended and when, and the pages it still holds, "Claude · Done 13:31
    /// · 1 page". One that ended on an earlier day says the day instead, "Claude · Done 21 Nov",
    /// as a time alone reads as today's in a list with no days over its rows. History's has them,
    /// and says when as its pages' rows do (`ArchivedTask::about_at`).
    pub(super) fn about(&self, task: &Task, cx: &App) -> String {
        let (about, pages) = self.about_parts(task, cx);
        let when = (task.ended.or(task.left)).map(|at| super::time_or_day(seconds(at)));
        about_at(&about, &when.unwrap_or_default(), &pages)
    }

    /// A finished task's row around when it ended: who worked on it and how it ended, "Claude ·
    /// Done", and the pages it still holds, "1 page" or nothing.
    fn about_parts(&self, task: &Task, cx: &App) -> (String, String) {
        let words = Word::of(&task.state, false).after_name();
        let pages = match self.open_tabs(task, cx).len() {
            0 => String::new(),
            1 => "1 page".to_string(),
            pages => format!("{pages} pages"),
        };
        (format!("{} · {words}", task.name), pages)
    }

    /// Does what the user did to `agent`'s task. Returns the tab Open moved into the user's
    /// Space, for the window to show, and the task Restore brought back.
    pub fn task_action(
        &mut self,
        agent: &str,
        action: TaskAction,
        cx: &mut Context<Self>,
    ) -> (Option<u64>, Option<String>) {
        // Whether `action` asks more of the agent: a reply to its ask only lets it finish.
        let answers = self.task(agent).is_some_and(|task| task.asking.is_some());
        let asks = match action {
            TaskAction::Reply(_) => !answers,
            TaskAction::Retry => true,
            _ => false,
        };
        // A session's user talks to its agent where they started it.
        let session = self.task(agent).is_some_and(|task| task.session.is_some());
        if session && asks {
            return (None, None);
        }
        // With tasks off nothing more is asked of a task; an answer still lets one finish.
        if !self.tasks_on() && asks {
            return (None, None);
        }
        match action {
            TaskAction::Dismiss => self.dismiss_task(agent, cx),
            TaskAction::Restore => {
                let named = Some(agent).filter(|agent| !agent.is_empty());
                return (None, self.restore_task(named, cx));
            }
            TaskAction::Reply(reply) => self.reply_with(agent, reply, cx),
            TaskAction::Retry => self.retry_task(agent, cx),
            TaskAction::Stop => self.stop_task(agent, cx),
            TaskAction::Open => return (self.open_result(agent, None, cx), None),
            TaskAction::OpenTab(tab) => return (self.open_result(agent, Some(tab), cx), None),
            TaskAction::Delete => self.delete_task(agent, true, cx),
            TaskAction::Undelete => self.delete_task(agent, false, cx),
            TaskAction::SignIn => self.sign_in_for_task(agent, cx),
            TaskAction::CancelSignIn => self.cancel_sign_in_for_task(agent),
            // A Stopped line's step up and the level's menu: the user's own press, the one way a
            // tier or a task's Sites change. A line's button isn't pressed for what was there
            // before a refusal's line appeared, nor once its task has ended.
            TaskAction::SetTier(_) | TaskAction::AnySite if self.just_stopped(agent) => {}
            // Nor for what was there before the helpers' limit's line appeared.
            TaskAction::SetTier(_) | TaskAction::AnySite
                if (self.task(agent)).is_some_and(|task| task.helpers.just_stopped()) => {}
            TaskAction::SetTier(_) | TaskAction::AnySite
                if (self.task(agent)).is_some_and(|task| task.ended.is_some()) => {}
            TaskAction::SetTier(tier) | TaskAction::PickTier(tier) => {
                self.set_tier(agent, tier, cx)
            }
            TaskAction::AnySite | TaskAction::PickAnySite => self.set_sites(agent, Sites::Any, cx),
            TaskAction::TakeOver(tab) => {
                if let Some(task) = self.task_mut(agent) {
                    task.helpers.settle(Some(tab));
                }
                self.take_over_stopped(agent, tab, cx)
            }
            // A step's line: the tab goes back to the agent, or stays the user's for now.
            TaskAction::HandBack(tab) => self.hand_back(tab, cx),
            TaskAction::PagesBack => self.pages_back(agent, cx),
            TaskAction::NotNow(key) => self.not_now(agent, key, cx),
            TaskAction::RevokeFolder => self.revoke_folder(agent, cx),
            TaskAction::ReturnTab(tab) => _ = self.return_tab(agent, tab, handed::By::User, cx),
        }
        (None, None)
    }

    /// `agent`'s task's conversation: what was asked, then each turn with what the agent said
    /// as it worked folded into its steps, so only the user's messages, the agent's questions and
    /// each turn's last message show.
    pub fn conversation(&self, agent: &str, cx: &App) -> Option<Conversation> {
        let task = self.task(agent)?;
        let ended = !matches!(task.state, State::Working | State::Asking { .. });
        let tabs: Vec<(u64, SharedString)> = (self.pages(task, cx).into_iter())
            .map(|tab| (tab, site(&self.browser.tab(tab, cx).url).into()))
            .collect();
        let card = self.card(task, cx);
        let on = (task.state == State::Working)
            .then(|| self.on(task))
            .flatten();
        // The card's Needs you reads the same question and refusals, so the line can't say
        // working while the card says the task waits on the user.
        // A page the user took over is theirs on the line unless the agent works on another.
        let taken = (tabs.iter())
            .find(|(tab, _)| self.held.contains_key(tab))
            .filter(|(tab, _)| on.is_none_or(|on| on == *tab));
        let live = match (&task.state, &card.question) {
            (State::Asking { .. }, None) => Live::Asked,
            (_, Some(_)) => Live::Handed,
            _ if task.stops.iter().any(|stop| !stop.settled) => Live::Refused,
            _ if let Some((tab, _)) = taken => Live::Yours {
                tab: *tab,
                site: (tabs.len() > 1)
                    .then(|| tier::plain(&site(&self.browser.tab(*tab, cx).url)).into()),
            },
            _ => Live::Working,
        };
        let working = task.state == State::Working;
        // A stretch heads as at work only while the task is: one that ended at a refused or handed
        // step, a question or a page the user took heads as worked, as the card's word says, unless
        // work went on past the wait, by the steps' own order and states (set 4, find 5).
        let at_work = working && (live == Live::Working || went_on(task, &self.held));
        let mut lines = fold(
            &task.messages,
            &task.trace,
            ended,
            at_work,
            // The assistant's words show as they come while the task works, whatever the head says.
            task.writing.as_ref().filter(|_| working),
            &task.helpers,
            &task.stopped_turns,
            // ilo's lines of its refusals, its steps handed to the user and its changes of access.
            self.ilo_lines(task, cx),
        );
        // A session's conversation is in its agent's terminal: here, only its steps, one by one.
        if task.session.is_some() {
            lines = steps(lines);
        }
        // A turn the CLI hasn't answered says it's thinking from the moment it's sent, and if the
        // CLI goes or never answers, why, so nothing is left running.
        // One a run before left has no turn here to say it of.
        if task.heard_nothing() && !task.from_before() {
            let failed = match &task.state {
                State::Working => Some(None),
                State::Failed { why, .. } => Some(Some(why.trim_end_matches('.').to_string())),
                State::Limit { resets } => Some(Some(limit_words(&task.name, resets))),
                _ => None,
            };
            if let Some(failed) = failed {
                let failed = failed.map(|why| page::clip(&why, 60).into());
                lines.push(Line::Thinking {
                    key: task.trace.len(),
                    failed,
                });
            }
        }
        // ilo's line of each time it stopped the task at its helpers' limit (#4102).
        helpers::lines(task, &mut lines);
        // ilo's own lines of a task that can be replied to after it ended (#4105).
        self.kept_lines(task, &mut lines);
        // The card's own word, so a refusal waiting on the user says Needs you in both. Done says
        // when, and a limit when it resets if the assistant said; without that it's the word, as
        // the name before it says whose limit it is.
        let about = match &task.state {
            State::Done { .. } if let Some(at) = task.ended => {
                done_at(&super::time_or_day(seconds(at)))
            }
            State::Limit { resets } if resets.is_some() => limit_words(&task.name, resets),
            _ => card.word.after_name().to_string(),
        };
        // A session's title is the agent's words or its sites, so the line under it leads with its
        // client's name, and its state goes on as a fact, in lower case (#3690).
        let about = match (&task.session, task.model.is_empty()) {
            (Some(_), _) => format!("{} · {}", task.name, lower_first(&about)),
            (None, true) => format!("{} · {about}", task.name),
            (None, false) => format!("{} · {} · {about}", task.name, task.model),
        };
        Some(Conversation {
            agent: task.agent.clone().into(),
            name: task.name.clone(),
            title: task.shown_title(),
            about: about.into(),
            asked: task.asked.clone(),
            page: task.page.clone(),
            lines,
            tabs,
            on,
            stoppable: matches!(task.state, State::Working | State::Asking { .. }),
            live,
            open: open_label(&self.open_tabs(task, cx)),
            answers: match &task.state {
                State::Asking { answers, .. } if self.ask_shown(task) => answers.clone(),
                _ => Vec::new(),
            },
            writing: working && task.writing.as_ref().is_some_and(|writing| !writing.said),
            // The chip under the reply field says the task's own level, the one its refusals read.
            tier: task.tier,
            only_added: matches!(task.sites, Sites::Only(_)),
            folder: (task.ended.is_none() && task.files.folder.is_some())
                .then(|| task.names.folder.clone().map(Into::into))
                .flatten(),
        })
    }

    fn task_question(&self, task: &Task) -> Option<Question> {
        let (from, first) = match &task.session {
            Some(session) => (session.agent.as_str(), self.session_prompt(session)?),
            None => (task.agent.as_str(), self.first_prompt(&task.agent)?),
        };
        (self.question()).filter(|question| question.from == from && question.id == first)
    }

    /// The number of the first of the prompts that wait on the user for a session: a wait on a
    /// page is of the session under way that holds the page, whichever of its client's
    /// connections asked, as ilo's line of it is (`place`), and its own call's where none does.
    fn session_prompt(&self, session: &Session) -> Option<u64> {
        let ours = |tab: u64, held: &super::Held| match self.page_task(&held.agent, tab) {
            Some(task) => (task.session.as_ref()).is_some_and(|its| std::ptr::eq(its, session)),
            None => held.agent == session.agent && session.calls.contains(&held.call),
        };
        (self.held.iter())
            .filter(|&(&tab, held)| held.reply.is_some() && ours(tab, held))
            .map(|(_, held)| held.question)
            .min()
    }

    /// The tab a task is on while it works: its agent's last, or a session's last step's, as its
    /// client's other connections have their own.
    pub(super) fn on(&self, task: &Task) -> Option<u64> {
        match &task.session {
            Some(_) => task.trace.iter().rev().find_map(|step| step.did.tab),
            None => self.current.get(&(task.agent.clone(), 0)).copied(),
        }
    }

    /// A task's pages: its Space's tabs, or of its client's Space, a session's own.
    pub(super) fn pages(&self, task: &Task, cx: &App) -> Vec<u64> {
        let tabs = self.browser.space_tab_ids(task.space, cx);
        match &task.session {
            Some(session) => session
                .tabs
                .iter()
                .copied()
                .filter(|tab| tabs.contains(tab))
                .collect(),
            None => tabs,
        }
    }

    /// The number of `agent`'s prompt that came first, of its confirmations, the handoffs that
    /// wait on the user and its task's ask. Its calls on different tabs go at once (#1476), so
    /// several can ask together: the first shows, and the others wait their turn.
    pub(super) fn first_prompt(&self, agent: &str) -> Option<u64> {
        let handoffs = (self.held.values())
            .filter(|held| held.agent == agent && held.reply.is_some())
            .map(|held| held.question);
        let ask = self
            .task(agent)
            .and_then(|task| task.asking.as_ref())
            .map(|(id, _)| *id);
        handoffs.chain(ask).min()
    }

    /// Whether the task's ask is the prompt its agent shows.
    fn ask_shown(&self, task: &Task) -> bool {
        (task.asking.as_ref()).is_some_and(|(id, _)| self.first_prompt(&task.agent) == Some(*id))
    }

    /// Done's tabs to open, with their sites, those still in the task's Space. A session's are
    /// in its client's Space, where its next session may work in them: those stay the agent's
    /// until it ends too, or Open would take them from under it (#3660).
    fn open_tabs(&self, task: &Task, cx: &App) -> Vec<(u64, SharedString)> {
        let State::Done { tabs, .. } = &task.state else {
            return Vec::new();
        };
        let own = self.browser.space_tab_ids(task.space, cx);
        let working = self.worked_in();
        (tabs
            .iter()
            .filter(|tab| own.contains(tab) && !working.contains(tab)))
        .map(|&tab| (tab, site(&self.browser.tab(tab, cx).url).into()))
        .collect()
    }

    /// The cards of the tasks shown at the top of `home`'s Today, newest first.
    pub fn cards(&self, home: u64, cx: &App) -> Vec<Card> {
        (self.tasks.tasks.iter().rev())
            .filter(|task| task.home == home && !task.dismissed)
            .map(|task| self.card(task, cx))
            .collect()
    }

    /// A task's Space, made again as the core lost it.
    pub(super) fn task_space_made(&mut self, agent: &str, space: u64) {
        if let Some(task) = self.task_mut(agent) {
            task.space = space;
        }
    }

    pub fn card(&self, task: &Task, cx: &App) -> Card {
        let question = self.task_question(task);
        let asks = self.ask_shown(task);
        let (status, dot) = match (&task.state, &question) {
            (State::Working | State::Asking { .. }, Some(question)) => {
                (needs_you(question), Dot::Waiting)
            }
            // A refusal the user hasn't settled says where it stopped, as its line says why.
            (State::Working, None) if let Some(stop) = task.stops.iter().rfind(|s| !s.settled) => {
                (format!("{STOPPED_AT} {}", stop.at).into(), Dot::Waiting)
            }
            (State::Working, None) => (working(task), Dot::Working),
            (State::Asking { question, .. }, None) if asks => (question.clone(), Dot::Waiting),
            (State::Asking { .. }, None) => (NEEDS_YOU.into(), Dot::Waiting),
            // A session has no result to say, so its receipt says what it did, or with nothing to
            // count, when it ended.
            (State::Done { .. }, _) if let Some(session) = &task.session => {
                let at = task
                    .ended
                    .map(|at| super::local_time(seconds(at)))
                    .unwrap_or_default();
                (receipt(session, &at).into(), Dot::Done)
            }
            // The tick says it's done, so a finish with no line says the result's first words.
            (
                State::Done {
                    result, summary, ..
                },
                _,
            ) => (
                summary.clone().unwrap_or_else(|| first_words(result)),
                Dot::Done,
            ),
            (State::Limit { resets }, _) => (limit_words(&task.name, resets).into(), Dot::Ended),
            (State::Stopped, _) => (stopped(task), Dot::Ended),
            (State::Failed { why, .. }, _) => (why.clone(), Dot::Ended),
        };
        let open = self.open_tabs(task, cx);
        let pages = self.pages(task, cx);
        // The core's auto-archive can take a task's page it never showed (window.cc), and a task
        // lives for one run, so its card says where they went rather than offer them.
        // A session's pages are its own to close, so it keeps its receipt (#3652).
        let status = match &task.state {
            _ if task.session.is_some() => status,
            State::Done { tabs, .. } if open.is_empty() && tabs.len() == 1 => {
                "Its page went to the Archive".into()
            }
            State::Done { tabs, .. } if open.is_empty() && !tabs.is_empty() => {
                "Its pages went to the Archive".into()
            }
            _ => status,
        };
        Card {
            agent: task.agent.clone().into(),
            home: task.home,
            space: task.space,
            name: task.name.clone(),
            replies: task.session.is_none(),
            title: task.shown_title(),
            unnamed: task.title.is_empty(),
            status,
            steps: last_steps(task),
            last_tab: last_tab(task, &pages),
            line: match &task.state {
                State::Done { result, .. } => first_line(result),
                State::Stopped => last_finished(task),
                _ => None,
            },
            ended_at: None,
            asks,
            dot,
            word: Word::of(&task.state, dot == Dot::Waiting),
            visited: visited(task, &pages),
            ended_time: (task.ended)
                .map(|at| super::local_time(seconds(at)).into())
                .unwrap_or_default(),
            finding: match task.state {
                State::Done { .. } => finding(&task.messages),
                _ => SharedString::default(),
            },
            told: match task.state {
                State::Done { .. } => told(&task.messages),
                _ => SharedString::default(),
            },
            pages,
            tier: task.tier,
            question,
            open,
            page_steps: match task.state {
                State::Working => page_steps(task),
                _ => Vec::new(),
            },
            retry: matches!(task.state, State::Limit { .. }) && self.tasks_on(),
            fix: match (&task.state, task.cli) {
                (
                    State::Failed {
                        assistant: true, ..
                    },
                    Some(cli),
                ) => self.card_offer(cli).map(|offer| (cli, offer)),
                _ => None,
            },
        }
    }

    /// The CLIs ⌘T's ask lists: those signed in, as last learned, the default first.
    pub fn ask_agents(&self) -> Vec<AskAgent> {
        if !self.tasks_on() {
            return Vec::new();
        }
        let mut clis: Vec<Cli> = (Cli::ALL.into_iter())
            .filter(|cli| {
                self.tasks
                    .found
                    .get(cli)
                    .is_some_and(|found| found.ready == Ready::SignedIn)
            })
            .collect();
        if let Some(default) = self.tasks.default {
            clis.sort_by_key(|cli| *cli != default);
        }
        (clis.into_iter())
            .map(|cli| AskAgent {
                cli,
                resets: self.tasks.limits.get(&cli).cloned().flatten(),
            })
            .collect()
    }

    /// The CLIs installed but not signed in, as last learned, which the composer offers to sign in.
    pub fn signed_out_agents(&self) -> Vec<Cli> {
        if !self.tasks_on() {
            return Vec::new();
        }
        (Cli::ALL.into_iter())
            .filter(|cli| {
                (self.tasks.found.get(cli)).is_some_and(|found| found.ready == Ready::SignedOut)
            })
            .collect()
    }

    /// Whether the browser has learned which CLIs are ready, in this run or a run before.
    pub fn task_agents_known(&self) -> bool {
        !self.tasks.found.is_empty()
    }

    /// ⌘T's ask shows: the first time in a run, the browser learns again which CLIs are ready, as
    /// one may have been installed or signed in since.
    pub fn task_ask_shown(&mut self, cx: &mut Context<Self>) {
        if !std::mem::replace(&mut self.tasks.asked_once, true) {
            self.look_for_task_agents(cx);
        }
    }

    /// Settings › Models opens: it looks for each assistant, as Other Apps does, so its rows say
    /// what's on this computer. Nothing else in Settings looks, so a launch never runs one.
    pub fn models_shown(&mut self, cx: &mut Context<Self>) {
        self.look_for_task_agents(cx);
    }

    /// What the browser last learned of `cli`.
    pub fn found(&self, cli: Cli) -> Ready {
        self.tasks
            .found
            .get(&cli)
            .map_or(Ready::Unknown, |found| found.ready)
    }

    /// Learns again whether each CLI is installed and signed in, off the UI thread, unless it's
    /// learning already. Once a show: Settings › Agents and ⌘T's ask call it as they first show.
    pub fn look_for_task_agents(&mut self, cx: &mut Context<Self>) {
        if self.tasks.looking {
            self.tasks.look_again = true;
            return;
        }
        self.tasks.looking = true;
        let clients = self.clients.clone();
        let looked = cx.background_spawn(async move {
            Cli::ALL.map(|cli| (cli, clients.look_for_task_cli(cli)))
        });
        cx.spawn(async move |agents, cx| {
            let found = looked.await;
            _ = agents.update(cx, |agents, cx| {
                agents.tasks.looking = false;
                for (cli, mut found) in found {
                    // A CLI that didn't answer this time keeps the list it gave last.
                    if found.models.is_none() && found.ready == Ready::SignedIn {
                        found.models = agents.cli_models(cli).cloned();
                    }
                    agents.tasks.found.insert(cli, found);
                }
                agents.save();
                // The first ready is the default until the user picks one.
                if agents
                    .tasks
                    .default
                    .is_none_or(|cli| agents.found(cli) != Ready::SignedIn)
                {
                    let ready = Cli::ALL
                        .into_iter()
                        .find(|cli| agents.found(*cli) == Ready::SignedIn);
                    agents.tasks.default = ready.or(agents.tasks.default);
                }
                // A sign-in that ended while it looked may have changed what it found.
                if std::mem::take(&mut agents.tasks.look_again) {
                    agents.look_for_task_agents(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The CLI ⌘T's ask lists first.
    pub fn default_task_agent(&self) -> Option<Cli> {
        self.tasks.default
    }

    /// What a sort of today's tabs into folders runs (#4252), from what the browser already knows:
    /// the user's own Claude Code, while tasks are on, it runs them and it's signed in, as last
    /// learned, with its task model. Codex has no switch that keeps a run to the list, so it sorts
    /// nothing.
    pub fn sort_program(&self) -> Option<crate::broom::Program> {
        let found = self.sorter()?;
        Some(crate::broom::Program {
            program: found.program.clone()?,
            path: found.path.clone(),
            model: Some(self.task_model(Cli::Claude).to_string()).filter(|model| !model.is_empty()),
        })
    }

    /// Whether a sort has a program to run, as `sort_program` finds it, without copying it: the
    /// broom is drawn only then.
    pub fn sorts(&self) -> bool {
        self.sorter().is_some()
    }

    fn sorter(&self) -> Option<&Found> {
        if !self.tasks_on() || self.tasks.default != Some(Cli::Claude) {
            return None;
        }
        let found = self.tasks.found.get(&Cli::Claude)?;
        (found.ready == Ready::SignedIn && found.program.is_some()).then_some(found)
    }

    /// The models `cli`'s Model pop-up offers after its own default, which is the empty id, as id
    /// and name in the CLI's order: those it listed when last looked for. ilo runs the CLI without
    /// the user's settings files, so the default is the CLI's, not theirs.
    pub fn task_models(&self, cli: Cli) -> Vec<(String, String)> {
        match self.cli_models(cli) {
            Some(models) => models.listed.clone(),
            None => (cli.unlisted_models().iter())
                .map(|(id, name)| (id.to_string(), name.to_string()))
                .collect(),
        }
    }

    /// `cli`'s Model menu, as its ids and items: its own default first, saying which model it is
    /// where the CLI says, then its models, as the CLI last listed them, so the menu never waits
    /// on the CLI.
    pub fn task_model_menu(&self, cli: Cli) -> (Vec<(String, String)>, Vec<MenuEntry>) {
        model_menu(
            self.task_model(cli),
            &model_default(cli),
            self.task_default_model(cli),
            self.task_models(cli),
        )
    }

    /// Which model `cli`'s own default is, by name, where it said when last looked for.
    pub fn task_default_model(&self, cli: Cli) -> Option<&str> {
        self.cli_models(cli)?.default.as_deref()
    }

    fn cli_models(&self, cli: Cli) -> Option<&super::ready::Models> {
        self.tasks.found.get(&cli)?.models.as_ref()
    }

    /// The model `agent`'s run passes its CLI, None for the CLI's own default: the one the user
    /// picked in the composer for it, or else Settings' Model as it is now.
    pub(super) fn launch_model(&self, agent: &str, cli: Cli) -> Option<String> {
        let picked = self.task(agent).and_then(|task| task.model_id.clone());
        let model = picked.unwrap_or_else(|| self.task_model(cli).to_string());
        Some(model).filter(|model| !model.is_empty())
    }

    /// Run tasks with, in Settings › Agents.
    pub fn set_default_task_agent(&mut self, cli: Cli, cx: &mut Context<Self>) {
        self.tasks.default = Some(cli);
        self.save();
        cx.notify();
    }

    /// The model `cli` runs tasks with, empty for its own default, which a saved model the CLI no
    /// longer lists falls back to. It stays saved, so it's back if the CLI lists it again.
    pub fn task_model(&self, cli: Cli) -> &str {
        let chosen = self.tasks.models.get(&cli).map_or("", String::as_str);
        match self.task_models(cli).iter().any(|(id, _)| id == chosen) {
            true => chosen,
            false => "",
        }
    }

    /// The model `cli` runs tasks with, by name, such as Haiku, or None for its own default.
    pub fn task_model_name(&self, cli: Cli) -> Option<SharedString> {
        let chosen = self.task_model(cli);
        let (_, name) = (self.task_models(cli).into_iter()).find(|(id, _)| id == chosen)?;
        Some(name.into())
    }

    /// Sets the model `cli` runs tasks with, only one its Model menu lists, so nothing else, typed
    /// or from a page, an agent or a file, is ever passed to the CLI as one.
    pub fn set_task_model(&mut self, cli: Cli, model: &str, cx: &mut Context<Self>) {
        let listed = self.task_models(cli).iter().any(|(id, _)| id == model);
        if !model.is_empty() && !listed {
            return;
        }
        match model.is_empty() {
            true => self.tasks.models.remove(&cli),
            false => self.tasks.models.insert(cli, model.to_string()),
        };
        self.save();
        cx.notify();
    }

    /// Starts a task: `cli` does what the user `asked`, on `page` if it goes with it, from the
    /// user's Space `home`, at `tier`: the user's own pick on the composer page's menu, else
    /// Work on pages. Returns its agent's id, or None if it couldn't start, which its card says.
    pub fn start_task(
        &mut self,
        cli: Cli,
        asked: &str,
        page: Option<AskedOn>,
        home: u64,
        start: impl Into<Start>,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let Start {
            tier,
            model: picked,
            pages,
            only_added,
            files: given,
            tabs,
        } = start.into();
        // Only an http(s) page is added, and only its address: the composer lists no other, and
        // nothing else is opened.
        let pages: Vec<String> = (pages.iter())
            .filter_map(|url| added_address(url))
            .collect();
        let asked = asked.trim();
        // A tab picked and no words starts a thread too: what its assistant is told of the tab
        // is then the whole message. With neither there is nothing to start.
        if !self.tasks_on() || (asked.is_empty() && !self.any_to_hand(home, &tabs, cx)) {
            return None;
        }
        // A private window starts no task, whichever way it was asked: agents get no path into
        // one, and nothing of what's asked there is kept.
        if self.browser.private_space(home) {
            log!("browser: no task from a private window");
            return None;
        }
        self.tasks.next += 1;
        let agent = format!("task-{}", self.tasks.next);
        // check.sh reads this line.
        log!(
            "browser: task {agent} on {} asked from space {home}",
            cli.id()
        );
        let space = self.browser.make_task_space(&agent, cli.name(), home, cx);
        let title: SharedString = page::clip(asked, 80).into();
        // A model the CLI no longer offers falls back to its own default, saying nothing, as
        // Settings' does.
        let model_id = picked.map(|id| {
            let listed = self
                .task_models(cli)
                .into_iter()
                .find(|(each, _)| *each == id);
            listed.map_or(String::new(), |(id, _)| id)
        });
        let model = match &model_id {
            None => self.task_model_name(cli).unwrap_or_default(),
            Some(id) => (self.task_models(cli).into_iter())
                .find(|(each, _)| each == id)
                .map(|(_, name)| name.into())
                .unwrap_or_default(),
        };
        let task = Task {
            agent: agent.clone(),
            cli: Some(cli),
            name: cli.name().into(),
            session: None,
            home,
            space,
            asked: asked.to_string().into(),
            page,
            title,
            model,
            model_id,
            added: pages.clone(),
            state: State::Working,
            ended: None,
            end_entry: 0,
            started: SystemTime::now(),
            kept: None,
            orders: 0,
            sent: 0,
            messages: Vec::new(),
            trace: Vec::new(),
            dismissed: false,
            left: None,
            deleted: false,
            token: super::random_token(),
            process: None,
            asking: None,
            waiting: None,
            writing: None,
            named: None,
            tier,
            // A tab of theirs picked for it is a page the user adds, as one typed is: its site
            // joins as the tab is handed.
            sites: match only_added && !(pages.is_empty() && tabs.is_empty()) {
                true => Sites::Only(
                    pages
                        .iter()
                        .filter_map(|url| self.browser.url_site(url))
                        .collect(),
                ),
                false => Sites::default(),
            },
            stops: Vec::new(),
            stopped_turns: Vec::new(),
            picked_from: None,
            placed: Vec::new(),
            told: None,
            helpers: Helpers::of_task(self.helpers_on() && cli == Cli::Claude),
            files: self.files_at_start(),
            names: self.names_at_start(),
            rising: None,
            rewalled: Vec::new(),
            copying: false,
            sends: Default::default(),
        };
        self.tasks.tasks.push(task);
        self.tokens_changed();
        // The user's tabs picked for it are its pages from here, and its assistant is told so.
        let titles = self.titles_of(&tabs, cx);
        let (chips, told) = self.hand_with_message(&agent, tabs, titles, cx);
        // No words, and no tab that went after all: nothing was asked, so no thread started.
        if asked.is_empty() && told.is_none() {
            self.end_task(&agent, true, cx);
            return None;
        }
        if let Some(task) = self.task_mut(&agent) {
            task.sends.showed(0, chips);
        }
        let task = self.task(&agent);
        let first = first_message(asked, task.and_then(|task| task.page.as_ref()));
        let first = with_added(first, task.map_or(&[], |task| &task.added));
        let first = handed::with_told(first, told);
        self.start_with(&agent, first, given, cx);
        cx.notify();
        Some(agent)
    }

    /// Starts `agent`'s task with what the user picked on the `+`: the folder, if ilo may give
    /// it, and ilo's copies of the files attached, made off the UI thread before its assistant
    /// starts, as a file may be large. A system that can't wall a task in gives it neither.
    fn start_with(
        &mut self,
        agent: &str,
        first: String,
        mut picked: Picked,
        cx: &mut Context<Self>,
    ) {
        if !self.files_available() {
            picked = Picked::default();
        }
        // The folder is checked again by its real path, as it may have moved since the panel
        // gave it: one that no longer resolves to itself, or that ilo never gives, isn't given,
        // and its line says so by the name the user saw (X6).
        let never = Never::here(Some(self.tasks_dir()));
        let refuses = |folder: &super::wall::Granted| {
            let root = folder.root();
            files::resolved(root).as_deref() != Some(root) || never_grantable(root, &never)
        };
        let refused = match &picked.folder {
            Some(folder) if refuses(folder) => picked.folder.take(),
            _ => None,
        };
        let names = Names::of(&Picked {
            folder: refused.clone().or(picked.folder.clone()),
            files: picked.files.clone(),
        });
        if let Some(task) = self.task_mut(agent)
            && (picked.folder.is_some() || refused.is_some() || !picked.files.is_empty())
        {
            task.files.folder = picked.folder.clone();
            task.names = names;
            if let Some(folder) = &refused {
                let files = Given {
                    folder: Some(folder.clone()),
                    attached: None,
                };
                task.rewalled(rewall::Rewalled::WithoutFiles { files });
            }
        }
        let with = StartedWith {
            pages: self.task(agent).map_or(0, |task| task.added.len()),
            folder: picked.folder.is_some(),
            files: picked.files.len(),
            left_out: refused.is_some(),
        };
        if picked.files.is_empty() {
            let wall = self.wall_to_begin(agent, with);
            self.launch_in(agent, &first, wall, cx);
            return self.wait_for_answer(agent, cx);
        }
        if let Some(task) = self.task_mut(agent) {
            task.copying = true;
        }
        let into = self.attached_dir(agent);
        let files = picked.files;
        let copy = (cx.background_executor())
            .spawn(async move { files::copy_attached(&files, &into).map(|()| into) });
        let agent = agent.to_string();
        cx.spawn(async move |agents, cx| {
            let copied = copy.await.ok();
            _ = agents.update(cx, |agents, cx| {
                agents.attached(&agent, &first, with, copied, cx)
            });
        })
        .detach();
    }

    /// Where ilo's copies of `agent`'s attached files are, beside its own folder and not in it,
    /// as the assistant writes in its own and only reads these.
    pub(super) fn attached_dir(&self, agent: &str) -> PathBuf {
        self.tasks_dir().join(format!("{agent} files"))
    }

    /// ilo's copies of `agent`'s attached files are made, or couldn't be: its assistant starts,
    /// with them or without. A task stopped meanwhile starts nothing, and its copies go.
    fn attached(
        &mut self,
        agent: &str,
        first: &str,
        with: StartedWith,
        copied: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self.task_mut(agent) else {
            _ = std::fs::remove_dir_all(self.attached_dir(agent));
            return;
        };
        task.copying = false;
        // Its folder is what it has now, which a Revoke meanwhile took.
        let with = StartedWith {
            folder: task.files.folder.is_some(),
            ..with
        };
        if task.state != State::Working {
            self.log_start(agent, with);
            _ = std::fs::remove_dir_all(self.attached_dir(agent));
            return;
        }
        // What ilo or the user had to say while the copies were made goes with its one start.
        let first = match task.told.take() {
            Some(told) => format!("{first}\n\n{told}"),
            None => first.to_string(),
        };
        let first = first.as_str();
        let copied_none = copied.is_none();
        match copied {
            Some(into) => {
                if let Some(task) = self.task_mut(agent) {
                    task.files.attached = Some(into);
                }
            }
            // Not silently: its line names what the task started without.
            None => {
                let files = Given {
                    folder: None,
                    attached: Some(self.attached_dir(agent)),
                };
                self.place_in(agent, super::lines::Happened::NotCopied { files });
            }
        }
        // The start's log row counts what it starts with, and that its files were left out
        // where their copies couldn't be made or its wall wasn't put up (rewall.rs).
        let left_out = with.left_out || copied_none;
        let wall = self.wall_to_begin(agent, StartedWith { left_out, ..with });
        self.launch_in(agent, first, wall, cx);
        self.wait_for_answer(agent, cx);
    }

    /// Whether this system can wall a task in with its files, which the `+` offers them by.
    pub fn files_available(&self) -> bool {
        #[cfg(test)]
        return self.tasks.files_available;
        #[cfg(not(test))]
        super::wall::available().is_ok()
    }

    /// While the task thinks, its CLI has NO_ANSWER to take its first step or say something,
    /// or the task ends, saying so: a CLI that hangs never leaves a Thinking going.
    pub(super) fn wait_for_answer(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent).filter(|task| task.thinking()) else {
            return;
        };
        let agent = agent.to_string();
        task.waiting = Some(cx.spawn(async move |agents, cx| {
            cx.background_executor().timer(NO_ANSWER).await;
            _ = agents.update(cx, |agents, cx| agents.no_answer(&agent, cx));
        }));
    }

    fn no_answer(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        // It's this timer that runs: it goes once it's done.
        if let Some(timer) = task.waiting.take() {
            timer.detach();
        }
        if !task.thinking() {
            return;
        }
        // check.sh reads this line.
        log!(
            "browser: task {agent} had no answer in {} s",
            NO_ANSWER.as_secs()
        );
        // Its CLI ends, so what it might say late isn't taken for the task's; a reply starts it again.
        task.process = None;
        let why = format!("{} didn't answer.", task.name);
        self.fail(agent, Failure::Other(why), cx);
    }

    /// Whether a task that starts now gets its assistant's helper tool: the switch. A test's
    /// tasks have what the test says, on or off, whatever the switch is.
    fn helpers_on(&self) -> bool {
        #[cfg(test)]
        let said = Some(self.tasks.helpers);
        #[cfg(not(test))]
        let said = None;
        said.unwrap_or(helpers::ON)
    }

    /// The files a task that starts now has: none, until a grant gives a task its own.
    fn files_at_start(&self) -> Given {
        #[cfg(test)]
        return self.tasks.files.clone();
        #[cfg(not(test))]
        Given::default()
    }

    /// The names of `files_at_start`, as the system's panel would show them.
    fn names_at_start(&self) -> Names {
        #[cfg(test)]
        return Names {
            folder: (self.tasks.files.folder.as_ref())
                .map(|folder| super::files::shown_name(folder.root())),
            files: Vec::new(),
        };
        #[cfg(not(test))]
        Names::default()
    }

    /// Starts the task's CLI with its first message.
    pub(super) fn launch(&mut self, agent: &str, first: &str, cx: &mut Context<Self>) {
        // A task with files starts inside its wall, or without them (rewall.rs).
        let wall = self.wall_to_start(agent);
        self.launch_in(agent, first, wall, cx);
    }

    /// Starts `agent`'s assistant inside `wall`, or with None as a task with no files starts.
    pub(super) fn launch_in(
        &mut self,
        agent: &str,
        first: &str,
        wall: Option<Arc<super::wall::Wall>>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((task, Some(cli))) = self.task(agent).map(|task| (task, task.cli)) else {
            return false;
        };
        let found = self.tasks.found.get(&cli).cloned().unwrap_or_default();
        let Some(program) = found.program.clone() else {
            self.fail(
                agent,
                Failure::Other(format!("{} isn't installed.", cli.name())),
                cx,
            );
            return false;
        };
        let dir = self.tasks_dir();
        let walled = wall.is_some();
        let mut args = self.bridge.clone();
        args.push(self.socket.to_string_lossy().into_owned());
        let command = args.remove(0);
        let model = self.launch_model(agent, cli);
        // A kept run's folder is its conversation's own, which outlives the run (kept.rs).
        let (keep, kept_in) = self.keep_for(agent, cli);
        let launch = Launch {
            cli,
            program,
            path: found.path.clone(),
            dir: kept_in.unwrap_or_else(|| dir.join(agent)),
            config: dir.join(format!("{agent}.json")),
            server: runner::Server {
                command,
                args,
                token: task.token.clone(),
            },
            model,
            instructions: match task.helpers.on {
                true => format!(
                    "{}\n\n{}",
                    tools::TASK_INSTRUCTIONS,
                    helpers::instructions()
                ),
                false => tools::TASK_INSTRUCTIONS.into(),
            },
            tools: tools::names(agent),
            helpers: task.helpers.on,
            wall,
        };
        let events = self.task_events(cx);
        match Process::start_kept(&launch, &keep, agent, first, events) {
            Ok(process) => {
                if let Some(task) = self.task_mut(agent) {
                    task.process = Some(process);
                    // What starts it again if its wall doesn't go up (rewall.rs).
                    task.rising = walled.then(|| rewall::Rising::of(first));
                    // Until its first line says which kinds of helper it has (#4102).
                    let new = matches!(keep, super::kept::Keep::New(_));
                    let had = (task.trace.len(), task.messages.len());
                    task.helpers.run_started(first, new, had);
                }
                self.kept_started(agent, cli, &keep);
                true
            }
            Err(error) => {
                // The folder made for a conversation that never began.
                if matches!(keep, super::kept::Keep::New(_)) {
                    _ = std::fs::remove_dir(&launch.dir);
                }
                log!("browser: task {agent} couldn't start {}: {error}", cli.id());
                self.fail(
                    agent,
                    Failure::Other(format!("{} couldn't start.", cli.name())),
                    cx,
                );
                false
            }
        }
    }

    /// The folder the tasks' own folders are in.
    pub(super) fn tasks_dir(&self) -> PathBuf {
        let dir = self.tasks.dir.clone();
        dir.unwrap_or_else(std::env::temp_dir).join("Tasks")
    }

    /// Where the tasks' processes hand their output, which wakes the UI only as they say something.
    fn task_events(
        &mut self,
        cx: &mut Context<Self>,
    ) -> futures::channel::mpsc::UnboundedSender<runner::Event> {
        if let Some((events, _)) = &self.tasks.events {
            return events.clone();
        }
        let (events, mut said) = futures::channel::mpsc::unbounded();
        let reading = cx.spawn(async move |agents, cx| {
            while let Some(event) = said.next().await {
                if agents
                    .update(cx, |agents, cx| agents.task_said(event, cx))
                    .is_err()
                {
                    return;
                }
            }
        });
        self.tasks.events = Some((events.clone(), reading));
        events
    }

    /// What a task's CLI said.
    pub fn task_said(&mut self, event: runner::Event, cx: &mut Context<Self>) {
        let runner::Event { agent, run, out } = event;
        // A line of a run ilo ended as it started the task again isn't the task's (helpers.rs).
        let under_way = (self.task(&agent))
            .and_then(|task| task.process.as_ref())
            .map(runner::Process::run);
        if under_way.is_some_and(|under_way| under_way != run) {
            return;
        }
        // Its assistant is inside its wall, which is no answer of the assistant's.
        if out == Out::Walled(Ok(())) {
            return self.walled_in(&agent);
        }
        // Whatever it said, it answered.
        if let Some(task) = self.task_mut(&agent) {
            task.waiting = None;
        }
        match out {
            Out::Said(text) => {
                let Some(task) = self.task_mut(&agent) else {
                    return;
                };
                // Finish's result is the answer, and the task has ended: what the CLI writes after
                // it, as its turn winds down, isn't shown.
                if matches!(task.state, State::Done { .. }) {
                    return;
                }
                // What it was writing is this message, which stays where it showed.
                if let Some(writing) = &mut task.writing {
                    *writing = Writing {
                        text: text.clone().into(),
                        said: true,
                    };
                }
                task.messages.push(Message::new(Who::Agent, text));
            }
            Out::Writing(text) => {
                let Some(task) = self.task_mut(&agent) else {
                    return;
                };
                if task.state != State::Working {
                    return;
                }
                task.writing = (!text.is_empty()).then(|| Writing {
                    text: text.into(),
                    said: false,
                });
            }
            // A call takes what it wrote into the trace, as what it said along the way.
            Out::Calling => {
                let Some(task) = self.task_mut(&agent) else {
                    return;
                };
                task.writing = None;
            }
            Out::Thought { text, took } => {
                let Some(task) = self.task_mut(&agent) else {
                    return;
                };
                let now = Instant::now();
                task.trace.push(Traced {
                    call: 0,
                    did: tools::Did {
                        tool: "thought".into(),
                        text: page::clip(&text, 4000),
                        ..Default::default()
                    },
                    after: task.messages.len(),
                    started: now.checked_sub(took).unwrap_or(now),
                    ended: Some(now),
                    error: None,
                    kind: None,
                    picture: None,
                    close: None,
                    notes: None,
                });
            }
            Out::Helper { id, told } => self.helper_told(&agent, id, told, cx),
            Out::Started { id, deep } => self.helper_started(&agent, id, deep, cx),
            Out::NotAllowed(limit) => self.helper_not_allowed(&agent, limit, cx),
            Out::Kinds { ours } => self.helper_kinds(&agent, ours, cx),
            // The turn ilo interrupted at the helpers' limit ends, however its CLI says so. The
            // first since the user wrote, the task goes on in a new turn that starts with what
            // ilo tells it; the second, the turn ends as any does and the task waits for them.
            Out::Ended(_)
                if let Some(after) = self
                    .task_mut(&agent)
                    .filter(|task| task.state == State::Working)
                    .and_then(|task| task.helpers.takes_end()) =>
            {
                match after {
                    helpers::After::Told(told) => self.tell_task(&agent, told, true, cx),
                    helpers::After::Ends => {
                        let out = Out::Ended(None);
                        self.task_said(runner::Event { agent, run, out }, cx)
                    }
                }
            }
            Out::Ended(None) => {
                let working = (self.task(&agent)).is_some_and(|task| task.state == State::Working);
                // The replies held for this turn's end are given now: the thread is under way on
                // them, and keeps what it holds.
                if working && self.give_held(&agent, cx) {
                    return cx.notify();
                }
                // No longer under way from here: the tabs it holds of the user's go back first.
                if working {
                    self.hand_all_back(&agent, cx);
                }
                let Some(task) = self.task_mut(&agent) else {
                    return;
                };
                task.writing = None;
                // A turn that ends without finish ends the task with what it said last.
                if task.state == State::Working {
                    let result = (task.messages.iter().rev())
                        .find(|message| message.who != Who::User)
                        .map_or_else(|| "Done".into(), |message| message.text.clone());
                    task.state = State::Done {
                        result,
                        summary: None,
                        tabs: Vec::new(),
                    };
                    task.end(SystemTime::now());
                }
                // However it ended, and by the name it has now (#4105).
                self.keep_ended(&agent);
            }
            // The turn the user stopped ends as an error; their stop is what the card says.
            Out::Ended(Some(_))
                if self
                    .task(&agent)
                    .is_some_and(|task| task.state == State::Stopped) => {}
            Out::Ended(Some(failure)) => self.fail(&agent, failure, cx),
            Out::Walled(Ok(())) => {}
            // The start the user stopped: their stop ended its launcher, and is what the card says.
            Out::Walled(Err(_))
                if self
                    .task(&agent)
                    .is_some_and(|task| task.state == State::Stopped) => {}
            // Nothing started, so nothing of the task's is behind a wall that isn't up. Its
            // launcher goes with the process, and the task goes on as the table says of a wall
            // refused (rewall.rs): its run before went with it, so the Gone that follows is
            // another run's and changes nothing.
            Out::Walled(Err(refused)) => {
                log!("browser: task {agent} wasn't walled in: {refused:?}");
                if let Some(task) = self.task_mut(&agent) {
                    task.end_run();
                }
                self.wall_refused(&agent, cx);
            }
            Out::Gone(said) => {
                // Its assistant no longer has the conversation it was to go on with, or what it
                // said goes to the log without the conversation's id.
                let (gone, said) = self.kept_gone(&agent, &said);
                let Some(task) = self.task_mut(&agent) else {
                    return;
                };
                task.process = None;
                if gone {
                    log!("browser: task {agent} has no conversation kept");
                    // Its row leaves the file, as it takes no reply any more (#4105).
                    self.write_kept();
                    self.forget_waiting(&agent, None, cx);
                } else if matches!(task.state, State::Working | State::Asking { .. }) {
                    let said = said.trim();
                    log!("browser: task {agent} ended: {said}");
                    let why = format!("{} stopped working.", task.name);
                    self.fail(&agent, Failure::Other(why), cx);
                }
            }
        }
        cx.notify();
    }

    /// The task can't go on: its card says why, and what fixes it.
    pub(super) fn fail(&mut self, agent: &str, failure: Failure, cx: &mut Context<Self>) {
        let Some(cli) = self.task(agent).and_then(|task| task.cli) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: task {agent} failed: {failure:?}");
        let state = match failure {
            Failure::Limit(resets) => State::Limit {
                resets: resets.map(clock),
            },
            Failure::SignedOut => {
                let why = format!("{} isn't signed in.", cli.name()).into();
                State::Failed {
                    why,
                    assistant: true,
                }
            }
            Failure::Other(why) if why.contains("isn't installed") => State::Failed {
                why: why.into(),
                assistant: true,
            },
            Failure::Other(why) => State::Failed {
                why: page::clip(&why, 200).into(),
                assistant: false,
            },
        };
        if let State::Limit { resets } = &state {
            self.tasks.limits.insert(cli, resets.clone());
        }
        self.hand_all_back(agent, cx);
        if let Some(task) = self.task_mut(agent) {
            task.state = state;
            task.end(SystemTime::now());
            task.waiting = None;
            task.writing = None;
        }
        self.keep_ended(agent);
        self.forget_waiting(agent, None, cx);
        // What's ready may have changed.
        self.look_for_task_agents(cx);
        cx.notify();
    }

    /// The user's reply, which the agent reads once its turn under way ends.
    pub fn reply(&mut self, agent: &str, text: &str, cx: &mut Context<Self>) {
        self.reply_with(agent, text.into(), cx);
    }

    /// The user's reply with the tabs of theirs picked for it, which are handed to the thread as
    /// its assistant is given the message, and told of in it after their words. A tab alone is a
    /// reply; no words and no tab that goes is none.
    fn reply_with(&mut self, agent: &str, reply: handed::Reply, cx: &mut Context<Self>) {
        let handed::Reply { text, tabs } = reply;
        let text = text.trim();
        if text.is_empty() && tabs.is_empty() {
            return;
        }
        // A chip shows its tab's title as it is at the user's send.
        let titles = self.titles_of(&tabs, cx);
        // A reply answers the ask waiting, as a button would.
        if self.task(agent).is_some_and(|task| task.asking.is_some()) {
            return self.answer_ask(agent, text, tabs, titles, cx);
        }
        // One a run before left comes back in its own profile, or not at all (#4105).
        if !self.goes_on(agent) {
            return cx.notify();
        }
        // A thread at work is handed nothing mid-turn: a reply with a tab of the user's waits for
        // the turn under way to end, out of the conversation until then, and one sent while it
        // waits goes after it. One that waits on the user is handed it at their send.
        let waits_on_user = self.waits_on_a_hold(agent);
        if let Some(task) = self.task_mut(agent)
            && task.state == State::Working
            && task.sends.holds(&tabs, waits_on_user)
        {
            let text = text.to_string().into();
            task.sends.hold(handed::Reply { text, tabs }, titles);
            return cx.notify();
        }
        self.give_reply(agent, text, tabs, titles, cx);
    }

    /// `agent`'s turn under way ended: the replies held for it are given, in the order sent, each
    /// handing its tabs as it goes, checked again now. Whether any was, on which the thread is
    /// still under way.
    fn give_held(&mut self, agent: &str, cx: &mut Context<Self>) -> bool {
        let held = (self.task_mut(agent)).map_or_else(Vec::new, |task| task.sends.take_held());
        let mut given = false;
        for (reply, titles) in held {
            given |= self.give_reply(agent, &reply.text, reply.tabs, titles, cx);
        }
        given
    }

    /// Gives `agent`'s assistant the user's reply, `text` with `tabs`. Whether there was anything
    /// to give: with no words and no tab that went, nothing is, and the thread is as it was.
    fn give_reply(
        &mut self,
        agent: &str,
        text: &str,
        tabs: handed::Picked,
        titles: Vec<SharedString>,
        cx: &mut Context<Self>,
    ) -> bool {
        let (chips, told) = self.hand_with_message(agent, tabs, titles, cx);
        if text.is_empty() && told.is_none() {
            return false;
        }
        // A reply to a task that has ended takes it up, if its conversation is kept (#4105).
        self.pick_up(agent, cx);
        let resumes = self.resumes(agent);
        let Some(task) = self.task_mut(agent) else {
            return false;
        };
        task.messages
            .push(Message::new(Who::User, text.to_string()));
        let which = pick_up::replies(task);
        task.sends.showed(which, chips);
        task.sends.replied();
        task.state = State::Working;
        task.writing = None;
        task.helpers.turned(task.trace.len());
        let reply = handed::with_told(text.to_string(), told);
        // Its assistant hasn't started: the reply goes with its one start, once the copies end.
        if task.copying {
            task.hold(reply);
            cx.notify();
            return true;
        }
        // What ilo had to tell it since its last turn comes first, as ilo's, not the user's.
        let text = match task.told.take() {
            Some(told) => format!("{told}\n\n{reply}"),
            None => reply.clone(),
        };
        let text = text.as_str();
        let sent = task.process.as_mut().map(|process| process.say(text));
        if !matches!(sent, Some(Ok(()))) {
            // Gone, or never started: a new run, which goes on with the conversation its assistant
            // kept, or is given the conversation so far, this reply last and as it's given here.
            let first = match resumes {
                true => text.to_string(),
                false => conversation_to(task, Some(&reply)),
            };
            self.launch(agent, &first, cx);
        }
        self.wait_for_answer(agent, cx);
        cx.notify();
        true
    }

    /// The user's reply to the task's ask, which answers it. The tabs of theirs picked for it are
    /// handed as it does, and the answer says so after their words.
    fn answer_ask(
        &mut self,
        agent: &str,
        answer: &str,
        tabs: handed::Picked,
        titles: Vec<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let (chips, told) = self.hand_with_message(agent, tabs, titles, cx);
        // No words and no tab that went: nothing answered it, and it still waits.
        if answer.is_empty() && told.is_none() {
            return;
        }
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        let Some((_, reply)) = task.asking.take() else {
            return;
        };
        task.messages
            .push(Message::new(Who::User, answer.to_string()));
        let which = pick_up::replies(task);
        task.sends.showed(which, chips);
        task.state = State::Working;
        task.writing = None;
        let answered = match answer.is_empty() {
            true => String::new(),
            false => format!("The user answered: {answer}"),
        };
        _ = reply.send(ToolResult::text(handed::with_told(answered, told)));
        self.note(agent, 0, "answered", "", "");
        self.wait_for_answer(agent, cx);
        cx.notify();
    }

    /// ⌘. or Stop: ends what the task is doing at once. A reply goes on from there.
    pub fn stop_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        if !matches!(task.state, State::Working | State::Asking { .. }) {
            return;
        }
        task.asking = None;
        task.writing = None;
        task.waiting = None;
        // A session's agent is its client's, whose calls under way the user stops, on each of its
        // connections, as the agent path always has. Its Stop is logged before each of its
        // sessions ends, this one too, so each one's record has it (session_stopped).
        if let Some(session) = &task.session {
            let client = session.agent.clone();
            self.stop_agent(&client, cx);
            cx.notify();
            return;
        }
        // Stopped, it holds no tab of the user's: each goes back before its record closes.
        self.hand_all_back(agent, cx);
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        let turn = (task.messages.iter())
            .filter(|message| message.who == Who::User)
            .count();
        task.stopped_turns.push(turn);
        task.state = State::Stopped;
        task.end(SystemTime::now());
        if let Some(process) = task.process.as_mut() {
            process.interrupt();
        }
        self.you_stopped(agent, cx);
        self.keep_ended(agent);
        self.stop_agent(agent, cx);
        cx.notify();
    }

    /// Limit's Retry: goes on from where it stopped. The browser never retries by itself.
    pub fn retry_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        if !matches!(task.state, State::Limit { .. }) {
            return;
        }
        // One a run before left comes back in its own profile, or not at all (#4105).
        if !self.goes_on(agent) {
            return cx.notify();
        }
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        task.state = State::Working;
        task.writing = None;
        let sent = task.process.as_mut().map(|process| process.say(RETRY));
        if let Some(cli) = task.cli {
            self.tasks.limits.remove(&cli);
        }
        // At work again: the file's row for a kept task says so (pick_up.rs).
        self.kept_under_way(agent);
        if !matches!(sent, Some(Ok(()))) {
            let first = match self.resumes(agent) {
                true => RETRY.to_string(),
                false => conversation(self.task(agent).expect("found")),
            };
            self.launch(agent, &first, cx);
        }
        self.wait_for_answer(agent, cx);
        cx.notify();
    }

    /// ✕ or ⌘W: the task goes to the Archive, which ⇧⌘T and ⌘Z bring it back from. What it was
    /// doing stops. An older dismissed task than the last few goes for good.
    pub fn dismiss_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        self.stop_task(agent, cx);
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        task.dismissed = true;
        task.deleted = false;
        task.left = Some(SystemTime::now());
        // check.sh reads this line.
        log!("browser: task {agent} dismissed");
        self.tasks.dismissed.retain(|each| each != agent);
        self.tasks.dismissed.push_back(agent.to_string());
        // Those deleted from the Archive go first, then the oldest.
        while self.tasks.dismissed.len() > KEPT {
            let deleted = (self.tasks.dismissed.iter())
                .position(|each| self.task(each).is_some_and(|task| task.deleted));
            if let Some(old) = self.tasks.dismissed.remove(deleted.unwrap_or(0)) {
                self.drop_task(&old, cx);
            }
        }
        cx.notify();
    }

    /// Delete from Archive: the dismissed task leaves the Archive and ⇧⌘T's reach, or with
    /// `deleted` false, ⌘Z brings it back there.
    fn delete_task(&mut self, agent: &str, deleted: bool, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent).filter(|task| task.dismissed) else {
            return;
        };
        task.deleted = deleted;
        // check.sh reads this line.
        log!(
            "browser: task {agent} {}",
            if deleted { "deleted" } else { "undeleted" }
        );
        // Its row leaves the file of the kept with it, and comes back with it (#4105).
        self.keep_ended(agent);
        cx.notify();
    }

    /// Brings a dismissed task back to its card: the one named, or the last dismissed.
    pub fn restore_task(&mut self, agent: Option<&str>, cx: &mut Context<Self>) -> Option<String> {
        let deleted = |each: &String| self.task(each).is_none_or(|task| task.deleted);
        let agent = match agent {
            Some(agent) => agent.to_string(),
            None => self
                .tasks
                .dismissed
                .iter()
                .rev()
                .find(|each| !deleted(each))?
                .clone(),
        };
        if deleted(&agent) {
            return None;
        }
        self.tasks.dismissed.retain(|each| *each != agent);
        // One a run before left takes its place in its own profile (#4105).
        self.place_past(&agent);
        let task = self.task_mut(&agent)?;
        task.dismissed = false;
        log!("browser: task {agent} restored");
        cx.notify();
        Some(agent)
    }

    /// Done's Open: the tabs it left the user, or `only` that one of them, move to the top of the
    /// Today tabs of the Space it was asked in, in the agent's order, the user's now. Returns the
    /// first, to show. A tab moved is out of the task's for good: bringing its card back with ⌘Z
    /// offers the rest.
    pub fn open_result(
        &mut self,
        agent: &str,
        only: Option<u64>,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let task = self.task(agent)?;
        let open: Vec<u64> = (self.open_tabs(task, cx).into_iter())
            .map(|(tab, _)| tab)
            .filter(|tab| only.is_none_or(|only| only == *tab))
            .collect();
        // A session's go to the Space the user is in, as it was asked from none.
        let home = match task.session {
            Some(_) => self.browser.user_space(task.space),
            None => task.home,
        };
        let first = *open.first()?;
        let task = self.task_mut(agent)?;
        if let State::Done { tabs, .. } = &mut task.state {
            tabs.retain(|tab| !open.contains(tab));
        }
        // Each goes to the top, so the last goes first.
        for &tab in open.iter().rev() {
            self.left(tab, super::Gone::Opened);
            self.browser.move_tab(tab, home, cx);
            // Kept, it's the user's tab, with its sound (#1080).
            self.browser.set_muted(tab, false);
            // check.sh reads this line.
            log!("browser: task {agent} opens {tab} in space {home}");
        }
        cx.notify();
        Some(first)
    }

    /// The task goes for good: its CLI ends and its Space goes, with its tabs.
    pub(super) fn drop_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        self.end_task(agent, true, cx);
        // What its assistant kept of it goes with it (#4105), once its run has ended, so nothing
        // writes it back.
        self.forget_kept(agent, cx);
    }

    /// The task goes: its CLI ends, and its Space too with `space`. A session's tabs close instead,
    /// as its Space is its client's.
    fn end_task(&mut self, agent: &str, space: bool, cx: &mut Context<Self>) {
        let Some(at) = self.tasks.tasks.iter().position(|task| task.agent == agent) else {
            return;
        };
        // A tab it holds of the user's never goes with it or with its Space: each goes back
        // first, at a quit too, whether or not the Space can be deleted then.
        self.hand_all_back(agent, cx);
        let task = self.tasks.tasks.remove(at);
        // check.sh reads this line.
        log!("browser: task {agent} dropped");
        // A session's Space is its client's, which keeps the tabs of its other sessions: its own
        // close, but for any a session under way works in.
        if let Some(session) = task.session {
            let pages = self.pages_of(&session, task.space, cx);
            let live = self.worked_in();
            for tab in pages.into_iter().filter(|tab| !live.contains(tab)) {
                // The task is gone from the list, so `left` wouldn't find whose it was.
                if !self.gone.contains_key(&tab) {
                    self.went(tab, &session.agent, super::Gone::PutAway);
                }
                self.browser.close_user_tab(tab, cx);
            }
            return;
        }
        self.forget_waiting(agent, None, cx);
        self.doing.remove(agent);
        self.tokens_changed();
        if space && task.space != 0 {
            self.browser.delete_space(task.space, cx);
        }
        self.remove_task_files(agent);
    }

    /// `tab`, a session's page, leaves its agent's Space for `gone`, unless it already said why.
    pub(super) fn left(&mut self, tab: u64, gone: super::Gone) {
        let whose = (self.tasks.tasks.iter())
            .filter_map(|task| task.session.as_ref())
            .find(|session| session.tabs.contains(&tab))
            .map(|session| session.agent.clone());
        if let Some(agent) = whose.filter(|_| !self.gone.contains_key(&tab)) {
            self.went(tab, &agent, gone);
        }
    }

    /// The tabs the sessions under way have opened or acted on.
    fn worked_in(&self) -> Vec<u64> {
        (self.tasks.tasks.iter())
            .filter(|task| task.state == State::Working)
            .filter_map(|task| task.session.as_ref())
            .flat_map(|session| session.tabs.iter().copied())
            .collect()
    }

    /// A session's pages still in its client's Space `space`.
    fn pages_of(&self, session: &Session, space: u64, cx: &App) -> Vec<u64> {
        let tabs = self.browser.space_tab_ids(space, cx);
        session
            .tabs
            .iter()
            .copied()
            .filter(|tab| tabs.contains(tab))
            .collect()
    }

    /// At quit: every task goes, and every session, and with `spaces` the tasks' Spaces and the
    /// clients'. With no window left, as when the last one closed on Windows or Linux, the core
    /// can't delete a Space (#1676), and the next launch deletes them, as after a crash.
    pub fn end_tasks(&mut self, spaces: bool, cx: &mut Context<Self>) {
        // A task still at work is there to reply to after the quit, as one stopped (#4105).
        self.keep_at_quit();
        let sessions = (self.tasks.tasks.iter())
            .filter(|task| task.session.is_some())
            .map(|task| task.agent.clone())
            .collect::<Vec<_>>();
        self.tasks.tasks.retain(|task| task.session.is_none());
        log!("browser: {} sessions end", sessions.len());
        let agents: Vec<String> = self
            .tasks
            .tasks
            .iter()
            .map(|task| task.agent.clone())
            .collect();
        for agent in agents {
            self.end_task(&agent, spaces, cx);
        }
        if !spaces {
            return;
        }
        for client in Client::ALL {
            let space = self.browser.agent_space(client.id());
            if space != 0 {
                self.browser.delete_space(space, cx);
            }
        }
    }

    /// At launch: the Spaces a run before left of its tasks and outside agents, as a crash does,
    /// and a quit with no window left (#1676). Each goes with its pages, as at any quit: an
    /// agent's pages were never the user's tabs, and ilo makes none of them the user's by itself,
    /// which only the user does. They stay among the profile's visited pages in History, and a
    /// task is in the Archive to reply to if its assistant kept it (#4105).
    pub fn end_old_tasks(&mut self, cx: &mut Context<Self>) {
        for space in self.browser.task_spaces() {
            log!("browser: deleting space {space} of a task before");
            self.browser.delete_space(space, cx);
        }
        for client in Client::ALL {
            let space = self.browser.agent_space(client.id());
            if space == 0 {
                continue;
            }
            log!(
                "browser: deleting space {space} of agent {} before",
                client.id()
            );
            self.browser.delete_space(space, cx);
        }
        if let Some(dir) = &self.tasks.dir {
            end_old_task_files(&dir.join("Tasks"));
        }
    }

    fn remove_task_files(&self, agent: &str) {
        let Some(dir) = &self.tasks.dir else { return };
        let dir = dir.join("Tasks");
        _ = std::fs::remove_dir_all(dir.join(agent));
        _ = std::fs::remove_file(dir.join(format!("{agent}.json")));
        _ = std::fs::remove_file(dir.join(format!("{agent}.md")));
        _ = std::fs::remove_dir_all(dir.join(format!("{agent} files")));
    }

    /// A task's call, as a step of its trace from when it came, or an outside agent's, as a step
    /// of its client's session, which its first call that opens or acts on a tab starts. A
    /// run's steps are steps of their own (`trace_step`); its name and its finish aren't steps of
    /// its work.
    pub(super) fn trace_call(&mut self, call: &super::Call, cx: &mut Context<Self>) {
        let tool = call.tool.as_str();
        let key = match self.task(&call.agent) {
            Some(_) => call.agent.clone(),
            None => match self.session_for(call, cx) {
                Some(key) => key,
                None => return,
            },
        };
        if matches!(tool, "title" | "finish" | "run_steps") {
            return;
        }
        let did = self.did(
            &call.agent,
            call.connection,
            tool,
            &call.arguments,
            None,
            cx,
        );
        self.trace(&key, call.id, did, call.at, cx);
        // A page opened while the task's helpers work may be one of theirs (#4102).
        if tool == "open_tab"
            && let Some(task) = self.task_mut(&key).filter(|task| task.helpers.on)
            && let Some(at) = task.trace.len().checked_sub(1)
        {
            task.helpers.opens_at(at);
        }
    }

    /// A step of the run `call` on `tab`, starting now, which ends the step before it.
    pub(super) fn trace_step(
        &mut self,
        agent: &str,
        connection: u64,
        call: u64,
        tab: u64,
        step: &Value,
        cx: &App,
    ) {
        let Some(key) = self.tracer(agent, call) else {
            return;
        };
        let tool = step
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let did = self.did(agent, connection, tool, step, Some(tab), cx);
        let now = Instant::now();
        let Some(task) = self.task_mut(&key) else {
            return;
        };
        let before = task
            .trace
            .iter_mut()
            .rev()
            .find(|each| each.call == call && each.ended.is_none());
        if let Some(before) = before {
            before.ended = Some(now);
        }
        self.trace(&key, call, did, now, cx);
    }

    fn trace(&mut self, key: &str, call: u64, did: tools::Did, at: Instant, cx: &App) {
        // A session's pages are the tabs it works in, in its agent's Space: a call refused on the
        // user's own tab doesn't make it one, or a change to the agent's Space would seem to be
        // the user moving it out (#3695).
        let in_space = |agents: &Self, tab: u64| {
            let session = agents.task(key).and_then(|task| task.session.as_ref());
            session.is_some_and(|session| {
                let space = agents.browser.agent_space(&session.agent);
                space != 0 && agents.browser.space_of(tab, cx) == space
            })
        };
        // A page an ended task of its client's held comes at no more than that task's access.
        let left = did.tab.and_then(|tab| self.left_at(key, tab));
        // A page of another session under way stays its card's, though this one acts on it.
        let others = |agents: &Self, tab: u64| {
            (agents.tasks.tasks.iter()).any(|task| {
                task.agent != key
                    && task.state == State::Working
                    && (task.session.as_ref()).is_some_and(|session| session.tabs.contains(&tab))
            })
        };
        // Nor is a page of a task the user stopped taken, until they hand it back.
        let stopped = |agents: &Self, tab: u64| {
            let session = agents.task(key).and_then(|task| task.session.as_ref());
            session.is_some_and(|session| agents.withheld(&session.agent, tab))
        };
        let page = did
            .tab
            .filter(|&tab| in_space(self, tab) && !others(self, tab) && !stopped(self, tab));
        let Some(task) = self.task_mut(key) else {
            return;
        };
        if let (Some(session), Some(tab)) = (&mut task.session, page)
            && !session.tabs.contains(&tab)
        {
            session.tabs.push(tab);
            task.tier = task.tier.min(left.unwrap_or(task.tier));
        }
        // An ask is done once asked: the wait for the answer is the user's.
        let ended = (did.tool == "ask").then_some(at);
        let after = task.messages.len();
        task.trace.push(Traced {
            call,
            did,
            after,
            started: at,
            ended,
            error: None,
            kind: None,
            picture: None,
            close: None,
            notes: None,
        });
        // Its first step: it's under way, and what it wrote before it was said along the way.
        task.waiting = None;
        task.writing = None;
        // A session its agent hasn't named is titled by its sites, at once at the first, then no
        // sooner than RENAMING after its last change, so a growing count can't flicker.
        let fallback = fallback(&task.trace);
        if let Some(session) = &mut task.session
            && task.named.is_none()
            && let Some(title) = fallback
            && task.title != title
            && session
                .renamed
                .is_none_or(|last| at.duration_since(last) >= RENAMING)
        {
            task.title = title.into();
            session.renamed = Some(at);
        }
    }

    /// `agent`'s call `call` names its task (title): a ⌘T task's own, or the session the call
    /// came in. The name is the agent's words in the user's interface, so it's kept to
    /// TITLE_KEPT characters, then written out as a request's text is, on one line (#3645);
    /// a different name sooner than RENAMING after the last is refused. It takes no tab and asks
    /// nothing, and stands only where a title does, never in a question (#3690).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn name_task(
        &mut self,
        agent: &str,
        connection: u64,
        call: u64,
        arguments: &Value,
        now: Instant,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let text = (arguments.get("text").and_then(Value::as_str))
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            _ = reply.send(ToolResult::error(
                "text is the task's name, in two to four words.",
            ));
            return;
        }
        let key = match super::is_task(agent) {
            true => self.task(agent).map(|task| task.agent.clone()),
            false => self.session_of(agent, call).map(|task| task.agent.clone()),
        };
        let Some(task) = key.and_then(|key| self.task_mut(&key)) else {
            _ = reply.send(ToolResult::error("There's no task under way to name."));
            return;
        };
        let kept = page::clip(text, TITLE_KEPT);
        let shown = as_title(text);
        let named = match task.named {
            Some(_) if task.title == shown => Ok(()),
            Some(last) if now.duration_since(last) < RENAMING => Err(NAMED_RECENTLY),
            _ => {
                task.title = shown.into();
                task.named = Some(now);
                Ok(())
            }
        };
        match named {
            Ok(()) => {
                self.note(agent, connection, "title", "", "");
                // A kept task's row in the file has its name from now (pick_up.rs).
                self.kept_named(agent);
                _ = reply.send(ToolResult::text(match kept == text {
                    true => "The task has its name.".to_string(),
                    false => format!("The task has its name: “{kept}”."),
                }));
            }
            Err(why) => {
                self.note(agent, connection, "title", "", why);
                _ = reply.send(ToolResult::error(why));
            }
        }
        cx.notify();
    }

    /// The card whose trace `agent`'s `call` goes in: its task's, or the session it came in.
    fn tracer(&self, agent: &str, call: u64) -> Option<String> {
        let task = self.task(agent).or_else(|| self.session_of(agent, call))?;
        Some(task.agent.clone())
    }

    /// The task an act of `agent`'s on `connection`, for its call `call` (0 for none), is of: a ⌘T
    /// task's own, else the session its call is part of, else its connection's session under way.
    pub(super) fn acting_task(&self, agent: &str, connection: u64, call: u64) -> Option<&Task> {
        if let Some(task) = self.task(agent) {
            return Some(task);
        }
        if let Some(task) = (call != 0).then(|| self.session_of(agent, call)).flatten() {
            return Some(task);
        }
        (self.tasks.tasks.iter()).find(|task| {
            task.state == State::Working
                && (task.session.as_ref()).is_some_and(|session| {
                    session.agent == agent && session.connection == connection
                })
        })
    }

    /// The task under way of `agent`'s whose session holds the page `tab`, among that client's
    /// sessions only: what ilo says and judges about a page is that task's, whichever of its
    /// client's connections calls. A tab that left its Space may still be listed: the core
    /// refuses a call on it before anything read here shows.
    pub(super) fn page_task(&self, agent: &str, tab: u64) -> Option<&Task> {
        (self.tasks.tasks.iter()).find(|task| {
            task.state == State::Working
                && (task.session.as_ref())
                    .is_some_and(|session| session.agent == agent && session.tabs.contains(&tab))
        })
    }

    /// The outside client `agent`'s tasks whose access its others are held to: each under way,
    /// and each that ended with no level picked on another of that client's since.
    fn held_to<'a>(&'a self, agent: &'a str) -> impl Iterator<Item = &'a Task> {
        (self.tasks.tasks.iter()).filter(move |task| {
            (task.session.as_ref()).is_some_and(|session| session.agent == agent && !session.passed)
        })
    }

    /// The access a new task of the outside client `agent`'s starts at. A level the user lowered
    /// outlives its task, for that client, until they pick again: it's the lowest of where every
    /// task starts and of the tasks the client's are held to, so never Full access, which no
    /// task is given but by the user. Only the tasks ilo holds say it: nothing is kept for it.
    pub(super) fn starting_tier(&self, agent: &str) -> Tier {
        let held = self.held_to(agent).map(|task| task.tier);
        held.fold(Tier::default(), Tier::min)
    }

    /// The access the session `key` takes the page `tab` at, if it's lower than its own: the
    /// lowest of the ended tasks of its client's that held the page and are still held to. The
    /// one place ilo changes a task's access, and only downward: the user raises it on its chip.
    fn left_at(&self, key: &str, tab: u64) -> Option<Tier> {
        let agent = &self.task(key)?.session.as_ref()?.agent;
        let held = |task: &&Task| {
            task.state != State::Working
                && (task.session.as_ref()).is_some_and(|session| session.tabs.contains(&tab))
        };
        self.held_to(agent).filter(held).map(|task| task.tier).min()
    }

    /// The user picked a level on the task `key`: the levels its client's other ended tasks were
    /// left at are passed, and its own is the one held to.
    pub(super) fn picked_past(&mut self, key: &str) {
        let Some(agent) =
            (self.task(key)).and_then(|task| Some(task.session.as_ref()?.agent.clone()))
        else {
            return;
        };
        for task in &mut self.tasks.tasks {
            let (picked, ended) = (task.agent == key, task.state != State::Working);
            if let Some(session) = (task.session.as_mut()).filter(|session| session.agent == agent)
            {
                session.passed = !picked && ended;
            }
        }
    }

    /// Tells the task `key` ilo runs what ilo has to say, `text`, which is ilo's own: now if it's
    /// working or `now` is set, which starts it on a turn, else with the user's next turn.
    pub(super) fn tell_task(&mut self, key: &str, text: String, now: bool, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        if task.copying {
            return task.hold(text);
        }
        if !now && task.state != State::Working {
            task.told = Some(text);
            return;
        }
        task.told = None;
        task.state = State::Working;
        task.writing = None;
        let sent = task.process.as_mut().map(|process| process.say(&text));
        if !matches!(sent, Some(Ok(()))) {
            let first = format!("{}\n\n{text}", conversation(task));
            self.launch(key, &first, cx);
        }
        self.wait_for_answer(key, cx);
    }

    /// The session of `agent`'s whose call `call` is under way.
    pub(super) fn session_of(&self, agent: &str, call: u64) -> Option<&Task> {
        (self.tasks.tasks.iter()).find(|task| {
            (task.session.as_ref())
                .is_some_and(|session| session.agent == agent && session.calls.contains(&call))
        })
    }

    /// `agent`'s call was answered `at`: its step ends, with why it failed or what it saw. A
    /// session with no call left under way ends once IDLE passes without another.
    pub(super) fn answered(
        &mut self,
        agent: &str,
        call: u64,
        at: Instant,
        error: Option<String>,
        kind: Option<tools::Kind>,
        image: Option<String>,
        notes: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.tracer(agent, call) else {
            return;
        };
        let Some(task) = self.task_mut(&key) else {
            return;
        };
        let now = cx.background_executor().now();
        if let Some(session) = &mut task.session {
            session.calls.retain(|each| *each != call);
            session.last = Some(now);
        }
        let idle = (task.session.as_ref())
            .is_some_and(|session| session.calls.is_empty() && task.state == State::Working);
        // The agent's own close of a page, done: with none of its pages left open, its task is
        // done unless it calls again within QUIET. The user's close isn't a call, so starts none.
        let closed = (task.trace.iter().rev())
            .find(|step| step.call == call && step.ended.is_none())
            .is_some_and(|step| {
                error.is_none() && matches!(step.did.tool.as_str(), "close_tab" | "remove_tab")
            });
        if idle {
            self.idle_from_now(&key, cx);
            let left = self.task(&key).map(|task| self.pages(task, cx));
            if closed && left.is_some_and(|left| left.is_empty()) {
                self.quiet_from_now(&key, cx);
            }
        }
        let Some(task) = self.task_mut(&key) else {
            return;
        };
        let step = task
            .trace
            .iter_mut()
            .rev()
            .find(|step| step.call == call && step.ended.is_none());
        let Some(step) = step else { return };
        step.ended = Some(at);
        step.notes = notes;
        // An error's first paragraph: a wait's goes on with the page's outline.
        step.kind = kind.filter(|_| error.is_some());
        step.error = error.map(|error| {
            page::clip(error.split("\n\n").next().unwrap_or_default().trim(), 300).into()
        });
        step.picture = image.and_then(|image| picture(&image));
        cx.notify();
    }

    /// `agent`'s close `call` ended as its page had a say, which its line says once answered.
    pub(super) fn close_ended(&mut self, agent: &str, call: u64, close: tools::Close) {
        let Some(key) = self.tracer(agent, call) else {
            return;
        };
        let Some(task) = self.task_mut(&key) else {
            return;
        };
        let step =
            (task.trace.iter_mut().rev()).find(|step| step.call == call && step.ended.is_none());
        if let Some(step) = step {
            step.close = Some(close);
        }
    }

    /// A client said hello on `connection`: the name that connection's session's card shows. One
    /// the user connected in Settings › Agents by name has that; any other MCP client, the name it
    /// gives, as plain text, unless it would pass for the browser, whose words the cards set beside
    /// an agent's.
    pub(super) fn hello(&mut self, agent: &str, connection: u64, client: &str) {
        let given = super::request::plain(client);
        // Each of the `ilo` command's commands says hello, and its session keeps the name it
        // started with, as a client's does.
        let under_way = (self.tasks.tasks.iter()).any(|task| {
            task.state == State::Working
                && (task.session.as_ref()).is_some_and(|session| session.agent == agent)
        });
        let name = match Client::from_id(agent) {
            Some(Client::Command) if under_way => return,
            Some(Client::Other | Client::Command)
                if !given.is_empty() && !names_the_browser(&given) && !names_a_client(client) =>
            {
                given
            }
            Some(known) => known.name().to_string(),
            None => return,
        };
        let name = page::clip(&name, 60).into();
        self.tasks.names.insert((agent.into(), connection), name);
    }

    /// The client's `connection` closed: its name goes, but for the client's last, which shows
    /// where no session's card has one (`agent_name`).
    pub(super) fn goodbye(&mut self, agent: &str, connection: u64) {
        let others = (self.tasks.names.keys()).any(|(each, on)| each == agent && *on != connection);
        if others {
            self.tasks.names.remove(&(agent.into(), connection));
        }
    }

    /// The name the client gave last, on whichever of its connections.
    pub(super) fn given_name(&self, agent: &str) -> Option<SharedString> {
        (self.tasks.names.iter())
            .filter(|((each, _), _)| each == agent)
            .max_by_key(|((_, connection), _)| *connection)
            .map(|(_, name)| name.clone())
    }

    /// The connection whose session's card has `tab` among its pages, of `agent`'s, for what its
    /// page does with no call behind it: the newest's, or 0 for none.
    pub(super) fn holder(&self, agent: &str, tab: u64) -> u64 {
        (self.tasks.tasks.iter().rev())
            .filter_map(|task| task.session.as_ref())
            .find(|session| session.agent == agent && session.tabs.contains(&tab))
            .map_or(0, |session| session.connection)
    }

    /// The user's Space a session's card shows in, as a task's shows in the Space it was asked
    /// from: the one the window in front shows, else the first of the user's in the profile of the
    /// agent's `space` (`user_space`). The browser's own state says it, never anything the client
    /// sends, and a private window's Space is never one: then no Space lists the card.
    fn session_home(&self, space: u64) -> u64 {
        let home = self.browser.user_space(space);
        match self.browser.private_space(home) {
            true => 0,
            false => home,
        }
    }

    /// The session under way of the call's client on the call's connection, or, for a call that
    /// opens or acts on a tab, a new one: its card shows from then. Its calls are its steps.
    fn session_for(&mut self, call: &super::Call, cx: &mut Context<Self>) -> Option<String> {
        // Only a client the user connected has sessions.
        Client::from_id(&call.agent)?;
        let live = (self.tasks.tasks.iter_mut()).find(|task| {
            task.state == State::Working
                && (task.session.as_ref()).is_some_and(|session| {
                    session.agent == call.agent && session.connection == call.connection
                })
        });
        if let Some(task) = live {
            let session = task.session.as_mut().expect("a session, as found");
            session.calls.push(call.id);
            session.idle = None;
            session.quiet = None;
            return Some(task.agent.clone());
        }
        // Listing its tabs isn't work, and done with no task under way has none to end.
        if matches!(call.tool.as_str(), "list_tabs" | "done") {
            return None;
        }
        // Nor is handing the user a page another of its client's sessions holds, or asking for
        // it back: ilo's line of it stands in that session's conversation (`place`), and this
        // connection's task starts with work of its own. A tab the core will refuse is none.
        if matches!(call.tool.as_str(), "hand_off" | "ask_for_tab" | "upload")
            && let Ok(Some(tab)) = tools::tab_argument(&call.arguments)
            && self.page_task(&call.agent, tab).is_some()
            && self.owns(&call.agent, tab, cx)
        {
            return None;
        }
        self.tasks.next += 1;
        let key = format!("session-{}", self.tasks.next);
        let named = self.tasks.names.get(&(call.agent.clone(), call.connection));
        let name = named
            .cloned()
            .unwrap_or_else(|| super::name(&call.agent).into());
        let space = self.space(&call.agent, cx);
        let home = self.session_home(space);
        // check.sh reads this line.
        log!("browser: session {key} of agent {} as {name}", call.agent);
        let session = Session {
            agent: call.agent.clone(),
            connection: call.connection,
            tabs: Vec::new(),
            orders: 0,
            messages: 0,
            calls: vec![call.id],
            idle: None,
            quiet: None,
            last: None,
            renamed: None,
            passed: false,
            stopped: None,
            withheld: false,
        };
        let tier = self.starting_tier(&call.agent);
        self.tasks.tasks.push(Task {
            agent: key.clone(),
            cli: None,
            title: name.clone(),
            name,
            session: Some(session),
            home,
            space,
            asked: SharedString::default(),
            page: None,
            model: SharedString::default(),
            model_id: None,
            added: Vec::new(),
            state: State::Working,
            ended: None,
            end_entry: 0,
            started: SystemTime::now(),
            kept: None,
            orders: 0,
            sent: 0,
            messages: Vec::new(),
            trace: Vec::new(),
            dismissed: false,
            left: None,
            deleted: false,
            token: String::new(),
            process: None,
            asking: None,
            waiting: None,
            writing: None,
            named: None,
            tier,
            sites: Sites::default(),
            stops: Vec::new(),
            stopped_turns: Vec::new(),
            picked_from: None,
            placed: Vec::new(),
            told: None,
            helpers: Helpers::default(),
            files: Given::default(),
            names: Default::default(),
            rising: None,
            rewalled: Vec::new(),
            copying: false,
            sends: Default::default(),
        });
        cx.notify();
        Some(key)
    }

    /// The agent's call `call` opened `tab`, which is a page of its session's, or the page of
    /// the helper of a task's that asked for it (#4102).
    pub(super) fn opened(&mut self, agent: &str, call: u64, tab: u64) {
        if let Some(task) = self.task_mut(agent).filter(|task| task.helpers.on) {
            let opens = |step: &Traced| step.call == call && step.did.tool == "open_tab";
            if let Some(at) = task.trace.iter().rposition(opens) {
                task.helpers.opened(at, tab);
            }
        }
        let Some(key) = self.session_of(agent, call).map(|task| task.agent.clone()) else {
            return;
        };
        let Some(session) = self.task_mut(&key).and_then(|task| task.session.as_mut()) else {
            return;
        };
        if !session.tabs.contains(&tab) {
            session.tabs.push(tab);
        }
    }

    /// `agent`'s call `call` did what asked first, allowed: its session counts it for its receipt,
    /// or, for a task of the user's, the task, for Library › Agents.
    pub(super) fn counted(&mut self, agent: &str, call: u64, did: super::Did) {
        let Some(key) = self.tracer(agent, call) else {
            return;
        };
        let Some(task) = self.task_mut(&key) else {
            return;
        };
        match (did, task.session.as_mut()) {
            (super::Did::Order, Some(session)) => session.orders += 1,
            (super::Did::Message, Some(session)) => session.messages += 1,
            (super::Did::Order, None) => task.orders += 1,
            (super::Did::Message, None) => task.sent += 1,
        }
    }

    /// Each task that has ended since Library › Agents last kept it, as its receipt shows it, with
    /// its home Space, or a session's own, whose profile keeps it. A later turn of a task ends
    /// again, and is kept again in its earlier one's place.
    pub fn take_ended(&mut self, cx: &App) -> Vec<(u64, crate::agent_history::Past)> {
        let finished = |task: &Task| {
            matches!(
                task.state,
                State::Done { .. } | State::Stopped | State::Failed { .. }
            )
        };
        let ended: Vec<String> = (self.tasks.tasks.iter())
            .filter(|task| finished(task) && task.ended.is_some() && task.ended != task.kept)
            .map(|task| task.agent.clone())
            .collect();
        let mut kept = Vec::new();
        for key in ended {
            let Some(task) = self.task(&key) else {
                continue;
            };
            // A session that only named itself did nothing to keep (#3690).
            if task.session.is_some() && task.trace.is_empty() {
                let ended = task.ended;
                if let Some(task) = self.task_mut(&key) {
                    task.kept = ended;
                }
                continue;
            }
            let tabs = match &task.session {
                Some(session) => session.tabs.clone(),
                None => self.browser.space_tab_ids(task.space, cx),
            };
            let pages = (tabs.iter())
                .map(|&tab| self.browser.tab(tab, cx))
                .filter(|page| !page.url.is_empty())
                .map(|page| (page.title.to_string(), page.url.to_string()))
                .collect();
            // Its activity log: the entries of its agent, a session's client's or the task's own,
            // from when it started to when it ended, a run's steps each its own.
            // A session's, of its own connection's and its client's own, as a Stop's, its client's
            // other connections being cards of their own.
            let (logged_as, connection, answers) = match (&task.session, task.cli) {
                (Some(session), _) => {
                    let agent = session.agent.clone();
                    (agent.clone(), session.connection, agent)
                }
                (None, Some(cli)) => (task.agent.clone(), 0, super::task_answers(cli)),
                (None, None) => (task.agent.clone(), 0, task.agent.clone()),
            };
            let (from, to) = (task.started, task.ended.unwrap_or_else(SystemTime::now));
            let entries = (self.log.iter())
                .filter(|entry| {
                    entry.agent == logged_as
                        && (entry.connection == connection || entry.connection == 0)
                        && entry.at >= from
                        && entry.at <= to
                        && entry.id < task.end_entry
                })
                .flat_map(|entry| match entry.steps.is_empty() {
                    true => vec![entry],
                    false => entry.steps.iter().collect(),
                });
            let mut steps: Vec<crate::agent_history::Step> = entries
                .map(|entry| crate::agent_history::Step {
                    at: seconds(entry.at),
                    action: entry.action.to_string(),
                    site: entry.site.to_string(),
                    refused: entry.refused.to_string(),
                    target: entry.target.to_string(),
                    url: entry.url.to_string(),
                    sent: entry.sent.clone(),
                    tier: entry.tier.to_string(),
                })
                .collect();
            let over = steps.len().saturating_sub(crate::agent_history::STEPS);
            steps.drain(..over);
            // A session its agent never named keeps its sites, all of them, as its title.
            let title = match (&task.session, task.named) {
                (Some(_), None) => fallback(&task.trace).unwrap_or_else(|| task.name.to_string()),
                _ => task.title.to_string(),
            };
            let past = crate::agent_history::Past {
                id: format!("{}-{}", task.agent, seconds(task.started)),
                title,
                agent: task.name.to_string(),
                started: seconds(task.started),
                ended: task.ended.map(seconds).unwrap_or_default(),
                pages,
                orders: task
                    .session
                    .as_ref()
                    .map_or(task.orders, |session| session.orders),
                messages: task
                    .session
                    .as_ref()
                    .map_or(task.sent, |session| session.messages),
                steps,
                answers,
                sorted: None,
            };
            // A session has no home of the user's, so its agent's Space says its profile.
            let home = if task.home != 0 {
                task.home
            } else {
                task.space
            };
            kept.push((home, past));
            let ended = task.ended;
            if let Some(task) = self.task_mut(&key) {
                task.kept = ended;
            }
        }
        kept
    }

    /// The client's Space was made again, as the core lost it: its sessions' Space.
    pub(super) fn client_space_made(&mut self, agent: &str, space: u64) {
        for task in &mut self.tasks.tasks {
            if task
                .session
                .as_ref()
                .is_some_and(|session| session.agent == agent)
            {
                task.space = space;
            }
        }
    }

    /// The client's `connection` closed: its session under way there is done.
    pub(super) fn end_session(&mut self, agent: &str, connection: u64, cx: &mut Context<Self>) {
        let live = (self.tasks.tasks.iter()).find(|task| {
            task.state == State::Working
                && (task.session.as_ref()).is_some_and(|session| {
                    session.agent == agent && session.connection == connection
                })
        });
        if let Some(key) = live.map(|task| task.agent.clone()) {
            self.session_done(&key, cx);
        }
    }

    /// The user stopped the outside agent `agent`: its sessions under way, on each connection,
    /// are stopped, and their pages are withheld from it until the user hands them back.
    pub(super) fn session_stopped(&mut self, agent: &str, cx: &App) {
        let now = Instant::now();
        let mut stopped = Vec::new();
        let live = (self.tasks.tasks.iter_mut()).filter(|task| {
            matches!(task.state, State::Working | State::Asking { .. })
                && (task.session.as_ref()).is_some_and(|session| session.agent == agent)
        });
        for task in live {
            task.state = State::Stopped;
            task.end(SystemTime::now());
            if let Some(session) = &mut task.session {
                session.idle = None;
                session.quiet = None;
                session.stopped = Some(now);
                session.withheld = true;
            }
            stopped.push(task.agent.clone());
        }
        for key in stopped {
            self.you_stopped(&key, cx);
        }
    }

    /// Whether `tab` is a page of a task of the outside client `agent`'s that the user stopped
    /// and hasn't handed back: refused to that client whichever connection calls and whatever
    /// its level, and never taken to another task's rail. Read from the stopped task's own
    /// state, never from what a call says.
    pub(super) fn withheld(&self, agent: &str, tab: u64) -> bool {
        (self.tasks.tasks.iter())
            .filter_map(|task| task.session.as_ref())
            .any(|session| {
                session.withheld && session.agent == agent && session.tabs.contains(&tab)
            })
    }

    /// Whether `tab` is a page of a session of the outside client `agent`'s, under way or ended.
    pub(super) fn session_page(&self, agent: &str, tab: u64) -> bool {
        (self.tasks.tasks.iter())
            .filter_map(|task| task.session.as_ref())
            .any(|session| session.agent == agent && session.tabs.contains(&tab))
    }

    /// Whether a call of the outside client `agent`'s that came at `at` came before the user
    /// stopped a task of its: the stopped task's own then, whichever connection it came on.
    pub(super) fn came_before_stop(&self, agent: &str, at: Instant) -> bool {
        (self.tasks.tasks.iter())
            .filter_map(|task| task.session.as_ref())
            .any(|session| {
                session.agent == agent && session.stopped.is_some_and(|stopped| at <= stopped)
            })
    }

    /// Whether the stopped line of `task` still offers its pages back.
    pub(super) fn offers_pages_back(task: &Task) -> bool {
        (task.session.as_ref()).is_some_and(|session| session.withheld)
    }

    /// Hand Back on the stopped line of the outside agent's task `key`: its pages are its
    /// client's again, as those of a task that ended by itself, so a task of that client's takes
    /// one at no more than this task's access (`left_at`). Only the user's press comes here, and
    /// not one meant for what was there before the line.
    fn pages_back(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task(key) else {
            return;
        };
        let Some(session) = (task.session.as_ref()).filter(|session| session.withheld) else {
            return;
        };
        if (session.stopped).is_some_and(|stopped| stopped.elapsed() < tier::PROTECT) {
            log!("browser: task {key} pages back too soon");
            return;
        }
        let agent = session.agent.clone();
        // A count, never a page's title or its site.
        let pages = tools::pages_counted(self.pages(task, cx).len()).unwrap_or_default();
        if let Some(session) = self.task_mut(key).and_then(|task| task.session.as_mut()) {
            session.withheld = false;
        }
        log!("browser: task {key} pages handed back");
        self.note_on(&agent, 0, tools::PAGES_BACK, "", &pages, "");
        cx.notify();
    }

    /// The user stopped the task `key`: the last line of its log says so, with how many pages it
    /// held then.
    fn you_stopped(&mut self, key: &str, cx: &App) {
        let Some(task) = self.task(key) else {
            return;
        };
        let pages = self.pages(task, cx).len();
        self.place_in(key, super::lines::Happened::YouStopped { pages });
    }

    /// `agent`'s call `call` said its task is done (#3652): its session under way is, unless
    /// another call of its own is still under way, such as one waiting on the user, which it
    /// names. A done never leaves a question behind.
    pub(super) fn said_done(
        &mut self,
        agent: &str,
        call: u64,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let live = (self.tasks.tasks.iter()).find(|task| {
            task.state == State::Working
                && (task.session.as_ref())
                    .is_some_and(|session| session.agent == agent && session.calls.contains(&call))
        });
        let Some(task) = live else {
            return Ok("No task of yours is under way.".into());
        };
        let session = task.session.as_ref().expect("a session, as found");
        let under_way = (session.calls.iter()).filter(|each| **each != call).min();
        if let Some(&other) = under_way {
            let step = (task.trace.iter()).find(|step| step.call == other);
            let tool = step.map_or("call", |step| step.did.tool.as_str());
            let on = match step.and_then(|step| step.did.tab) {
                Some(tab) => format!(" on tab {tab}"),
                None => String::new(),
            };
            return Err(format!(
                "Your {tool}{on} is still under way: call done once it's answered."
            ));
        }
        let key = task.agent.clone();
        self.session_done(&key, cx);
        Ok(
            "Your task is done: its card says so. Pages of yours still open stay in your Space \
            for the user to open from it. Your next call starts a new task."
                .into(),
        )
    }

    /// From now, the session ends once QUIET passes without a call, if none of its pages is
    /// open then.
    fn quiet_from_now(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(session) = self.task_mut(key).and_then(|task| task.session.as_mut()) else {
            return;
        };
        let key = key.to_string();
        session.quiet = Some(cx.spawn(async move |agents, cx| {
            cx.background_executor()
                .timer(super::Waits::launch().quiet)
                .await;
            _ = agents.update(cx, |agents, cx| agents.quiet(&key, cx));
        }));
    }

    fn quiet(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task(key) else { return };
        let open = self.pages(task, cx);
        let Some(session) = self.task_mut(key).and_then(|task| task.session.as_mut()) else {
            return;
        };
        if let Some(timer) = session.quiet.take() {
            timer.detach();
        }
        if !session.calls.is_empty() || !open.is_empty() {
            return;
        }
        // check.sh reads this line.
        log!(
            "browser: session {key} quiet for {} s after its last page",
            super::Waits::launch().quiet.as_secs()
        );
        self.session_done(key, cx);
    }

    /// From now, the session ends once IDLE passes without a call.
    fn idle_from_now(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(session) = self.task_mut(key).and_then(|task| task.session.as_mut()) else {
            return;
        };
        let key = key.to_string();
        session.idle = Some(cx.spawn(async move |agents, cx| {
            cx.background_executor().timer(IDLE).await;
            _ = agents.update(cx, |agents, cx| agents.idle(&key, cx));
        }));
    }

    fn idle(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(session) = self.task_mut(key).and_then(|task| task.session.as_mut()) else {
            return;
        };
        // It's this timer that runs: it goes once it's done.
        if let Some(timer) = session.idle.take() {
            timer.detach();
        }
        if !session.calls.is_empty() {
            return;
        }
        // check.sh reads this line.
        log!("browser: session {key} idle for {} s", IDLE.as_secs());
        self.session_done(key, cx);
    }

    /// The session is done: its card has the tick, with the pages it worked in, the first
    /// PAGES, to open.
    fn session_done(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task(key) else { return };
        let Some(session) = &task.session else { return };
        if task.state != State::Working {
            return;
        }
        let mut tabs = self.pages_of(session, task.space, cx);
        tabs.truncate(PAGES);
        // Its work stopped when its last call ended, however long it then waited to end.
        let since = match (session.calls.is_empty(), session.last) {
            (true, Some(last)) => cx
                .background_executor()
                .now()
                .saturating_duration_since(last),
            _ => Duration::ZERO,
        };
        let Some(task) = self.task_mut(key) else {
            return;
        };
        task.state = State::Done {
            result: SharedString::default(),
            summary: None,
            tabs,
        };
        task.end(SystemTime::now() - since);
        if let Some(session) = &mut task.session {
            session.idle = None;
            session.quiet = None;
        }
        // check.sh reads this line.
        log!("browser: session {key} done");
        let agent = (task.session.as_ref()).map(|session| session.agent.clone());
        if let Some(agent) = agent {
            self.command_gone(&agent, cx);
        }
        cx.notify();
    }

    /// The `ilo` command's session ended: with no connection of its own left to say so, it's no
    /// longer live, as a client is once its last connection closes.
    fn command_gone(&mut self, agent: &str, cx: &mut Context<Self>) {
        if agent == Client::Command.id() && self.live.remove(agent) {
            self.forget_waiting(agent, None, cx);
            self.browser.disconnected(agent, cx);
            self.note(agent, 0, "disconnected", "", "");
        }
    }

    /// What a call or a run's step does, as its trace says it: on `tab`, else the tab it names,
    /// else the one its connection used last.
    fn did(
        &self,
        agent: &str,
        connection: u64,
        tool: &str,
        arguments: &Value,
        tab: Option<u64>,
        cx: &App,
    ) -> tools::Did {
        let string = |name: &str| {
            arguments
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let tab = (tab.or_else(|| tools::tab_argument(arguments).ok().flatten()))
            .or_else(|| self.current.get(&(agent.into(), connection)).copied());
        let url = tab
            .map(|tab| self.browser.tab(tab, cx).url.to_string())
            .unwrap_or_default();
        let element = (arguments.get("ref").and_then(Value::as_str).zip(tab))
            .and_then(|(name, tab)| self.refs.get(&tab)?.get(name).cloned());
        let named = |element: &page::Element| match element.name.trim().is_empty() {
            true => format!("the {}", element.role),
            false => page::curly(element.name.trim(), 40),
        };
        let (on, page) = (site(&url), address(&url));
        let mut did = tools::Did {
            tool: tool.to_string(),
            site: on,
            tab,
            page,
            ..Default::default()
        };
        match tool {
            "open_tab" | "navigate" => match string("url").as_str() {
                back @ ("back" | "forward") => did.tool = back.to_string(),
                // Its title says the site; the line opens to the address, whole.
                url => {
                    match self.browser.url_for(url) {
                        Some(url) => {
                            did.site = site(&url);
                            did.page = address(&url);
                            did.text = url;
                        }
                        None => (did.site, did.page) = (page::clip(url, 40), page::clip(url, 60)),
                    }
                    if tool == "open_tab" {
                        did.tab = None;
                    }
                }
            },
            "click" | "type" => {
                did.target = element.as_ref().map(named).unwrap_or_default();
                let typed = string("text");
                // Never a password field's value, nor one whose name says it's a secret.
                let shows = element.as_ref().is_some_and(|element| {
                    let name = element.name.to_lowercase();
                    !element.password && !name.contains("password") && !name.contains("passcode")
                });
                if tool == "type" && shows && typed.chars().count() <= tools::TYPED_SHOWN {
                    did.text = typed;
                }
            }
            "select" => {
                did.target = element.as_ref().map(named).unwrap_or_default();
                did.text = page::clip(&page::one_line(&string("option")), tools::TYPED_SHOWN);
            }
            "press_key" => did.target = string("key"),
            "request" => did.target = super::request::described(arguments),
            "scroll" => {
                did.target = match &element {
                    Some(element) => named(element),
                    None => string("direction"),
                };
                if element.is_some() {
                    did.text = string("direction");
                }
            }
            "wait" => {
                let (text, gone) = (string("text"), string("gone"));
                let seconds = arguments.get("seconds").and_then(Value::as_f64);
                did.target = match (text.is_empty(), gone.is_empty(), seconds) {
                    (false, _, _) => page::curly(&text, 40),
                    (true, false, _) => format!("{} to go", page::curly(&gone, 40)),
                    _ if string("until") == "quiet" => tools::SETTLE.into(),
                    (true, true, Some(seconds)) => duration(seconds.round().max(1.) as u64),
                    (true, true, None) => String::new(),
                };
            }
            "check" => did.target = format!("“{}”", page::clip(&string("text"), 40)),
            "list_tabs" | "ask" => {
                (did.site, did.tab, did.page) = (String::new(), None, String::new())
            }
            _ => {}
        }
        did
    }

    /// The tabs finish's `tabs` leaves the user, in its order, each once: the task's own, and at
    /// most `PAGES`. Else why not.
    fn left_tabs(&self, agent: &str, arguments: &Value, cx: &App) -> Result<Vec<u64>, String> {
        let ids = match arguments.get("tabs") {
            None | Some(Value::Null) => return Ok(Vec::new()),
            Some(Value::Array(ids)) => ids,
            Some(_) => return Err("tabs is a list of your tabs' ids, from list_tabs.".into()),
        };
        let mut tabs = Vec::new();
        for id in ids {
            let tab = id
                .as_u64()
                .ok_or("tabs is a list of your tabs' ids, from list_tabs.")?;
            if !tabs.contains(&tab) {
                tabs.push(tab);
            }
        }
        if tabs.len() > PAGES {
            return Err(format!(
                "tabs takes {PAGES} tabs at most: leave the user the {PAGES} that matter most."
            ));
        }
        let space = self.task(agent).map_or(0, |task| task.space);
        let own = self.browser.space_tab_ids(space, cx);
        match tabs.iter().find(|tab| !own.contains(tab)) {
            Some(tab) => Err(format!(
                "Tab {tab} isn't one of yours: give the ids of your own tabs, from list_tabs."
            )),
            None => Ok(tabs),
        }
    }

    /// The task-only tools: title, ask and finish. Each writes an activity-log entry.
    pub(super) fn task_tool(
        &mut self,
        agent: &str,
        tool: &str,
        arguments: &Value,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let text = |name: &str| {
            arguments
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
        };
        match tool {
            "ask" => {
                let question = text("question");
                if question.is_empty() {
                    _ = reply.send(ToolResult::error("question is what you ask the user."));
                    return;
                }
                let answers: Vec<SharedString> =
                    (arguments.get("answers").and_then(Value::as_array))
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|answer| !answer.is_empty())
                        .map(|answer| page::clip(answer, 60).into())
                        .collect();
                // Up to three short answers are buttons; else the user answers in the
                // conversation.
                let buttons = match answers.len() <= ANSWERS
                    && answers
                        .iter()
                        .all(|answer| answer.chars().count() <= ANSWER_LENGTH)
                {
                    true => answers,
                    false => Vec::new(),
                };
                // The conversation has the context over the question; the card, the question.
                let said = match text("context") {
                    "" => page::clip(question, 2000),
                    context => format!(
                        "{}\n\n{}",
                        page::clip(context, 2000),
                        page::clip(question, 300)
                    ),
                };
                let question: SharedString = page::clip(card_question(question), 300).into();
                let number = self.next_request;
                self.next_request += 1;
                let Some(task) = self.task_mut(agent) else {
                    return;
                };
                task.messages.push(Message::new(Who::Asked, said));
                task.state = State::Asking {
                    question,
                    answers: buttons,
                };
                task.asking = Some((number, reply));
                self.note(agent, 0, "ask", "", "");
            }
            "finish" => {
                let result = text("result");
                if result.is_empty() {
                    _ = reply.send(ToolResult::error(
                        "result is what you did, in a sentence or two.",
                    ));
                    return;
                }
                // A model can write its other arguments into result as markup, which would show as
                // it is and lose them, so it calls again.
                if result.contains("<parameter name=") || result.contains("</result>") {
                    _ = reply.send(ToolResult::error(FINISH_AGAIN));
                    return;
                }
                let mut tabs = match self.left_tabs(agent, arguments, cx) {
                    Ok(tabs) => tabs,
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
                // Done, it holds no tab of the user's: each goes back to their tabs, and is none
                // of the pages the thread leaves them.
                let users = self.handed_order(agent);
                tabs.retain(|tab| !users.contains(tab));
                self.hand_all_back(agent, cx);
                let site = match tabs.as_slice() {
                    [tab] => site(&self.browser.tab(*tab, cx).url),
                    [] => String::new(),
                    tabs => format!("{} pages", tabs.len()),
                };
                // Whole: the card's ellipsis is the only cut.
                let summary = Some(text("summary"))
                    .filter(|summary| !summary.is_empty())
                    .map(SharedString::from);
                let Some(task) = self.task_mut(agent) else {
                    return;
                };
                // The answer written just before finish is its result, which shows once.
                if (task.messages.last())
                    .is_some_and(|last| last.who == Who::Agent && last.text.trim() == result)
                {
                    task.messages.pop();
                    let messages = task.messages.len();
                    for step in &mut task.trace {
                        step.after = step.after.min(messages);
                    }
                }
                let result: SharedString = page::clip(result, 2000).into();
                task.messages
                    .push(Message::new(Who::Result, result.clone()));
                task.state = State::Done {
                    result,
                    summary,
                    tabs,
                };
                task.end(SystemTime::now());
                self.keep_ended(agent);
                self.note(agent, 0, "finish", &site, "");
                _ = reply.send(ToolResult::text(
                    "Done. The user sees your result. Stop here.",
                ));
            }
            _ => _ = reply.send(ToolResult::error(format!("There's no tool {tool}."))),
        }
        cx.notify();
    }
}

/// An ask's line on the card: the question alone. An agent that put what the user needs to know
/// in it too has its last sentence that asks, else all of it.
pub fn card_question(asked: &str) -> &str {
    let Some(end) = asked.rfind('?') else {
        return asked;
    };
    let before = &asked[..end];
    // A sentence starts after a newline, or after an end and a space with no lowercase after it,
    // so "i.e. the museum" stays one.
    let start = (before.char_indices().rev())
        .find(|&(at, char)| {
            let rest = &asked[at + char.len_utf8()..];
            char == '\n'
                || (matches!(char, '.' | '!' | '?')
                    && rest.starts_with(char::is_whitespace)
                    && !rest.trim_start().starts_with(char::is_lowercase))
        })
        .map_or(0, |(at, char)| at + char.len_utf8());
    asked[start..=end].trim()
}

/// What finish says to a result with its other arguments written into it.
pub const FINISH_AGAIN: &str = "result has your other arguments written into it: call finish again with result, summary and tabs each an argument of its own.";

/// What Retry says to the agent.
const RETRY: &str = "Go on from where you stopped.";

/// The first message: what the user asked, and the page they were on, by its address and title.
pub fn first_message(asked: &str, page: Option<&AskedOn>) -> String {
    // With no words, as a thread started with a tab alone, the page is the whole of it.
    let before = match asked.is_empty() {
        true => "",
        false => "\n\n",
    };
    match page {
        Some(page) => format!(
            "{asked}{before}(I'm on “{}”, {}. Open it in a tab of your own if you need it.)",
            super::request::plain(&page.title),
            page.url
        ),
        None => asked.to_string(),
    }
}

/// `first` with the addresses of the pages the user added, for the task to open in tabs of its own:
/// their addresses alone, as the user's own tabs are never the task's. Each is on a line of its own
/// under the sentence, as an address may hold a comma or a bracket, which would read as the
/// sentence's; a parsed address holds no line break (`added_address`).
pub(super) fn with_added(first: String, pages: &[String]) -> String {
    match pages {
        [] => first,
        pages => format!(
            "{first}\n\nPages I added, to open in tabs of your own:\n{}",
            pages.join("\n")
        ),
    }
}

/// The conversation so far, for a new run of a task whose CLI went.
pub(super) fn conversation(task: &Task) -> String {
    conversation_to(task, None)
}

/// The conversation so far, with `last`, if any, as the user's last message in it: the reply as
/// its assistant is given it now. A message of theirs before it that handed the thread tabs says
/// how many and nothing of which: they went back when the thread was last no longer under way.
fn conversation_to(task: &Task, last: Option<&str>) -> String {
    let first = with_added(first_message(&task.asked, task.page.as_ref()), &task.added);
    let mut text = handed::with_told(first, task.sends.told_again(0));
    let replies = pick_up::replies(task);
    let mut reply = 0;
    for message in &task.messages {
        let said = match message.who {
            Who::User => {
                reply += 1;
                match last.filter(|_| reply == replies) {
                    Some(last) => last.to_string(),
                    None => {
                        let told = task.sends.told_again(reply);
                        handed::with_told(message.text.to_string(), told)
                    }
                }
            }
            _ => message.text.to_string(),
        };
        let who = if message.who == Who::User {
            "Me"
        } else {
            "You"
        };
        text.push_str(&format!("\n\n{who}: {said}"));
    }
    text.push_str("\n\nGo on from here.");
    text
}

/// A time in seconds since 1970 as the clock reads it here, such as 18:00.
fn clock(at: u64) -> SharedString {
    crate::agents::local_time(at).into()
}

fn seconds(at: SystemTime) -> u64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// What a task at its plan's limit says: when it resets, if the CLI, by `name`, said.
fn limit_words(name: &str, resets: &Option<SharedString>) -> String {
    match resets {
        Some(at) => format!("Limit resets at {at}"),
        None => format!("{name} hit its usage limit"),
    }
}

/// A done task's finding: the first paragraph of the agent's last answer, as plain text on one
/// line, with no link, formatting or line break, kept to FINDING_KEPT characters and written out as
/// its title is (#3690), so nothing in it can pose as ilo's words or turn the text around. It is
/// the agent's words: shown on its card only, never logged.
pub fn finding(messages: &[Message]) -> SharedString {
    let Some(answer) = last_answer(messages) else {
        return SharedString::default();
    };
    let paragraph: Vec<&str> = (answer.lines())
        .map(str::trim)
        .skip_while(|line| line.is_empty())
        .take_while(|line| !line.is_empty())
        .filter(|line| !line.starts_with("```"))
        .map(|line| {
            let line = line.trim_start_matches(['#', '>']).trim_start();
            let unlisted = (line.split_once(". "))
                .filter(|(number, _)| {
                    !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
                })
                .map(|(_, rest)| rest);
            match unlisted {
                Some(rest) => rest,
                None => (["- ", "* ", "+ "].iter())
                    .find_map(|mark| line.strip_prefix(mark))
                    .unwrap_or(line),
            }
        })
        .collect();
    let plain = plain_text(&paragraph.join(" "));
    let kept = page::clip(&plain, FINDING_KEPT);
    super::request::escaped(&kept, false).0.into()
}

/// What a screen reader is told of a done task's finding: its last answer's first line as the
/// conversation tells it (`told_line`), up to TOLD_KEPT characters ending at a whole word, as
/// nothing marks a cut in speech. Empty with no answer.
pub fn told(messages: &[Message]) -> SharedString {
    last_answer(messages)
        .and_then(told_line)
        .unwrap_or_default()
}

/// The agent's last answer, trimmed, if it gave one.
fn last_answer(messages: &[Message]) -> Option<&str> {
    (messages.iter().rev())
        .filter(|message| matches!(message.who, Who::Agent | Who::Result))
        .map(|message| message.text.trim())
        .find(|text| !text.is_empty())
}

/// Markdown's inline marks taken out of `text`: a link or an image is its words, a bare link in
/// angle brackets its address, and emphasis, code and strikethrough marks go. Runs of white space
/// are one space.
fn plain_text(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let opens = rest.strip_prefix("![").or_else(|| rest.strip_prefix('['));
        let link = opens.and_then(|inside| {
            let (words, after) = inside.split_once("](")?;
            let (_, after) = after.split_once(')')?;
            (!words.contains(']')).then_some((words, after))
        });
        if let Some((words, after)) = link {
            plain.push_str(words);
            rest = after;
            continue;
        }
        let autolink = (rest.strip_prefix('<'))
            .and_then(|inside| inside.split_once('>'))
            .filter(|(address, _)| {
                address.contains("://") && !address.contains(char::is_whitespace)
            });
        if let Some((address, after)) = autolink {
            plain.push_str(address);
            rest = after;
            continue;
        }
        if let Some(after) = ["**", "__", "~~"]
            .iter()
            .find_map(|mark| rest.strip_prefix(mark))
        {
            rest = after;
            continue;
        }
        if !matches!(c, '*' | '`') {
            plain.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A result's first words for its card's line: its first line of text, or "Finished" with none.
fn first_words(result: &str) -> SharedString {
    first_line(result).unwrap_or_else(|| "Finished".into())
}

/// A result's first line of text, without the marks that make it a heading, a list's item or
/// bold, a link's address left for its label, then cleaned as a title is: clipped at TITLE_KEPT
/// and what wouldn't show or would move the text around it written out. None with no text.
pub fn first_line(result: &str) -> Option<SharedString> {
    let kept = page::clip(&first_text(result)?, TITLE_KEPT);
    Some(super::request::escaped(&kept, false).0.into())
}

/// A result's first line as a screen reader is told it: the card's line's text, cleaned the same,
/// but up to TOLD_KEPT characters and ending with a whole word, as nothing marks a cut in speech.
pub fn told_line(result: &str) -> Option<SharedString> {
    let line = first_text(result)?;
    Some(
        super::request::escaped(words_within(&line, TOLD_KEPT), false)
            .0
            .into(),
    )
}

/// A result's first line of text without its marks, a link its label. None with no text.
fn first_text(result: &str) -> Option<String> {
    let line = (result.lines())
        .map(|line| {
            line.trim()
                .trim_start_matches(['#', '>', '-', '*', ' '])
                .trim_end_matches(['*', ' '])
        })
        .find(|line| !line.is_empty())?;
    Some(labels(&line.replace("**", "")))
}

/// `text` whole, or the words of it that end within its first `chars` characters. A first word
/// longer than that is cut there.
fn words_within(text: &str, chars: usize) -> &str {
    let Some((end, next)) = text.char_indices().nth(chars) else {
        return text;
    };
    let head = &text[..end];
    let whole = match next.is_whitespace() {
        true => head,
        false => head
            .rfind(char::is_whitespace)
            .map_or("", |space| &head[..space]),
    };
    match whole.trim_end() {
        "" => head,
        words => words,
    }
}

/// `text` with each "[label](address)" its label alone.
fn labels(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        plain.push_str(&rest[..open]);
        let link = &rest[open + 1..];
        let label = link.find("](").and_then(|close| {
            let end = link[close + 2..].find(')')?;
            Some((&link[..close], close + 2 + end + 1))
        });
        match label {
            Some((label, length)) => {
                plain.push_str(label);
                rest = &link[length..];
            }
            None => {
                plain.push('[');
                rest = link;
            }
        }
    }
    plain.push_str(rest);
    plain
}

/// A stopped task's last finished step, as its trace line says it, its thoughts left out. A step
/// under way at Stop, or one that failed, never is one, so a card never says `Couldn't …` of it.
fn last_finished(task: &Task) -> Option<SharedString> {
    (task.trace.iter().rev())
        .filter(|step| step.did.tool != "thought")
        .find(|step| step.ended.is_some() && step.error.is_none())
        .map(|step| tools::step_title(&step.did, tools::Tense::Done).into())
}

/// What a card's status leads with when a confirmation or a handoff waits on the user, the one
/// status the card sets heavier (`status_lead`).
pub const NEEDS_YOU: &str = "Needs you";
/// A card's status while a refusal of its task's access is unsettled, before what it stopped.
pub const STOPPED_AT: &str = "Stopped at";

/// The most steps a card lists.
pub const CARD_STEPS: usize = 3;

/// A task's last steps this turn, oldest first, by the line that says the step it's on.
fn last_steps(task: &Task) -> Vec<SharedString> {
    let mut steps: Vec<SharedString> = (turn_steps(task).into_iter().take(CARD_STEPS))
        .map(|step| tools::step_title(&step.did, tense(step)).into())
        .collect();
    steps.reverse();
    steps
}

/// Those of `pages` a step of `task`'s has been on.
fn visited(task: &Task, pages: &[u64]) -> Vec<u64> {
    (pages.iter().copied())
        .filter(|&tab| task.trace.iter().any(|step| step.did.tab == Some(tab)))
        .collect()
}

/// Of a task's `pages` now, the last its steps acted on, else its last.
fn last_tab(task: &Task, pages: &[u64]) -> Option<u64> {
    (task.trace.iter().rev())
        .filter_map(|step| step.did.tab)
        .find(|tab| pages.contains(tab))
        .or(pages.last().copied())
}

/// A task's status while it works, saying how far it has got by its own steps: "Step 5 · Clicking
/// “Tickets”", the step of the turn it's on, else the last it took, counted among the turn's steps
/// with its thoughts left out; "Thinking · no steps yet" until it takes one. It changes only as a
/// step comes, never on a clock.
fn working(task: &Task) -> SharedString {
    // With helpers at work, the conversation's live line, in its own words: their steps come in
    // turn, and a card that said each would flicker between them.
    let helpers = helpers_under_way(task);
    if helpers > 0 {
        let (title, counted) = summary(None, (helpers, Counted::Helpers));
        return format!("{title} · {counted}").into();
    }
    let taken = turn_steps(task);
    // A session's title is the agent's words or its sites, so its line leads with its client's
    // name, then what it's doing, whose room the step count would take (#3690).
    match (taken.first(), &task.session) {
        (Some(step), Some(_)) => format!(
            "{} · {}",
            task.name,
            tools::step_title(&step.did, tense(step))
        )
        .into(),
        (Some(step), None) => format!(
            "Step {} · {}",
            taken.len(),
            tools::step_title(&step.did, tense(step))
        )
        .into(),
        (None, Some(_)) => format!("{} · no steps yet", task.name).into(),
        (None, None) => "Thinking · no steps yet".into(),
    }
}

/// How many helpers the stretch under way of a working task has, which its conversation's live
/// line counts: those its CLI said started since the last message that ends a stretch, the
/// user's or a question of the task's, each from its start and whether it has ended or not, and
/// those the steps since are labelled as. A card is made again whenever any task changes, so it
/// counts them from those alone, as it reads its own step, and never folds the conversation
/// (`fold`), whose live line a test holds it to.
fn helpers_under_way(task: &Task) -> usize {
    if !task.helpers.on || task.state != State::Working {
        return 0;
    }
    let ends = |message: &Message| matches!(message.who, Who::User | Who::Asked);
    let from = (task.messages.iter().rposition(ends)).map_or(0, |at| at + 1);
    let began = task.helpers.began().filter(|began| began.after >= from);
    let mut helpers: Vec<usize> = began.map(|began| began.helper).collect();
    for (key, step) in (task.trace.iter().enumerate().rev()).take_while(|(_, s)| s.after >= from) {
        if let Some(helper) = task.helpers.of(key, step.did.tab)
            && !helpers.contains(&helper)
        {
            helpers.push(helper);
        }
    }
    helpers.len()
}

/// Each page's own last step of the turn under way, newest first, once the turn's steps are on
/// more than one page or the task has helpers: else the task's step is its one page's. A page a
/// helper opened has that step from when it's open, and one whose last step was a helper's has
/// none once that helper has ended, as nothing works in it.
fn page_steps(task: &Task) -> Vec<(u64, SharedString)> {
    let turn = task.turn();
    let steps = (task.trace.iter().enumerate().rev())
        .take_while(|(_, step)| step.after >= turn)
        .filter(|(_, step)| step.did.tool != "thought" && !step.stopped());
    let mut pages: Vec<(u64, SharedString)> = Vec::new();
    let mut over: Vec<u64> = Vec::new();
    for (key, step) in steps {
        let Some(tab) = step.did.tab.or_else(|| task.helpers.opened_at(key)) else {
            continue;
        };
        if pages.iter().any(|(each, _)| *each == tab) || over.contains(&tab) {
            continue;
        }
        let helper = task.helpers.of(key, Some(tab));
        match helper.is_some_and(|helper| task.helpers.ended(helper)) {
            true => over.push(tab),
            false => pages.push((tab, tools::step_title(&step.did, tense(step)).into())),
        }
    }
    match pages.len() {
        0 | 1 if !task.helpers.on => Vec::new(),
        _ => pages,
    }
}

/// The web sites a task's steps acted on, in the order it first did, for a title from them.
fn sites(trace: &[Traced]) -> Vec<&str> {
    let mut sites: Vec<&str> = Vec::new();
    for step in trace {
        let site = step.did.site.as_str();
        if super::web_site(site) && !sites.contains(&site) {
            sites.push(site);
        }
    }
    sites
}

/// The title of a session its agent didn't name: its first site, and how many more it acted on,
/// "docs.example and 2 more", or none before it acted on one (#3690). Written out as the agent's
/// title is, as a site the core couldn't read as an address is the agent's text.
fn fallback(trace: &[Traced]) -> Option<String> {
    let first = super::request::escaped(&page::clip(sites(trace).first()?, TITLE_KEPT), false).0;
    match sites(trace).len() - 1 {
        0 => Some(first),
        more => Some(format!("{first} and {more} more")),
    }
}

/// Whether a task waiting on the user works on past the wait: a step came after a refusal it
/// hasn't settled, or a step of its, or a helper's, is under way that isn't itself a wait or on a
/// page the user holds. Read from the steps alone, never from what the agent says.
fn went_on<T>(task: &Task, held: &HashMap<u64, T>) -> bool {
    let after_refusal = (task.stops.iter())
        .filter(|stop| !stop.settled)
        .any(|stop| {
            task.trace[stop.traced.min(task.trace.len())..]
                .iter()
                .any(|step| step.did.tool != "thought")
        });
    let under_way = (task.trace.iter()).any(|step| {
        step.ended.is_none()
            && step.did.tool != "thought"
            && !WAIT_ON_USER.contains(&step.did.tool.as_str())
            && step.did.tab.is_none_or(|tab| !held.contains_key(&tab))
    });
    after_refusal || under_way
}

/// The steps of the turn under way, newest first, its thoughts left out: what the card and the
/// conversation's header count.
fn turn_steps(task: &Task) -> Vec<&Traced> {
    let turn = task.turn();
    (task.trace.iter().rev())
        .take_while(|step| step.after >= turn)
        .filter(|step| step.did.tool != "thought" && !step.stopped())
        .collect()
}

/// `text` with its first letter in lower case, as a fact after " · " reads (#3690).
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

/// A stopped task's status: how far it got before the user stopped it, by its steps, its thoughts
/// left out.
fn stopped(task: &Task) -> SharedString {
    match (task.trace.iter())
        .filter(|step| step.did.tool != "thought" && !step.stopped())
        .count()
    {
        0 => "Stopped".into(),
        1 => "Stopped after 1 step".into(),
        steps => format!("Stopped after {steps} steps").into(),
    }
}

/// The status while a step waits on the user: that it does, and why, as the agent gave it.
fn needs_you(question: &Question) -> SharedString {
    match question.why.is_empty() {
        true => NEEDS_YOU.into(),
        false => format!("{NEEDS_YOU} · {}", question.why.trim_end_matches('.')).into(),
    }
}

/// How much of a card's `status` leads it, set in the title's ink and heavier: the step count
/// while it works ("Step 5"), and "Needs you" while a step waits on the
/// user, the one state that asks for them. Zero for the rest. By the words alone, as a status
/// rolling out has left its card's state behind.
pub fn status_lead(status: &str) -> usize {
    let counts = (status.strip_prefix("Step "))
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()));
    match counts || status.starts_with(NEEDS_YOU) {
        true => status.find(" · ").unwrap_or(status.len()),
        false => 0,
    }
}

fn tense(step: &Traced) -> tools::Tense {
    match (step.ended, &step.error) {
        (None, _) => tools::Tense::Now,
        (Some(_), None) => tools::Tense::Done,
        (Some(_), Some(_)) => tools::Tense::Failed,
    }
}

/// A page's address as a trace's line says it: without its scheme, "www." or a lone "/".
fn address(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    page::clip(rest.strip_suffix('/').unwrap_or(rest), 60)
}

/// A duration in whole seconds as a trace says it, in the units the app's other times have:
/// "41 s", "2 min 5 s".
pub fn duration(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds} s"),
        _ if seconds % 60 == 0 => format!("{} min", seconds / 60),
        _ => format!("{} min {} s", seconds / 60, seconds % 60),
    }
}

#[cfg(test)]
thread_local! {
    /// How many conversations this thread folded, for the test that a card costs none.
    pub(super) static FOLDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// An ilo line, and how many of its task's messages and steps came before it, which places it
/// among them.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub after: usize,
    pub traced: usize,
    pub line: ilo::IloLine,
}

/// A conversation's lines from its `messages` and its `trace`, oldest first. Each turn, from a
/// message of the user's to the next, shows its questions and, once it has `ended`, its last
/// message; the steps between those are a trace each, with what the agent said along the way in
/// it. The last stretch is under way while `working`, and what the agent is `writing` shows under
/// it, as it comes and once it's whole, until the turn ends or a call takes it in: `working` heads
/// the stretch and never withholds the words, which the caller gives only while its task works.
/// ilo's lines, `marks`, each go where it came among the messages and the steps, ending the
/// stretch of steps before it.
pub fn fold(
    messages: &[Message],
    trace: &[Traced],
    ended: bool,
    working: bool,
    writing: Option<&Writing>,
    helpers: &Helpers,
    stopped: &[usize],
    marks: Vec<Mark>,
) -> Vec<Line> {
    #[cfg(test)]
    FOLDS.with(|folds| folds.set(folds.get() + 1));
    let writing = writing.filter(|_| !ended);
    // Once whole, it's the last message, which shows where it was written rather than in the trace.
    let held = (writing.is_some_and(|writing| writing.said))
        .then(|| messages.len().checked_sub(1))
        .flatten()
        .filter(|&last| messages[last].who == Who::Agent);
    // Which messages show as lines of their own.
    let mut shown = vec![false; messages.len()];
    let mut first = 0;
    let turns: Vec<&[Message]> = messages
        .split_inclusive(|message| message.who == Who::User)
        .collect();
    for (index, turn) in turns.iter().enumerate() {
        // A turn the user has written after is over, even as the next is under way.
        let turn_ended = ended || turn.last().is_some_and(|message| message.who == Who::User);
        // The turn's last message: its result, else the last thing it said, once it's over by
        // itself. One the user stopped has no answer: what it said stays in its stretch.
        let last = (turn.iter().rposition(|message| message.who == Who::Result))
            .or_else(|| turn.iter().rposition(|message| message.who != Who::User))
            .filter(|_| turn_ended && !stopped.contains(&index));
        for (at, message) in turn.iter().enumerate() {
            shown[first + at] = matches!(message.who, Who::User | Who::Asked) || Some(at) == last;
        }
        first += turn.len();
    }
    let mut lines = Vec::new();
    let mut stretch = Stretch {
        helpers: Some(helpers),
        ..Stretch::default()
    };
    let mut steps = trace.iter().enumerate().peekable();
    let mut began = helpers.began().peekable();
    let mut marks = marks.into_iter().peekable();
    for (index, message) in messages.iter().enumerate() {
        stretch.gather(&mut steps, &mut began, Some(index), &mut lines, &mut marks);
        place(&mut lines, &mut stretch, &mut marks, trace.len(), index);
        if Some(index) == held {
            continue;
        }
        if !shown[index] {
            stretch.gathered.push(Gathered::Said(message.text.clone()));
            continue;
        }
        let end = (message.who != Who::User).then_some(message.at);
        stretch.flush(&mut lines, false, end);
        lines.push(match message.who {
            Who::User => Line::You(message.text.clone()),
            _ => Line::Said(message.text.clone()),
        });
    }
    stretch.gather(&mut steps, &mut began, None, &mut lines, &mut marks);
    stretch.flush(&mut lines, working, None);
    lines.extend(marks.map(|mark| Line::Ilo(mark.line)));
    if let Some(writing) = writing {
        // Never under a button: above the trailing line of ilo's that waits on the user, which
        // stays the last over the field.
        let trailing = lines.len()
            - (lines.iter().rev())
                .take_while(|line| matches!(line, Line::Ilo(_)))
                .count();
        let at = (trailing..lines.len())
            .find(|&at| matches!(&lines[at], Line::Ilo(line) if !ilo::buttons(line).is_empty()))
            .unwrap_or(lines.len());
        lines.insert(at, Line::Said(writing.text.clone()));
    }
    lines
}

/// ilo's lines that came before the step `key` of the trace and the message `index`, each where
/// it came: the stretch of steps before it ends there.
fn place(
    lines: &mut Vec<Line>,
    stretch: &mut Stretch,
    marks: &mut Peekable<std::vec::IntoIter<Mark>>,
    key: usize,
    index: usize,
) {
    while let Some(mark) = marks.next_if(|mark| mark.traced <= key && mark.after <= index) {
        stretch.flush(lines, false, None);
        lines.push(Line::Ilo(mark.line));
    }
}

/// A session's lines: its traces' steps, each a line of its own, newest last.
fn steps(lines: Vec<Line>) -> Vec<Line> {
    (lines.into_iter())
        .flat_map(|line| match line {
            Line::Trace(trace) => (trace.items.into_iter())
                .filter_map(|item| match item {
                    TraceItem::Step(step) => Some(Line::Step(step)),
                    TraceItem::Said(_) | TraceItem::Helper { .. } => None,
                })
                .collect(),
            // ilo's lines are a session's too.
            Line::Ilo(line) => vec![Line::Ilo(line)],
            _ => Vec::new(),
        })
        .collect()
}

/// The stretch of work a trace is made of, as it gathers.
#[derive(Default)]
struct Stretch<'a> {
    /// The task's helpers, for the names it gave them.
    helpers: Option<&'a Helpers>,
    gathered: Vec<Gathered<'a>>,
    key: Option<usize>,
    started: Option<Instant>,
    ended: Option<Instant>,
    /// Its steps that waited on the user, from their start to their end, and the rest, which
    /// worked: what it took counts the time the first alone filled as none (`worked`).
    waits: Vec<(Instant, Instant)>,
    busy: Vec<(Instant, Instant)>,
}

/// The tools whose step waits on the user from its start to its end: none of that is work.
const WAIT_ON_USER: [&str; 4] = ["hand_off", "upload", "ask_for_tab", "ask"];

/// How long a stretch from `started` to `ended` worked: all of it but the time only `waits`
/// filled, while no step of `busy` ran. From the steps' own marks, never a timer.
fn worked(
    started: Instant,
    ended: Instant,
    waits: &[(Instant, Instant)],
    busy: &[(Instant, Instant)],
) -> std::time::Duration {
    let mut marks: Vec<Instant> = (waits.iter().chain(busy))
        .flat_map(|&(from, to)| [from, to])
        .map(|mark| mark.clamp(started, ended))
        .chain([started, ended])
        .collect();
    marks.sort();
    marks.dedup();
    let within = |spans: &[(Instant, Instant)], from: Instant, to: Instant| {
        spans.iter().any(|&(start, end)| start <= from && to <= end)
    };
    let idle: std::time::Duration = (marks.windows(2))
        .filter(|pair| within(waits, pair[0], pair[1]) && !within(busy, pair[0], pair[1]))
        .map(|pair| pair[1] - pair[0])
        .sum();
    ended.saturating_duration_since(started) - idle
}

/// A step of a stretch, with its place in the task's trace, or what the agent said in it.
enum Gathered<'a> {
    Step(usize, &'a Traced),
    Said(SharedString),
    /// A step labelled as a helper's (#4102), by which of the task's helpers.
    Helper(usize, usize, &'a Traced),
    /// A helper started, which shows from then.
    Began(usize),
}

impl<'a> Stretch<'a> {
    /// Takes in what came before the message at `before`, or all that's left: each step, and
    /// each helper from where it started among them (#4102).
    fn gather(
        &mut self,
        steps: &mut Peekable<impl Iterator<Item = (usize, &'a Traced)>>,
        began: &mut Peekable<impl Iterator<Item = helpers::Began>>,
        before: Option<usize>,
        lines: &mut Vec<Line>,
        marks: &mut Peekable<std::vec::IntoIter<Mark>>,
    ) {
        let due = |after: usize| before.is_none_or(|index| after <= index);
        loop {
            let step = steps.peek().copied().filter(|(_, step)| due(step.after));
            let helper = began.peek().filter(|began| due(began.after));
            match (step, helper) {
                (step, Some(helper)) if step.is_none_or(|(key, _)| helper.from <= key) => {
                    self.began(helper);
                    began.next();
                }
                (Some((key, step)), _) => {
                    place(lines, self, marks, key, before.unwrap_or(usize::MAX));
                    let helper = self.helpers.and_then(|all| all.of(key, step.did.tab));
                    self.step(key, step, helper);
                    steps.next();
                }
                (None, _) => break,
            }
        }
    }

    fn step(&mut self, key: usize, step: &'a Traced, helper: Option<usize>) {
        if step.stopped() {
            return;
        }
        self.key.get_or_insert(key);
        self.started = Some(
            self.started
                .map_or(step.started, |started| started.min(step.started)),
        );
        let end = step.ended.unwrap_or(step.started);
        self.ended = Some(self.ended.map_or(end, |ended| ended.max(end)));
        match WAIT_ON_USER.contains(&step.did.tool.as_str()) {
            true => self.waits.push((step.started, end)),
            false => self.busy.push((step.started, end)),
        }
        self.gathered.push(match helper {
            Some(helper) => Gathered::Helper(helper, key, step),
            None => Gathered::Step(key, step),
        });
    }

    /// A helper started in it, whose line stands here whether or not a step of its own follows.
    fn began(&mut self, began: &helpers::Began) {
        self.key.get_or_insert(helpers::BEGAN + began.helper);
        self.started = Some(
            self.started
                .map_or(began.at, |started| started.min(began.at)),
        );
        self.ended = Some(self.ended.map_or(began.at, |ended| ended.max(began.at)));
        self.gathered.push(Gathered::Began(began.helper));
    }

    /// The stretch as its lines: a trace, or with no steps, what the agent said, as itself. It's
    /// `running` if it's the stretch under way, and ends at the agent's message `end`, if one ends
    /// it.
    fn flush(&mut self, lines: &mut Vec<Line>, running: bool, end: Option<Instant>) {
        let gathered = std::mem::take(&mut self.gathered);
        let (key, started, ended) = (self.key.take(), self.started.take(), self.ended.take());
        let (waits, busy) = (
            std::mem::take(&mut self.waits),
            std::mem::take(&mut self.busy),
        );
        let Some(key) = key else {
            lines.extend(gathered.into_iter().filter_map(|item| match item {
                Gathered::Said(text) => Some(Line::Said(text)),
                Gathered::Step(..) | Gathered::Helper(..) | Gathered::Began(_) => None,
            }));
            return;
        };
        let mut items: Vec<TraceItem> = (together(gathered).into_iter())
            .map(|item| match item {
                Together::Steps(steps) => TraceItem::Step(trace_step(&steps)),
                Together::Said(text) => TraceItem::Said(text),
                // A helper's one line is the step it's on, where it started or in the place of
                // its first. Its page is that step's, the one the step opened, or before it has
                // a step, the one it opened or named.
                Together::Helper { helper, key, on } => {
                    let all = self.helpers;
                    let page = |key: usize| all.and_then(|all| all.opened_at(key));
                    TraceItem::Helper {
                        step: match on {
                            Some(on) => helper_step(key, on),
                            None => helper_began(key, all.is_some_and(|all| !all.ended(helper))),
                        },
                        name: all.map_or_else(SharedString::default, |all| all.name(helper)),
                        tab: match on {
                            Some((at, step)) => step.did.tab.or_else(|| page(at)),
                            None => all.and_then(|all| all.page(helper)),
                        },
                    }
                }
            })
            .collect();
        // A stretch that's over has nothing under way, a helper's line neither: what its
        // assistant does with helpers once a turn ends or is interrupted isn't ilo's to show.
        if !running {
            for item in &mut items {
                if let TraceItem::Step(step) | TraceItem::Helper { step, .. } = item {
                    step.running = false;
                    if let Opens::Steps(steps) = &mut step.opens {
                        steps.iter_mut().for_each(|step| step.running = false);
                    }
                }
            }
        }
        let took = (!running).then(|| {
            let started = started.unwrap_or_else(Instant::now);
            let ended = ended.into_iter().chain(end).max().unwrap_or(started);
            (worked(started, ended, &waits, &busy).as_secs_f32().round() as u64).max(1)
        });
        lines.push(Line::Trace(Trace {
            key,
            running,
            took,
            items,
        }));
    }
}

/// A line of a stretch as it gathers: its steps, or what the agent said.
enum Together<'a> {
    Steps(Vec<(usize, &'a Traced)>),
    Said(SharedString),
    /// A helper as one line (#4102): which helper, the line's key, which is its own from where
    /// it started and else its first step's place, and the step it's on, if it has one yet.
    Helper {
        helper: usize,
        key: usize,
        on: Option<(usize, &'a Traced)>,
    },
}

/// A stretch's lines, in which a turn's same step on several tabs is one (#1476), as an agent
/// sends a call a tab at once, or one after another. A step joins the last line of its tool and
/// target since the agent last said or thought something, if that line has nothing on the step's
/// tab and nothing came on its tab since that line began, so each tab's steps still read in their
/// order; a page it opens joins the line right before it.
fn together(gathered: Vec<Gathered<'_>>) -> Vec<Together<'_>> {
    let mut lines: Vec<Together> = Vec::new();
    // The first line a step may join: after what the agent last said or thought.
    let mut from = 0;
    for item in gathered {
        let (key, step) = match item {
            Gathered::Step(key, step) => (key, step),
            Gathered::Said(text) => {
                lines.push(Together::Said(text));
                from = lines.len();
                continue;
            }
            Gathered::Helper(helper, at, step) => {
                let line = lines.iter_mut().find_map(|line| match line {
                    Together::Helper {
                        helper: each, on, ..
                    } if *each == helper => Some(on),
                    _ => None,
                });
                match line {
                    Some(on) => *on = Some((at, step)),
                    None => lines.push(Together::Helper {
                        helper,
                        key: at,
                        on: Some((at, step)),
                    }),
                }
                continue;
            }
            Gathered::Began(helper) => {
                let has = |line: &Together| matches!(line, Together::Helper { helper: each, .. } if *each == helper);
                if !lines.iter().any(has) {
                    lines.push(Together::Helper {
                        helper,
                        key: helpers::BEGAN + helper,
                        on: None,
                    });
                }
                continue;
            }
        };
        let did = &step.did;
        let groups = (did.tab.is_some() || did.tool == "open_tab")
            && !matches!(did.tool.as_str(), "thought" | "ask" | "list_tabs");
        // Closes join only those their pages answered alike.
        let like = |line: &Together| {
            matches!(line, Together::Steps(steps)
                if steps[0].1.did.tool == did.tool && steps[0].1.did.target == did.target
                    && steps[0].1.close == step.close)
        };
        let on_tab = |line: &Together| matches!(line, Together::Steps(steps) if steps.iter().any(|(_, each)| each.did.tab == did.tab));
        let joins = (lines.len() > from && groups)
            .then(|| (from..lines.len()).rev().find(|&at| like(&lines[at])))
            .flatten()
            .filter(|&at| match did.tab {
                None => at + 1 == lines.len(),
                Some(_) => !lines[at..].iter().any(on_tab),
            });
        match joins.map(|at| &mut lines[at]) {
            Some(Together::Steps(steps)) => steps.push((key, step)),
            _ => {
                lines.push(Together::Steps(vec![(key, step)]));
                if did.tool == "thought" {
                    from = lines.len();
                }
            }
        }
    }
    lines
}

/// A helper's line: its step `on`, at `key`. A page it handed the user is handed over from the
/// moment it asks, with nothing under way on it but the user's own step, though its call waits.
fn helper_step(key: usize, on: (usize, &Traced)) -> TraceStep {
    let line = TraceStep {
        key,
        ..trace_step(&[on])
    };
    let step = on.1;
    match step.did.tool == "hand_off" && step.ended.is_none() {
        true => TraceStep {
            title: tools::step_title(&step.did, tools::Tense::Done).into(),
            running: false,
            ..line
        },
        false => line,
    }
}

/// The line of a helper that has started and has no step yet, `running` until its CLI says it
/// ended. It says nothing of its own: its task's name for it stands on it.
fn helper_began(key: usize, running: bool) -> TraceStep {
    TraceStep {
        key,
        glyph: tools::step_glyph(""),
        title: SharedString::default(),
        description: SharedString::default(),
        running,
        failed: false,
        opens: Opens::Nothing,
        under: SharedString::default(),
    }
}

/// A line of a trace: a step, or a turn's same step on several tabs, which opens to each tab's.
fn trace_step(steps: &[(usize, &Traced)]) -> TraceStep {
    let &[(key, step)] = steps else {
        return trace_group(steps);
    };
    let took = step
        .ended
        .map(|ended| ended.saturating_duration_since(step.started));
    // A page that asked to stay answered the agent: its tab is open, and nothing failed.
    let close = step.close.filter(|_| step.ended.is_some());
    if let Some(close) = close {
        return TraceStep {
            key,
            glyph: tools::step_glyph(&step.did.tool),
            title: tools::close_title(&step.did, close).into(),
            description: "".into(),
            running: false,
            failed: false,
            opens: Opens::Nothing,
            under: SharedString::default(),
        };
    }
    let description = match (&step.error, took) {
        (Some(_), _) => tools::reason(step.kind),
        // A wait's title says its time.
        (None, Some(took)) if took >= Duration::from_secs(1) && step.did.tool != "wait" => {
            duration(took.as_secs())
        }
        _ => String::new(),
    };
    let text = (!step.did.text.is_empty()).then(|| step.did.text.clone());
    let opened = matches!(step.did.tool.as_str(), "open_tab" | "navigate");
    let opens = match (&step.error, &step.picture, text) {
        (Some(error), _, Some(address)) if opened => {
            Opens::Text(format!("{address}\n{error}").into())
        }
        (Some(error), _, _) => Opens::Text(error.clone()),
        (None, Some(picture), _) => Opens::Picture(picture.clone()),
        (None, None, Some(text)) if opened || step.did.tool == "thought" => {
            Opens::Text(text.into())
        }
        _ => Opens::Nothing,
    };
    TraceStep {
        key,
        glyph: tools::step_glyph(&step.did.tool),
        title: tools::step_title(&step.did, tense(step)).into(),
        description: description.into(),
        running: step.ended.is_none(),
        failed: step.error.is_some(),
        opens,
        // The app's own words for the app's own note: nothing of the answer's text is read.
        under: (step
            .notes
            .and_then(|name| notes::NOTES.iter().find(|note| note.name == name)))
        .map(|note| note.with().into())
        .unwrap_or_default(),
    }
}

/// A turn's same step on several tabs, as one line with how many, which opens to each tab's line
/// by its address. While it works, it says how many are done; once done, how many failed, or what
/// it took.
fn trace_group(steps: &[(usize, &Traced)]) -> TraceStep {
    let (key, first) = steps[0];
    let tabs: Vec<TraceStep> = (steps.iter())
        .map(|&(key, step)| {
            let line = trace_step(&[(key, step)]);
            match step.did.page.is_empty() {
                true => line,
                false => TraceStep {
                    title: step.did.page.clone().into(),
                    ..line
                },
            }
        })
        .collect();
    let count = tabs.len();
    let running = tabs.iter().filter(|tab| tab.running).count();
    // Closes their pages answered alike, as `together` joins them.
    if let (0, Some(close)) = (running, first.close) {
        let did = tools::Did {
            site: format!("{count} tabs"),
            ..first.did.clone()
        };
        return TraceStep {
            key,
            glyph: tools::step_glyph(&did.tool),
            title: tools::close_title(&did, close).into(),
            description: "".into(),
            running: false,
            failed: false,
            opens: Opens::Steps(tabs),
            under: SharedString::default(),
        };
    }
    let failed = tabs.iter().filter(|tab| tab.failed).count();
    let tense = match (running, failed) {
        (1.., _) => tools::Tense::Now,
        (0, failed) if failed == count => tools::Tense::Failed,
        _ => tools::Tense::Done,
    };
    // What it typed, if it typed the same on each.
    let mut did = first.did.clone();
    if steps.iter().any(|(_, step)| step.did.text != did.text) {
        did.text.clear();
    }
    let started = steps
        .iter()
        .map(|(_, step)| step.started)
        .min()
        .unwrap_or(first.started);
    let ended = steps
        .iter()
        .map(|(_, step)| step.ended)
        .collect::<Option<Vec<_>>>();
    let took =
        (ended.into_iter().flatten().max()).map(|ended| ended.saturating_duration_since(started));
    let description = match (running, failed) {
        (1.., _) => format!("{} of {count}", count - running),
        // Failed on each, it says why if it's the same why.
        (0, failed) if failed == count => {
            let why = &tabs[0].description;
            match tabs.iter().all(|tab| tab.description == *why) {
                true => why.to_string(),
                false => String::new(),
            }
        }
        (0, 1..) => format!("{failed} failed"),
        _ => match took {
            Some(took) if took >= Duration::from_secs(1) && did.tool != "wait" => {
                duration(took.as_secs())
            }
            _ => String::new(),
        },
    };
    TraceStep {
        key,
        glyph: tools::step_glyph(&did.tool),
        title: tools::group_title(&did, count, tense).into(),
        description: description.into(),
        running: running > 0,
        failed: failed > 0,
        opens: Opens::Steps(tabs),
        under: SharedString::default(),
    }
}

/// A screenshot's picture, from its JPEG in base64.
fn picture(base64: &str) -> Option<Arc<gpui::Image>> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64)
        .ok()?;
    Some(Arc::new(gpui::Image::from_bytes(
        gpui::ImageFormat::Jpeg,
        bytes,
    )))
}

/// When a task was done, "Done · 10:42", as its card and its conversation say it.
fn done_at(time: &str) -> String {
    format!("Done · {time}")
}

/// A session's receipt as it ends: what it did that had consequences, the orders and messages the
/// user allowed it, once done, "Done · 1 order placed · 2 messages sent". Its pages are Open's
/// button's to count. With none of them, "Done · 13:28", when its work stopped.
fn receipt(session: &Session, ended: &str) -> String {
    let count = |n: usize, one: &str, many: &str| match n {
        0 => None,
        1 => Some(format!("1 {one}")),
        n => Some(format!("{n} {many}")),
    };
    let did: Vec<String> = [
        count(session.orders, "order placed", "orders placed"),
        count(session.messages, "message sent", "messages sent"),
    ]
    .into_iter()
    .flatten()
    .collect();
    match did.is_empty() {
        true if ended.is_empty() => "Done".to_string(),
        true => done_at(ended),
        false => format!("Done · {}", did.join(" · ")),
    }
}

/// Settings › Agents' row under each assistant's: what its new tasks start with. Its name carries
/// it, so it has no line.
pub const MODEL: &str = "Model";
/// Model's first choice, `{assistant}` its name: the assistant's own default, passing it no model.
const MODEL_DEFAULT: &str = "{assistant}'s default";

/// `cli`'s Model's first choice, as its pop-up and menu say it.
pub fn model_default(cli: Cli) -> String {
    MODEL_DEFAULT.replace("{assistant}", cli.assistant())
}

/// A CLI's Model menu, as its ids and items, with `chosen` checked: its own default first, as
/// `first` says it, followed by the name of the model it is where the CLI says, then `models`.
pub fn model_menu(
    chosen: &str,
    first: &str,
    default: Option<&str>,
    models: Vec<(String, String)>,
) -> (Vec<(String, String)>, Vec<MenuEntry>) {
    let default = default.map(|name| SharedString::from(name.to_string()));
    let mut all = vec![(String::new(), first.to_string())];
    all.extend(models);
    let items = (all.iter().enumerate())
        .map(|(at, (id, name))| {
            let on = id == chosen;
            MenuEntry {
                checked: on,
                over: on,
                detail: default.clone().filter(|_| at == 0),
                ..MenuEntry::named(name.clone(), true)
            }
        })
        .collect();
    (all, items)
}

/// A name as it reads: compatibility forms as the letters they show, each letter as the one it
/// passes for (UTS #39's skeleton, which drops what doesn't show), in lower case and without
/// accents, and i, l and 1 as one letter, as a capital I and a small l draw alike; then words of
/// letters and digits split by one space. Two names that read alike read the same here. Only for
/// comparing: a name is shown as given.
fn reads_as(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};
    let folded = name.nfkc().collect::<String>().to_lowercase();
    let read = unicode_security::skeleton(&folded).collect::<String>();
    let read = (read.to_lowercase().chars())
        .filter(|c| c.general_category_group() != GeneralCategoryGroup::Mark)
        .map(|c| if matches!(c, 'l' | '1') { 'i' } else { c })
        .collect::<String>();
    let words = read.split(|c: char| !c.is_alphanumeric());
    words
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a given name could pass for a client's own, so a card saying "Claude Code" is always
/// Claude Code's connection: one that reads as another client's or holds an app's name, or with a
/// word mixing scripts, which a real name doesn't and a lookalike does.
fn names_a_client(name: &str) -> bool {
    use unicode_normalization::UnicodeNormalization;
    use unicode_security::MixedScript;
    let given = reads_as(name).replace(' ', "");
    let folded = name.nfkc().collect::<String>();
    (folded.split(|c: char| !c.is_alphanumeric())).any(|word| !word.is_single_script())
        || (Client::ALL.iter()).any(|&client| {
            let own = reads_as(client.name()).replace(' ', "");
            let app = matches!(client, Client::ClaudeCode | Client::Codex);
            given == own || app && given.contains(&own)
        })
}

/// Whether a client's name would pass for the browser's own words: one with the app's name as a
/// word, or that calls itself the browser. "Browser Use" is an agent's.
fn names_the_browser(name: &str) -> bool {
    let read = reads_as(name);
    let app = reads_as(env!("APP_NAME"));
    read.split(' ').any(|word| word == app)
        || ["the browser", "your browser", "this browser", "web browser"]
            .iter()
            .any(|browser| read.contains(&reads_as(browser)))
        || read == reads_as("browser")
}

/// The files of the tasks a run before left, but for the places walled tasks left, which are swept
/// first (leftovers.rs). They're listed here, before any task can make one, and swept off the UI
/// thread, as a sweep walks every process.
fn end_old_task_files(tasks: &std::path::Path) {
    use super::wall::leftovers;
    let root = tasks.join(leftovers::ROOT);
    let left = leftovers::left_in(&root);
    if left.is_empty() {
        _ = std::fs::remove_dir_all(tasks);
        return;
    }
    for entry in std::fs::read_dir(tasks).into_iter().flatten().flatten() {
        if entry.file_name() == leftovers::ROOT {
            continue;
        }
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => _ = std::fs::remove_dir_all(entry.path()),
            Ok(_) => _ = std::fs::remove_file(entry.path()),
            Err(_) => {}
        }
    }
    _ = std::thread::Builder::new()
        .name("task-leftovers".into())
        .spawn(move || {
            let cleaned = leftovers::clean_up(&root, &left);
            log!(
                "browser: walled tasks a run before left: {} ended, {} places removed, {} kept",
                cleaned.ended,
                cleaned.removed,
                cleaned.kept
            );
        });
}
