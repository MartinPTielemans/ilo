//! The wall a task's assistant runs inside when the task has files (#4101): the system holds the
//! level of files::allows around the assistant's whole process, so the assistant's own checks
//! aren't what keeps it in. The app grants nothing until put_up answers that the wall is up, and a
//! task whose wall can't be put up doesn't start with its files. No system has the launcher yet, so
//! every answer here is no.
//!
//! Which refusals reach ilo, for the line that names them: a command the assistant asks ilo about
//! first (Codex under an approval policy that asks, Claude Code's permission prompt) is refused by
//! ilo's answer and gets the line. One refused by the wall itself, or by the assistant's own
//! sandbox or deny rules, never asks: the command fails inside the task and ilo hears nothing then.
//!
//! No path lets an agent name a file to ilo's file chooser: `upload` takes a ref and a tab, and the
//! field gets only what the user picked. A new tool is read against that.
//!
//! A command inside the task can reach ilo's MCP socket as the assistant's bridge does, and so it
//! is the task's own agent, with its level, its permissions and its activity log. That holds
//! because nothing it can read holds another task's token. A task's token is in Claude Code's MCP
//! config, `Tasks/<agent>.json`, or in Codex's environment, and the wall lets a task read its own
//! folder and its own two files beside it (ilo_reads), never `Tasks/` or another task's.
//!
//! Codex sets every variable in `CODEX_HOME/.env` over its launch environment
//! (codex-rs/arg0/src/lib.rs:276-301 at rust-v0.172.0). ilo never opens that file (#4100), so a
//! proxy set there isn't seen: Codex's own requests go to a host the wall refuses, and its task
//! fails as on a lost network.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::runner::Cli;
use super::tier::Tier;

pub mod listener;

/// Why this system can't wall a task in, for the menu's file and folder rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// The launcher isn't built yet on this system.
    NotBuilt,
    /// The system has no sandbox ilo uses (Windows).
    NoSandbox,
}

/// Whether this system can wall a task in at all. It answers the same for the whole run.
pub fn available() -> Result<(), Unavailable> {
    if cfg!(windows) {
        Err(Unavailable::NoSandbox)
    } else {
        Err(Unavailable::NotBuilt)
    }
}

/// A folder the user granted, resolved at their pick. Only ilo's own folder panel makes one, so no
/// agent tool, page or outside client can hand put_up a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Granted {
    root: PathBuf,
}

impl Granted {
    /// The user's pick, its path already resolved: the folder panel's alone.
    pub fn picked(resolved: PathBuf) -> Granted {
        Granted { root: resolved }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// What a task's assistant is to start inside.
pub struct WallFor<'a> {
    pub cli: Cli,
    pub level: Tier,
    pub folder: Option<&'a Granted>,
    /// ilo's copies of the files the user attached.
    pub attached: Option<&'a Path>,
    pub task_dir: &'a Path,
    /// The task's MCP config, which holds its agent's token.
    pub config: &'a Path,
}

/// What a walled task may read of ilo's own: its folder, its MCP config and the instructions beside
/// it. Nothing else under `Tasks/`, so no task reads another's token.
pub fn ilo_reads(task: &WallFor) -> [PathBuf; 3] {
    [
        task.task_dir.to_path_buf(),
        task.config.to_path_buf(),
        task.config.with_extension("md"),
    ]
}

/// Why a wall wasn't put up: the task doesn't start with its files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WallRefused {
    Unavailable(Unavailable),
    /// The folder can't be granted now: on the never-grantable list, moved or gone.
    Refused(Refusal),
    /// The launcher couldn't apply its rules, and nothing started.
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NeverGrantable,
    Gone,
}

/// The wall a task's assistant runs inside: the runner starts the assistant in it, and it's held
/// for the run and dropped with the process.
#[derive(Debug)]
pub struct Wall {
    /// The granted folder, or None for a task with no folder.
    folder: Option<PathBuf>,
    /// When it went up: the task's own links are the ones made, moved or retargeted since.
    started: SystemTime,
}

impl Wall {
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    pub fn started(&self) -> SystemTime {
        self.started
    }
}

/// Puts the wall up for a task's start, a raise or a take-back. A task with no files needs none and
/// starts as it always has.
pub fn put_up(task: &WallFor) -> Result<Wall, WallRefused> {
    if task.folder.is_none() && task.attached.is_none() {
        return Ok(Wall {
            folder: None,
            started: SystemTime::now(),
        });
    }
    available().map_err(WallRefused::Unavailable)?;
    Err(WallRefused::Failed("the launcher isn't built".into()))
}

