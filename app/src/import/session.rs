//! The tabs open in a Chromium-based browser's last session, from the newest `Sessions/Session_*`
//! file of a profile: SNSS, a log of commands Chromium replays at launch to put its windows back
//! (components/sessions/core, session_service_commands.cc and command_storage_backend.cc). This
//! replays the ones that say which tabs are open, in which window and where, and at which page.
//!
//! Chromium 156 writes the session twice, in the clear here and encrypted with the profile's key
//! in `Sessions_Encrypted`. The encrypted copy is never read, as import never decrypts, so a
//! browser that writes only that one has no open tabs to bring.

use std::collections::HashMap;
use std::path::Path;

use super::found::{Tab, opens_here};
use super::read::ReadError;

/// "SNSS", as the file's first four bytes.
const SIGNATURE: &[u8] = b"SNSS";
/// The version Chromium writes in the clear; 5 is its encrypted one.
const VERSION: i32 = 3;
/// What ends the state Chromium writes on opening the file, which a whole session file has.
const MARKER: u8 = 255;

const SET_TAB_WINDOW: u8 = 0;
const SET_TAB_INDEX_IN_WINDOW: u8 = 2;
const PRUNED_FROM_BACK: u8 = 5;
const UPDATE_TAB_NAVIGATION: u8 = 6;
const SET_SELECTED_NAVIGATION_INDEX: u8 = 7;
const SET_WINDOW_TYPE: u8 = 9;
const PRUNED_FROM_FRONT: u8 = 11;
const TAB_CLOSED: u8 = 16;
const WINDOW_CLOSED: u8 = 17;
const PRUNED: u8 = 24;

/// A browser window's type; the rest are popups, apps and DevTools.
const NORMAL_WINDOW: i32 = 0;

