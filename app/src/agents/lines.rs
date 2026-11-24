//! ilo's lines of what happened to a task (#4086 step 3): each refusal, each step that's the
//! user's, each time an agent asks for a tab back, each change of the task's access, and each wall
//! around its files that ilo couldn't put up, with what a run inside one left running (#4101,
//! rewall.rs). The app
//! makes each from the task's own state, at the end of the turn it came in: nothing an agent sends
//! and no page's text makes one, names its buttons or gives it a link.

use std::time::Instant;

use gpui::{App, Context, SharedString};

use super::files::{Given, Names};
use super::held;
use super::task::ilo::{IloKind, IloLine, LeftOut, Reach};
use super::task::rewall::Rewalled;
use super::task::{Mark, Task, Who};
use super::tier::{self, Tier};
use super::{Agents, site};

/// Where the keys of the task's refusals' lines and of its other lines start, apart from the
/// keys of its steps below and of the helpers' and the kept task's lines above.
const STOPS: usize = usize::MAX / 8;
const PLACED: usize = usize::MAX / 4;
/// Where the keys of the lines of the walls that weren't put up start: two to each, as a start
/// without a folder and without attached files says each on a line.
const REWALLED: usize = usize::MAX / 8 * 3;

/// What the user gave a task as a line names it: the folder, and each file they attached.
#[derive(Default)]
struct Shown {
    folder: Option<SharedString>,
    attached: Vec<SharedString>,
}

/// The names `files` are shown by: the one place a line takes a folder's or a file's name from,
/// so nothing else of a line holds one. Each is the name the user saw where they gave the task its
/// files (`names`), and only for what `files` still has.
fn shown(names: &Names, files: &Given) -> Shown {
    let Names { folder, files } = names.kept_for(files);
    Shown {
        folder: folder.map(Into::into),
        attached: files.into_iter().map(Into::into).collect(),
    }
}

/// What `files` are said as where a line says what a task is without, by the names in `names`:
/// the folder first, then the attached files, one by its name and more by their count.
fn left_out(names: &Names, files: &Given) -> impl Iterator<Item = LeftOut> {
    let Shown { folder, attached } = shown(names, files);
    let attached = match attached.as_slice() {
        [] => None,
        [file] => Some(LeftOut::File(file.clone())),
        files => Some(LeftOut::Files(files.len())),
    };
    folder.map(LeftOut::Folder).into_iter().chain(attached)
}

/// The lines of one wall that wasn't put up, or of what a run left running: none where a line
/// would have no name to say.
fn rewalled(came: &Rewalled, names: &Names) -> Vec<IloKind> {
    match came {
        Rewalled::Stays { at, refused, files } => (shown(names, files).folder.into_iter())
            .map(|folder| IloKind::KeptAt {
                at: *at,
                refused: *refused,
                folder,
            })
            .collect(),
        Rewalled::SetBack { at, refused, files } => (shown(names, files).folder.into_iter())
            .map(|folder| IloKind::SetBack {
                at: *at,
                refused: *refused,
                folder,
            })
            .collect(),
        Rewalled::WithoutFiles { files } => (left_out(names, files))
            .map(|left| IloKind::StartedWithout { left })
            .collect(),
        Rewalled::Stopped { files } => (left_out(names, files))
            .map(|left| IloKind::NotRestarted { left })
            .collect(),
        Rewalled::NotRaised { refused } => vec![IloKind::NotRaised { refused: *refused }],
        Rewalled::LeftRunning { files } => {
            // The files the user attached are said as the task's, so need no name.
            let reach = match (shown(names, files).folder, files.attached.is_some()) {
                (Some(folder), true) => Some(Reach::Both(folder)),
                (Some(folder), false) => Some(Reach::Folder(folder)),
                (None, true) => Some(Reach::Files),
                (None, false) => None,
            };
            (reach.into_iter())
                .map(|reach| IloKind::LeftRunning { reach })
                .collect()
        }
        Rewalled::EndedLeft => vec![IloKind::EndedLeft],
    }
}

/// One of a task's lines other than a refusal, which a refusal's count keeps in `Task::stops`.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub kind: Happened,
    /// How many of the task's messages and steps came before it, which places it among them.
    after: usize,
    traced: usize,
    appeared: Instant,
    /// Acted on, or passed: its tab came back, or the user kept it.
    pub settled: bool,
}

