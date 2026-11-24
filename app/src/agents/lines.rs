//! ilo's lines of what happened to a task (#4086 step 3): each refusal, each step that's the
//! user's, each time an agent asks for a tab back, and each change of the task's access. The app
//! makes each from the task's own state, at the end of the turn it came in: nothing an agent sends
//! and no page's text makes one, names its buttons or gives it a link.

use std::time::Instant;

use gpui::{App, Context};

use super::held;
use super::task::ilo::{IloKind, IloLine};
use super::task::{Line, Task, Who};
use super::tier::{self, Tier};
use super::{Agents, site};

/// Where the keys of the task's refusals' lines and of its other lines start, apart from the
/// keys of its steps below and of the helpers' and the kept task's lines above.
const STOPS: usize = usize::MAX / 8;
const PLACED: usize = usize::MAX / 4;

/// One of a task's lines other than a refusal, which a refusal's count keeps in `Task::stops`.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub kind: Happened,
    /// How many messages the user had written, which places it at the end of that turn.
    turn: usize,
    appeared: Instant,
    /// Acted on, or passed: its tab came back, or the user kept it.
    pub settled: bool,
}

/// What a placed line says, from the task's state only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Happened {
    /// A step in `tab` only the user can do: a hand-off, a sign-in or a passkey.
    Yours { tab: u64 },
    /// The agent asks for `tab` back.
    TabBack { tab: u64 },
    /// The user changed the task's access.
    TierChanged { from: Tier, to: Tier },
}

impl Happened {
    fn tab(self) -> Option<u64> {
        match self {
            Happened::Yours { tab } | Happened::TabBack { tab } => Some(tab),
            Happened::TierChanged { .. } => None,
        }
    }
}

/// The turn of `task` under way, as its user's messages count it.
fn turn(task: &Task) -> usize {
    (task.messages.iter())
        .filter(|m| m.who == Who::User)
        .count()
}

/// The user's messages among `task`'s first `after` messages.
fn turn_at(task: &Task, after: usize) -> usize {
    (task.messages.iter().take(after))
        .filter(|m| m.who == Who::User)
        .count()
}

impl Agents {
    /// Puts `made` in the conversation of the task `agent`'s act on `connection`, for its call
    /// `call`, is of.
    pub(super) fn place(&mut self, agent: &str, connection: u64, call: u64, made: Happened) {
        let key = (self.acting_task(agent, connection, call)).map(|task| task.agent.clone());
        if let Some(key) = key {
            self.place_in(&key, made);
        }
    }

