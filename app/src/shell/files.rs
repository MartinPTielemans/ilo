//! Files and folders from the desktop in the sidebar. A file is a tab, as a dropped one always
//! was; a folder becomes one of the sidebar's folders, named for it, holding a tab for each of its
//! files and a folder for each of its folders, so it expands in place as any folder does and is
//! saved with the Space. A file's row dragged onto the page hands the file to the system's drag,
//! to upload it there, as a download's row does. The +'s Add File or Folder… adds either.
//!
//! Agents get none of this: they open no pages outside http(s) (principle 3).

use std::path::{Path, PathBuf};

use gpui::{AppContext, Context, PathPromptOptions};

use super::{Shell, file_url};

/// The most files and folders a folder from the desktop brings in, its folders' included, taken
/// a level at a time, so a home folder dropped by mistake doesn't make thousands of rows (ours).
pub(super) const FOLDER_ROWS: usize = 100;
/// How many folders deep it's brought in, in case a link loops (ours).
const FOLDER_DEPTH: usize = 8;
/// Whether the system's open panel picks files and folders together, as the Mac's does; the
/// others' pick one or the other.
pub(super) const PANEL_PICKS_BOTH: bool = cfg!(target_os = "macos");

/// A folder on the desktop as it comes in: its name, then its folders and its files' URLs, each
/// by name.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Listing {
    pub name: String,
    pub folders: Vec<Listing>,
    pub files: Vec<String>,
}

/// The path a `file:` URL names, if it names one.
pub(super) fn file_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut path = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| bytes.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                path.push(byte);
                at += 3;
            }
            None => {
                path.push(bytes[at]);
                at += 1;
            }
        }
    }
    let path = String::from_utf8(path).ok()?;
    // Windows' "/C:/Users", a drive after the URL's slash, with the path's own backslashes.
    if cfg!(windows) && path.as_bytes().get(2) == Some(&b':') {
        return Some(PathBuf::from(path[1..].replace('/', "\\")));
    }
    Some(PathBuf::from(path))
}

/// The folder a dropped `url` names on the desktop, if it names one.
pub(super) fn folder_path(url: &str) -> Option<PathBuf> {
    file_path(url).filter(|path| path.is_dir())
}

/// The folder at `path` as it comes in, by name, hidden files and folders left out, each taken
/// costing one of `rows`: this folder's first, then its folders' in turn.
pub(super) fn list(path: &Path, rows: &mut usize, depth: usize) -> Listing {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned());
    let name = name.unwrap_or_else(|| path.display().to_string());
    let mut listing = Listing { name, ..Listing::default() };
    let Ok(entries) = std::fs::read_dir(path) else {
        return listing;
    };
    let mut entries: Vec<(String, PathBuf, bool)> = (entries.flatten())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            // Through a link, to what it names.
            let meta = std::fs::metadata(&path).ok()?;
            let kept = !name.starts_with('.') && (meta.is_dir() || meta.is_file());
            kept.then_some((name, path, meta.is_dir()))
        })
        .collect();
    entries.sort_by_key(|(name, ..)| name.to_lowercase());
    let taken = entries.len().min(*rows);
    *rows -= taken;
    let (folders, files): (Vec<_>, Vec<_>) = entries.into_iter().take(taken).partition(|e| e.2);
    listing.files = files.iter().map(|(_, path, _)| file_url(&path.to_string_lossy())).collect();
    listing.folders = (folders.into_iter())
        .map(|(name, path, _)| match depth + 1 < FOLDER_DEPTH {
            true => list(&path, rows, depth + 1),
            false => Listing { name, ..Listing::default() },
        })
        .collect();
    listing
}

impl Shell {
    /// Folders from the desktop, dropped or added: each becomes an open folder of the Space shown,
    /// at `index` in `parent`, else at the bottom of the pinned tabs, as `make_folder` makes it.
    /// They're read away from the main thread, as a folder can be slow to read.
    pub(super) fn add_folders(
        &mut self,
        paths: Vec<PathBuf>,
        parent: u64,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        let space = self.space;
        let listed = cx.background_spawn(async move {
            let list = |path: &PathBuf| list(path, &mut FOLDER_ROWS.clone(), 0);
            paths.iter().map(list).collect::<Vec<_>>()
        });
        cx.spawn(async move |shell, cx| {
            let listings = listed.await;
            shell
                .update(cx, |shell, cx| {
                    // Another Space shown meanwhile takes them at the bottom of its pinned tabs.
                    let (parent, index) = match shell.space == space {
                        true => (parent, index),
                        false => (0, usize::MAX),
                    };
                    for (at, listing) in listings.iter().enumerate() {
                        shell.make_folder(listing, parent, index.saturating_add(at), true, cx);
                    }
                })
                .ok();
        })
        .detach();
    }

    /// Makes `listing` a folder at `index` in `parent`, with its folders, closed, then a tab for
    /// each of its files, each loading once it's shown, and returns it.
    fn make_folder(
        &mut self,
        listing: &Listing,
        parent: u64,
        index: usize,
        open: bool,
        cx: &mut Context<Self>,
    ) -> u64 {
        let folder = self.browser.create_folder(self.space, &listing.name, parent, index, cx);
        if folder == 0 {
            return 0;
        }
        let (folders, files) = (listing.folders.len(), listing.files.len());
        // check.sh waits for this line.
        log!("browser: folder {folder} from disk {folders} {files} {}", listing.name);
        for (at, inner) in listing.folders.iter().enumerate() {
            self.make_folder(inner, folder, at, false, cx);
        }
        self.browser.open_behind(&listing.files, true, folder, folders, cx);
        if !open {
            self.browser.set_folder_open(folder, false, cx);
        }
        folder
    }

    /// The +'s Add File or Folder…: what's picked in the system's panel comes in, `files` and
    /// `folders` saying what it offers, since off the Mac it picks one or the other.
    pub(super) fn add_from_disk(&mut self, files: bool, folders: bool, cx: &mut Context<Self>) {
        // A check run names its made-up files, as it can't drive the system's panel.
        if let Some(paths) = std::env::var_os("BROWSER_ADD_PATHS") {
            return self.add_paths(std::env::split_paths(&paths).collect(), cx);
        }
        let asked = cx.prompt_for_paths(PathPromptOptions {
            files,
            directories: folders,
            multiple: true,
            prompt: Some("Add".into()),
        });
        cx.spawn(async move |shell, cx| {
            let paths = asked.await.ok().and_then(Result::ok).flatten().unwrap_or_default();
            shell.update(cx, |shell, cx| shell.add_paths(paths, cx)).ok();
        })
        .detach();
    }

    /// Picked files open as tabs at the top of Today, the first shown, as a new tab does; picked
    /// folders come in at the bottom of the pinned tabs.
    pub(super) fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let (folders, files): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|path| path.is_dir());
        let url = |path: &PathBuf| file_url(&path.to_string_lossy());
        let urls: Vec<String> = files.iter().map(url).collect();
        // check.sh waits for this line.
        log!("browser: added {} files {} folders", urls.len(), folders.len());
        if !urls.is_empty() {
            self.browser.open_at(&urls, false, 0, 0, cx);
        }
        self.add_folders(folders, 0, usize::MAX, cx);
    }

    /// The file on the desktop `tab` shows, if it's a file's tab alone in its row.
    pub(super) fn file_of_row(&self, tab: u64) -> Option<String> {
        if self.is_folder(tab) || self.panes(tab).len() != 1 {
            return None;
        }
        let path = file_path(&self.page(tab).url)?;
        path.is_file().then(|| path.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
#[path = "files/tests.rs"]
mod tests;
