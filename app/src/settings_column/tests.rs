//! The column as the user meets it: a click on a section shows it, the foot opens Chromium's
//! settings.

use std::cell::{Cell, RefCell};

use gpui::{
    Context, Focusable, InputEvent as _, Modifiers, TestAppContext, VisualTestContext, point, size,
};

use super::*;
use crate::tokens::shell::library::SEARCH_HEIGHT;

/// The column alone, showing what it last picked.
struct Shown {
    sections: Sections,
    shown: Section,
    logged: Rc<RefCell<Vec<String>>>,
    picked: Rc<RefCell<Vec<Section>>>,
    foot: Rc<Cell<usize>>,
}

impl Render for Shown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.weak_entity();
        let picked = self.picked.clone();
        let pick: Pick = Rc::new(move |section, _, cx| {
            picked.borrow_mut().push(section);
            view.update(cx, |view, cx| {
                view.shown = section;
                cx.notify();
            })
            .ok();
        });
        let foot = self.foot.clone();
        let foot: Foot = Rc::new(move |_, _| foot.set(foot.get() + 1));
        div().size_full().child(column(
            &self.sections,
            self.shown,
            true,
            self.logged.clone(),
            div().h(px(SEARCH_HEIGHT)).into_any_element(),
            None,
            pick,
            foot,
            window.is_a11y_active(),
        ))
    }
}

fn shown(cx: &mut TestAppContext) -> (gpui::Entity<Shown>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, _| Shown {
        sections: Sections::default(),
        shown: Section::General,
        logged: Rc::default(),
        picked: Rc::default(),
        foot: Rc::default(),
    });
    cx.simulate_resize(size(px(820.), px(660.)));
    cx.run_until_parked();
    (view, cx)
}

/// Where the middle of `section`'s plate is: the sections 2 apart from 12 under the search field,
/// 48 down, their groups 12 apart.
fn plate_y(section: Section) -> f32 {
    let (mut y, mut first) = (COLUMN_TOP + SEARCH_HEIGHT + SEARCH_GAP, true);
    for group in Section::GROUPS {
        if !first {
            y += SECTION_GROUP_GAP;
        }
        first = false;
        for &each in group {
            if each == section {
                return y + PLATE_HEIGHT / 2.;
            }
            y += PLATE_HEIGHT + PLATE_GAP;
        }
    }
    unreachable!()
}

/// The thirteen sections run down the column in their three groups, and a click on each shows it.
#[gpui::test]
fn a_click_on_a_section_shows_it(cx: &mut TestAppContext) {
    let (view, cx) = shown(cx);
    let names: Vec<&str> = Section::all().map(Section::name).collect();
    assert_eq!(
        names,
        [
            "General",
            "Tabs & Sidebar",
            "Features",
            "Agents",
            "Links",
            "Shortcuts",
            "Profiles",
            "Passwords",
            "Privacy",
            "Ad Blocking",
            "Sync",
            "Import & Export",
            "About"
        ]
    );
    // Last first, so each click is a change.
    for section in Section::all().collect::<Vec<_>>().into_iter().rev() {
        cx.simulate_click(
            point(px(COLUMN / 2.), px(plate_y(section))),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |view, _| view.shown),
            section,
            "{}",
            section.name()
        );
    }
    // Each click picked once: none reached the ground, whose press moves the window, which the
    // test window can't.
    assert_eq!(view.read_with(cx, |view, _| view.picked.borrow().len()), 13);
}

/// Chromium's settings sit at the column's foot, 8 up from the window's bottom, and a click there
/// opens them.
#[gpui::test]
fn the_foot_opens_chromiums_settings(cx: &mut TestAppContext) {
    let (view, cx) = shown(cx);
    let y = 660. - FOOT_BOTTOM - PLATE_HEIGHT / 2.;
    cx.simulate_click(point(px(COLUMN / 2.), px(y)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.foot.get()), 1);
    assert!(view.read_with(cx, |view, _| view.picked.borrow().is_empty()));
}

