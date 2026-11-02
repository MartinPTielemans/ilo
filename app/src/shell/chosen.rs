//! Several sidebar rows chosen at once, as Arc's ⌘-click and ⇧-click choose them, for what's
//! done to them together: ⌘W closes them, ⇧⌘C copies their links, and ⌘D, New Folder and their
//! menu pin, folder or move them, each as one step to undo. The tab shown stays the one shown;
//! the others are drawn with the secondary fill. A plain click, Esc or another Space lets them
//! go. A chosen folder stands for itself and what's in it: it moves whole, its links are copied,
//! and ⌘W and ⌘D, which are a tab's, leave it be.

use gpui::{Context, MouseDownEvent, SharedString};

use super::{Edit, NEW_FOLDER, Shell, What, clean_link};

impl Shell {
    /// The rows that can be chosen, in the sidebar's order: the shown Space's pinned tabs and
    /// folders, outside closed folders, then its Today tabs. A split view is one row, by its first
    /// tab.
    fn choosable_rows(&self) -> Vec<u64> {
        let (pinned, today) = self.rows();
        let space = self.shown_space();
        let shut = |row: u64| space.is_some_and(|space| space.shut(row));
        let pinned = pinned.into_iter().filter(|&row| !shut(row));
        pinned.chain(today).collect()
    }

    /// A left press on `tab`'s row or segment: ⌘ chooses it and ⇧ the rows through it, and
    /// returns true, as the press neither shows the tab nor drags it. A plain press lets the
    /// chosen rows go, and is the row's own.
    pub(super) fn choose_by_press(
        &mut self,
        tab: u64,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let (command, shift) = (event.modifiers.secondary(cx), event.modifiers.shift);
        match (command, shift) {
            (true, false) => self.choose_row(tab, cx),
            (false, true) => self.choose_through(tab, cx),
            _ => {
                self.unchoose(cx);
                return false;
            }
        }
        true
    }

    /// ⌘-click on `tab`'s row: chooses it, with the row shown, or lets it go if it's chosen.
    pub(super) fn choose_row(&mut self, tab: u64, cx: &mut Context<Self>) {
        let rows = self.choosable_rows();
        let row = self.row_of(tab);
        if !rows.contains(&row) {
            return;
        }
        let shown = self.row_of(self.selected);
        if self.chosen.is_empty() && shown != row && rows.contains(&shown) {
            self.chosen.push(shown);
        }
        match self.chosen.iter().position(|&each| each == row) {
            Some(at) => {
                self.chosen.remove(at);
            }
            None => self.chosen.push(row),
        }
        self.chosen_from = row;
        self.chosen_changed(cx);
    }

    /// ⇧-click on `tab`'s row: chooses the rows from the one last ⌘-clicked, else the one shown,
    /// through it, keeping those chosen already.
    pub(super) fn choose_through(&mut self, tab: u64, cx: &mut Context<Self>) {
        let rows = self.choosable_rows();
        let from = match self.chosen_from {
            0 => self.row_of(self.selected),
            from => from,
        };
        let (Some(from), Some(to)) = (
            rows.iter().position(|&row| row == from),
            rows.iter().position(|&row| row == self.row_of(tab)),
        ) else {
            return self.choose_row(tab, cx);
        };
        for &row in &rows[from.min(to)..=from.max(to)] {
            if !self.chosen.contains(&row) {
                self.chosen.push(row);
            }
        }
        self.chosen_from = rows[from];
        self.chosen_changed(cx);
    }

    /// Lets the chosen rows go, and returns whether there were any.
    pub(super) fn unchoose(&mut self, cx: &mut Context<Self>) -> bool {
        if self.chosen.is_empty() {
            return false;
        }
        self.chosen.clear();
        self.chosen_from = 0;
        self.chosen_changed(cx);
        true
    }

    /// Keeps only the chosen rows still in the sidebar, after the core reports the lists.
    pub(super) fn keep_chosen(&mut self, cx: &mut Context<Self>) {
        if self.chosen.is_empty() {
            return;
        }
        let rows = self.choosable_rows();
        let before = self.chosen.len();
        self.chosen.retain(|row| rows.contains(row));
        if self.chosen.len() != before {
            self.chosen_changed(cx);
        }
    }

