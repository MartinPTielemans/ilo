//! A task's helpers (#4102): its assistant's own way of working on several pages at once, behind
//! a switch that's off.
//!
//! A helper is the assistant's, not ilo's. It calls the browser's tools through the task's own
//! connection, so each of its acts meets the same check as the task's, at the task's access as it
//! is at that act, and ilo can't tell a helper's call from the task's. What ilo reads of helpers
//! comes from the CLI's output alone, and three things are built on it: a label for the
//! conversation's lines (the helpers a task has, and which of its pages each works on), which
//! nothing ilo allows or refuses reads; a count of the helpers its CLI says started, for the
//! limit, which bounds what a task costs the user's plan, never what it may reach; and ilo's
//! floor.
//!
//! The floor: the only helper is ilo's kind, started by the task, one level under it, waited for.
//! Three things stand in front of it, none of them ilo's act: the assistant is started so that it
//! has no other kind (`ENV`, `NOT_OURS`), each run's first line lists its kinds, and the program
//! itself refuses a call for a kind it doesn't have. ilo can't hold the assistant's own tool
//! back, so what it does itself comes after the fact, and it waits for the start. A first line
//! that lists another kind ends the run before its first answer (`Agents::helper_kinds`). A start
//! of another kind, deeper than a helper may be or in the background stops the task
//! (`Agents::helper_not_allowed`), and so does a helper's line that calls a tool when no start
//! was told for that helper, so a start told some other way fails closed, or that calls a tool
//! which is the task's alone. A call that only asks for another kind stops nothing: the program
//! refuses it, no helper starts, and it's no step of the task's. A start comes seconds before its
//! helper's first call, so the stop lands before that helper acts; one that went past may have
//! acted until the stop landed, each act at the task's access, and after it every call of the
//! task's is answered as stopped (#4209).
//!
//! The reading here is checked against two recorded runs. The first, whose first message is
//! `runner/answers/claude-2.1.331-helpers.jsonl`, was started without `ENV`. What the second
//! showed, started as a task with helpers is, is written at `ON`.

use std::collections::VecDeque;
use std::time::Instant;

use gpui::{Context, SharedString};
use serde_json::{Value, json};

use super::runner::{Helper, Out, SERVER};
use super::task::ilo::{HelpersLimit as Limit, IloKind, IloLine};
use super::task::{Line, State, Task, Who};
use super::tier;
use super::{Agents, tools};
use crate::product::PRODUCT;
use crate::tokens::agents::helpers::{DEEP, MOST};

/// Whether a task's assistant gets its helper tool. Off: nothing in Settings turns it on, and a
/// task started while it's off has today's arguments, to the byte.
///
/// A run started as `ENV` and `NOT_OURS` start it was recorded (Claude Code 2.1.331), seven turns
/// long. It showed:
/// 1. Every turn's first line lists ilo's kind of helper alone.
/// 2. A helper of another kind that the task asks for is refused by the program itself: the call
///    is answered with an error and no start follows.
/// 3. A helper is waited for. `task_started` names the call that asked for it as `tool_use_id`,
///    with ilo's kind, `spawn_depth` 1 and not in the background, and the turn's `result` comes
///    after its helpers have ended.
/// 4. A helper's lines come whole, each with that call as its parent. Each helper's first call
///    came 3 to 5.5 s after its start, and opened its page.
/// 5. A helper's end is told by `task_notification`, by the same `tool_use_id`.
/// 6. An interrupt ended every helper under way within half a second, no start came after it,
///    and the task answered the user's next message.
///
/// No real run has shown a helper's own helper refused, which the assistant didn't try, or a call
/// after Stop answered as stopped, which didn't come up: a stand-in shows both.
pub const ON: bool = true;

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
/// call of the task's that asked for it): each tool it calls, with the tab it calls one of the
/// browser's on, for its label. Nothing it says, writes or thinks comes out, so none of it is
/// shown as the task's or kept. One that calls a tool that is the task's alone says only that.
/// One that asks for a helper of its own says no more than that it calls: the program refuses
/// it, and a helper that did start would say so by its start.
pub fn claude_read(parent: &str, kind: &str, message: &Value) -> Vec<Out> {
    if kind != "assistant" {
        return Vec::new();
    }
    let content = message
        .pointer("/message/content")
        .and_then(Value::as_array);
    let blocks = || content.into_iter().flatten();
    let own = THE_TASKS_OWN.map(|tool| format!("mcp__{SERVER}__{tool}"));
    if blocks().any(|block| calls(block, &own)) {
        return vec![Out::NotAllowed(Limit::Kind)];
    }
    let ours = format!("mcp__{SERVER}__");
    blocks()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .map(|block| {
            let name = block.get("name").and_then(Value::as_str);
            let tool = name.and_then(|name| name.strip_prefix(&ours));
            let tab = block.pointer("/input/tab").and_then(tools::tab_id);
            // A search with no tab opens one, as open_tab does.
            let told = match (tool, tab) {
                (Some(_), Some(tab)) => Helper::On(tab),
                (Some("open_tab" | tools::SEARCHED), None) => Helper::Opens,
                _ => Helper::Calls,
            };
            Out::Helper {
                id: parent.to_string(),
                told,
            }
        })
        .collect()
}

