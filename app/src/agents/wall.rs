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
#[cfg(unix)]
mod links;
pub mod listener;
pub mod rules;
/// The Mac's answers for ending what a walled task left running, which nothing asks yet.
#[cfg(target_os = "macos")]
#[path = "../../platform/mac/wall_walk.rs"]
pub mod walk_mac;
mod watch;

pub use watch::{Gone, Identity, Watch};

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

/// A folder the user granted, resolved at their pick, with its identity then. Only ilo's own
/// folder panel makes one, so no agent tool, page or outside client can hand put_up a folder.
#[derive(Clone, PartialEq, Eq)]
pub struct Granted {
    root: PathBuf,
    identity: Identity,
}

impl std::fmt::Debug for Granted {
    /// Never its path or the folder's name, as whatever holds a Granted may reach a log (#3656).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Granted").finish_non_exhaustive()
    }
}

impl Granted {
    /// The user's pick, its path already resolved: the folder panel's alone. Gone when the path
    /// isn't a folder by then, or is a link.
    pub fn picked(resolved: PathBuf) -> Result<Granted, Refusal> {
        let identity = Identity::of(&resolved).ok_or(Refusal::Gone)?;
        Ok(Granted {
            root: resolved,
            identity,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether the folder at its path is still the one picked.
    fn still_picked(&self) -> bool {
        Identity::of(&self.root) == Some(self.identity)
    }

    /// Whether a handle opened at its path holds the folder picked, and not one put in its place.
    #[cfg(unix)]
    fn is_open(&self, dir: std::os::fd::BorrowedFd) -> bool {
        Identity::of_open(dir) == Some(self.identity)
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
    /// The user's home, and ilo's own data: the profile the tasks' folder is in.
    pub home: &'a Path,
    pub data: &'a Path,
    /// ilo's own executable.
    pub ilo: &'a Path,
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
    folder: Option<Granted>,
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
        self.folder.as_ref().map(Granted::root)
    }

    /// The watch on the task's granted folder, for the task to hold as long as its wall, or None
    /// for a task with no folder. `gone` is called once, on the UI thread, when the folder goes:
    /// moved, renamed, deleted or its volume gone, it or a folder above it. The grant ends then as
    /// the user's Revoke ends it, and a folder put back at its path isn't granted again.
    pub fn watch(
        &self,
        cx: &mut gpui::App,
        gone: impl FnOnce(Gone, &mut gpui::App) + 'static,
    ) -> Option<Watch> {
        let (told, heard) = futures::channel::oneshot::channel();
        let watch = self.watch_with(move |why| _ = told.send(why))?;
        cx.spawn(async move |cx| {
            // A watch ended with its task drops the sender, and nothing is called.
            if let Ok(why) = heard.await {
                cx.update(|cx| gone(why, cx));
            }
        })
        .detach();
        Some(watch)
    }

    fn watch_with(&self, gone: impl FnOnce(Gone) + Send + 'static) -> Option<Watch> {
        let folder = self.folder.as_ref()?;
        Some(Watch::start(folder.root(), folder.identity, gone))
    }

    pub fn started(&self) -> SystemTime {
        self.started
    }

    /// Where the assistant inside keeps its temporary files, which its start is told (runner.rs).
    pub fn tmp(&self) -> Option<&Path> {
        self.tmp.as_deref()
    }

    /// The port of its listener, which a walled start's HTTPS_PROXY, HTTP_PROXY and ALL_PROXY
    /// name, with NO_PROXY cleared (runner.rs, `sets` and `environment`).
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

    /// The wall of a task given `folder` that went up now, for a test of what's read in it as
    /// the run ends: no launcher, as nothing starts inside it.
    #[cfg(test)]
    pub(crate) fn granted_for_test(folder: Granted) -> Wall {
        Wall {
            folder: Some(folder),
            ..Wall::for_test(None)
        }
    }

    /// As `for_test`, with its listener started, as put_up starts one.
    #[cfg(test)]
    pub(crate) fn for_test_listening(cli: crate::agents::runner::Cli) -> Wall {
        let mut wall = Wall::for_test(None);
        wall.listener = Some(listener::Listener::start(cli).unwrap());
        wall
    }
}

impl Granted {
    /// A folder granted in a test of what a task with one does, which is nowhere: the folder
    /// panel alone grants a real one. Its identity is the root folder's, which no such test reads,
    /// as none puts a wall up with it.
    #[cfg(test)]
    pub(crate) fn for_test() -> Granted {
        Granted {
            root: PathBuf::from("/nonexistent/folder"),
            identity: Identity::of(Path::new("/")).expect("the root is a folder"),
        }
    }

    /// A folder of the test's own, which is there, with its identity, as the panel would give it.
    #[cfg(test)]
    pub(crate) fn at_for_test(root: PathBuf) -> Granted {
        let identity = Identity::of(&root).expect("the test's folder is there");
        Granted { root, identity }
    }

    /// A folder granted at `root` while `was` was there: what's at `root` now isn't it.
    #[cfg(test)]
    pub(crate) fn was_for_test(root: PathBuf, was: &Path) -> Granted {
        let identity = Identity::of(was).expect("the test's folder is there");
        Granted { root, identity }
    }

    /// The same by a name of the test's own, for a test of where a folder's name reaches.
    #[cfg(test)]
    pub(crate) fn named_for_test(name: &str) -> Granted {
        Granted {
            root: Path::new("/nonexistent").join(name),
            ..Granted::for_test()
        }
    }

    /// A real folder of a test's own, granted as the folder panel grants one, for a test of what
    /// is read in it: made here, so the panel's file and this one stay the only makers.
    #[cfg(test)]
    pub(crate) fn picked_for_test(folder: PathBuf) -> Granted {
        Granted::picked(folder).expect("the test's folder is there")
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
    // The folder at its path is still the one picked, or nothing is granted.
    if task.folder.is_some_and(|folder| !folder.still_picked()) {
        return Err(WallRefused::Refused(Refusal::Gone));
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
const OVERLAPS: &str = "the granted folder holds, or is inside, the assistant's own files or ilo's";

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
    // A path the checks below compare that can't be resolved could hide an overlap behind a
    // link, so it refuses the wall: never the path as given. One not made yet is its folder
    // resolved and its own name, as `resolved` answers.
    let resolve = |path: &Path| rules::resolved(path).map_err(|_| failed(NOT_RESOLVED));
    let (cli_home, data, home) = (
        resolve(task.cli_home)?,
        resolve(task.data)?,
        resolve(task.home)?,
    );
    let mut kept_out = vec![cli_home.as_path(), data.as_path()];
    kept_out.extend(folder.as_ref().map(Granted::root));
    let program_named = match program_folder_whole(program_dir, &home, &kept_out) {
        true => rules::Program::Folder(program_dir),
        false => rules::Program::File(&program),
    };
    // A walled task with no bridge would start with no tools, so it starts without its files.
    let bridge = (task.bridge.is_absolute())
        .then(|| std::fs::canonicalize(task.bridge).ok())
        .flatten()
        .filter(|bridge| bridge.is_file())
        .ok_or_else(|| failed(NO_BRIDGE))?;
    // The folder is read whole at the task's level, and only the home's writes are kept out, so
    // a folder over what the assistant or ilo runs on would open it to the task. The pick refuses
    // such a folder first; this is where the wall is made.
    if let Some(granted) = &folder {
        let root = granted.root();
        // An ilo that can't be named, as an empty path, resolves to nothing and refuses too.
        let ilo = resolve(task.ilo)?;
        let claude_config = task.claude_config.map(resolve).transpose()?;
        let files = [
            Some(&program),
            Some(&bridge),
            Some(&ilo),
            claude_config.as_ref(),
        ];
        if cli_home.starts_with(root)
            || root.starts_with(&cli_home)
            || (files.into_iter().flatten()).any(|file| file.starts_with(root))
        {
            return Err(failed(OVERLAPS));
        }
    }
    // The app's developer tools, if the system's link picks an app an administrator installed:
    // read once here, so a link moved later doesn't move the wall. Anything else, today's wall.
    #[cfg(target_os = "macos")]
    let developer = rules::developer::developer(Path::new("/"), 0, &home);
    #[cfg(not(target_os = "macos"))]
    let developer: Option<PathBuf> = None;
    let place = leftovers::Place::make(task.places).map_err(|_| failed(NO_PLACE))?;
    let walled: Result<(Wall, listener::Listener), WallRefused> = (|| {
        let listener = listener::Listener::start(task.cli).map_err(|_| failed(NO_LISTENER))?;
        let paths = rules::Paths {
            folder: folder.as_ref().map(Granted::root),
            attached: task.attached,
            task_dir: task.task_dir,
            config: task.config,
            tmp: place.tmp(),
            socket: task.socket,
            program: program_named,
            bridge: &bridge,
            cli_home: task.cli_home,
            claude_config: task.claude_config,
            developer: developer.as_deref(),
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

/// Whether the rules may name the assistant's program by `dir`, the folder it's in, whole, every
/// path resolved: outside the user's `home`, or inside it in `~/bin` or below one of its
/// dot-folders, never a dot-folder itself; and never a folder that is, holds or is above the home
/// or one of `kept_out`. A program kept loose in a folder of the user's would open that folder
/// to the task, so anywhere else the rules name the program's one file.
pub fn program_folder_whole(dir: &Path, home: &Path, kept_out: &[&Path]) -> bool {
    if (kept_out.iter())
        .chain([&home])
        .any(|kept| kept.starts_with(dir))
    {
        return false;
    }
    let Ok(inside) = dir.strip_prefix(home) else {
        return true;
    };
    let mut names = inside
        .components()
        .map(|name| name.as_os_str().to_string_lossy());
    match (names.next(), names.next()) {
        (Some(first), _) if first == "bin" => true,
        (Some(first), Some(_)) => first.starts_with('.'),
        _ => false,
    }
}

/// The granted folder, if it's still the folder the user picked: the same folder, at the path its
/// pick resolved, with no link in it now. Moved, gone or replaced, it can't be granted now.
fn still_there(granted: &Granted) -> Result<Granted, WallRefused> {
    let root = granted.root();
    match std::fs::canonicalize(root) {
        Ok(now) if now == root && granted.still_picked() => Ok(granted.clone()),
        _ => Err(WallRefused::Refused(Refusal::Gone)),
    }
}

/// A link in the granted folder that leads out of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeavingLink {
    /// Where it is, relative to the folder.
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
    /// Folders the walk couldn't wholly read: one it couldn't open or list, deeper than it goes, on
    /// another device, or holding an entry it couldn't look at. Each counts once.
    pub unread: usize,
    /// The first of them, relative to the folder, for the reveal. It is the user's own folder
    /// layout, so it stays in memory for the line: never a log line, the 30-day file or an
    /// agent's answer.
    pub first_unread: Option<PathBuf>,
    /// Set when the folder itself couldn't be opened as the task ended, so nothing in it was read.
    pub root_unread: bool,
    /// Set when the walk stopped short, so the line says it checked part of the folder.
    pub stopped: Option<Stop>,
}

impl Walked {
    /// Whether its task's line has anything to say: a link found, or a part of the folder it
    /// couldn't read. A clean walk says nothing.
    pub fn any(&self) -> bool {
        !self.links.is_empty() || self.unread > 0 || self.root_unread || self.stopped.is_some()
    }
}

/// What the walk at a walled run's end found, for its task's line and that line's buttons alone,
/// with the wall it was walked in, which Delete Links removes them from. It stays in memory: what
/// it lists is the user's own folder's layout.
#[derive(Clone)]
pub struct Found {
    pub walked: std::sync::Arc<Walked>,
    pub wall: std::sync::Arc<Wall>,
}

impl std::fmt::Debug for Found {
    /// Counts alone, never a path, as what holds one may reach a log (#3656).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Found")
            .field("links", &self.walked.links.len())
            .field("unread", &self.walked.unread)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Found {
    /// The same walk, of the same wall.
    fn eq(&self, other: &Found) -> bool {
        std::sync::Arc::ptr_eq(&self.walked, &other.walked)
            && std::sync::Arc::ptr_eq(&self.wall, &other.wall)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Removed {
    pub removed: usize,
    /// Listed but changed since the walk, so left as they are.
    pub left: usize,
}

/// The links the task made, moved or retargeted that leave its folder: once, as the task ends,
/// off the UI thread, never on a timer or while the task runs. It reads directories and links only
/// (links.rs), and stops at a million entries or 30 s, saying so, and at 64 folders deep, counting
/// what's deeper as unread.
#[cfg(unix)]
pub fn links_outside(wall: &Wall) -> Walked {
    links::outside(wall, links::LIMITS)
}

/// Windows walls no task in, so no wall has a folder.
#[cfg(not(unix))]
pub fn links_outside(_: &Wall) -> Walked {
    Walked::default()
}

/// The user's Delete Links: exactly the listed links, each only if it is still a link the task
/// made, with the target the walk read, still leading out. No task can ask for it, and where the
/// folder keeps no change time to trust, the list was every leaving link, and it deletes none.
#[cfg(unix)]
pub fn remove_links(wall: &Wall, links: &[LeavingLink]) -> Removed {
    self::links::remove(wall, links)
}

#[cfg(not(unix))]
pub fn remove_links(_: &Wall, links: &[LeavingLink]) -> Removed {
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
            home: Path::new("/nonexistent/home"),
            data: Path::new("/nonexistent"),
            ilo: Path::new("/nonexistent/ilo"),
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
            let folders = [
                "granted",
                "Tasks/task",
                "home/.codex",
                "home/.claude",
                "bin",
                "ilo",
                "app",
                "conf",
            ];
            for folder in folders {
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
        built_as(made, granted, Cli::Codex, None)
    }

    /// As `built`, for `cli`'s task: Claude Code's has its own home and its `.claude.json`.
    /// As `built`, for `cli`'s task: Claude Code's has its own home and its `.claude.json`. `odd`
    /// replaces one of ilo's own paths, `"ilo"` or `"data"`, by the one given.
    fn built_as(
        made: &Made,
        granted: &Granted,
        cli: Cli,
        odd: Option<(&str, &Path)>,
    ) -> Result<Wall, WallRefused> {
        let given = |name: &str, default: PathBuf| match odd {
            Some((odd, path)) if odd == name => path.to_path_buf(),
            _ => default,
        };
        let paths = [
            "Tasks/task",
            "Tasks/task.json",
            "bin/codex",
            "Agents.sock",
            "Tasks/.tmp",
            "home/.codex",
            "ilo/mcp-bridge",
            "home",
            "profile",
        ]
        .map(|path| made.at(path));
        let [
            task_dir,
            config,
            program,
            socket,
            places,
            cli_home,
            bridge,
            home,
            data,
        ] = &paths;
        build(&WallFor {
            folder: Some(granted),
            task_dir,
            config,
            program,
            home,
            data: &given("data", data.clone()),
            socket,
            bridge,
            places,
            cli_home: &match cli {
                Cli::Codex => cli_home.clone(),
                Cli::Claude => made.at("home/.claude"),
            },
            claude_config: (cli == Cli::Claude)
                .then(|| made.at("conf/.claude.json"))
                .as_deref(),
            ilo: &given("ilo", made.at("app/browser")),
            cli,
            ..task(None, None)
        })
    }

    /// A granted folder over what the assistant or ilo runs on, or inside the assistant's home,
    /// isn't walled and leaves no place, and the sentence names no path.
    fn overlaps(made: &Made, at: &str, cli: Cli) {
        std::fs::create_dir_all(made.at(at)).unwrap();
        let granted = Granted::picked(made.at(at)).unwrap();
        let refused = built_as(made, &granted, cli, None).unwrap_err();
        assert_eq!(refused, WallRefused::Failed(OVERLAPS.into()), "{at}");
        assert!(!OVERLAPS.contains('/'));
        assert_eq!(made.places(), 0, "{at}");
    }

    /// A path the overlap checks compare that can't be resolved refuses the wall, never compared
    /// as given: here ilo's data two folders below one that is there.
    #[test]
    fn a_path_the_checks_compare_that_cant_be_resolved_isnt_walled() {
        let made = Made::new("unresolved");
        let granted = Granted::picked(made.at("granted")).unwrap();
        let deep = made.at("missing/profile");
        let refused = built_as(&made, &granted, Cli::Codex, Some(("data", &deep))).unwrap_err();
        assert_eq!(refused, WallRefused::Failed(NOT_RESOLVED.into()));
        assert_eq!(made.places(), 0);
    }

    /// An ilo whose executable can't be named, which put_up hands as an empty path, isn't walled.
    #[test]
    fn an_ilo_that_cant_be_named_isnt_walled() {
        let made = Made::new("no-ilo");
        let granted = Granted::picked(made.at("granted")).unwrap();
        let empty = Path::new("");
        let refused = built_as(&made, &granted, Cli::Codex, Some(("ilo", empty))).unwrap_err();
        assert_eq!(refused, WallRefused::Failed(NOT_RESOLVED.into()));
        assert_eq!(made.places(), 0);
    }

    #[test]
    fn a_folder_that_holds_the_assistants_home_isnt_walled() {
        overlaps(&Made::new("holds-home"), "home", Cli::Codex);
    }

    #[test]
    fn a_folder_inside_the_assistants_home_isnt_walled() {
        overlaps(&Made::new("in-home"), "home/.codex/sessions", Cli::Codex);
        overlaps(
            &Made::new("in-claude-home"),
            "home/.claude/projects",
            Cli::Claude,
        );
    }

    #[test]
    fn a_folder_that_holds_claude_codes_config_isnt_walled() {
        overlaps(&Made::new("holds-config"), "conf", Cli::Claude);
    }

    #[test]
    fn a_folder_that_holds_the_program_isnt_walled() {
        overlaps(&Made::new("holds-program"), "bin", Cli::Codex);
    }

    #[test]
    fn a_folder_that_holds_the_bridge_isnt_walled() {
        overlaps(&Made::new("holds-bridge"), "ilo", Cli::Codex);
    }

    #[test]
    fn a_folder_that_holds_ilo_isnt_walled() {
        overlaps(&Made::new("holds-ilo"), "app", Cli::Codex);
    }

    /// A program's folder is read whole only where no folder of the user's is: outside the home,
    /// or in `~/bin` or below a dot-folder of it; never the home, a folder above it, one of its
    /// own folders, a dot-folder itself, or a folder that is or holds one kept out.
    #[test]
    fn a_programs_folder_is_read_whole_only_where_no_folder_of_the_users_is() {
        let home = Path::new("/Users/someone");
        let cli_home = home.join(".codex");
        let data = home.join("Library/Application Support/ilo");
        let granted = Path::new("/Volumes/Work/project");
        let kept_out = [cli_home.as_path(), data.as_path(), granted];
        let whole = |dir: &str| program_folder_whole(Path::new(dir), home, &kept_out);
        for dir in [
            "/opt/homebrew/Cellar/codex/1.0.0/bin",
            "/usr/local/lib/node_modules/@openai/codex/bin",
            "/Users/someone/.local/share/claude/versions",
            "/Users/someone/.nvm/versions/node/v22.0.0/bin",
            "/Users/someone/.local/bin",
            "/Users/someone/bin",
            "/Users/someone/bin/tools",
        ] {
            assert!(whole(dir), "{dir} isn't read whole");
        }
        for dir in [
            "/",
            "/Users",
            "/Users/someone",
            "/Users/someone/Desktop",
            "/Users/someone/Documents/tools",
            "/Users/someone/Downloads",
            "/Users/someone/.local",
            "/Users/someone/.codex",
            "/Users/someone/Library/Application Support",
            "/Users/someone/Library/Application Support/ilo",
            "/Volumes/Work",
            "/Volumes/Work/project",
        ] {
            assert!(!whole(dir), "{dir} is read whole");
        }
        // An assistant's home outside the user's keeps the folders above it out too.
        let elsewhere = [Path::new("/opt/codex-home")];
        assert!(!program_folder_whole(Path::new("/opt"), home, &elsewhere));
        assert!(program_folder_whole(
            Path::new("/opt/bin"),
            home,
            &elsewhere
        ));
    }

    #[test]
    fn a_folder_moved_gone_or_now_a_link_isnt_walled_and_leaves_no_place() {
        let made = Made::new("gone");
        let gone = Granted::picked(made.at("granted")).unwrap();
        std::fs::rename(made.at("granted"), made.at("moved-away")).unwrap();
        assert_eq!(
            built(&made, &gone).unwrap_err(),
            WallRefused::Refused(Refusal::Gone)
        );
        #[cfg(unix)]
        {
            std::fs::create_dir(made.at("now-a-link")).unwrap();
            let linked = Granted::picked(made.at("now-a-link")).unwrap();
            std::fs::remove_dir(made.at("now-a-link")).unwrap();
            std::os::unix::fs::symlink(made.at("moved-away"), made.at("now-a-link")).unwrap();
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
        let granted = Granted::picked(made.at("granted")).unwrap();
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
        let granted = Granted::picked(deep).unwrap();
        assert_eq!(
            built(&made, &granted).unwrap_err(),
            WallRefused::Failed(rules::TOO_DEEP.into())
        );
        assert_eq!(made.places(), 0);
    }

    #[test]
    fn a_task_with_no_bridge_beside_ilo_isnt_walled_and_leaves_no_place() {
        let made = Made::new("bridge");
        let granted = Granted::picked(made.at("granted")).unwrap();
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
        let granted = Granted::picked(made.at("granted")).unwrap();
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
            folder: Some(Granted {
                root: PathBuf::from("/nonexistent/Private Letters"),
                ..Granted::for_test()
            }),
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

    /// A Granted printed on its own, or as a task's folder, names no folder either.
    #[test]
    fn a_granted_folder_printed_names_no_folder() {
        let granted = Granted::named_for_test("Private Letters");
        let task = task(Some(&granted), None);
        for printed in [
            format!("{granted:?}"),
            format!("{granted:#?}"),
            format!("{:?}", Some(granted.clone())),
            format!("{:?}", task.folder),
        ] {
            assert!(printed.starts_with("Granted") || printed.starts_with("Some"));
            assert!(
                !printed.contains("nonexistent") && !printed.contains("Letters"),
                "{printed}"
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

    /// A folder of the test's own, as the folder panel resolves it.
    fn picked(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("ilo-wall-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder.canonicalize().unwrap()
    }

    #[test]
    fn a_task_with_files_does_not_start_with_them() {
        let made = picked("files");
        let folder = Granted::picked(made.clone()).unwrap();
        let attached = Path::new("/nonexistent/attached");
        for (folder, attached) in [(Some(&folder), None), (None, Some(attached))] {
            let refused = put_up(&task(folder, attached)).unwrap_err();
            assert_eq!(refused, WallRefused::Unavailable(available().unwrap_err()));
        }
        std::fs::remove_dir_all(made).unwrap();
    }

    #[test]
    fn only_a_folder_itself_is_picked() {
        let made = picked("only");
        let file = made.join("file");
        std::fs::write(&file, "").unwrap();
        assert_eq!(Granted::picked(file), Err(Refusal::Gone));
        assert_eq!(Granted::picked(made.join("missing")), Err(Refusal::Gone));
        #[cfg(unix)]
        {
            let link = made.join("link");
            std::os::unix::fs::symlink(&made, &link).unwrap();
            assert_eq!(Granted::picked(link), Err(Refusal::Gone));
        }
        assert!(Granted::picked(made.clone()).is_ok());
        std::fs::remove_dir_all(made).unwrap();
    }

    /// The folder at the picked path is checked again at the grant, so one swapped meanwhile isn't
    /// granted. Windows keeps no identity, as it has no wall, so there it's refused for that.
    #[test]
    fn a_folder_swapped_since_the_pick_is_not_granted() {
        let made = picked("swapped");
        let folder = made.join("folder");
        std::fs::create_dir(&folder).unwrap();
        let granted = Granted::picked(folder.clone()).unwrap();
        std::fs::rename(&folder, made.join("away")).unwrap();
        std::fs::create_dir(&folder).unwrap();
        let refused = put_up(&task(Some(&granted), None)).unwrap_err();
        let expected = match cfg!(unix) {
            true => WallRefused::Refused(Refusal::Gone),
            false => WallRefused::Unavailable(available().unwrap_err()),
        };
        assert_eq!(refused, expected);
        std::fs::remove_dir_all(made).unwrap();
    }

    #[test]
    fn a_task_without_files_starts_as_it_always_has() {
        let wall = put_up(&task(None, None)).unwrap();
        assert_eq!(wall.folder(), None);
        assert!(wall.watch_with(|_| ()).is_none(), "no folder, no watch");
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
        // This file's tests alone: no surface grants one until the composer's `+` brings the
        // system's panel back (#4140).
        makers.sort();
        assert_eq!(makers, ["wall.rs"]);
    }
}