    fn chosen_changed(&mut self, cx: &mut Context<Self>) {
        // A choice of the row shown alone is no choice.
        if self.chosen == [self.row_of(self.selected)] {
            self.chosen.clear();
        }
        let rows: Vec<String> = self.chosen_in_order().iter().map(u64::to_string).collect();
        // check.sh reads this line.
        log!("browser: chosen {}", rows.join(" "));
        cx.notify();
    }

    /// The chosen rows in the sidebar's order, but those in a chosen folder, which go with it.
    pub(super) fn chosen_in_order(&self) -> Vec<u64> {
        let space = self.shown_space();
        let in_chosen = |row: u64| {
            let mut folder = space.map_or(0, |space| space.folder_of(row));
            while folder != 0 && !self.chosen.contains(&folder) {
                folder = space.map_or(0, |space| space.folder_of(folder));
            }
            folder != 0
        };
        let rows = self.choosable_rows();
        rows.into_iter().filter(|&row| self.chosen.contains(&row) && !in_chosen(row)).collect()
    }

    /// The chosen rows that are tabs, which ⌘W and ⌘D act on.
    pub(super) fn chosen_tabs(&self) -> Vec<u64> {
        let rows = self.chosen_in_order();
        rows.into_iter().filter(|&row| !self.is_folder(row)).collect()
    }

    /// The tabs of `rows`, each folder's in it, in the sidebar's order.
    fn tabs_of(&self, rows: &[u64]) -> Vec<u64> {
        (rows.iter())
            .flat_map(|&row| match self.is_folder(row) {
                true => self.folder_tabs(row),
                false => self.panes(row),
            })
            .collect()
    }

    /// ⌘W with rows chosen closes them all, as Arc's does: Today's archive, as one step to undo,
    /// and pinned tabs unload and keep their rows, as ⌘W does to one.
    pub(super) fn close_chosen(&mut self, cx: &mut Context<Self>) {
        let rows = self.chosen_tabs();
        let today = self.today();
        let tabs: Vec<u64> = (rows.iter().filter(|row| today.contains(row)))
            .flat_map(|&row| self.panes(row))
            .collect();
        let show = self.shown_among(&tabs);
        // Bottom up, as Close Tabs Below closes them, so each reopens at its place.
        for &row in rows.iter().rev() {
            self.browser.close(row, true, cx);
        }
        self.chosen.clear();
        self.chosen_from = 0;
        let what = if tabs.len() > 1 { What::CLOSE_TABS } else { What::CLOSE_TAB };
        self.keep(what, (!tabs.is_empty()).then_some(Edit::Reopen { tabs, show }), cx);
    }

    /// ⇧⌘C with rows chosen copies each of their pages' links, one a line, without tracking
    /// parameters, as Arc's Copy URLs does.
    pub(super) fn copy_chosen(&mut self, cx: &mut Context<Self>) {
        let rows = self.chosen_in_order();
        let links: Vec<String> = (self.tabs_of(&rows).into_iter())
            .map(|tab| clean_link(&self.url(tab)))
            .filter(|link| !link.is_empty())
            .collect();
        if !links.is_empty() {
            self.copy(&links.join("\n"), "Copied URLs", cx);
        }
    }

    /// Whether ⌘D and the chosen rows' menu pin them: if any is a Today tab. Otherwise they unpin.
    pub(super) fn chosen_pin(&self) -> bool {
        let today = self.today();
        self.chosen.iter().any(|row| today.contains(row))
    }

