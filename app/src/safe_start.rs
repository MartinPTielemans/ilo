//! Safe start (docs/ux-spec.md › Safe start, #2345). After the browser ended by a crash at launch
//! twice in a row, the next launch starts with extensions and Boosts not loaded, and a card at the
//! sidebar's foot says so. "At launch" is the first minute after the window shows. Turn Back On, or
//! the next quit, ends it. Nothing the user set changes: the core holds the extensions for this
//! launch alone (core/extensions.cc), and the Boosts stay as their files and Settings have them.
//!
//! The count is one small mark in the app's folder, where the last launch got to: written as the
//! app launches, once a minute after its window shows, and as it quits. It's never sent anywhere.

use std::time::Duration;

/// The mark's file, in the app's folder.
pub const MARK: &str = "Last launch";
/// The core's switch, given at every launch: "1" holds the extensions (core/extensions.cc). Every
/// launch says, as a restart starts the browser again with the switches it was given.
pub const SWITCH: &str = "--safe-start";
/// How long after the window shows a crash is still one at launch.
pub const AT_LAUNCH: Duration = Duration::from_secs(60);
/// The crashes at launch in a row after which a launch starts safe.
const CRASHES: u32 = 2;

/// Where a launch got to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// In its first minute, after this many launches in a row that ended by a crash in theirs.
    Launching(u32),
    /// Past its first minute, so a crash now is an ordinary one.
    Running,
    /// Quit.
    Quit,
}

impl Mark {
    /// The mark in the file's words, None for anything else, as a file of an older build.
    pub fn read(text: &str) -> Option<Mark> {
        let mut words = text.split_whitespace();
        let mark = match (words.next()?, words.next()) {
            ("launching", Some(crashes)) => Mark::Launching(crashes.parse().ok()?),
            ("running", None) => Mark::Running,
            ("quit", None) => Mark::Quit,
            _ => return None,
        };
        words.next().is_none().then_some(mark)
    }

    /// The file's line.
    pub fn line(self) -> String {
        match self {
            Mark::Launching(crashes) => format!("launching {crashes}\n"),
            Mark::Running => "running\n".into(),
            Mark::Quit => "quit\n".into(),
        }
    }

    /// This launch's mark, after the last one's: one left launching ended by a crash at launch.
    pub fn after(last: Option<Mark>) -> Mark {
        match last {
            Some(Mark::Launching(crashes)) => Mark::Launching(crashes.saturating_add(1)),
            _ => Mark::Launching(0),
        }
    }

    /// Whether a launch marked so starts safe.
    pub fn is_safe(self) -> bool {
        matches!(self, Mark::Launching(crashes) if crashes >= CRASHES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The launches after `last`, each ended as `ends` says: true for a crash before its minute.
    fn launches(last: Option<Mark>, ends: &[bool]) -> Vec<bool> {
        let mut mark = last;
        ends.iter()
            .map(|crashed| {
                let launch = Mark::after(mark);
                mark = Some(if *crashed { launch } else { Mark::Quit });
                launch.is_safe()
            })
            .collect()
    }

    #[test]
    fn two_crashes_at_launch_in_a_row_start_the_third_safe() {
        assert_eq!(launches(None, &[true, true, false]), [false, false, true]);
    }

    #[test]
    fn one_crash_changes_nothing() {
        assert_eq!(
            launches(Some(Mark::Quit), &[true, false, true, false]),
            [false; 4]
        );
    }

    #[test]
    fn a_safe_launch_that_crashes_at_launch_starts_the_next_safe_too() {
        assert_eq!(
            launches(None, &[true, true, true, false]),
            [false, false, true, true]
        );
    }

    #[test]
    fn a_crash_past_the_first_minute_counts_nothing() {
        // Each launch crashed after its minute had marked it running.
        let after_minute = Mark::after(Some(Mark::Running));
        assert_eq!(after_minute, Mark::Launching(0));
        assert!(!Mark::after(Some(after_minute)).is_safe());
    }

    #[test]
    fn a_quit_ends_it() {
        assert_eq!(Mark::after(Some(Mark::Quit)), Mark::Launching(0));
    }

    #[test]
    fn the_file_reads_back_what_it_wrote_and_nothing_else() {
        for mark in [
            Mark::Launching(0),
            Mark::Launching(2),
            Mark::Running,
            Mark::Quit,
        ] {
            assert_eq!(Mark::read(&mark.line()), Some(mark));
        }
        for text in [
            "",
            "launching",
            "launching x",
            "running 1",
            "quit now",
            "crashed",
        ] {
            assert_eq!(Mark::read(text), None, "{text:?}");
        }
    }
}
