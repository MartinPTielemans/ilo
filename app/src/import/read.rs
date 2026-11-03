//! Reading what the user picked from a browser, whichever its family: what can come from each,
//! the rules every family's bookmarks follow, and the copy of a database it reads in place of the
//! one the browser keeps open.
//!
//! A browser without Spaces gets one here for each of its profiles, named after the browser, whose
//! pinned tabs and folders are its bookmarks and whose Today tabs are the tabs it had open.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::extensions::Extension;
use super::found::{Address, Found, FromProfile, Node, Page, Space, Tab};
use super::sources::{Family, Profile, Source};
use super::{chromium, firefox, safari};

/// The most a bookmarks bar puts straight among the pinned tabs; a longer one comes in as a folder
/// of its name, as more would push the Space's Today tabs off the sidebar.
pub const BAR_ROWS: usize = 12;

/// How far back history comes: as far as Chromium keeps it, so all of it is kept here too.
pub const HISTORY_DAYS: u64 = 90;

/// What the user can bring from a browser, each shown as a switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Arc's Spaces, with their pinned tabs and folders, and its link rules.
    Spaces,
    /// Arc's favorites.
    Favorites,
    Bookmarks,
    /// The tabs a Chromium-based browser had open in its last session.
    OpenTabs,
    /// Safari's Reading List, as a folder beside its bookmarks.
    ReadingList,
    History,
    /// A Chromium-based browser's saved addresses, never its cards.
    Addresses,
    /// A Chromium browser's Web Store extensions, listed to add from the store, never installed.
    Extensions,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Spaces => "Spaces, with their pinned tabs, folders and link rules",
            Kind::Favorites => "Favorites",
            Kind::Bookmarks => "Bookmarks, as a Space's pinned tabs",
            Kind::OpenTabs => "Open tabs, as that Space's Today tabs",
            Kind::Addresses => "Addresses",
            Kind::ReadingList => "Reading List, as a folder in that Space",
            Kind::History => "History",
            Kind::Extensions => "Extensions, to add from the Chrome Web Store",
        }
    }
}

/// Why a browser's files couldn't be read.
#[derive(Clone, Debug, PartialEq)]
pub enum ReadError {
    /// The system keeps them from this app until the user gives it Full Disk Access, as macOS
    /// does Safari's.
    NoAccess,
    Failed(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::NoAccess => f.write_str("the system doesn't let this app read them"),
            ReadError::Failed(error) => f.write_str(error),
        }
    }
}

impl From<std::io::Error> for ReadError {
    fn from(error: std::io::Error) -> ReadError {
        match error.kind() {
            std::io::ErrorKind::PermissionDenied => ReadError::NoAccess,
            _ => ReadError::Failed(error.to_string()),
        }
    }
}

impl From<rusqlite::Error> for ReadError {
    fn from(error: rusqlite::Error) -> ReadError {
        ReadError::Failed(error.to_string())
    }
}

impl From<String> for ReadError {
    fn from(error: String) -> ReadError {
        ReadError::Failed(error)
    }
}

/// What can come from `source`: Arc's sidebar in place of its bookmarks, which it doesn't show,
/// and the extensions of Chromium's family, which Firefox's and Safari's aren't.
pub fn kinds(source: &Source) -> &'static [Kind] {
    if source.arc_sidebar().is_some() {
        &[Kind::Spaces, Kind::Favorites, Kind::History, Kind::Addresses, Kind::Extensions]
    } else {
        match source.browser.family {
            Family::Safari => &[Kind::Bookmarks, Kind::ReadingList, Kind::History],
            Family::Firefox => &[Kind::Bookmarks, Kind::History],
            Family::Chromium => &[
                Kind::Bookmarks,
                Kind::OpenTabs,
                Kind::History,
                Kind::Addresses,
                Kind::Extensions,
            ],
        }
    }
}

/// Whether `source` has anything of `kinds(source)` to bring.
pub fn has_any(source: &Source) -> bool {
    let files: &[&str] = match source.browser.family {
        Family::Chromium => &["Bookmarks", "History"],
        Family::Firefox => &["places.sqlite"],
        // macOS hides what's in Safari's folder until the user gives Full Disk Access, which the
        // import asks for once it's picked.
        Family::Safari => return true,
    };
    source.arc_sidebar().is_some()
        || (source.profiles.iter())
            .any(|profile| files.iter().any(|file| profile.path.join(file).is_file()))
}

/// Reads what of `picked` `source` has, for the import to bring in. History goes back
/// HISTORY_DAYS from `now`. Slow for a long history, so it runs away from the UI.
pub fn read(source: &Source, picked: &[Kind], now: SystemTime) -> Result<Found, ReadError> {
    let mut found = Found::default();
    let wants = |kind| picked.contains(&kind);
    if let Some(path) = source.arc_sidebar()
        && (wants(Kind::Spaces) || wants(Kind::Favorites))
    {
        // Arc's link rules come with its Spaces, each to the Space its Arc Space becomes.
        let routing = (source.arc_link_routing())
            .filter(|_| wants(Kind::Spaces))
            .and_then(|routing| std::fs::read_to_string(routing).ok());
        let sidebar = super::arc::parse(&std::fs::read_to_string(&path)?, routing.as_deref())?;
        if wants(Kind::Spaces) {
            found.spaces = sidebar.spaces;
            found.link_rules = sidebar.link_rules;
        }
        if wants(Kind::Favorites) {
            found.favorites = sidebar.favorites;
        }
    }
    let name = source.browser.name;
    let since = now
        .checked_sub(Duration::from_secs(HISTORY_DAYS * 24 * 60 * 60))
        .unwrap_or(UNIX_EPOCH);
    for profile in &source.profiles {
        let read = match source.browser.family {
            Family::Chromium => chromium::read(&profile.path, picked, since)?,
            Family::Firefox => firefox::read(&profile.path, picked, since)?,
            Family::Safari => safari::read(&profile.path, picked, since)?,
        };
        let from = from(profile);
        if !read.pinned.is_empty() || !read.today.is_empty() {
            found.spaces.push(Space {
                // One Space for the browser, or one for each of its profiles, by its name.
                name: if source.profiles.len() == 1 {
                    name.into()
                } else {
                    format!("{name} – {}", profile.name)
                },
                profile: from.clone(),
                icon: String::new(),
                theme: None,
                pinned: read.pinned,
                today: read.today,
            });
        }
        if !read.pages.is_empty() {
            found.history.push((from.clone(), read.pages));
        }
        if !read.addresses.is_empty() {
            found.addresses.push((from, read.addresses));
        }
        for extension in read.extensions {
            match found.extensions.iter_mut().find(|each| each.id == extension.id) {
                Some(each) => each.pinned |= extension.pinned,
                None => found.extensions.push(extension),
            }
        }
    }
    Ok(found)
}

