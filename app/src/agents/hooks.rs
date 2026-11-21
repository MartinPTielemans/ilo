//! Hooks (#3668): commands of the user's that run when an agent ends a task, asks them something
//! or hands them a tab. They're the user's alone: set and removed in Settings › Agents, and no
//! agent adds, reads or runs one, or learns there are any. A hook is given the event, the agent's
//! name and the task's title in its environment, never in its command line, and never a page's
//! address. It runs on a thread of its own, so nothing waits for it.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use web_time::SystemTime;

#[cfg(unix)]
#[path = "../../platform/unix/process_group.rs"]
mod process_group;
#[cfg(windows)]
#[path = "../../platform/windows/process_group.rs"]
mod process_group;
/// No processes to group where nothing starts one, as on the web.
#[cfg(not(any(unix, windows)))]
mod process_group {
    pub struct Group;
    pub fn prepare(_: &mut std::process::Command) {}
    pub fn of(_: &std::process::Child) -> std::io::Result<Group> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
    impl Group {
        pub fn kill(&self) {}
    }
}

/// The spec's: how long a hook runs before it's killed, with all it started.
pub const LIMIT: Duration = Duration::from_secs(30);
/// The most of the task's title a hook is given.
const TITLE: usize = 24;

/// What a hook runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    TaskEnded,
    Asked,
    HandedToYou,
}

impl Event {
    pub const ALL: [Event; 3] = [Event::TaskEnded, Event::Asked, Event::HandedToYou];

    /// Its name in the saved settings and in ILO_EVENT.
    pub fn id(self) -> &'static str {
        match self {
            Event::TaskEnded => "task_ended",
            Event::Asked => "asked",
            Event::HandedToYou => "handed_to_you",
        }
    }

    pub fn from_id(id: &str) -> Option<Event> {
        Event::ALL.into_iter().find(|event| event.id() == id)
    }

    /// Its name in Settings.
    pub fn name(self) -> &'static str {
        match self {
            Event::TaskEnded => "When a task ends",
            Event::Asked => "When an agent asks you",
            Event::HandedToYou => "When a tab is handed to you",
        }
    }
}

/// How a task ended, for a hook on TaskEnded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    Done,
    Failed,
    Stopped,
}

impl Ended {
    fn id(self) -> &'static str {
        match self {
            Ended::Done => "done",
            Ended::Failed => "failed",
            Ended::Stopped => "stopped",
        }
    }
}

/// A hook of the user's, and how its last run went.
#[derive(Clone, Debug, PartialEq)]
pub struct Hook {
    pub id: u64,
    pub event: Event,
    pub command: String,
    pub last: Option<Ran>,
    /// Whether it's running now, when its event skips it.
    pub running: bool,
}

/// How a hook's last run went: when it started, why it failed if it did, and how many times its
/// event came while it ran.
#[derive(Clone, Debug, PartialEq)]
pub struct Ran {
    pub at: SystemTime,
    pub failed: Option<String>,
    pub skipped: u32,
}

/// What a hook is given, as environment variables.
pub fn environment(
    event: Event,
    agent: &str,
    task: &str,
    ended: Option<Ended>,
) -> Vec<(&'static str, String)> {
    let task: String = clean(task).chars().take(TITLE).collect();
    let mut given = vec![
        ("ILO_EVENT", event.id().to_string()),
        ("ILO_AGENT", clean(agent)),
        ("ILO_TASK", task),
    ];
    if let Some(ended) = ended {
        given.push(("ILO_ENDED", ended.id().to_string()));
    }
    given
}

/// `text` with only letters and digits of any script, the plain space and `. , - _ :`, the same
/// on every OS, and no word starting with `-`. An agent names its task and itself, and cmd.exe
/// expands a variable before it parses the line it's in, so what a hook is given can't hold what a
/// shell would act on. sh splits an unquoted variable into words, so none may pass the user's
/// command an option.
pub fn clean(text: &str) -> String {
    let kept = (text.chars())
        .filter(|char| char.is_alphanumeric() || matches!(char, ' ' | '.' | ',' | '-' | '_' | ':'));
    let mut clean = String::new();
    for char in kept {
        if char == '-' && matches!(clean.chars().last(), None | Some(' ')) {
            continue;
        }
        clean.push(char);
    }
    clean
}

