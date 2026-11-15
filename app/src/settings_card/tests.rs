//! The card as the user meets it at Settings' smallest: scrolled, the section's name and the
//! heading gone under it hold at its top, and back at the top nothing does.

use gpui::{
    InputEvent as _, Render, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, Window,
    size,
};

use super::Paging as To;
use super::*;

/// A section at Settings' 720 × 480 minimum: a page much taller than its card, with two
/// headings far apart.
struct Section {
    card: Card,
}

impl Render for Section {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let heading = |name: &'static str| {
            div()
                .relative()
                .h(px(20.))
                .child(name)
                .child(self.card.mark(name.into()))
        };
        let page = div()
            .flex()
            .flex_col()
            .child(div().h(px(300.)))
            .child(heading("Import"))
            .child(div().h(px(600.)))
            .child(heading("Export"))
            .child(div().h(px(600.)));
        let colors = Colors {
            line: rgb(0x333333),
            name: rgb(0xffffff),
            heading: rgb(0x999999),
        };
        div()
            .size_full()
            .flex()
            .child(div().w(px(208.)))
            .child(self.card.draw("Import & Export", page, true, colors))
    }
}

fn held(view: &gpui::Entity<Section>, cx: &mut VisualTestContext) -> Option<SharedString> {
    view.read_with(cx, |view, _| view.card.held("Import & Export"))
}

/// Moves the page by `dy` under the wheel and draws it twice, as the next frames do: the first
/// places the headings where the scroll put them, the second reads them.
fn wheel(view: &gpui::Entity<Section>, cx: &mut VisualTestContext, dy: f32) {
    cx.update(|window, cx| {
        let event = ScrollEvent {
            position: point(px(400.), px(240.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
            ..Default::default()
        };
        window.dispatch_event(event.to_platform_input(), cx);
    });
    for _ in 0..2 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
}

#[gpui::test]
fn the_section_and_the_heading_scrolled_past_hold_at_the_top(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| Section {
        card: Card::default(),
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.run_until_parked();
    assert_eq!(
        held(&view, cx),
        None,
        "the card's top holds something before a scroll"
    );
    // Past the page's top, before either heading: the section's name alone.
    wheel(&view, cx, -100.);
    assert_eq!(held(&view, cx).as_deref(), Some("Import & Export"));
    // Import's heading gone under it.
    wheel(&view, cx, -400.);
    assert_eq!(held(&view, cx).as_deref(), Some("Import & Export › Import"));
    // Then Export's.
    wheel(&view, cx, -700.);
    assert_eq!(held(&view, cx).as_deref(), Some("Import & Export › Export"));
    // Back at the top, nothing holds, and a section shown afresh starts there.
    wheel(&view, cx, -400.);
    view.update(cx, |view, _| view.card.reset());
    wheel(&view, cx, 0.);
    assert_eq!(
        held(&view, cx),
        None,
        "a section shown again still held its place"
    );
}

/// A page `tall` points high on the card, at Settings' smallest.
struct Page {
    card: Card,
    tall: f32,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let colors = Colors {
            line: rgb(0x333333),
            name: rgb(0xffffff),
            heading: rgb(0x999999),
        };
        let page = div().h(px(self.tall));
        div()
            .size_full()
            .flex()
            .child(div().w(px(208.)))
            .child(self.card.draw("Sync", page, true, colors))
    }
}

/// The card around a page `tall` high, turned by a wheel as far up the page as it goes: how far it
/// scrolled, how far it could, the scroll view's height, and whether the wheel's frame, which
/// nothing draws again after over a page that didn't move, held the section's name at the top.
fn scrolled(tall: f32, cx: &mut TestAppContext) -> (f32, f32, f32, bool) {
    let (view, cx) = cx.add_window_view(|_, _| Page {
        card: Card::default(),
        tall,
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let event = ScrollEvent {
            position: point(px(400.), px(240.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-2000.))),
            ..Default::default()
        };
        window.dispatch_event(event.to_platform_input(), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        (
            -f32::from(view.card.scroll.offset().y),
            f32::from(view.card.scroll.max_offset().y),
            view.card.height.get(),
            view.card.logged.borrow().is_some(),
        )
    })
}

#[gpui::test]
fn a_page_that_fits_its_card_doesnt_scroll(cx: &mut TestAppContext) {
    let (_, _, card, _) = scrolled(0., cx);
    // Its rows, its top margin and its bottom one, a point short of the card.
    let (offset, most, _, held) = scrolled(card - PAGE_TOP - PAGE_BOTTOM - 1., cx);
    assert_eq!((offset, most), (0., 0.), "a page that fits scrolled");
    assert!(
        !held,
        "a wheel over a page that fits held its name over its title"
    );
}

#[gpui::test]
fn a_long_page_scrolls_to_its_bottom_margin_and_no_further(cx: &mut TestAppContext) {
    let (offset, most, card, held) = scrolled(1000., cx);
    assert_eq!(most, 1000. + PAGE_TOP + PAGE_BOTTOM - card);
    assert_eq!(
        offset, most,
        "a wheel past its end scrolled past its bottom margin"
    );
    assert!(held, "a long page scrolled held nothing at its top");
}

#[gpui::test]
fn a_page_scrolled_and_then_grown_into_its_card_holds_nothing(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| Page {
        card: Card::default(),
        tall: 600.,
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let event = ScrollEvent {
            position: point(px(400.), px(240.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-2000.))),
            ..Default::default()
        };
        window.dispatch_event(event.to_platform_input(), cx);
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.card.logged.borrow().is_some()));
    // The window made taller than the page, as a resize does.
    cx.simulate_resize(size(px(720.), px(900.)));
    cx.run_until_parked();
    // And the frame after, which the card asks for once the page's new layout holds it.
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(f32::from(view.card.scroll.offset().y), 0.);
        assert!(
            view.card.logged.borrow().is_none(),
            "a page grown into its card still held its name over its title"
        );
    });
}

