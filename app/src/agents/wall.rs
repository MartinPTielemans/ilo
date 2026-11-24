//! The wall a task's assistant runs inside when the task has files (#4101): the system holds the
//! level of files::allows around the assistant's whole process, so the assistant's own checks
//! aren't what keeps it in. The app grants nothing until put_up answers that the wall is up, and a
//! task whose wall can't be put up doesn't start with its files. The Mac has its launcher
//! (launch.rs), which nothing starts through yet, so every answer here is still no.
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

pub mod launch;
pub mod leftovers;
pub mod listener;
pub mod rules;
/// The Mac's answers for ending what a walled task left running, which nothing asks yet.
#[cfg(target_os = "macos")]
#[path = "../../platform/mac/wall_walk.rs"]
pub mod walk_mac;

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
    /// The assistant's program, as ilo found it.
    pub program: &'a Path,
    /// ilo's MCP socket, which the assistant's bridge reaches.
    pub socket: &'a Path,
    /// ilo's MCP bridge, as `bridge` finds it.
    pub bridge: &'a Path,
    /// Where each walled task's place is made: `Tasks/.tmp`.
    pub places: &'a Path,
    /// The assistant's own home, `CODEX_HOME` or Claude Code's, and Claude Code's `.claude.json`.
    pub cli_home: &'a Path,
    pub claude_config: Option<&'a Path>,
}

/// Where `cli` keeps its own files on this computer, as it reads its variables: its home, and
/// for Claude Code its `.claude.json`, which `CLAUDE_CONFIG_DIR` moves into its home.
pub fn cli_home(cli: Cli) -> (PathBuf, Option<PathBuf>) {
    let home = super::clients::home();
    let from = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    match cli {
        Cli::Codex => (
            from("CODEX_HOME").map_or(home.join(".codex"), PathBuf::from),
            None,
        ),
        Cli::Claude => match from("CLAUDE_CONFIG_DIR").map(PathBuf::from) {
            Some(dir) => (dir.clone(), Some(dir.join(".claude.json"))),
            None => (home.join(".claude"), Some(home.join(".claude.json"))),
        },
    }
}

/// ilo's MCP bridge, beside ilo's own executable: never from a variable, a setting or a task.
pub fn bridge() -> Option<PathBuf> {
    let program = std::env::current_exe().ok()?;
    Some(program.with_file_name(super::bridge::NAME))
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
    /// The launcher couldn't apply its rules, and nothing started. The string is for the dev log
    /// only and never holds a path or a folder's name (#3656); whatever fills it from the
    /// launcher's exit keeps it so.
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NeverGrantable,
    Gone,
}

/// The wall a task's assistant runs inside: the runner starts the assistant in it, and it's held
/// for the run and dropped with the process.
pub struct Wall {
    /// The granted folder, or None for a task with no folder.
    folder: Option<PathBuf>,
    /// When it went up: the task's own links are the ones made, moved or retargeted since.
    started: SystemTime,
    /// What starts a program inside it, or None for a task with no files, whose program starts
    /// as it always has.
    launcher: Option<launch::Launcher>,
    /// The task's own folder for temporary files, which put_up makes for it alone, or None
    /// until it makes one.
    tmp: Option<PathBuf>,
    /// The task's TMP and mark, which its rules allow: what tells what is inside this wall.
    place: Option<std::sync::Arc<leftovers::Place>>,
    /// The one way out of its network, alive while the wall is held.
    listener: Option<listener::Listener>,
}

impl std::fmt::Debug for Wall {
    /// Whether it has a folder and never which, as what holds a Wall may reach a log (#3656).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wall")
            .field("folder", &self.folder.is_some())
            .finish_non_exhaustive()
    }
}

impl Wall {
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    pub fn started(&self) -> SystemTime {
        self.started
    }

    /// Where the assistant inside keeps its temporary files, which its start is told (runner.rs).
    pub fn tmp(&self) -> Option<&Path> {
        self.tmp.as_deref()
    }

    /// The port of its listener, which its start's proxy variables are to name.
    pub fn port(&self) -> Option<u16> {
        self.listener.as_ref().map(listener::Listener::port)
    }

    /// A wall as put_up answers one for a task with files, for a test of what starts inside it:
    /// no launcher, so the test's stand-in starts as any program, and no folder's name.
    #[cfg(test)]
    pub(crate) fn for_test(tmp: Option<PathBuf>) -> Wall {
        Wall {
            folder: None,
            started: SystemTime::now(),
            launcher: None,
            tmp,
            place: None,
            listener: None,
        }
    }
}

impl Granted {
    /// A folder granted in a test of what a task with one does, which is nowhere: the folder
    /// panel alone grants a real one.
    #[cfg(test)]
    pub(crate) fn for_test() -> Granted {
        Granted {
            root: PathBuf::from("/nonexistent/folder"),
        }
    }
}

