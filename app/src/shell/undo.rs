//! ⌘Z and ⇧⌘Z: one history for the window, of what the user did that loses something or moves it
//! away, each step undone by doing its opposite. Undoing an edit gives back the edit that undoes
//! the undo, so redo is the same code as undo, run from the other stack.

use std::{rc::Rc, time::Duration};

use gpui::{Context, SharedString};

use crate::link_preview::Closed;

use super::{Shell, Space};

/// How long the toast that offers an Undo stays, and for as long as the pointer is over it.
const OFFER_TIME: Duration = Duration::from_secs(5);

/// How many steps a window can undo.
const KEPT: usize = 100;

/// What an edit is called: in the Edit menu after Undo and Redo, and in the toast that offers the
/// Undo where the edit takes something out of sight without a trace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct What {
    pub menu: &'static str,
    pub toast: Option<&'static str>,
}

impl What {
    const fn quiet(menu: &'static str) -> Self {
        What { menu, toast: None }
    }

    const fn said(menu: &'static str, toast: &'static str) -> Self {
        What {
            menu,
            toast: Some(toast),
        }
    }

    pub const CLOSE_TAB: What = What::quiet("Close Tab");
    pub const CLOSE_TABS: What = What::said("Close Tabs", "Tabs Closed");
    pub const CLOSE_PREVIEW: What = What::quiet("Close Link Preview");
    pub const PIN: What = What::quiet("Pin Tab");
    pub const UNPIN: What = What::quiet("Unpin Tab");
    pub const PIN_TABS: What = What::quiet("Pin Tabs");
    pub const UNPIN_TABS: What = What::quiet("Unpin Tabs");
    pub const MOVE_TABS_TO_SPACE: What = What::said("Move to Space", "Tabs Moved to Space");
    pub const NEW_FOLDER: What = What::quiet("New Folder");
    pub const ADD_FAVORITE: What = What::quiet("Add to Favorites");
    pub const REMOVE_FAVORITE: What = What::said("Remove from Favorites", "Removed from Favorites");
    pub const MOVE_TAB: What = What::quiet("Move Tab");
    pub const DUPLICATE: What = What::quiet("Duplicate");
    pub const MOVE_FAVORITE: What = What::quiet("Move Favorite");
    pub const MOVE_TO_SPACE: What = What::said("Move to Space", "Tab Moved to Space");
    pub const DELETE_FOLDER: What = What::said("Delete Folder", "Folder Deleted");
    pub const REMOVE_FROM_SPLIT: What = What::quiet("Remove from Split");
    pub const SPLIT: What = What::quiet("Split View");
    pub const CHANGE_ORIENTATION: What = What::quiet("Change Orientation");
    pub const RENAME_TAB: What = What::quiet("Rename Tab");
    pub const RENAME_FOLDER: What = What::quiet("Rename Folder");
    pub const RENAME_SPACE: What = What::quiet("Rename Space");
    pub const CHANGE_ICON: What = What::quiet("Change Icon");
    pub const CHANGE_PINNED_URL: What = What::quiet("Change Pinned URL");
    pub const CHANGE_THEME: What = What::quiet("Change Theme Color");
    pub const REMOVE_THEME: What = What::quiet("Remove Theme Color");
    pub const CLEAR: What = What::quiet("Clear");
    /// Arc says it in its own words (docs/ux-spec.md › Toast), so the shell shows that toast.
    pub const DELETE_SPACE: What = What::quiet("Delete Space");
    pub const DELETE_ARCHIVED: What = What::said("Delete from Archive", "Deleted from Archive");
    pub const CLEAR_ARCHIVE: What = What::said("Clear Archive", "Archive Cleared");
}

/// Where a tab, a favorite or a folder is, exactly enough to put it back.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Spot {
    /// The Space it's in, or the Space shown for a favorite.
    space: u64,
    at: At,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum At {
    /// At this index among its profile's favorites.
    Favorite(usize),
    /// At this index among the pinned tabs and folders in `folder`, or directly in the pinned tabs
    /// for 0, counted as `Browser::move_tab` counts.
    Pinned {
        folder: u64,
        index: usize,
    },
    Today(usize),
}

/// A folder as it was when it was deleted, to make it again.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Folder {
    id: u64,
    space: u64,
    name: SharedString,
    open: bool,
    /// The folder it was in, else 0, and its index there.
    container: u64,
    index: usize,
    /// What it held, in order: the first tab of each split view, and the folders.
    children: Vec<u64>,
}

/// What a name, an icon or a colour belongs to. A Space's colour comes with the theme it's the
/// main colour of, as the theme picker gives both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Prop {
    TabName(u64),
    TabIcon(u64),
    /// The URL a pinned tab or a favorite goes back to.
    PinnedUrl(u64),
    FolderName(u64),
    FolderIcon(u64),
    SpaceName(u64),
    SpaceIcon(u64),
    SpaceColor(u64),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    Text(String),
    /// A Space's colour, or none for the system's accent, and its theme as `Theme` writes it, or
    /// empty for none.
    Color { color: Option<u32>, theme: String },
}