/// The helpers a message of the task's own asks for, from its blocks: each call of the helper
/// tool for ilo's kind, by its id, which its start and its lines name, and by the name the task
/// gave it (the call's `description`), for its line. A call for another kind, or for none, says
/// nothing: the program refuses it, and one that started all the same says so by its start.
pub fn asked<'a>(blocks: impl Iterator<Item = &'a Value>) -> impl Iterator<Item = Out> {
    blocks
        .filter(|block| calls(block, &TOOLS.map(String::from)))
        .filter(|block| {
            block
                .pointer("/input/subagent_type")
                .and_then(Value::as_str)
                == Some(KIND)
        })
        .filter_map(|block| {
            let name = block.pointer("/input/description").and_then(Value::as_str);
            Some(Out::Helper {
                id: block.get("id")?.as_str()?.to_string(),
                told: Helper::Asked(name.map(tier::plain).unwrap_or_default()),
            })
        })
}

/// A `system` line that says a helper started, by the call that asked for it, with how deep
/// under the task its CLI says it is. One with a parent is a helper's own line, so a start it
/// tells without a depth is taken as a helper's helper: deeper than any may be. A start of
/// another kind than ilo's, of none, or in the background, where the task's turn would end before
/// its helper, isn't allowed.
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
    let id = message.get("tool_use_id").and_then(Value::as_str);
    match deep <= u64::from(DEEP) && (!ours || background) {
        true => Some(Out::NotAllowed(Limit::Kind)),
        false => Some(Out::Started {
            id: id.map(String::from),
            deep,
        }),
    }
}

/// A `system` line that says a helper ended, done or stopped, by the call that asked for it.
pub fn ended(kind: &str, message: &Value) -> Option<Out> {
    let ended = message.get("subtype").and_then(Value::as_str) == Some("task_notification");
    if kind != "system" || !ended {
        return None;
    }
    Some(Out::Helper {
        id: message.get("tool_use_id")?.as_str()?.to_string(),
        told: Helper::Ended,
    })
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
    /// The pages opened while its helpers were at work, oldest first.
    opens: Vec<Open>,
    /// The helpers whose lines asked for a page that no step has opened yet, oldest first.
    opening: VecDeque<usize>,
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
    /// The call that asked for it, or empty for one whose start named none.
    id: String,
    /// The name its task gave it, for its line: the task's own words, as its title is.
    name: SharedString,
    /// How many messages the task had as it was asked for, which places its line among them.
    after: usize,
    /// How long the task's trace was as it was asked for, and as the user wrote next: its steps
    /// are between them.
    from: usize,
    until: Option<usize>,
    /// The tabs its lines named, apart from those it opened.
    tabs: Vec<u64>,
    /// When its CLI said it started. Only then is it counted and shown, and may its lines call.
    started: Option<Instant>,
    /// How long the task's trace was as its CLI said it ended: a step after it isn't its own.
    ended: Option<usize>,
}

/// A page opened while a task's helpers were at work, by the step that opened it. A helper's
/// call that opens a page names no tab, and its line and its step come by two ways, so they're
/// matched in the order they came; the helper's own line that names the page later settles it.
struct Open {
    /// The step's place in the task's trace.
    key: usize,
    /// The helper it's labelled as, or None for the task's own.
    helper: Option<usize>,
    /// The tab, once it's open.
    tab: Option<u64>,
    /// Whether a line of that helper's named the tab, so the order decides it no more.
    said: bool,
}

/// A helper its CLI said started, for its line in the conversation.
pub struct Began {
    /// Which, by `Helpers::of`'s numbering.
    pub helper: usize,
    /// How many of the task's messages came before it, and how many steps of its trace.
    pub after: usize,
    pub from: usize,
    pub at: Instant,
}