/// Puts the wall up for a task's start, a raise or a take-back. A task with no files needs none and
/// starts as it always has.
pub fn put_up(task: &WallFor) -> Result<Wall, WallRefused> {
    if task.folder.is_none() && task.attached.is_none() {
        return Ok(Wall {
            folder: None,
            started: SystemTime::now(),
            launcher: None,
            tmp: None,
            place: None,
            listener: None,
        });
    }
    available().map_err(WallRefused::Unavailable)?;
    build(task)
}

/// Why a wall wasn't put up, each a sentence for the dev log that names no path and no count.
const SIGN_IN_LINK: &str = "the assistant's sign-in is a link";
const NO_PROGRAM: &str = "the assistant's program couldn't be resolved";
const NO_BRIDGE: &str = "ilo's bridge isn't beside its program";
const NO_PLACE: &str = "the task's place couldn't be made";
const NO_LISTENER: &str = "the task's listener couldn't be started";
const NOT_RESOLVED: &str = "a path the rules name couldn't be resolved";

/// The wall for a task with files, once the system can wall one in: the granted folder still
/// where it was picked, the assistant's sign-in plain files, then a place of its own, its
/// listener, and its rules. A place made for a wall that isn't put up is removed at once.
fn build(task: &WallFor) -> Result<Wall, WallRefused> {
    let failed = |sentence: &str| WallRefused::Failed(sentence.into());
    let folder = match task.folder {
        Some(granted) => Some(still_there(granted)?),
        None => None,
    };
    // Its own save replaces a sign-in by a rename, so a link there is none of its making.
    for name in rules::sign_in(task.cli) {
        let meta = std::fs::symlink_metadata(task.cli_home.join(name));
        if meta.is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(failed(SIGN_IN_LINK));
        }
    }
    let program = std::fs::canonicalize(task.program).map_err(|_| failed(NO_PROGRAM))?;
    let program_dir = program.parent().ok_or_else(|| failed(NO_PROGRAM))?;
    // A walled task with no bridge would start with no tools, so it starts without its files.
    let bridge = (task.bridge.is_absolute())
        .then(|| std::fs::canonicalize(task.bridge).ok())
        .flatten()
        .filter(|bridge| bridge.is_file())
        .ok_or_else(|| failed(NO_BRIDGE))?;
    let place = leftovers::Place::make(task.places).map_err(|_| failed(NO_PLACE))?;
    let walled: Result<(Wall, listener::Listener), WallRefused> = (|| {
        let listener = listener::Listener::start(task.cli).map_err(|_| failed(NO_LISTENER))?;
        let paths = rules::Paths {
            folder: folder.as_deref(),
            attached: task.attached,
            task_dir: task.task_dir,
            config: task.config,
            tmp: place.tmp(),
            socket: task.socket,
            program_dir,
            bridge: &bridge,
            cli_home: task.cli_home,
            claude_config: task.claude_config,
        };
        let rules = rules::rules(task.cli, task.level, listener.port(), &paths, place.mark())
            .map_err(|why| match why.to_string() == rules::TOO_DEEP {
                true => failed(rules::TOO_DEEP),
                false => failed(NOT_RESOLVED),
            })?;
        let wall = Wall::inside(folder.clone(), &rules, &super::runner::kept(task.cli))?;
        Ok((wall, listener))
    })();
    match walled {
        Ok((mut wall, listener)) => {
            wall.listener = Some(listener);
            Ok(wall.in_place(std::sync::Arc::new(place)))
        }
        Err(refused) => {
            place.discard();
            Err(refused)
        }
    }
}

