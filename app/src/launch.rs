//! The arguments the app gives Chromium as the browser starts.

/// Chromium's arguments from the app's `own` switches and the arguments the process was `given`,
/// its program first. The switches given come after the app's, so they override them: Chromium
/// keeps a repeated switch's last value.
///
/// On the Mac the app starts Chromium (`bc_main`) with all of them: the program, the switches, and
/// the pages and files it was given. On Windows and Linux Chromium has `started` with the given
/// arguments, and the core adds what the app returns to them (`BC_APP_ENTRY`), so there it's the
/// switches alone: the program, or a page given, would be one more argument there, which Chromium
/// reads as a link from another app to open (#911).
pub fn chromium_args(given: Vec<String>, own: Vec<String>, started: bool) -> Vec<String> {
    let mut given = given.into_iter();
    let program = given.next();
    if !started {
        return program.into_iter().chain(own).chain(given).collect();
    }
    // Chromium reads what follows a bare "--" as arguments, never switches.
    let switches = given
        .take_while(|arg| arg != "--")
        .filter(|arg| is_switch(arg));
    own.into_iter().chain(switches).collect()
}

/// Whether Chromium reads `arg` as a switch: "--name", "-name", or on Windows "/name" too
/// (base::CommandLine).
fn is_switch(arg: &str) -> bool {
    let prefixes: &[&str] = if cfg!(windows) {
        &["--", "-", "/"]
    } else {
        &["--", "-"]
    };
    prefixes
        .iter()
        .any(|prefix| arg.len() > prefix.len() && arg.starts_with(prefix))
}

/// Whether this launch will exit as soon as Chromium has read the `profile`'s lock, as it reads it
/// just after the app's `launching`: it hands its links to the ilo that holds the lock, or the
/// lock is another computer's (ProcessSingleton::NotifyOtherProcessWithTimeout). Work the app
/// starts for a window must not run then: the process exits under it (#2488).
#[cfg(target_os = "linux")]
pub fn exits_at_lock(profile: &std::path::Path) -> bool {
    let Ok(lock) = std::fs::read_link(profile.join("SingletonLock")) else {
        return false;
    };
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    let this_program = std::fs::read_link("/proc/self/exe").ok();
    lock_exits(
        &lock.to_string_lossy(),
        host.trim(),
        std::process::id(),
        |pid| {
            this_program.is_some()
                && std::fs::read_link(format!("/proc/{pid}/exe")).ok() == this_program
        },
    )
}

/// exits_at_lock for a lock reading `lock`, "<host>-<pid>", on `host` in process `own`, with
/// `runs_this_program` saying whether a process is another copy of this program, as Chromium's
/// IsChromeProcess does.
#[cfg(target_os = "linux")]
fn lock_exits(lock: &str, host: &str, own: u32, runs_this_program: impl Fn(u32) -> bool) -> bool {
    let Some((lock_host, pid)) = lock.rsplit_once('-') else {
        return false;
    };
    let Ok(pid) = pid.parse::<u32>() else {
        return false;
    };
    // Chromium takes a lock it finds stale, whose process is gone or isn't ilo, or is this one.
    let held = pid != own && runs_this_program(pid);
    !lock_host.is_empty() && (held || lock_host != host)
}

