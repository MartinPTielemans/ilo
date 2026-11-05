//! The column as the user meets it: a click on a section shows it, the foot opens Chromium's
//! settings.

use std::cell::{Cell, RefCell};

use gpui::{Context, InputEvent as _, Modifiers, TestAppContext, VisualTestContext, point, size};

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