/// Settings at its smallest with a page taller than its window: thirty controls down the card,
/// each a stop for ⇥ that a press doesn't give the keyboard, as Settings' are, in a window that
/// takes the keys as Settings does.
struct Controls {
    card: Card,
    focus: gpui::FocusHandle,
    controls: Vec<gpui::FocusHandle>,
    /// Where each control was last drawn.
    drawn: Rc<RefCell<Vec<Bounds<Pixels>>>>,
}

impl Render for Controls {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        use crate::settings_search::{
            NextControl, PageBottom, PageDown, PageTop, PageUp, PreviousControl,
        };
        let colors = Colors {
            line: rgb(0x333333),
            name: rgb(0xffffff),
            heading: rgb(0x999999),
        };
        let rows = self.controls.iter().enumerate().map(|(at, focus)| {
            let drawn = self.drawn.clone();
            div()
                .id(at)
                .relative()
                .h(px(ROW))
                .track_focus(focus)
                .focus_from_keys_only()
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, _, _| drawn.borrow_mut()[at] = bounds,
                    )
                    .absolute()
                    .inset_0(),
                )
        });
        let page = div().flex().flex_col().children(rows);
        div()
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, window, cx| {
                let to = To::space(&event.keystroke).filter(|_| view.focus.is_focused(window));
                if let Some(to) = to {
                    cx.stop_propagation();
                    view.page(to, cx);
                }
            }))
            .on_action(|_: &NextControl, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousControl, window, cx| window.focus_prev(cx))
            .on_action(cx.listener(|view, _: &PageDown, _, cx| view.page(To::Down, cx)))
            .on_action(cx.listener(|view, _: &PageUp, _, cx| view.page(To::Up, cx)))
            .on_action(cx.listener(|view, _: &PageTop, _, cx| view.page(To::Top, cx)))
            .on_action(cx.listener(|view, _: &PageBottom, _, cx| view.page(To::Bottom, cx)))
            .size_full()
            .flex()
            .child(div().w(px(208.)))
            .child(self.card.draw("General", page, true, colors))
    }
}

