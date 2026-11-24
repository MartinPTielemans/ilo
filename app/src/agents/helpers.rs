//! A task's helpers (#4102): its assistant's own way of working on several pages at once, behind
//! a switch that's off.
//!
//! A helper is the assistant's, not ilo's. It calls the browser's tools through the task's own
//! connection, so each of its acts meets the same check as the task's, at the task's access as it
//! is at that act, and ilo can't tell a helper's call from the task's. What ilo reads of helpers
//! comes from the CLI's output alone, and three things are built on it: a label for the
//! conversation's lines (which of the task's pages a helper works on), which nothing ilo allows or
//! refuses reads; a count of the helpers its CLI says started, for the limit, which bounds what a
//! task costs the user's plan, never what it may reach; and ilo's floor.
//!
//! The floor: the only helper is ilo's kind, started by the task, one level under it, waited for.
//! The assistant is started so that it has no other (`ENV`, `NOT_OURS`), by its maker's
//! documentation, which no run has shown yet. ilo can't hold the assistant's own tool back, so
//! what it does itself comes after the fact: a first line that lists another kind ends the run
//! before its first answer (`Agents::helper_kinds`), and a start, a call or a helper's line that
//! shows what isn't allowed stops the task (`Agents::helper_not_allowed`). A helper that went past
//! may have acted until the stop landed, each act at the task's access; after it every call of
//! the task's is answered as stopped (#4209).
//!
//! The reading here is checked against one recorded run, whose first message is
//! `runner/answers/claude-2.1.331-helpers.jsonl`. What that run showed keeps `ON` off, and is
//! written at it.

use std::time::Instant;

use gpui::{App, Context, SharedString};
use serde_json::{Value, json};

use super::runner::{Helper, Out, SERVER};
use super::task::ilo::{HelpersLimit as Limit, IloKind, IloLine};
use super::task::{Line, State, Task, Who};
use super::tier;
use super::{Agents, tools};
use crate::tokens::agents::helpers::{DEEP, MOST};

/// Whether a task's assistant gets its helper tool. Off: nothing in Settings turns it on, and a
/// task started while it's off has today's arguments, to the byte.
///
/// One run was recorded (Claude Code 2.1.331), started as a task with helpers is. It showed:
/// 1. Under `--tools Agent` the assistant keeps kinds of helper of its own beside ilo's. ilo's
///    kind had the browser's tools alone. A built-in one had no shell, files, fetch or search, but
///    every tool the task is allowed, the task's own among them.
/// 2. So a built-in helper called `title`, and ilo took the call for the task's. `ask` and
///    `finish` weren't tried.
/// 3. A helper starts in the background: `task_started` names the call that asked for it as
///    `tool_use_id`, with `spawn_depth` 1 and no parent, and the turn's `result` comes while the
///    helpers are at work, so the task reads as done while they still act. No helper started one
///    of its own, so that start's shape isn't known.
/// 4. An interrupt ended every helper under way at once, and no start came after it. A helper's
///    call that waited in ilo's own queue was still answered after the Stop.
/// 5. The helper tool needs no `--allowedTools`.
///
/// It stays off until a run shows no helper with the task's own tools, a task that isn't done
/// while its helpers work, and nothing of a stopped task's answered: started as `ENV` and
/// `NOT_OURS` start it, which no run has been yet.
pub const ON: bool = false;

/// What the assistant's process is started with while helpers are on, each by its maker's
/// documentation, and none while they're off. ilo sets them on the process itself, so its value
/// is the one the assistant reads whatever ilo's own environment holds.
pub const ENV: [(&str, &str); 4] = [
    // None of the assistant's own kinds of helper, in print mode: ilo's kind alone is left.
    ("CLAUDE_AGENT_SDK_DISABLE_BUILTIN_AGENTS", "1"),
    // No helper that is the task over again, with its conversation and every tool of its own.
    ("CLAUDE_CODE_FORK_SUBAGENT", "0"),
    // A helper is waited for, so the task's turn ends after its helpers. It takes the background
    // from every tool, so a slow call of the browser's is waited for too.
    ("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1"),
    // One level under the task: a helper has no tool that starts another.
    ("CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH", "1"),
];

