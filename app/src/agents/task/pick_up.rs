//! A reply to a task that has ended (#4105): the task model's side of kept.rs. A task's run is
//! kept when kept.rs says so, the user's reply after it ended takes it up at the access the app
//! decides, and what was kept goes with the task, with Settings › Agents' Remove All, or when the
//! browser starts again, as its tasks went with the run before.
//!
//! Only the user's own reply in the task's conversation takes a task up: no tool of an agent's
//! and no outside client reaches any of this, to take one up, list them or remove one.

use std::path::PathBuf;

use gpui::{AppContext, Context};

use super::ilo::{IloKind, IloLine};
use super::{Line, State, Task, Who};
use crate::agents::Agents;
use crate::agents::kept::{self, Keep, Keeps};
use crate::agents::runner::Cli;

/// The keys of the lines made here, from the top of the range, apart from the lines that count
/// up from a task's events.
const IN_FIELD: usize = usize::MAX;
const KEPT_HERE: usize = usize::MAX - 1;
const RESUMED: usize = usize::MAX - 2;

impl Agents {
    /// Where the kept tasks' folders are, under the profile's. None with no profile folder, when
    /// nothing is kept.
    pub(super) fn kept_root(&self) -> Option<PathBuf> {
        (self.tasks.dir.as_ref()).map(|dir| dir.join(kept::FOLDER))
    }

    /// Settings › Agents' Keep finished tasks so you can reply.
    pub fn keeps_tasks(&self) -> bool {
        !self.tasks.kept.off
    }

    /// The switch. Off, a new task's run keeps nothing; what's kept stays until it's removed,
    /// which Settings offers as it's turned off.
    pub fn set_keeps_tasks(&mut self, on: bool, cx: &mut Context<Self>) {
        if on != self.tasks.kept.off {
            return;
        }
        self.tasks.kept.off = !on;
        self.save();
        cx.notify();
    }

    /// How many finished tasks ilo holds a conversation's id for, and how many of them the last
    /// try couldn't remove.
    pub fn kept_tasks(&self) -> (usize, usize) {
        (self.tasks.kept.count(), self.tasks.kept.stuck())
    }

    /// What `agent`'s run that's about to start keeps, and the folder it runs in if it's kept. A
    /// task with a conversation kept goes on with it; a new one keeps what kept.rs decides.
    pub(super) fn keep_for(&self, agent: &str, cli: Cli) -> (Keep, Option<PathBuf>) {
        let Some(root) = self.kept_root() else {
            return (Keep::Nothing, None);
        };
        let keep = match self.tasks.kept.of(agent) {
            Some(record) if record.gone => Keep::Nothing,
            Some(record) => Keep::Resume(record.id.clone()),
            None => kept::decide(cli, self.keeps_tasks()),
        };
        let folder = match &keep {
            Keep::Nothing => None,
            Keep::New(id) | Keep::Resume(id) => Some(kept::folder(&root, id)),
        };
        (keep, folder)
    }

    /// `agent`'s run started, keeping `keep`: a new conversation's id is held from now. The
    /// profile's first says, at the foot of its log once it ends, where the conversation lives.
    pub(super) fn kept_started(&mut self, agent: &str, cli: Cli, keep: &Keep) {
        let (Keep::New(id), Some(root)) = (keep, self.kept_root()) else {
            return;
        };
        self.tasks.kept.hold(agent, cli, id.clone());
        if !std::mem::replace(&mut self.tasks.kept.told, true) {
            if let Some(record) = self.tasks.kept.of_mut(agent) {
                record.told = true;
            }
            self.save();
        }
        self.tasks.kept.write(&root);
    }

    /// Whether `agent`'s next run goes on with a conversation its assistant kept, and so takes
    /// only what's new as its first message.
    pub(super) fn resumes(&self, agent: &str) -> bool {
        (self.tasks.kept.of(agent)).is_some_and(|record| !record.gone)
    }

    /// The user replies to `agent`'s task: if it had ended and its conversation is kept, the reply
    /// takes it up, at the access the app decides from the one it ended at, which ilo's line over
    /// the reply says. Nothing the assistant or a page says has a part in it.
    pub(super) fn pick_up(&mut self, agent: &str) {
        let Some(task) = self.task(agent) else {
            return;
        };
        if !ended(task) {
            return;
        }
        let was = task.state.clone();
        let said = (task.messages.iter())
            .filter(|message| message.who == Who::User)
            .count();
        let Some(record) = self.tasks.kept.of_mut(agent).filter(|record| !record.gone) else {
            return;
        };
        record.tier = kept::comes_back(record.tier);
        record.resumed.push((said, record.tier));
        self.tasks.picked_up.insert(agent.to_string(), was);
        log!("browser: task {agent} picked up");
        // The activity log's entry for it, under the task's own name.
        self.note(agent, 0, "resumed", "", "");
    }

    /// `agent`'s run went, saying `said`. If it was to go on with a conversation its assistant no
    /// longer has, the task is as it was before the reply, which never reached it, and its
    /// conversation says so in the reply field's place. Returns whether that's what happened, and
    /// what it said with the conversation's id out of it, for the log.
    pub(super) fn kept_gone(&mut self, agent: &str, said: &str) -> (bool, String) {
        let Some(record) = self.tasks.kept.of_mut(agent) else {
            return (false, said.to_string());
        };
        let said = kept::without(said, &record.id);
        if record.gone || !kept::gone(&said) {
            return (false, said);
        }
        record.gone = true;
        record.resumed.pop();
        let was = self.tasks.picked_up.remove(agent);
        if let Some(task) = self.task_mut(agent) {
            if task
                .messages
                .last()
                .is_some_and(|last| last.who == Who::User)
            {
                task.messages.pop();
            }
            task.state = was.unwrap_or(State::Stopped);
            task.waiting = None;
            task.writing = None;
        }
        (true, said)
    }

