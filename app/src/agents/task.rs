//! Agent tasks: the user asks in ⌘T, and their own `claude` or `codex` does it in the browser
//! (Agent task). Each task is an agent of its own, `task-<n>`, with a token the server knows, so
//! the rules, the activity log, the confirmations and the handoffs are the Agent Space's. Its Space
//! is hidden, in the profile of the Space it was asked in, and it shows as a card at the top of
//! that Space's Today tabs. Tasks live as long as the browser runs: their Spaces go when a task is
//! dropped, at quit, and at the next launch for any a crash left.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::SystemTime;

use futures::StreamExt;
use gpui::{App, AppContext, Context, SharedString};
use serde_json::Value;

use super::ready::{Found, Ready};
use super::runner::{self, Cli, Failure, Launch, Out, Process};
use super::{Agents, Question, ToolResult, page, site, tools};

/// The answers an ask shows as buttons on the card, at most, and how long each may be.
pub const ANSWERS: usize = 3;
pub const ANSWER_LENGTH: usize = 24;
/// The dismissed tasks ⇧⌘T and ⌘Z can bring back. An older one is dropped, its Space with it.
const KEPT: usize = 10;

/// A task's state, as its card shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Working,
    /// The agent asks the user (ask), and waits for the answer.
    Asking { question: SharedString, answers: Vec<SharedString> },
    /// The agent finished, with its result, and the tab the user may open, if any.
    Done { result: SharedString, tab: Option<u64> },
    /// The user's plan is at its limit, until this time, as the clock reads it.
    Limit { resets: Option<SharedString> },
    Stopped,
    /// It couldn't go on: why, and the command that fixes it, if one does.
    Failed { why: SharedString, command: Option<&'static str> },
}

/// A line of the conversation, and when it was said, which places the steps between lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub from_user: bool,
    pub text: SharedString,
    pub at: SystemTime,
}

impl Message {
    fn new(from_user: bool, text: impl Into<SharedString>) -> Self {
        Self { from_user, text: text.into(), at: SystemTime::now() }
    }
}

/// What a task's conversation shows, in the page's place (Agent task): compared with the last, so
/// it redraws only when it changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Conversation {
    pub agent: SharedString,
    /// The agent's name, in text: Claude or Codex.
    pub name: &'static str,
    pub title: SharedString,
    /// What was asked, and the page it was asked on, by its title.
    pub asked: SharedString,
    pub page: Option<AskedOn>,
    pub lines: Vec<Line>,
    /// What it's doing now, while it works.
    pub doing: Option<SharedString>,
    /// Its pages, for their pictures, each by its tab and site.
    pub tabs: Vec<(u64, SharedString)>,
    /// Whether it works or waits on the user, which ⌘. stops.
    pub stoppable: bool,
}

/// A line of a task's conversation.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    You(SharedString),
    Said(SharedString),
    /// The steps between two lines, folded into one: what the last did, and how many there were.
    Steps { last: SharedString, count: usize },
}

/// The page the user was on when they asked, which goes as its address and title only.
#[derive(Clone, Debug, PartialEq)]
pub struct AskedOn {
    pub url: SharedString,
    pub title: SharedString,
}

pub struct Task {
    pub agent: String,
    pub cli: Cli,
    /// The user's Space it was asked in, whose Today it shows at the top of.
    pub home: u64,
    /// Its own Space, hidden, where its tabs open.
    pub space: u64,
    pub asked: SharedString,
    pub page: Option<AskedOn>,
    /// The name the agent gives it (title), else what was asked.
    pub title: SharedString,
    pub state: State,
    pub messages: Vec<Message>,
    pub dismissed: bool,
    token: String,
    process: Option<Process>,
    /// The ask's call, which waits for the user's answer.
    asking: Option<mpsc::Sender<ToolResult>>,
}

impl Task {
    pub(super) fn token(&self) -> &str {
        &self.token
    }
}

/// What a task's card shows: compared with the last, so the card redraws only when it changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub agent: SharedString,
    pub home: u64,
    /// The task's own Space, which the page area shows it from.
    pub space: u64,
    pub cli: Cli,
    pub title: SharedString,
    /// What it's doing, as the activity log says it, or what it waits for, or its result.
    pub status: SharedString,
    pub dot: Dot,
    /// An ask's answers, as buttons.
    pub answers: Vec<SharedString>,
    /// A confirmation or a handoff of the task's, which the card asks instead of the windows'.
    pub question: Option<Question>,
    /// Done's tab to open, and its site.
    pub open: Option<(u64, SharedString)>,
    /// Limit's Retry.
    pub retry: bool,
    /// The command that fixes a failure.
    pub command: Option<&'static str>,
}

