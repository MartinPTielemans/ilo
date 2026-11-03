//! What a profile of Firefox, or of a browser made from it such as Zen or LibreWolf, holds: its
//! bookmarks and history, both in its `places.sqlite`.

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use super::found::{Node, Page, Tab, opens_here};
use super::read::{Copy, Kind, Read, ReadError, folder_name, micros, pinned};

/// Firefox's own folders, by the ids every profile gives them, in the order they come in, with
/// the names they're shown by. The toolbar comes first, as Chromium's bar does; tags aren't
/// bookmarks.
const TOOLBAR: &str = "toolbar_____";
const FOLDERS: &[(&str, &str)] = &[
    ("menu________", "Bookmarks Menu"),
    ("unfiled_____", "Other Bookmarks"),
    ("mobile______", "Mobile Bookmarks"),
];

/// What of `picked` the profile at `path` has, its history since `since`. It reads a copy, as
/// Firefox keeps the file locked while it runs.
pub fn read(path: &Path, picked: &[Kind], since: SystemTime) -> Result<Read, ReadError> {
    let places = path.join("places.sqlite");
    let wants = |kind| picked.contains(&kind);
    if !places.is_file() || !(wants(Kind::Bookmarks) || wants(Kind::History)) {
        return Ok(Read::default());
    }
    let copy = Copy::of(&places)?;
    let db = copy.open()?;
    Ok(Read {
        pinned: if wants(Kind::Bookmarks) {
            bookmarks(&db)?
        } else {
            Vec::new()
        },
        pages: if wants(Kind::History) {
            history(&db, since)?
        } else {
            Vec::new()
        },
        ..Read::default()
    })
}

/// A bookmark or folder, by the folder it's in.
struct Entry {
    id: i64,
    folder: bool,
    title: String,
    url: Option<String>,
}

/// The profile's bookmarks as pinned tabs and folders, by read.rs's rules: the toolbar as the
/// bar, then the menu, the other bookmarks and the phone's, each in a folder.
pub fn bookmarks(db: &rusqlite::Connection) -> Result<Vec<Node>, ReadError> {
    // Separators (type 3) and anything newer have no place here.
    let mut query = db.prepare(
        "SELECT b.id, b.type, b.parent, b.title, p.url, b.guid FROM moz_bookmarks b \
         LEFT JOIN moz_places p ON p.id = b.fk WHERE b.type IN (1, 2) \
         ORDER BY b.parent, b.position",
    )?;
    let mut children: HashMap<i64, Vec<Entry>> = HashMap::new();
    let mut roots: HashMap<String, i64> = HashMap::new();
    let rows = query.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    })?;
    for row in rows {
        let (id, kind, parent, title, url, guid) = row?;
        if let Some(guid) = guid {
            roots.insert(guid, id);
        }
        children
            .entry(parent.unwrap_or(0))
            .or_default()
            .push(Entry {
                id,
                folder: kind == 2,
                title: title.unwrap_or_default().trim().into(),
                url,
            });
    }
    let nodes = |guid: &str| {
        let id = roots.get(guid).copied();
        id.map(|id| nodes(id, &children, 0)).unwrap_or_default()
    };
    let bar = Some(("Bookmarks Toolbar".to_string(), nodes(TOOLBAR)));
    let folders = FOLDERS
        .iter()
        .map(|(guid, name)| (name.to_string(), nodes(guid)))
        .collect();
    Ok(pinned(bar, folders))
}

/// How deep folders go before the rest is left out, as a file whose folders hold themselves would
/// otherwise never end.
const DEEPEST: usize = 64;

fn nodes(folder: i64, children: &HashMap<i64, Vec<Entry>>, depth: usize) -> Vec<Node> {
    if depth > DEEPEST {
        return Vec::new();
    }
    let entries = children.get(&folder).into_iter().flatten();
    entries
        .filter_map(|entry| {
            if entry.folder {
                let children = nodes(entry.id, children, depth + 1);
                return (!children.is_empty()).then(|| Node::Folder {
                    name: folder_name(&entry.title),
                    children,
                });
            }
            let url = entry.url.as_deref()?.trim();
            opens_here(url).then(|| {
                Node::Tab(Tab {
                    url: url.into(),
                    name: entry.title.clone(),
                })
            })
        })
        .collect()
}

/// The pages visited since `since`, the latest first, but for those Firefox hides, such as a
/// frame's, and those that can't open here.
pub fn history(db: &rusqlite::Connection, since: SystemTime) -> Result<Vec<Page>, ReadError> {
    let mut query = db.prepare(
        "SELECT url, title, visit_count, typed, last_visit_date FROM moz_places \
         WHERE hidden = 0 AND visit_count > 0 AND last_visit_date > ?1 \
         ORDER BY last_visit_date DESC",
    )?;
    let rows = query.query_map([micros(since)], |row| {
        Ok(Page {
            url: row.get(0)?,
            title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            visits: row.get::<_, i64>(2)?.max(0) as u32,
            // Firefox keeps only whether the page was ever typed.
            typed: row.get::<_, Option<i64>>(3)?.unwrap_or(0).clamp(0, 1) as u32,
            last_visit: row.get(4)?,
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
