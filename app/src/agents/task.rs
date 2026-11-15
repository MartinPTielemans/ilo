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
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use web_time::{Instant, SystemTime};

use futures::StreamExt;
use gpui::{App, AppContext, Context, SharedString};
use serde_json::Value;

use super::ready::{Found, Ready};
use super::runner::{self, Cli, Failure, Launch, Out, Process};
use super::{Agents, Client, Question, ToolResult, page, site, tools};
use crate::shell::MenuEntry;

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
    /// It couldn't go on: why, and the command that fixes it, if one does.
    Failed {
        why: SharedString,
        command: Option<&'static str>,
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
    pub(super) fn new(who: Who, text: impl Into<SharedString>) -> Self {
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
    /// How far it has got while it works, after where it is, from its own steps: "step 5 · 4
    /// pages", or empty.
    pub progress: SharedString,
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
    /// Open under the result, which moves the tabs it left the user to them: "Open maat.pt", or
    /// "Open 2 pages".
    pub open: Option<SharedString>,
    /// The answers to the question it waits on, as chips under it.
    pub answers: Vec<SharedString>,
    /// Whether its last line is the message the agent is writing, which comes in as it's written.
    pub writing: bool,
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
}

/// A stretch of the agent's work between two messages: a summary line, "Working · 5 steps" or
/// "Worked for 41s · 8 steps", with its steps under it.
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
    pub fn steps(&self) -> usize {
        (self.items.iter())
            .filter(|item| matches!(item, TraceItem::Step(_)))
            .count()
    }

    /// Its summary line's title and description.
    pub fn summary(&self) -> (SharedString, SharedString) {
        let steps = match self.steps() {
            1 => "1 step".to_string(),
            count => format!("{count} steps"),
        };
        let title = match self.took {
            Some(seconds) => format!("Worked for {}", duration(seconds)),
            None => "Working".to_string(),
        };
        (title.into(), steps.into())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TraceItem {
    Step(TraceStep),
    /// What the agent said as it worked, which isn't its turn's last message.
    Said(SharedString),
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
    pub picture: Option<Arc<gpui::Image>>,
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
    /// The name the agent gives it (title), else what was asked.
    pub title: SharedString,
    /// The model it runs on, by name, or empty for its CLI's own default.
    pub model: SharedString,
    pub state: State,
    /// When it finished, done, stopped or failed, for its conversation and the Archive.
    pub ended: Option<SystemTime>,
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
    /// When it last left the foot for the Archive, which lists it by then. Kept in memory only.
    pub left: Option<SystemTime>,
    /// Deleted from the Archive: listed nowhere and out of ⇧⌘T's reach, until ⌘Z brings it back
    /// or it goes for good with the dismissed tasks past the last few.
    pub deleted: bool,
    token: String,
    process: Option<Process>,
    /// The ask's call, which waits for the user's answer, with the number it takes among the
    /// agent's prompts, which show in the order they came.
    pub(super) asking: Option<(u64, mpsc::Sender<ToolResult>)>,
    /// While it's thinking, what ends the turn if its CLI never answers (NO_ANSWER). It goes as
    /// the CLI answers, so nothing waits once the agent is under way.
    pub(super) waiting: Option<gpui::Task<()>>,
    /// The message the agent is writing at the end of the turn under way, as it comes.
    pub(super) writing: Option<Writing>,
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

/// An outside agent's session: a client the user connected at work, on any of its connections,
/// from its first call that opens or acts on a tab until its last connection closes, the user
/// stops it, or IDLE passes. Its connections share its client's Space, rules and card.
pub struct Session {
    /// The agent's id, which its calls, its rules, its Space and its activity go by.
    pub agent: String,
    /// Its pages: the tabs it opened or acted in, in that order.
    pub tabs: Vec<u64>,
    /// The orders it placed and the messages it sent, each once the user allowed it and it was
    /// done, for its receipt. For this run only, as the session is.
    pub(super) orders: usize,
    pub(super) messages: usize,
    /// Its calls under way, which it waits on.
    pub(super) calls: Vec<u64>,
    /// What ends it once it has waited IDLE for its next call.
    idle: Option<gpui::Task<()>>,
}

impl Task {
    pub(super) fn token(&self) -> &str {
        &self.token
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
    /// The step it's on, such as "Opening gulbenkian.pt", what it waits for, or its result's line.
    pub status: SharedString,
    /// Whether the status is the agent's question, which may take two lines.
    pub asks: bool,
    pub dot: Dot,
    /// An ask's answers, as buttons.
    pub answers: Vec<SharedString>,
    /// A confirmation or a handoff of the task's, which the card asks instead of the windows'.
    pub question: Option<Question>,
    /// Done's tabs to open, each with its site, in the agent's order.
    pub open: Vec<(u64, SharedString)>,
    /// Limit's Retry.
    pub retry: bool,
    /// The command that fixes a failure.
    pub command: Option<&'static str>,
    /// The task's ask to point, whose chips are Not Now and Point until the user points.
    pub point: Option<super::TaskPoint>,
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

/// What the user does to a task from its card or its conversation.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskAction {
    Dismiss,
    /// ⇧⌘T or ⌘Z: the task named, or with none named, the last dismissed.
    Restore,
    Answer(SharedString),
    Reply(SharedString),
    Retry,
    Stop,
    /// Done's Open <site> or Open 3 pages: every tab it left the user.
    Open,
    /// A page's own Open <site>, from the strip under it.
    OpenTab(u64),
    /// The Archive's Delete from Archive, and ⌘Z's undoing of it.
    Delete,
    Undelete,
}

/// A task the user dismissed, as the Library's Archived Tabs lists it among the tabs.
#[derive(Clone, Debug, PartialEq)]
pub struct ArchivedTask {
    pub agent: SharedString,
    pub title: SharedString,
    pub asked: SharedString,
    /// Done's tick, or Stopped's and Failed's grey dot, as its card had.
    pub dot: Dot,
    /// Who worked on it, how it ended and when, and the pages it still holds:
    /// "Claude · Done 13:31 · 1 page".
    pub about: SharedString,
    /// When it left the foot, in milliseconds since the Unix epoch, as the archive dates its tabs.
    pub left: f64,
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
    /// The dismissed, newest last, for ⇧⌘T.
    pub(super) dismissed: VecDeque<String>,
    pub(super) next: u64,
    /// The name each outside agent last gave, by its id, for its session's card.
    pub(super) names: HashMap<String, SharedString>,
    pub found: HashMap<Cli, Found>,
    pub(super) looking: bool,
    /// Whether ⌘T's ask has shown in this run, which learns again once.
    pub(super) asked_once: bool,
    /// The default agent, from Settings › Agents.
    pub default: Option<Cli>,
    /// Each CLI's model, or none for its own default.
    pub models: HashMap<Cli, String>,
    /// When each CLI's limit resets, as its last task learned.
    pub(super) limits: HashMap<Cli, Option<SharedString>>,
    pub(super) events: Option<(
        futures::channel::mpsc::UnboundedSender<runner::Event>,
        gpui::Task<()>,
    )>,
    /// Where tasks run: a directory of their own each, under the profile's.
    pub dir: Option<PathBuf>,
}

impl Agents {
    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.tasks.iter()
    }

    pub fn task(&self, agent: &str) -> Option<&Task> {
        self.tasks.tasks.iter().find(|task| task.agent == agent)
    }

    pub(super) fn task_mut(&mut self, agent: &str) -> Option<&mut Task> {
        self.tasks.tasks.iter_mut().find(|task| task.agent == agent)
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
                let (words, dot) = match task.state {
                    State::Done { .. } => ("Done", Dot::Done),
                    State::Failed { .. } => ("Failed", Dot::Ended),
                    _ => ("Stopped", Dot::Ended),
                };
                let mut about = format!("{} · {words}", task.name);
                if let Some(at) = task.ended.or(task.left) {
                    about = format!("{about} {}", super::local_time(seconds(at)));
                }
                match self.open_tabs(task, cx).len() {
                    0 => {}
                    1 => about.push_str(" · 1 page"),
                    pages => about.push_str(&format!(" · {pages} pages")),
                }
                let left = task
                    .left
                    .and_then(|at| at.duration_since(SystemTime::UNIX_EPOCH).ok());
                ArchivedTask {
                    agent: task.agent.clone().into(),
                    title: task.title.clone(),
                    asked: task.asked.clone(),
                    dot,
                    about: about.into(),
                    left: left.map_or(0., |since| since.as_secs_f64() * 1000.),
                }
            })
            .collect()
    }

    /// Does what the user did to `agent`'s task. Returns the tab Open moved into the user's
    /// Space, for the window to show, and the task Restore brought back.
    pub fn task_action(
        &mut self,
        agent: &str,
        action: TaskAction,
        cx: &mut Context<Self>,
    ) -> (Option<u64>, Option<String>) {
        // A session's user talks to its agent where they started it.
        let session = self.task(agent).is_some_and(|task| task.session.is_some());
        if session
            && matches!(
                action,
                TaskAction::Answer(_) | TaskAction::Reply(_) | TaskAction::Retry
            )
        {
            return (None, None);
        }
        // With tasks off nothing more is asked of a task; an answer still lets one finish.
        if !self.tasks_on() && matches!(action, TaskAction::Reply(_) | TaskAction::Retry) {
            return (None, None);
        }
        match action {
            TaskAction::Dismiss => self.dismiss_task(agent, cx),
            TaskAction::Restore => {
                let named = Some(agent).filter(|agent| !agent.is_empty());
                return (None, self.restore_task(named, cx));
            }
            TaskAction::Answer(answer) => self.answer_task(agent, &answer, cx),
            TaskAction::Reply(text) => self.reply(agent, &text, cx),
            TaskAction::Retry => self.retry_task(agent, cx),
            TaskAction::Stop => self.stop_task(agent, cx),
            TaskAction::Open => return (self.open_result(agent, None, cx), None),
            TaskAction::OpenTab(tab) => return (self.open_result(agent, Some(tab), cx), None),
            TaskAction::Delete => self.delete_task(agent, true, cx),
            TaskAction::Undelete => self.delete_task(agent, false, cx),
        }
        (None, None)
    }

    /// `agent`'s task's conversation: what was asked, then each turn with what the agent said
    /// as it worked folded into its steps, so only the user's messages, the agent's questions and
    /// each turn's last message show.
    pub fn conversation(&self, agent: &str, cx: &App) -> Option<Conversation> {
        let task = self.task(agent)?;
        let ended = !matches!(task.state, State::Working | State::Asking { .. });
        let working = task.state == State::Working;
        let mut lines = fold(
            &task.messages,
            &task.trace,
            ended,
            working,
            task.writing.as_ref(),
        );
        // A session's conversation is in its agent's terminal: here, only its steps, one by one.
        if task.session.is_some() {
            lines = steps(lines);
        }
        // A turn the CLI hasn't answered says it's thinking from the moment it's sent, and if the
        // CLI goes or never answers, why, so nothing is left running.
        if task.heard_nothing() {
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
        let tabs: Vec<(u64, SharedString)> = (self.pages(task, cx).into_iter())
            .map(|tab| (tab, site(&self.browser.tab(tab, cx).url).into()))
            .collect();
        let progress = match task.state == State::Working && !self.waits_on_user(task) {
            true => progress(turn_steps(task).len(), tabs.len()),
            false => String::new(),
        };
        let about = match &task.state {
            State::Working if self.waits_on_user(task) => "Waiting for you".to_string(),
            State::Working => "Working".to_string(),
            State::Asking { .. } => "Waiting for you".to_string(),
            State::Done { .. } => match task.ended {
                Some(at) => format!("Done {}", super::local_time(seconds(at))),
                None => "Done".to_string(),
            },
            State::Limit { resets } => limit_words(&task.name, resets),
            State::Stopped | State::Failed { .. } => "Stopped".to_string(),
        };
        // A session's title is its agent's name, so what's under it is how it's doing alone.
        let about = match (&task.session, task.model.is_empty()) {
            (Some(_), _) => about,
            (None, true) => format!("{} · {about}", task.name),
            (None, false) => format!("{} · {} · {about}", task.name, task.model),
        };
        Some(Conversation {
            agent: task.agent.clone().into(),
            name: task.name.clone(),
            title: task.title.clone(),
            about: about.into(),
            progress: progress.into(),
            asked: task.asked.clone(),
            page: task.page.clone(),
            lines,
            tabs,
            on: (task.state == State::Working)
                .then(|| self.on(task))
                .flatten(),
            stoppable: matches!(task.state, State::Working | State::Asking { .. }),
            open: open_label(&self.open_tabs(task, cx)),
            answers: match &task.state {
                State::Asking { answers, .. } if self.ask_shown(task) => answers.clone(),
                _ => Vec::new(),
            },
            writing: working && task.writing.as_ref().is_some_and(|writing| !writing.said),
        })
    }

    /// Whether a confirmation or a handoff of the task's waits on the user.
    fn waits_on_user(&self, task: &Task) -> bool {
        self.task_question(task).is_some()
    }

    fn task_question(&self, task: &Task) -> Option<Question> {
        let (from, first) = match &task.session {
            Some(session) => (session.agent.as_str(), self.session_prompt(session)?),
            None => (task.agent.as_str(), self.first_prompt(&task.agent)?),
        };
        (self.asking.iter().map(|asking| &asking.question))
            .chain(
                self.question()
                    .as_ref()
                    .filter(|question| question.handoff.is_some()),
            )
            .find(|question| question.from == from && question.id == first)
            .cloned()
    }

    /// The number of the first of the prompts a session's calls under way wait on: its client's
    /// other connections' calls wait on theirs.
    fn session_prompt(&self, session: &Session) -> Option<u64> {
        let ours = |agent: &str, call: u64| agent == session.agent && session.calls.contains(&call);
        let confirmations = (self.asking.iter())
            .filter(|asking| ours(&asking.act.agent, asking.act.call))
            .map(|asking| asking.question.id);
        let handoffs = (self.held.values())
            .filter(|held| held.reply.is_some() && ours(&held.agent, held.call))
            .map(|held| held.question);
        confirmations.chain(handoffs).min()
    }

    /// The tab a task is on while it works: its agent's last, or a session's last step's, as its
    /// client's other connections have their own.
    fn on(&self, task: &Task) -> Option<u64> {
        match &task.session {
            Some(_) => task.trace.iter().rev().find_map(|step| step.did.tab),
            None => self.current.get(&task.agent).copied(),
        }
    }

    /// A task's pages: its Space's tabs, or of its client's Space, a session's own.
    fn pages(&self, task: &Task, cx: &App) -> Vec<u64> {
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
        let confirmations = (self.asking.iter())
            .filter(|asking| asking.act.agent == agent)
            .map(|asking| asking.question.id);
        let handoffs = (self.held.values())
            .filter(|held| held.agent == agent && held.reply.is_some())
            .map(|held| held.question);
        let ask = self
            .task(agent)
            .and_then(|task| task.asking.as_ref())
            .map(|(id, _)| *id);
        (confirmations.chain(handoffs).chain(ask))
            .chain(self.point_ids(agent))
            .min()
    }

    /// Whether the task's ask, or its ask to point, is the prompt its agent shows.
    fn ask_shown(&self, task: &Task) -> bool {
        let first = self.first_prompt(&task.agent);
        (task.asking.as_ref()).is_some_and(|(id, _)| first == Some(*id))
            || self
                .task_point(&task.agent)
                .is_some_and(|point| first == Some(point.id))
    }

    /// Done's tabs to open, with their sites, those still in the task's Space.
    fn open_tabs(&self, task: &Task, cx: &App) -> Vec<(u64, SharedString)> {
        let State::Done { tabs, .. } = &task.state else {
            return Vec::new();
        };
        let own = self.browser.space_tab_ids(task.space, cx);
        (tabs.iter().filter(|tab| own.contains(tab)))
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
        // The core's auto-archive can take a task's page it never showed (window.cc), and a task
        // lives for one run, so its card says where they went rather than offer them.
        let status = match &task.state {
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
            title: task.title.clone(),
            status,
            asks,
            dot,
            answers: match &task.state {
                State::Asking { answers, .. } if asks => answers.clone(),
                _ => Vec::new(),
            },
            question,
            open,
            retry: matches!(task.state, State::Limit { .. }) && self.tasks_on(),
            command: match task.state {
                State::Failed { command, .. } => command,
                _ => None,
            },
            point: self.task_point(&task.agent).filter(|_| asks),
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
                cx.notify();
            });
        })
        .detach();
    }

    /// The CLI ⌘T's ask lists first.
    pub fn default_task_agent(&self) -> Option<Cli> {
        self.tasks.default
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

    /// Run tasks on, in Settings › Agents.
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

    pub fn set_task_model(&mut self, cli: Cli, model: &str, cx: &mut Context<Self>) {
        match model.is_empty() {
            true => self.tasks.models.remove(&cli),
            false => self.tasks.models.insert(cli, model.to_string()),
        };
        self.save();
        cx.notify();
    }

    /// Starts a task: `cli` does what the user `asked`, on `page` if it goes with it, from the
    /// user's Space `home`. Returns its agent's id, or None if it couldn't start, which its card
    /// says.
    pub fn start_task(
        &mut self,
        cli: Cli,
        asked: &str,
        page: Option<AskedOn>,
        home: u64,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let asked = asked.trim();
        if asked.is_empty() || !self.tasks_on() {
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
        let model = self.task_model_name(cli).unwrap_or_default();
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
            state: State::Working,
            ended: None,
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
        };
        self.tasks.tasks.push(task);
        self.tokens_changed();
        let first = first_message(asked, self.task(&agent).and_then(|task| task.page.as_ref()));
        self.launch(&agent, &first, cx);
        self.wait_for_answer(&agent, cx);
        cx.notify();
        Some(agent)
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

    /// Starts the task's CLI with its first message.
    fn launch(&mut self, agent: &str, first: &str, cx: &mut Context<Self>) {
        let Some((task, Some(cli))) = self.task(agent).map(|task| (task, task.cli)) else {
            return;
        };
        let found = self.tasks.found.get(&cli).cloned().unwrap_or_default();
        let Some(program) = found.program.clone() else {
            return self.fail(
                agent,
                Failure::Other(format!("{} isn't installed.", cli.name())),
                cx,
            );
        };
        let dir = self
            .tasks
            .dir
            .clone()
            .unwrap_or_else(std::env::temp_dir)
            .join("Tasks");
        let mut args = self.bridge.clone();
        args.push(self.socket.to_string_lossy().into_owned());
        let command = args.remove(0);
        let model = Some(self.task_model(cli).to_string()).filter(|model| !model.is_empty());
        let launch = Launch {
            cli,
            program,
            path: found.path.clone(),
            dir: dir.join(agent),
            config: dir.join(format!("{agent}.json")),
            server: runner::Server {
                command,
                args,
                token: task.token.clone(),
            },
            model,
            instructions: tools::TASK_INSTRUCTIONS.into(),
            tools: tools::names(agent),
        };
        let events = self.task_events(cx);
        match Process::start(&launch, agent, first, events) {
            Ok(process) => {
                if let Some(task) = self.task_mut(agent) {
                    task.process = Some(process);
                }
            }
            Err(error) => {
                log!("browser: task {agent} couldn't start {}: {error}", cli.id());
                self.fail(
                    agent,
                    Failure::Other(format!("{} couldn't start.", cli.name())),
                    cx,
                );
            }
        }
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
        let runner::Event { agent, out } = event;
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
                    picture: None,
                });
            }
            Out::Ended(None) => {
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
                    task.ended = Some(SystemTime::now());
                }
            }
            // The turn the user stopped ends as an error; their stop is what the card says.
            Out::Ended(Some(_))
                if self
                    .task(&agent)
                    .is_some_and(|task| task.state == State::Stopped) => {}
            Out::Ended(Some(failure)) => self.fail(&agent, failure, cx),
            Out::Gone(said) => {
                let Some(task) = self.task_mut(&agent) else {
                    return;
                };
                task.process = None;
                if matches!(task.state, State::Working | State::Asking { .. }) {
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
    fn fail(&mut self, agent: &str, failure: Failure, cx: &mut Context<Self>) {
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
                    command: Some(cli.sign_in()),
                }
            }
            Failure::Other(why) if why.contains("isn't installed") => State::Failed {
                why: why.into(),
                command: Some(cli.install()),
            },
            Failure::Other(why) => State::Failed {
                why: page::clip(&why, 200).into(),
                command: None,
            },
        };
        if let State::Limit { resets } = &state {
            self.tasks.limits.insert(cli, resets.clone());
        }
        if let Some(task) = self.task_mut(agent) {
            task.state = state;
            task.ended = Some(SystemTime::now());
            task.waiting = None;
            task.writing = None;
        }
        self.forget_waiting(agent, cx);
        // What's ready may have changed.
        self.look_for_task_agents(cx);
        cx.notify();
    }

    /// The user's reply, which the agent reads once its turn under way ends.
    pub fn reply(&mut self, agent: &str, text: &str, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        // A reply answers the ask waiting, as a button would.
        if self.task(agent).is_some_and(|task| task.asking.is_some()) {
            return self.answer_task(agent, text, cx);
        }
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        task.messages
            .push(Message::new(Who::User, text.to_string()));
        task.state = State::Working;
        task.writing = None;
        let sent = task.process.as_mut().map(|process| process.say(text));
        if !matches!(sent, Some(Ok(()))) {
            // Gone, or never started: a new run, with the conversation so far.
            let first = conversation(task);
            self.launch(agent, &first, cx);
        }
        self.wait_for_answer(agent, cx);
        cx.notify();
    }

    /// The user's answer to the task's ask.
    pub fn answer_task(&mut self, agent: &str, answer: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        let Some((_, reply)) = task.asking.take() else {
            return;
        };
        task.messages
            .push(Message::new(Who::User, answer.to_string()));
        task.state = State::Working;
        task.writing = None;
        _ = reply.send(ToolResult::text(format!("The user answered: {answer}")));
        self.note(agent, "answered", "", "");
        self.wait_for_answer(agent, cx);
        // The agent's next prompt, if one waited its turn.
        self.asked(cx);
    }

    /// ⌘. or Stop: ends what the task is doing at once. A reply goes on from there.
    pub fn stop_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        if !matches!(task.state, State::Working | State::Asking { .. }) {
            return;
        }
        task.state = State::Stopped;
        task.ended = Some(SystemTime::now());
        task.asking = None;
        task.writing = None;
        task.waiting = None;
        if let Some(process) = task.process.as_mut() {
            process.interrupt();
        }
        // A session's agent is its client's, whose calls under way the user stops, on each of its
        // connections, as the agent path always has.
        let driver = match &mut task.session {
            Some(session) => {
                session.idle = None;
                session.agent.clone()
            }
            None => agent.to_string(),
        };
        self.stop_agent(&driver, cx);
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
        task.state = State::Working;
        task.writing = None;
        let sent = task.process.as_mut().map(|process| process.say(RETRY));
        if let Some(cli) = task.cli {
            self.tasks.limits.remove(&cli);
        }
        if !matches!(sent, Some(Ok(()))) {
            let first = conversation(self.task(agent).expect("found"));
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
    }

    /// The task goes: its CLI ends, and its Space too with `space`. A session's tabs close instead,
    /// as its Space is its client's.
    fn end_task(&mut self, agent: &str, space: bool, cx: &mut Context<Self>) {
        let Some(at) = self.tasks.tasks.iter().position(|task| task.agent == agent) else {
            return;
        };
        let task = self.tasks.tasks.remove(at);
        // check.sh reads this line.
        log!("browser: task {agent} dropped");
        // A session's Space is its client's, which keeps the tabs of its other sessions: its own
        // close, but for any a session under way works in.
        if let Some(session) = task.session {
            let pages = self.pages_of(&session, task.space, cx);
            let live = (self.tasks.tasks.iter())
                .filter(|other| other.state == State::Working)
                .filter_map(|other| other.session.as_ref())
                .flat_map(|other| other.tabs.clone())
                .collect::<Vec<_>>();
            for tab in pages.into_iter().filter(|tab| !live.contains(tab)) {
                self.browser.close_user_tab(tab, cx);
            }
            return;
        }
        self.forget_waiting(agent, cx);
        self.doing.remove(agent);
        self.tokens_changed();
        if space && task.space != 0 {
            self.browser.delete_space(task.space, cx);
        }
        self.remove_task_files(agent);
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
    /// and before #1080 a quit of an outside agent's, which the user saw as a Space of theirs, each
    /// give their tabs to the top of the user's Space in its profile, as Open or Keep would, then
    /// go. A crash showed no receipt, so the user chose neither Keep nor Dismiss, and of the two
    /// only Keep can be undone: the tabs close to the archive as any do (#1080).
    pub fn end_old_tasks(&mut self, cx: &mut Context<Self>) {
        let left = (self
            .browser
            .task_spaces()
            .into_iter()
            .map(|space| (space, None)))
        .chain(Client::ALL.into_iter().filter_map(|client| {
            let space = self.browser.agent_space(client.id());
            (space != 0).then_some((space, Some(client)))
        }))
        .collect::<Vec<_>>();
        for (space, client) in left {
            let home = self.browser.user_space(space);
            // Each goes to the top, so the last goes first.
            for tab in self.browser.space_tab_ids(space, cx).into_iter().rev() {
                self.browser.move_tab(tab, home, cx);
            }
            // check.sh reads these lines.
            match client {
                Some(client) => log!(
                    "browser: deleting space {space} of agent {} before",
                    client.id()
                ),
                None => log!("browser: deleting space {space} of a task before"),
            }
            self.browser.delete_space(space, cx);
        }
        if let Some(dir) = &self.tasks.dir {
            _ = std::fs::remove_dir_all(dir.join("Tasks"));
        }
    }

    fn remove_task_files(&self, agent: &str) {
        let Some(dir) = &self.tasks.dir else { return };
        let dir = dir.join("Tasks");
        _ = std::fs::remove_dir_all(dir.join(agent));
        _ = std::fs::remove_file(dir.join(format!("{agent}.json")));
        _ = std::fs::remove_file(dir.join(format!("{agent}.md")));
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
        let did = self.did(&call.agent, tool, &call.arguments, None, cx);
        self.trace(&key, call.id, did, call.at);
    }

    /// A step of the run `call` on `tab`, starting now, which ends the step before it.
    pub(super) fn trace_step(&mut self, agent: &str, call: u64, tab: u64, step: &Value, cx: &App) {
        let Some(key) = self.tracer(agent, call) else {
            return;
        };
        let tool = step
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let did = self.did(agent, tool, step, Some(tab), cx);
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
        self.trace(&key, call, did, now);
    }

    fn trace(&mut self, key: &str, call: u64, did: tools::Did, at: Instant) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        // A session's pages are the tabs it works in.
        if let (Some(session), Some(tab)) = (&mut task.session, did.tab)
            && !session.tabs.contains(&tab)
        {
            session.tabs.push(tab);
        }
        // An ask is done once asked: the wait for the answer is the user's.
        let ended = matches!(did.tool.as_str(), "ask" | "point").then_some(at);
        let after = task.messages.len();
        task.trace.push(Traced {
            call,
            did,
            after,
            started: at,
            ended,
            error: None,
            picture: None,
        });
        // Its first step: it's under way, and what it wrote before it was said along the way.
        task.waiting = None;
        task.writing = None;
    }

    /// The card whose trace `agent`'s `call` goes in: its task's, or the session it came in.
    fn tracer(&self, agent: &str, call: u64) -> Option<String> {
        let task = self.task(agent).or_else(|| self.session_of(agent, call))?;
        Some(task.agent.clone())
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
        image: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.tracer(agent, call) else {
            return;
        };
        let Some(task) = self.task_mut(&key) else {
            return;
        };
        if let Some(session) = &mut task.session {
            session.calls.retain(|each| *each != call);
        }
        let idle = (task.session.as_ref())
            .is_some_and(|session| session.calls.is_empty() && task.state == State::Working);
        if idle {
            self.idle_from_now(&key, cx);
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
        // An error's first paragraph: a wait's goes on with the page's outline.
        step.error = error.map(|error| {
            page::clip(error.split("\n\n").next().unwrap_or_default().trim(), 300).into()
        });
        step.picture = image.and_then(|image| picture(&image));
        cx.notify();
    }

    /// A client said hello: the name its sessions' cards show. One the user connected in
    /// Settings › Agents by name has that; any other MCP client, the name it gives.
    pub(super) fn hello(&mut self, agent: &str, client: &str) {
        let name = match Client::from_id(agent) {
            Some(Client::Other) if !client.is_empty() => client.to_string(),
            Some(known) => known.name().to_string(),
            None => return,
        };
        self.tasks
            .names
            .insert(agent.into(), page::clip(&name, 60).into());
    }

    /// The session under way of the call's client, on whichever of its connections, or, for a call
    /// that opens or acts on a tab, a new one: its card shows from then. Its calls are its steps.
    fn session_for(&mut self, call: &super::Call, cx: &mut Context<Self>) -> Option<String> {
        // Only a client the user connected has sessions.
        Client::from_id(&call.agent)?;
        let live = (self.tasks.tasks.iter_mut()).find(|task| {
            task.state == State::Working
                && (task.session.as_ref()).is_some_and(|session| session.agent == call.agent)
        });
        if let Some(task) = live {
            let session = task.session.as_mut().expect("a session, as found");
            session.calls.push(call.id);
            session.idle = None;
            return Some(task.agent.clone());
        }
        if call.tool == "list_tabs" {
            return None;
        }
        self.tasks.next += 1;
        let key = format!("session-{}", self.tasks.next);
        let name = self.agent_name(&call.agent);
        let space = self.space(&call.agent, cx);
        // check.sh reads this line.
        log!("browser: session {key} of agent {} as {name}", call.agent);
        let session = Session {
            agent: call.agent.clone(),
            tabs: Vec::new(),
            orders: 0,
            messages: 0,
            calls: vec![call.id],
            idle: None,
        };
        self.tasks.tasks.push(Task {
            agent: key.clone(),
            cli: None,
            title: name.clone(),
            name,
            session: Some(session),
            home: 0,
            space,
            asked: SharedString::default(),
            page: None,
            model: SharedString::default(),
            state: State::Working,
            ended: None,
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
        });
        cx.notify();
        Some(key)
    }

    /// The agent's call `call` opened `tab`, which is a page of its session's.
    pub(super) fn opened(&mut self, agent: &str, call: u64, tab: u64) {
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
            let (logged_as, answers) = match (&task.session, task.cli) {
                (Some(session), _) => (session.agent.clone(), session.agent.clone()),
                (None, Some(cli)) => (task.agent.clone(), super::task_answers(cli)),
                (None, None) => (task.agent.clone(), task.agent.clone()),
            };
            let (from, to) = (task.started, task.ended.unwrap_or_else(SystemTime::now));
            let entries = (self.log.iter())
                .filter(|entry| entry.agent == logged_as && entry.at >= from && entry.at <= to)
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
                })
                .collect();
            let over = steps.len().saturating_sub(crate::agent_history::STEPS);
            steps.drain(..over);
            let past = crate::agent_history::Past {
                id: format!("{}-{}", task.agent, seconds(task.started)),
                title: task.title.to_string(),
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

    /// The client's last connection closed: its session under way is done.
    pub(super) fn end_session(&mut self, agent: &str, cx: &mut Context<Self>) {
        let live = (self.tasks.tasks.iter()).find(|task| {
            task.state == State::Working
                && (task.session.as_ref()).is_some_and(|session| session.agent == agent)
        });
        if let Some(key) = live.map(|task| task.agent.clone()) {
            self.session_done(&key, cx);
        }
    }

    /// The user stopped the outside agent `agent`: its session under way is stopped.
    pub(super) fn session_stopped(&mut self, agent: &str) {
        let live = (self.tasks.tasks.iter_mut()).find(|task| {
            task.state == State::Working
                && (task.session.as_ref()).is_some_and(|session| session.agent == agent)
        });
        if let Some(task) = live {
            task.state = State::Stopped;
            task.ended = Some(SystemTime::now());
            if let Some(session) = &mut task.session {
                session.idle = None;
            }
        }
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
        let Some(task) = self.task_mut(key) else {
            return;
        };
        task.state = State::Done {
            result: SharedString::default(),
            summary: None,
            tabs,
        };
        task.ended = Some(SystemTime::now());
        if let Some(session) = &mut task.session {
            session.idle = None;
        }
        // check.sh reads this line.
        log!("browser: session {key} done");
        cx.notify();
    }

    /// What a call or a run's step does, as its trace says it: on `tab`, else the tab it names,
    /// else the agent's last.
    fn did(
        &self,
        agent: &str,
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
            .or_else(|| self.current.get(agent).copied());
        let url = tab
            .map(|tab| self.browser.tab(tab, cx).url.to_string())
            .unwrap_or_default();
        let element = (arguments.get("ref").and_then(Value::as_str).zip(tab))
            .and_then(|(name, tab)| self.refs.get(&tab)?.get(name).cloned());
        let named = |element: &page::Element| match element.name.trim().is_empty() {
            true => format!("the {}", element.role),
            false => format!("“{}”", page::clip(element.name.trim(), 40)),
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
                did.text = page::clip(&string("option"), tools::TYPED_SHOWN);
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
                    (false, _, _) => format!("“{}”", page::clip(&text, 40)),
                    (true, false, _) => format!("“{}” to go", page::clip(&gone, 40)),
                    _ if string("until") == "quiet" => tools::SETTLE.into(),
                    (true, true, Some(seconds)) => duration(seconds.round().max(1.) as u64),
                    (true, true, None) => String::new(),
                };
            }
            "list_tabs" | "ask" | "point" => {
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
                "Tab {tab} isn't one of yours: give the ids of tabs in your Space, from list_tabs."
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
            "title" => {
                let title = text("text");
                if title.is_empty() {
                    _ = reply.send(ToolResult::error(
                        "text is the task's name, in two to four words.",
                    ));
                    return;
                }
                let Some(task) = self.task_mut(agent) else {
                    return;
                };
                task.title = page::clip(title, 60).into();
                self.note(agent, "title", "", "");
                _ = reply.send(ToolResult::text("The task has its name."));
            }
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
                self.note(agent, "ask", "", "");
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
                let tabs = match self.left_tabs(agent, arguments, cx) {
                    Ok(tabs) => tabs,
                    Err(why) => {
                        _ = reply.send(ToolResult::error(why));
                        return;
                    }
                };
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
                task.ended = Some(SystemTime::now());
                self.note(agent, "finish", &site, "");
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
    match page {
        Some(page) => format!(
            "{asked}\n\n(I'm on “{}”, {}. Open it in your Space if you need it.)",
            page.title, page.url
        ),
        None => asked.to_string(),
    }
}

/// The conversation so far, for a new run of a task whose CLI went.
fn conversation(task: &Task) -> String {
    let mut text = first_message(&task.asked, task.page.as_ref());
    for message in &task.messages {
        let who = if message.who == Who::User {
            "Me"
        } else {
            "You"
        };
        text.push_str(&format!("\n\n{who}: {}", message.text));
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

/// A result's first words for its card's line: its first line of text, without the marks that
/// make it a heading, a list's item or bold.
fn first_words(result: &str) -> SharedString {
    let line = (result.lines())
        .map(|line| {
            line.trim()
                .trim_start_matches(['#', '>', '-', '*', ' '])
                .trim_end_matches(['*', ' '])
        })
        .find(|line| !line.is_empty());
    match line {
        Some(line) => page::clip(&line.replace("**", ""), 120).into(),
        None => "Finished".into(),
    }
}
/// What a card's status leads with when a confirmation or a handoff waits on the user, the one
/// status the card sets heavier (`status_lead`).
pub const NEEDS_YOU: &str = "Needs you";
const NEEDS_OK: &str = "Needs your OK";

/// A task's status while it works, saying how far it has got by its own steps: "Step 5 · Clicking
/// “Tickets”", the step of the turn it's on, else the last it took, counted among the turn's steps
/// with its thoughts left out; "Thinking · no steps yet" until it takes one. It changes only as a
/// step comes, never on a clock.
fn working(task: &Task) -> SharedString {
    let taken = turn_steps(task);
    match taken.first() {
        Some(step) => format!(
            "Step {} · {}",
            taken.len(),
            tools::step_title(&step.did, tense(step))
        )
        .into(),
        None => "Thinking · no steps yet".into(),
    }
}

/// The steps of the turn under way, newest first, its thoughts left out: what the card and the
/// conversation's header count.
fn turn_steps(task: &Task) -> Vec<&Traced> {
    let turn = task.turn();
    (task.trace.iter().rev())
        .take_while(|step| step.after >= turn)
        .filter(|step| step.did.tool != "thought")
        .collect()
}

/// The header's progress while the task works: its steps this turn and its pages, each left out
/// while there are none.
fn progress(steps: usize, pages: usize) -> String {
    let steps = (steps > 0).then(|| format!("step {steps}"));
    let pages = match pages {
        0 => None,
        1 => Some("1 page".to_string()),
        pages => Some(format!("{pages} pages")),
    };
    steps
        .into_iter()
        .chain(pages)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A stopped task's status: how far it got before the user stopped it, by its steps, its thoughts
/// left out.
fn stopped(task: &Task) -> SharedString {
    match (task.trace.iter())
        .filter(|step| step.did.tool != "thought")
        .count()
    {
        0 => "Stopped".into(),
        1 => "Stopped after 1 step".into(),
        steps => format!("Stopped after {steps} steps").into(),
    }
}

/// The status while a confirmation or a handoff waits: what the user is needed for, after the
/// words that say they are. A handoff says why, as the agent gave it; a confirmation, what the
/// agent would do.
fn needs_you(question: &Question) -> SharedString {
    match question.handoff {
        Some(_) if question.why.is_empty() => NEEDS_YOU.into(),
        Some(_) => format!("{NEEDS_YOU} · {}", question.why.trim_end_matches('.')).into(),
        None => format!("{NEEDS_OK} · {}", question.what).into(),
    }
}

/// How much of a card's `status` leads it, set in the title's ink and heavier: the step count
/// while it works ("Step 5"), and "Needs you" or "Needs your OK" while a confirmation or a handoff
/// waits, the one state that asks for the user. Zero for the rest. By the words alone, as a status
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

/// A duration in whole seconds as a trace says it: "41s", "2m 5s".
pub fn duration(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        _ if seconds % 60 == 0 => format!("{}m", seconds / 60),
        _ => format!("{}m {}s", seconds / 60, seconds % 60),
    }
}

/// A conversation's lines from its `messages` and its `trace`, oldest first. Each turn, from a
/// message of the user's to the next, shows its questions and, once it has `ended`, its last
/// message; the steps between those are a trace each, with what the agent said along the way in
/// it. The last stretch is under way while the task is `working`, and what the agent is `writing`
/// shows under it, as it comes and once it's whole, until the turn ends or a call takes it in.
pub fn fold(
    messages: &[Message],
    trace: &[Traced],
    ended: bool,
    working: bool,
    writing: Option<&Writing>,
) -> Vec<Line> {
    let writing = writing.filter(|_| working && !ended);
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
    for turn in &turns {
        // A turn the user has written after is over, even as the next is under way.
        let turn_ended = ended || turn.last().is_some_and(|message| message.who == Who::User);
        // The turn's last message: its result, else the last thing it said, once it's over.
        let last = (turn.iter().rposition(|message| message.who == Who::Result))
            .or_else(|| turn.iter().rposition(|message| message.who != Who::User))
            .filter(|_| turn_ended);
        for (at, message) in turn.iter().enumerate() {
            shown[first + at] = matches!(message.who, Who::User | Who::Asked) || Some(at) == last;
        }
        first += turn.len();
    }
    let mut lines = Vec::new();
    let mut stretch = Stretch::default();
    let mut steps = trace.iter().enumerate().peekable();
    for (index, message) in messages.iter().enumerate() {
        while let Some((key, step)) = steps.next_if(|(_, step)| step.after <= index) {
            stretch.step(key, step);
        }
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
    for (key, step) in steps {
        stretch.step(key, step);
    }
    stretch.flush(&mut lines, working, None);
    lines.extend(writing.map(|writing| Line::Said(writing.text.clone())));
    lines
}

/// A session's lines: its traces' steps, each a line of its own, newest last.
fn steps(lines: Vec<Line>) -> Vec<Line> {
    let steps = lines.into_iter().flat_map(|line| match line {
        Line::Trace(trace) => trace.items,
        _ => Vec::new(),
    });
    (steps.filter_map(|item| match item {
        TraceItem::Step(step) => Some(Line::Step(step)),
        TraceItem::Said(_) => None,
    }))
    .collect()
}

/// The stretch of work a trace is made of, as it gathers.
#[derive(Default)]
struct Stretch<'a> {
    gathered: Vec<Gathered<'a>>,
    key: Option<usize>,
    started: Option<Instant>,
    ended: Option<Instant>,
}

/// A step of a stretch, with its place in the task's trace, or what the agent said in it.
enum Gathered<'a> {
    Step(usize, &'a Traced),
    Said(SharedString),
}

impl<'a> Stretch<'a> {
    fn step(&mut self, key: usize, step: &'a Traced) {
        self.key.get_or_insert(key);
        self.started = Some(
            self.started
                .map_or(step.started, |started| started.min(step.started)),
        );
        let end = step.ended.unwrap_or(step.started);
        self.ended = Some(self.ended.map_or(end, |ended| ended.max(end)));
        self.gathered.push(Gathered::Step(key, step));
    }

    /// The stretch as its lines: a trace, or with no steps, what the agent said, as itself. It's
    /// `running` if it's the stretch under way, and ends at the agent's message `end`, if one ends
    /// it.
    fn flush(&mut self, lines: &mut Vec<Line>, running: bool, end: Option<Instant>) {
        let gathered = std::mem::take(&mut self.gathered);
        let (key, started, ended) = (self.key.take(), self.started.take(), self.ended.take());
        let Some(key) = key else {
            lines.extend(gathered.into_iter().filter_map(|item| match item {
                Gathered::Said(text) => Some(Line::Said(text)),
                Gathered::Step(..) => None,
            }));
            return;
        };
        let mut items: Vec<TraceItem> = (together(gathered).into_iter())
            .map(|item| match item {
                Together::Steps(steps) => TraceItem::Step(trace_step(&steps)),
                Together::Said(text) => TraceItem::Said(text),
            })
            .collect();
        if !running {
            for item in &mut items {
                if let TraceItem::Step(step) = item {
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
            (ended
                .saturating_duration_since(started)
                .as_secs_f32()
                .round() as u64)
                .max(1)
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
        };
        let did = &step.did;
        let groups = (did.tab.is_some() || did.tool == "open_tab")
            && !matches!(did.tool.as_str(), "thought" | "ask" | "point" | "list_tabs");
        let like = |line: &Together| {
            matches!(line, Together::Steps(steps)
                if steps[0].1.did.tool == did.tool && steps[0].1.did.target == did.target)
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

/// A line of a trace: a step, or a turn's same step on several tabs, which opens to each tab's.
fn trace_step(steps: &[(usize, &Traced)]) -> TraceStep {
    let &[(key, step)] = steps else {
        return trace_group(steps);
    };
    let took = step
        .ended
        .map(|ended| ended.saturating_duration_since(step.started));
    let description = match (&step.error, took) {
        (Some(error), _) => tools::reason(error),
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

/// A session's receipt as it ends: what it did that had consequences, the orders and messages the
/// user allowed it, once done, "Done · 1 order placed · 2 messages sent". Its pages are Open's
/// button's to count. With none of them, "Done 13:28", when it ended.
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
        true => format!("Done {ended}").trim_end().to_string(),
        false => format!("Done · {}", did.join(" · ")),
    }
}

/// A CLI's Model menu, as its ids and items, with `chosen` checked: Default first, followed by the
/// name of the model it is where the CLI says, then `models`.
pub fn model_menu(
    chosen: &str,
    default: Option<&str>,
    models: Vec<(String, String)>,
) -> (Vec<(String, String)>, Vec<MenuEntry>) {
    let default = default.map(|name| SharedString::from(name.to_string()));
    let mut all = vec![(String::new(), MenuEntry::label("Default").to_string())];
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