/// At Settings' 720 × 480 minimum the thirteen sections don't fit over the foot: they scroll above
/// it, so the last, scrolled to, shows clear of the foot, and a click on the foot is the foot's.
#[gpui::test]
fn at_the_least_height_the_sections_scroll_above_the_foot(cx: &mut TestAppContext) {
    let (view, cx) = shown(cx);
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let event = gpui::ScrollEvent {
            position: point(px(COLUMN / 2.), px(200.)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-400.))),
            ..Default::default()
        };
        window.dispatch_event(event.to_platform_input(), cx);
    });
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let about = view.read_with(cx, |view, _| {
        let logged = view.logged.borrow();
        let line = logged
            .iter()
            .find(|line| line.contains(" toolbar About at "))
            .cloned();
        line.and_then(|line| line.rsplit(' ').next()?.parse::<f32>().ok())
    });
    let foot_top = 480. - FOOT_BOTTOM - PLATE_HEIGHT;
    let about = about.expect("About wasn't drawn");
    assert!(
        about + PLATE_HEIGHT / 2. <= foot_top,
        "About at {about} runs under the foot at {foot_top}"
    );
    let y = 480. - FOOT_BOTTOM - PLATE_HEIGHT / 2.;
    cx.simulate_click(point(px(COLUMN / 2.), px(y)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.foot.get()), 1);
    assert!(
        view.read_with(cx, |view, _| view.picked.borrow().is_empty()),
        "the click picked a section"
    );
}

/// VoiceOver reads the column as a tab list of the thirteen sections, the one shown selected, and
/// Chromium settings as a button; its press on a section shows it, as a click does (#1381).
#[gpui::test]
fn voiceover_reads_the_sections_as_tabs(cx: &mut TestAppContext) {
    use gpui::accesskit::{Action, Role};
    let (view, cx) = shown(cx);
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let tabs: Vec<(String, bool)> = (tree.nodes.iter())
        .filter(|(_, node)| node.role() == Role::Tab)
        .map(|(_, node)| {
            (
                node.label().unwrap_or_default().into(),
                node.is_selected() == Some(true),
            )
        })
        .collect();
    let names: Vec<&str> = tabs.iter().map(|(name, _)| name.as_str()).collect();
    let all: Vec<&str> = Section::all().map(Section::name).collect();
    assert_eq!(names, all);
    let selected: Vec<&str> = (tabs.iter().filter(|(_, selected)| *selected))
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(selected, ["General"]);
    let list = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::TabList);
    assert_eq!(list.and_then(|(_, node)| node.label()), Some("Sections"));
    let chromium = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button && node.label() == Some("Chromium settings"));
    assert!(chromium.is_some(), "no Chromium settings button");
    let privacy = (tree.nodes.iter())
        .find(|(_, node)| node.role() == Role::Tab && node.label() == Some("Privacy"))
        .map(|(id, _)| *id)
        .expect("no Privacy tab");
    cx.simulate_a11y_action(privacy, Action::Click);
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.shown), Section::Privacy);
}

/// Settings in small with one of each of its tab stops, drawn as Settings draws them: the column
/// and its search field, and on the card a button, a switch, a pop-up, and a command's chip with
/// its ↺, while VoiceOver asks.
struct Stops {
    input: gpui::Entity<crate::text_input::TextInput>,
    card: crate::settings_card::Card,
}

