//! The New Space form, which a new Space's page shows in place of its list: its name,
//! its profile and its theme (docs/ux-spec.md › Spaces).

use crate::tokens::text;
use web_time::Instant;

use gpui::{
    AnyElement, Context, FontWeight, MouseButton, MouseDownEvent, Pixels, Point, Window,
    accesskit::{Action, HasPopup, Role},
    div, point,
    prelude::*,
    px, svg,
};

use super::access::Accessible;
use super::{
    CANCEL_BOTTOM, CANCEL_HEIGHT, CREATE_BOTTOM, CREATE_UNNAMED, CreateSpace, FORM_CHEVRON,
    FORM_CHEVRON_GAP, FORM_FIELD_TOP, FORM_GLYPH, FORM_GLYPH_COLOR, FORM_GLYPH_X, FORM_INSET,
    FORM_LABEL_X, FORM_LEAVE, FORM_LEAVE_FIELD, FORM_LEAVE_ROWS, FORM_LEAVE_TITLE, FORM_PILL,
    FORM_PILL_HEIGHT, FORM_PILL_LEFT, FORM_PILL_RIGHT, FORM_PILL_X, FORM_PLACEHOLDER, FORM_PLATE,
    FORM_PLATE_HEIGHT, FORM_PROFILE_TOP, FORM_RADIUS, FORM_SINK, FORM_TEXT, FORM_TEXT_X,
    FORM_THEME_TOP, FORM_TITLE_HEIGHT, FORM_TITLE_TOP, HOVERED, Naming, SECONDARY, Shell,
    SpaceForm,
};
use crate::{text_input::TextInput, theme_picker};

impl Shell {
    /// Makes a Space and shows it with the form for naming it, or shows the form already open.
    pub(super) fn new_space(&mut self, cx: &mut Context<Self>) {
        self.new_space_with(None, cx)
    }

    /// New Space…, and with `tab` Move to › New Space… in a tab's menu, which moves the tab into
    /// the Space once the form makes it, so Cancel leaves the tab where it was.
    pub(super) fn new_space_with(&mut self, tab: Option<u64>, cx: &mut Context<Self>) {
        self.new_space_from(tab, None, cx)
    }

    /// New Space's form, its Space in `profile`, else in the profile of the Space it's made from.
    pub(super) fn new_space_from(
        &mut self,
        tab: Option<u64>,
        profile: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        if self.one_space() {
            return;
        }
        let space = match &mut self.form {
            Some(form) => {
                form.tab = tab.or(form.tab);
                form.space
            }
            None => {
                // Arc's name for a Space the user doesn't name.
                let name = format!("Space {}", self.spaces.len() + 1);
                // In the profile of the Space it's made from, and right after it (Arc), but
                // after the others from an agent's.
                let shown = self.shown_space();
                let profile = profile.unwrap_or_else(|| shown.map_or(0, |space| space.profile));
                let after = shown
                    .filter(|space| space.agent.is_empty())
                    .map_or(0, |space| space.id);
                let space = self.browser.create_space(&name, profile, after, cx);
                let name = cx.new(|cx| {
                    TextInput::new("Space name", cx)
                        .placeholder_color(self.palette.text(FORM_PLACEHOLDER))
                });
                self.form = Some(SpaceForm {
                    space,
                    from: self.space,
                    name,
                    tab,
                });
                space
            }
        };
        if space == self.space {
            self.focus_form(cx);
        } else {
            self.show_space(space, cx);
        }
    }

    /// The form, if the window shows its Space.
    pub(super) fn form(&self) -> Option<&SpaceForm> {
        self.form.as_ref().filter(|form| form.space == self.space)
    }

    pub(super) fn focus_form(&mut self, cx: &mut Context<Self>) {
        // The panel never takes the keyboard, and goes when the pointer leaves it.
        self.dock(cx);
        self.browser.focus_ui(cx);
        self.focus_form = true;
        cx.notify();
    }