/// A step. Applying one does it and returns the step that undoes it, or None if it can't be done
/// any more, such as a tab that's gone.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Edit {
    /// Reopens closed Today tabs, by the ids they had, behind the tab shown, and shows `show`, the
    /// one that was shown when they closed, if it's among them.
    Reopen { tabs: Vec<u64>, show: Option<u64> },
    /// Closes Today tabs.
    Close(Vec<u64>),
    /// Puts a tab, a favorite or a folder at a spot, and a pinned tab's or favorite's URL back.
    Put {
        tab: u64,
        spot: Spot,
        url: String,
    },
    /// Makes a deleted folder again, with what it held.
    Restore(Folder),
    /// Deletes a folder.
    Delete(u64),
    /// Puts a tab in a split view with another, at its left end or right end.
    Join {
        tab: u64,
        with: u64,
        left: bool,
    },
    /// Takes a tab out of its split view.
    Separate(u64),
    /// Puts a tab in a split view with another, as `Join` does, stacking the split if `stacked`,
    /// which the user did and ⌘Z takes back out.
    Split {
        tab: u64,
        with: u64,
        left: bool,
        stacked: bool,
    },
    /// Takes a tab out of the split view it joined, with `back`: the `Put` back to its row and
    /// place, or the `Join` back to the split it was in. The tab shown stays shown.
    Unsplit { tab: u64, back: Box<Edit> },
    /// Stacks the pages of a tab's split view top to bottom, or puts them side by side.
    Stack { tab: u64, stacked: bool },
    /// Gives a name, an icon or a colour back.
    Set(Prop, Value),
    /// Brings back the tabs the window's Clear `clear` closed in `space`. Each of these three asks
    /// the core for its own step by id, so it never takes one that another window made.
    UndoClear {
        space: u64,
        clear: u64,
    },
    /// Clears the Space shown again, if it's this one.
    Clear(u64),
    /// Brings back the Space, which `DeleteSpace` deleted.
    UndoDeleteSpace(u64),
    DeleteSpace(u64),
    /// Puts back what the archive removal `removal` took out: the archived tab `id`, or all.
    UndoArchive {
        removal: u64,
        id: Option<u64>,
    },
    /// Takes the archived tab out of the archive, or all of them.
    Archive(Option<u64>),
    /// Several steps the user took as one, such as pinning the rows chosen, each undone in turn.
    Many(Vec<Edit>),
    /// Opens the link preview the user closed again, over its tab.
    ReopenPreview(Closed),
    /// Closes the link preview over the tab, which Redo does.
    ClosePreview(Closed),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Entry {
    pub what: What,
    pub edit: Edit,
}

#[derive(Default)]
pub(super) struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    /// A `Many` step's parts still to do and the steps that undo those done, which a new id made
    /// on the way reaches too.
    todo: Vec<Edit>,
    done: Vec<Edit>,
}

impl History {
    /// What ⌘Z and ⇧⌘Z would do.
    pub(super) fn titles(&self) -> (Option<&'static str>, Option<&'static str>) {
        let title = |stack: &[Entry]| stack.last().map(|entry| entry.what.menu);
        (title(&self.undo), title(&self.redo))
    }

    /// A reopened tab or a made folder has a new id, which the steps that mention it need.
    fn rekey(&mut self, from: u64, to: u64) {
        for entry in self.undo.iter_mut().chain(&mut self.redo) {
            entry.edit.rekey(from, to);
        }
        for edit in self.todo.iter_mut().chain(&mut self.done) {
            edit.rekey(from, to);
        }
    }
}

impl Edit {
    fn rekey(&mut self, from: u64, to: u64) {
        let id = |id: &mut u64| {
            if *id == from {
                *id = to;
            }
        };
        let ids = |ids: &mut Vec<u64>| ids.iter_mut().for_each(|each| id_of(each, from, to));
        match self {
            Edit::Reopen { tabs, show } => {
                ids(tabs);
                show.iter_mut().for_each(|each| id_of(each, from, to));
            }
            Edit::Close(tabs) => ids(tabs),
            Edit::Put { tab, spot, .. } => {
                id(tab);
                if let At::Pinned { folder, .. } = &mut spot.at {
                    id(folder);
                }
            }
            Edit::Restore(folder) => {
                id(&mut folder.id);
                id(&mut folder.container);
                ids(&mut folder.children);
            }
            Edit::Delete(folder) => id(folder),
            Edit::Join { tab, with, .. } => {
                id(tab);
                id(with);
            }
            Edit::Separate(tab) | Edit::Stack { tab, .. } => id(tab),
            Edit::Split { tab, with, .. } => {
                id(tab);
                id(with);
            }
            Edit::Unsplit { tab, back } => {
                id(tab);
                back.rekey(from, to);
            }
            Edit::ReopenPreview(closed) | Edit::ClosePreview(closed) => id(&mut closed.over),
            Edit::Set(prop, _) => match prop {
                Prop::TabName(each)
                | Prop::TabIcon(each)
                | Prop::PinnedUrl(each)
                | Prop::FolderName(each)
                | Prop::FolderIcon(each) => id(each),
                Prop::SpaceName(_) | Prop::SpaceIcon(_) | Prop::SpaceColor(_) => {}
            },
            Edit::UndoClear { .. }
            | Edit::Clear(_)
            | Edit::UndoDeleteSpace(_)
            | Edit::DeleteSpace(_)
            | Edit::UndoArchive { .. }
            | Edit::Archive(_) => {}
            Edit::Many(edits) => edits.iter_mut().for_each(|edit| edit.rekey(from, to)),
        }
    }

