//! What a Chromium-based browser's profile holds: its bookmarks, from its `Bookmarks`, the tabs it
//! had open, from its last session (session.rs), its history, from its `History`, and its saved
//! addresses, from its `Web Data`, both SQLite files. Web Data holds cards too, encrypted, which
//! are never read. Arc's sidebar is arc.rs's.

use std::path::Path;
use std::time::SystemTime;

use serde_json::Value;

use super::extensions;
use super::found::{Address, Field, Node, Page, Tab, opens_here};
use super::read::{Copy, Kind, Read, ReadError, folder_name, micros, pinned};

/// Chromium's times count microseconds from 1601, 11,644,473,600 seconds before 1970.
const FROM_1601: i64 = 11_644_473_600_000_000;

/// What of `picked` the profile at `path` has, its history since `since`.
pub fn read(path: &Path, picked: &[Kind], since: SystemTime) -> Result<Read, ReadError> {
    let mut read = Read::default();
    let bookmarks = path.join("Bookmarks");
    if picked.contains(&Kind::Bookmarks) && bookmarks.is_file() {
        read.pinned = self::bookmarks(&std::fs::read_to_string(&bookmarks)?)?;
    }
    if picked.contains(&Kind::OpenTabs) {
        read.today = super::session::open_tabs(path)?;
    }
    let history = path.join("History");
    if picked.contains(&Kind::History) && history.is_file() {
        read.pages = self::history(&history, since)?;
    }
    let web_data = path.join("Web Data");
    if picked.contains(&Kind::Addresses) && web_data.is_file() {
        read.addresses = addresses(&web_data)?;
    }
    if picked.contains(&Kind::Extensions) {
        read.extensions = extensions::read(path);
    }
    Ok(read)
}

/// The addresses saved in the `Web Data` file at `path`, the user's own and their account's, with
/// each field as Chromium keeps it. Since Web Data's version 134 they're in one table, before it in
/// one for the user's own and one for their account's. It reads a copy, as history() does.
pub fn addresses(path: &Path) -> Result<Vec<Address>, ReadError> {
    let copy = Copy::of(path)?;
    let db = copy.open()?;
    let mut addresses: Vec<(String, Address)> = Vec::new();
    for table in [
        "address_type_tokens",
        "local_addresses_type_tokens",
        "contact_info_type_tokens",
    ] {
        let exists: bool = db.query_row(
            "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            continue;
        }
        let mut query = db.prepare(&format!(
            "SELECT guid, type, value, verification_status FROM {table} \
             WHERE value != '' ORDER BY guid, type"
        ))?;
        let rows = query.query_map([], |row| {
            let field = Field {
                kind: row.get(1)?,
                value: row.get(2)?,
                status: row.get::<_, Option<i32>>(3)?.unwrap_or_default(),
            };
            Ok((row.get::<_, String>(0)?, field))
        })?;
        for row in rows {
            let (guid, field) = row?;
            match addresses.last_mut() {
                Some((last, address)) if *last == guid => address.fields.push(field),
                _ => addresses.push((
                    guid,
                    Address {
                        fields: vec![field],
                    },
                )),
            }
        }
    }
    Ok(addresses.into_iter().map(|(_, address)| address).collect())
}

/// A profile's bookmarks as pinned tabs and folders: the bar's at the top, or in a folder of its
/// name once it holds more than BAR_ROWS, then the other bookmarks and the phone's, each in a
/// folder of its name. Pages that can't open here and folders left empty stay out.
pub fn bookmarks(json: &str) -> Result<Vec<Node>, String> {
    let root: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let roots = root.get("roots").ok_or("no bookmarks in the file")?;
    let folder = |key: &str, fallback: &str| {
        let folder = roots.get(key)?;
        let name = folder
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(fallback);
        Some((name.to_string(), nodes(folder)))
    };
    let folders = [("other", "Other Bookmarks"), ("synced", "Mobile Bookmarks")];
    Ok(pinned(
        folder("bookmark_bar", "Bookmarks Bar"),
        folders
            .iter()
            .filter_map(|(key, fallback)| folder(key, fallback))
            .collect(),
    ))
}

/// A bookmark folder's children that come in.
fn nodes(folder: &Value) -> Vec<Node> {
    let children = folder.get("children").and_then(Value::as_array);
    let text = |node: &Value, key| {
        node.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    children
        .into_iter()
        .flatten()
        .filter_map(|node| match node.get("type").and_then(Value::as_str)? {
            "url" => {
                let url = text(node, "url");
                opens_here(&url).then(|| {
                    Node::Tab(Tab {
                        url,
                        name: text(node, "name"),
                    })
                })
            }
            "folder" => {
                let children = nodes(node);
                let name = text(node, "name");
                (!children.is_empty()).then(|| Node::Folder {
                    name: folder_name(&name),
                    children,
                })
            }
            _ => None,
        })
        .collect()
}

/// The pages of the `History` file at `path` visited since `since`, the latest first, but for
/// those Chromium hides, such as a frame's, and those that can't open here. It reads a copy, as the
/// browser keeps the file locked while it runs.
pub fn history(path: &Path, since: SystemTime) -> Result<Vec<Page>, ReadError> {
    let copy = Copy::of(path)?;
    let db = copy.open()?;
    let mut query = db.prepare(
        "SELECT url, title, visit_count, typed_count, last_visit_time FROM urls \
         WHERE hidden = 0 AND last_visit_time > ?1 ORDER BY last_visit_time DESC",
    )?;
    let rows = query.query_map([micros(since) + FROM_1601], |row| {
        Ok(Page {
            url: row.get(0)?,
            title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            visits: row.get::<_, i64>(2)?.max(0) as u32,
            typed: row.get::<_, i64>(3)?.max(0) as u32,
            last_visit: row.get::<_, i64>(4)? - FROM_1601,
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