/// The kinds of helper the assistant is known to keep of its own, denied by name beside `ENV`:
/// a second belt only, as a list misses the kind a later version adds.
const NOT_OURS: [&str; 8] = [
    "fork",
    "claude",
    "general-purpose",
    "Explore",
    "Plan",
    "statusline-setup",
    "claude-code-guide",
    "web-fetch",
];

/// The assistant's tool that starts a helper: as its arguments and its own calls name it, and as
/// its init line lists it.
pub const TOOLS: [&str; 2] = ["Agent", "Task"];

/// The kind of helper ilo describes to the assistant, by the name the task asks for it with.
const KIND: &str = "page-helper";

/// The task's own tools, which no helper has: the task's name, its question, its end and a tab
/// handed to the user, each of which speaks to the user as the task. ilo's server can't tell a
/// helper's call from the task's, so a helper's line that calls one stops the task.
const THE_TASKS_OWN: [&str; 5] = ["title", "ask", "finish", "done", "hand_off"];

/// What a call without a tab is told once the task has more than one, as a helper's "tab used
/// last" may be another's page. It names no page.
pub const NAME_YOUR_TAB: &str = "This task has more than one tab. Give the call its tab, a number \
                                 from list_tabs or from the open_tab that made it. Nothing was \
                                 done.";

/// What a task with helpers is told after `tools::TASK_INSTRUCTIONS`: the two numbers, so the
/// limit is known before it's met, and that every call names its tab.
pub fn instructions() -> String {
    format!(
        "You can start helpers with the {} tool, as “{KIND}”, when the same work repeats across \
         pages: one page each. Start at most {MOST} for each message of the user's, and a helper \
         may start none of its own: ilo stops the turn if either limit is passed. If there are \
         more pages, do the rest yourself in your own tabs. A helper works in the tab you give it \
         or one it opens, at this task's access level, and what it finds comes back to you as its \
         result. Once this task has more than one tab, give every browser call its tab; one \
         without is refused.",
        TOOLS[0]
    )
}

/// The arguments that describe ilo's kind of helper to Claude, after the task's own: the browser's
/// tools without the task's own and without the tool that starts a helper. The shape is the CLI's
/// documented one; whether a helper so described can start another wasn't run, so the depth its
/// CLI says each start is at is what ilo holds the limit to (`Agents::helper_started`).
pub fn claude_args(allowed: &[String]) -> Vec<String> {
    let own = THE_TASKS_OWN.map(|tool| format!("mcp__{SERVER}__{tool}"));
    let tools: Vec<&String> = (allowed.iter())
        .filter(|tool| !own.contains(tool))
        .collect();
    let kinds = json!({
        KIND: {
            "description": "Does one page's part of the task in the browser, in a tab of its own.",
            "prompt": "You do one page's part of a task in the user's web browser, through its \
                       tools only. Give every call its tab. A sign-in, a 2FA prompt or a CAPTCHA \
                       is the user's: stop there and say so in your result, with the tab, so the \
                       task hands it over. Never pay, order, book or send a message unless your \
                       instructions say the user asked for it. End with what you found, short \
                       and plain.",
            "tools": tools,
        }
    });
    let denied = NOT_OURS.map(|kind| format!("{}({kind})", TOOLS[0]));
    vec![
        "--disallowedTools".into(),
        denied.join(","),
        "--agents".into(),
        kinds.to_string(),
    ]
}

/// Whether a block is a call of `names`' tool.
fn calls(block: &Value, names: &[String]) -> bool {
    let name = block.get("name").and_then(Value::as_str);
    block.get("type").and_then(Value::as_str) == Some("tool_use")
        && name.is_some_and(|name| names.iter().any(|each| each == name))
}

/// The kinds of helper a run's first line lists: whether each is ilo's. One that lists none it
/// can read is taken as listing another, as ilo can't tell.
pub fn kinds(kind: &str, message: &Value) -> Option<Out> {
    let init = message.get("subtype").and_then(Value::as_str) == Some("init");
    if kind != "system" || !init {
        return None;
    }
    let listed = message.get("agents").and_then(Value::as_array);
    let ours = listed.is_some_and(|listed| listed.iter().all(|each| each.as_str() == Some(KIND)));
    Some(Out::Kinds { ours })
}

