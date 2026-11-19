//! What the find bar's count says to a screen reader, as the bar draws it for a tab's find.

use gpui::{TestAppContext, VisualTestContext, Window, WindowHandle};

use super::*;

/// The count for a tab's find, as the bar draws it.
struct Bar(Find);

impl Render for Bar {
    fn render(&mut self, window: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div().child(status(self.0.found(), window.is_a11y_active()))
    }
}

/// The status in the tree the window last sent: whether it's live, its name and its value.
type Said = Option<(Option<Live>, String, String)>;

fn said(window: &VisualTestContext) -> Said {
    let tree = window.a11y_tree().expect("GPUI built a tree");
    tree.nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Status)
        .map(|(_, node)| {
            let text = |text: Option<&str>| text.unwrap_or_default().to_string();
            (node.live(), text(node.label()), text(node.value()))
        })
}

fn named(live: Option<Live>, name: &str, value: &str) -> Said {
    Some((live, name.to_string(), value.to_string()))
}

/// The core reports `find`, and the bar draws it.
fn report(find: Find, bar: WindowHandle<Bar>, window: &mut VisualTestContext) {
    bar.update(window, |bar, _, cx| {
        bar.0 = find;
        cx.notify();
    })
    .unwrap();
    window.run_until_parked();
}

fn open(text: &str, selected: i32, matches: i32) -> Find {
    Find {
        open: true,
        text: text.to_string(),
        matches,
        selected,
    }
}

/// The bar drawn hidden at launch says nothing: its count is live only while the tab's find is
/// open with a count, as a live region is said as it's added and as its name changes (#3499).
#[gpui::test]
fn the_find_bars_count_is_said_only_while_its_find_has_one(cx: &mut TestAppContext) {
    let bar = cx.add_window(|_: &mut Window, _: &mut gpui::Context<Bar>| Bar(Find::default()));
    let mut window = VisualTestContext::from_window(bar.into(), cx);
    window.simulate_a11y_activation();
    assert_eq!(said(&window), named(None, "Matches", ""));

    // Typed, before the page has looked: nothing to say yet.
    report(open("cherry", 0, -1), bar, &mut window);
    let live = said(&window).and_then(|(live, _, _)| live);
    assert_eq!(live, None, "the count went live before the page had looked");

    report(open("cherry", 1, 12), bar, &mut window);
    let counted = named(Some(Live::Polite), "Matches, 1 of 12", "1 of 12");
    assert_eq!(said(&window), counted);

    report(open("cherries", 0, 0), bar, &mut window);
    let none = named(Some(Live::Polite), "Matches, Not found", "Not found");
    assert_eq!(said(&window), none);

    // Closed, its text and count kept for when it opens again: the hidden bar says nothing, as it
    // doesn't when it's drawn for another tab whose find is closed.
    let closed = Find {
        open: false,
        ..open("cherry", 1, 12)
    };
    report(closed, bar, &mut window);
    assert_eq!(said(&window), named(None, "Matches", ""));
}
