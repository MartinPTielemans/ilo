//! A thread's strands (#4102), which the assistant is told are its helpers: its way of working on
//! several pages at once, with the browser's `strand` tool. A strand is a run of the thread's
//! assistant of its own, which ilo starts, counts and ends itself. It has its own token and
//! connection, the thread's tools but those that are the thread's own (`tools::THE_THREADS_OWN`),
//! and its calls are its thread's: in the thread's tabs, at the thread's access at each act, so
//! the user lowering the thread lowers its strands at once. Its last message answers the call
//! that started it, and it ends with its turn, or with its thread.
//!
//! What ilo holds of them: the labels of the conversation's lines (each strand's pages), the
//! count that bounds what a thread costs the user's plan, refused at the call past `MOST`, and
//! the lines of the times ilo stopped a thread over them. A strand can't start one of its own,
//! or call a thread's own tool: it has neither, and a call of one stops the thread.

use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui::{Context, SharedString};
use serde_json::Value;

use super::runner::{self, Failure, Out, Process};
use super::task::ilo::{HelpersLimit as Limit, IloKind, IloLine};
use super::task::{Line, State, Task, Who};
use super::{Agents, Call, ToolResult, tier, tools};
use crate::product::PRODUCT;
use crate::tokens::agents::helpers::MOST;

/// The kind of helper the task was told of, by the name it asked for it with, which what it's
/// told when stopped still names.
const KIND: &str = "page-helper";

/// What ilo holds of a task's helpers, in memory only: nothing of it is logged or kept.
#[derive(Default)]
pub struct Helpers {
    /// Whether the task has helpers at all, which none has now.
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
    /// The times ilo stopped it after it started more, as its conversation shows each.
    pub(super) over: Vec<Over>,
    /// Its strands under way.
    strands: Vec<Strand>,
}

/// A strand under way: a run of its own, whose last message answers its thread's call.
struct Strand {
    /// Its id, task-N-K, which its calls and its run's lines come by.
    agent: String,
    /// Which helper it is, by `Helpers::of`'s numbering.
    helper: usize,
    token: String,
    /// Its run, once started.
    process: Option<Process>,
    /// The thread's call that started it, until it's answered.
    reply: Option<mpsc::Sender<ToolResult>>,
    /// What it said last, which answers the call.
    said: Option<String>,
    /// Whether it has called or said anything yet: until then it has NO_ANSWER from its start,
    /// as its thread has while it thinks (`Agents::watch_strands`).
    heard: bool,
    /// When it started, by the executor's clock, which its thread's timer goes by.
    since: Instant,
}

impl Drop for Strand {
    /// A strand that goes unanswered is answered as stopped, so no call of its thread's waits.
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            _ = reply.send(ToolResult::failed(tools::Kind::Stopped, super::STOPPED));
        }
    }
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
    /// Whether a strand is under way, which its thread's call waits on.
    pub(super) fn has_strands(&self) -> bool {
        !self.strands.is_empty()
    }

    /// The start of the strand longest without a call or a word, if one is under way.
    pub(super) fn unheard_since(&self) -> Option<Instant> {
        (self.strands.iter())
            .filter(|each| !each.heard)
            .map(|each| each.since)
            .min()
    }

    /// Ends each strand that hasn't called or said anything in `most` since it started, by
    /// `now`, at `trace`, its call answered with `why`; whether any did.
    pub(super) fn end_unheard(
        &mut self,
        (now, most): (Instant, Duration),
        why: &str,
        trace: usize,
    ) -> bool {
        let due = |each: &Strand| !each.heard && now.duration_since(each.since) >= most;
        let (due, going): (Vec<Strand>, Vec<Strand>) =
            std::mem::take(&mut self.strands).into_iter().partition(due);
        self.strands = going;
        let ended = !due.is_empty();
        for mut strand in due {
            log!(
                "browser: strand {} had no answer in {} s",
                strand.agent,
                most.as_secs()
            );
            self.seen[strand.helper].ended.get_or_insert(trace);
            if let Some(reply) = strand.reply.take() {
                _ = reply.send(ended_badly(why));
            }
        }
        ended
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

    /// Whether the strand `agent` runs, which only then may call.
    pub(super) fn runs(&self, agent: &str) -> bool {
        (self.strands.iter()).any(|strand| strand.agent == agent && strand.process.is_some())
    }

    /// The token of the strand `agent`.
    pub(super) fn token_of(&self, agent: &str) -> Option<String> {
        let strand = self.strands.iter().find(|strand| strand.agent == agent)?;
        Some(strand.token.clone())
    }

    /// Each strand's token, with its id, which its calls come by.
    pub(super) fn tokens(&self) -> impl Iterator<Item = (String, String)> + '_ {
        (self.strands.iter()).map(|strand| (strand.token.clone(), strand.agent.clone()))
    }

    /// Its thread ended, when its trace was `trace` long: each strand ends with it, its run
    /// ended and its call answered as stopped.
    pub(super) fn end_strands(&mut self, trace: usize) {
        for strand in std::mem::take(&mut self.strands) {
            self.seen[strand.helper].ended.get_or_insert(trace);
        }
        self.opening.clear();
    }

    /// What follows the turn that just ended, if it's one ilo interrupted at the limit.
    pub(super) fn takes_end(&mut self) -> Option<After> {
        self.stopping.take()
    }
}

