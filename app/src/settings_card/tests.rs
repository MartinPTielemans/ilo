//! The card as the user meets it at Settings' smallest: scrolled, the section's name and the
//! heading gone under it hold at its top, and back at the top nothing does.

use gpui::{
    InputEvent as _, Render, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, Window,
    size,
};

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
