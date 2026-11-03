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

#[cfg(test)]
mod tests {
    use super::*;

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