/// The tabs open in `profile`'s last session, window by window, each at the page it showed. Those
/// that can't open here stay out, and so does a session Chromium wouldn't restore.
pub fn open_tabs(profile: &Path) -> Result<Vec<Tab>, ReadError> {
    let dir = profile.join("Sessions");
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    // Each is named for when it was begun, in microseconds since 1601; the newest whole one is the
    // session, as Chromium picks it.
    let mut files: Vec<(u64, std::path::PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let time = name.to_str()?.strip_prefix("Session_")?.parse().ok()?;
            Some((time, entry.path()))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in files {
        if let Some(tabs) = parse(&std::fs::read(path)?) {
            return Ok(tabs);
        }
    }
    Ok(Vec::new())
}

#[derive(Default)]
struct SessionTab {
    window: i32,
    /// Its place in its window's tab strip.
    index: i32,
    /// Its pages, by their index in its history, in order.
    pages: Vec<(i32, Tab)>,
    /// The index of the page it shows.
    current: i32,
}

/// The open tabs a session file holds, or none if it isn't one Chromium would restore: another
/// version, such as the encrypted one, or a file left before its first state was written.
pub fn parse(bytes: &[u8]) -> Option<Vec<Tab>> {
    if bytes.get(..4)? != SIGNATURE || int(bytes, 4)? != VERSION {
        return None;
    }
    let mut tabs: HashMap<i32, SessionTab> = HashMap::new();
    // Each window's type, once it's been said.
    let mut windows: HashMap<i32, i32> = HashMap::new();
    let mut marked = false;
    let mut at = 8;
    // A command is its size, two bytes, then its id and what it says. A last one cut short by a
    // crash is left out, as Chromium leaves it.
    while let Some(size) = bytes.get(at..at + 2) {
        let size = u16::from_le_bytes([size[0], size[1]]) as usize;
        let Some(command) = bytes.get(at + 2..at + 2 + size).filter(|_| size > 0) else {
            break;
        };
        at += 2 + size;
        let (id, body) = (command[0], &command[1..]);
        // A command too short for what it says is passed over.
        let ints = |count: usize| (0..count).map(|n| int(body, n * 4)).collect::<Option<Vec<_>>>();
        match id {
            MARKER => marked = true,
            SET_TAB_WINDOW => {
                if let Some([window, id]) = ints(2).as_deref() {
                    tab(&mut tabs, *id).window = *window;
                }
            }
            SET_TAB_INDEX_IN_WINDOW => {
                if let Some([id, index]) = ints(2).as_deref() {
                    tab(&mut tabs, *id).index = *index;
                }
            }
            SET_SELECTED_NAVIGATION_INDEX => {
                if let Some([id, index]) = ints(2).as_deref() {
                    tab(&mut tabs, *id).current = *index;
                }
            }
            SET_WINDOW_TYPE => {
                if let Some([id, kind]) = ints(2).as_deref() {
                    windows.insert(*id, *kind);
                }
            }
            TAB_CLOSED => {
                if let Some([id]) = ints(1).as_deref() {
                    tabs.remove(id);
                }
            }
            WINDOW_CLOSED => {
                if let Some([id]) = ints(1).as_deref() {
                    windows.remove(id);
                }
            }
            PRUNED_FROM_BACK => {
                if let Some([id, index]) = ints(2).as_deref() {
                    tab(&mut tabs, *id).pages.retain(|(each, _)| each < index);
                }
            }
            PRUNED_FROM_FRONT => {
                if let Some([id, count]) = ints(2).as_deref()
                    && *count > 0
                {
                    prune(tab(&mut tabs, *id), 0, *count);
                }
            }
            PRUNED => {
                if let Some([id, index, count]) = ints(3).as_deref()
                    && *index >= 0
                    && *count > 0
                {
                    prune(tab(&mut tabs, *id), *index, *count);
                }
            }
            UPDATE_TAB_NAVIGATION => {
                if let Some((id, index, page)) = navigation(body) {
                    let pages = &mut tab(&mut tabs, id).pages;
                    match pages.iter().position(|(each, _)| *each >= index) {
                        Some(at) if pages[at].0 == index => pages[at].1 = page,
                        Some(at) => pages.insert(at, (index, page)),
                        None => pages.push((index, page)),
                    }
                }
            }
            _ => {}
        }
    }
    if !marked {
        return None;
    }
    // A tab of a window that closed, or of one that isn't a browser window, doesn't come back.
    let mut open: Vec<(i32, i32, i32, SessionTab)> = tabs
        .into_iter()
        .filter(|(_, tab)| windows.get(&tab.window) == Some(&NORMAL_WINDOW))
        .map(|(id, tab)| (tab.window, tab.index, id, tab))
        .collect();
    open.sort_by_key(|&(window, index, id, _)| (window, index, id));
    Some(
        open.into_iter()
            .filter_map(|(_, _, _, tab)| {
                // The page it shows, or its last if that one was pruned.
                let at = tab.pages.iter().position(|(index, _)| *index >= tab.current);
                let (_, page) = at.map_or(tab.pages.last(), |at| tab.pages.get(at))?;
                opens_here(&page.url).then(|| page.clone())
            })
            .collect(),
    )
}

fn tab(tabs: &mut HashMap<i32, SessionTab>, id: i32) -> &mut SessionTab {
    tabs.entry(id).or_default()
}

/// Takes `count` pages from `index` out of `tab`'s history, as Chromium's
/// ProcessTabNavigationPathPrunedCommand does.
fn prune(tab: &mut SessionTab, index: i32, count: i32) {
    let end = index.saturating_add(count);
    if (index..end).contains(&tab.current) {
        tab.current = index - 1;
    } else if tab.current >= end {
        tab.current -= count;
    }
    tab.pages.retain(|(each, _)| !(index..end).contains(each));
    for (each, _) in &mut tab.pages {
        if *each >= index {
            *each -= count;
        }
    }
}

/// An UpdateTabNavigation's tab, the page's index in its history, and the page: a base::Pickle of
/// the tab's id, then SerializedNavigationEntry::WriteToPickle's index and URL. Its title is the
/// page's, not a name the user gave, so it's left to the page.
fn navigation(body: &[u8]) -> Option<(i32, i32, Tab)> {
    let mut pickle = Pickle::new(body)?;
    let id = pickle.int()?;
    let index = pickle.int()?;
    let url = String::from_utf8_lossy(pickle.string()?).into_owned();
    Some((
        id,
        index,
        Tab {
            url,
            name: String::new(),
        },
    ))
}

/// A little-endian int at `at` in `bytes`.
fn int(bytes: &[u8], at: usize) -> Option<i32> {
    let bytes = bytes.get(at..at + 4)?;
    Some(i32::from_le_bytes(bytes.try_into().ok()?))
}

/// base::Pickle's reader: a header with the payload's size, then fields each padded to 4 bytes.
struct Pickle<'a> {
    payload: &'a [u8],
    at: usize,
}

impl<'a> Pickle<'a> {
    fn new(data: &'a [u8]) -> Option<Pickle<'a>> {
        let size = int(data, 0)?.try_into().ok()?;
        let payload = data.get(4..)?.get(..size)?;
        Some(Pickle { payload, at: 0 })
    }

    fn int(&mut self) -> Option<i32> {
        let value = int(self.payload, self.at)?;
        self.at += 4;
        Some(value)
    }

    /// A string's bytes: its length, then the bytes.
    fn string(&mut self) -> Option<&'a [u8]> {
        let size: usize = self.int()?.try_into().ok()?;
        let bytes = self.payload.get(self.at..self.at.checked_add(size)?)?;
        self.at += size.next_multiple_of(4);
        Some(bytes)
    }
}
