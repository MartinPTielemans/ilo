//! The daily usage count (#1977): what a copy sends, and when, decided from the date of its last
//! count, the only thing it keeps for it besides the user's yes. It's off until the user says yes,
//! in onboarding or Settings › Privacy (#2978). The core posts it (bc_send_daily_count) to the
//! endpoint the build names; this decides only whether one is due today and its flags.

use chrono::Datelike;
// The app's crates on Linux and Windows have no chrono of their own, so the host takes it here.
pub use chrono::NaiveDate;

/// The file in the app's data, beside the profiles, with the UTC date of the copy's last count.
pub const LAST: &str = "daily-count";
/// The file that's the user's yes, in onboarding or Settings › Privacy › Send a Daily Usage Count:
/// a copy sends a count only while it's there. A copy from before the count was opt-in has none,
/// so one that sent counts under that default stops, with no ask after the update.
pub const YES: &str = "daily-count-yes";
/// The file that turned the count off under that default, which a yes takes away.
pub const OLD_OFF: &str = "no-daily-count";
/// The Chromium switch, read by the core too, naming where the count goes. Only a build that names
/// a server passes it, or a check.
pub const SWITCH: &str = "--daily-count-url";

/// A count's flags besides `daily`, which is always true.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    /// The copy's first count, the first it sends after the user's yes.
    pub first: bool,
    /// The copy's first count this ISO week.
    pub weekly: bool,
    /// The copy's first count this calendar month.
    pub monthly: bool,
}

/// The count due `today`, given the date of the `last` one, or none if today's has gone. A last
/// date after today, as a clock set back leaves, counts as today's, so no day counts twice.
pub fn due(last: Option<NaiveDate>, today: NaiveDate) -> Option<Flags> {
    if last.is_some_and(|last| last >= today) {
        return None;
    }
    Some(Flags {
        // Only the copy's first count: whenever the yes came, and however long the first run was
        // open, and a copy that sent counts before the yes was asked for never counts as new.
        first: last.is_none(),
        weekly: last.is_none_or(|last| last.iso_week() != today.iso_week()),
        monthly: last
            .is_none_or(|last| (last.year(), last.month()) != (today.year(), today.month())),
    })
}

/// Today, the UTC day, which a count is for.
pub fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

/// The date `LAST` holds, as `date` writes it.
pub fn parse(text: &str) -> Option<NaiveDate> {
    let mut parts = text.trim().splitn(3, '-').map(str::parse::<u32>);
    let (Some(Ok(year)), Some(Ok(month)), Some(Ok(day))) =
        (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    NaiveDate::from_ymd_opt(year as i32, month, day)
}

/// Whether the copy whose app data is `dir` sends a count: only once its user has said yes.
pub fn on(dir: &std::path::Path) -> bool {
    dir.join(YES).exists()
}

/// Keeps the user's answer in `dir`: a yes as its file, and a no as none, so the copy sends
/// nothing. A yes takes the old default's off file with it.
pub fn answer(dir: &std::path::Path, yes: bool) -> std::io::Result<()> {
    if !yes {
        return match std::fs::remove_file(dir.join(YES)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            done => done,
        };
    }
    std::fs::write(dir.join(YES), "")?;
    _ = std::fs::remove_file(dir.join(OLD_OFF));
    Ok(())
}

/// `2026-11-06`.
pub fn date(day: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", day.year(), day.month(), day.day())
}

/// The endpoint the arguments the process was `given` name, else the build's own `named`, which
/// only the prod build has. A dev build or a check names none unless given one, and sends nothing.
pub fn endpoint(given: &[String], named: Option<&str>) -> Option<String> {
    let prefix = format!("{SWITCH}=");
    let given = given.iter().rev().find_map(|arg| arg.strip_prefix(&prefix));
    given
        .or(named)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_owned)
}