    /// ilo's lines in a kept task's log: over each reply that took it up, the access it came back
    /// at, and at its foot, once, where the conversation lives. Then, last, the line that stands
    /// in the reply field's place once a task is done or stopped and can't take a reply: its
    /// assistant keeps none, or no longer has this one. An outside agent's session has none of
    /// them.
    pub(super) fn kept_lines(&self, task: &Task, lines: &mut Vec<Line>) {
        let Some(cli) = task.cli.filter(|_| task.session.is_none()) else {
            return;
        };
        let line = |key, kind| {
            Line::Ilo(IloLine {
                key,
                kind,
                settled: false,
            })
        };
        let record = self.tasks.kept.of(&task.agent);
        let resumed = record.map_or(&[][..], |record| &record.resumed[..]);
        // The last first, so each one's place still counts the lines before it.
        for (index, (said, tier)) in resumed.iter().enumerate().rev() {
            let at = (lines.iter().enumerate())
                .filter(|(_, line)| matches!(line, Line::You(_)))
                .nth(*said)
                .map(|(at, _)| at);
            if let Some(at) = at {
                let kind = IloKind::Resumed { tier: *tier };
                lines.insert(at, line(RESUMED - index, kind));
            }
        }
        // One that couldn't go on keeps its field: at its assistant's limit it waits for Retry,
        // and failed, for what fixes it.
        // And with no profile folder nothing is kept or looked for, so a task takes a reply as it
        // did before any was.
        if !matches!(task.state, State::Done { .. } | State::Stopped) || self.kept_root().is_none()
        {
            return;
        }
        match record {
            Some(record) if !record.gone => {
                if record.told {
                    lines.push(line(KEPT_HERE, IloKind::KeptHere));
                }
            }
            Some(_) => lines.push(line(IN_FIELD, IloKind::Gone)),
            None => lines.push(line(
                IN_FIELD,
                match kept::keeps(cli) {
                    Keeps::Never => IloKind::KeepsNothing,
                    Keeps::WhenWanted => IloKind::Gone,
                },
            )),
        }
    }

    /// `agent`'s task goes for good: what its assistant kept of it goes with it.
    pub(super) fn forget_kept(&mut self, agent: &str, cx: &mut Context<Self>) {
        self.tasks.picked_up.remove(agent);
        let agent = agent.to_string();
        self.remove_kept(
            move |record| record.task.as_deref() == Some(agent.as_str()),
            cx,
        );
    }

    /// Settings › Agents' Remove All: every finished task's conversation goes, and those an
    /// earlier try couldn't remove are tried again. A task still at work keeps its own, as its
    /// assistant is writing it; a finished one's run ends first, so nothing writes it back.
    pub fn remove_kept_tasks(&mut self, cx: &mut Context<Self>) {
        let mut working = Vec::new();
        for task in &mut self.tasks.tasks {
            match ended(task) {
                true => task.process = None,
                false => working.push(task.agent.clone()),
            }
        }
        self.remove_kept(
            move |record| !(record.task.as_ref()).is_some_and(|task| working.contains(task)),
            cx,
        );
    }

    /// At launch: the conversations an earlier run of the browser left go, as its tasks went.
    /// From `load`, which the app calls once the first window's UI has shown, so none of this is
    /// in the time to the first frame. With no file of ids, as on every launch after a run that
    /// kept nothing, the one look for the file is all a launch pays: nothing is read, started or
    /// spawned.
    pub(crate) fn remove_old_kept(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.kept_root() else {
            return;
        };
        if !self.tasks.kept.read(&root) {
            return;
        }
        self.remove_kept(|record| record.task.is_none(), cx);
    }

    /// Removes the conversations `which` picks, off the UI's thread, as each waits on its
    /// assistant. One that couldn't be removed stays held and counted, so nothing reads as removed
    /// that wasn't, and the user can try again.
    fn remove_kept(&mut self, which: impl Fn(&kept::Record) -> bool, cx: &mut Context<Self>) {
        let Some(root) = self.kept_root() else {
            return;
        };
        let removing: Vec<_> = (self.tasks.kept.removing(which).into_iter())
            .map(|(id, cli)| {
                let found = self.tasks.found.get(&cli).cloned().unwrap_or_default();
                (id, found.program, found.path)
            })
            .collect();
        if removing.is_empty() {
            return;
        }
        cx.notify();
        let folder = root.clone();
        let removed = cx.background_spawn(async move {
            (removing.into_iter())
                .map(|(id, program, path)| {
                    let removed = program.is_some_and(|program| {
                        kept::remove(&program, path.as_deref(), &folder, &id)
                    });
                    (id, removed)
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |agents, cx| {
            let removed = removed.await;
            _ = agents.update(cx, |agents, cx| {
                let stuck = removed.iter().filter(|(_, removed)| !removed).count();
                for (id, removed) in &removed {
                    agents.tasks.kept.removed(id, *removed);
                }
                log!(
                    "browser: kept tasks: {} removed, {stuck} couldn't be",
                    removed.len() - stuck
                );
                agents.tasks.kept.write(&root);
                cx.notify();
            });
        })
        .detach();
    }
}

/// Whether the task has ended: done, stopped, failed or at its assistant's limit.
fn ended(task: &Task) -> bool {
    !matches!(task.state, State::Working | State::Asking { .. })
}
