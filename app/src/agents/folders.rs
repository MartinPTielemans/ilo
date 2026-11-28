//! Settings › Agents › Folders (#4092): the folders the user lets every ⌘T thread view (Can view)
//! or edit (Can edit), beside each thread's own folder, which leads Can edit and can't be removed.
//! A folder comes only from the user's pick in the system's panel, through the one road a folder
//! is granted by (`files::grant`), so no agent tool adds one. Each is checked again by its real
//! path as the settings are read, and the path floor checks every access by the path it resolves
//! to (files/resolve.rs, files/give.rs), so a folder listed here never opens what `Never`
//! refuses. Each add and remove is an entry in the log under `AGENT`, by the folder's name and
//! never its path (#3656). Kept with the agents' settings, never synced.

use std::path::Path;
use std::time::SystemTime;

use gpui::{Context, SharedString};
use serde_json::{Value, json};

use super::files::{self, Listed, Never, Row};
use super::wall::Granted;
use super::{Activity, Agents, LOG_LENGTH, entry_id};

/// The log's agent for the user's changes to the folders, which Settings › Agents lists as it
/// does scripts' entries.
pub const AGENT: &str = "folders";
/// The entries' actions; each one's target is the row's name and the folder's.
pub const ADDED: &str = "folder_added";
pub const REMOVED: &str = "folder_removed";
/// The rows, as the spec names them.
pub const CAN_VIEW: &str = "Can view";
pub const CAN_EDIT: &str = "Can edit";

/// A row's entry, as Settings lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Folder {
    /// Each thread's own folder, always there: a thread writes in it at every level.
    Thread,
    /// One the user added, by its name.
    Added(SharedString),
}

/// The folders the user added to each row, in the order added. One folder is in one row at most.
#[derive(Clone, Default)]
pub struct Folders {
    view: Vec<Granted>,
    edit: Vec<Granted>,
}

impl Folders {
    fn row(&self, row: Row) -> &Vec<Granted> {
        match row {
            Row::Read => &self.view,
            Row::Change => &self.edit,
        }
    }

    fn row_mut(&mut self, row: Row) -> &mut Vec<Granted> {
        match row {
            Row::Read => &mut self.view,
            Row::Change => &mut self.edit,
        }
    }

    /// `row`'s entries as Settings lists them: Can edit's first is the thread's own.
    pub fn shown(&self, row: Row) -> Vec<Folder> {
        let thread = (row == Row::Change).then_some(Folder::Thread);
        let added = (self.row(row).iter()).map(|folder| Folder::Added(name_of(folder)));
        thread.into_iter().chain(added).collect()
    }

    /// Adds `folder` to `row`, moving it from the other row if it's there. Whether it changed.
    fn add(&mut self, row: Row, folder: Granted) -> bool {
        if self.row(row).contains(&folder) {
            return false;
        }
        let other = match row {
            Row::Read => Row::Change,
            Row::Change => Row::Read,
        };
        self.row_mut(other).retain(|each| *each != folder);
        self.row_mut(row).push(folder);
        true
    }

    /// Takes the `index`th of `row`'s entries off, as `shown` lists them, and gives it: never the
    /// thread's own.
    fn remove(&mut self, row: Row, index: usize) -> Option<Granted> {
        let index = match row {
            Row::Change => index.checked_sub(1)?,
            Row::Read => index,
        };
        let added = self.row_mut(row);
        (index < added.len()).then(|| added.remove(index))
    }

    /// The folders as a thread's file acts are judged by (`Never::at`), beside its own.
    pub fn listed(&self) -> Vec<Listed> {
        let view = (self.view.iter()).map(|folder| Listed::of(folder, Row::Read));
        let edit = (self.edit.iter()).map(|folder| Listed::of(folder, Row::Change));
        view.chain(edit).collect()
    }

    /// As the agents' settings keep them.
    pub(super) fn saved(&self) -> Value {
        let paths = |row: &Vec<Granted>| -> Vec<String> {
            (row.iter())
                .map(|folder| folder.root().to_string_lossy().into_owned())
                .collect()
        };
        json!({ "view": paths(&self.view), "edit": paths(&self.edit) })
    }

    /// As the agents' settings kept them, each granted again by its real path: one gone, moved
    /// behind a link or refused by `never` by now is left out.
    pub(super) fn read(saved: Option<&Value>, never: &Never) -> Folders {
        let mut folders = Folders::default();
        for row in [Row::Read, Row::Change] {
            let key = match row {
                Row::Read => "view",
                Row::Change => "edit",
            };
            let paths = (saved.and_then(|saved| saved.get(key)))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str);
            for path in paths {
                match files::grant(Path::new(path), never) {
                    Some(folder) if folder.root() == Path::new(path) => {
                        folders.add(row, folder);
                    }
                    // Never its path (#3656).
                    _ => crate::log!("browser: agents folders left one out"),
                }
            }
        }
        folders
    }
}

/// A folder's name as the user sees it, never its path.
fn name_of(folder: &Granted) -> SharedString {
    files::shown_name(folder.root()).into()
}

fn row_name(row: Row) -> &'static str {
    match row {
        Row::Read => CAN_VIEW,
        Row::Change => CAN_EDIT,
    }
}

impl Agents {
    /// Settings › Agents › Folders' `row`, the thread's own folder first in Can edit.
    pub fn folders(&self, row: Row) -> Vec<Folder> {
        self.folders.shown(row)
    }

    /// The folders the user added, as a thread's file acts are judged by.
    pub fn listed_folders(&self) -> Vec<Listed> {
        self.folders.listed()
    }

    /// The user picked `picked` in the system's panel for `row` (+ Add Folder): it's added if ilo
    /// grants it by its real path, and the log keeps it. Whether it was granted, as the row says
    /// otherwise.
    pub fn add_folder(&mut self, row: Row, picked: &Path, cx: &mut Context<Self>) -> bool {
        let never = Never::here(Some(self.tasks_dir()));
        let Some(folder) = files::grant(picked, &never) else {
            // Never the folder's name or path.
            crate::log!("browser: agents folders refused one for {}", row_name(row));
            return false;
        };
        let name = name_of(&folder);
        if self.folders.add(row, folder) {
            self.note_folders(ADDED, row, &name);
            self.save();
            cx.notify();
        }
        true
    }

    /// The user removes `row`'s `index`th entry, as `folders` lists them, from its ⋯ menu. The
    /// thread's own folder stays.
    pub fn remove_folder(&mut self, row: Row, index: usize, cx: &mut Context<Self>) {
        let Some(folder) = self.folders.remove(row, index) else {
            return;
        };
        self.note_folders(REMOVED, row, &name_of(&folder));
        self.save();
        cx.notify();
    }

    /// The log's entry for a change of `row`, by the folder's name alone.
    fn note_folders(&mut self, action: &str, row: Row, name: &str) {
        let target = format!("{}: {name}", row_name(row));
        crate::log!("browser: agents {action} to {}", row_name(row));
        let entry = Activity {
            at: SystemTime::now(),
            agent: AGENT.into(),
            connection: 0,
            action: SharedString::from(action.to_string()),
            site: SharedString::default(),
            target: target.into(),
            refused: SharedString::default(),
            steps: Vec::new(),
            by: SharedString::default(),
            id: entry_id(),
            url: SharedString::default(),
            sent: Vec::new(),
            said: SharedString::default(),
            tier: SharedString::default(),
            notes: SharedString::default(),
            strand: SharedString::default(),
        };
        if self.log.len() >= LOG_LENGTH {
            self.log.remove(0);
        }
        self.log.push(entry);
    }
}