/// What a line of Claude's output with a parent says, which is a helper's line (`parent` is the
/// call of the task's that asked for it): the tabs it calls the browser's tools on, for its label.
/// Nothing it says, writes or thinks comes out, so none of it is shown as the task's or kept.
/// One that starts a helper, or calls a tool that is the task's alone, says only that.
pub fn claude_read(parent: &str, kind: &str, message: &Value) -> Vec<Out> {
    if kind != "assistant" {
        return Vec::new();
    }
    let content = message
        .pointer("/message/content")
        .and_then(Value::as_array);
    let blocks = || content.into_iter().flatten();
    if blocks().any(|block| calls(block, &TOOLS.map(String::from))) {
        return vec![Out::NotAllowed(Limit::Depth)];
    }
    let own = THE_TASKS_OWN.map(|tool| format!("mcp__{SERVER}__{tool}"));
    if blocks().any(|block| calls(block, &own)) {
        return vec![Out::NotAllowed(Limit::Kind)];
    }
    let ours = format!("mcp__{SERVER}__");
    (content.into_iter().flatten())
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter(|block| {
            (block.get("name").and_then(Value::as_str)).is_some_and(|name| name.starts_with(&ours))
        })
        .filter_map(|block| tools::tab_id(block.pointer("/input/tab")?))
        .map(|tab| Out::Helper {
            id: parent.to_string(),
            told: Helper::On(tab),
        })
        .collect()
}

/// The helpers a message of the task's own asks for, from its blocks: each call of the helper
/// tool, by its id, which a helper's lines name as their parent, and by the name the task gave it
/// (the call's `description`), for its line. One asked for as another kind
/// than ilo's, as none, or for the background isn't allowed, which the line says before the
/// helper starts.
pub fn asked<'a>(blocks: impl Iterator<Item = &'a Value>) -> impl Iterator<Item = Out> {
    blocks
        .filter(|block| calls(block, &TOOLS.map(String::from)))
        .filter_map(|block| {
            let ours = block
                .pointer("/input/subagent_type")
                .and_then(Value::as_str)
                == Some(KIND);
            let background = block.pointer("/input/run_in_background") == Some(&Value::Bool(true));
            if !ours || background {
                return Some(Out::NotAllowed(Limit::Kind));
            }
            let name = block.pointer("/input/description").and_then(Value::as_str);
            Some(Out::Helper {
                id: block.get("id")?.as_str()?.to_string(),
                told: Helper::Asked(name.map(tier::plain).unwrap_or_default()),
            })
        })
}

/// A `system` line that says a helper started, with how deep under the task its CLI says it is.
/// One with a parent is a helper's own line, so a start it tells without a depth is taken as a
/// helper's helper: deeper than any may be. A start of another kind than ilo's, of none, or in
/// the background, where the task's turn would end before its helper, isn't allowed.
pub fn started(kind: &str, message: &Value) -> Option<Out> {
    let started = message.get("subtype").and_then(Value::as_str) == Some("task_started");
    if kind != "system" || !started {
        return None;
    }
    let parent = (message.get("parent_tool_use_id")).is_some_and(Value::is_string);
    let deep = match (message.get("spawn_depth").and_then(Value::as_u64), parent) {
        (Some(deep), _) => deep,
        (None, true) => u64::from(DEEP) + 1,
        (None, false) => 0,
    };
    let ours = message.get("subagent_type").and_then(Value::as_str) == Some(KIND);
    let background = message.get("is_backgrounded") == Some(&Value::Bool(true));
    match deep <= u64::from(DEEP) && (!ours || background) {
        true => Some(Out::NotAllowed(Limit::Kind)),
        false => Some(Out::Started { deep }),
    }
}

/// A run of a task with helpers, until its first line is read: what it would be started with
/// again, without them.
struct Run {
    /// Its first message, as it was sent.
    first: String,
    /// Whether the run began a conversation its assistant keeps, which goes with it.
    new: bool,
    /// How much the task had done and said as it started: a run that has added to either has
    /// answered, and isn't started again.
    had: (usize, usize),
}

