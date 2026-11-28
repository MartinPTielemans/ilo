//! The box an assistant's whole process runs inside (#4101): fixed rules, the same at every
//! level, which the system holds around it, so the assistant's own checks aren't what keeps it in.
//! It may read its program, read and write its own sign-in, read its run's folder and its MCP
//! config, keep its own temporary files, reach ilo's MCP socket, and reach the network only
//! through the box's listener. Nothing of the user's is in it: files reach the model only through
//! ilo's own tools, so a change of level never touches the box. On the Mac every Claude Code run
//! starts in one (task.rs, broom.rs, ready.rs); Windows has no box, and Linux runs no Claude Code
//! until it has one.
//!
//! A command inside the box can reach ilo's MCP socket as the assistant's bridge does, and so it
//! is the run's own agent, with its level, its permissions and its activity log. That holds
//! because nothing it can read holds another run's token. A run's token is in Claude Code's MCP
//! config, `Tasks/<agent>.json`, or in Codex's environment, and the box lets a run read its own
//! folder and its own two files beside it (ilo_reads), never `Tasks/` or another run's.
//!
//! Codex sets every variable in `CODEX_HOME/.env` over its launch environment
//! (codex-rs/arg0/src/lib.rs:276-301 at rust-v0.172.0). ilo never opens that file (#4100), so a
//! proxy set there isn't seen: Codex's own requests go to a host the box refuses, and its run
//! fails as on a lost network.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::runner::Cli;

pub mod launch;
pub mod leftovers;
pub mod listener;
pub mod rules;
/// The Mac's answers for ending what a boxed run left running.
#[cfg(target_os = "macos")]
#[path = "../../platform/mac/wall_walk.rs"]
pub mod walk_mac;

/// Why this system can't box a run in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// The launcher isn't on this system: Linux until its box (slice 8), or a Mac build without
    /// it.
    NotBuilt,
    /// The system has no sandbox ilo uses (Windows).
    NoSandbox,
}

/// Whether this system can box a run in at all: the Mac, with ilo's launcher beside its program.
/// It answers the same for the whole run.
pub fn available() -> Result<(), Unavailable> {
    if cfg!(windows) {
        return Err(Unavailable::NoSandbox);
    }
    match launch::launcher().is_some_and(|launcher| launcher.is_file()) {
        true => Ok(()),
        false => Err(Unavailable::NotBuilt),
    }
}

/// Where a run of `cli` starts on this system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runs {
    /// Inside its box.
    Boxed,
    /// As a program of the user's, with none of its own tools: Codex until its box (slice 6),
    /// and Claude Code on Windows, which has none.
    Unboxed,
    /// Not at all: Claude Code where a box could be and isn't, Linux until it has one.
    Not,
}

/// Where a run of `cli` starts, by what `available` answered: Claude Code runs only inside its box
/// where the system has a sandbox.
pub fn runs(cli: Cli, available: Result<(), Unavailable>) -> Runs {
    match (cli, available) {
        (Cli::Codex, _) => Runs::Unboxed,
        (Cli::Claude, Ok(())) => Runs::Boxed,
        (Cli::Claude, Err(Unavailable::NoSandbox)) => Runs::Unboxed,
        (Cli::Claude, Err(Unavailable::NotBuilt)) => Runs::Not,
    }
}

/// A folder the user granted, resolved at their pick, with its identity then. Only ilo's own
/// folder panel makes one, so no agent tool, page or outside client can hand ilo a folder.
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
    pub fn still_picked(&self) -> bool {
        Identity::of(&self.root) == Some(self.identity)
    }
}

/// A folder's own identity, so a folder of the same name, or a link put in its place, isn't it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    device: u64,
    inode: u64,
}

impl Identity {
    /// The folder at `path` itself, never through a link at its end: None when nothing is there,
    /// or a link or a file is.
    pub fn of(path: &Path) -> Option<Identity> {
        let found = std::fs::symlink_metadata(path).ok()?;
        if !found.file_type().is_dir() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Some(Identity {
                device: found.dev(),
                inode: found.ino(),
            })
        }
        // Windows keeps no identity ilo reads, so a folder there is only itself by its path.
        #[cfg(not(unix))]
        Some(Identity {
            device: 0,
            inode: 0,
        })
    }
}

