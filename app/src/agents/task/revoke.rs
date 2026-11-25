//! A live task's folder goes (#4101): the user revokes it, on Settings › Agents › Folders in use,
//! or ilo does, when the folder is no longer where it was. Either way the task goes on without
//! it, its assistant started again outside any wall for it (rewall.rs), and its conversation says
//! so by the folder's name; nothing else does. No agent tool reaches any of this.

use gpui::{Context, SharedString};

use super::rewall::Change;
use crate::agents::Agents;
use crate::agents::access::ACCESS_ACTION;
use crate::agents::files::Given;
use crate::agents::lines::Happened;

/// What a task's assistant is told as it starts again without its folder: that the user revoked
/// it (J2), or ilo did as it was no longer where it was (J6), and that files attached to the task
/// are still there if it has some. The lead's words. They name no folder, no path and no reason,
/// so the assistant learns nothing it didn't have.
pub(in crate::agents) fn revoked_told(by_user: bool, attached: bool) -> String {
    let who = match by_user {
        true => "The user revoked this task's folder.",
        false => "ilo revoked this task's folder.",
    };
    let mut told = format!(
        "{who} You can no longer read or change anything in it. Don't try to reach it another \
         way: carry on with the rest, and say what you couldn't finish. Only the user can give a \
         task a folder."
    );
    if attached {
        told.push_str(" The files attached to this task are still there.");
    }
    told
}

/// The activity log's words for a folder the user took back: no name and no path.
pub const REVOKED_LOGGED: &str = "You revoked its folder";

/// A folder a live task holds, as Settings › Agents › Folders in use lists it.
#[derive(Clone, PartialEq)]
pub struct FolderInUse {
    /// The task, which Revoke acts on.
    pub agent: SharedString,
    /// Its title, as its card names it.
    pub task: SharedString,
    /// The folder, by its name as the system's panel showed it.
    pub folder: SharedString,
}

impl std::fmt::Debug for FolderInUse {
    /// The task alone: a folder's name is never logged.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FolderInUse")
            .field("agent", &self.agent)
            .finish()
    }
}

impl Agents {
    /// Every folder a live task holds, the newest task first. An ended task holds none.
    pub fn folders_in_use(&self) -> Vec<FolderInUse> {
        (self.tasks.tasks.iter().rev())
            .filter(|task| task.ended.is_none())
            .filter(|task| task.files.folder.is_some())
            .filter_map(|task| {
                Some(FolderInUse {
                    agent: task.agent.clone().into(),
                    task: task.title.clone(),
                    folder: task.names.folder.clone()?.into(),
                })
            })
            .collect()
    }

    /// The user's Revoke on `agent`'s folder: it goes at once, with no dialog, and the
    /// conversation says so (J2). The way back is a new task with the folder picked again.
    pub fn revoke_folder(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(agent).filter(|task| task.ended.is_none()) else {
            return;
        };
        let Some(folder) = task.files.folder.take() else {
            return;
        };
        let (runs, attached) = (task.cli.is_some(), task.files.attached.is_some());
        let files = Given {
            folder: Some(folder),
            attached: None,
        };
        self.place_in(
            agent,
            Happened::Revoked {
                files: files.clone(),
            },
        );
        self.note_on(agent, 0, ACCESS_ACTION, "", REVOKED_LOGGED, "");
        if runs {
            self.rewall(
                agent,
                Change::TookBack(files),
                revoked_told(true, attached),
                cx,
            );
        }
        cx.notify();
    }

    /// `agent`'s folder is no longer where it was, moved, renamed, deleted or on a disk gone, as
    /// the wall found at its start: ilo ends the grant and the task goes on, started by the
    /// caller (J6).
    pub(super) fn folder_gone(&mut self, agent: &str) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        let Some(folder) = task.files.folder.take() else {
            return;
        };
        let files = Given {
            folder: Some(folder),
            attached: None,
        };
        self.place_in(agent, Happened::FolderGone { files });
    }
}