/// What the bookmarks bar is called in `family`'s browsers, for its bookmarks that come in loose
/// among the pinned tabs, not in a folder.
pub fn bar_name(family: Family) -> &'static str {
    match family {
        Family::Chromium => "Bookmarks Bar",
        Family::Firefox => "Bookmarks Toolbar",
        Family::Safari => "Favorites Bar",
    }
}

/// The folders `source`'s bookmarks come in as, by name, once each however many profiles have
/// one, to choose from: the bar's own bookmarks by its name (bar_name), each of the bar's folders,
/// or the bar's folder past BAR_ROWS, and the other top folders, as they come in.
pub fn folders(source: &Source, now: SystemTime) -> Result<Vec<String>, ReadError> {
    let found = read(source, &[Kind::Bookmarks], now)?;
    let bar = bar_name(source.browser.family);
    let mut names: Vec<String> = Vec::new();
    for node in found.spaces.iter().flat_map(|space| &space.pinned) {
        let name = match node {
            Node::Tab(_) => bar,
            Node::Folder { name, .. } => name,
        };
        if !names.iter().any(|each| each == name) {
            names.push(name.into());
        }
    }
    Ok(names)
}

/// Leaves out of what `found` brings the folders named among `left_out`, and the bar's own
/// bookmarks if its name (bar_name of `family`) is; a Space left with nothing doesn't come.
pub fn leave_out(found: &mut Found, left_out: &[String], family: Family) {
    if left_out.is_empty() {
        return;
    }
    let out = |name: &str| left_out.iter().any(|each| each == name);
    let bar = out(bar_name(family));
    for space in &mut found.spaces {
        space.pinned.retain(|node| match node {
            Node::Tab(_) => !bar,
            Node::Folder { name, .. } => !out(name),
        });
    }
    found.spaces.retain(|space| !space.pinned.is_empty());
}

/// What a family's reader found in one profile.
#[derive(Default)]
pub struct Read {
    pub pinned: Vec<Node>,
    pub today: Vec<Tab>,
    pub pages: Vec<Page>,
    pub addresses: Vec<Address>,
    pub extensions: Vec<Extension>,
}

/// The profile here that `profile`'s things go to: the one this browser started with for the
/// browser's own default, else one of its name.
fn from(profile: &Profile) -> FromProfile {
    if profile.default {
        FromProfile::Default
    } else {
        FromProfile::Custom(profile.dir.clone())
    }
}

/// A profile's bookmarks as pinned tabs and folders: the bar's at the top, or in a folder of its
/// name once it holds more than BAR_ROWS, then each of `folders` in a folder of its name. Folders
/// left empty stay out.
pub fn pinned(bar: Option<(String, Vec<Node>)>, folders: Vec<(String, Vec<Node>)>) -> Vec<Node> {
    let mut pinned = Vec::new();
    if let Some((name, children)) = bar.filter(|(_, children)| !children.is_empty()) {
        if children.len() <= BAR_ROWS {
            pinned = children;
        } else {
            pinned.push(Node::Folder { name, children });
        }
    }
    for (name, children) in folders {
        if !children.is_empty() {
            pinned.push(Node::Folder { name, children });
        }
    }
    pinned
}

/// A folder's name as it comes in: an unnamed one is "Folder".
pub fn folder_name(name: &str) -> String {
    match name.trim() {
        "" => "Folder".into(),
        name => name.into(),
    }
}

/// A copy of the database at `path`, with what its write-ahead log holds that the browser hasn't
/// written into it yet, in a folder of its own that goes when it does. The browser keeps the file
/// open, and locked, while it runs.
pub struct Copy {
    dir: PathBuf,
    pub path: PathBuf,
}

impl Copy {
    pub fn of(path: &Path) -> Result<Copy, ReadError> {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ilo-import-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir)?;
        let copy = Copy {
            path: dir.join(path.file_name().unwrap_or_default()),
            dir,
        };
        std::fs::copy(path, &copy.path)?;
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        let wal = PathBuf::from(wal);
        if wal.is_file() {
            let mut to = copy.path.as_os_str().to_owned();
            to.push("-wal");
            std::fs::copy(&wal, to)?;
        }
        Ok(copy)
    }

    /// Opens the copy, which is the import's own, so SQLite can fold its log into it.
    pub fn open(&self) -> Result<rusqlite::Connection, ReadError> {
        Ok(rusqlite::Connection::open(&self.path)?)
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// Microseconds since 1970 of `time`.
pub fn micros(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_micros() as i64)
}