/// What a run's assistant is to start inside.
pub struct WallFor<'a> {
    pub cli: Cli,
    /// The folder the run starts in, which it reads and never writes.
    pub task_dir: &'a Path,
    /// The run's MCP config, which holds its agent's token.
    pub config: &'a Path,
    /// The assistant's program, as ilo found it.
    pub program: &'a Path,
    /// The user's home, and ilo's own data: the profile the tasks' folder is in.
    pub data: &'a Path,
    pub home: &'a Path,
    /// ilo's MCP socket, which the assistant's bridge reaches.
    pub socket: &'a Path,
    /// ilo's MCP bridge, as `bridge` finds it: None for a run with none of ilo's tools.
    pub bridge: Option<&'a Path>,
    /// Where each boxed run's place is made: `Tasks/.tmp`.
    pub places: &'a Path,
    /// The assistant's own home, `CODEX_HOME` or Claude Code's, and Claude Code's `.claude.json`.
    pub cli_home: &'a Path,
    pub claude_config: Option<&'a Path>,
    /// Whether the run keeps its conversation (kept.rs), which Claude Code files in a folder of
    /// its home named for the run's folder: the one place in its home it may write but its sign-in.
    pub keeps: bool,
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

/// What a boxed run may read of ilo's own: its folder, its MCP config and the instructions beside
/// it. Nothing else under `Tasks/`, so no run reads another's token.
pub fn ilo_reads(task: &WallFor) -> [PathBuf; 3] {
    [
        task.task_dir.to_path_buf(),
        task.config.to_path_buf(),
        task.config.with_extension("md"),
    ]
}

/// Why a box wasn't put up: nothing starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WallRefused {
    Unavailable(Unavailable),
    /// The launcher couldn't apply its rules, and nothing started. The string is for the dev log
    /// only and never holds a path or a folder's name (#3656); whatever fills it from the
    /// launcher's exit keeps it so.
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Gone,
}

/// The box a run's assistant runs inside: the runner starts the assistant in it, and it's held
/// for the run and dropped with the process.
pub struct Wall {
    /// When it went up.
    started: SystemTime,
    /// What starts a program inside it, or None in a test whose stand-in starts as any program.
    launcher: Option<launch::Launcher>,
    /// The run's own folder for temporary files, which put_up makes for it alone.
    tmp: Option<PathBuf>,
    /// The run's TMP and mark, which its rules allow: what tells what is inside this box.
    place: Option<std::sync::Arc<leftovers::Place>>,
    /// The one way out of its network, alive while the box is held.
    listener: Option<listener::Listener>,
}

impl std::fmt::Debug for Wall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wall").finish_non_exhaustive()
    }
}

impl Wall {
    pub fn started(&self) -> SystemTime {
        self.started
    }

    /// Where the assistant inside keeps its temporary files, which its start is told (runner.rs).
    pub fn tmp(&self) -> Option<&Path> {
        self.tmp.as_deref()
    }

    /// The port of its listener, which a boxed start's HTTPS_PROXY, HTTP_PROXY and ALL_PROXY
    /// name, with NO_PROXY cleared (runner.rs, `sets` and `environment`).
    pub fn port(&self) -> Option<u16> {
        self.listener.as_ref().map(listener::Listener::port)
    }

