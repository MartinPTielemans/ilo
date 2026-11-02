//! Naming a Space, a tab or a folder in a field in place of its name, a new folder as
//! it's made; and opening the pickers for a Space's or a tab's icon and a Space's theme.

use crate::tokens::text;
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{App, Context, Focusable, Point, SharedString, Window, div, point, prelude::*, px};

use super::{
    undo::{Prop, Value},
    CancelName, FAVICON_SIZE, FAVICON_X, FOLDER_INDENT, FORM_PLACEHOLDER, FORM_TEXT, KeepName,
    NEW_FOLDER, Naming, PLATE_INSET, Pending, Renaming, SPACE_ICON_SLOT, SPACE_ICON_X, Shell,
    ShellHandle,
};
use crate::{
    icon_picker::{self, Picked, Subject},
    space_icon::has_icon,
    text_input::TextInput,
    theme_picker,
};

/// How long a picker waits for the name row it points at to stop moving.
const PICKER_WAIT: Duration = Duration::from_secs(2);

impl Shell {
    /// Shows the Space with its name in a field, all selected, to type a new one over.
    pub(super) fn rename_space(&mut self, space: u64, cx: &mut Context<Self>) {
        // A private window's Space is named for what it is.
        if self.private {
            return;
        }
        let Some(name) = self.spaces.iter().find(|each| each.id == space).map(|each| &each.name)
        else {
            return;
        };
        let name = name.clone();
        self.show_space(space, cx);
        // The field is in the sidebar, which the Library covers.
        self.set_library(false, cx);
        // The form names a Space it made.
        if self.form.as_ref().is_some_and(|form| form.space == space) {
            return;
        }
        log!("browser: rename space {space}");
        let name = cx.new(|cx| {
            TextInput::new("Space name", cx)
                .placeholder_color(self.palette.text(FORM_PLACEHOLDER))
                .with_text(name)
        });
        self.renaming = Some(Renaming {
            naming: Naming::Space(space),
            name,
            in_library: false,
            _blur: None,
        });
        // As for the form.
        self.dock(cx);
        self.browser.focus_ui(cx);
        self.focus_renaming = true;
        cx.notify();
    }