/// A link in the granted folder that leads out of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeavingLink {
    /// Where it is, under the folder.
    pub path: PathBuf,
    /// Where it points, as read at the walk.
    pub target: PathBuf,
}

/// Why a walk ended before the whole folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Entries,
    Time,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Walked {
    pub links: Vec<LeavingLink>,
    /// Set when no change time could be trusted or the system's change record dropped some, so the
    /// list is every leaving link in the folder, offered to reveal only and never to delete.
    pub every_leaving: bool,
    /// Set when the walk stopped short, so the line says it checked part of the folder.
    pub stopped: Option<Stop>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Removed {
    pub removed: usize,
    /// Listed but changed since the walk, so left as they are.
    pub left: usize,
}

/// The links the task made, moved or retargeted that leave its folder: once, as the task ends,
/// off the UI thread. No wall has a folder until a launcher is built, so there are none yet.
pub fn links_outside(wall: &Wall) -> Walked {
    let _ = wall;
    Walked::default()
}

/// The user's Delete Links: exactly the listed links, each only if it is still a link with the
/// target the walk read, still leading out. No task can ask for it, and a list of every leaving
/// link offers none.
pub fn remove_links(wall: &Wall, links: &[LeavingLink]) -> Removed {
    let _ = wall;
    Removed {
        removed: 0,
        left: links.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task<'a>(folder: Option<&'a Granted>, attached: Option<&'a Path>) -> WallFor<'a> {
        WallFor {
            cli: Cli::Codex,
            level: Tier::Work,
            folder,
            attached,
            task_dir: Path::new("/nonexistent/Tasks/task"),
            config: Path::new("/nonexistent/Tasks/task.json"),
        }
    }

    #[test]
    fn a_task_reads_none_of_another_tasks_files() {
        let tasks = Path::new("/nonexistent/Tasks");
        let [one, other] =
            ["one", "other"].map(|agent| (tasks.join(agent), tasks.join(format!("{agent}.json"))));
        let reads = |(dir, config): &(PathBuf, PathBuf)| {
            let mut task = task(None, None);
            task.task_dir = dir;
            task.config = config;
            ilo_reads(&task)
        };
        for (mine, theirs) in [(&one, &other), (&other, &one)] {
            let mine = reads(mine);
            for path in reads(theirs) {
                assert!(
                    mine.iter()
                        .all(|read| !path.starts_with(read) && !read.starts_with(&path)),
                    "{path:?}"
                );
            }
            assert!(
                mine.iter()
                    .all(|read| read != tasks && read.starts_with(tasks))
            );
        }
    }

    #[test]
    fn no_system_walls_a_task_in_yet() {
        let expected = match cfg!(windows) {
            true => Unavailable::NoSandbox,
            false => Unavailable::NotBuilt,
        };
        assert_eq!(available(), Err(expected));
    }

    #[test]
    fn a_task_with_files_does_not_start_with_them() {
        let folder = Granted::picked(PathBuf::from("/nonexistent/folder"));
        let attached = Path::new("/nonexistent/attached");
        for (folder, attached) in [(Some(&folder), None), (None, Some(attached))] {
            let refused = put_up(&task(folder, attached)).unwrap_err();
            assert_eq!(refused, WallRefused::Unavailable(available().unwrap_err()));
        }
    }

    #[test]
    fn a_task_without_files_starts_as_it_always_has() {
        let wall = put_up(&task(None, None)).unwrap();
        assert_eq!(wall.folder(), None);
        assert_eq!(links_outside(&wall), Walked::default());
        let link = LeavingLink {
            path: PathBuf::from("a"),
            target: PathBuf::from("/b"),
        };
        assert_eq!(
            remove_links(&wall, &[link]),
            Removed {
                removed: 0,
                left: 1
            }
        );
    }

    /// The folder panel is the one place a folder is granted, so no other source file makes one.
    #[test]
    fn only_the_folder_panel_grants_a_folder() {
        let made = ["Granted", "::picked("].concat();
        let mut folders = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        let mut makers = Vec::new();
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && std::fs::read_to_string(&path).unwrap().contains(&made)
                {
                    makers.push(path.file_name().unwrap().to_string_lossy().into_owned());
                }
            }
        }
        // This file's tests alone, until the panel's file joins the list by name.
        assert_eq!(makers, ["wall.rs"]);
    }
}