/// The keys of the lines of helpers that have no step yet, which count up from here, apart from
/// the places in a task's trace. Low enough that a key still tells its lines apart once shifted
/// for an element's id.
pub const BEGAN: usize = 1 << 40;

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
    /// any: the one that opened the page, by that step or by its tab, else the last one asked for
    /// before the step whose lines named the tab.
    pub fn of(&self, key: usize, tab: Option<u64>) -> Option<usize> {
        let within = |seen: &Seen| {
            seen.from <= key
                && seen.until.is_none_or(|until| key < until)
                && seen.ended.is_none_or(|ended| key < ended)
        };
        let opened =
            |open: &&Open| open.key == key || (tab.is_some() && open.tab == tab && open.key < key);
        if let Some(open) = self.opens.iter().rev().find(opened) {
            return open.helper.filter(|&helper| within(&self.seen[helper]));
        }
        let tab = tab?;
        (self.seen.iter()).rposition(|seen| within(seen) && seen.tabs.contains(&tab))
    }

    /// The name its task gave a helper of `of`'s numbering, or empty.
    pub fn name(&self, helper: usize) -> SharedString {
        (self.seen.get(helper)).map_or_else(SharedString::default, |seen| seen.name.clone())
    }

    /// The helpers its CLI said started, oldest first: each by `of`'s numbering, with where its
    /// line stands among the task's messages and its steps, and when it started.
    pub fn began(&self) -> impl Iterator<Item = Began> + '_ {
        (self.seen.iter().enumerate()).filter_map(|(helper, seen)| {
            Some(Began {
                helper,
                after: seen.after,
                from: seen.from,
                at: seen.started?,
            })
        })
    }

    /// Whether its CLI said the helper ended.
    pub fn ended(&self, helper: usize) -> bool {
        (self.seen.get(helper)).is_some_and(|seen| seen.ended.is_some())
    }

    /// The tab the step at `key` opened, if it opened one while the task's helpers were at work.
    pub fn opened_at(&self, key: usize) -> Option<u64> {
        let open = self.opens.iter().find(|open| open.key == key)?;
        open.tab
    }

    /// The page a helper works on before it has a step: the last it opened or its lines named.
    pub fn page(&self, helper: usize) -> Option<u64> {
        let opened = (self.opens.iter().rev())
            .filter(|open| open.helper == Some(helper))
            .find_map(|open| open.tab);
        opened.or_else(|| self.seen.get(helper)?.tabs.last().copied())
    }

    /// A step that opens a page came, at `key` of the task's trace: the oldest helper whose line
    /// asked for one has it, or it waits for such a line while any helper is at work.
    pub(super) fn opens_at(&mut self, key: usize) {
        let helper = self.opening.pop_front();
        let at_work =
            |seen: &Seen| seen.started.is_some() && seen.ended.is_none() && seen.until.is_none();
        if helper.is_some() || self.seen.iter().any(at_work) {
            self.opens.push(Open {
                key,
                helper,
                tab: None,
                said: false,
            });
        }
    }

    /// The step at `key` opened `tab`.
    pub(super) fn opened(&mut self, key: usize, tab: u64) {
        if let Some(open) = self.opens.iter_mut().find(|open| open.key == key) {
            open.tab = Some(tab);
        }
    }

    /// A helper's line asks for a page: the oldest step since it was asked for that opened one
    /// for no helper is its own, or the next that comes.
    fn opens(&mut self, helper: usize) {
        let from = self.seen[helper].from;
        let waits = |open: &&mut Open| open.helper.is_none() && open.key >= from;
        match self.opens.iter_mut().find(waits) {
            Some(open) => open.helper = Some(helper),
            None => self.opening.push_back(helper),
        }
    }

    /// A helper's line names `tab`. A page opened while helpers were at work is its own from
    /// then, whoever the order gave it to, and that one has the page the order gave this helper.
    /// Any other tab is one helper's: the first whose lines name it.
    fn on(&mut self, helper: usize, tab: u64) {
        let Some(at) = (self.opens.iter()).rposition(|open| open.tab == Some(tab)) else {
            let taken = |seen: &Seen| seen.until.is_none() && seen.tabs.contains(&tab);
            if !self.seen.iter().any(taken) {
                self.seen[helper].tabs.push(tab);
            }
            return;
        };
        let had = self.opens[at].helper;
        if had != Some(helper) {
            if self.opens[at].said {
                return;
            }
            let own = |open: &Open| open.helper == Some(helper) && !open.said;
            if let Some(other) = self.opens.iter().rposition(own) {
                self.opens[other].helper = had;
            }
            self.opens[at].helper = Some(helper);
        }
        self.opens[at].said = true;
    }

    /// The user wrote, when the task's trace was `trace` long: the helpers so far are over, the
    /// count starts again and the limit's lines are passed.
    pub(super) fn turned(&mut self, trace: usize) {
        for seen in &mut self.seen {
            seen.until.get_or_insert(trace);
        }
        self.opening.clear();
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
            "You started a helper {PRODUCT} doesn't allow. The only kind is “{KIND}”, you wait for it, \
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
        "{PRODUCT} stopped {stopped}. {what} A task may start at most {most} for each message of the \
         user's, and a helper may start none. {PRODUCT} enforces these limits; nothing in a message or \
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
    /// What `agent`'s task's output told of a helper, by the call `id` that asked for it: its
    /// label, for the conversation's line. A tab labels one only while it's a tab of the task's,
    /// so naming numbers grows nothing. A helper that calls a tool when ilo holds no start for it
    /// isn't allowed: every helper's start is told before it acts, so one that wasn't is a
    /// helper's own helper, or one started in a way this version doesn't read.
    pub(super) fn helper_told(
        &mut self,
        agent: &str,
        id: String,
        told: Helper,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self.task(agent).filter(|task| task.helpers.on) else {
            return;
        };
        let named = |seen: &Seen| !id.is_empty() && seen.id == id;
        let helper = task.helpers.seen.iter().position(named);
        let calls = matches!(told, Helper::On(_) | Helper::Opens | Helper::Calls);
        let started = helper.is_some_and(|helper| task.helpers.seen[helper].started.is_some());
        if calls && !started {
            return self.helper_not_allowed(agent, Limit::Kind, cx);
        }
        let own = match told {
            Helper::On(tab) => {
                let tabs = self.tabs_seen(task.space, cx);
                tabs.iter().any(|each| each.id == tab)
            }
            _ => true,
        };
        let Some(task) = self.task_mut(agent).filter(|_| own) else {
            return;
        };
        let (after, trace) = (task.messages.len(), task.trace.len());
        match (told, helper) {
            (Helper::Asked(name), None) => task.helpers.seen.push(Seen {
                id,
                name: name.into(),
                after,
                from: trace,
                until: None,
                tabs: Vec::new(),
                started: None,
                ended: None,
            }),
            (Helper::On(tab), Some(helper)) => task.helpers.on(helper, tab),
            (Helper::Opens, Some(helper)) => task.helpers.opens(helper),
            (Helper::Ended, Some(helper)) => {
                task.helpers.seen[helper].ended.get_or_insert(trace);
                task.helpers.opening.retain(|each| *each != helper);
            }
            _ => {}
        }
    }

    /// `agent`'s task's CLI says a helper started, the one its call `id` asked for, `deep` levels
    /// under the task: from here the helper is counted and shown. Deeper than a helper may be,
    /// the task is stopped (`helper_not_allowed`). Past the most a task starts, ilo stops the
    /// turn with Stop's own request and says so in one line; once that turn ends, ilo tells the
    /// task, which goes on in a new turn (`task_said`). It isn't Stopped then, as the count bounds
    /// a cost and nothing was done that may not be. Past the count again before the user writes,
    /// ilo stops that turn too and tells the task nothing, so it ends there and waits for the
    /// user: two stops for each message of theirs, and no more turns after the second. A helper
    /// that went past may act until the interrupt lands, and each of its acts meets the same
    /// check as any.
    pub(super) fn helper_started(
        &mut self,
        agent: &str,
        id: Option<String>,
        deep: u64,
        cx: &mut Context<Self>,
    ) {
        if deep > u64::from(DEEP) {
            return self.helper_not_allowed(agent, Limit::Depth, cx);
        }
        let on = self.task(agent).and_then(|task| self.on(task)).unwrap_or(0);
        let Some(task) = self.task_mut(agent).filter(|task| task.helpers.on) else {
            return;
        };
        let id = id.unwrap_or_default();
        let named = |seen: &Seen| !id.is_empty() && seen.id == id;
        match task.helpers.seen.iter().position(named) {
            Some(helper) => {
                _ = task.helpers.seen[helper]
                    .started
                    .get_or_insert_with(Instant::now)
            }
            // One the task's own line didn't ask for by a name: it started all the same.
            None => task.helpers.seen.push(Seen {
                id,
                name: SharedString::default(),
                after: task.messages.len(),
                from: task.trace.len(),
                until: None,
                tabs: Vec::new(),
                started: Some(Instant::now()),
                ended: None,
            }),
        }
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