    /// Pins the chosen Today tabs, or unpins the chosen rows if all are pinned, in their order.
    pub(super) fn pin_chosen(&mut self, cx: &mut Context<Self>) {
        let pin = self.chosen_pin();
        let today = self.today();
        let mut rows: Vec<u64> = (self.chosen_tabs().into_iter())
            .filter(|row| today.contains(row) == pin)
            .collect();
        // Pinning puts a tab at the bottom of the pinned tabs and unpinning at the top of Today.
        if !pin {
            rows.reverse();
        }
        let mut edits = Vec::new();
        for row in rows {
            edits.extend(self.put_back(row));
            self.browser.set_pinned(row, pin, cx);
        }
        let what = if pin { What::PIN_TABS } else { What::UNPIN_TABS };
        self.keep(what, (!edits.is_empty()).then_some(Edit::Many(edits)), cx);
    }

    /// Moves the chosen rows to `space`, a Space of the same profile, in their order, and lets
    /// them go.
    pub(super) fn move_chosen_to_space(&mut self, space: u64, cx: &mut Context<Self>) {
        let today = self.today();
        let (today_rows, pinned): (Vec<u64>, Vec<u64>) =
            self.chosen_in_order().into_iter().partition(|row| today.contains(row));
        let mut edits = Vec::new();
        // A pinned tab or folder goes to the bottom of the Space's pinned tabs and a Today tab to
        // the top of its Today.
        for row in pinned.into_iter().chain(today_rows.into_iter().rev()) {
            edits.extend(self.put_back(row));
            match self.is_folder(row) {
                true => self.browser.move_folder_to_space(row, space, cx),
                false => self.browser.move_to_space(row, space, cx),
            }
        }
        self.unchoose(cx);
        self.keep(What::MOVE_TABS_TO_SPACE, (!edits.is_empty()).then_some(Edit::Many(edits)), cx);
    }

    /// The chosen rows' Add to Folder › New Folder: as `add_to_new_folder`.
    pub(super) fn folder_chosen(&mut self, cx: &mut Context<Self>) {
        self.add_to_new_folder(self.chosen_in_order(), cx);
    }

    /// Add to Folder › New Folder pins `rows` into a new folder, in their order, named for their
    /// site, with its name in a field to type over (#1088). One pinned tab's folder takes its
    /// place; a Today tab's, or several rows', goes at the bottom of the pinned tabs, as Arc's New
    /// Folder with Tabs does.
    pub(super) fn add_to_new_folder(&mut self, rows: Vec<u64>, cx: &mut Context<Self>) {
        let name = self.folder_name_for(&rows);
        let (parent, index) = match (rows.as_slice(), self.shown_space()) {
            ([row], Some(space)) if space.pinned.contains(row) => {
                let parent = space.folder_of(*row);
                let before = space.pinned.iter().take_while(|&each| each != row);
                (parent, before.filter(|&&each| space.folder_of(each) == parent).count())
            }
            _ => (0, usize::MAX),
        };
        let folder = self.browser.create_folder(self.space, &name, parent, index, cx);
        if folder == 0 {
            return;
        }
        let mut edits = vec![Edit::Delete(folder)];
        let mut index = 0;
        for row in rows {
            edits.extend(self.put_back(row));
            self.put_in_folder(row, folder, index, cx);
            index += self.panes(row).len();
        }
        self.unchoose(cx);
        self.keep(What::NEW_FOLDER, Some(Edit::Many(edits)), cx);
        self.name_folder(folder, name, cx);
    }

    /// Add to Folder › one of the Space's folders: `rows` go to its bottom, pinned, in their
    /// order, as one step to undo (#1088).
    pub(super) fn add_to_folder(&mut self, rows: Vec<u64>, folder: u64, cx: &mut Context<Self>) {
        let Some(space) = self.shown_space() else {
            return;
        };
        let moving: Vec<u64> = rows.iter().flat_map(|&row| self.panes(row)).collect();
        let held = |row: &&u64| space.folder_of(**row) == folder && !moving.contains(row);
        let mut index = space.pinned.iter().filter(held).count();
        let mut edits = Vec::new();
        // Not a folder into itself, or into one it holds.
        let holds = |row: u64| {
            let mut inside = folder;
            while inside != 0 && inside != row {
                inside = space.folder_of(inside);
            }
            inside != 0
        };
        let rows: Vec<u64> = rows.into_iter().filter(|&row| !holds(row)).collect();
        for row in rows {
            edits.extend(self.put_back(row));
            self.put_in_folder(row, folder, index, cx);
            index += self.panes(row).len();
        }
        self.unchoose(cx);
        self.keep(What::MOVE_TAB, (!edits.is_empty()).then_some(Edit::Many(edits)), cx);
    }