impl Render for Stops {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use crate::settings_rows::{
            Chip, Look, stop_button, stop_chip, stop_popup, stop_restore, stop_switch,
        };
        let a11y = window.is_a11y_active();
        let look = Look::settings(true);
        let accent = look.accent;
        let field = div()
            .h(px(SEARCH_HEIGHT))
            .child(self.input.clone())
            .into_any_element();
        let controls = div()
            .flex()
            .flex_col()
            .child(stop_button(&look, "restore", "Restore…", a11y))
            .child(stop_switch(
                &look,
                "haptics",
                "Haptic feedback",
                true,
                1.,
                accent,
                a11y,
            ))
            .child(stop_popup(
                &look,
                "appearance",
                "Appearance",
                "Dark".into(),
                155.,
                false,
                a11y,
            ))
            .child(stop_restore(&look, "restore-new-tab", "New Tab", a11y))
            .child(stop_chip(
                &look,
                rgb(PAPER),
                "new-tab",
                "New Tab",
                "⌘T".into(),
                Chip::Keys,
                a11y,
            ));
        let colors = crate::settings_card::Colors {
            line: look.line,
            name: look.ink,
            heading: look.heading,
        };
        let card = self.card.draw("General", controls, true, colors);
        div()
            .size_full()
            .flex()
            .child(column(
                &Sections::default(),
                Section::General,
                true,
                Rc::default(),
                field,
                None,
                Rc::new(|_, _, _| {}),
                Rc::new(|_, _| {}),
                a11y,
            ))
            .child(card)
    }
}

/// Every place ⇥ stops in Settings is one VoiceOver can name: each has a role and a name, and ⇥
/// goes round all twenty, the thirteen sections, the field, the card's five and the foot.
#[gpui::test]
fn voiceover_names_every_stop_the_keyboard_reaches(cx: &mut TestAppContext) {
    use gpui::accesskit::{NodeId, Role};
    cx.update(crate::text_input::bind_keys);
    let (view, cx) = cx.add_window_view(|_, cx| {
        let input = cx.new(|cx| crate::text_input::TextInput::new("Search", cx));
        let _ = input.read(cx).focus_handle(cx).tab_stop(true);
        Stops {
            input,
            card: Default::default(),
        }
    });
    cx.simulate_resize(size(px(820.), px(660.)));
    cx.update(|window, cx| {
        window.activate_window();
        let field = view.read(cx).input.read(cx).focus_handle(cx);
        window.focus(&field, cx);
    });
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    let mut stops: Vec<(NodeId, Role, String)> = Vec::new();
    for _ in 0..40 {
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let focused = (tree.nodes.iter()).find(|(id, _)| *id == tree.focus);
        let (id, node) = focused.expect("the focus has no node in the tree");
        if stops.first().is_some_and(|(first, ..)| first == id) {
            break;
        }
        let name = node.label().unwrap_or_default().to_string();
        stops.push((*id, node.role(), name));
        cx.update(|window, cx| window.focus_next(cx));
        cx.run_until_parked();
    }
    let unnamed: Vec<_> = (stops.iter())
        .filter(|(_, role, name)| {
            name.is_empty() || matches!(role, Role::Window | Role::GenericContainer | Role::Unknown)
        })
        .collect();
    assert!(
        unnamed.is_empty(),
        "stops VoiceOver can't name: {unnamed:?} of {stops:?}"
    );
    let names: Vec<&str> = stops.iter().map(|(_, _, name)| name.as_str()).collect();
    for name in [
        "Restore…",
        "Haptic feedback",
        "Appearance",
        "Restore Defaults for New Tab",
        "New Tab, ⌘T",
        "Chromium settings",
        "General",
        "About",
    ] {
        assert!(names.contains(&name), "⇥ never reached {name}: {names:?}");
    }
    assert_eq!(stops.len(), 20, "{names:?}");
}

/// Settings in small for the keyboard: the column with the search field, and the card with a
/// button, under Settings' key context, as the window has them.
struct Keyed {
    focus: gpui::FocusHandle,
    input: gpui::Entity<crate::text_input::TextInput>,
    card: crate::settings_card::Card,
    sections: Sections,
    shown: Section,
    pressed: Rc<Cell<usize>>,
}