    /// Where a part of a `Many` step goes among the others: a folder made again and a tab taken
    /// out of its split first, the moves down each list, and a folder deleted and a tab joining a
    /// split last, so it joins the row where the moves put it.
    fn order(&self) -> (u8, usize) {
        match self {
            Edit::Restore(_) | Edit::Unsplit { .. } => (0, 0),
            Edit::Put { spot, .. } => match spot.at {
                At::Favorite(index) | At::Pinned { index, .. } | At::Today(index) => (1, index),
            },
            Edit::Delete(_) | Edit::Split { .. } => (2, 0),
            _ => (1, 0),
        }
    }

    /// Does the step, and returns the one that undoes it.
    fn apply(self, shell: &mut Shell, cx: &mut Context<Shell>) -> Option<Edit> {
        match self {
            Edit::Reopen { tabs: closed, show } => {
                let mut tabs = Vec::new();
                let mut shown = None;
                for old in closed {
                    let new = shell.browser.reopen_tab(old, cx);
                    if new != 0 {
                        shell.history.rekey(old, new);
                        tabs.push(new);
                        if show == Some(old) {
                            shown = Some(new);
                        }
                    }
                }
                // The core leaves the tab shown alone, so a tab closed behind it comes back behind it.
                if let Some(tab) = shown {
                    shell.select(tab, cx);
                }
                (!tabs.is_empty()).then_some(Edit::Close(tabs))
            }
            Edit::Close(tabs) => {
                let tabs: Vec<u64> = tabs.into_iter().filter(|&tab| shell.has_tab(tab)).collect();
                let show = shell.shown_among(&tabs);
                for &tab in &tabs {
                    shell.browser.close(tab, false, cx);
                }
                (!tabs.is_empty()).then_some(Edit::Reopen { tabs, show })
            }
            Edit::Put { tab, spot, url } => {
                let now = shell.spot_of(tab)?;
                let was = shell.browser.pinned_url(tab);
                shell.put(tab, &now, &spot, cx);
                if !url.is_empty() && !matches!(spot.at, At::Today(_)) {
                    shell.browser.set_pinned_url(tab, &url, cx);
                }
                Some(Edit::Put {
                    tab,
                    spot: now,
                    url: was,
                })
            }
            Edit::Restore(folder) => {
                let made = shell.restore_folder(&folder, cx);
                shell.history.rekey(folder.id, made);
                Some(Edit::Delete(made))
            }
            Edit::Delete(folder) => {
                let snapshot = shell.folder_snapshot(folder)?;
                shell.browser.delete_folder(folder, cx);
                Some(snapshot)
            }
            Edit::Join { tab, with, left } => {
                if !shell.has_tab(tab) || !shell.has_tab(with) {
                    return None;
                }
                shell.browser.split_with(tab, with, left, cx);
                Some(Edit::Separate(tab))
            }
            Edit::Separate(tab) => {
                let join = shell.join_of(tab)?;
                shell.browser.separate(tab, cx);
                Some(join)
            }
            Edit::Split {
                tab,
                with,
                left,
                stacked,
            } => {
                if !shell.has_tab(tab) || !shell.has_tab(with) {
                    return None;
                }
                let undo = shell.unsplit_of(tab, with)?;
                shell.browser.split_with(tab, with, left, cx);
                if stacked {
                    shell.browser.set_split_stacked(with, true, cx);
                }
                Some(undo)
            }
            Edit::Unsplit { tab, back } => {
                let Some(Edit::Join { with, left, .. }) = shell.join_of(tab) else {
                    return None;
                };
                let (panes, stacked) = shell.split_of(tab)?;
                // A split of two goes with the tab, and its orientation with it, for Redo to give.
                let stacked = stacked && panes.len() == 2;
                let shown = shell.selected;
                match *back {
                    join @ Edit::Join { .. } => {
                        if join.apply(shell, cx).is_none() {
                            shell.browser.separate(tab, cx);
                        }
                    }
                    put => {
                        shell.browser.separate(tab, cx);
                        put.apply(shell, cx);
                    }
                }
                // The core shows the tab that leaves, where the user had another of its split. The
                // shell still has that one selected, so this goes past its check.
                if shown != tab && panes.contains(&shown) {
                    shell.browser.select(shown, cx);
                }
                Some(Edit::Split {
                    tab,
                    with,
                    left,
                    stacked,
                })
            }
            Edit::Stack { tab, stacked } => {
                let (_, now) = shell.split_of(tab)?;
                shell.browser.set_split_stacked(tab, stacked, cx);
                Some(Edit::Stack { tab, stacked: now })
            }
            Edit::Set(prop, value) => {
                let now = shell.value_of(prop)?;
                shell.set_prop(prop, &value, cx);
                Some(Edit::Set(prop, now))
            }
            Edit::UndoClear { space, clear } => {
                (shell.browser.undo_clear(clear, cx) > 0).then_some(Edit::Clear(space))
            }
            Edit::Clear(space) => {
                if shell.space != space {
                    return None;
                }
                let clear = shell.clear_now(cx)?;
                Some(Edit::UndoClear { space, clear })
            }
            Edit::UndoDeleteSpace(space) => shell
                .browser
                .undo_delete_space(space, cx)
                .then_some(Edit::DeleteSpace(space)),
            Edit::DeleteSpace(space) => shell
                .browser
                .delete_space(space, cx)
                .then_some(Edit::UndoDeleteSpace(space)),
            Edit::UndoArchive { removal, id } => {
                (shell.browser.undo_archive_remove(removal, cx) > 0).then_some(Edit::Archive(id))
            }
            Edit::Archive(id) => {
                let removal = match id {
                    Some(id) => shell.browser.remove_archived(id, cx),
                    None => shell.browser.clear_archive(cx),
                };
                (removal != 0).then_some(Edit::UndoArchive { removal, id })
            }
            Edit::Many(mut edits) => {
                // The core reports the lists once the calls have returned, so every part's spot
                // is from the lists as they were before any of them, and each goes back in place
                // only once the ones above it in its list have.
                edits.sort_by_key(Edit::order);
                edits.reverse();
                let todo = std::mem::replace(&mut shell.history.todo, edits);
                let done = std::mem::take(&mut shell.history.done);
                while let Some(edit) = shell.history.todo.pop() {
                    if let Some(back) = edit.apply(shell, cx) {
                        shell.history.done.push(back);
                    }
                }
                shell.history.todo = todo;
                let back = std::mem::replace(&mut shell.history.done, done);
                (!back.is_empty()).then_some(Edit::Many(back))
            }
            Edit::ReopenPreview(closed) => {
                shell.reopen_preview(&closed, "undo", cx).then_some(Edit::ClosePreview(closed))
            }
            Edit::ClosePreview(closed) => (shell.browser.close_preview(closed.over, cx))
                .then_some(Edit::ReopenPreview(closed)),
        }
    }
}