/// What a placed line says, from the task's state only.
#[derive(Clone, Debug, PartialEq)]
pub enum Happened {
    /// A step in `tab` only the user can do: a hand-off, a sign-in or a passkey.
    Yours { tab: u64 },
    /// The agent asks for `tab` back.
    TabBack { tab: u64 },
    /// The user changed the task's access.
    TierChanged { from: Tier, to: Tier },
    /// The user revoked the task's folder, `files` (J2).
    Revoked { files: Given },
    /// The task's folder, `files`, was no longer where it was, so ilo ended its grant (J6).
    FolderGone { files: Given },
    /// ilo couldn't copy the files the user attached, so the task started without them. `files`
    /// is where the copies were to be.
    NotCopied { files: Given },
}

impl Happened {
    fn tab(&self) -> Option<u64> {
        match self {
            Happened::Yours { tab } | Happened::TabBack { tab } => Some(*tab),
            Happened::TierChanged { .. }
            | Happened::Revoked { .. }
            | Happened::FolderGone { .. }
            | Happened::NotCopied { .. } => None,
        }
    }
}

/// The turn of `task` under way, as its user's messages count it.
pub(super) fn turn(task: &Task) -> usize {
    (task.messages.iter())
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
        let (after, traced) = (task.messages.len(), task.trace.len());
        // A tab comes once at a time: the same step on it isn't said twice while it stands.
        let standing = |placed: &Placed| !placed.settled && placed.kind == made;
        if made.tab().is_some() && task.placed.iter().any(standing) {
            return;
        }
        // The task's access or its files changed: the line said so, and it needs nothing of the user.
        let settled = made.tab().is_none();
        task.placed.push(Placed {
            kind: made,
            after,
            traced,
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

    /// ilo's lines of `task`'s refusals and of the rest that happened to it, in the order they
    /// came, each with how many of the task's messages and steps came before it, for `fold` to
    /// place among them. An ended task's have no buttons.
    pub(super) fn ilo_lines(&self, task: &Task, cx: &App) -> Vec<Mark> {
        let ended = task.ended.is_some();
        let pages = self.pages(task, cx);
        let mut made: Vec<(Instant, Mark)> = Vec::new();
        for (index, stop) in task.stops.iter().enumerate() {
            let line = IloLine {
                key: STOPS + index,
                kind: IloKind::Stopped {
                    tab: stop.tab,
                    at: stop.at.clone().into(),
                    doing: stop.doing.clone().into(),
                    why: stop.why,
                    tier: stop.tier,
                    count: stop.count,
                },
                settled: stop.settled || ended,
            };
            made.push((
                stop.appeared,
                Mark {
                    after: stop.after,
                    traced: stop.traced,
                    line,
                },
            ));
        }
        for (index, placed) in task.placed.iter().enumerate() {
            let kind = match &placed.kind {
                // Among several pages, it says which the step is on (O1).
                Happened::Yours { tab } => IloKind::Yours {
                    tab: *tab,
                    site: (pages.len() > 1)
                        .then(|| tier::plain(&site(&self.browser.tab(*tab, cx).url)).into()),
                },
                Happened::TabBack { tab } => IloKind::TabBack { tab: *tab },
                Happened::TierChanged { from, to } => IloKind::TierChanged {
                    from: *from,
                    to: *to,
                },
                // None where a line would have no name to say.
                Happened::Revoked { files } => match shown(&task.names, files).folder {
                    Some(folder) => IloKind::Revoked { folder },
                    None => continue,
                },
                Happened::FolderGone { files } => match shown(&task.names, files).folder {
                    Some(folder) => IloKind::FolderGone { folder },
                    None => continue,
                },
                Happened::NotCopied { files } => match left_out(&task.names, files).next() {
                    Some(left) => IloKind::NotCopied { left },
                    None => continue,
                },
            };
            let line = IloLine {
                key: PLACED + index,
                kind,
                settled: placed.settled || ended,
            };
            let mark = Mark {
                after: placed.after,
                traced: placed.traced,
                line,
            };
            made.push((placed.appeared, mark));
        }
        // What came of a wall stands where it came, as the rest does: after the messages and
        // steps the task had then.
        for (index, came) in task.rewalled.iter().enumerate() {
            for (nth, kind) in rewalled(&came.what, &task.names).into_iter().enumerate() {
                let line = IloLine {
                    key: REWALLED + 2 * index + nth,
                    kind,
                    settled: true,
                };
                let mark = Mark {
                    after: came.after,
                    traced: came.traced,
                    line,
                };
                made.push((came.appeared, mark));
            }
        }
        made.sort_by_key(|(appeared, mark)| (mark.after, mark.traced, *appeared));
        made.into_iter().map(|(_, mark)| mark).collect()
    }
}
