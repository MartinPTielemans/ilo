//! Each run of Claude Code in its box (#4101): put up at the run's start, for the run's own
//! folder, and the same at every level, so a change of level or files is told and ends nothing.
//! Where there's no box Claude Code doesn't start, but on Windows, which has no sandbox ilo uses
//! and where it runs with its tools off (wall.rs `runs`). A box that isn't put up, at once or by
//! the system's own no, fails the run as any start that can't: never a start outside it.
//!
//! Ending a run can leave some of what its assistant started running, which the sweep that ends
//! it says (wall/leftovers.rs). That's in the activity log, and a later launch's sweep that ends
//! it is a line on the task too (`came_clean`).

use std::path::Path;
use std::sync::Arc;

use gpui::Context;

use crate::agents::Agents;
use crate::agents::access::ACCESS_ACTION;
use crate::agents::lines::Happened;
use crate::agents::runner::Cli;
#[cfg(test)]
use crate::agents::wall::WallRefused;
use crate::agents::wall::leftovers::{SweepFailed, Swept};
use crate::agents::wall::{self, Runs, Wall, WallFor};

/// What answers for `wall::put_up` in a test.
#[cfg(test)]
pub type PutUp = std::rc::Rc<dyn Fn(&WallFor) -> Result<Wall, WallRefused>>;

/// The activity log's words, each its entry's target as a change of level's is
/// `tier::changed_logged`: the event, and never a file's name or a path. A run ended with some of
/// what its assistant started still running.
pub const LEFT_RUNNING_LOGGED: &str = "Couldn't end everything it started";

/// A later sweep ended the `count` processes a run had left running.
pub fn ended_left_logged(count: usize) -> String {
    match count {
        1 => "Ended 1 process left running".into(),
        count => format!("Ended {count} processes left running"),
    }
}

impl Agents {
    /// Whether this system boxes a run of `cli` in, runs it outside a box, or doesn't run it.
    pub(in crate::agents) fn runs(&self, cli: Cli) -> Runs {
        #[cfg(test)]
        return wall::runs(
            cli,
            (self.tasks.box_available).unwrap_or(Err(wall::Unavailable::NoSandbox)),
        );
        #[cfg(not(test))]
        wall::runs(cli, wall::available())
    }

    /// The box a run of `agent`'s task, `run`, starts in, with its folder at `task_dir`: None
    /// where it runs outside one. Err where it doesn't start, which the dev log says why of.
    pub(super) fn box_for(
        &self,
        agent: &str,
        run: &str,
        task_dir: &Path,
        keeps: bool,
    ) -> Result<Option<Arc<Wall>>, ()> {
        let cli = self.task(agent).and_then(|task| task.cli).ok_or(())?;
        match self.runs(cli) {
            Runs::Unboxed => return Ok(None),
            Runs::Not => {
                log!(
                    "browser: {run} couldn't start {}: no box on this system",
                    cli.id()
                );
                return Err(());
            }
            Runs::Boxed => {}
        }
        let dir = self.tasks_dir();
        let config = dir.join(format!("{run}.json"));
        let places = dir.join(wall::leftovers::ROOT);
        let found = self.tasks.found.get(&cli);
        // None resolves to nothing, so its box isn't put up.
        let program = (found.and_then(|found| found.program.clone())).unwrap_or_default();
        let (cli_home, claude_config) = wall::cli_home(cli);
        let bridge = wall::bridge().unwrap_or_default();
        let home = crate::agents::clients::home();
        let data = dir.parent().unwrap_or(&dir);
        let wall = WallFor {
            cli,
            task_dir,
            config: &config,
            program: &program,
            home: &home,
            data,
            socket: &self.socket,
            bridge: Some(&bridge),
            places: &places,
            cli_home: &cli_home,
            claude_config: claude_config.as_deref(),
            keeps,
        };
        #[cfg(test)]
        let put = match &self.tasks.wall {
            Some(put_up) => put_up(&wall),
            None => wall::put_up(&wall),
        };
        #[cfg(not(test))]
        let put = wall::put_up(&wall);
        match put {
            Ok(wall) => Ok(Some(Arc::new(wall))),
            Err(why) => {
                log!("browser: {run} couldn't start {}: {why:?}", cli.id());
                Err(())
            }
        }
    }

    /// What the sweep after the end of `run`, a boxed run of `key`'s, found. A sweep that
    /// couldn't end everything leaves an entry in the log; one that could says nothing, as the
    /// run's end already showed.
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
        let Some(at) = (task.ran_boxed.iter()).position(|(boxed, _)| *boxed == run) else {
            return;
        };
        let (_, place) = task.ran_boxed.swap_remove(at);
        if answer.is_ok() {
            return;
        }
        self.note_on(key, 0, ACCESS_ACTION, "", LEFT_RUNNING_LOGGED, "");
        // A kept task's row carries the place's name, for the launch whose sweep of it comes
        // clean to say so on the task.
        let (Some(place), Some(root)) = (place, self.kept_root()) else {
            return cx.notify();
        };
        if let Some(record) = self.tasks.kept.of_mut(key) {
            record.left.push(place);
            self.tasks.kept.write(&root);
        }
        cx.notify();
    }

    /// What the launch's cleanup found of the places runs before left: each it swept clean, by
    /// name with how many it ended. The kept task whose row carries that name gets ilo's line
    /// that ilo ended what its assistant had left running, and an entry of how many; its row no
    /// longer carries it. They wait for the kept tasks to be read (`take_old_kept`), as the
    /// cleanup can end first. A sweep that ended nothing found it already ended, which ilo didn't
    /// do, so it says nothing.
    pub(in crate::agents) fn came_clean(
        &mut self,
        clean: Vec<(String, usize)>,
        cx: &mut Context<Self>,
    ) {
        self.tasks.came_clean.extend(clean);
        if !self.tasks.kept_taken {
            return;
        }
        let mut changed = false;
        for (place, ended) in std::mem::take(&mut self.tasks.came_clean) {
            let Some(task) = self.tasks.kept.came_clean(&place) else {
                continue;
            };
            changed = true;
            let Some(task) = task.filter(|_| ended > 0) else {
                continue;
            };
            self.place_in(&task, Happened::EndedLeft);
            self.note_on(&task, 0, ACCESS_ACTION, "", &ended_left_logged(ended), "");
        }
        if !changed {
            return;
        }
        if let Some(root) = self.kept_root() {
            self.tasks.kept.write(&root);
        }
        cx.notify();
    }
}