/// The granted folder, if it's still the folder the user picked: a folder, at the path its pick
/// resolved, with no link in it now. Moved, gone or replaced, it can't be granted now.
fn still_there(granted: &Granted) -> Result<PathBuf, WallRefused> {
    let root = granted.root();
    match std::fs::canonicalize(root) {
        Ok(now) if now == root && now.is_dir() => Ok(now),
        _ => Err(WallRefused::Refused(Refusal::Gone)),
    }
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
    /// Set by pip's walk when no change time could be trusted or the system's change record dropped
    /// some, so the list is every leaving link in the folder, offered to reveal only and never to
    /// delete.
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
            program: Path::new("/nonexistent/codex"),
            socket: Path::new("/nonexistent/Agents.sock"),
            bridge: Path::new("/nonexistent/mcp-bridge"),
            places: Path::new("/nonexistent/Tasks/.tmp"),
            cli_home: Path::new("/nonexistent/home/.codex"),
            claude_config: None,
        }
    }

    /// A made-up profile and home of a test's own under the system's temp folder, gone with it:
    /// never the user's.
    struct Made(PathBuf);

    impl Made {
        fn new(name: &str) -> Made {
            let root =
                std::env::temp_dir().join(format!("ilo-put-up-{name}-{}", std::process::id()));
            _ = std::fs::remove_dir_all(&root);
            for folder in ["granted", "Tasks/task", "home/.codex", "bin", "ilo"] {
                std::fs::create_dir_all(root.join(folder)).unwrap();
            }
            std::fs::write(root.join("bin/codex"), "").unwrap();
            std::fs::write(root.join("ilo/mcp-bridge"), "").unwrap();
            Made(std::fs::canonicalize(&root).unwrap())
        }

        fn at(&self, path: &str) -> PathBuf {
            self.0.join(path)
        }

        /// The places made under its `Tasks/.tmp`, marks and folders.
        fn places(&self) -> usize {
            std::fs::read_dir(self.at("Tasks/.tmp")).map_or(0, |entries| entries.count())
        }
    }

    impl Drop for Made {
        fn drop(&mut self) {
            _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// What build is handed for a task of `made`'s with `granted`, with each path its own.
    fn built(made: &Made, granted: &Granted) -> Result<Wall, WallRefused> {
        let paths = [
            "Tasks/task",
            "Tasks/task.json",
            "bin/codex",
            "Agents.sock",
            "Tasks/.tmp",
            "home/.codex",
            "ilo/mcp-bridge",
        ]
        .map(|path| made.at(path));
        let [task_dir, config, program, socket, places, cli_home, bridge] = &paths;
        build(&WallFor {
            folder: Some(granted),
            task_dir,
            config,
            program,
            socket,
            bridge,
            places,
            cli_home,
            ..task(None, None)
        })
    }

    #[test]
    fn a_folder_moved_gone_or_now_a_link_isnt_walled_and_leaves_no_place() {
        let made = Made::new("gone");
        let gone = Granted::picked(made.at("moved-away"));
        assert_eq!(
            built(&made, &gone).unwrap_err(),
            WallRefused::Refused(Refusal::Gone)
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(made.at("granted"), made.at("now-a-link")).unwrap();
            let linked = Granted::picked(made.at("now-a-link"));
            assert_eq!(
                built(&made, &linked).unwrap_err(),
                WallRefused::Refused(Refusal::Gone)
            );
        }
        assert_eq!(made.places(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_sign_in_that_is_a_link_isnt_walled_and_leaves_no_place() {
        let made = Made::new("sign-in");
        std::fs::write(made.at("elsewhere.json"), "").unwrap();
        std::os::unix::fs::symlink(made.at("elsewhere.json"), made.at("home/.codex/auth.json"))
            .unwrap();
        let granted = Granted::picked(made.at("granted"));
        assert_eq!(
            built(&made, &granted).unwrap_err(),
            WallRefused::Failed(SIGN_IN_LINK.into())
        );
        assert_eq!(made.places(), 0);
    }

    #[test]
    fn a_folder_too_deep_for_the_rules_isnt_walled_and_leaves_no_place() {
        let made = Made::new("deep");
        let deep = (0..launch::PARAMETERS_MAX)
            .fold(made.at("granted"), |path, at| path.join(at.to_string()));
        std::fs::create_dir_all(&deep).unwrap();
        let granted = Granted::picked(deep);
        assert_eq!(
            built(&made, &granted).unwrap_err(),
            WallRefused::Failed(rules::TOO_DEEP.into())
        );
        assert_eq!(made.places(), 0);
    }

    #[test]
    fn a_task_with_no_bridge_beside_ilo_isnt_walled_and_leaves_no_place() {
        let made = Made::new("bridge");
        let granted = Granted::picked(made.at("granted"));
        std::fs::remove_file(made.at("ilo/mcp-bridge")).unwrap();
        let no_bridge = WallRefused::Failed(NO_BRIDGE.into());
        assert_eq!(built(&made, &granted).unwrap_err(), no_bridge);
        // A folder by its name is no bridge either.
        std::fs::create_dir(made.at("ilo/mcp-bridge")).unwrap();
        assert_eq!(built(&made, &granted).unwrap_err(), no_bridge);
        assert_eq!(made.places(), 0);
    }

    /// Where there's no launcher the wall isn't put up and its place goes; on the Mac it's up,
    /// in a place of its own under the tasks' and with its listener.
    #[test]
    fn a_wall_is_put_up_in_a_place_of_its_own_or_leaves_none() {
        let made = Made::new("up");
        let granted = Granted::picked(made.at("granted"));
        let built = built(&made, &granted);
        if cfg!(target_os = "macos") {
            let wall = built.unwrap();
            assert_eq!(wall.folder(), Some(made.at("granted").as_path()));
            assert!(wall.tmp().unwrap().starts_with(made.at("Tasks/.tmp")));
            assert!(wall.port().is_some());
            // Its folder and its mark.
            assert_eq!(made.places(), 2);
        } else {
            assert!(matches!(built, Err(WallRefused::Failed(_))));
            assert_eq!(made.places(), 0);
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
    fn a_wall_printed_names_no_folder() {
        let wall = Wall {
            folder: Some(PathBuf::from("/nonexistent/Private Letters")),
            started: SystemTime::now(),
            launcher: None,
            tmp: None,
            place: None,
            listener: None,
        };
        let printed = format!("{wall:?}");
        assert!(printed.contains("folder: true"), "{printed}");
        assert!(!printed.contains("nonexistent") && !printed.contains("Letters"));
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