    /// A box as put_up answers one, for a test of what starts inside it: no launcher, so the
    /// test's stand-in starts as any program.
    #[cfg(test)]
    pub(crate) fn for_test(tmp: Option<PathBuf>) -> Wall {
        Wall {
            started: SystemTime::now(),
            launcher: None,
            tmp,
            place: None,
            listener: None,
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
    /// A folder granted in a test of what a task with one does: the folder panel alone grants a
    /// real one. Its identity is the root folder's, which no such test reads.
    #[cfg(test)]
    pub(crate) fn for_test() -> Granted {
        Granted {
            root: PathBuf::from("/nonexistent/folder"),
            identity: Identity::of(Path::new("/")).expect("the root is a folder"),
        }
    }

    /// The same at `root`, a real folder, as the panel would grant it.
    #[cfg(test)]
    pub(crate) fn at_for_test(root: PathBuf) -> Granted {
        Granted {
            identity: Identity::of(&root).expect("a folder"),
            root,
        }
    }

    /// The same by a name of the test's own, for a test of where a folder's name reaches.
    #[cfg(test)]
    pub(crate) fn named_for_test(name: &str) -> Granted {
        Granted {
            root: Path::new("/nonexistent").join(name),
            ..Granted::for_test()
        }
    }
}

/// Puts the box up for a run's start, where this system has one.
pub fn put_up(task: &WallFor) -> Result<Wall, WallRefused> {
    available().map_err(WallRefused::Unavailable)?;
    build(task)
}

/// What a box with none of ilo's tools names for ilo's socket, its bridge and its config: a name
/// in the run's own folder that nothing makes, so it's given nothing the folder doesn't hold.
const NO_TOOLS: &str = "no-tools";

/// The box for a run of `cli` with none of ilo's tools, the broom's sort or a look at its models:
/// its folder is `folder`, a temporary one of the run's own that its place is made in too, so
/// nothing of it outlives the folder, and it keeps nothing.
pub fn without_tools(cli: Cli, program: &Path, folder: &Path) -> Result<Wall, WallRefused> {
    let (cli_home, claude_config) = cli_home(cli);
    let none = folder.join(NO_TOOLS);
    put_up(&WallFor {
        cli,
        task_dir: folder,
        config: &none,
        program,
        data: folder,
        home: &super::clients::home(),
        socket: &none,
        bridge: None,
        places: folder,
        cli_home: &cli_home,
        claude_config: claude_config.as_deref(),
        keeps: false,
    })
}

/// Why a box wasn't put up, each a sentence for the dev log that names no path and no count.
const SIGN_IN_LINK: &str = "the assistant's sign-in is a link";
const NO_PROGRAM: &str = "the assistant's program couldn't be resolved";
const NO_BRIDGE: &str = "ilo's bridge isn't beside its program";
const NO_PLACE: &str = "the task's place couldn't be made";
const NO_LISTENER: &str = "the task's listener couldn't be started";
const NOT_RESOLVED: &str = "a path the rules name couldn't be resolved";
const NO_PROJECT: &str = "the run's folder has no name its assistant keeps conversations by";

/// The box for a run, once the system can box one in: the assistant's sign-in plain files, then a
/// place of its own, its listener, and its rules. A place made for a box that isn't put up is
/// removed at once.
fn build(task: &WallFor) -> Result<Wall, WallRefused> {
    let failed = |sentence: &str| WallRefused::Failed(sentence.into());
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
    // link, so it refuses the box: never the path as given. One not made yet is its folder
    // resolved and its own name, as `resolved` answers.
    let resolve = |path: &Path| rules::resolved(path).map_err(|_| failed(NOT_RESOLVED));
    let (cli_home, data, home) = (
        resolve(task.cli_home)?,
        resolve(task.data)?,
        resolve(task.home)?,
    );
    let kept_out = [cli_home.as_path(), data.as_path()];
    let program_named = match program_folder_whole(program_dir, &home, &kept_out) {
        true => rules::Program::Folder(program_dir),
        false => rules::Program::File(&program),
    };
    // A boxed run of ilo's tools with no bridge would start with none, so it doesn't start.
    let bridge = match task.bridge {
        Some(bridge) => (bridge.is_absolute())
            .then(|| std::fs::canonicalize(bridge).ok())
            .flatten()
            .filter(|bridge| bridge.is_file())
            .ok_or_else(|| failed(NO_BRIDGE))?,
        None => task.task_dir.join(NO_TOOLS),
    };
    // The folder its assistant files a kept conversation in, by the run's folder as the program
    // sees it, resolved: its home's `projects/<name>` (kept.rs).
    let project = match task.keeps {
        false => None,
        true => {
            let dir = resolve(task.task_dir)?;
            let name = super::kept::project_name(&dir).ok_or_else(|| failed(NO_PROJECT))?;
            Some(resolve(&cli_home.join(rules::PROJECTS_NAME))?.join(name))
        }
    };
    // The app's developer tools, if the system's link picks an app an administrator installed:
    // read once here, so a link moved later doesn't move the box. Anything else, today's box.
    #[cfg(target_os = "macos")]
    let developer = rules::developer::developer(Path::new("/"), 0, &home);
    #[cfg(not(target_os = "macos"))]
    let developer: Option<PathBuf> = None;
    let place = leftovers::Place::make(task.places).map_err(|_| failed(NO_PLACE))?;
    let walled: Result<(Wall, listener::Listener), WallRefused> = (|| {
        let listener = listener::Listener::start(task.cli).map_err(|_| failed(NO_LISTENER))?;
        let paths = rules::Paths {
            task_dir: task.task_dir,
            config: task.config,
            tmp: place.tmp(),
            socket: task.socket,
            program: program_named,
            bridge: &bridge,
            cli_home: task.cli_home,
            claude_config: task.claude_config,
            project: project.as_deref(),
            developer: developer.as_deref(),
        };
        let rules = rules::rules(task.cli, listener.port(), &paths, place.mark()).map_err(
            |why| match why.to_string() == rules::TOO_DEEP {
                true => failed(rules::TOO_DEEP),
                false => failed(NOT_RESOLVED),
            },
        )?;
        let wall = Wall::inside(&rules, &super::runner::kept(task.cli))?;
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
/// to the run, so anywhere else the rules name the program's one file.
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

#[cfg(test)]
mod tests;
