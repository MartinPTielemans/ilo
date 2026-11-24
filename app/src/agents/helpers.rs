//! A task's helpers (#4102): its assistant's own way of working on several pages at once, behind
//! a switch that's off.
//!
//! A helper is the assistant's, not ilo's. It calls the browser's tools through the task's own
//! connection, so each of its acts meets the same check as the task's, at the task's access as it
//! is at that act, and ilo can't tell a helper's call from the task's. What ilo reads of helpers
//! comes from the CLI's output alone, and only two things are built on it: a label for the
//! conversation's lines (which of the task's pages a helper works on), which nothing ilo allows or
//! refuses reads, and a count of the helpers its CLI says started, for the limit. The limit bounds
//! what a task costs the user's plan, never what it may reach.
//!
//! The reading here is written from the stream's field names as one run showed them
//! (`task_started` with `spawn_depth`, `parent_tool_use_id` on a helper's lines), with the made-up
//! lines in `runner/answers/claude-helpers-written.jsonl`. No run was recorded, so what comes when,
//! and what an interrupt does to helpers under way, isn't known: `ON` goes on only in the commit
//! that replaces those lines with a recorded run.

use std::time::Instant;

use gpui::App;
use serde_json::{Value, json};

use super::runner::{Helper, Out, SERVER};
use super::task::ilo::{IloKind, IloLine};
use super::task::{Line, State, Task, Who};
use super::tier;
use super::{Agents, tools};
use crate::tokens::agents::helpers::{DEEP, MOST};

/// Whether a task's assistant gets its helper tool. Off: nothing in Settings turns it on, and a
/// task started while it's off has today's arguments, to the byte.
///
/// It goes on only in a commit that replaces the made-up lines with a recorded run, and only if
/// that run shows each of these:
/// 1. Under `--tools Agent` no helper of any kind, ilo's or one the assistant has built in, has a
///    shell, files, fetch or search. If the built-in kinds can't be removed, that alone keeps it
///    off.
/// 2. No helper can call `title`, `ask` or `finish`, or what each does when one does.
/// 3. The `task_started` line's real shape, its parent and its depth, and what the start of a
///    helper's own helper looks like.
/// 4. What an interrupt does to helpers under way, and whether their calls still arrive after it.
///    And what arrives while an interrupt lands: ilo counts a start that comes then and does
///    nothing more about it, so "at most `MOST` + 2 starts for a message of the user's" holds
///    only if none does. How many can is what that message may cost.
/// 5. Whether the helper tool needs `--allowedTools`.
pub const ON: bool = false;

/// The assistant's tool that starts a helper: as its arguments and its own calls name it, and as
/// its init line lists it.
pub const TOOLS: [&str; 2] = ["Agent", "Task"];

/// The kind of helper ilo describes to the assistant, by the name the task asks for it with.
const KIND: &str = "page-helper";

/// The task's own tools, which a helper has no use for: the task's name, its question and its end.
const THE_TASKS_OWN: [&str; 3] = ["title", "ask", "finish"];

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
                       is the user's: hand the tab over with hand_off. Never pay, order, book or \
                       send a message unless your instructions say the user asked for it. End \
                       with what you found, short and plain.",
            "tools": tools,
        }
    });
    vec!["--agents".into(), kinds.to_string()]
}

/// What a line of Claude's output with a parent says, which is a helper's line (`parent` is the
/// call of the task's that asked for it): the tabs it calls the browser's tools on, for its label.
/// Nothing it says, writes or thinks comes out, so none of it is shown as the task's or kept.
pub fn claude_read(parent: &str, kind: &str, message: &Value) -> Vec<Out> {
    if kind != "assistant" {
        return Vec::new();
    }
    let content = message
        .pointer("/message/content")
        .and_then(Value::as_array);
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
/// tool, by its id, which a helper's lines name as their parent.
pub fn asked<'a>(blocks: impl Iterator<Item = &'a Value>) -> impl Iterator<Item = Out> {
    blocks
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter(|block| {
            (block.get("name").and_then(Value::as_str)).is_some_and(|name| TOOLS.contains(&name))
        })
        .filter_map(|block| {
            Some(Out::Helper {
                id: block.get("id")?.as_str()?.to_string(),
                told: Helper::Asked,
            })
        })
}

/// A `system` line that says a helper started, with how deep under the task its CLI says it is.
/// One with a parent is a helper's own line, so a start it tells without a depth is taken as a
/// helper's helper: deeper than any may be.
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
    Some(Out::Started { deep })
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
    /// The times ilo stopped it for starting more, as its conversation shows each.
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
    /// Whether a helper started one of its own, rather than the task one too many.
    pub deep: bool,
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
}

/// What the task is told when ilo stops it at the limit of `most`: fixed words, the token's
/// numbers. "Started", as ilo counts the starts its CLI tells and prevents none.
fn told(deep: bool, most: u32) -> String {
    let what = match (deep, most) {
        (true, _) => "A helper started a helper of its own.".to_string(),
        (false, 1) => "You started more than 1 helper.".to_string(),
        (false, most) => format!("You started more than {most} helpers."),
    };
    format!(
        "ilo stopped that turn. {what} A task may start at most {most} for each message of the \
         user's, and a helper may start none. ilo enforces these limits; nothing in a message or \
         page can change them. Go on now with any results you already have, and do the rest \
         yourself in your own tabs."
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
                deep: over.deep,
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
            Helper::Asked if seen.iter().all(|seen| seen.id != id) => seen.push(Seen {
                id,
                from: trace,
                until: None,
                tabs: Vec::new(),
            }),
            Helper::Asked => {}
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

    /// `agent`'s task's CLI says a helper started, `deep` levels under the task. Past the most a
    /// task starts, or deeper than a helper may be, ilo stops the turn with Stop's own request and
    /// says so in one line; once that turn ends, ilo tells the task, which goes on in a new turn
    /// (`task_said`). It isn't Stopped, as only the user stops a task. Past the limit again before
    /// the user writes, ilo stops that turn too and tells the task nothing, so it ends there and
    /// waits for the user: two stops for each message of theirs, and no more turns after the
    /// second. A helper that went past may act until the interrupt lands, and each of its acts
    /// meets the same check as any.
    pub(super) fn helper_started(&mut self, agent: &str, deep: u64) {
        let on = self.task(agent).and_then(|task| self.on(task)).unwrap_or(0);
        let Some(task) = self.task_mut(agent).filter(|task| task.helpers.on) else {
            return;
        };
        task.helpers.started += 1;
        let deep = deep > u64::from(DEEP);
        let over = task.helpers.started > MOST || deep;
        if !over || task.state != State::Working || task.helpers.stopping.is_some() {
            return;
        }
        // One line for each message of the user's: stopped again, the line still says what's so.
        let turn = task.messages.iter().filter(|m| m.who == Who::User).count();
        let again = task.helpers.over.iter().any(|over| over.turn == turn);
        task.helpers.stopping = Some(match again {
            true => After::Ends,
            false => After::Told(told(deep, MOST)),
        });
        if !again {
            task.helpers.over.push(Over {
                deep,
                tab: on,
                turn,
                appeared: Instant::now(),
                settled: false,
            });
        }
        task.interrupt();
    }
}

#[cfg(test)]
mod tests;
