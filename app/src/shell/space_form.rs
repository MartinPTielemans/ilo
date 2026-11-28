//! The New Space form, which a new Space's page shows in place of its list: its name,
//! its profile and its theme (docs/ux-spec.md › Spaces).

use crate::tokens::text;
use std::time::Instant;

use gpui::{
    AnyElement, Context, Focusable as _, FontWeight, MouseButton, MouseDownEvent, Pixels, Point,
    Window,
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
use crate::dismissal::{By, Close, FocusReturn, Why};
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
                        .placeholder_color(self.palette.gray(FORM_PLACEHOLDER))
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
        self.take_keyboard(cx);
        self.focus_form = true;
        cx.notify();
    }

    /// Names the form's Space, if it has a name, and closes the form, by `by`.
    fn create_space(&mut self, by: By, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = self.form() else {
            return;
        };
        let name = form.name.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        // check.sh waits for these lines.
        log!("browser: create space {} {name}", form.space);
        let (space, tab, field) = (form.space, form.tab, form.name.clone());
        self.browser.rename_space(space, &name, cx);
        if let Some(tab) = tab {
            self.move_to_space(tab, space, false, cx);
        }
        self.leaving_form = self.form.take().map(|form| (form, Instant::now()));
        self.give_keyboard_back(&field, Close::new(Why::Chosen, by), window, cx);
        cx.notify();
    }

    /// Shows the Space the form was opened from and deletes the form's, closed for `close`.
    pub(super) fn cancel_space(
        &mut self,
        close: Close,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(form) = self.close_form(close, window, cx) {
            self.browser.delete_space(form.space, cx);
        }
    }

    /// Shows the Space the form was opened from, and returns the form, closed for `close`, whose
    /// Space is left for the caller to delete.
    pub(super) fn close_form(
        &mut self,
        close: Close,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<SpaceForm> {
        let form = self.form.take_if(|form| form.space == self.space)?;
        log!("browser: cancel space {}", form.space);
        self.show_space(form.from, cx);
        self.give_keyboard_back(&form.name, close, window, cx);
        cx.notify();
        Some(form)
    }

    /// The form's way back for the keyboard, the shared one (dismissal): to the window, and only
    /// while one of the form's fields, its name or the Profile row's, still has it. Nothing that
    /// closes the form gives it back to what opened it, as it has no Escape and no key of its own
    /// that closes it, so it remembers no opener.
    fn give_keyboard_back(
        &mut self,
        name: &gpui::Entity<TextInput>,
        close: Close,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = name.focus_handle(cx);
        let profile = (self.renaming.as_ref())
            .filter(|renaming| matches!(renaming.naming, Naming::Profile(_)))
            .map(|renaming| renaming.name.focus_handle(cx));
        let own: Vec<_> = std::iter::once(&name).chain(profile.as_ref()).collect();
        let window_focus = self.focus_handle.clone();
        FocusReturn::default().give_back(close, &own, &window_focus, window, cx);
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
            .children((!self.bar_on()).then(|| self.url_field(cx)))
            .children(self.scroll_edge_line())
            .children(self.tiles(cx));
        Some(div().absolute().size_full().overflow_hidden().child(page))
    }

    /// The Create a Space form, in place of the URL field and the Space's tabs, and as it leaves,
    /// `left` seconds after Create, over the Space's page, which takes the mouse then.
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
                // Leaving, it lets the Space's page under it take the mouse.
                .when(left.is_none(), |plate| plate.block_mouse_except_scroll())
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
            .when(left.is_none(), |button| {
                button.on_click(
                    cx.listener(|shell, _, window, cx| shell.create_space(By::Pointer, window, cx)),
                )
            });
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
                    .text_size(px(text::SMALL))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.palette.gray(SECONDARY))
                    .child("Cancel")
                    // It brightens to the form's text under the pointer (ours: Arc's isn't
                    // recorded). Create Space doesn't change, as Arc's doesn't.
                    .when(left.is_none(), |button| {
                        button
                            .block_mouse_except_scroll()
                            .hover(|style| style.text_color(self.palette.text(FORM_TEXT)))
                            .on_click(cx.listener(|shell, _, window, cx| {
                                shell.cancel_space(Close::new(Why::Chosen, By::Pointer), window, cx)
                            }))
                    }),
            );
        div()
            .id("space-form")
            .when(a11y, |form| form.role(Role::Form).aria_label("New Space"))
            .key_context("SpaceForm")
            .on_action(cx.listener(|shell, _: &CreateSpace, window, cx| {
                // Unless it was an input method's, composing in the name (dismissal's rule).
                let name = shell.form().map(|form| form.name.clone());
                if crate::dismissal::submits(&name, cx) {
                    shell.create_space(By::Keyboard, window, cx)
                }
            }))
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
    use std::{rc::Rc, time::Duration};

    use gpui::{
        Bounds, Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext,
        accesskit::{Action, HasPopup, Node, NodeId, Role},
        point, px, size,
    };
    use std::time::Instant;

    use crate::shell::{
        Command, FORM_FIELD_TOP, FORM_PROFILE_TOP, FORM_THEME_TOP, Shell, SpaceEntry,
        recorder::Recorder,
    };
    use crate::tokens::shell::{CREATE_BOTTOM, FORM_PLATE_HEIGHT, NAME_HEIGHT};

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

    /// Cancel pressed with the form's name typing: the form goes and the window has the keyboard.
    /// Pressed once the keyboard has gone outside the window's elements, as it does to the page,
    /// the form goes and leaves the keyboard there.
    #[gpui::test]
    fn cancel_gives_the_window_the_keyboard_only_while_the_form_has_it(cx: &mut TestAppContext) {
        for moved in [false, true] {
            let (shell, cx, _) = form(cx);
            shell.update(cx, |shell, cx| {
                shell.settle = None;
                cx.notify();
            });
            cx.simulate_a11y_activation();
            cx.run_until_parked();
            if moved {
                cx.update(|window, cx| window.blur(cx));
                cx.run_until_parked();
            }
            let (cancel, _) = named(cx, Role::Button, "Cancel");
            cx.simulate_a11y_action(cancel, Action::Click);
            cx.run_until_parked();
            assert!(
                shell.read_with(cx, |shell, _| shell.form.is_none()),
                "the form stayed"
            );
            let focused = cx.update(|window, cx| {
                let shell = shell.read(cx).focus_handle.clone();
                (shell.is_focused(window), window.focused(cx).is_none())
            });
            assert_eq!(focused, (!moved, moved), "moved: {moved}");
        }
    }

    /// ↩ while an input method composes in the Space's name is the composition's: it makes no
    /// Space and leaves the marked text for the method. Once the method has committed it, the
    /// next ↩ makes the Space, with the name as committed.
    #[gpui::test]
    fn return_while_composing_makes_no_space(cx: &mut TestAppContext) {
        use gpui::EntityInputHandler as _;
        let (shell, cx, host) = form(cx);
        host.take_edits();
        let name = shell.read_with(cx, |shell, _| shell.form.as_ref().unwrap().name.clone());
        cx.update(|window, cx| {
            name.update(cx, |name, cx| {
                name.replace_and_mark_text_in_range(None, "ぴ", None, window, cx)
            })
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("enter");
        assert!(
            host.take_edits().is_empty(),
            "↩ made the Space while composing"
        );
        assert!(
            shell.read_with(cx, |shell, _| shell.form.is_some()),
            "↩ closed the form while composing"
        );
        let typed = name.read_with(cx, |name, _| (name.text().to_string(), name.composing()));
        assert!(typed.1, "↩ ended the composition");
        cx.update(|window, cx| {
            name.update(cx, |name, cx| {
                name.replace_text_in_range(None, "ぴ", window, cx)
            })
        });
        cx.run_until_parked();
        let typed = name.read_with(cx, |name, _| (name.text().to_string(), name.composing()));
        assert!(!typed.1);
        cx.simulate_keystrokes("enter");
        assert_eq!(host.take_edits(), [format!("rename space 7 {}", typed.0)]);
    }

    /// ↩ makes the Space, and the form starts to leave.
    fn create(cx: &mut VisualTestContext, host: &Recorder) {
        cx.simulate_keystrokes("enter");
        assert_eq!(host.take_edits(), ["rename space 7 Trips"]);
    }

    /// ↩ makes the Space, and puts the form 150 ms into its 280 ms leave.
    fn mid_leave(shell: &Entity<Shell>, cx: &mut VisualTestContext, host: &Recorder) {
        create(cx, host);
        shell.update(cx, |shell, cx| {
            let (_, left) = shell.leaving_form.as_mut().expect("the form isn't leaving");
            *left = Instant::now() - Duration::from_millis(150);
            cx.notify();
        });
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.leaving_form.is_some()),
            "the form had gone"
        );
    }

    /// While the form leaves, the new Space's sidebar takes the mouse, as motion never blocks a
    /// click: a right-click on its name shows its menu, and a click on the URL field opens the
    /// command bar over it (#3554).
    #[gpui::test]
    fn mid_leave_a_right_click_on_the_name_shows_the_space_menu(cx: &mut TestAppContext) {
        let (shell, cx, host) = form(cx);
        mid_leave(&shell, cx, &host);
        host.menus.borrow_mut().clear();
        let name = shell.read_with(cx, |shell, _| shell.drawn_list_top() + NAME_HEIGHT / 2.);
        let at = point(px(60.), px(name));
        cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let first = host.menus.borrow().first().map(|menu| menu[0].0.clone());
        assert_eq!(first.as_deref(), Some("Change Space Icon…"));
    }

    #[gpui::test]
    fn mid_leave_a_click_on_the_url_field_opens_the_command_bar(cx: &mut TestAppContext) {
        let (shell, cx, host) = form(cx);
        mid_leave(&shell, cx, &host);
        host.command_bars.take();
        cx.simulate_click(point(px(100.), px(64.)), Modifiers::none());
        let field = Bounds::new(point(px(8.), px(46.)), size(px(212.), px(36.)));
        assert_eq!(host.command_bars.take(), [Some(field)]);
    }

    /// Holds the leaving form at its first frame, for as long as the test needs it: a start ahead
    /// of now reads as no time gone, so the leave can't end under clicks however slow the runner
    /// is (#3739).
    fn hold_leave(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |shell, cx| {
            let (_, left) = shell.leaving_form.as_mut().expect("the form isn't leaving");
            *left = Instant::now() + Duration::from_secs(3600);
            cx.notify();
        });
        cx.run_until_parked();
    }

    /// The menus a right-click at each of `ys`, 60 in, shows.
    fn right_clicks(cx: &mut VisualTestContext, host: &Recorder, ys: &[f32]) -> Vec<Vec<String>> {
        ys.iter()
            .map(|&y| {
                host.menus.borrow_mut().clear();
                let at = point(px(60.), px(y));
                cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
                cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
                cx.run_until_parked();
                let menus = host.menus.borrow();
                let first = menus.iter().filter_map(|menu| menu.first());
                first.map(|(title, _)| title.to_string()).collect()
            })
            .collect()
    }

    /// The leaving form's plates let the mouse through: a right-click on the field's, a row's or
    /// Create Space's place mid-leave does what it does there once the form has gone. The leave is
    /// held at its first frame, where every plate is still drawn, as the four clicks can outlast
    /// it on a slow runner.
    #[gpui::test]
    fn mid_leave_the_forms_plates_let_the_mouse_through(cx: &mut TestAppContext) {
        let (shell, cx, host) = form(cx);
        create(cx, &host);
        hold_leave(&shell, cx);
        let middle = FORM_PLATE_HEIGHT / 2.;
        let bottom = cx.update(|window, _| f32::from(window.viewport_size().height));
        let ys = [
            FORM_FIELD_TOP + middle,
            FORM_PROFILE_TOP + middle,
            FORM_THEME_TOP + middle,
            bottom - CREATE_BOTTOM - middle,
        ];
        let leaving = right_clicks(cx, &host, &ys);
        shell.update(cx, |shell, cx| {
            let (_, left) = shell.leaving_form.as_mut().expect("the form had gone");
            *left = Instant::now() - Duration::from_secs(1);
            cx.notify();
        });
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.leaving_form.is_none()),
            "the form stayed"
        );
        let gone = right_clicks(cx, &host, &ys);
        assert!(gone.iter().any(|menus| !menus.is_empty()), "{gone:?}");
        assert_eq!(leaving, gone);
    }

    /// While the form leaves, the keyboard is the Space's, as it is once the form has gone: ⌘T
    /// gives the new Space's field the keyboard, as in any Space with no page (#4140), and what's
    /// typed goes nowhere near the form's name.
    #[gpui::test]
    fn mid_leave_keys_go_to_the_space(cx: &mut TestAppContext) {
        let (shell, cx, host) = form(cx);
        mid_leave(&shell, cx, &host);
        cx.simulate_input("x");
        let name = shell.read_with(cx, |shell, cx| {
            let (form, _) = shell.leaving_form.as_ref().expect("the form had gone");
            form.name.read(cx).text().to_string()
        });
        assert_eq!(name, "Trips", "a key went to the leaving form");
        host.command_bars.take();
        cx.simulate_keystrokes("cmd-t");
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(host.command_bars.take().is_empty(), "⌘T opened a bar");
        // It takes it once the new Space's page has settled.
        shell.update(cx, |shell, cx| {
            shell.settle = None;
            cx.notify();
        });
        cx.run_until_parked();
        let field = cx.update(|window, cx| {
            let resting = shell
                .read(cx)
                .resting
                .as_ref()
                .expect("no field in the new Space");
            resting.bar.read(cx).field_focus(cx).is_focused(window)
        });
        assert!(field, "⌘T left the new Space's field without the keyboard");
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