impl Controls {
    /// As Settings pages its card.
    fn page(&mut self, to: To, cx: &mut gpui::Context<Self>) {
        if self.card.page(to) {
            cx.notify();
        }
    }
}

/// A control's height on the test's page, and how many it has: three cards' worth.
const ROW: f32 = 40.;
const COUNT: usize = 30;

fn controls(cx: &mut TestAppContext) -> (gpui::Entity<Controls>, &mut VisualTestContext) {
    cx.update(crate::settings_search::bind_keys);
    let (view, cx) = cx.add_window_view(|_, cx| Controls {
        card: Card::default(),
        focus: cx.focus_handle(),
        controls: (0..COUNT)
            .map(|_| cx.focus_handle().tab_index(0).tab_stop(true))
            .collect(),
        drawn: Rc::new(RefCell::new(vec![Bounds::default(); COUNT])),
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.update(|window, cx| {
        window.activate_window();
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    settle(&view, cx);
    (view, cx)
}

/// The frames after a key: the one that draws its change, and those the card asks for after it.
fn settle(view: &gpui::Entity<Controls>, cx: &mut VisualTestContext) {
    for _ in 0..3 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
}

/// The control with the keyboard, and whether the card shows it whole: under the held title once
/// the page is scrolled, and over the fade.
fn focused(view: &gpui::Entity<Controls>, cx: &mut VisualTestContext) -> (Option<usize>, bool) {
    cx.update(|window, cx| {
        let view = view.read(cx);
        let at = view
            .controls
            .iter()
            .position(|focus| focus.is_focused(window));
        let card = view.card.top.get();
        let held = if f32::from(view.card.scroll.offset().y) < 0. {
            HELD_TITLE
        } else {
            0.
        };
        let whole = at.is_some_and(|at| {
            let row = view.drawn.borrow()[at];
            f32::from(row.origin.y) >= card + held - 0.5
                && f32::from(row.bottom()) <= card + view.card.height.get() - FADE + 0.5
        });
        (at, whole)
    })
}

fn offset(view: &gpui::Entity<Controls>, cx: &mut VisualTestContext) -> f32 {
    view.read_with(cx, |view, _| f32::from(view.card.scroll.offset().y))
}

/// ⇥ down a page taller than its window keeps each control it reaches in sight, scrolling only as
/// a control would leave it, and ⇧⇥ back up does the same the other way (#2506).
#[gpui::test]
fn the_keyboard_scrolls_the_control_it_moves_to_into_sight(cx: &mut TestAppContext) {
    let (view, cx) = controls(cx);
    assert_eq!(offset(&view, cx), 0.);
    let mut scrolled_at = None;
    for at in 0..COUNT {
        cx.simulate_keystrokes("tab");
        settle(&view, cx);
        let (focused, whole) = focused(&view, cx);
        assert_eq!(focused, Some(at), "⇥ didn't move to the next control");
        assert!(whole, "⇥ to control {at} left it out of sight");
        if scrolled_at.is_none() && offset(&view, cx) < 0. {
            scrolled_at = Some(at);
        }
    }
    // The first controls were in sight already: the page moved only for one under the fade.
    assert!(
        scrolled_at.is_some_and(|at| at > 3),
        "the page scrolled before a control left sight: {scrolled_at:?}"
    );
    let bottom = view.read_with(cx, |view, _| f32::from(view.card.scroll.max_offset().y));
    assert_eq!(
        offset(&view, cx),
        -bottom,
        "the last control didn't take the page to its foot"
    );
    for at in (0..COUNT - 1).rev() {
        cx.simulate_keystrokes("shift-tab");
        settle(&view, cx);
        assert_eq!(focused(&view, cx), (Some(at), true), "⇧⇥ to control {at}");
    }
    assert_eq!(
        offset(&view, cx),
        0.,
        "the first control didn't take the page to its top"
    );
}

/// A control followed into sight is left where a wheel puts it, out of sight again, until the
/// keys move.
#[gpui::test]
fn a_wheel_takes_the_page_from_the_control_with_the_keyboard(cx: &mut TestAppContext) {
    let (view, cx) = controls(cx);
    for _ in 0..12 {
        cx.simulate_keystrokes("tab");
    }
    settle(&view, cx);
    assert_eq!(focused(&view, cx), (Some(11), true));
    cx.update(|window, cx| {
        let event = ScrollEvent {
            position: point(px(400.), px(240.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(2000.))),
            ..Default::default()
        };
        window.dispatch_event(event.to_platform_input(), cx);
    });
    settle(&view, cx);
    assert_eq!(
        offset(&view, cx),
        0.,
        "the page went back to the control with the keyboard"
    );
    assert_eq!(focused(&view, cx), (Some(11), false));
    cx.simulate_keystrokes("tab");
    settle(&view, cx);
    assert_eq!(focused(&view, cx), (Some(12), true));
}

/// A press on a control leaves the page where it is: the keyboard it leaves to the card itself
/// is nothing in the page to scroll to, and following it moved the page from under the pointer by
/// the held title's height (#2522).
#[gpui::test]
fn a_press_on_a_control_leaves_the_page_where_it_is(cx: &mut TestAppContext) {
    let (view, cx) = controls(cx);
    cx.update(|window, cx| {
        let event = ScrollEvent {
            position: point(px(400.), px(240.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-300.))),
            ..Default::default()
        };
        window.dispatch_event(event.to_platform_input(), cx);
    });
    settle(&view, cx);
    let scrolled = offset(&view, cx);
    assert!(
        scrolled < -HELD_TITLE,
        "the wheel didn't scroll the page: {scrolled}"
    );
    let row = view.read_with(cx, |view, _| view.drawn.borrow()[10]);
    cx.simulate_click(row.center(), gpui::Modifiers::none());
    settle(&view, cx);
    assert_eq!(offset(&view, cx), scrolled, "a press moved the page");
    assert_eq!(view.read_with(cx, |view, _| view.drawn.borrow()[10]), row);
}

/// Page Down and Page Up move the page by what the card shows, Home and End to its ends, and Space
/// and ⇧Space page while the window has the keyboard, but are a control's while it has.
#[gpui::test]
fn the_paging_keys_move_the_page(cx: &mut TestAppContext) {
    let (view, cx) = controls(cx);
    let (sight, bottom) = view.read_with(cx, |view, _| {
        (
            view.card.height.get() - HELD_TITLE - FADE,
            f32::from(view.card.scroll.max_offset().y),
        )
    });
    assert!(
        bottom > 2. * sight,
        "the page isn't three cards tall: {bottom} {sight}"
    );
    cx.simulate_keystrokes("pagedown");
    settle(&view, cx);
    assert_eq!(offset(&view, cx), -sight);
    cx.simulate_keystrokes("space");
    settle(&view, cx);
    assert_eq!(offset(&view, cx), -2. * sight, "Space didn't page down");
    cx.simulate_keystrokes("shift-space");
    settle(&view, cx);
    assert_eq!(offset(&view, cx), -sight, "⇧Space didn't page up");
    cx.simulate_keystrokes("pageup pageup");
    settle(&view, cx);
    assert_eq!(offset(&view, cx), 0., "Page Up went past the top");
    cx.simulate_keystrokes("end");
    settle(&view, cx);
    assert_eq!(offset(&view, cx), -bottom);
    cx.simulate_keystrokes("pagedown");
    settle(&view, cx);
    assert_eq!(offset(&view, cx), -bottom, "Page Down went past the foot");
    cx.simulate_keystrokes("home");
    settle(&view, cx);
    assert_eq!(offset(&view, cx), 0.);
    // With a control, Space is the control's.
    cx.simulate_keystrokes("tab space");
    settle(&view, cx);
    assert_eq!(offset(&view, cx), 0., "Space with a control paged");
}
