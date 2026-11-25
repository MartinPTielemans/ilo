//! A task with files as its level changes or its files are taken back (#4101). Its assistant runs
//! inside a wall made for one level, which nothing changes once it's up: a change that changes
//! what the wall allows ends that run at once and starts another inside the new wall, from the
//! task's own words, as its assistant keeps no conversation inside a wall (kept.rs). A change that
//! leaves the wall as it is, as every change is for a task whose files are attached copies alone,
//! is told as any task's is and nothing ends.
//!
//! A wall that isn't put up, at once by `put_up` or later by the system's own no (`wall_refused`):
//! a higher level's leaves the task at the level it runs at; a start's or a lower level's lets the
//! task go on without its files, which then needs no wall; and files taken back whose task can't
//! be started again without them leave it stopped. Each is kept on the task (`Rewalled`) for the
//! line that says it (lines.rs), whose words aren't here. A task that goes on without its files,
//! or stops for them, is in the activity log too, as an event with no file's name: the first on
//! the row of the start it came at, the second a row of its own.
//!
//! Ending a run can leave some of what its assistant started running, which the sweep that ends
//! it says. That, and that a later sweep ended it, are kept the same way (`left_running`,
//! `ended_left`), from the sweep's own answer.

use std::sync::Arc;
use std::time::Instant;

use gpui::Context;

use super::{State, Task, conversation};
use crate::agents::Agents;
use crate::agents::access::ACCESS_ACTION;
use crate::agents::files::Given;
use crate::agents::runner::{self, Cli, Failure};
use crate::agents::tier::{self, StartedWith, Tier};
use crate::agents::wall::leftovers::{SweepFailed, Swept};
use crate::agents::wall::{self, Refusal, Wall, WallFor, WallRefused};

/// What answers for `wall::put_up` in a test.
#[cfg(test)]
pub type PutUp = std::rc::Rc<dyn Fn(&WallFor) -> Result<Wall, WallRefused>>;

/// A start inside a wall, until the system says whether the wall is up.
pub struct Rising {
    /// What the start was given to read, which a launcher that ended never read.
    first: String,
    /// The level a raise took the task from, which it goes back to if the wall isn't up.
    raised_from: Option<Tier>,
}

impl Rising {
    pub(super) fn of(first: &str) -> Rising {
        Rising {
            first: first.into(),
            raised_from: None,
        }
    }
}

/// What changed for a task ilo runs.
pub enum Change {
    /// Its level, from `from`. A raise that changes its wall comes with the new level's, which
    /// `wall_to_raise` put up before the level changed.
    Level { from: Tier, raised: Option<Wall> },
    /// The user took `files` back, which the task no longer has.
    TookBack(Given),
}

/// A start again after a change, which waits on the sweep after the run before it: it goes once
/// that sweep answers that everything the run started has ended, so nothing of the old run is
/// still at work beside the new one. Nothing else waits: a Stop drops it.
pub(in crate::agents) struct Restart {
    /// The run whose sweep it waits on, by its process.
    after: u32,
    /// What ilo tells its assistant of the change, after the task's words as they are then.
    told: String,
    change: Change,
}

/// A later start's log row where it left the task's files out: the level it started at, then
/// that, as `tier::started_logged` says it.
const LEFT_OUT: StartedWith = StartedWith {
    pages: 0,
    folder: false,
    files: 0,
    left_out: true,
};

/// The activity log's words, each its entry's target as a change of level's is
/// `tier::changed_logged`: the event, and never a file's name or a path.
/// A task that stopped as it couldn't be started again without the folder the user took back,
/// or without the files they attached.
pub const STOPPED_FOLDER_LOGGED: &str = "Stopped · couldn't restart without its folder";
pub const STOPPED_FILES_LOGGED: &str = "Stopped · couldn't restart without its files";
/// A run ended with some of what its assistant started still running.
pub const LEFT_RUNNING_LOGGED: &str = "Couldn't end everything it started";

/// A later sweep ended the `count` processes a run had left running.
pub fn ended_left_logged(count: usize) -> String {
    match count {
        1 => "Ended 1 process left running".into(),
        count => format!("Ended {count} processes left running"),
    }
}

/// What came of a wall that wasn't put up, or of ending a run inside one, with what its line
/// names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rewalled {
    /// The wall of `refused`, a higher level the task never had: it stays at `at`, as it ran,
    /// with `files`.
    Stays {
        at: Tier,
        refused: Tier,
        files: Given,
    },
    /// The same by the system's own no, which came once the task was at `refused`: it's back at
    /// `at`.
    SetBack {
        at: Tier,
        refused: Tier,
        files: Given,
    },
    /// Its wall at a start or at a lower level: the task goes on without `files`.
    WithoutFiles { files: Given },
    /// Its assistant couldn't be started again without `files`, which the user took back: the
    /// task stopped.
    Stopped { files: Given },
    /// Its assistant couldn't be started again at `refused`, the level the user raised it to:
    /// the task stopped.
    NotRaised { refused: Tier },
    /// Not everything its assistant started could be ended with its run, which was inside the
    /// wall of `files`.
    LeftRunning { files: Given },
    /// A later sweep ended what a run of its had left running.
    EndedLeft,
}