/// Runs `command` in the user's shell, from their home folder, with `given` in its environment,
/// for at most LIMIT, and says why it failed if it did. It blocks, so it runs on a thread of its
/// own.
pub fn run(command: &str, given: &[(&'static str, String)]) -> Result<(), String> {
    run_for(command, given, LIMIT)
}

/// `run`, killed with all it started after `limit`.
fn run_for(command: &str, given: &[(&'static str, String)], limit: Duration) -> Result<(), String> {
    let mut shell = shell(command);
    shell
        .envs(given.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(home) = std::env::home_dir() {
        shell.current_dir(home);
    }
    process_group::prepare(&mut shell);
    let mut child = shell
        .spawn()
        .map_err(|error| format!("didn't start: {error}"))?;
    let group = process_group::of(&child);
    let stderr = child.stderr.take();
    let (done, ended) = mpsc::channel();
    let waiting = std::thread::Builder::new()
        .name("hook".into())
        .spawn(move || _ = done.send(wait(child, stderr)));
    if waiting.is_err() {
        return Err("didn't start: no thread for it".into());
    }
    match ended.recv_timeout(limit) {
        Ok(result) => result,
        Err(_) => {
            match &group {
                Ok(group) => group.kill(),
                Err(error) => crate::log!("browser: a hook's group couldn't be made: {error}"),
            }
            Err(format!("still running after {} s", limit.as_secs()))
        }
    }
}

/// The shell that runs `command`: sh on the Mac and Linux, cmd.exe on Windows, each without the
/// user's start-up scripts that would print or prompt.
fn shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut shell = crate::child::command("cmd.exe");
        // /s keeps the command's own quotes, taking only the pair around it.
        shell.raw_arg(format!("/d /s /c \"{command}\""));
        shell
    }
    #[cfg(not(windows))]
    {
        let mut shell = crate::child::command("/bin/sh");
        shell.arg("-c").arg(command);
        shell
    }
}

/// Waits for `child` to end, and says why it failed if it did: its exit status, with the last
/// line it wrote to stderr.
fn wait(mut child: Child, stderr: Option<std::process::ChildStderr>) -> Result<(), String> {
    // Only the end of what it wrote is kept, so a chatty hook holds no more than this.
    let mut tail = Vec::new();
    if let Some(mut stderr) = stderr {
        let mut chunk = [0u8; 4096];
        while let Ok(read) = stderr.read(&mut chunk) {
            if read == 0 {
                break;
            }
            tail.extend_from_slice(&chunk[..read]);
            let over = tail.len().saturating_sub(1024);
            tail.drain(..over);
        }
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    if status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&tail);
    let last = said.lines().rev().find(|line| !line.trim().is_empty());
    let exit = match status.code() {
        Some(code) => format!("exit {code}"),
        None => "killed".into(),
    };
    Err(match last {
        Some(line) => format!("{exit}: {}", super::page::clip(line.trim(), 200)),
        None => exit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character sh or cmd.exe acts on is dropped from what a hook is given, and letters and
    /// digits of any script stay (#3668).
    #[test]
    fn what_a_hook_is_given_holds_nothing_a_shell_acts_on() {
        let shells = "&|;<>()$`\\\"'*?[]{}~#!%^=\n\r\t\u{0}\u{7}/+@";
        assert_eq!(clean(shells), "");
        for char in shells.chars() {
            assert_eq!(clean(&format!("a{char}b")), "ab", "{char:?}");
        }
        assert_eq!(
            clean("Book: a table, at 8.30, Lisboa_2"),
            "Book: a table, at 8.30, Lisboa_2"
        );
        assert_eq!(clean("Réserver 東京 مرحبا"), "Réserver 東京 مرحبا");
        assert_eq!(clean("Pay & echo no %PATH%"), "Pay  echo no PATH");
        // No word starts with -, so an unquoted $ILO_TASK hands sh's command no option.
        assert_eq!(clean("-rf ."), "rf .");
        assert_eq!(clean("a -o b"), "a o b");
        assert_eq!(clean("--x"), "x");
        assert_eq!(clean("a  ---x"), "a  x");
        assert_eq!(clean("a &-x"), "a x");
        assert_eq!(clean("Book - a table"), "Book  a table");
        assert_eq!(clean("re-run 2-3"), "re-run 2-3");
        let given = environment(Event::Asked, "-x Codex", "-rf . --all", None);
        assert_eq!(given[1].1, "x Codex");
        assert_eq!(given[2].1, "rf . all");
        let given = environment(
            Event::TaskEnded,
            "Claude & Co",
            "A title far longer than twenty-four",
            Some(Ended::Done),
        );
        assert_eq!(
            given,
            [
                ("ILO_EVENT", "task_ended".to_string()),
                ("ILO_AGENT", "Claude  Co".to_string()),
                ("ILO_TASK", "A title far longer than ".to_string()),
                ("ILO_ENDED", "done".to_string()),
            ]
        );
        assert_eq!(environment(Event::Asked, "Codex", "Find", None).len(), 3);
    }

    /// A hook runs with what it's given in its environment and says how it failed: its exit and
    /// its last line on stderr (#3668).
    #[cfg(unix)]
    #[test]
    fn a_hook_says_how_it_failed() {
        let given = environment(Event::Asked, "Codex", "Find a table", None);
        assert_eq!(run("test \"$ILO_TASK\" = 'Find a table'", &given), Ok(()));
        assert_eq!(
            run("echo first >&2; echo 'went wrong' >&2; exit 3", &given),
            Err("exit 3: went wrong".into())
        );
        assert_eq!(run("exit 1", &given), Err("exit 1".into()));
    }

    /// A hook still running at its limit is killed with what it started, though that's still
    /// running too (#3668).
    #[cfg(unix)]
    #[test]
    fn a_hook_past_its_limit_is_killed_with_what_it_started() {
        let folder = std::env::temp_dir().join(format!("ilo-hook-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let late = folder.join("late");
        // What it started would leave a file once the limit has passed, unless it's killed too.
        let command = format!("(sleep 0.5; touch '{}') & wait", late.display());
        let ran = run_for(&command, &[], Duration::from_millis(200));
        assert_eq!(ran, Err("still running after 0 s".into()));
        std::thread::sleep(Duration::from_millis(800));
        let outlived = late.exists();
        std::fs::remove_dir_all(&folder).unwrap();
        assert!(!outlived, "what the hook started outlived it");
    }

    /// A title holding cmd.exe's command separator runs no second command: what the hook echoes is
    /// the title with the separator dropped (#3668).
    #[cfg(windows)]
    #[test]
    fn a_title_runs_no_second_command_on_windows() {
        let folder = std::env::temp_dir().join(format!("ilo-hook-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let out = folder.join("out.txt");
        let given = environment(Event::Asked, "Codex", "a & echo no", None);
        let command = format!("echo %ILO_TASK%> \"{}\"", out.display());
        assert_eq!(run(&command, &given), Ok(()));
        let said = std::fs::read_to_string(&out).unwrap();
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(said.trim_end(), "a  echo no");
        assert_eq!(said.lines().count(), 1, "{said:?}");
    }
}
