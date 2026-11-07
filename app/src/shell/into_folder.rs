//! Export "name" into Folder ›, in a Space's menu (Arc 1.171): the Space becomes a folder at the
//! bottom of another Space's pinned tabs, with its name and emoji and its pinned tabs and folders in
//! order, its Today tabs go to the top of that Space's Today, the window shows them there, and the
//! Space goes. One ⌘Z brings it back as it was (docs/ux-spec.md › Spaces › Renaming and deleting).

use gpui::{Context, SharedString};

use super::{
    Shell,
    undo::{Edit, What},
};

impl Shell {
    /// The Spaces the menu lists for `space` to go into: every one of the user's, each with
    /// whether it can. Not itself, as Arc's lists it dimmed, and not another profile's, where its
    /// tabs would go as new ones with nothing for ⌘Z to give back (ours).
    pub(super) fn folder_targets(&self, space: u64) -> Vec<(u64, SharedString, bool)> {
        let profile = self.spaces.iter().find(|each| each.id == space);
        let profile = profile.map(|each| each.profile);
        (self.spaces.iter())
            .filter(|each| each.agent.is_empty())
            .map(|each| {
                let can = each.id != space && Some(each.profile) == profile;
                (each.id, each.name.clone(), can)
            })
            .collect()
    }

    /// Makes `space` a folder in `into`, for ⌘Z to take back.
    pub(super) fn export_into_folder(&mut self, space: u64, into: u64, cx: &mut Context<Self>) {
        let edit = Edit::Export { space, into }.apply(self, cx);
        self.keep(What::EXPORT_INTO_FOLDER, edit, cx);
    }

    /// Does what Export into Folder does, and returns the steps that put it all back: the Space
    /// first, then each row in its place, then the folder going, as `Edit::Many` orders them.
    pub(super) fn export_now(
        &mut self,
        space: u64,
        into: u64,
        cx: &mut Context<Self>,
    ) -> Option<Edit> {
        let from = self.spaces.iter().find(|each| each.id == space)?;
        let can = self
            .folder_targets(space)
            .iter()
            .any(|&(each, _, can)| each == into && can);
        if !can || !from.agent.is_empty() {
            return None;
        }
        let name = from.name.clone();
        let icon = from.icon.clone();
        let pinned = from.children(0);
        let today: Vec<u64> = (from.today.iter().copied())
            .filter(|&tab| !from.later(tab))
            .collect();
        let mut back = vec![Edit::UndoDeleteSpace(space)];
        back.extend(
            pinned
                .iter()
                .chain(&today)
                .filter_map(|&row| self.back_from(row)),
        );
        // There first, so the Space goes from a window that no longer shows it, and the user sees
        // where its rows went.
        self.show_space(into, cx);
        let folder = self.browser.create_folder(into, &name, 0, usize::MAX, cx);
        if folder == 0 {
            return None;
        }
        // A folder's icon is an emoji; a Space's glyph has no place there.
        if !icon.is_empty() && !icon.starts_with("glyph:") {
            self.browser.set_folder_icon(folder, &icon, cx);
        }
        // Each at the top, last first, so they keep their order.
        for &row in pinned.iter().rev() {
            self.send_row(row, into, cx);
            match self.is_folder_anywhere(row) {
                true => self.browser.move_folder(row, folder, 0, cx),
                false => self.browser.move_tab(row, true, folder, 0, cx),
            }
        }
        for &tab in today.iter().rev() {
            self.send_row(tab, into, cx);
            self.browser.move_tab(tab, false, 0, 0, cx);
        }
        if !self.browser.delete_space(space, cx) {
            return None;
        }
        // check.sh waits for this line.
        log!("browser: space {space} exported into folder {folder} in {into}");
        back.push(Edit::Delete(folder));
        Some(Edit::Many(back))
    }

    /// The step that puts the row `row` back where it is: with its split view, as one.
    fn back_from(&self, row: u64) -> Option<Edit> {
        let Some((tabs, look)) = self.split_of(row) else {
            return self.put_back(row);
        };
        let spot = self.spot_of(row)?;
        Some(Edit::MoveSplit { tabs, look, spot })
    }

    /// Moves the row `row` to `space`: a folder with what's in it, a split view whole.
    fn send_row(&mut self, row: u64, space: u64, cx: &mut Context<Self>) {
        if self.is_folder_anywhere(row) {
            return self.browser.move_folder_to_space(row, space, cx);
        }
        match self.split_of(row) {
            Some((tabs, look)) => self.move_split(&tabs, &look, space, cx),
            None => self.browser.move_to_space(row, space, cx),
        }
    }
}
