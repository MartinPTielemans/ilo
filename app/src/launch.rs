//! The arguments the app gives Chromium as the browser starts.

/// Chromium's arguments from the app's `own` switches and the arguments the process was `given`,
/// its program first. The switches given come after the app's, so they override them: Chromium
/// keeps a repeated switch's last value. The feature lists are the exception: the app's and the
/// given ones are joined into one switch each, so a launcher's own list, as the Flatpak's Cobalt
/// gives, doesn't turn the app's back off (#3158).
///
/// On the Mac the app starts Chromium (`bc_main`) with all of them: the program, the switches, and
/// the pages and files it was given. On Windows and Linux Chromium has `started` with the given
/// arguments, and the core adds what the app returns to them (`BC_APP_ENTRY`), so there it's the
/// switches alone: the program, or a page given, would be one more argument there, which Chromium
/// reads as a link from another app to open (#911). The core adds the joined lists after the given
/// ones, so they're the ones Chromium keeps.
pub fn chromium_args(given: Vec<String>, own: Vec<String>, started: bool) -> Vec<String> {
    let mut given = given.into_iter();
    let program = given.next();
    let mut given: Vec<String> = given.collect();
    // Chromium reads what follows a bare "--" as arguments, never switches.
    let after = given.split_off(
        given
            .iter()
            .position(|arg| arg == "--")
            .unwrap_or(given.len()),
    );
    if started {
        given.retain(|arg| is_switch(arg));
        return join_feature_lists(own.into_iter().chain(given));
    }
    program
        .into_iter()
        .chain(join_feature_lists(own.into_iter().chain(given)))
        .chain(after)
        .collect()
}

/// The feature lists Chromium reads, each of which it keeps only the last of.
const FEATURE_LISTS: [&str; 2] = ["enable-features", "disable-features"];

/// `args` with each feature list's switches joined into one, where the first was, its features in
/// order and each once.
fn join_feature_lists(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let args: Vec<String> = args.into_iter().collect();
    let mut joined: [Vec<&str>; 2] = Default::default();
    for arg in &args {
        if let Some((list, features)) = feature_list(arg) {
            for feature in features.split(',').filter(|feature| !feature.is_empty()) {
                if !joined[list].contains(&feature) {
                    joined[list].push(feature);
                }
            }
        }
    }
    let mut placed = [false; 2];
    let mut out = Vec::with_capacity(args.len());
    for arg in &args {
        match feature_list(arg) {
            Some((list, _)) if placed[list] => {}
            Some((list, _)) => {
                placed[list] = true;
                if !joined[list].is_empty() {
                    out.push(format!(
                        "--{}={}",
                        FEATURE_LISTS[list],
                        joined[list].join(",")
                    ));
                }
            }
            None => out.push(arg.clone()),
        }
    }
    out
}

/// Which of FEATURE_LISTS `arg` gives, and its features, as Chromium reads the switch.
fn feature_list(arg: &str) -> Option<(usize, &str)> {
    let name = ["--", "-"]
        .iter()
        .chain(if cfg!(windows) { &["/"][..] } else { &[] })
        .find_map(|prefix| arg.strip_prefix(prefix))?;
    FEATURE_LISTS
        .iter()
        .enumerate()
        .find_map(|(list, switch)| Some((list, name.strip_prefix(switch)?.strip_prefix('=')?)))
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

/// The switch that keeps Chromium to its FATAL lines in a launch the checks don't run (`checked`),
/// unless the arguments it was `given` ask for its logging, as a launch to debug it does. A
/// release build writes Chromium's ERROR lines to stderr, which a desktop can keep, and the core's
/// and Chromium's own can name a site the user browses (#3731). A check reads the core's lines, so
/// a launch it runs keeps them.
pub fn quiet_log_level(checked: bool, given: &[String]) -> Option<String> {
    const ASKED: [&str; 4] = ["enable-logging", "v", "vmodule", "log-level"];
    // Chromium reads what follows a bare "--" as arguments, never switches.
    let asked = given
        .iter()
        .take_while(|arg| *arg != "--")
        .filter(|arg| is_switch(arg))
        .any(|arg| {
            let name = arg.trim_start_matches(['-', '/']);
            ASKED.contains(&name.split('=').next().unwrap_or(name))
        });
    // 3 is logging::LOGGING_FATAL, which Chromium writes at any level.
    (!checked && !asked).then(|| "--log-level=3".to_string())
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

    #[test]
    fn chromium_writes_no_error_line_unless_a_check_runs_it_or_its_logging_was_asked_for() {
        let given = |args: &[&str]| args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        let quiet = Some("--log-level=3".to_string());
        assert_eq!(
            quiet_log_level(false, &given(&["Browser"])),
            quiet,
            "a user's launch"
        );
        assert_eq!(
            quiet_log_level(false, &given(&["Browser", "--verbose", "--", "--v=1"])),
            quiet,
            "neither another switch nor an argument after -- asks"
        );
        assert_eq!(
            quiet_log_level(true, &given(&["Browser"])),
            None,
            "a check's"
        );
        for asked in [
            "--enable-logging=stderr",
            "--enable-logging",
            "--v=1",
            "--vmodule=platform_auth_policy_observer=1",
            "--log-level=0",
            "-v=2",
        ] {
            assert_eq!(
                quiet_log_level(false, &given(&["Browser", asked])),
                None,
                "{asked}"
            );
        }
    }

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
    fn a_launchers_feature_lists_join_the_apps_own() {
        // As the Flatpak's Cobalt gives them (#3158): Chromium would keep only the last.
        let own = strings(&[
            "--user-data-dir=/profile",
            "--disable-features=LinkCopiedToast,TabHoverCardImages",
            "--enable-features=PartitionAllocAdaptiveMemoryReclaimInterval",
        ]);
        let given = strings(&[
            "/app/ilo/chrome",
            "--disable-features=WebAssemblyTrapHandler,LinkCopiedToast",
            "--disable-features=DesktopPWAsRunOnOsLogin",
            "--enable-features=WebRtcPipeWireCamera",
            "--ozone-platform=wayland",
        ]);
        assert_eq!(
            chromium_args(given, own, true),
            strings(&[
                "--user-data-dir=/profile",
                "--disable-features=LinkCopiedToast,TabHoverCardImages,WebAssemblyTrapHandler,\
                 DesktopPWAsRunOnOsLogin",
                "--enable-features=PartitionAllocAdaptiveMemoryReclaimInterval,WebRtcPipeWireCamera",
                "--ozone-platform=wayland",
            ]),
        );
    }

    #[test]
    fn an_empty_feature_list_adds_nothing() {
        let own = strings(&["--disable-features=LinkCopiedToast"]);
        let given = strings(&[
            "/opt/ilo/chrome",
            "--disable-features=",
            "--enable-features=",
        ]);
        assert_eq!(
            chromium_args(given, own, true),
            strings(&["--disable-features=LinkCopiedToast"]),
        );
    }

    #[test]
    fn on_the_mac_a_feature_list_after_a_bare_double_dash_is_a_page() {
        let own = strings(&["--disable-features=LinkCopiedToast"]);
        let given = strings(&[
            "/Applications/ilo.app/Contents/MacOS/ilo",
            "--disable-features=TranslateToast",
            "--",
            "--disable-features=page.html",
        ]);
        assert_eq!(
            chromium_args(given, own, false),
            strings(&[
                "/Applications/ilo.app/Contents/MacOS/ilo",
                "--disable-features=LinkCopiedToast,TranslateToast",
                "--",
                "--disable-features=page.html",
            ]),
        );
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
