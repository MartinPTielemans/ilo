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
//! line that says it, whose words aren't here.

use std::sync::Arc;

use gpui::Context;

use super::{State, Task, conversation};
use crate::agents::Agents;
use crate::agents::access::ACCESS_ACTION;
use crate::agents::files::Given;
use crate::agents::lines;
use crate::agents::runner::{Cli, Failure};
use crate::agents::tier::{self, Tier};
use crate::agents::wall::{self, Wall, WallFor, WallRefused};

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

/// What came of a wall that wasn't put up, with what its line names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rewalled {
    /// The wall of `refused`, a higher level: the task stays at `at`, as it ran.
    Stays { at: Tier, refused: Tier },
    /// Its wall at a start or at a lower level: the task goes on without `files`.
    WithoutFiles { files: Given },
    /// Its assistant couldn't be started again without `files`, which the user took back: the
    /// task stopped.
    Stopped { files: Given },
}

impl Task {
    /// Keeps what came of a wall that wasn't put up, with the turn under way.
    fn rewalled(&mut self, came: Rewalled) {
        self.rewalled.push((lines::turn(self), came));
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
        let places = dir.join(wall::leftovers::ROOT);
        let cli = task.cli.ok_or_else(refused)?;
        let found = self.tasks.found.get(&cli);
        // None resolves to nothing, so its wall isn't put up; its start then says it isn't there.
        let program = (found.and_then(|found| found.program.clone())).unwrap_or_default();
        let (cli_home, claude_config) = wall::cli_home(cli);
        // None resolves to nothing too, so the start says it's without its files.
        let bridge = wall::bridge().unwrap_or_default();
        let wall = WallFor {
            cli,
            level,
            folder: task.files.folder.as_ref(),
            attached: task.files.attached.as_deref(),
            task_dir: &task_dir,
            config: &config,
            program: &program,
            socket: &self.socket,
            bridge: &bridge,
            places: &places,
            cli_home: &cli_home,
            claude_config: claude_config.as_deref(),
        };
        #[cfg(test)]
        if let Some(put_up) = &self.tasks.wall {
            return put_up(&wall);
        }
        wall::put_up(&wall)
    }

    /// The wall a start of `agent`'s task goes inside: none for a task with no files, which
    /// starts as it always has, and none for one whose wall isn't put up, which then starts
    /// without its files.
    pub(super) fn wall_to_start(&mut self, agent: &str) -> Option<Arc<Wall>> {
        let task = self.task(agent)?;
        if !task.files.any() {
            return None;
        }
        match self.put_up(agent, task.tier) {
            Ok(wall) => Some(Arc::new(wall)),
            Err(_) => {
                self.without_files(agent);
                None
            }
        }
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
                task.rewalled(Rewalled::Stays { at, refused });
                // What it was told of the raise is no longer so.
                let first = conversation(task);
                // The log had the raise, and has that it didn't hold.
                let back = tier::changed_logged(refused, at);
                self.note_on(agent, 0, ACCESS_ACTION, "", &back, "");
                first
            }
            Some(rising) => {
                self.without_files(agent);
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
        if task.files.same_rules(from, to) {
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
    /// assistant: a task at work or asking starts again at once inside the new wall, as does any
    /// raised, and another starts with the user's next message.
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
        let (raise, same) = match &change {
            Change::Level { from, .. } => {
                (task.tier > *from, task.files.same_rules(*from, task.tier))
            }
            Change::TookBack(_) => (false, false),
        };
        if same {
            return self.tell_task(key, told, raise, cx);
        }
        let now = raise || matches!(task.state, State::Working | State::Asking { .. });
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
                    task.rewalled(Rewalled::Stopped { files });
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
