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