impl Render for Keyed {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::settings_search::{NextControl, PreviousControl, field};
        let view = cx.weak_entity();
        let pick: Pick = Rc::new(move |section, _, cx| {
            view.update(cx, |view, cx| {
                view.shown = section;
                cx.notify();
            })
            .ok();
        });
        let field = field(
            &self.input,
            rgb(PAPER),
            gpui::canvas(|_, _, _| {}, |_, _, _, _| {}),
            |_, _| {},
            cx,
        );
        let pressed = self.pressed.clone();
        let look = crate::settings_rows::Look::settings(true);
        let button = crate::settings_rows::button(&look, "restore", "Restore…")
            .on_click(move |_, _, _| pressed.set(pressed.get() + 1));
        let colors = crate::settings_card::Colors {
            line: look.line,
            name: look.ink,
            heading: look.heading,
        };
        let card = self.card.draw("General", div().child(button), true, colors);
        div()
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(|_: &NextControl, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousControl, window, cx| window.focus_prev(cx))
            .size_full()
            .flex()
            .child(column(
                &self.sections,
                self.shown,
                true,
                Rc::default(),
                field.into_any_element(),
                None,
                pick,
                Rc::new(|_, _| {}),
                window.is_a11y_active(),
            ))
            .child(card)
    }
}

/// A key pressed and let go, as a keyboard sends it: Space and Return click on the release.
fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    let keystroke = gpui::Keystroke::parse(key).unwrap();
    cx.update(|window, cx| {
        window.dispatch_event(gpui::KeyUpEvent { keystroke }.to_platform_input(), cx);
    });
    cx.run_until_parked();
}

/// From the search field, where Settings opens, ⇥ goes to the card's button and Space or Return
/// clicks it; ⇧⇥ goes back through the field to the sections, the last first; ↑ and ↓ move in the
/// list without leaving it at its ends; Return shows the section that has the keyboard.
#[gpui::test]
fn the_keyboard_goes_through_settings_in_reading_order(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::text_input::bind_keys(cx);
        crate::settings_search::bind_keys(cx);
    });
    let (view, cx) = cx.add_window_view(|_, cx| {
        let input = cx.new(|cx| crate::text_input::TextInput::new("Search", cx));
        let _ = input.read(cx).focus_handle(cx).tab_stop(true);
        Keyed {
            focus: cx.focus_handle(),
            input,
            card: Default::default(),
            sections: Default::default(),
            shown: Section::General,
            pressed: Rc::default(),
        }
    });
    cx.simulate_resize(size(px(820.), px(660.)));
    cx.update(|window, cx| {
        window.activate_window();
        let field = view.read(cx).input.read(cx).focus_handle(cx);
        window.focus(&field, cx);
    });
    cx.run_until_parked();
    let pressed = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.pressed.get());
    let shown = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.shown);

    cx.simulate_keystrokes("tab");
    press("enter", cx);
    assert_eq!(
        pressed(cx),
        1,
        "⇥ from the field didn't reach the card's button"
    );
    press("space", cx);
    assert_eq!(pressed(cx), 2, "Space didn't click the button");

    // Back through the field to the list's last section, About.
    cx.simulate_keystrokes("shift-tab shift-tab");
    press("enter", cx);
    assert_eq!(shown(cx), Section::About);
    // ↓ at the list's end stays there; ↑ goes to the section above.
    cx.simulate_keystrokes("down");
    press("enter", cx);
    assert_eq!(shown(cx), Section::About, "↓ left the list at its end");
    cx.simulate_keystrokes("up up");
    press("enter", cx);
    assert_eq!(shown(cx), Section::Sync);
    // ⇥ from the list goes on to the field, where typing searches.
    for _ in 0..3 {
        cx.simulate_keystrokes("down");
    }
    cx.simulate_keystrokes("tab");
    cx.simulate_input("pip");
    assert_eq!(
        view.read_with(cx, |view, cx| view.input.read(cx).text().to_string()),
        "pip"
    );
    assert_eq!(pressed(cx), 2);
}

gpui::actions!(settings_routes, [RoutedUndo, RoutedClose, RoutedEscape]);