    /// Names the form's Space, if it has a name, and closes the form.
    fn create_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = self.form() else {
            return;
        };
        let name = form.name.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        // check.sh waits for these lines.
        log!("browser: create space {} {name}", form.space);
        let (space, tab) = (form.space, form.tab);
        self.browser.rename_space(space, &name, cx);
        if let Some(tab) = tab {
            self.move_to_space(tab, space, false, cx);
        }
        self.leaving_form = self.form.take().map(|form| (form, Instant::now()));
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// Shows the Space the form was opened from and deletes the form's.
    pub(super) fn cancel_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(form) = self.close_form(window, cx) {
            self.browser.delete_space(form.space, cx);
        }
    }

    /// Shows the Space the form was opened from, and returns the form, whose Space is left for
    /// the caller to delete.
    pub(super) fn close_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<SpaceForm> {
        let form = self.form.take_if(|form| form.space == self.space)?;
        log!("browser: cancel space {}", form.space);
        self.show_space(form.from, cx);
        window.focus(&self.focus_handle, cx);
        cx.notify();
        Some(form)
    }

    /// The form shown, or the one a swipe cancelled, and where its page is while fingers or a
    /// settle move it off its place.
    pub(super) fn paged_form(&self) -> Option<(&SpaceForm, f32)> {
        let form = self.form().or(self.form_going.as_ref())?;
        let (_, x) = self
            .pages()
            .into_iter()
            .find(|&(space, _)| space == form.space)?;
        (x != 0.).then_some((form, x))
    }

    /// The URL field and the favorites on the page of the Space beside the form's, which comes in
    /// or goes out with it as its rows do. None with no page beside, as the form rubber-bands.
    pub(super) fn beside_form(
        &self,
        form: u64,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let (_, x) = self.pages().into_iter().find(|&(space, _)| space != form)?;
        let page = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(x))
            .w(px(self.width()));
        let page = page
            .child(self.url_field(cx))
            .children(self.scroll_edge_line())
            .children(self.tiles(cx));
        Some(div().absolute().size_full().overflow_hidden().child(page))
    }

    /// The Create a Space form, in place of the URL field and the Space's tabs, and as it leaves,
    /// `left` seconds after Create.
    pub(super) fn space_form(
        &self,
        form: &SpaceForm,
        left: Option<f32>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let named = !form.name.read(cx).text().trim().is_empty();
        // How far a part has gone, from 0 to 1, if it starts to leave `delay` after Create. It
        // fades and sinks slowly at first, as Arc's do.
        let gone = |delay: f32| left.map_or(0., |left| ((left - delay) / FORM_LEAVE).clamp(0., 1.));
        let still = crate::motion::still(cx);
        let sinking = |delay: f32| {
            let gone = gone(delay);
            let sink = if still { 0. } else { FORM_SINK * gone * gone };
            (1. - gone.powi(3), sink)
        };
        let (title_opacity, title_sink) = sinking(FORM_LEAVE_TITLE);
        let (field_opacity, field_sink) = sinking(FORM_LEAVE_FIELD);
        let (rows_opacity, rows_sink) = sinking(FORM_LEAVE_ROWS);
        let buttons_opacity = 1. - gone(0.);
        let plate = |id: &'static str| {
            div()
                .id(id)
                .absolute()
                .left(px(FORM_INSET))
                .w(px(self.width() - 2. * FORM_INSET))
                .h(px(FORM_PLATE_HEIGHT))
                .flex()
                .items_center()
                .rounded(px(FORM_RADIUS))
                .block_mouse_except_scroll()
        };
        let a11y = self.a11y && left.is_none();
        let title = div()
            .id("space-form-title")
            .when(a11y, |title| {
                title.role(Role::Heading).aria_label("New Space")
            })
            .absolute()
            .top(px(FORM_TITLE_TOP + title_sink))
            .opacity(title_opacity)
            .w_full()
            .h(px(FORM_TITLE_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(text::HEADING))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(self.palette.text(FORM_TEXT))
            .child("New Space");
        let field = plate("space-name-field")
            .top(px(FORM_FIELD_TOP + field_sink))
            .opacity(field_opacity)
            .px(px(FORM_TEXT_X - FORM_INSET))
            .bg(self.palette.plate(FORM_PLATE))
            .text_size(px(text::BODY))
            .font_weight(FontWeight::MEDIUM)
            .text_color(self.palette.text(FORM_TEXT))
            .child(form.name.clone());
        // A row with a glyph and a label, whose menu a press shows while the form is up.
        let row = |id: &'static str, top: f32, glyph: &'static str, label: AnyElement| {
            plate(id)
                .top(px(top + rows_sink))
                .opacity(rows_opacity)
                .pl(px(FORM_GLYPH_X - FORM_GLYPH / 2. - FORM_INSET))
                .bg(self.palette.plate(FORM_PLATE))
                .text_size(px(text::BODY))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(self.palette.text(FORM_TEXT))
                .child(
                    svg()
                        .path(glyph)
                        .size(px(FORM_GLYPH))
                        .text_color(self.palette.glyph(FORM_GLYPH_COLOR)),
                )
                .child(
                    div()
                        .ml(px(FORM_LABEL_X - FORM_GLYPH_X - FORM_GLYPH / 2.))
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .child(label),
                )
        };
        let profile = (self.spaces.iter().find(|each| each.id == form.space))
            .and_then(|space| self.profiles.iter().find(|(id, _)| *id == space.profile))
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        let pill = div()
            .absolute()
            .left(px(FORM_PILL_X - FORM_INSET))
            .h(px(FORM_PILL_HEIGHT))
            .flex()
            .items_center()
            .pl(px(FORM_PILL_LEFT))
            .pr(px(FORM_PILL_RIGHT))
            .rounded(px(FORM_PILL_HEIGHT / 2.))
            .bg(self.palette.plate(FORM_PILL))
            .text_size(px(text::SMALL))
            .font_weight(FontWeight::SEMIBOLD)
            .child(profile.clone())
            .child(
                svg()
                    .path("icons/chevron-down.svg")
                    .ml(px(FORM_CHEVRON_GAP))
                    .size(px(FORM_CHEVRON))
                    .text_color(self.palette.text(FORM_TEXT)),
            );
        // A new profile's name, which the row holds in a field until it's kept.
        let naming = (self.renaming.as_ref())
            .filter(|renaming| matches!(renaming.naming, Naming::Profile(_)) && left.is_none())
            .map(|renaming| self.naming_field(renaming, cx).into_any_element());
        let menu = |show: fn(&mut Self, Point<Pixels>, &mut Context<Self>)| {
            cx.listener(move |shell, event: &MouseDownEvent, _, cx| show(shell, event.position, cx))
        };
        // A row VoiceOver presses as a click does, which opens `popup`.
        let popup_named = |row: gpui::Stateful<gpui::Div>, name: String, popup: HasPopup| {
            row.when(a11y, |row| {
                row.button_named(name)
                    .a11y_synthetic_children(move |builder| {
                        let node = builder.parent_node();
                        node.add_action(Action::Click);
                        node.set_has_popup(popup);
                    })
            })
        };
        // Under the pointer a row takes the list's hover fill, while a press still shows its menu.
        let profile_row = match naming {
            Some(field) => row("space-profile", FORM_PROFILE_TOP, "icons/user.svg", field),
            None => {
                let label = "Profile".into_any_element();
                let said = format!("Profile, {profile}");
                let row = row("space-profile", FORM_PROFILE_TOP, "icons/user.svg", label);
                popup_named(row, said, HasPopup::Menu)
                    .child(pill)
                    .when(left.is_none(), |row| {
                        row.hover(|style| style.bg(self.palette.plate(HOVERED)))
                            .on_mouse_down(MouseButton::Left, menu(Self::form_profile_menu))
                    })
            }
        };
        let theme_label = "Choose a Theme".into_any_element();
        let theme_row = row(
            "space-theme",
            FORM_THEME_TOP,
            "icons/paintbrush.svg",
            theme_label,
        );
        let theme_row = popup_named(theme_row, "Choose a Theme".into(), HasPopup::Dialog).when(
            left.is_none(),
            |row| {
                row.hover(|style| style.bg(self.palette.plate(HOVERED)))
                    .on_mouse_down(MouseButton::Left, menu(Self::form_theme_picker))
            },
        );
        let create = plate("create-space")
            .bottom(px(CREATE_BOTTOM))
            .justify_center()
            .opacity(buttons_opacity * if named { 1. } else { CREATE_UNNAMED })
            .bg(self.palette.accent)
            .text_size(px(text::BODY))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(self.palette.accent_label)
            .child("Create Space")
            // Unnamed, it's dimmed and its press does nothing, so VoiceOver says it's off.
            .when(a11y, |button| {
                button.button_named("Create Space").when(!named, |button| {
                    button.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
                })
            })
            .on_click(cx.listener(|shell, _, window, cx| shell.create_space(window, cx)));
        let cancel = div()
            .absolute()
            .bottom(px(CANCEL_BOTTOM))
            .w_full()
            .h(px(CANCEL_HEIGHT))
            .flex()
            .justify_center()
            .opacity(buttons_opacity)
            .child(
                div()
                    .id("cancel-space")
                    .when(a11y, |button| button.button_named("Cancel"))
                    .h_full()
                    .flex()
                    .items_center()
                    .block_mouse_except_scroll()
                    .text_size(px(text::SMALL))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.palette.text(SECONDARY))
                    // It brightens to the form's text under the pointer (ours: Arc's isn't
                    // recorded). Create Space doesn't change, as Arc's doesn't.
                    .hover(|style| style.text_color(self.palette.text(FORM_TEXT)))
                    .child("Cancel")
                    .on_click(cx.listener(|shell, _, window, cx| shell.cancel_space(window, cx))),
            );
        div()
            .id("space-form")
            .when(a11y, |form| form.role(Role::Form).aria_label("New Space"))
            .key_context("SpaceForm")
            .on_action(
                cx.listener(|shell, _: &CreateSpace, window, cx| shell.create_space(window, cx)),
            )
            .absolute()
            .top_0()
            .w(px(self.width()))
            .bottom_0()
            .child(title)
            .child(field)
            .child(profile_row)
            .child(theme_row)
            .child(create)
            .child(cancel)
    }

    /// The form's Profile row's menu: the profiles its Space can be in, the Space's on the pill's
    /// name, as a pop-up button's menu is, and a new one.
    fn form_profile_menu(&mut self, _: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(space) = self.form().map(|form| form.space) {
            let items = self.profile_items(space);
            let over = items.iter().position(|(entry, _)| entry.checked);
            let name = point(
                px(FORM_PILL_X + FORM_PILL_LEFT),
                px(FORM_PROFILE_TOP + (FORM_PLATE_HEIGHT - FORM_PILL_HEIGHT) / 2.),
            );
            self.show_menu(items, name, over, cx);
        }
    }

    /// The form's Choose a Theme row's picker, pointing at the row's end toward the page, as Arc's
    /// does. A
    /// press on the row while it shows closes it, as a click outside it does, without opening it
    /// again.
    fn form_theme_picker(&mut self, _: Point<Pixels>, cx: &mut Context<Self>) {
        if theme_picker::is_open(cx) {
            return;
        }
        if let Some(space) = self.form().map(|form| form.space) {
            let end = match self.right {
                true => self.column_x() + FORM_INSET,
                false => self.width() - FORM_INSET,
            };
            let anchor = point(end, FORM_THEME_TOP + FORM_PLATE_HEIGHT / 2.);
            self.show_theme_picker(space, theme_picker::At::Row(anchor), self.right, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{
        Entity, TestAppContext, VisualTestContext,
        accesskit::{Action, HasPopup, Node, NodeId, Role},
    };

    use crate::shell::{Command, Shell, SpaceEntry, recorder::Recorder};

    fn space(id: u64, name: &str) -> SpaceEntry {
        (
            id,
            name.to_string().into(),
            None,
            1,
            "".into(),
            "".into(),
            "".into(),
        )
    }

    /// A window showing the New Space form for Space 7, in the profile Default, with its name
    /// typed and the edits so far taken.
    fn form(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        cx.update(|cx| {
            crate::shell::commands::bind_keys(cx);
            crate::text_input::bind_keys(cx);
        });
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.update(|window, _| window.activate_window());
        shell.update(cx, |shell, cx| {
            shell.set_profiles(vec![(1, "Default".into())], cx);
            shell.set_spaces(vec![space(1, "Home")], cx);
            shell.set_space(1, cx);
            shell.run(Command::NewSpace, "menu", cx);
            // As the core reports the Space made and shown.
            shell.set_spaces(vec![space(1, "Home"), space(7, "Space 2")], cx);
            shell.set_space(7, cx);
        });
        cx.run_until_parked();
        cx.simulate_input("Trips");
        host.take_edits();
        (shell, cx, host)
    }

    /// New Profile in the form, as its Profile row's menu and Spaces › Change Profile give it.
    fn new_profile(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |shell, cx| {
            shell.run(Command::ChangeProfile(None), "menu", cx)
        });
        cx.run_until_parked();
    }

    fn naming(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
        shell.read_with(cx, |shell, _| shell.renaming.is_some())
    }

    /// The form's name has the keyboard: what's typed goes on its name.
    fn typing_the_space_name(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
        cx.simulate_input("!");
        shell.read_with(cx, |shell, cx| {
            shell
                .form
                .as_ref()
                .is_some_and(|form| form.name.read(cx).text() == "Trips!")
        })
    }

    /// New Profile makes the profile numbered and puts the Space in it, with the Profile row a
    /// field holding the name, all selected: typing replaces it, and ↩ keeps what's typed and gives
    /// the Space's name the keyboard back, without making the Space.
    #[gpui::test]
    fn a_new_profile_is_named_in_the_profile_row(cx: &mut TestAppContext) {
        let (shell, cx, host) = form(cx);
        new_profile(&shell, cx);
        assert_eq!(
            host.take_edits(),
            ["create profile Profile 2", "space 7 profile 9"]
        );
        assert!(naming(&shell, cx), "the Profile row isn't a field");
        cx.simulate_input("Work");
        cx.simulate_keystrokes("enter");
        assert!(!naming(&shell, cx), "↩ didn't close the field");
        assert_eq!(host.take_edits(), ["rename profile 9 Work"]);
        cx.run_until_parked();
        assert!(
            typing_the_space_name(&shell, cx),
            "the Space's name didn't get the keyboard back"
        );
        assert!(
            shell.read_with(cx, |shell, _| shell.form.is_some()),
            "↩ made the Space"
        );
    }

    /// Escape keeps the numbered name, and so does a field left empty.
    #[gpui::test]
    fn escape_or_an_empty_field_keeps_the_numbered_name(cx: &mut TestAppContext) {
        let (shell, cx, host) = form(cx);
        new_profile(&shell, cx);
        host.take_edits();
        cx.simulate_input("Work");
        cx.simulate_keystrokes("escape");
        assert!(!naming(&shell, cx), "escape didn't close the field");
        assert!(
            shell.read_with(cx, |shell, _| shell.form.is_some()),
            "escape closed the form"
        );
        cx.run_until_parked();
        assert!(
            typing_the_space_name(&shell, cx),
            "the Space's name didn't get the keyboard back"
        );

        new_profile(&shell, cx);
        host.take_edits();
        cx.simulate_keystrokes("backspace enter");
        assert!(!naming(&shell, cx), "↩ didn't close the field");
        assert!(
            host.take_edits().is_empty(),
            "an empty field renamed the profile"
        );
    }

    /// A click elsewhere keeps what's typed, and leaves the keyboard where the click put it.
    #[gpui::test]
    fn leaving_the_field_keeps_the_name(cx: &mut TestAppContext) {
        let (shell, cx, host) = form(cx);
        new_profile(&shell, cx);
        host.take_edits();
        cx.simulate_input("Work");
        shell.update_in(cx, |shell, window, cx| {
            window.focus(&shell.focus_handle, cx)
        });
        cx.run_until_parked();
        assert!(!naming(&shell, cx), "the field stayed");
        assert_eq!(host.take_edits(), ["rename profile 9 Work"]);
        assert!(
            shell.read_with(cx, |shell, _| !shell.focus_form),
            "the Space's name took the keyboard from where the click put it"
        );
    }

    /// Outside the form, New Profile asks for the name in the dialog first, the numbered one in its
    /// field: Create makes the profile with what's typed, or the numbered name if it's empty, and
    /// puts the Space in it. Cancel makes none.
    #[gpui::test]
    fn elsewhere_the_dialog_asks_for_the_name(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            shell.set_profiles(vec![(1, "Default".into())], cx);
            shell.set_spaces(vec![space(1, "Home")], cx);
            shell.set_space(1, cx);
        });
        new_profile(&shell, cx);
        assert_eq!(*host.told.borrow(), ["New Profile: Create / Cancel / "]);
        assert!(
            host.take_edits().is_empty(),
            "a profile was made before the name"
        );
        let create = host.told_action.borrow_mut().take().expect("its action");
        cx.update(|_, cx| create(vec![" Work ".into()], cx));
        assert_eq!(
            host.take_edits(),
            ["create profile Work", "space 1 profile 9"]
        );
        new_profile(&shell, cx);
        let create = host.told_action.borrow_mut().take().expect("its action");
        cx.update(|_, cx| create(vec![String::new()], cx));
        assert_eq!(
            host.take_edits(),
            ["create profile Profile 2", "space 1 profile 9"]
        );
    }

    /// The node with `role` named `label` in what the window last told VoiceOver.
    fn named(cx: &mut VisualTestContext, role: Role, label: &str) -> (NodeId, Node) {
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let found = tree
            .nodes
            .into_iter()
            .find(|(_, node)| node.role() == role && node.label() == Some(label));
        found.unwrap_or_else(|| panic!("no {role:?} called {label}"))
    }

    /// VoiceOver reads the form as a form called New Space, its rows as buttons that say what they
    /// open, and Create Space as off until there's a name. Its presses do what clicks do.
    #[gpui::test]
    fn voiceover_reads_the_form_and_presses_its_rows(cx: &mut TestAppContext) {
        let (shell, cx, host) = form(cx);
        // As the page slides in, its presses land where it's drawn when it's done.
        shell.update(cx, |shell, cx| {
            shell.settle = None;
            cx.notify();
        });
        cx.simulate_a11y_activation();
        cx.run_until_parked();
        named(cx, Role::Form, "New Space");
        named(cx, Role::Heading, "New Space");
        named(cx, Role::TextInput, "Space name");
        let (_, theme) = named(cx, Role::Button, "Choose a Theme");
        assert_eq!(theme.has_popup(), Some(HasPopup::Dialog));
        named(cx, Role::Button, "Cancel");
        let (create, node) = named(cx, Role::Button, "Create Space");
        assert!(!node.is_disabled(), "Create Space with a name");
        let (profile, node) = named(cx, Role::Button, "Profile, Default");
        assert_eq!(node.has_popup(), Some(HasPopup::Menu));
        cx.simulate_a11y_action(profile, Action::Click);
        cx.run_until_parked();
        let menu = host
            .menus
            .borrow()
            .last()
            .cloned()
            .expect("no menu for the Profile row");
        assert!(menu.iter().any(|(title, _)| title == "Default"), "{menu:?}");

        cx.simulate_a11y_action(create, Action::Click);
        cx.run_until_parked();
        assert_eq!(host.take_edits(), ["rename space 7 Trips"]);
        assert!(
            shell.read_with(cx, |shell, _| shell.form.is_none()),
            "the form stayed"
        );
    }

    #[gpui::test]
    fn without_a_name_voiceover_hears_create_space_is_off(cx: &mut TestAppContext) {
        let (_, cx, _) = form(cx);
        cx.simulate_keystrokes("cmd-a backspace");
        cx.simulate_a11y_activation();
        cx.run_until_parked();
        let (_, create) = named(cx, Role::Button, "Create Space");
        assert!(create.is_disabled(), "Create Space without a name");
    }
}