/// What ilo holds of a task's helpers, in memory only: nothing of it is logged or kept.
#[derive(Default)]
pub struct Helpers {
    /// Whether the task's assistant was started with its helper tool, which the switch decided as
    /// it started.
    pub on: bool,
    /// The helpers the task asked for, oldest first.
    seen: Vec<Seen>,
    /// How many its CLI said started since the user's last message.
    started: u32,
    /// What follows the end of the turn ilo interrupted for the limit, while that end is awaited.
    stopping: Option<After>,
    /// The run under way, until its first line is read.
    run: Option<Run>,
    /// The times ilo stopped it after it started more, as its conversation shows each.
    pub(super) over: Vec<Over>,
}

/// What follows the end of a turn ilo interrupted at the limit.
pub(super) enum After {
    /// ilo tells the task why, and the task goes on in a new turn: that end wasn't its own.
    Told(String),
    /// Nothing: stopped a second time since the user wrote, the turn ends as any does and the
    /// task waits for the user, so a task that keeps starting helpers can't go on doing so.
    Ends,
}

/// A helper the task asked for: a label for its line. Which steps are its own is read from the
/// output's word alone, so nothing but the line is built on it.
struct Seen {
    /// The call that asked for it.
    id: String,
    /// The name its task gave it, for its line: the task's own words, as its title is.
    name: SharedString,
    /// How long the task's trace was as it was asked for, and as the user wrote next: its steps
    /// are between them.
    from: usize,
    until: Option<usize>,
    /// The tabs its lines named.
    tabs: Vec<u64>,
}

/// A time ilo stopped a task at its helpers' limit.
#[derive(Clone, Debug, PartialEq)]
pub struct Over {
    pub limit: Limit,
    /// The page the task stood on, which Take Over takes, or 0 with none.
    pub tab: u64,
    /// How many messages the user had written, which places it at the end of that turn.
    turn: usize,
    /// When it showed, which a click already under way then mustn't answer.
    pub(super) appeared: Instant,
    /// Acted on, or passed: Take Over or the user's next message.
    settled: bool,
}

/// The keys of the limit's lines, which count up from here, apart from the lines made from a
/// task's events and those that count down from the top (pick_up.rs).
const OVER: usize = usize::MAX / 2;

impl Helpers {
    /// A new task's, `on` if its assistant gets the helper tool.
    pub(super) fn of_task(on: bool) -> Self {
        Helpers {
            on,
            ..Helpers::default()
        }
    }

    /// Which of the task's helpers the step at `key` of its trace, on `tab`, is labelled as, if
    /// any: the last one asked for before the step whose lines named the tab.
    pub fn of(&self, key: usize, tab: Option<u64>) -> Option<usize> {
        let tab = tab?;
        self.seen.iter().rposition(|seen| {
            seen.from <= key
                && seen.until.is_none_or(|until| key < until)
                && seen.tabs.contains(&tab)
        })
    }

    /// The name its task gave a helper of `of`'s numbering, or empty.
    pub fn name(&self, helper: usize) -> SharedString {
        (self.seen.get(helper)).map_or_else(SharedString::default, |seen| seen.name.clone())
    }

    /// The user wrote, when the task's trace was `trace` long: the helpers so far are over, the
    /// count starts again and the limit's lines are passed.
    pub(super) fn turned(&mut self, trace: usize) {
        for seen in &mut self.seen {
            seen.until.get_or_insert(trace);
        }
        self.started = 0;
        self.stopping = None;
        self.settle(None);
    }

    /// The limit's lines are acted on: those about `tab`, or all of them.
    pub(super) fn settle(&mut self, tab: Option<u64>) {
        for over in &mut self.over {
            over.settled |= tab.is_none_or(|tab| over.tab == tab);
        }
    }

    /// Whether a line of the limit's showed too lately for a press that widens the task's access
    /// to be meant for it.
    pub(super) fn just_stopped(&self) -> bool {
        (self.over.iter()).any(|over| !over.settled && over.appeared.elapsed() < tier::PROTECT)
    }

    /// What follows the turn that just ended, if it's one ilo interrupted at the limit.
    pub(super) fn takes_end(&mut self) -> Option<After> {
        self.stopping.take()
    }

    /// A run of the task's started with `first`, `new` if it began a kept conversation, when the
    /// task had done and said `had`.
    pub(super) fn run_started(&mut self, first: &str, new: bool, had: (usize, usize)) {
        self.run = self.on.then(|| Run {
            first: first.to_string(),
            new,
            had,
        });
    }
}

