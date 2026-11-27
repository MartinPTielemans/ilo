//! Installing, against stand-ins only: no test runs a maker's installer, its download, or Claude
//! Code (`spawn` refuses a program outside the test's folder). The real steps are read, never run.

use std::ffi::OsStr;

use futures::StreamExt;
use futures::channel::mpsc::unbounded;

use super::*;

/// The real install's programs and arguments, as it would run them, without running them.
fn said(steps: &[Step]) -> Vec<Vec<String>> {
    (steps.iter())
        .map(|step| {
            let program = step.command.get_program().to_string_lossy().into_owned();
            let args = step.command.get_args().map(OsStr::to_string_lossy);
            std::iter::once(program)
                .chain(args.map(|arg| arg.into_owned()))
                .collect()
        })
        .collect()
}

/// Claude Code's install fetches Anthropic's installer over https alone to a file of its own, then
/// runs that file with nothing of ilo's: no flag that changes where or what it installs.
#[test]
fn claude_runs_anthropics_installer_as_published() {
    let folder = Path::new("/made/up/install");
    let steps = steps(Cli::Claude, folder).unwrap();
    let file = match cfg!(windows) {
        true => folder.join("install.ps1"),
        false => folder.join("install.sh"),
    };
    let file = file.to_string_lossy().into_owned();
    let url = if cfg!(windows) { CLAUDE_PS1 } else { CLAUDE_SH };
    let fetch = ["curl", "-fsSL", "--proto", "=https", "-o", &file, url];
    let run: Vec<&str> = match cfg!(windows) {
        true => vec![
            "powershell",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &file,
        ],
        false => vec!["bash", &file],
    };
    assert_eq!(said(&steps), [fetch.to_vec(), run]);
    assert_eq!(steps[0].offline, CURL_OFFLINE);
    assert!(steps[1].offline.is_empty());
}

/// Codex has no road yet, so its row never offers one.
#[test]
fn codex_has_no_install_yet() {
    assert!(steps(Cli::Codex, Path::new("/made/up")).is_none());
    assert!(!Cli::Codex.installs());
    assert!(Cli::Claude.installs());
    let (ended, _) = unbounded();
    assert_eq!(
        start(Cli::Codex, ended).err().map(|error| error.kind()),
        Some(std::io::ErrorKind::Unsupported)
    );
}

#[cfg(unix)]
mod stand_ins {
    use super::*;

    /// A folder of the test's own, under the temp folder `spawn` allows.
    fn folder() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, Ordering::SeqCst);
        let folder =
            std::env::temp_dir().join(format!("install-test-{}-{number}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// A stand-in program in `folder` named `name` that runs `body` as sh.
    fn stand_in(folder: &Path, name: &str, body: &str, offline: &'static [i32]) -> Step {
        use std::os::unix::fs::PermissionsExt;
        let path = folder.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        Step {
            command: crate::child::command(path),
            offline,
        }
    }

    fn ended(steps: Vec<Step>, folder: Option<PathBuf>) -> (Install, Ended) {
        let (send, mut ended) = unbounded();
        let install = Install::run(steps, folder, send);
        let end = futures::executor::block_on(ended.next()).unwrap();
        (install, end)
    }

    #[test]
    fn each_step_runs_in_turn_and_the_install_is_done_once_all_have() {
        let folder = folder();
        let log = folder.join("ran");
        let first = format!("echo fetched >> '{}'", log.display());
        let second = format!("echo installed >> '{}'", log.display());
        let steps = vec![
            stand_in(&folder, "fetch", &first, CURL_OFFLINE),
            stand_in(&folder, "run", &second, &[]),
        ];
        let (_, end) = ended(steps, None);
        assert_eq!(end, Ended::Done);
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "fetched\ninstalled\n"
        );
    }

    /// A fetch that can't reach the maker says the computer is offline, and nothing after it runs.
    #[test]
    fn a_fetch_with_no_connection_says_youre_offline() {
        let folder = folder();
        let ran = folder.join("ran");
        let steps = vec![
            stand_in(&folder, "fetch", "exit 6", CURL_OFFLINE),
            stand_in(&folder, "run", &format!("touch '{}'", ran.display()), &[]),
        ];
        let (_, end) = ended(steps, None);
        assert_eq!(end, Ended::Failed(Some(OFFLINE.into())));
        assert!(!ran.exists());
    }

    /// The installer's own failure has no reason ilo knows, and its words are never kept.
    #[test]
    fn an_installer_that_fails_ends_with_no_reason() {
        let folder = folder();
        let steps = vec![
            stand_in(&folder, "fetch", "true", CURL_OFFLINE),
            stand_in(&folder, "run", "echo 'token-like words' >&2; exit 6", &[]),
        ];
        let (_, end) = ended(steps, None);
        assert_eq!(end, Ended::Failed(None));
    }

    /// Cancel ends the installer and what it started, and the install ends cancelled.
    #[test]
    fn cancel_ends_the_installer_and_what_it_started() {
        let folder = folder();
        let started = folder.join("started");
        let child = folder.join("child");
        let body = format!(
            "sleep 30 & echo $! > '{}'; touch '{}'; wait",
            child.display(),
            started.display()
        );
        let steps = vec![stand_in(&folder, "run", &body, &[])];
        let (send, mut ended) = unbounded();
        let mut install = Install::run(steps, None, send);
        for _ in 0..500 {
            if started.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(started.exists(), "the stand-in never started");
        install.cancel();
        let end = futures::executor::block_on(ended.next()).unwrap();
        assert_eq!(end, Ended::Cancelled);
        // What the installer started went with it.
        let pid: i32 = std::fs::read_to_string(&child)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let mut gone = false;
        for _ in 0..500 {
            if unsafe { libc::kill(pid, 0) } != 0 {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(gone, "the installer's own download outlived Cancel");
    }

    /// The installer's folder goes once the install ends, its file with it.
    #[test]
    fn the_installers_folder_goes_as_the_install_ends() {
        let folder = folder();
        let scratch = folder.join("scratch");
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(scratch.join("install.sh"), "made up").unwrap();
        let steps = vec![stand_in(&folder, "run", "true", &[])];
        let (_, end) = ended(steps, Some(scratch.clone()));
        assert_eq!(end, Ended::Done);
        assert!(!scratch.exists());
    }
}