/// What came of a wall as its task keeps it: with how many of the task's messages and steps came
/// before it, which place its line among them as a refusal's are placed (lines.rs), and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Came {
    pub after: usize,
    pub traced: usize,
    pub appeared: Instant,
    pub what: Rewalled,
}

impl Task {
    /// Keeps what came of a wall that wasn't put up, after the messages and steps the task has
    /// now.
    pub(in crate::agents) fn rewalled(&mut self, what: Rewalled) {
        self.rewalled.push(Came {
            after: self.messages.len(),
            traced: self.trace.len(),
            appeared: Instant::now(),
            what,
        });
    }

    /// Its assistant couldn't be started again at `refused`, the level the user raised it to,
    /// and the task is stopped: what the wait on a raise's restart found.
    pub(in crate::agents) fn not_raised(&mut self, refused: Tier) {
        self.rewalled(Rewalled::NotRaised { refused });
    }

    /// The sweep that ended a run of its couldn't end everything the assistant had started.
    /// `had` is what that run was given, which what's left can still reach: the app's own
    /// record of the user's pick, and nothing an assistant or a page says.
    pub(in crate::agents) fn left_running(&mut self, had: Given) {
        self.rewalled(Rewalled::LeftRunning { files: had });
    }

    /// A later sweep ended what a run of its had left running.
    pub(in crate::agents) fn ended_left(&mut self) {
        self.rewalled(Rewalled::EndedLeft);
    }
}

impl Agents {
    /// What the sweep after the end of `run`, a walled run of `key`'s, found. A sweep that
    /// couldn't end everything leaves a line naming what that run was given, and an entry in the
    /// log; one that could says nothing, as the run's end already showed.
    pub(in crate::agents) fn swept(
        &mut self,
        key: &str,
        run: u32,
        answer: Result<Swept, SweepFailed>,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        let Some(at) = (task.ran_walled.iter()).position(|(walled, _)| *walled == run) else {
            return;
        };
        let (_, had) = task.ran_walled.swap_remove(at);
        let restart = task.restart.take_if(|restart| restart.after == run);
        if answer.is_ok() {
            if let Some(Restart { told, change, .. }) = restart {
                self.start_again(key, &told, change, cx);
            }
            return;
        }
        if let Some(restart) = restart {
            self.not_restarted(key, restart.change, cx);
        }
        if let Some(task) = self.task_mut(key) {
            task.left_running(had);
        }
        self.note_on(key, 0, ACCESS_ACTION, "", LEFT_RUNNING_LOGGED, "");
        cx.notify();
    }

    /// The start after `change` waited on a sweep that couldn't end everything the run before
    /// had started, so it starts nothing and ilo stops the task, with the line of what didn't
    /// start: the level a raise was to, or what a lower or a take-back would have started
    /// without, as a start that fails without them says it.
    fn not_restarted(&mut self, key: &str, change: Change, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        let files = match change {
            Change::Level {
                raised: Some(_), ..
            } => {
                let refused = task.tier;
                task.not_raised(refused);
                None
            }
            Change::Level { raised: None, .. } => Some(task.files.clone()),
            Change::TookBack(files) => Some(files),
        };
        if let Some(files) = files {
            let stopped = match files.attached {
                Some(_) => STOPPED_FILES_LOGGED,
                None => STOPPED_FOLDER_LOGGED,
            };
            task.rewalled(Rewalled::Stopped { files });
            self.note_on(key, 0, ACCESS_ACTION, "", stopped, "");
        }
        if let Some(task) = self.task_mut(key) {
            task.state = State::Stopped;
            task.end(std::time::SystemTime::now());
            task.waiting = None;
            task.writing = None;
        }
        self.keep_ended(key);
        self.forget_waiting(key, None, cx);
        cx.notify();
    }
}

impl Agents {
    /// The wall for `agent`'s task as it is, at `level`.
    fn put_up(&self, agent: &str, level: Tier) -> Result<Wall, WallRefused> {
        let refused = || WallRefused::Failed("the task isn't one ilo runs".into());
        let task = self.task(agent).ok_or_else(refused)?;
        // Where its start puts them (`launch_in`): a task with files is never kept, so its
        // folder is its own under the tasks'.
        let dir = self.tasks_dir();
        let (task_dir, config) = (dir.join(agent), dir.join(format!("{agent}.json")));
        let wall = WallFor {
            cli: task.cli.ok_or_else(refused)?,
            level,
            folder: task.files.folder.as_ref(),
            attached: task.files.attached.as_deref(),
            task_dir: &task_dir,
            config: &config,
        };
        #[cfg(test)]
        if let Some(put_up) = &self.tasks.wall {
            return put_up(&wall);
        }
        wall::put_up(&wall)
    }