    /// The first Space's card's name in its field, for the preview.
    pub fn preview_rename_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(space) = self.spaces.first().map(|space| space.id) {
            self.rename_space_in_card(space, window, cx);
        }
    }

    /// Puts the Space's name in a field in its card in the Library, all selected, to type a new one
    /// over, and leaves the Library open and the Space where it was, as Arc does. It keeps the
    /// name in another card's field first.
    pub(super) fn rename_space_in_card(
        &mut self,
        space: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self.spaces.iter().find(|each| each.id == space).map(|each| &each.name)
        else {
            return;
        };
        let name = name.clone();
        self.keep_name(window, cx);
        // check.sh waits for this line.
        log!("browser: rename space {space} in library");
        let name = cx.new(|cx| {
            TextInput::new("Space name", cx)
                .placeholder_color(self.palette.text(FORM_PLACEHOLDER))
                .with_text(name)
        });
        self.renaming = Some(Renaming {
            naming: Naming::Space(space),
            name,
            in_library: true,
            _blur: None,
        });
        self.browser.focus_ui(cx);
        self.focus_renaming = true;
        cx.notify();
    }

    /// Shows the tab's title in a field, all selected, to type a name over.
    pub(super) fn rename_tab(&mut self, tab: u64, cx: &mut Context<Self>) {
        log!("browser: rename tab {tab}");
        let title = self.title(tab);
        let name = cx.new(|cx| {
            TextInput::new("Tab name", cx)
                .placeholder_color(self.palette.text(FORM_PLACEHOLDER))
                .with_text(title)
        });
        self.renaming = Some(Renaming {
            naming: Naming::Tab(tab),
            name,
            in_library: false,
            _blur: None,
        });
        // As for a Space's name.
        self.dock(cx);
        self.browser.focus_ui(cx);
        self.focus_renaming = true;
        cx.notify();
    }

    /// Shows the pinned tab's pinned URL in a field in its row, all selected, to type another over,
    /// as Rename Tab shows its title.
    pub(super) fn edit_pinned_url(&mut self, tab: u64, cx: &mut Context<Self>) {
        log!("browser: edit pinned url {tab}");
        let url = self.browser.pinned_url(tab);
        let field = cx.new(|cx| {
            TextInput::new("Pinned URL", cx)
                .placeholder_color(self.palette.text(FORM_PLACEHOLDER))
                .with_text(url.into())
        });
        self.renaming = Some(Renaming {
            naming: Naming::PinnedUrl(tab),
            name: field,
            in_library: false,
            _blur: None,
        });
        self.dock(cx);
        self.browser.focus_ui(cx);
        self.focus_renaming = true;
        cx.notify();
    }

    /// Shows the icon picker under the tab's favicon, once its row has stopped moving, with the
    /// sidebar docked.
    pub(super) fn change_icon(&mut self, tab: u64, cx: &mut Context<Self>) {
        self.pick_icon_from_row(Subject::Tab(tab), cx);
    }

    /// Shows the icon picker under the folder's folder, once its row has stopped moving, with the
    /// sidebar docked: Arc's folder › Change Icon….
    pub(super) fn change_folder_icon(&mut self, folder: u64, cx: &mut Context<Self>) {
        self.pick_icon_from_row(Subject::Folder(folder), cx);
    }

    fn pick_icon_from_row(&mut self, subject: Subject, cx: &mut Context<Self>) {
        if !self.dock(cx) && self.sidebar_hidden {
            self.toggle_sidebar(cx);
        }
        self.picker_pending = Some((Pending::Icon(subject), Instant::now()));
        self.tab_row_drawn.set(None);
        self.name_row_before = None;
        cx.notify();
    }

    /// Shows the Space, and the icon picker under its name once the name row has stopped moving.
    pub(super) fn change_space_icon(&mut self, space: u64, cx: &mut Context<Self>) {
        self.open_picker(space, Pending::Icon(Subject::Space(space)), cx);
    }

    /// Shows the Space, and the theme picker beside its name row once the row has stopped
    /// moving: Arc's Edit Theme Color…, whose picker opened from there isn't recorded.
    pub(super) fn edit_space_theme(&mut self, space: u64, cx: &mut Context<Self>) {
        self.open_picker(space, Pending::Theme(space), cx);
    }

    /// Hangs the icon picker from the icon slot of `space`'s card in the Library, which stays open
    /// with the Space shown where it was, so several can be changed in a row, as in Arc (#1074).
    /// The press closed a picker showing for another card, which on Linux hears so only after the
    /// click, so it opens from this card once that one has closed.
    pub(super) fn change_space_icon_in_card(&mut self, space: u64, cx: &mut Context<Self>) {
        let showing = icon_picker::open_for(cx);
        if let Some(other) = showing.filter(|&other| other != Subject::Space(space)) {
            let picker = icon_picker::entity(cx);
            self.card_move = picker.map(|picker| {
                cx.observe(&picker, move |shell, _, cx| match icon_picker::open_for(cx) {
                    Some(now) if now == other => {}
                    None => {
                        shell.card_move = None;
                        shell.change_space_icon_in_card(space, cx);
                    }
                    Some(_) => shell.card_move = None,
                })
            });
            return;
        }
        let Some(slot) = self.slot_frames.borrow().get(&space).copied() else {
            return;
        };
        let anchor = point(f32::from(slot.center().x), f32::from(slot.center().y));
        self.show_space_icon_picker(space, anchor, cx);
        self.light_card_while_picking(space, cx);
    }

    /// Opens the theme picker beside the brush of `space`'s card in the Library, which stays open
    /// under it, the card's gradient taking the theme as it's made (#1074), once a picker showing
    /// for another card has closed, as the icon picker does.
    pub(super) fn edit_space_theme_in_card(&mut self, space: u64, cx: &mut Context<Self>) {
        if let Some(other) = theme_picker::open_for(cx).filter(|&other| other != space) {
            let picker = theme_picker::entity(cx);
            self.card_move = picker.map(|picker| {
                cx.observe(&picker, move |shell, _, cx| match theme_picker::open_for(cx) {
                    Some(now) if now == other => {}
                    None => {
                        shell.card_move = None;
                        shell.edit_space_theme_in_card(space, cx);
                    }
                    Some(_) => shell.card_move = None,
                })
            });
            return;
        }
        let Some(brush) = self.brush_frames.borrow().get(&space).copied() else {
            return;
        };
        let anchor = point(f32::from(brush.right()), f32::from(brush.center().y));
        self.show_theme_picker(space, anchor, cx);
        self.light_card_while_picking(space, cx);
    }

    /// Keeps `space`'s card's field lit while a picker shows for it, as the pointer leaves the
    /// field for the picker.
    fn light_card_while_picking(&mut self, space: u64, cx: &mut Context<Self>) {
        let showing = move |cx: &App| {
            icon_picker::open_for(cx) == Some(Subject::Space(space))
                || theme_picker::open_for(cx) == Some(space)
        };
        if !showing(cx) {
            return;
        }
        let watch = move |shell: &mut Self, cx: &mut Context<Self>| {
            if !showing(cx) {
                shell.card_picker = None;
                cx.notify();
            }
        };
        let watch = if icon_picker::open_for(cx) == Some(Subject::Space(space)) {
            let picker = icon_picker::entity(cx);
            picker.map(|picker| cx.observe(&picker, move |shell, _, cx| watch(shell, cx)))
        } else {
            let picker = theme_picker::entity(cx);
            picker.map(|picker| cx.observe(&picker, move |shell, _, cx| watch(shell, cx)))
        };
        self.card_picker = watch.map(|watch| (space, watch));
        cx.notify();
    }

    /// Whether `space`'s card's field is held lit for a picker shown from it.
    pub(super) fn card_picking(&self, space: u64) -> bool {
        self.card_picker.as_ref().is_some_and(|(picking, _)| *picking == space)
    }

    fn open_picker(&mut self, space: u64, picker: Pending, cx: &mut Context<Self>) {
        if !self.spaces.iter().any(|each| each.id == space) {
            return;
        }
        self.show_space(space, cx);
        // The picker hangs under the sidebar's name row, which the Library covers.
        self.set_library(false, cx);
        // The form names a Space it made, with no name row.
        if self.form.as_ref().is_some_and(|form| form.space == space) {
            return;
        }
        if !self.dock(cx) && self.sidebar_hidden {
            self.toggle_sidebar(cx);
        }
        self.picker_pending = Some((picker, Instant::now()));
        self.name_row_drawn.set(None);
        self.name_row_before = None;
        cx.notify();
    }

    /// Opens the picker asked for once the row it points at has drawn where it drew the frame
    /// before, and the Library has gone, as the sidebar, the Library or a Space's slide may be
    /// moving it. A Space's icon picker points at where its icon goes in the name row, a tab's at
    /// its favicon, and the theme picker at the name row's end. It gives up if the row doesn't
    /// settle in two seconds, such as in a full-screen page, which has no sidebar.
    pub(super) fn open_picker_when_settled(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((picker, asked)) = self.picker_pending else {
            return;
        };
        let (drawn, here) = match picker {
            Pending::Icon(Subject::Space(space)) | Pending::Theme(space) => {
                (self.name_row_drawn.get(), space == self.space)
            }
            Pending::Icon(Subject::Tab(_) | Subject::Folder(_)) => {
                (self.tab_row_drawn.get(), true)
            }
        };
        // Not before the Library has gone and the sidebar has come in, as a frame of either can
        // leave the row where it was.
        let still = self.library.is_none() && self.sidebar_motion.is_none();
        let settled = still && drawn.is_some() && drawn == self.name_row_before && here;
        self.name_row_before = drawn;
        if !settled {
            if asked.elapsed() < PICKER_WAIT {
                window.request_animation_frame();
            } else {
                self.picker_pending = None;
            }
            return;
        }
        self.picker_pending = None;
        // The picker shows over the key window, which may have changed while the row moved.
        let key = cx.try_global::<ShellHandle>().is_some_and(|key| key.0 == cx.weak_entity());
        if !key {
            return;
        }
        let Some(row) = drawn else {
            return;
        };
        let subject = match picker {
            Pending::Icon(Subject::Space(space)) => {
                let anchor = point(
                    f32::from(row.left()) + SPACE_ICON_X + SPACE_ICON_SLOT / 2.,
                    f32::from(row.center().y),
                );
                let iconless =
                    self.spaces.iter().any(|each| each.id == space && !has_icon(&each.icon));
                self.show_space_icon_picker(space, anchor, cx);
                if iconless {
                    self.hold_icon_place(space, cx);
                }
                return;
            }
            Pending::Icon(subject) => subject,
            Pending::Theme(space) => {
                let anchor = point(f32::from(row.right()), f32::from(row.center().y));
                return self.show_theme_picker(space, anchor, cx);
            }
        };
        let shell = cx.weak_entity();
        let (icon, anchor, picked) = match subject {
            // Shown above.
            Subject::Space(_) => return,
            Subject::Tab(tab) => {
                // The favicon's middle: a tile's, else in from the row by the folders the tab is
                // in.
                let x = match self.favorites.contains(&tab) {
                    true => f32::from(row.center().x),
                    false => {
                        f32::from(row.left())
                            + FAVICON_X
                            + self.depth(tab) as f32 * FOLDER_INDENT
                            + FAVICON_SIZE / 2.
                    }
                };
                let anchor = point(x, f32::from(row.center().y));
                let picked: Picked = Rc::new(move |icon, cx| {
                    shell
                        .update(cx, |shell, cx| {
                            shell.change(Prop::TabIcon(tab), Value::Text(icon.to_string()), cx)
                        })
                        .ok();
                });
                (self.page(tab).icon, anchor, picked)
            }
            Subject::Folder(folder) => {
                // The folder's middle, in from the row by the folders it's in.
                let x = f32::from(row.left())
                    + FAVICON_X
                    + self.depth(folder) as f32 * FOLDER_INDENT
                    + FAVICON_SIZE / 2.;
                let anchor = point(x, f32::from(row.center().y));
                let picked: Picked = Rc::new(move |icon, cx| {
                    shell
                        .update(cx, |shell, cx| {
                            let icon = Value::Text(icon.to_string());
                            shell.change(Prop::FolderIcon(folder), icon, cx)
                        })
                        .ok();
                });
                (self.folder_icon(folder), anchor, picked)
            }
        };
        // A card showing would stay under the picker, as rows' cards wait while it shows.
        self.browser.hover_card(crate::hover_card::Hover::Close, cx);
        icon_picker::show(subject, icon, anchor, picked, cx);
    }

    /// Shows the icon picker for `space`, its arrow at `anchor` in the window, giving the Space
    /// the icon picked.
    fn show_space_icon_picker(&mut self, space: u64, anchor: Point<f32>, cx: &mut Context<Self>) {
        let Some(icon) = self.spaces.iter().find(|each| each.id == space).map(|each| &each.icon)
        else {
            return;
        };
        let shell = cx.weak_entity();
        let picked: Picked = Rc::new(move |icon, cx| {
            shell
                .update(cx, |shell, cx| {
                    shell.change(Prop::SpaceIcon(space), Value::Text(icon.to_string()), cx)
                })
                .ok();
        });
        // As for a tab's or a folder's.
        self.browser.hover_card(crate::hover_card::Hover::Close, cx);
        icon_picker::show(Subject::Space(space), icon.clone(), anchor, picked, cx);
    }

    /// Holds the icon's place in `space`'s name row, which has no icon, with the placeholder, the
    /// name after it, while the icon picker shows for it (Arc), so the arrow points at where the
    /// icon will go.
    pub(super) fn hold_icon_place(&mut self, space: u64, cx: &mut Context<Self>) {
        let Some(picker) = icon_picker::entity(cx) else {
            return;
        };
        let watch = cx.observe(&picker, move |shell, picker, cx| {
            if picker.read(cx).picking().map(|(subject, _)| subject) != Some(Subject::Space(space))
            {
                shell.icon_placeholder = None;
                cx.notify();
            }
        });
        self.icon_placeholder = Some((space, watch));
        cx.notify();
    }

    /// Whether the shown Space's name row holds the icon's place with the placeholder: while the
    /// icon picker shows for it, until it has an icon.
    pub(super) fn holds_icon_place(&self) -> bool {
        self.icon_placeholder.as_ref().is_some_and(|(space, _)| *space == self.space)
            && self.shown_space().is_some_and(|space| !has_icon(&space.icon))
    }

    /// Shows the folder's name in a field, all selected, to type a new one over.
    pub(super) fn rename_folder(&mut self, folder: u64, cx: &mut Context<Self>) {
        let name = self.folder_name(folder);
        self.name_folder(folder, name, cx);
    }

    pub(super) fn name_folder(&mut self, folder: u64, name: SharedString, cx: &mut Context<Self>) {
        let name = cx.new(|cx| {
            TextInput::new("Folder name", cx)
                .placeholder_color(self.palette.text(FORM_PLACEHOLDER))
                .with_text(name)
        });
        self.renaming = Some(Renaming {
            naming: Naming::Folder(folder),
            name,
            in_library: false,
            _blur: None,
        });
        // As for a Space's name.
        self.dock(cx);
        self.browser.focus_ui(cx);
        self.focus_renaming = true;
        cx.notify();
    }

    /// Makes a folder at the bottom of the pinned tabs of the Space shown, with its name in a field
    /// to type over.
    pub(super) fn new_folder(&mut self, cx: &mut Context<Self>) {
        let name = SharedString::from(NEW_FOLDER);
        let folder = self.browser.create_folder(self.space, &name, 0, usize::MAX, cx);
        if folder != 0 {
            self.name_folder(folder, name, cx);
        }
    }

    /// Puts the new profile's name in the New Space form's Profile row, all selected, to type
    /// another over, as Finder's New Folder does.
    pub(super) fn name_profile(
        &mut self,
        profile: u64,
        name: SharedString,
        cx: &mut Context<Self>,
    ) {
        let name = cx.new(|cx| {
            TextInput::new(name.clone(), cx)
                .placeholder_color(self.palette.text(FORM_PLACEHOLDER))
                .with_text(name)
        });
        self.renaming = Some(Renaming {
            naming: Naming::Profile(profile),
            name,
            in_library: false,
            _blur: None,
        });
        self.browser.focus_ui(cx);
        self.focus_renaming = true;
        cx.notify();
    }

    /// The folder's name in the Space shown.
    /// The emoji the user gave `folder`, else empty.
    pub(super) fn folder_icon(&self, folder: u64) -> SharedString {
        let space = self.spaces.iter().find(|space| space.is_folder(folder));
        space.and_then(|space| space.folders.icons.get(&folder).cloned()).unwrap_or_default()
    }

    pub(super) fn folder_name(&self, folder: u64) -> SharedString {
        // Its own Space's, which isn't the one shown while it's dragged to another.
        let mut folders = self.spaces.iter().map(|space| &space.folders.folders);
        let folder = folders.find_map(|folders| folders.get(&folder));
        folder.map(|(name, _)| name.clone()).unwrap_or_default()
    }

    /// Renames the Space, the tab, the folder or the profile to what the field holds, and closes
    /// the field. A Space, a folder or a profile keeps its name rather than none, and a tab named
    /// nothing shows its page's title again.
    pub(super) fn keep_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        let name = renaming.name.read(cx).text().trim().to_string();
        match renaming.naming {
            Naming::Space(space) if !name.is_empty() => {
                self.change(Prop::SpaceName(space), Value::Text(name), cx)
            }
            Naming::Space(_) => {}
            // Left as it was, the title stays the page's, which changes with it.
            Naming::Tab(tab) if *name == *self.title(tab) => {}
            Naming::Tab(tab) => self.change(Prop::TabName(tab), Value::Text(name), cx),
            // Kept empty, it stays where it went back to.
            Naming::PinnedUrl(tab) => {
                if let Some(url) = self.browser.typed_url(&name).filter(|_| !name.is_empty()) {
                    self.change(Prop::PinnedUrl(tab), Value::Text(url), cx)
                }
            }
            Naming::Folder(folder) if !name.is_empty() => {
                self.change(Prop::FolderName(folder), Value::Text(name), cx)
            }
            Naming::Folder(_) => {}
            Naming::Profile(profile) => {
                let listed =
                    self.profiles.iter().any(|(id, listed)| *id == profile && **listed == name);
                if !name.is_empty() && !listed {
                    // check.sh waits for this line.
                    log!("browser: name profile {profile} {name}");
                    self.browser.rename_profile(profile, &name, cx);
                }
            }
        }
        self.leave_name(&renaming, window, cx);
    }

    pub(super) fn cancel_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(renaming) = self.renaming.take() {
            log!("browser: rename cancelled");
            self.leave_name(&renaming, window, cx);
        }
    }

    /// Gives the keyboard back from the closed field: to the New Space form's name from its
    /// Profile row, unless a click put it elsewhere, else to the window.
    fn leave_name(&mut self, renaming: &Renaming, window: &mut Window, cx: &mut Context<Self>) {
        match renaming.naming {
            Naming::Profile(_) => {
                self.focus_form = renaming.name.focus_handle(cx).is_focused(window);
            }
            _ => window.focus(&self.focus_handle, cx),
        }
        cx.notify();
    }

    /// The field that renames a tab, in place of its title.
    pub(super) fn naming_field(&self, renaming: &Renaming, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .pr(px(PLATE_INSET))
            .key_context("Naming")
            .on_action(cx.listener(|shell, _: &KeepName, window, cx| shell.keep_name(window, cx)))
            .on_action(
                cx.listener(|shell, _: &CancelName, window, cx| shell.cancel_name(window, cx)),
            )
            .text_size(px(text::BODY))
            .text_color(self.palette.text(FORM_TEXT))
            .child(renaming.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Entity, TestAppContext, VisualTestContext};

    use crate::shell::{Shell, recorder::Recorder};

    /// A window showing the Space Home with its name a field.
    fn renaming(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        cx.update(|cx| {
            crate::shell::commands::bind_keys(cx);
            crate::text_input::bind_keys(cx);
        });
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.update(|window, _| window.activate_window());
        shell.update(cx, |shell, cx| {
            shell.set_spaces(vec![(1, "Home".into(), None, 1, "".into(), "".into(), "".into())], cx);
            shell.set_space(1, cx);
            shell.rename_space(1, cx);
        });
        cx.run_until_parked();
        (shell, cx, host)
    }

    /// Another window taking the keyboard a moment, as ilo's menu does for the field's own menu,
    /// leaves the name a field with the keyboard, so escape still cancels once it's back.
    #[gpui::test]
    fn the_name_stays_a_field_while_another_window_has_the_keyboard(cx: &mut TestAppContext) {
        let (shell, cx, host) = renaming(cx);
        cx.simulate_input("gone");
        cx.deactivate_window();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.renaming.is_some()), "the field closed");
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        assert!(shell.read_with(cx, |shell, _| shell.renaming.is_none()), "escape didn't cancel");
        assert!(host.take_edits().is_empty(), "the Space was renamed");
    }

    /// Leaving the field for something else in its window keeps the name typed.
    #[gpui::test]
    fn leaving_the_name_for_the_sidebar_keeps_it(cx: &mut TestAppContext) {
        let (shell, cx, host) = renaming(cx);
        cx.simulate_input("Work");
        shell.update_in(cx, |shell, window, cx| window.focus(&shell.focus_handle, cx));
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.renaming.is_none()), "the field stayed");
        assert_eq!(host.take_edits(), ["rename space 1 Work"]);
    }

    /// Edit… puts the pinned URL in the row's field: ↩ makes what's typed the address it goes back
    /// to, as the address bar reads it, and ⌘Z gives the old one back. Kept empty, nothing changes.
    #[gpui::test]
    fn an_edited_pinned_url_is_kept_and_undone(cx: &mut TestAppContext) {
        use crate::shell::{Command, Folders};

        let (shell, cx, host) = renaming(cx);
        cx.simulate_keystrokes("escape");
        host.pinned_urls.borrow_mut().insert(2, "https://old.test/".into());
        let edit = |cx: &mut VisualTestContext| {
            shell.update(cx, |shell, cx| shell.edit_pinned_url(2, cx));
            cx.run_until_parked();
        };
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![2], vec![3], Vec::new(), Folders::default(), cx);
            shell.set_selected(3, cx);
        });
        edit(cx);
        cx.simulate_input("new.test");
        cx.simulate_keystrokes("enter");
        assert!(shell.read_with(cx, |shell, _| shell.renaming.is_none()), "the field stayed");
        assert_eq!(host.take_edits(), ["pinned url 2 https://new.test/"]);
        shell.update(cx, |shell, cx| shell.run(Command::Undo, "ui", cx));
        assert_eq!(host.take_edits(), ["pinned url 2 https://old.test/"]);
        edit(cx);
        cx.simulate_keystrokes("backspace enter");
        assert!(host.take_edits().is_empty(), "an empty field changed the pinned URL");
    }
}