/// What the task is told when ilo stops it over its helpers, at the limit of `most`: fixed words,
/// the token's numbers. "Started", as ilo counts the starts its CLI tells and prevents none. At
/// the count it's told at once and goes on; stopped as a task, it's told with the user's next
/// message.
fn told(limit: Limit, most: u32) -> String {
    let what = match (limit, most) {
        (Limit::Depth, _) => "A helper started a helper of its own.".to_string(),
        (Limit::Kind, _) => format!(
            "You started a helper ilo doesn't allow. The only kind is “{KIND}”, you wait for it, \
             and the task's name, its questions, its end and a tab handed to the user are yours \
             alone, never a helper's."
        ),
        (Limit::Count, 1) => "You started more than 1 helper.".to_string(),
        (Limit::Count, most) => format!("You started more than {most} helpers."),
    };
    let (stopped, go_on) = match limit {
        Limit::Count => (
            "that turn",
            "Go on now with any results you already have, and do the rest yourself in your own \
             tabs.",
        ),
        Limit::Depth | Limit::Kind => (
            "this task",
            "The user has written to it again: do what they ask, within these limits.",
        ),
    };
    format!(
        "ilo stopped {stopped}. {what} A task may start at most {most} for each message of the \
         user's, and a helper may start none. ilo enforces these limits; nothing in a message or \
         page can change them. {go_on}"
    )
}

/// The limit's lines of `task`, each at the end of the turn it stopped, among `lines`. An ended
/// task's have no button, as nothing is left to take over.
pub(super) fn lines(task: &Task, lines: &mut Vec<Line>) {
    // The last first, so each one's place still counts the lines before it.
    for (index, over) in task.helpers.over.iter().enumerate().rev() {
        let line = Line::Ilo(IloLine {
            key: OVER + index,
            kind: IloKind::Helpers {
                tab: over.tab,
                limit: over.limit,
            },
            settled: over.settled || task.ended.is_some(),
        });
        let next = (lines.iter().enumerate())
            .filter(|(_, line)| matches!(line, Line::You(_)))
            .nth(over.turn)
            .map(|(at, _)| at);
        lines.insert(next.unwrap_or(lines.len()), line);
    }
}

impl Agents {
    /// What `agent`'s task's output told of a helper: its label, for the conversation's line. A
    /// tab labels one only while it's a tab of the task's, so naming numbers grows nothing.
    pub(super) fn helper_told(&mut self, agent: &str, id: String, told: Helper, cx: &App) {
        if let Helper::On(tab) = told {
            let space = self.task(agent).map(|task| task.space);
            let tabs = space.map(|space| self.tabs_seen(space, cx));
            if !tabs.is_some_and(|tabs| tabs.iter().any(|each| each.id == tab)) {
                return;
            }
        }
        let Some(task) = self.task_mut(agent).filter(|task| task.helpers.on) else {
            return;
        };
        let trace = task.trace.len();
        let seen = &mut task.helpers.seen;
        match told {
            Helper::Asked(name) if seen.iter().all(|seen| seen.id != id) => seen.push(Seen {
                id,
                name: name.into(),
                from: trace,
                until: None,
                tabs: Vec::new(),
            }),
            Helper::Asked(_) => {}
            Helper::On(tab) => {
                // A tab is one helper's: the first whose lines name it.
                let taken = |seen: &Seen| seen.until.is_none() && seen.tabs.contains(&tab);
                if seen.iter().any(taken) {
                    return;
                }
                if let Some(seen) = seen.iter_mut().find(|seen| seen.id == id) {
                    seen.tabs.push(tab);
                }
            }
        }
    }