    /// The wall a start of `agent`'s task goes inside: none for a task with no files, which
    /// starts as it always has. Err for one whose wall isn't put up, which from then goes on
    /// without its files.
    fn wall_or_without(&mut self, agent: &str) -> Result<Option<Arc<Wall>>, ()> {
        let Some(task) = self.task(agent).filter(|task| task.files.any()) else {
            return Ok(None);
        };
        match self.put_up(agent, task.tier) {
            Ok(wall) => Ok(Some(Arc::new(wall))),
            // The folder is no longer where it was: its grant ends (J6), and what else the task
            // has goes inside a wall of its own.
            Err(WallRefused::Refused(Refusal::Gone)) if task.files.folder.is_some() => {
                self.folder_gone(agent);
                self.wall_or_without(agent)
            }
            Err(_) => {
                self.without_files(agent);
                Err(())
            }
        }
    }

    /// The wall the task's first start goes inside, with the log's row of that start: the level
    /// it starts at, what it starts `with`, and on the same row that its files were left out,
    /// where its wall wasn't put up or their copies couldn't be made.
    pub(super) fn wall_to_begin(&mut self, agent: &str, with: StartedWith) -> Option<Arc<Wall>> {
        let wall = self.wall_or_without(agent);
        let left_out = with.left_out || wall.is_err();
        self.log_start(agent, StartedWith { left_out, ..with });
        wall.unwrap_or_default()
    }

    /// The wall a later start of `agent`'s task goes inside. One that leaves its files out is in
    /// the log as that start's row, which a later start otherwise has none of.
    pub(super) fn wall_to_start(&mut self, agent: &str) -> Option<Arc<Wall>> {
        let wall = self.wall_or_without(agent);
        if wall.is_err() {
            self.log_start(agent, LEFT_OUT);
        }
        wall.unwrap_or_default()
    }

    /// The log's row for a start of `agent`'s task at the level it has, with what it starts
    /// with: counts alone, never a name or a path.
    pub(super) fn log_start(&mut self, agent: &str, with: StartedWith) {
        let Some(task) = self.task(agent) else {
            return;
        };
        let started = tier::started_logged(task.tier, with);
        self.note_on(agent, 0, ACCESS_ACTION, "", &started, "");
    }