fn id_of(id: &mut u64, from: u64, to: u64) {
    if *id == from {
        *id = to;
    }
}

impl Shell {
    /// Remembers `edit`, which undoes what the user just did, as `what`. Not in a private window,
    /// which keeps nothing. What was undone before is gone, as in every app. If `what` takes
    /// something out of sight without a trace, the toast offers the Undo.
    pub(super) fn keep(&mut self, what: What, edit: Option<Edit>, cx: &mut Context<Self>) {
        let Some(edit) = edit.filter(|_| !self.private) else {
            return;
        };
        // ⇧⌘T reopens what closed last: a tab closed since comes before the preview.
        if matches!(edit, Edit::Reopen { .. } | Edit::UndoClear { .. }) {
            self.closed_preview = None;
            self.closed_pane = None;
        }
        self.history.redo.clear();
        self.history.undo.push(Entry { what, edit });
        if self.history.undo.len() > KEPT {
            self.history.undo.remove(0);
        }
        match what.toast {
            Some(words) => self.offer_undo(words, cx),
            None => self.withdraw_offer(cx),
        }
        self.send_edit_titles(false, cx);
    }

    /// Undoes the last step, once, and returns whether it did. A step that can't be done any more
    /// is dropped and the one before it is undone.
    pub(super) fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        self.step(true, cx)
    }

    /// Does again what the last Undo undid.
    pub(super) fn redo(&mut self, cx: &mut Context<Self>) -> bool {
        self.step(false, cx)
    }

    fn step(&mut self, undoing: bool, cx: &mut Context<Self>) -> bool {
        if self.private {
            return false;
        }
        // The Undo the toast offers is done, or is no longer the one.
        self.withdraw_offer(cx);
        let done = loop {
            let stack = match undoing {
                true => &mut self.history.undo,
                false => &mut self.history.redo,
            };
            let Some(Entry { what, edit }) = stack.pop() else {
                break false;
            };
            if let Some(inverse) = edit.apply(self, cx) {
                let stack = match undoing {
                    true => &mut self.history.redo,
                    false => &mut self.history.undo,
                };
                stack.push(Entry {
                    what,
                    edit: inverse,
                });
                break true;
            }
        };
        // check.sh waits for these lines.
        let (verb, result) = (
            if undoing { "undo" } else { "redo" },
            if done { "done" } else { "none" },
        );
        log!("browser: {verb} {result}");
        self.send_edit_titles(false, cx);
        done
    }

    fn offer_undo(&mut self, words: &str, cx: &mut Context<Self>) {
        let shell = cx.weak_entity();
        let undo = Rc::new(move |cx: &mut gpui::App| {
            shell.update(cx, |shell, cx| shell.undo(cx)).ok();
        });
        self.browser.toast_undo(words, undo, OFFER_TIME, cx);
        self.undo_offered = true;
    }

    fn withdraw_offer(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.undo_offered) {
            self.browser.hide_undo_toast(cx);
        }
    }

    /// Gives the Edit menu what Undo and Redo would do, if that's new or `always`.
    pub(super) fn send_edit_titles(&mut self, always: bool, cx: &mut Context<Self>) {
        let titles = self.history.titles();
        if always || titles != self.edit_titles {
            self.edit_titles = titles;
            self.browser.set_edit_titles(titles.0, titles.1, cx);
        }
    }

    /// The user closed the link preview over `closed.over`, which ⌘Z and ⇧⌘T open again, unless
    /// Redo closed it (`undone`), as the history has it already.
    pub fn preview_closed(&mut self, closed: Closed, undone: bool, cx: &mut Context<Self>) {
        if self.private {
            return;
        }
        self.closed_preview = Some(closed.clone());
        self.closed_pane = None;
        if !undone {
            self.keep(What::CLOSE_PREVIEW, Some(Edit::ReopenPreview(closed)), cx);
        }
    }

    /// Opens the link preview the user closed again over its tab, showing the tab first if another
    /// shows. Whether it could: the tab is still a pinned tab or a favorite.
    fn reopen_preview(&mut self, closed: &Closed, via: &str, cx: &mut Context<Self>) -> bool {
        if !self.has_tab(closed.over) {
            return false;
        }
        let Some(page) = self.page_laid() else {
            return false;
        };
        if !self.shows(closed.over) {
            self.select(closed.over, cx);
        }
        self.closed_preview = None;
        // check.sh waits for this line.
        log!("browser: link preview reopened over {} via {via}", closed.over);
        self.browser.reopen_preview(closed, page, cx);
        true
    }

    /// ⇧⌘T: the link preview the user closed, if no tab has closed since, else the tab closed
    /// last. ⌘Z then has nothing of the preview's to reopen.
    pub(super) fn reopen_closed(&mut self, cx: &mut Context<Self>) {
        if let Some(closed) = self.closed_preview.clone()
            && self.reopen_preview(&closed, "reopen-closed-tab", cx)
        {
            let reopen = Edit::ReopenPreview(closed);
            self.history.undo.retain(|entry| entry.edit != reopen);
            self.send_edit_titles(false, cx);
            return;
        }
        self.closed_preview = None;
        // Unless ⌘Z has put it back already, which takes its step off the history.
        if let Some(join) = self.closed_pane.take() {
            let kept = self.history.undo.len();
            self.history.undo.retain(|entry| entry.edit != join);
            if self.history.undo.len() < kept && join.apply(self, cx).is_some() {
                self.send_edit_titles(false, cx);
                return;
            }
        }
        self.browser.reopen_closed_tab(cx);
    }

    // What the user did, each remembered with how to take it back. The calls to the browser are
    // the ones the user's gestures made before, so there's no second path to keep in step.

    /// Closes `tab` (and its split's rest, if `split`), a Today tab as ⌘Z reopens at its place.
    /// A pinned tab or a favorite keeps its row, and only its page goes: a pinned tab's pane leaves
    /// its split, which ⌘Z and ⇧⌘T put it back in, as they would a Today tab's.
    pub(super) fn close_tab(&mut self, tab: u64, split: bool, cx: &mut Context<Self>) {
        let closing = self
            .spot_of(tab)
            .filter(|spot| matches!(spot.at, At::Today(_)));
        if closing.is_none()
            && !split
            && let Some(join) = self.join_of(tab)
        {
            self.browser.close(tab, false, cx);
            self.closed_preview = None;
            self.closed_pane = (!self.private).then(|| join.clone());
            return self.keep(What::CLOSE_TAB, Some(join), cx);
        }
        let tabs = match (closing, split) {
            (None, _) => Vec::new(),
            (Some(_), true) => self.panes(tab),
            (Some(_), false) => vec![tab],
        };
        let show = self.shown_among(&tabs);
        self.browser.close(tab, split, cx);
        let what = if tabs.len() > 1 {
            What::CLOSE_TABS
        } else {
            What::CLOSE_TAB
        };
        self.keep(what, (!tabs.is_empty()).then_some(Edit::Reopen { tabs, show }), cx);
    }

    /// Closes the Today tabs below `tab`.
    pub(super) fn close_tabs_below(&mut self, tab: u64, cx: &mut Context<Self>) {
        // Below its split view, if it's in one.
        let last = self.panes(tab).last().copied().unwrap_or(tab);
        let today = self.today();
        let below: Vec<u64> = match today.iter().position(|&each| each == last) {
            Some(at) => today[at + 1..].to_vec(),
            None => Vec::new(),
        };
        let show = self.shown_among(&below);
        self.browser.close_below(tab, cx);
        // The core closes them bottom up, so the top one reopens first, and each at its place.
        let what = if below.len() > 1 {
            What::CLOSE_TABS
        } else {
            What::CLOSE_TAB
        };
        let reopen = (!below.is_empty()).then_some(Edit::Reopen { tabs: below, show });
        self.keep(what, reopen, cx);
    }

    /// Pins `tab`, or unpins it.
    pub(super) fn pin_tab(&mut self, tab: u64, pinned: bool, cx: &mut Context<Self>) {
        let before = self.put_back(tab);
        self.browser.set_pinned(tab, pinned, cx);
        self.keep(if pinned { What::PIN } else { What::UNPIN }, before, cx);
    }

    /// Makes `tab` a favorite at `index`, or moves it among them.
    pub(super) fn favorite_tab(&mut self, tab: u64, index: usize, cx: &mut Context<Self>) {
        if !self.has_room_for(tab) {
            log!("browser: favorites full");
            return;
        }
        let before = self.put_back(tab);
        let moving = self.favorites.contains(&tab);
        self.browser.move_to_favorites(tab, index, cx);
        self.keep(
            if moving {
                What::MOVE_FAVORITE
            } else {
                What::ADD_FAVORITE
            },
            before,
            cx,
        );
    }

    /// Moves `tab` (or the folder `tab`) to `index` of the pinned tabs, `folder`'s, or Today's.
    pub(super) fn move_tab_to(
        &mut self,
        tab: u64,
        pinned: bool,
        folder: u64,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let before = self.put_back(tab);
        let was = self.spot_of(tab).map(|spot| spot.at);
        match self.is_folder(tab) {
            true => self.browser.move_folder(tab, folder, index, cx),
            false => self.browser.move_tab(tab, pinned, folder, index, cx),
        }
        let what = match was {
            Some(At::Favorite(_)) => What::REMOVE_FAVORITE,
            Some(At::Today(_)) if pinned => What::PIN,
            Some(At::Pinned { .. }) if !pinned => What::UNPIN,
            _ => What::MOVE_TAB,
        };
        self.keep(what, before, cx);
    }

    /// Moves `tab` to `space`, pinning it first if `pin`, which the one undo takes back too. Into
    /// another profile's Space it goes as a new tab, which has nothing to give back.
    pub(super) fn send_to_space(
        &mut self,
        tab: u64,
        space: u64,
        pin: bool,
        cx: &mut Context<Self>,
    ) {
        let same_profile = |this: &Self| {
            let profile = |id: u64| {
                this.spaces
                    .iter()
                    .find(|each| each.id == id)
                    .map(|each| each.profile)
            };
            let from = this.spot_of(tab).and_then(|spot| profile(spot.space));
            from.is_some() && from == profile(space)
        };
        let before = same_profile(self).then(|| self.put_back(tab)).flatten();
        // The core moves a pinned tab pinned, into another profile's Space too.
        let pinned = matches!(self.spot_of(tab).map(|spot| spot.at), Some(At::Pinned { .. }));
        if pin && !pinned {
            self.browser.set_pinned(tab, true, cx);
        }
        self.browser.move_to_space(tab, space, cx);
        self.keep(What::MOVE_TO_SPACE, before, cx);
    }

    /// Deletes `folder`, leaving what's in it where it is.
    pub(super) fn delete_folder_now(&mut self, folder: u64, cx: &mut Context<Self>) {
        let snapshot = self.folder_snapshot(folder);
        self.browser.delete_folder(folder, cx);
        self.keep(What::DELETE_FOLDER, snapshot, cx);
    }

    /// Takes `tab` out of its split view.
    pub(super) fn separate_tab(&mut self, tab: u64, cx: &mut Context<Self>) {
        let join = self.join_of(tab);
        self.browser.separate(tab, cx);
        self.keep(What::REMOVE_FROM_SPLIT, join, cx);
    }

    /// Puts `tab` in a split view with `with`, at its left end if `left`, else its right end, and
    /// stacks the split if `stacked`: one step, which ⌘Z takes back to where `tab` was.
    pub(super) fn join_split(
        &mut self,
        tab: u64,
        with: u64,
        left: bool,
        stacked: bool,
        cx: &mut Context<Self>,
    ) {
        self.keep_split(tab, with, cx);
        self.browser.split_with(tab, with, left, cx);
        if stacked {
            self.browser.set_split_stacked(with, true, cx);
        }
    }

    /// `tab` is joining `with` in a split view, which ⌘Z takes it back out of. A pinned tab
    /// joining a Today tab's split takes the split into its place in the pinned tabs first, as the
    /// core moves a tab into its partner's list, so the pin is never lost to a split; the one step
    /// undoes both.
    pub(super) fn keep_split(&mut self, tab: u64, with: u64, cx: &mut Context<Self>) {
        let undo = self.unsplit_of(tab, with);
        let pinned = match undo {
            Some(_) => self.pin_beside(tab, with, cx),
            None => None,
        };
        let undo = match (undo, pinned) {
            (Some(undo), Some(unpin)) => Some(Edit::Many(vec![undo, unpin])),
            (undo, _) => undo,
        };
        self.keep(What::SPLIT, undo, cx);
    }

    /// Pins `with`'s row, a Today tab or split, at its page in pinned `tab`'s place, for `tab` to
    /// join it there. The step that puts it back, or None where `tab` isn't pinned or `with` isn't
    /// a Today tab of its Space.
    fn pin_beside(&mut self, tab: u64, with: u64, cx: &mut Context<Self>) -> Option<Edit> {
        let (at, from) = (self.spot_of(tab)?, self.spot_of(with)?);
        let (At::Pinned { folder, index }, At::Today(_)) = (at.at, from.at) else {
            return None;
        };
        if at.space != from.space {
            return None;
        }
        let back = self.put_back(with)?;
        self.browser.move_tab(with, true, folder, index, cx);
        Some(back)
    }

    /// `tab`, opened to join a split view, joined it: ⌘Z closes it.
    pub fn split_opened(&mut self, tab: u64, cx: &mut Context<Self>) {
        self.keep(What::SPLIT, Some(Edit::Close(vec![tab])), cx);
    }

    /// Stacks the pages of `tab`'s split view top to bottom, or puts them side by side.
    pub(super) fn stack_split(&mut self, tab: u64, stacked: bool, cx: &mut Context<Self>) {
        let undo = self.split_of(tab).map(|(_, now)| Edit::Stack { tab, stacked: now });
        self.browser.set_split_stacked(tab, stacked, cx);
        self.keep(What::CHANGE_ORIENTATION, undo, cx);
    }

    /// Gives a name, an icon or a colour its new value, remembering the old, unless it's the same.
    pub(super) fn change(&mut self, prop: Prop, value: Value, cx: &mut Context<Self>) {
        let before = self.value_of(prop);
        if before.as_ref() == Some(&value) {
            return;
        }
        self.set_prop(prop, &value, cx);
        let what = match prop {
            Prop::TabName(_) => What::RENAME_TAB,
            Prop::FolderName(_) => What::RENAME_FOLDER,
            Prop::SpaceName(_) => What::RENAME_SPACE,
            Prop::TabIcon(_) | Prop::FolderIcon(_) | Prop::SpaceIcon(_) => What::CHANGE_ICON,
            Prop::PinnedUrl(_) => What::CHANGE_PINNED_URL,
            Prop::SpaceColor(_) => match value {
                Value::Color { color: None, .. } => What::REMOVE_THEME,
                _ => What::CHANGE_THEME,
            },
        };
        self.keep(what, before.map(|before| Edit::Set(prop, before)), cx);
    }

    /// Takes the archived tab out of the archive.
    pub(super) fn delete_archived(&mut self, id: u64, cx: &mut Context<Self>) {
        let removal = self.browser.remove_archived(id, cx);
        let edit = (removal != 0).then_some(Edit::UndoArchive {
            removal,
            id: Some(id),
        });
        self.keep(What::DELETE_ARCHIVED, edit, cx);
    }

    /// Empties the archive.
    pub(super) fn empty_archive(&mut self, cx: &mut Context<Self>) {
        let removal = self.browser.clear_archive(cx);
        let edit = (removal != 0).then_some(Edit::UndoArchive { removal, id: None });
        self.keep(What::CLEAR_ARCHIVE, edit, cx);
    }

    // What the steps need to know of the windows' tabs.

    /// Where `tab` is, in the Space that has it, else among the favorites shown.
    fn spot_of(&self, tab: u64) -> Option<Spot> {
        if let Some(index) = self.favorites.iter().position(|&each| each == tab) {
            return Some(Spot {
                space: self.space,
                at: At::Favorite(index),
            });
        }
        self.spaces.iter().find_map(|space| space.spot_of(tab))
    }

    /// The tab shown, if it's one of `tabs`.
    pub(super) fn shown_among(&self, tabs: &[u64]) -> Option<u64> {
        tabs.contains(&self.selected).then_some(self.selected)
    }

    /// Whether some Space, or the favorites, has `tab`.
    fn has_tab(&self, tab: u64) -> bool {
        self.spot_of(tab).is_some()
    }

    /// The step that puts `tab` where it is now, with the URL it goes back to.
    pub(super) fn put_back(&self, tab: u64) -> Option<Edit> {
        let spot = self.spot_of(tab)?;
        let url = self.browser.pinned_url(tab);
        Some(Edit::Put { tab, spot, url })
    }

    /// Puts `tab`, or the folder, from `now` to `spot`.
    fn put(&mut self, tab: u64, now: &Spot, spot: &Spot, cx: &mut Context<Self>) {
        let favorite = |at: At| matches!(at, At::Favorite(_));
        if now.space != spot.space && !favorite(now.at) && !favorite(spot.at) {
            self.browser.move_to_space(tab, spot.space, cx);
        }
        match spot.at {
            At::Favorite(index) => self.browser.move_to_favorites(tab, index, cx),
            At::Pinned { folder, index } => match self.is_folder_anywhere(tab) {
                true => self.browser.move_folder(tab, folder, index, cx),
                false => self.browser.move_tab(tab, true, folder, index, cx),
            },
            At::Today(index) => self.browser.move_tab(tab, false, 0, index, cx),
        }
    }

    /// `folder` as it is, to make it again once it's deleted.
    fn folder_snapshot(&self, folder: u64) -> Option<Edit> {
        let space = self.spaces.iter().find(|space| space.is_folder(folder))?;
        let (name, open) = space.folders.folders.get(&folder)?.clone();
        let Spot {
            at: At::Pinned {
                folder: container,
                index,
            },
            ..
        } = space.spot_of(folder)?
        else {
            return None;
        };
        let children = space.children(folder);
        Some(Edit::Restore(Folder {
            id: folder,
            space: space.id,
            name,
            open,
            container,
            index,
            children,
        }))
    }

    /// Makes `folder` again where it was, puts what it held back in it, and returns its new id.
    fn restore_folder(&mut self, folder: &Folder, cx: &mut Context<Self>) -> u64 {
        let made = self.browser.create_folder(
            folder.space,
            &folder.name,
            folder.container,
            folder.index,
            cx,
        );
        let mut index = 0;
        for &child in &folder.children {
            let panes = self.panes_in(folder.space, child);
            match self.is_folder_anywhere(child) {
                true => self.browser.move_folder(child, made, index, cx),
                false => self.browser.move_tab(child, true, made, index, cx),
            }
            index += panes;
        }
        if !folder.open {
            self.browser.set_folder_open(made, false, cx);
        }
        made
    }

    fn is_folder_anywhere(&self, row: u64) -> bool {
        self.spaces.iter().any(|space| space.is_folder(row))
    }

    /// How many tabs the row `row` of `space` is: its split view's, else one.
    fn panes_in(&self, space: u64, row: u64) -> usize {
        let space = self.spaces.iter().find(|each| each.id == space);
        space.map_or(1, |space| space.panes(row).len())
    }

    /// The step that puts `tab` back in a split view with the tab beside it.
    fn join_of(&self, tab: u64) -> Option<Edit> {
        let panes = self
            .spaces
            .iter()
            .map(|space| space.panes(tab))
            .find(|panes| panes.len() > 1)?;
        let at = panes.iter().position(|&each| each == tab)?;
        // Beside the one before it, else the one after.
        Some(match at {
            0 => Edit::Join {
                tab,
                with: panes[1],
                left: true,
            },
            _ => Edit::Join {
                tab,
                with: panes[at - 1],
                left: false,
            },
        })
    }

    /// The step that takes `tab` out of the split view it's joining with `with`, back to where it
    /// is: the split it's in, else its row. None where the core won't join them, as `with`'s split
    /// is full or has it.
    fn unsplit_of(&self, tab: u64, with: u64) -> Option<Edit> {
        let panes = self.split_of(with).map_or_else(|| vec![with], |(panes, _)| panes);
        if panes.contains(&tab) || panes.len() >= super::MAX_PANES {
            return None;
        }
        let back = self.join_of(tab).or_else(|| self.put_back(tab))?;
        Some(Edit::Unsplit {
            tab,
            back: Box::new(back),
        })
    }

    /// The tabs of `tab`'s split view, in any Space, and whether it's stacked.
    fn split_of(&self, tab: u64) -> Option<(Vec<u64>, bool)> {
        self.spaces.iter().find_map(|space| {
            let panes = space.panes(tab);
            (panes.len() > 1).then(|| (panes, space.stacked(tab)))
        })
    }

    fn value_of(&self, prop: Prop) -> Option<Value> {
        let space = |id: u64| self.spaces.iter().find(|space| space.id == id);
        Some(match prop {
            Prop::TabName(tab) => {
                self.has_tab(tab).then_some(())?;
                Value::Text(self.browser.tab_name(tab))
            }
            Prop::TabIcon(tab) => {
                self.has_tab(tab).then_some(())?;
                Value::Text(
                    self.tabs
                        .get(&tab)
                        .map_or_else(String::new, |tab| tab.page.icon.to_string()),
                )
            }
            Prop::PinnedUrl(tab) => {
                self.has_tab(tab).then_some(())?;
                Value::Text(self.browser.pinned_url(tab))
            }
            Prop::FolderName(folder) => {
                let space = self.spaces.iter().find(|space| space.is_folder(folder))?;
                Value::Text(space.folders.folders.get(&folder)?.0.to_string())
            }
            Prop::FolderIcon(folder) => {
                self.spaces.iter().any(|space| space.is_folder(folder)).then_some(())?;
                Value::Text(self.folder_icon(folder).to_string())
            }
            Prop::SpaceName(id) => Value::Text(space(id)?.name.to_string()),
            Prop::SpaceIcon(id) => Value::Text(space(id)?.icon.to_string()),
            Prop::SpaceColor(id) => {
                let space = space(id)?;
                let theme = space.theme.map_or_else(String::new, |theme| theme.to_text());
                Value::Color { color: space.color, theme }
            }
        })
    }

    fn set_prop(&mut self, prop: Prop, value: &Value, cx: &mut Context<Self>) {
        let browser = self.browser.clone();
        match (prop, value) {
            (Prop::TabName(tab), Value::Text(name)) => browser.rename_tab(tab, name, cx),
            (Prop::TabIcon(tab), Value::Text(icon)) => browser.set_icon(tab, icon, cx),
            (Prop::PinnedUrl(tab), Value::Text(url)) => browser.set_pinned_url(tab, url, cx),
            (Prop::FolderName(folder), Value::Text(name)) => {
                browser.rename_folder(folder, name, cx)
            }
            (Prop::FolderIcon(folder), Value::Text(icon)) => {
                browser.set_folder_icon(folder, icon, cx)
            }
            (Prop::SpaceName(space), Value::Text(name)) => browser.rename_space(space, name, cx),
            (Prop::SpaceIcon(space), Value::Text(icon)) => browser.set_space_icon(space, icon, cx),
            (Prop::SpaceColor(space), Value::Color { color, theme }) => {
                // The theme being made gives way to the one ⌘Z gives back.
                if self.theme_preview.is_some_and(|(previewed, made)| {
                    previewed == space && made.to_text() != *theme
                }) {
                    self.theme_preview = None;
                }
                browser.set_space_theme(space, theme, cx);
                browser.set_space_color(space, *color, cx);
                cx.notify();
            }
            _ => {}
        }
    }
}

impl Space {
    /// Where `tab` is among its pinned tabs and folders or its Today tabs, counted as
    /// `Browser::move_tab` counts: without its split view, whose tabs move with it.
    fn spot_of(&self, tab: u64) -> Option<Spot> {
        let first = self.panes(tab)[0];
        if let Some(index) = self.today.iter().position(|&each| each == first) {
            return Some(Spot {
                space: self.id,
                at: At::Today(index),
            });
        }
        if !self.pinned.contains(&first) {
            return None;
        }
        let folder = self.folder_of(first);
        let index = (self.pinned.iter().take_while(|&&each| each != first))
            .filter(|&&each| self.folder_of(each) == folder)
            .count();
        Some(Spot {
            space: self.id,
            at: At::Pinned { folder, index },
        })
    }

    /// What `folder` holds directly, in order: the first tab of each split view, and folders.
    fn children(&self, folder: u64) -> Vec<u64> {
        let later = |tab: &u64| {
            self.splits
                .iter()
                .any(|split| split.tabs[1..].contains(tab))
        };
        (self.pinned.iter().copied())
            .filter(|&row| self.folder_of(row) == folder && !later(&row))
            .collect()
    }
}
