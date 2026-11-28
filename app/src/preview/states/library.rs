//! The Library's made-up archive, history and downloads.

use crate::*;

// The archive: tabs closed hours and days ago, newest first.
pub(crate) const ARCHIVE: [(&str, &str, f64); 6] = [
    ("Wikipedia", "https://www.wikipedia.org/", 0.05),
    (
        "Internet Assigned Numbers Authority",
        "https://www.iana.org/",
        0.2,
    ),
    ("W3C", "https://www.w3.org/", 1.1),
    (
        "The GNU Operating System and the Free Software Movement",
        "https://www.gnu.org/",
        2.5,
    ),
    ("Chromium", "https://www.chromium.org/", 6.),
    ("", "https://example.org/", 12.),
];

pub(crate) const DAY: f64 = 86_400_000.;

// The profile's visited pages, History lists beside the closed tabs: how many days ago each was
// last visited, newest first. Wikipedia is a closed tab too, so it shows once, as the closed tab,
// and Page 0 the first made-up agent task's page, so it shows as its tab's.
pub(crate) const VISITED: [(&str, &str, f64); 13] = [
    (
        "Rust Programming Language",
        "https://www.rust-lang.org/",
        0.02,
    ),
    ("Page 0", "https://example.com/0", 0.023),
    ("Wikipedia", "https://www.wikipedia.org/", 0.06),
    ("Hacker News", "https://news.ycombinator.com/", 0.08),
    ("MDN Web Docs", "https://developer.mozilla.org/en-US/", 0.15),
    ("OpenStreetMap", "https://www.openstreetmap.org/", 0.3),
    (
        "HTTP - Wikipedia",
        "https://en.wikipedia.org/wiki/HTTP",
        0.9,
    ),
    (
        "The Rust Programming Language",
        "https://doc.rust-lang.org/book/",
        1.05,
    ),
    (
        "Internet for people, not profit — Mozilla",
        "https://www.mozilla.org/",
        1.3,
    ),
    (
        "Internet Archive: Digital Library",
        "https://archive.org/",
        2.2,
    ),
    (
        "World Wide Web - Wikipedia",
        "https://en.wikipedia.org/wiki/World_Wide_Web",
        2.4,
    ),
    ("BBC Weather", "https://www.bbc.com/weather", 3.),
    ("Welcome to Python.org", "https://www.python.org/", 7.),
];

// The downloads: one running, one paused, one held, and finished ones, newest first, with their
// bytes so far and in all, and how many days ago they started.
pub(crate) const DOWNLOADS: [(&str, &str, i64, i64, DownloadState, bool, f64); 6] = [
    (
        "zed-0.170.dmg",
        "https://zed.dev/",
        48_200_000,
        121_500_000,
        RUNNING,
        false,
        0.,
    ),
    (
        "rust-book.pdf",
        "https://doc.rust-lang.org/",
        3_100_000,
        7_800_000,
        PAUSED,
        false,
        0.01,
    ),
    (
        "installer.command",
        "http://example.com/",
        2_000,
        4_000,
        RUNNING,
        true,
        0.02,
    ),
    (
        "report.csv",
        "https://data.example.org/",
        18_400,
        18_400,
        DONE,
        false,
        0.3,
    ),
    (
        "photo.jpg",
        "https://commons.wikimedia.org/",
        2_480_000,
        2_480_000,
        DONE,
        false,
        1.2,
    ),
    (
        "old-notes.txt",
        "https://example.net/",
        0,
        0,
        DownloadState::Cancelled,
        false,
        9.,
    ),
];

pub(crate) const RUNNING: DownloadState = DownloadState::Running;

pub(crate) const PAUSED: DownloadState = DownloadState::Paused;

pub(crate) const DONE: DownloadState = DownloadState::Done;
