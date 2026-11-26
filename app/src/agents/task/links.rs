//! The check for links a walled run left in its task's folder leading out of it (#4101), which
//! the reap thread walks once as the run ends (runner.rs). What it found is kept on the task, in
//! memory alone, for its lines (X10, and X11 under it) and their two buttons, which are the
//! user's alone: no agent's tool reaches them. Nothing it lists reaches a log line, the kept
//! file, the activity log or an agent's answer; the log counts.

use std::path::PathBuf;
use std::time::Instant;

use gpui::{AppContext, Context, SharedString};

use super::ilo::{IloKind, IloLine, Unchecked};
use super::{Mark, Task};
use crate::agents::wall::{self, Found, Walked};
use crate::agents::{Agents, lines};

/// Where the keys of the checks' lines start, two to each: X10, then X11.
const LINKS: usize = usize::MAX / 8 * 5;

/// What one check found, where it came among the task's messages and steps.
#[derive(Clone, Debug)]
pub(in crate::agents) struct Checked {
    found: Found,
    after: usize,
    traced: usize,
    appeared: Instant,
    /// The turn under way as it came, by the user's messages: once they write again, its lines
    /// are passed and their buttons go, ended task or not.
    turn: usize,
    /// Delete Links was pressed, which settles X10.
    deleted: bool,
}

impl Checked {
    fn passed(&self, task: &Task) -> bool {
        lines::turn(task) > self.turn
    }
}

/// What X11 says of `walked`, one form at most, the widest first: a walk that read nothing, or
/// stopped short, can't count the folders it missed.
fn unchecked(walked: &Walked) -> Option<Unchecked> {
    if walked.root_unread {
        Some(Unchecked::Folder)
    } else if walked.stopped.is_some() {
        Some(Unchecked::Part)
    } else {
        (walked.unread > 0).then_some(Unchecked::Inside(walked.unread))
    }
}

/// The lines of `task`'s checks, each with when it came, for `ilo_lines` to place: none where the
/// task has no folder's name to say. They stay unsettled on a task that has ended, as the links
/// are still in the folder.
pub(in crate::agents) fn lines(task: &Task) -> Vec<(Instant, Mark)> {
    let Some(folder) = task.names.folder.as_deref().map(SharedString::from) else {
        return Vec::new();
    };
    let mut made = Vec::new();
    for (index, checked) in task.links.iter().enumerate() {
        let walked = &checked.found.walked;
        let passed = checked.passed(task);
        let out = (!walked.links.is_empty()).then(|| {
            let kind = IloKind::LinksOut {
                folder: folder.clone(),
                count: walked.links.len(),
                every: walked.every_leaving,
            };
            (kind, passed || checked.deleted)
        });
        let unread = unchecked(walked).map(|unchecked| {
            let folder = folder.clone();
            (IloKind::LinksUnchecked { folder, unchecked }, passed)
        });
        for (nth, each) in [out, unread].into_iter().enumerate() {
            let Some((kind, settled)) = each else {
                continue;
            };
            let line = IloLine {
                key: LINKS + 2 * index + nth,
                kind,
                settled,
            };
            let mark = Mark {
                after: checked.after,
                traced: checked.traced,
                line,
            };
            made.push((checked.appeared, mark));
        }
    }
    made
}

/// The check `line` is of, and whether it's X11.
fn of_line(task: &Task, line: usize) -> Option<(&Checked, bool)> {
    let at = line.checked_sub(LINKS)?;
    Some((task.links.get(at / 2)?, at % 2 == 1))
}

impl Agents {
    /// What the check as a walled run of `key`'s ended found, which comes after that run's sweep,
    /// whichever run is under way now: kept for its lines while the task is.
    pub(in crate::agents) fn links_found(
        &mut self,
        key: &str,
        found: Found,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        let walked = &found.walked;
        log!(
            "browser: task {key} links out {}, folders unread {}, whole {}",
            walked.links.len(),
            walked.unread,
            !walked.root_unread && walked.stopped.is_none()
        );
        let turn = lines::turn(task);
        task.links.push(Checked {
            after: task.messages.len(),
            traced: task.trace.len(),
            appeared: Instant::now(),
            turn,
            deleted: false,
            found,
        });
        cx.notify();
    }

    /// Where Show in Finder on `line` of `key`'s task points, for the system's file manager: the
    /// first link X10 lists, the folder X11 couldn't open, or the first folder in it X11 couldn't
    /// read. The user's own folder's layout, which only that press is given.
    pub fn link_to_reveal(&self, key: &str, line: usize) -> Option<PathBuf> {
        let (checked, x11) = of_line(self.task(key)?, line)?;
        let (walked, folder) = (&checked.found.walked, checked.found.folder.root());
        if !x11 {
            return walked.links.first().map(|link| folder.join(&link.path));
        }
        match unchecked(walked)? {
            Unchecked::Folder => Some(folder.to_path_buf()),
            Unchecked::Inside(_) => walked.first_unread.as_ref().map(|at| folder.join(at)),
            Unchecked::Part => None,
        }
    }

    /// The user's Delete Links on X10, `line` of `key`'s task: exactly the links it lists, each
    /// only if it's still the one the walk read (`wall::remove_links`), off the UI thread. The line
    /// settles at the press, so nothing deletes twice. A list of every leaving link is the user's
    /// own too, so it has no Delete, and a passed line takes no press.
    pub(in crate::agents) fn delete_links(
        &mut self,
        key: &str,
        line: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        let Some(at) = of_line(task, line).and_then(|(checked, x11)| {
            let walked = &checked.found.walked;
            let offered = !x11 && !walked.every_leaving && !walked.links.is_empty();
            (offered && !checked.deleted && !checked.passed(task)).then_some(line - LINKS)
        }) else {
            return;
        };
        let checked = &mut task.links[at / 2];
        checked.deleted = true;
        let Found { walked, folder } = checked.found.clone();
        let removing =
            cx.background_spawn(async move { wall::remove_links(&folder, &walked.links) });
        let key = key.to_string();
        cx.spawn(async move |agents, cx| {
            let removed = removing.await;
            log!(
                "browser: task {key} links deleted {}, left {}",
                removed.removed,
                removed.left
            );
            _ = agents.update(cx, |_, cx| cx.notify());
        })
        .detach();
        cx.notify();
    }
}