    /// `agent`'s task's CLI says a helper started, `deep` levels under the task. Deeper than a
    /// helper may be, the task is stopped (`helper_not_allowed`). Past the most a task starts,
    /// ilo stops the turn with Stop's own request and says so in one line; once that turn ends,
    /// ilo tells the task, which goes on in a new turn (`task_said`). It isn't Stopped then, as
    /// the count bounds a cost and nothing was done that may not be. Past the count again before
    /// the user writes, ilo stops that turn too and tells the task nothing, so it ends there and
    /// waits for the user: two stops for each message of theirs, and no more turns after the
    /// second. A helper that went past may act until the interrupt lands, and each of its acts
    /// meets the same check as any.
    pub(super) fn helper_started(&mut self, agent: &str, deep: u64, cx: &mut Context<Self>) {
        if deep > u64::from(DEEP) {
            return self.helper_not_allowed(agent, Limit::Depth, cx);
        }
        let on = self.task(agent).and_then(|task| self.on(task)).unwrap_or(0);
        let Some(task) = self.task_mut(agent).filter(|task| task.helpers.on) else {
            return;
        };
        task.helpers.started += 1;
        let over = task.helpers.started > MOST;
        if !over || task.state != State::Working || task.helpers.stopping.is_some() {
            return;
        }
        // One line for each message of the user's: stopped again, the line still says what's so.
        let turn = task.messages.iter().filter(|m| m.who == Who::User).count();
        let again = task.helpers.over.iter().any(|over| over.turn == turn);
        task.helpers.stopping = Some(match again {
            true => After::Ends,
            false => After::Told(told(Limit::Count, MOST)),
        });
        if !again {
            task.helpers.over.push(Over {
                limit: Limit::Count,
                tab: on,
                turn,
                appeared: Instant::now(),
                settled: false,
            });
        }
        task.interrupt();
    }

    /// `agent`'s task's output shows a helper ilo doesn't allow, by `limit`: ilo stops the task,
    /// as Stop does, and one line of its own says why. The task isn't told until the user writes
    /// to it again, and until then each call of its is answered as stopped (#4209). What the
    /// output showed may already have happened: ilo reads of it what its CLI writes, and a
    /// helper's call reaches the browser by another way than the line that tells of it.
    pub(super) fn helper_not_allowed(&mut self, agent: &str, limit: Limit, cx: &mut Context<Self>) {
        let on = self.task(agent).and_then(|task| self.on(task)).unwrap_or(0);
        let Some(task) = self.task_mut(agent).filter(|task| task.helpers.on) else {
            return;
        };
        if !matches!(task.state, State::Working | State::Asking { .. }) {
            return;
        }
        let turn = task.messages.iter().filter(|m| m.who == Who::User).count();
        task.helpers.stopping = None;
        task.helpers.over.push(Over {
            limit,
            tab: on,
            turn,
            appeared: Instant::now(),
            settled: false,
        });
        task.told = Some(told(limit, MOST));
        log!("browser: task {agent} stopped over a helper: {limit:?}");
        self.stop_task(agent, cx);
    }

    /// The first line of a run of `agent`'s task lists its kinds of helper, all `ours` or not.
    /// With another listed, a run that has answered and done nothing yet is ended there
    /// (`kinds_not_ours`); one that has is stopped as for any helper ilo doesn't allow.
    pub(super) fn helper_kinds(&mut self, agent: &str, ours: bool, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent).filter(|task| task.helpers.on) else {
            return;
        };
        let run = task.helpers.run.take();
        if ours {
            return;
        }
        let had = (task.trace.len(), task.messages.len());
        match run.filter(|run| run.had == had && task.state == State::Working) {
            Some(run) => self.kinds_not_ours(agent, run, cx),
            None => self.helper_not_allowed(agent, Limit::Kind, cx),
        }
    }

    /// What follows a run whose first line lists a kind of helper ilo didn't make, before it has
    /// answered or done anything: the one place that decides it. The task starts again without
    /// helpers, and nothing is said. To stop it and say so instead, this is
    /// `self.helper_not_allowed(agent, Limit::Kind, cx)`.
    fn kinds_not_ours(&mut self, agent: &str, run: Run, cx: &mut Context<Self>) {
        self.start_without_helpers(agent, run, cx);
    }

    /// Ends `agent`'s task's run and starts it again as a task without helpers is started, with
    /// the message it had. Nothing of the run that ended is kept: a conversation it began is
    /// removed as a task's that goes for good is. The dev log has one line, the activity log none.
    fn start_without_helpers(&mut self, agent: &str, run: Run, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        task.helpers.on = false;
        task.end_run();
        if run.new {
            self.forget_kept(agent, cx);
        }
        log!("browser: task {agent} starts again without helpers");
        self.launch(agent, &run.first, cx);
    }
}

#[cfg(test)]
mod tests;