    /// Pins `row` at `index` in `folder`, a folder whole.
    fn put_in_folder(&mut self, row: u64, folder: u64, index: usize, cx: &mut Context<Self>) {
        match self.is_folder(row) {
            true => self.browser.move_folder(row, folder, index, cx),
            false => self.browser.move_tab(row, true, folder, index, cx),
        }
    }

    /// The name of a folder made from `rows`: their site's, if every tab in them is on one, else
    /// New Folder.
    fn folder_name_for(&self, rows: &[u64]) -> SharedString {
        let tabs: Vec<(SharedString, SharedString)> = (self.tabs_of(rows).into_iter())
            .map(|tab| (self.page(tab).url, self.title(tab)))
            .collect();
        let tabs = tabs.iter().map(|(url, title)| (url.as_ref(), title.as_ref()));
        site_name(tabs).map_or_else(|| NEW_FOLDER.into(), Into::into)
    }
}

/// The site every one of `tabs`, by URL and title, is on, by the name its titles give it, else its
/// host without "www.": github.com's tabs, titled "Pull requests · GitHub", make "GitHub". None
/// for tabs on different sites, or for a tab that isn't on a web site at all.
pub(super) fn site_name<'a>(tabs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Option<String> {
    let mut site: Option<String> = None;
    let mut titles = Vec::new();
    for (url, title) in tabs {
        let (scheme, rest) = url.split_once("://")?;
        if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
            return None;
        }
        let host = &rest[..rest.find(['/', '?', '#', ':']).unwrap_or(rest.len())];
        let host = host.to_ascii_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
        if host.is_empty() || site.as_ref().is_some_and(|site| *site != host) {
            return None;
        }
        site = Some(host);
        titles.push(title);
    }
    let site = site?;
    // A part of a title between the separators titles put round a site's name that spells one
    // of the host's labels but its last, "GitHub" for github.com.
    let letters = |text: &str| -> String {
        text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
    };
    let labels: Vec<&str> = site.split('.').collect();
    let labels = &labels[..labels.len().saturating_sub(1)];
    let named = (titles.iter())
        .flat_map(|title| title.split(['-', '|', '·', '—', '–', ':', '•']))
        .map(str::trim)
        .find(|part| {
            !part.is_empty() && labels.iter().any(|label| letters(part) == letters(label))
        });
    Some(named.map_or(site.clone(), str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::site_name;

    /// A folder made from one site's tabs takes the site's name as its titles write it, else its
    /// host; tabs on two sites, or a file, leave it New Folder.
    #[test]
    fn a_folder_of_one_sites_tabs_is_named_for_it() {
        let github = [
            ("https://github.com/pulls", "Pull requests · GitHub"),
            ("https://www.github.com/zed-industries/zed", "GitHub - zed-industries/zed: Code"),
        ];
        assert_eq!(site_name(github).as_deref(), Some("GitHub"));
        let docs = [("https://docs.github.com/en", "GitHub Docs")];
        assert_eq!(site_name(docs).as_deref(), Some("docs.github.com"));
        let example = [("https://example.com/", "Example Domain")];
        assert_eq!(site_name(example).as_deref(), Some("example.com"));
        let youtube = [
            ("https://www.youtube.com/watch?v=1", "A talk - YouTube"),
            ("https://youtube.com/", "YouTube"),
        ];
        assert_eq!(site_name(youtube).as_deref(), Some("YouTube"));
        let two = [("https://github.com/", "GitHub"), ("https://example.com/", "Example")];
        assert_eq!(site_name(two), None);
        assert_eq!(site_name([("file:///tmp/notes.txt", "notes.txt")]), None);
        assert_eq!(site_name([]), None);
    }
}