/// Settings in small for the keys' routes, as the window nests them: Settings holding the
/// keyboard, as it opens; a row whose press doesn't reach Settings, as a link rule's with its
/// trash; the import, a view with the keyboard of its own and its Esc, whose Import… gives way to
/// what comes next; and a control that goes once pressed, as an ad-free site's row.
struct Routes {
    focus: gpui::FocusHandle,
    import: gpui::FocusHandle,
    logged: Rc<RefCell<Vec<String>>>,
    trashed: Rc<Cell<bool>>,
    imported: Rc<Cell<bool>>,
    gone: Rc<Cell<bool>>,
    undone: Rc<Cell<usize>>,
    closed: Rc<Cell<usize>>,
    escaped: Rc<Cell<usize>>,
}

impl Render for Routes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = crate::settings_rows::Look::settings(true);
        let control =
            |id: &'static str, flag: &Rc<Cell<bool>>, logged: &Rc<RefCell<Vec<String>>>| {
                let flag = flag.clone();
                crate::settings_rows::button(&look, id, id)
                    .relative()
                    .child(drawn(id.to_string(), logged.clone()))
                    .on_click(move |_, _, _| flag.set(true))
            };
        let (undone, closed, escaped) = (
            self.undone.clone(),
            self.closed.clone(),
            self.escaped.clone(),
        );
        let rule = div()
            .h(px(40.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when(!self.trashed.get(), |row| {
                row.child(control("trash", &self.trashed, &self.logged))
            });
        let import = div()
            .h(px(40.))
            .track_focus(&self.import)
            .key_context("Import")
            .on_action(move |_: &RoutedEscape, _, _| escaped.set(escaped.get() + 1))
            .child(match self.imported.get() {
                false => control("import", &self.imported, &self.logged).into_any_element(),
                true => div().child("Chrome's choices").into_any_element(),
            });
        let site = div().h(px(40.)).when(!self.gone.get(), |row| {
            row.child(control("show ads", &self.gone, &self.logged))
        });
        div()
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(move |_: &RoutedUndo, _, _| undone.set(undone.get() + 1))
            .on_action(move |_: &RoutedClose, _, _| closed.set(closed.get() + 1))
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.notify()))
            .size_full()
            .flex()
            .flex_col()
            .child(rule)
            .child(import)
            .child(site)
    }
}