    /// `agent`'s task goes on without its files.
    fn without_files(&mut self, agent: &str) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        let files = std::mem::take(&mut task.files);
        task.rewalled(Rewalled::WithoutFiles { files });
    }

    /// The system says `agent`'s assistant is inside its wall.
    pub(super) fn walled_in(&mut self, agent: &str) {
        if let Some(task) = self.task_mut(agent) {
            task.rising = None;
        }
    }

    /// The system refused the wall `agent`'s assistant was starting inside, so nothing started
    /// and the run is over. A raise's goes back to the level the task ran at, inside that
    /// level's wall; any other start goes on without its files.
    pub(super) fn wall_refused(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        let rising = task.rising.take();
        let under_way = task.state == State::Working;
        let first = match rising {
            Some(Rising {
                raised_from: Some(at),
                ..
            }) => {
                let refused = std::mem::replace(&mut task.tier, at);
                let files = task.files.clone();
                task.rewalled(Rewalled::SetBack { at, refused, files });
                // What it was told of the raise is no longer so.
                let first = conversation(task);
                // The log had the raise, and has that it didn't hold.
                let back = tier::changed_logged(refused, at);
                self.note_on(agent, 0, ACCESS_ACTION, "", &back, "");
                first
            }
            Some(rising) => {
                self.without_files(agent);
                self.log_start(agent, LEFT_OUT);
                rising.first
            }
            // No start of ilo's was rising: it couldn't start, as any start that can't.
            None => {
                let Some(cli) = task.cli else {
                    return;
                };
                let why = format!("{} couldn't start.", cli.name());
                return self.fail(agent, Failure::Other(why), cx);
            }
        };
        if under_way {
            self.launch(agent, &first, cx);
            self.wait_for_answer(agent, cx);
        }
    }

    /// Before the user's raise of the task `key` from `from` to `to` is taken: the wall of the
    /// new level for a task whose wall it changes, put up while the task runs on as it was, or
    /// None for a task whose wall stays. Err, and the task stays at `from`, if that wall isn't
    /// put up.
    pub(in crate::agents) fn wall_to_raise(
        &mut self,
        key: &str,
        from: Tier,
        to: Tier,
    ) -> Result<Option<Wall>, ()> {
        let Some(task) = self.task(key).filter(|_| to > from) else {
            return Ok(None);
        };
        // Its one start, once its copies end, puts up the wall of the level it then has.
        if task.files.same_rules(from, to) || task.copying {
            return Ok(None);
        }
        let wall = match task.cli {
            Some(Cli::Claude) => self.put_up(key, to).ok(),
            // Codex inside a wall isn't started again at another level yet.
            Some(Cli::Codex) | None => None,
        };
        if wall.is_none()
            && let Some(task) = self.task_mut(key)
        {
            let stays = Rewalled::Stays {
                at: from,
                refused: to,
                files: task.files.clone(),
            };
            task.rewalled(stays);
        }
        wall.map(Some).ok_or(())
    }

    /// The run the task `key` has is over, as the wall it's inside is no longer the task's.
    /// Whatever starts it next starts it inside the wall of the level it then has.
    pub(super) fn wall_outgrown(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        task.end_run();
        task.rising = None;
        // What that run asked goes unanswered with it, so no answer to it is the next run's act.
        task.asking = None;
        self.forget_waiting(key, None, cx);
    }

    /// The task `key`, which ilo runs, after `change`, with ilo's words about it for its
    /// assistant, `told`. With its wall as it was the change is told as any task's is. Otherwise
    /// its run ends here, before anything is asked of the system, so no change waits on the
    /// assistant: a task at work or asking starts again inside the new wall, as does any raised,
    /// and another starts with the user's next message. A run that was walled is swept first, and
    /// the start waits on that sweep's answer (`swept`).
    pub(in crate::agents) fn rewall(
        &mut self,
        key: &str,
        change: Change,
        told: String,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        // Its assistant hasn't started: it starts once, as its copies end, with what it then has.
        if task.copying {
            return;
        }
        let (raise, same) = match &change {
            Change::Level { from, .. } => {
                (task.tier > *from, task.files.same_rules(*from, task.tier))
            }
            Change::TookBack(_) => (false, false),
        };
        if same {
            // A start waiting on a sweep is told it with the change it waits to start after.
            if let Some(restart) = &mut task.restart {
                restart.told = format!("{}\n\n{told}", restart.told);
                return cx.notify();
            }
            return self.tell_task(key, told, raise, cx);
        }
        let now = raise || matches!(task.state, State::Working | State::Asking { .. });
        // The run a start now waits on: one already waited on, else the walled run this ends,
        // whose sweep is to come.
        let run = (task.process.as_ref()).map(runner::Process::run);
        let walled = run.filter(|run| (task.ran_walled.iter()).any(|(walled, _)| walled == run));
        let after = (task.restart.take().map(|restart| restart.after)).or(walled);
        self.wall_outgrown(key, cx);
        let Some(task) = self.task_mut(key) else {
            return;
        };
        if !now {
            task.told = Some(told);
            return;
        }
        task.told = None;
        task.state = State::Working;
        task.writing = None;
        match after {
            Some(after) => {
                task.restart = Some(Restart {
                    after,
                    told,
                    change,
                });
                cx.notify();
            }
            None => self.start_again(key, &told, change, cx),
        }
    }

    /// The task `key`'s start after `change`, inside the wall it now has, with ilo's words about
    /// the change, `told`, after the task's own.
    fn start_again(&mut self, key: &str, told: &str, change: Change, cx: &mut Context<Self>) {
        let Some(task) = self.task(key) else {
            return;
        };
        let first = format!("{}\n\n{told}", conversation(task));
        let (wall, from) = match change {
            Change::Level {
                from,
                raised: Some(wall),
            } => (Some(Arc::new(wall)), Some(from)),
            Change::Level { raised: None, .. } => (self.wall_to_start(key), None),
            Change::TookBack(files) => {
                let wall = self.wall_to_start(key);
                // Without them and without a wall, a start that fails leaves it stopped.
                if !self.launch_in(key, &first, wall, cx)
                    && let Some(task) = self.task_mut(key)
                {
                    let stopped = match files.attached {
                        Some(_) => STOPPED_FILES_LOGGED,
                        None => STOPPED_FOLDER_LOGGED,
                    };
                    task.rewalled(Rewalled::Stopped { files });
                    self.note_on(key, 0, ACCESS_ACTION, "", stopped, "");
                }
                return self.wait_for_answer(key, cx);
            }
        };
        self.launch_in(key, &first, wall, cx);
        if let Some(rising) = self.task_mut(key).and_then(|task| task.rising.as_mut()) {
            rising.raised_from = from;
        }
        self.wait_for_answer(key, cx);
    }
}
