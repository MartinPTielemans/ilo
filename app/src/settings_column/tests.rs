//! The column as the user meets it: a click on a section shows it, the foot opens Chromium's
//! settings.

use std::cell::{Cell, RefCell};

use gpui::{Context, InputEvent as _, Modifiers, TestAppContext, VisualTestContext, point, size};

use super::*;
use crate::settings_rows::{NextStop, PreviousStop, StopDown, StopUp, bind_stop_keys};

gpui::actions!(column_tests, [Before, After]);

/// The column alone, showing what it last picked.
struct Shown {
    shown: Section,
    logged: Rc<RefCell<Vec<String>>>,
    picked: Rc<RefCell<Vec<Section>>>,
    foot: Rc<Cell<usize>>,
    stops: crate::settings_rows::Stops,
    focus: gpui::FocusHandle,
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
        self.stops.begin();
        div()
            .key_context("Shown")
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|view, _: &NextStop, window, cx| {
                view.stops.go(true, true, window, cx);
            }))
            .on_action(cx.listener(|view, _: &PreviousStop, window, cx| {
                view.stops.go(false, true, window, cx);
            }))
            .on_action(cx.listener(|view, _: &StopDown, window, cx| {
                view.stops.go(true, false, window, cx);
            }))
            .on_action(cx.listener(|view, _: &StopUp, window, cx| {
                view.stops.go(false, false, window, cx);
            }))
            // As Settings' ↑ and ↓ on the column show the section beside, its ring with it.
            .on_action(cx.listener(|view, _: &Before, window, cx| view.key(false, window, cx)))
            .on_action(cx.listener(|view, _: &After, window, cx| view.key(true, window, cx)))
            .child(column(
                self.shown,
                true,
                self.logged.clone(),
                pick,
                foot,
                &self.stops,
                cx,
            ))
    }
}

impl Shown {
    fn key(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.shown = beside(self.shown, forward);
        window.focus(&self.stops.handle(self.shown.name(), cx), cx);
        cx.notify();
    }

    /// The name of the stop with the keyboard, if one has it.
    fn keyed(&self, window: &Window, cx: &App) -> Option<String> {
        let focused = window.focused(cx)?;
        let names = Section::all()
            .map(Section::name)
            .chain(["chromium-settings"]);
        names.map(String::from).find(|name| {
            self.stops
                .handle(gpui::SharedString::from(name.clone()), cx)
                == focused
        })
    }
}

fn shown(cx: &mut TestAppContext) -> (gpui::Entity<Shown>, &mut VisualTestContext) {
    cx.update(|cx| {
        bind_stop_keys("Shown", cx);
        let column = Some(COLUMN_KEYS);
        cx.bind_keys([
            gpui::KeyBinding::new("up", Before, column),
            gpui::KeyBinding::new("down", After, column),
        ]);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Shown {
        shown: Section::General,
        logged: Rc::default(),
        picked: Rc::default(),
        foot: Rc::default(),
        stops: Default::default(),
        focus: cx.focus_handle(),
    });
    // As Settings' window has the keyboard as it opens.
    cx.update(|window, cx| window.focus(&view.read(cx).focus.clone(), cx));
    cx.simulate_resize(size(px(820.), px(660.)));
    cx.run_until_parked();
    (view, cx)
}

/// Where the middle of `section`'s plate is: the sections 2 apart from 48 down, their groups 12
/// apart.
fn plate_y(section: Section) -> f32 {
    let (mut y, mut first) = (COLUMN_TOP, true);
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

/// From the keyboard the column is one stop, the shown section's plate: Tab rings it, ↓ and ↑ show
/// the section after or before with the ring on it, stopping at either end, and Tab goes on to
/// Chromium settings at the foot, which Space opens.
#[gpui::test]
fn the_keys_go_down_the_column(cx: &mut TestAppContext) {
    let (view, cx) = shown(cx);
    let keyed =
        |cx: &mut VisualTestContext| cx.update(|window, cx| view.read(cx).keyed(window, cx));
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx).as_deref(), Some("General"));
    cx.simulate_keystrokes("up");
    assert_eq!(view.read_with(cx, |view, _| view.shown), Section::General);
    cx.simulate_keystrokes("down down");
    assert_eq!(view.read_with(cx, |view, _| view.shown), Section::Features);
    assert_eq!(keyed(cx).as_deref(), Some("Features"));
    for _ in 0..12 {
        cx.simulate_keystrokes("down");
    }
    assert_eq!(view.read_with(cx, |view, _| view.shown), Section::About);
    cx.simulate_keystrokes("up");
    assert_eq!(keyed(cx).as_deref(), Some("Import & Export"));
    // The sections it passed were picked by no click.
    assert!(view.read_with(cx, |view, _| view.picked.borrow().is_empty()));
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx).as_deref(), Some("chromium-settings"));
    cx.simulate_keystrokes("space");
    let keystroke = gpui::Keystroke::parse("space").unwrap();
    cx.update(|window, cx| {
        let up = gpui::PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke });
        window.dispatch_event(up, cx);
    });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.foot.get()), 1);
    // And round, to the column's one stop.
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx).as_deref(), Some("Import & Export"));
}
