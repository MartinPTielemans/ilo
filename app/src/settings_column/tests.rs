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
    shown: Section,
    logged: Rc<RefCell<Vec<String>>>,
    picked: Rc<RefCell<Vec<Section>>>,
    foot: Rc<Cell<usize>>,
}

impl Render for Shown {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            self.shown,
            true,
            self.logged.clone(),
            div().h(px(SEARCH_HEIGHT)).into_any_element(),
            None,
            pick,
            foot,
        ))
    }
}

fn shown(cx: &mut TestAppContext) -> (gpui::Entity<Shown>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, _| Shown {
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

/// The twelve sections run down the column in their three groups, and a click on each shows it.
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
    assert_eq!(view.read_with(cx, |view, _| view.picked.borrow().len()), 12);
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

/// At Settings' 720 × 480 minimum the twelve sections don't fit over the foot: they scroll above
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

/// Settings in small for the keyboard: the column with the search field, and the card with a
/// button, under Settings' key context, as the window has them.
struct Keyed {
    focus: gpui::FocusHandle,
    input: gpui::Entity<crate::text_input::TextInput>,
    card: crate::settings_card::Card,
    shown: Section,
    pressed: Rc<Cell<usize>>,
}

impl Render for Keyed {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                self.shown,
                true,
                Rc::default(),
                field.into_any_element(),
                None,
                pick,
                Rc::new(|_, _| {}),
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