/// The thread's answer when its strand's run ended badly, for `reason` (AP29): what went wrong and
/// what to do next, as the model otherwise learns only that something broke.
fn ended_badly(reason: &str) -> ToolResult {
    ToolResult::error(format!("The helper ended: {reason} Do its part yourself."))
}

/// What went wrong with a strand's turn, for its thread's call: as the thread's own card would say
/// it, by the assistant's `name`.
fn failure_words(name: &str, cli: Option<runner::Cli>, failure: Failure) -> String {
    let name = cli.map_or(name, |cli| cli.name());
    match failure {
        Failure::Limit(_) => format!("{name} is at its usage limit."),
        Failure::SignedOut => format!("{name} isn't signed in."),
        Failure::NotInstalled => format!("{name} isn't installed."),
        Failure::NeedsUpdate => format!("{name} needs an update to work in {PRODUCT}."),
        Failure::Other(why) => why,
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
    /// The thread `agent` calls `strand`: a helper for `goal`, in `tab` if it names one. Past the
    /// most a thread starts for a message of the user's, the call is refused, nothing starts,
    /// and ilo stops the turn with Stop's own request and says so in one line; once that turn
    /// ends, ilo tells the thread, which goes on in a new turn (`task_said`). It isn't Stopped
    /// then, as the count bounds a cost and nothing was done that may not be. Past the count
    /// again before the user writes, ilo stops that turn too and tells the thread nothing, so it
    /// ends there and waits for the user: two stops for each message of theirs, and no more turns
    /// after the second.
    pub(super) fn strand(&mut self, call: Call, cx: &mut Context<Self>) {
        let Call {
            agent,
            connection,
            arguments,
            reply,
            ..
        } = call;
        let goal = (arguments.get("goal").and_then(Value::as_str))
            .map(str::trim)
            .unwrap_or_default();
        if goal.is_empty() {
            _ = reply.send(ToolResult::error("goal is what the helper is to do."));
            return;
        }
        let tab = match tools::tab_argument(&arguments) {
            Ok(tab) => tab,
            Err(not_a_tab) => {
                _ = reply.send(ToolResult::error(not_a_tab));
                return;
            }
        };
        let Some(task) = self.task_mut(&agent) else {
            return;
        };
        if task.helpers.started >= MOST {
            _ = reply.send(ToolResult::error(told(Limit::Count, MOST)));
            return self.past_most(&agent);
        }
        task.helpers.on = true;
        task.helpers.started += 1;
        let helper = task.helpers.seen.len();
        let strand = format!("{agent}-{}", helper + 1);
        task.helpers.seen.push(Seen {
            id: strand.clone(),
            name: tier::plain(goal).into(),
            after: task.messages.len(),
            from: task.trace.len(),
            until: None,
            tabs: tab.into_iter().collect(),
            started: Some(Instant::now()),
            ended: None,
        });
        // Its token is the server's before its run can connect with it.
        task.helpers.strands.push(Strand {
            agent: strand.clone(),
            helper,
            token: super::random_token(),
            process: None,
            reply: Some(reply),
            said: None,
            heard: false,
            since: cx.background_executor().now(),
        });
        self.tokens_changed();
        let first = match tab {
            Some(tab) => format!("{goal}\n\nWork in tab {tab}."),
            None => goal.to_string(),
        };
        let started = self.launch_strand(&agent, &strand, &first, cx);
        // Its start is its thread's entry in the activity log, naming the strand as its calls'
        // entries do, with its goal's first line cut as the assistant's own words are: one
        // entry for each strand, never a count.
        if started.is_ok() {
            let goal = super::page::clip(goal.lines().next().unwrap_or_default(), 60);
            self.as_strand(&strand, |agents| {
                agents.note_on(&agent, connection, tools::STRAND, "", &goal, "")
            });
        }
        let Some(task) = self.task_mut(&agent) else {
            return;
        };
        let trace = task.trace.len();
        let Some(at) = (task.helpers.strands.iter()).position(|each| each.agent == strand) else {
            return;
        };
        match started {
            Ok(process) => task.helpers.strands[at].process = Some(process),
            Err(why) => {
                let mut strand = task.helpers.strands.remove(at);
                task.helpers.seen[helper].ended = Some(trace);
                if let Some(reply) = strand.reply.take() {
                    _ = reply.send(ended_badly(&why));
                }
                self.tokens_changed();
            }
        }
        self.watch_strands(&agent, cx);
        cx.notify();
    }

    /// The thread `agent` called past the most it starts: ilo stops its turn and says so once for
    /// each message of the user's (`strand`).
    fn past_most(&mut self, agent: &str) {
        let on = self.task(agent).and_then(|task| self.on(task)).unwrap_or(0);
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        if task.state != State::Working || task.helpers.stopping.is_some() {
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

    /// The strand `strand` of the thread `thread` makes `call`, which is its thread's from here:
    /// the tab it names is its page, if it's one of the thread's, and a page it opens is its own.
    /// A label, which nothing ilo allows or refuses reads.
    pub(super) fn strand_calls(
        &mut self,
        thread: &str,
        strand: &str,
        call: &Call,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self.task(thread) else {
            return;
        };
        let space = task.space;
        let Some(helper) = (task.helpers.strands.iter())
            .find(|each| each.agent == strand)
            .map(|each| each.helper)
        else {
            return;
        };
        let tab = call.arguments.get("tab").and_then(tools::tab_id);
        let own =
            tab.is_some_and(|tab| (self.tabs_seen(space, cx).iter()).any(|each| each.id == tab));
        let Some(task) = self.task_mut(thread) else {
            return;
        };
        if let Some(each) = (task.helpers.strands.iter_mut()).find(|each| each.agent == strand) {
            each.heard = true;
        }
        match (tab, call.tool.as_str()) {
            (Some(tab), _) if own => task.helpers.on(helper, tab),
            (None, "open_tab" | tools::SEARCHED) => task.helpers.opens(helper),
            _ => {}
        }
    }

    /// A strand of `thread`'s called `tool`, which it hasn't: a strand of its own, or one of its
    /// thread's own tools, stops the thread as a helper ilo doesn't allow. Any other is only
    /// refused.
    pub(super) fn strand_not_allowed(&mut self, thread: &str, tool: &str, cx: &mut Context<Self>) {
        let limit = match tool {
            tools::STRAND => Limit::Depth,
            tool if tools::THE_THREADS_OWN.contains(&tool) => Limit::Kind,
            _ => return,
        };
        self.helper_not_allowed(thread, limit, cx);
    }

    /// What the strand `strand` of `thread`'s run `run` said: what it says is kept for its answer,
    /// its turn's end answers its thread's call with it, or with what went wrong, and it ends
    /// there. Nothing it says, writes or thinks is the thread's own.
    pub(super) fn strand_said(
        &mut self,
        thread: &str,
        strand: &str,
        run: u32,
        out: Out,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self.task_mut(thread) else {
            return;
        };
        let (trace, name, cli) = (task.trace.len(), task.name.clone(), task.cli);
        let ours = |each: &Strand| {
            let run_of = each.process.as_ref().map(Process::run);
            each.agent == strand && run_of == Some(run)
        };
        let Some(at) = task.helpers.strands.iter().position(ours) else {
            return;
        };
        let answer = match out {
            Out::Said(text) => {
                let strand = &mut task.helpers.strands[at];
                (strand.said, strand.heard) = (Some(text), true);
                return;
            }
            Out::Ended(None) => {
                let said = task.helpers.strands[at].said.take();
                ToolResult::text(said.unwrap_or_default())
            }
            Out::Ended(Some(failure)) => ended_badly(&failure_words(&name, cli, failure)),
            Out::Refused(why) => {
                log!("browser: strand {strand} ended as {why}");
                use crate::shell::conversation::words;
                let (said, _) = words::head(&IloKind::NotOurTools, &name);
                ended_badly(&format!("{PRODUCT} {said}."))
            }
            Out::Walled(Err(refused)) => {
                log!("browser: strand {strand} wasn't walled in: {refused:?}");
                ended_badly(&format!("{name} couldn't start."))
            }
            Out::Gone(_) => ended_badly(&format!("{name} stopped working.")),
            _ => return,
        };
        let mut ended = task.helpers.strands.remove(at);
        task.helpers.seen[ended.helper].ended.get_or_insert(trace);
        if let Some(reply) = ended.reply.take() {
            _ = reply.send(answer);
        }
        drop(ended);
        self.tokens_changed();
        cx.notify();
    }

    /// The strands of `thread` under way, by id.
    #[cfg(test)]
    pub(super) fn strands_of(&self, thread: &str) -> Vec<String> {
        let strands = self.task(thread).map(|task| task.helpers.strands.iter());
        strands
            .into_iter()
            .flatten()
            .map(|strand| strand.agent.clone())
            .collect()
    }

    /// The run of the strand `strand`, or 0 with none.
    #[cfg(test)]
    pub(super) fn strand_run(&self, strand: &str) -> u32 {
        let task = super::strand_parent(strand).and_then(|thread| self.task(thread));
        (task
            .into_iter()
            .flat_map(|task| task.helpers.strands.iter()))
        .find(|each| each.agent == strand)
        .and_then(|each| each.process.as_ref())
        .map_or(0, Process::run)
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
}

#[cfg(test)]
mod tests;