/// Settings holding the keyboard as it opens, with the keys bound as the app binds them.
fn routes(cx: &mut TestAppContext) -> (gpui::Entity<Routes>, &mut VisualTestContext) {
    cx.update(|cx| {
        cx.bind_keys([
            gpui::KeyBinding::new("cmd-z", RoutedUndo, Some("Settings")),
            gpui::KeyBinding::new("cmd-w", RoutedClose, Some("Settings")),
            gpui::KeyBinding::new("escape", RoutedEscape, Some("Import")),
        ])
    });
    let (view, cx) = cx.add_window_view(|_, cx| Routes {
        focus: cx.focus_handle(),
        import: cx.focus_handle(),
        logged: Rc::default(),
        trashed: Rc::default(),
        imported: Rc::default(),
        gone: Rc::default(),
        undone: Rc::default(),
        closed: Rc::default(),
        escaped: Rc::default(),
    });
    cx.simulate_resize(size(px(820.), px(660.)));
    cx.update(|window, cx| {
        window.activate_window();
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    (view, cx)
}

/// A press on the control `what`, where it was drawn, and the frame after it.
fn press_on(view: &gpui::Entity<Routes>, what: &str, cx: &mut VisualTestContext) {
    let at = view.read_with(cx, |view, _| {
        let logged = view.logged.borrow();
        let line = logged
            .iter()
            .find(|line| line.contains(&format!(" {what} at ")))
            .cloned();
        let line = line.unwrap_or_else(|| panic!("{what} wasn't drawn"));
        let mut parts = line.rsplit(' ');
        let y: f32 = parts.next().unwrap().parse().unwrap();
        let x: f32 = parts.next().unwrap().parse().unwrap();
        point(px(x), px(y))
    });
    cx.simulate_click(at, Modifiers::none());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// A link rule's trash takes its row with it, and ⌘Z then reaches Settings, which brings the rule
/// back: the press left the keyboard with Settings, not with the trash that went.
#[gpui::test]
fn after_the_trash_cmd_z_reaches_settings(cx: &mut TestAppContext) {
    let (view, cx) = routes(cx);
    press_on(&view, "trash", cx);
    assert!(view.read_with(cx, |view, _| view.trashed.get()));
    cx.simulate_keystrokes("cmd-z");
    assert_eq!(
        view.read_with(cx, |view, _| view.undone.get()),
        1,
        "⌘Z didn't reach Settings"
    );
}

/// A press on Import… gives the import the keyboard, as before, so Esc in what replaces it
/// reaches the import.
#[gpui::test]
fn after_import_esc_reaches_the_import(cx: &mut TestAppContext) {
    let (view, cx) = routes(cx);
    press_on(&view, "import", cx);
    assert!(view.read_with(cx, |view, _| view.imported.get()));
    cx.simulate_keystrokes("escape");
    assert_eq!(
        view.read_with(cx, |view, _| view.escaped.get()),
        1,
        "Esc didn't reach the import"
    );
}

/// A control that goes once pressed leaves the keyboard with Settings, so ⌘W still hides it.
#[gpui::test]
fn after_a_control_goes_cmd_w_reaches_settings(cx: &mut TestAppContext) {
    let (view, cx) = routes(cx);
    press_on(&view, "show ads", cx);
    assert!(view.read_with(cx, |view, _| view.gone.get()));
    cx.simulate_keystrokes("cmd-w");
    assert_eq!(
        view.read_with(cx, |view, _| view.closed.get()),
        1,
        "⌘W didn't reach Settings"
    );
}

/// At Settings' least height the sections scroll above the foot, and the keys keep the one they
/// move to in sight: ⇧⇥ from the field to About, the last, scrolls the list to it, and ↑ back to
/// General scrolls it back (#2506).
#[gpui::test]
fn the_keys_keep_the_section_they_move_to_in_sight(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::text_input::bind_keys(cx);
        crate::settings_search::bind_keys(cx);
    });
    let (view, cx) = cx.add_window_view(|_, cx| {
        let input = cx.new(|cx| crate::text_input::TextInput::new("Search", cx));
        let _ = input.read(cx).focus_handle(cx).tab_stop(true);
        Keyed {
            focus: cx.focus_handle(),
            input,
            card: Default::default(),
            sections: Default::default(),
            shown: Section::General,
            pressed: Rc::default(),
        }
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.update(|window, cx| {
        window.activate_window();
        let field = view.read(cx).input.read(cx).focus_handle(cx);
        window.focus(&field, cx);
    });
    let settle = |cx: &mut VisualTestContext| {
        for _ in 0..3 {
            view.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
    };
    settle(cx);
    let scroll = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            let scroll = &view.sections.scroll;
            (
                f32::from(scroll.offset().y),
                f32::from(scroll.max_offset().y),
            )
        })
    };
    let (offset, most) = scroll(cx);
    assert_eq!(offset, 0.);
    assert!(
        most > PLATE_HEIGHT,
        "the sections fit at the least height: {most}"
    );
    cx.simulate_keystrokes("shift-tab");
    settle(cx);
    let (offset, _) = scroll(cx);
    assert!(
        offset <= PLATE_GAP - most,
        "⇧⇥ to About left it under the foot: {offset} of {most}"
    );
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.shown), Section::About);
    for _ in 1..Section::all().count() {
        cx.simulate_keystrokes("up");
    }
    settle(cx);
    assert_eq!(scroll(cx).0, 0., "↑ to General left it above the list");
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.shown), Section::General);
}