/// exits_at_lock on Windows, where the ilo that holds the `profile` keeps its "lockfile" open
/// with no one else let write to it (ProcessSingleton::Create): an ilo that has quit, even
/// crashed, has closed it.
#[cfg(target_os = "windows")]
pub fn exits_at_lock(profile: &std::path::Path) -> bool {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    std::fs::OpenOptions::new()
        .write(true)
        .open(profile.join("lockfile"))
        .is_err_and(|error| error.raw_os_error() == Some(ERROR_SHARING_VIOLATION))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn a_launch_exits_at_a_lock_a_running_ilo_or_another_computer_holds() {
        let ilo = |pid| pid == 41;
        assert!(lock_exits("desk-41", "desk", 50, ilo), "hands over to 41");
        assert!(
            lock_exits("my-desk-41", "my-desk", 50, ilo),
            "a host with a dash"
        );
        assert!(
            lock_exits("laptop-7", "desk", 50, ilo),
            "another computer's"
        );
        assert!(
            !lock_exits("desk-7", "desk", 50, ilo),
            "7 is gone or isn't ilo"
        );
        assert!(!lock_exits("desk-50", "desk", 50, |_| true), "its own");
        assert!(!lock_exits("-41", "desk", 50, ilo), "no host");
        assert!(!lock_exits("desk", "desk", 50, ilo), "no pid");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_profile_with_no_lock_starts_the_browser() {
        let dir = std::env::temp_dir().join(format!("ilo-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!exits_at_lock(&dir));
        // A lock this test process holds is its own, which Chromium takes.
        let host = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap();
        let lock = dir.join("SingletonLock");
        std::os::unix::fs::symlink(format!("{}-{}", host.trim(), std::process::id()), &lock)
            .unwrap();
        assert!(!exits_at_lock(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_launch_exits_at_a_lockfile_a_running_ilo_holds() {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_SHARE_READ: u32 = 1;
        let dir = std::env::temp_dir().join(format!("ilo-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!exits_at_lock(&dir), "no lockfile");
        // As Chromium holds it: open for writing, others only let read.
        let held = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(FILE_SHARE_READ)
            .open(dir.join("lockfile"))
            .unwrap();
        assert!(exits_at_lock(&dir), "held by a running ilo");
        drop(held);
        assert!(!exits_at_lock(&dir), "left by an ilo that quit");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    /// The links Chromium opens on Windows and Linux: the arguments it started with that aren't
    /// switches, then the ones in what the app returned, which the core adds after them.
    fn opened(given: &[&str]) -> Vec<String> {
        let own = strings(&["--user-data-dir=/profile"]);
        let added = chromium_args(strings(given), own, true);
        let not_switch = |arg: &&String| !is_switch(arg) && *arg != "--";
        let started = strings(&given[1..]);
        started
            .iter()
            .chain(&added)
            .filter(not_switch)
            .cloned()
            .collect()
    }

    #[test]
    fn a_launch_opens_only_the_pages_and_files_it_was_given_once() {
        // On Windows a path starting with "/" is a switch, as Chromium reads it.
        let file = if cfg!(windows) {
            r"C:\Users\ada\page.html"
        } else {
            "/home/ada/page.html"
        };
        let given = [
            "/opt/ilo/chrome",
            "--ozone-platform=x11",
            "https://example.org/",
            file,
        ];
        assert_eq!(opened(&given), strings(&["https://example.org/", file]));
    }

    #[test]
    fn a_plain_launch_opens_nothing() {
        assert!(opened(&["/opt/ilo/chrome"]).is_empty());
        assert!(opened(&["/opt/ilo/chrome", "--enable-logging=stderr", "--v=1"]).is_empty());
    }

    #[test]
    fn the_switches_given_come_after_the_apps_own() {
        let own = strings(&["--user-data-dir=/profile", "--use-mock-keychain"]);
        let given = strings(&[
            "/opt/ilo/chrome",
            "--user-data-dir=/other",
            "https://example.org/",
        ]);
        assert_eq!(
            chromium_args(given, own, true),
            strings(&[
                "--user-data-dir=/profile",
                "--use-mock-keychain",
                "--user-data-dir=/other"
            ]),
        );
    }

    #[test]
    fn after_a_bare_double_dash_nothing_is_a_switch() {
        let given = strings(&["/opt/ilo/chrome", "--", "--not-a-switch.html"]);
        assert!(chromium_args(given, vec![], true).is_empty());
    }

    #[test]
    fn on_the_mac_the_app_starts_chromium_with_everything() {
        let own = strings(&["--user-data-dir=/profile"]);
        let given = strings(&[
            "/Applications/ilo.app/Contents/MacOS/ilo",
            "--v=1",
            "https://example.org/",
        ]);
        assert_eq!(
            chromium_args(given, own, false),
            strings(&[
                "/Applications/ilo.app/Contents/MacOS/ilo",
                "--user-data-dir=/profile",
                "--v=1",
                "https://example.org/",
            ]),
        );
    }
}