/// What Settings › Privacy says under the switch: exactly what a count holds.
pub fn words(app: &str) -> String {
    format!(
        "Once a day, with no ID: {app}'s version and channel, your system and its version, your \
         chip, and whether it's this copy's first count, or its first this week or month."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> NaiveDate {
        parse(text).unwrap()
    }

    /// App data of a copy of its own, made empty, as a new one is.
    fn copy(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ilo-count-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_new_copy_sends_nothing_until_its_user_says_yes() {
        let dir = copy("new");
        assert!(!on(&dir));
        answer(&dir, false).unwrap();
        assert!(!on(&dir), "a no turned the count on");
        answer(&dir, true).unwrap();
        assert!(on(&dir), "a yes left the count off");
        answer(&dir, false).unwrap();
        assert!(!on(&dir), "a no after a yes left the count on");
        _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_from_before_the_ask_sends_nothing_until_its_user_says_yes() {
        // One that sent counts under the old default, on: it kept their date, and no file of yes.
        let sent = copy("sent");
        std::fs::write(sent.join(LAST), "2026-11-14").unwrap();
        assert!(
            !on(&sent),
            "a copy that sent counts by default went on sending them"
        );
        // One whose user turned it off then stays off, and a yes later takes that file away.
        let off = copy("off");
        std::fs::write(off.join(OLD_OFF), "").unwrap();
        assert!(!on(&off));
        answer(&off, true).unwrap();
        assert!(on(&off));
        assert!(
            !off.join(OLD_OFF).exists(),
            "a yes left the old default's off file"
        );
        _ = std::fs::remove_dir_all(&sent);
        _ = std::fs::remove_dir_all(&off);
    }

    #[test]
    fn a_new_copy_counts_as_first_this_week_and_this_month() {
        let today = day("2026-11-06");
        let flags = Flags {
            first: true,
            weekly: true,
            monthly: true,
        };
        assert_eq!(due(None, today), Some(flags));
    }

    #[test]
    fn only_the_copys_first_count_is_first() {
        let today = day("2026-11-06");
        // The first run, still open on the next day, or a copy that sent counts before the ask.
        let flags = due(Some(day("2026-11-05")), today).unwrap();
        assert!(!flags.first);
        // A first count that never reached the server, or that a yes after the first run sends,
        // is the first.
        assert!(due(None, today).unwrap().first);
    }

    #[test]
    fn a_copy_counts_once_a_day() {
        let today = day("2026-11-06");
        assert_eq!(due(Some(today), today), None);
        // A clock set back doesn't count a day again.
        assert_eq!(due(Some(day("2026-11-07")), today), None);
    }

    #[test]
    fn a_new_day_says_whether_the_week_and_month_are_new() {
        let flags = |last, today| due(Some(day(last)), day(today)).unwrap();
        // Friday after Thursday: the same week and month.
        let same = Flags {
            first: false,
            weekly: false,
            monthly: false,
        };
        assert_eq!(flags("2026-11-05", "2026-11-06"), same);
        // Monday after Sunday: a new ISO week, the same month.
        assert_eq!(
            flags("2026-11-08", "2026-11-09"),
            Flags {
                weekly: true,
                ..same
            }
        );
        // Sunday 1 November after Saturday 31 October: a new month, the same week.
        assert_eq!(
            flags("2026-10-31", "2026-11-01"),
            Flags {
                monthly: true,
                ..same
            }
        );
        // A year later, on the same ISO week number: both new.
        let both = Flags {
            weekly: true,
            monthly: true,
            ..same
        };
        assert_eq!(flags("2025-11-06", "2026-11-06"), both);
        // A missed day isn't sent later: the next count is today's alone.
        assert_eq!(flags("2026-11-02", "2026-11-06"), same);
    }

    #[test]
    fn the_kept_date_reads_back() {
        let today = day("2026-11-06");
        assert_eq!(parse(&date(today)), Some(today));
        assert_eq!(parse("2026-11-06\n"), Some(today));
        for text in ["", "new", "2026-13-01", "2026-11", "06-11-2026x"] {
            assert_eq!(parse(text), None, "{text}");
        }
    }

    #[test]
    fn only_a_build_or_a_check_names_the_endpoint() {
        let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        assert_eq!(endpoint(&args(&["chrome"]), None), None);
        assert_eq!(
            endpoint(
                &args(&["chrome"]),
                Some("https://sync.example.org/v1/daily\n")
            ),
            Some("https://sync.example.org/v1/daily".into())
        );
        let given = args(&["chrome", "--daily-count-url=http://127.0.0.1:9/v1/daily"]);
        assert_eq!(
            endpoint(&given, Some("https://sync.example.org/v1/daily")),
            Some("http://127.0.0.1:9/v1/daily".into())
        );
        assert_eq!(endpoint(&args(&["chrome"]), Some("")), None);
    }
}