/// The card's status dot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dot {
    /// Signal, breathing, as a tab's while an agent works in it.
    Working,
    /// Still: it waits for the user.
    Waiting,
    /// A tick.
    Done,
    None,
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
    /// Done's Open <site>.
    Open,
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
    pub(super) events: Option<(futures::channel::mpsc::UnboundedSender<runner::Event>, gpui::Task<()>)>,
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

    /// Does what the user did to `agent`'s task. Returns the tab Open moved into the user's
    /// Space, for the window to show, and the task Restore brought back.
    pub fn task_action(
        &mut self,
        agent: &str,
        action: TaskAction,
        cx: &mut Context<Self>,
    ) -> (Option<u64>, Option<String>) {
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
            TaskAction::Open => return (self.open_result(agent, cx), None),
        }
        (None, None)
    }

    /// `agent`'s task's conversation: what was asked, then what each side said, with the steps
    /// between folded into a line each.
    pub fn conversation(&self, agent: &str, cx: &App) -> Option<Conversation> {
        let task = self.task(agent)?;
        let mut steps: Vec<&super::Activity> = self.activity(agent).collect();
        steps.reverse();
        let mut steps = steps.into_iter().peekable();
        let mut lines = Vec::new();
        let mut fold = |lines: &mut Vec<Line>, until: Option<SystemTime>| {
            let mut folded = Vec::new();
            while let Some(step) = steps.next_if(|step| until.is_none_or(|until| step.at <= until)) {
                folded.push(step);
            }
            if let Some(last) = folded.last() {
                let last = match last.site.is_empty() {
                    true => tools::describe(&last.action).to_string(),
                    false => format!("{} · {}", tools::describe(&last.action), last.site),
                };
                lines.push(Line::Steps { last: last.into(), count: folded.len() });
            }
        };
        for message in &task.messages {
            fold(&mut lines, Some(message.at));
            lines.push(match message.from_user {
                true => Line::You(message.text.clone()),
                false => Line::Said(message.text.clone()),
            });
        }
        let working = task.state == State::Working;
        // While it works, the steps since the last line are what it's doing, under the rest.
        if !working {
            fold(&mut lines, None);
        }
        let tabs = (self.browser.space_tabs(task.space, cx).into_iter())
            .map(|tab| (tab.id, site(&tab.url).to_string().into()))
            .collect();
        Some(Conversation {
            agent: task.agent.clone().into(),
            name: task.cli.name(),
            title: task.title.clone(),
            asked: task.asked.clone(),
            page: task.page.clone(),
            lines,
            doing: working.then(|| self.doing.get(agent).cloned().unwrap_or_else(|| "Starting".into())),
            tabs,
            stoppable: matches!(task.state, State::Working | State::Asking { .. }),
        })
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
        let question = (self.asking.iter().map(|asking| &asking.question))
            .chain(self.question().as_ref().filter(|question| question.handoff.is_some()))
            .find(|question| question.from == task.agent.as_str())
            .cloned();
        let doing = self.doing.get(&task.agent).cloned();
        let (status, dot) = match (&task.state, &question) {
            (State::Working, Some(question)) => match question.handoff {
                Some(_) => ("Waiting for you".into(), Dot::Waiting),
                None => ("Waiting for your OK".into(), Dot::Waiting),
            },
            (State::Working, None) => (doing.unwrap_or_else(|| "Starting".into()), Dot::Working),
            (State::Asking { question, .. }, _) => (question.clone(), Dot::Waiting),
            (State::Done { result, .. }, _) => (result.clone(), Dot::Done),
            (State::Limit { resets: Some(at) }, _) => (format!("Limit resets at {at}").into(), Dot::None),
            (State::Limit { resets: None }, _) => ("At its plan's limit".into(), Dot::None),
            (State::Stopped, _) => ("Stopped".into(), Dot::None),
            (State::Failed { why, .. }, _) => (why.clone(), Dot::None),
        };
        let mut status = status;
        let open = match task.state {
            // The core's auto-archive can take a task's page it never showed (window.cc), and a
            // task lives for one run, so its card says where it went rather than offer it.
            State::Done { tab: Some(tab), .. }
                if !self.browser.space_tab_ids(task.space, cx).contains(&tab) =>
            {
                status = "Its page went to the Archive".into();
                None
            }
            State::Done { tab: Some(tab), .. } => {
                let url = self.browser.tab(tab, cx).url;
                Some((tab, site(&url).into()))
            }
            _ => None,
        };
        Card {
            agent: task.agent.clone().into(),
            home: task.home,
            space: task.space,
            cli: task.cli,
            title: task.title.clone(),
            status,
            dot,
            answers: match &task.state {
                State::Asking { answers, .. } => answers.clone(),
                _ => Vec::new(),
            },
            question,
            open,
            retry: matches!(task.state, State::Limit { .. }),
            command: match task.state {
                State::Failed { command, .. } => command,
                _ => None,
            },
        }
    }

    /// The CLIs ⌘T's ask lists: those signed in, as last learned, the default first.
    pub fn ask_agents(&self) -> Vec<AskAgent> {
        if !self.on {
            return Vec::new();
        }
        let mut clis: Vec<Cli> = (Cli::ALL.into_iter())
            .filter(|cli| self.tasks.found.get(cli).is_some_and(|found| found.ready == Ready::SignedIn))
            .collect();
        if let Some(default) = self.tasks.default {
            clis.sort_by_key(|cli| *cli != default);
        }
        (clis.into_iter())
            .map(|cli| AskAgent { cli, resets: self.tasks.limits.get(&cli).cloned().flatten() })
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
        self.tasks.found.get(&cli).map_or(Ready::Unknown, |found| found.ready)
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
                agents.tasks.found.extend(found);
                agents.save();
                // The first ready is the default until the user picks one.
                if agents.tasks.default.is_none_or(|cli| agents.found(cli) != Ready::SignedIn) {
                    let ready = Cli::ALL.into_iter().find(|cli| agents.found(*cli) == Ready::SignedIn);
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

    /// The models `cli`'s Model pop-up offers after its own default, as id and name: Claude's
    /// aliases, and the models Codex listed when it was last looked for.
    pub fn task_models(&self, cli: Cli) -> Vec<(String, String)> {
        match cli.models() {
            [] => self.tasks.found.get(&cli).map(|found| found.models.clone()).unwrap_or_default(),
            models => models.iter().map(|(id, name)| (id.to_string(), name.to_string())).collect(),
        }
    }

    /// Run tasks on, in Settings › Agents.
    pub fn set_default_task_agent(&mut self, cli: Cli, cx: &mut Context<Self>) {
        self.tasks.default = Some(cli);
        self.save();
        cx.notify();
    }

    /// The model `cli` runs tasks with, empty for its own default.
    pub fn task_model(&self, cli: Cli) -> &str {
        self.tasks.models.get(&cli).map_or("", String::as_str)
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
        if asked.is_empty() || !self.on {
            return None;
        }
        self.tasks.next += 1;
        let agent = format!("task-{}", self.tasks.next);
        // check.sh reads this line.
        log!("browser: task {agent} on {} asked from space {home}", cli.id());
        let space = self.browser.make_task_space(&agent, cli.name(), home, cx);
        let title: SharedString = page::clip(asked, 80).into();
        let task = Task {
            agent: agent.clone(),
            cli,
            home,
            space,
            asked: asked.to_string().into(),
            page,
            title,
            state: State::Working,
            messages: Vec::new(),
            dismissed: false,
            token: super::random_token(),
            process: None,
            asking: None,
        };
        self.tasks.tasks.push(task);
        self.tokens_changed();
        let first = first_message(asked, self.task(&agent).and_then(|task| task.page.as_ref()));
        self.launch(&agent, &first, cx);
        cx.notify();
        Some(agent)
    }

    /// Starts the task's CLI with its first message.
    fn launch(&mut self, agent: &str, first: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task(agent) else { return };
        let cli = task.cli;
        let found = self.tasks.found.get(&cli).cloned().unwrap_or_default();
        let Some(program) = found.program.clone() else {
            return self.fail(agent, Failure::Other(format!("{} isn't installed.", cli.name())), cx);
        };
        let dir = self.tasks.dir.clone().unwrap_or_else(std::env::temp_dir).join("Tasks");
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
            server: runner::Server { command, args, token: task.token.clone() },
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
                self.fail(agent, Failure::Other(format!("{} couldn't start.", cli.name())), cx);
            }
        }
    }

    /// Where the tasks' processes hand their output, which wakes the UI only as they say something.
    fn task_events(&mut self, cx: &mut Context<Self>) -> futures::channel::mpsc::UnboundedSender<runner::Event> {
        if let Some((events, _)) = &self.tasks.events {
            return events.clone();
        }
        let (events, mut said) = futures::channel::mpsc::unbounded();
        let reading = cx.spawn(async move |agents, cx| {
            while let Some(event) = said.next().await {
                if agents.update(cx, |agents, cx| agents.task_said(event, cx)).is_err() {
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
        match out {
            Out::Said(text) => {
                let Some(task) = self.task_mut(&agent) else { return };
                task.messages.push(Message::new(false, text));
            }
            Out::Ended(None) => {
                let Some(task) = self.task_mut(&agent) else { return };
                // A turn that ends without finish ends the task with what it said last.
                if task.state == State::Working {
                    let result = (task.messages.iter().rev())
                        .find(|message| !message.from_user)
                        .map_or_else(|| "Done".into(), |message| message.text.clone());
                    task.state = State::Done { result, tab: None };
                }
            }
            Out::Ended(Some(failure)) => self.fail(&agent, failure, cx),
            Out::Gone(said) => {
                let Some(task) = self.task_mut(&agent) else { return };
                task.process = None;
                if matches!(task.state, State::Working | State::Asking { .. }) {
                    let said = said.trim();
                    log!("browser: task {agent} ended: {said}");
                    let why = format!("{} stopped working.", task.cli.name());
                    self.fail(&agent, Failure::Other(why), cx);
                }
            }
        }
        cx.notify();
    }

    /// The task can't go on: its card says why, and what fixes it.
    fn fail(&mut self, agent: &str, failure: Failure, cx: &mut Context<Self>) {
        let Some(cli) = self.task(agent).map(|task| task.cli) else { return };
        // check.sh reads this line.
        log!("browser: task {agent} failed: {failure:?}");
        let state = match failure {
            Failure::Limit(resets) => State::Limit { resets: resets.map(clock) },
            Failure::SignedOut => {
                let why = format!("{} isn't signed in.", cli.name()).into();
                State::Failed { why, command: Some(cli.sign_in()) }
            }
            Failure::Other(why) if why.contains("isn't installed") => {
                State::Failed { why: why.into(), command: Some(cli.install()) }
            }
            Failure::Other(why) => State::Failed { why: page::clip(&why, 200).into(), command: None },
        };
        if let State::Limit { resets } = &state {
            self.tasks.limits.insert(cli, resets.clone());
        }
        if let Some(task) = self.task_mut(agent) {
            task.state = state;
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
        let Some(task) = self.task_mut(agent) else { return };
        task.messages.push(Message::new(true, text.to_string()));
        task.state = State::Working;
        let sent = task.process.as_mut().map(|process| process.say(text));
        if !matches!(sent, Some(Ok(()))) {
            // Gone, or never started: a new run, with the conversation so far.
            let first = conversation(task);
            self.launch(agent, &first, cx);
        }
        cx.notify();
    }

    /// The user's answer to the task's ask.
    pub fn answer_task(&mut self, agent: &str, answer: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else { return };
        let Some(reply) = task.asking.take() else { return };
        task.messages.push(Message::new(true, answer.to_string()));
        task.state = State::Working;
        _ = reply.send(ToolResult::text(format!("The user answered: {answer}")));
        self.note(agent, "answered", "", "");
        cx.notify();
    }

    /// ⌘. or Stop: ends what the task is doing at once. A reply goes on from there.
    pub fn stop_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else { return };
        if !matches!(task.state, State::Working | State::Asking { .. }) {
            return;
        }
        task.state = State::Stopped;
        task.asking = None;
        if let Some(process) = task.process.as_mut() {
            process.interrupt();
        }
        self.stop_agent(agent, cx);
        cx.notify();
    }

    /// Limit's Retry: goes on from where it stopped. The browser never retries by itself.
    pub fn retry_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else { return };
        if !matches!(task.state, State::Limit { .. }) {
            return;
        }
        task.state = State::Working;
        let sent = task.process.as_mut().map(|process| process.say(RETRY));
        let cli = task.cli;
        self.tasks.limits.remove(&cli);
        if !matches!(sent, Some(Ok(()))) {
            let first = conversation(self.task(agent).expect("found"));
            self.launch(agent, &first, cx);
        }
        cx.notify();
    }

    /// ✕ or ⌘W: the task goes to the Archive, which ⇧⌘T and ⌘Z bring it back from. What it was
    /// doing stops. An older dismissed task than the last few goes for good.
    pub fn dismiss_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        self.stop_task(agent, cx);
        let Some(task) = self.task_mut(agent) else { return };
        task.dismissed = true;
        // check.sh reads this line.
        log!("browser: task {agent} dismissed");
        self.tasks.dismissed.retain(|each| each != agent);
        self.tasks.dismissed.push_back(agent.to_string());
        while self.tasks.dismissed.len() > KEPT {
            if let Some(old) = self.tasks.dismissed.pop_front() {
                self.drop_task(&old, cx);
            }
        }
        cx.notify();
    }

    /// Brings a dismissed task back to its card: the one named, or the last dismissed.
    pub fn restore_task(&mut self, agent: Option<&str>, cx: &mut Context<Self>) -> Option<String> {
        let agent = match agent {
            Some(agent) => agent.to_string(),
            None => self.tasks.dismissed.back()?.clone(),
        };
        self.tasks.dismissed.retain(|each| *each != agent);
        let task = self.task_mut(&agent)?;
        task.dismissed = false;
        log!("browser: task {agent} restored");
        cx.notify();
        Some(agent)
    }

    /// Done's Open <site>: the tab moves into the task's Space's Today, at its top, the user's.
    pub fn open_result(&mut self, agent: &str, cx: &mut Context<Self>) -> Option<u64> {
        let task = self.task_mut(agent)?;
        let State::Done { tab: Some(tab), result } = task.state.clone() else {
            return None;
        };
        let home = task.home;
        task.state = State::Done { result, tab: None };
        self.browser.move_tab(tab, home, cx);
        // check.sh reads this line.
        log!("browser: task {agent} opens {tab} in space {home}");
        cx.notify();
        Some(tab)
    }

    /// The task goes for good: its CLI ends and its Space goes, with its tabs.
    fn drop_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(at) = self.tasks.tasks.iter().position(|task| task.agent == agent) else {
            return;
        };
        let task = self.tasks.tasks.remove(at);
        self.forget_waiting(agent, cx);
        self.doing.remove(agent);
        self.tokens_changed();
        if task.space != 0 {
            self.browser.delete_space(task.space, cx);
        }
        self.remove_task_files(agent);
        log!("browser: task {agent} dropped");
    }

    /// At quit: every task goes, with its Space.
    pub fn end_tasks(&mut self, cx: &mut Context<Self>) {
        let agents: Vec<String> = self.tasks.tasks.iter().map(|task| task.agent.clone()).collect();
        for agent in agents {
            self.drop_task(&agent, cx);
        }
    }

    /// At launch: the Spaces a run before left of its tasks, as a crash would, go.
    pub fn end_old_tasks(&mut self, cx: &mut Context<Self>) {
        for space in self.browser.task_spaces() {
            log!("browser: deleting space {space} of a task before");
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
        let text = |name: &str| arguments.get(name).and_then(Value::as_str).unwrap_or("").trim();
        match tool {
            "title" => {
                let title = text("text");
                if title.is_empty() {
                    _ = reply.send(ToolResult::error("text is the task's name, in a few words."));
                    return;
                }
                let Some(task) = self.task_mut(agent) else { return };
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
                let answers: Vec<SharedString> = (arguments.get("answers").and_then(Value::as_array))
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
                    && answers.iter().all(|answer| answer.chars().count() <= ANSWER_LENGTH)
                {
                    true => answers,
                    false => Vec::new(),
                };
                let Some(task) = self.task_mut(agent) else { return };
                let question: SharedString = page::clip(question, 300).into();
                task.messages.push(Message::new(false, question.clone()));
                task.state = State::Asking { question, answers: buttons };
                task.asking = Some(reply);
                self.note(agent, "ask", "", "");
            }
            "finish" => {
                let result = text("result");
                if result.is_empty() {
                    _ = reply.send(ToolResult::error("result is what you did, in a sentence or two."));
                    return;
                }
                let tab = arguments.get("tab").and_then(Value::as_u64);
                let space = self.task(agent).map_or(0, |task| task.space);
                let tab = tab.filter(|tab| self.browser.space_tab_ids(space, cx).contains(tab));
                let site = tab.map(|tab| site(&self.browser.tab(tab, cx).url));
                let Some(task) = self.task_mut(agent) else { return };
                task.state = State::Done { result: page::clip(result, 300).into(), tab };
                self.note(agent, "finish", site.as_deref().unwrap_or(""), "");
                _ = reply.send(ToolResult::text("Done. The user sees your result. Stop here."));
            }
            _ => _ = reply.send(ToolResult::error(format!("There's no tool {tool}."))),
        }
        cx.notify();
    }
}

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
        let who = if message.from_user { "Me" } else { "You" };
        text.push_str(&format!("\n\n{who}: {}", message.text));
    }
    text.push_str("\n\nGo on from here.");
    text
}

/// A time in seconds since 1970 as the clock reads it here, such as 18:00.
fn clock(at: u64) -> SharedString {
    crate::agents::local_time(at).into()
}