    /// Puts `made` in the task `key`'s conversation, at the end of the turn under way.
    pub(super) fn place_in(&mut self, key: &str, made: Happened) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        let turn = turn(task);
        // A tab comes once at a time: the same step on it isn't said twice while it stands.
        let standing = |placed: &Placed| !placed.settled && placed.kind == made;
        if made.tab().is_some() && task.placed.iter().any(standing) {
            return;
        }
        // The task's access changed: the line said so, and it needs nothing of the user.
        let settled = matches!(made, Happened::TierChanged { .. });
        task.placed.push(Placed {
            kind: made,
            turn,
            appeared: Instant::now(),
            settled,
        });
    }

    /// The step on `tab`, or its asking back, is over: its line keeps its words and loses its
    /// buttons.
    pub(super) fn settle_placed(&mut self, tab: u64) {
        for task in &mut self.tasks.tasks {
            for placed in (task.placed.iter_mut()).filter(|placed| placed.kind.tab() == Some(tab)) {
                placed.settled = true;
            }
        }
    }

    /// Carries out what a hold on `tab` that `event` moved to `life` says in its task's
    /// conversation: a step handed to the user, or an ask for the tab back, comes as a line; a
    /// wait that ends settles it.
    pub(super) fn held_lines(&mut self, tab: u64, event: held::Event, life: held::Life) {
        use held::{Event as E, Life as L};
        let made = match (event, life) {
            (E::HandOff, L::HandedOff) | (E::Passkey, L::Held) => Happened::Yours { tab },
            (E::Ask { holder: true }, L::Asked) => Happened::TabBack { tab },
            // The hold ended, or the wait on it did with the tab still the user's.
            (_, L::Free)
            | (E::Keep | E::Timer | E::NotNow | E::Stop { .. } | E::Gone { .. }, L::Held) => {
                return self.settle_placed(tab);
            }
            _ => return,
        };
        let Some((agent, connection, call)) =
            (self.held.get(&tab)).map(|held| (held.agent.clone(), held.connection, held.call))
        else {
            return;
        };
        // A tab asked back is no longer the step the user was given there.
        self.settle_placed(tab);
        self.place(&agent, connection, call, made);
    }

    /// The user's Not Now on the line `key` of the task `agent`: the step stays theirs, and the
    /// agent's call that waits on it hears they won't do it now. On an ask for the tab back, it
    /// keeps the tab.
    pub(super) fn not_now(&mut self, agent: &str, key: usize, cx: &mut Context<Self>) {
        let Some(at) = key.checked_sub(PLACED) else {
            return;
        };
        let placed = (self.task(agent))
            .and_then(|task| task.placed.get(at))
            .cloned();
        let Some(Placed {
            kind: Happened::Yours { tab } | Happened::TabBack { tab },
            settled: false,
            ..
        }) = placed
        else {
            return;
        };
        let event = match self.held.get(&tab).map(|held| held.life()) {
            Some(held::Life::Asked) => held::Event::Keep,
            _ => held::Event::NotNow,
        };
        self.let_go(tab, event, cx);
        if let Some(placed) = (self.task_mut(agent)).and_then(|task| task.placed.get_mut(at)) {
            placed.settled = true;
        }
        cx.notify();
    }

    /// ilo's lines of `task`'s refusals and of the rest that happened to it, each at the end of
    /// the turn it came in, in the order they came. An ended task's have no buttons.
    pub(super) fn ilo_lines(&self, task: &Task, lines: &mut Vec<Line>, cx: &App) {
        let ended = task.ended.is_some();
        let pages = self.pages(task, cx);
        let mut made: Vec<(usize, Instant, IloLine)> = Vec::new();
        for (index, stop) in task.stops.iter().enumerate() {
            let line = IloLine {
                key: STOPS + index,
                kind: IloKind::Stopped {
                    tab: stop.tab,
                    at: stop.at.clone().into(),
                    why: stop.why,
                    tier: stop.tier,
                    count: stop.count,
                },
                settled: stop.settled || ended,
            };
            made.push((turn_at(task, stop.after), stop.appeared, line));
        }
        for (index, placed) in task.placed.iter().enumerate() {
            let kind = match placed.kind {
                // Among several pages, it says which the step is on (O1).
                Happened::Yours { tab } => IloKind::Yours {
                    tab,
                    site: (pages.len() > 1)
                        .then(|| tier::plain(&site(&self.browser.tab(tab, cx).url)).into()),
                },
                Happened::TabBack { tab } => IloKind::TabBack { tab },
                Happened::TierChanged { from, to } => IloKind::TierChanged { from, to },
            };
            let line = IloLine {
                key: PLACED + index,
                kind,
                settled: placed.settled || ended,
            };
            made.push((placed.turn, placed.appeared, line));
        }
        made.sort_by_key(|(turn, appeared, _)| (*turn, *appeared));
        // In the order they came: each goes before the user's next message, after those of its
        // turn already placed, as placing one moves no message of the user's.
        for (turn, _, line) in made {
            let next = (lines.iter().enumerate())
                .filter(|(_, line)| matches!(line, Line::You(_)))
                .nth(turn)
                .map(|(at, _)| at);
            lines.insert(next.unwrap_or(lines.len()), Line::Ilo(line));
        }
    }
}
