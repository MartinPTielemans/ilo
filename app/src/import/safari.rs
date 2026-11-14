//! What Safari holds, in `~/Library/Safari`: its bookmarks and Reading List, in `Bookmarks.plist`,
//! and its history, in `History.db`, a SQLite file. macOS keeps the folder from other apps until
//! the user gives them Full Disk Access, so reading it may say ReadError::NoAccess.

use std::path::Path;
use web_time::SystemTime;

use plist::{Dictionary, Value};

use super::found::{Node, Page, Tab, opens_here};
#[cfg(not(target_family = "wasm"))]
use super::read::{Copy, micros};
use super::read::{Kind, Read, ReadError, folder_name, pinned};

/// Safari's times count seconds from 2001, 978,307,200 seconds after 1970.
#[cfg(not(target_family = "wasm"))]
const FROM_2001: f64 = 978_307_200.;

/// The names Safari's own folders have in the file, which it shows by others.
const BAR: &str = "BookmarksBar";
const MENU: &str = "BookmarksMenu";
const READING_LIST: &str = "com.apple.ReadingList";

/// What of `picked` Safari's folder at `path` has, its history since `since`.
pub fn read(path: &Path, picked: &[Kind], since: SystemTime) -> Result<Read, ReadError> {
    let wants = |kind| picked.contains(&kind);
    // A file that isn't there is nothing to bring; one the system won't show is an error, which
    // asks for Full Disk Access.
    let there = |file: &Path| !matches!(file.try_exists(), Ok(false));
    let mut read = Read::default();
    let file = path.join("Bookmarks.plist");
    if (wants(Kind::Bookmarks) || wants(Kind::ReadingList)) && there(&file) {
        let file = std::fs::read(file)?;
        read.pinned = bookmarks(&file, wants(Kind::Bookmarks), wants(Kind::ReadingList))?;
    }
    let file = path.join("History.db");
    if wants(Kind::History) && there(&file) {
        read.pages = history(&file, since)?;
    }
    Ok(read)
}

/// The bookmarks in `file`, a `Bookmarks.plist`, binary or XML, as pinned tabs and folders by
/// read.rs's rules: the bar as the bar, then the menu and any folder of the user's beside them,
/// then the Reading List, each in a folder. With `bookmarks` or `reading_list` false, that part
/// stays out.
pub fn bookmarks(file: &[u8], bookmarks: bool, reading_list: bool) -> Result<Vec<Node>, String> {
    let root = Value::from_reader(std::io::Cursor::new(file)).map_err(|error| error.to_string())?;
    let root = root.as_dictionary().ok_or("no bookmarks in the file")?;
    let mut bar = None;
    let mut folders = Vec::new();
    let mut list = None;
    // Bookmarks kept at the top, outside any folder, which Safari shows in its menu.
    let mut loose = Vec::new();
    for child in children(root) {
        let title = text(child, "Title");
        match (title.as_str(), kind(child)) {
            (BAR, Some(List)) if bookmarks => {
                bar = Some(("Favorites Bar".to_string(), nodes(child, 0)))
            }
            (MENU, Some(List)) if bookmarks => loose.extend(nodes(child, 0)),
            (READING_LIST, Some(List)) if reading_list => {
                list = Some(("Reading List".to_string(), nodes(child, 0)))
            }
            (BAR | MENU | READING_LIST, _) => {}
            (_, Some(List)) if bookmarks => folders.push((folder_name(&title), nodes(child, 0))),
            (_, Some(Leaf)) if bookmarks => loose.extend(tab(child).map(Node::Tab)),
            _ => {}
        }
    }
    folders.insert(0, ("Bookmarks Menu".to_string(), loose));
    folders.extend(list);
    Ok(pinned(bar, folders))
}

#[derive(Clone, Copy, PartialEq)]
enum Type {
    List,
    Leaf,
}
use Type::*;

/// A node's type; the history's proxy and anything newer have none here.
fn kind(node: &Dictionary) -> Option<Type> {
    match node.get("WebBookmarkType")?.as_string()? {
        "WebBookmarkTypeList" => Some(List),
        "WebBookmarkTypeLeaf" => Some(Leaf),
        _ => None,
    }
}

fn children(node: &Dictionary) -> impl Iterator<Item = &Dictionary> {
    let children = node.get("Children").and_then(Value::as_array);
    children
        .into_iter()
        .flatten()
        .filter_map(Value::as_dictionary)
}

fn text(node: &Dictionary, key: &str) -> String {
    let text = node.get(key).and_then(Value::as_string);
    text.unwrap_or_default().trim().into()
}

/// How deep folders go before the rest is left out, as a broken file could nest without end.
const DEEPEST: usize = 64;

fn nodes(folder: &Dictionary, depth: usize) -> Vec<Node> {
    if depth > DEEPEST {
        return Vec::new();
    }
    children(folder)
        .filter_map(|child| match kind(child)? {
            Leaf => tab(child).map(Node::Tab),
            List => {
                let children = nodes(child, depth + 1);
                (!children.is_empty()).then(|| Node::Folder {
                    name: folder_name(&text(child, "Title")),
                    children,
                })
            }
        })
        .collect()
}

/// A bookmark whose page can open here, by the title the user gave it.
fn tab(leaf: &Dictionary) -> Option<Tab> {
    let url = text(leaf, "URLString");
    let title = leaf
        .get("URIDictionary")
        .and_then(Value::as_dictionary)
        .map(|uri| text(uri, "title"))
        .unwrap_or_default();
    opens_here(&url).then_some(Tab { url, name: title })
}

#[cfg(not(target_family = "wasm"))]
/// The pages of `History.db` at `path` visited since `since`, the latest first, each by the title
/// of its last visit. It reads a copy, as Safari keeps the file open while it runs.
pub fn history(path: &Path, since: SystemTime) -> Result<Vec<Page>, ReadError> {
    let copy = Copy::of(path)?;
    let db = copy.open()?;
    // SQLite gives the other columns of the row MAX picks: the last visit's title.
    let mut query = db.prepare(
        "SELECT i.url, v.title, i.visit_count, MAX(v.visit_time) AS last FROM history_items i \
         JOIN history_visits v ON v.history_item = i.id WHERE v.visit_time > ?1 \
         GROUP BY i.id ORDER BY last DESC",
    )?;
    let since = micros(since) as f64 / 1e6 - FROM_2001;
    let rows = query.query_map([since], |row| {
        Ok(Page {
            url: row.get(0)?,
            title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            visits: row.get::<_, i64>(2)?.max(1) as u32,
            // Safari keeps no count of typed visits.
            typed: 0,
            last_visit: ((row.get::<_, f64>(3)? + FROM_2001) * 1e6) as i64,
        })
    })?;
    let mut pages = Vec::new();
    for page in rows {
        let page = page?;
        if opens_here(&page.url) {
            pages.push(page);
        }
    }
    Ok(pages)
}

#[cfg(target_family = "wasm")]
pub fn history(_: &Path, _: SystemTime) -> Result<Vec<Page>, ReadError> {
    Err(super::read::on_the_web())
}
