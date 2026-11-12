//! Pages that break, on a host that records what the shell asks of the core.

use std::rc::Rc;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};

use crate::shell::{Folders, Page, Shell, Split, Trouble, recorder::Recorder};

/// A drawn shell showing one Space with Today tabs 10, 11 and 12, the first two in a split view
/// when `split`, and tab 10 shown.
fn window_with_tabs(
    cx: &mut TestAppContext,
    split: bool,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Space".into(), None, 0, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        let splits = match split {
            true => vec![Split {
                tabs: vec![10, 11],
                ..Split::default()
            }],
            false => Vec::new(),
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11, 12],
            splits,
            Folders::default(),
            cx,
        );
        for tab in [10, 11, 12] {
            shell.set_page(tab, page(Trouble::Fine), cx);
        }
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

fn page(trouble: Trouble) -> Page {
    Page {
        url: "https://example.com/".into(),
        trouble,
        ..Page::default()
    }
}

fn report(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64, trouble: Trouble) {
    shell.update(cx, |shell, cx| shell.set_page(tab, page(trouble), cx));
    cx.run_until_parked();
}

fn select(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) {
    shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
    cx.run_until_parked();
}

/// Where the card's Reload is drawn, if it is.
fn reload_button(
    shell: &Entity<Shell>,
    cx: &mut VisualTestContext,
) -> Option<gpui::Bounds<gpui::Pixels>> {
    shell.update(cx, |shell, cx| {
        shell.crash_reload_drawn.set(None);
        cx.notify();
    });
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| shell.crash_reload_drawn.get())
}

/// The card shows in the pane of the tab shown whose page died, and its Reload asks the core to
/// load that tab's page again.
#[gpui::test]
fn a_page_that_died_shows_the_card_and_reload_loads_it_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_tabs(cx, false);
    assert!(
        reload_button(&shell, cx).is_none(),
        "a page that's fine has no card"
    );
    report(&shell, cx, 10, Trouble::Crashed);
    let button = reload_button(&shell, cx).expect("the card's Reload is drawn");
    let frame = shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport).0);
    assert!(
        frame.contains(&button.center()),
        "{button:?} is in the page's frame {frame:?}"
    );
    cx.simulate_click(button.center(), Modifiers::none());
    assert_eq!(host.take(), ["reload 10"]);

    // Once the core says the page is back, the card goes.
    report(&shell, cx, 10, Trouble::Fine);
    assert!(reload_button(&shell, cx).is_none());
}

/// Only the tab shown has a card: another tab shown in its place has its page, and the card is
/// there again once the dead tab is.
#[gpui::test]
fn only_the_tab_shown_has_the_card(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_tabs(cx, false);
    report(&shell, cx, 10, Trouble::Crashed);
    select(&shell, cx, 12);
    assert!(reload_button(&shell, cx).is_none());
    select(&shell, cx, 10);
    assert!(reload_button(&shell, cx).is_some());
}

/// In a split view, the card is in the dead page's own pane, and the page beside it keeps its.
#[gpui::test]
fn in_a_split_view_the_card_is_in_its_own_pane(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_tabs(cx, true);
    report(&shell, cx, 11, Trouble::Crashed);
    let button = reload_button(&shell, cx).expect("the card shows beside the tab shown");
    let frame = shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport).0);
    assert!(
        button.center().x > frame.center().x,
        "{button:?} is in the right half of {frame:?}"
    );
}

/// A page that died or hung marks its row; one with an error page, which says so itself, doesn't.
#[gpui::test]
fn the_row_is_marked_while_its_page_died_or_hung(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_tabs(cx, false);
    let marked = |cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.trouble_mark(12).is_some())
    };
    assert!(!marked(cx));
    for (trouble, expected) in [
        (Trouble::Crashed, true),
        (Trouble::Hung, true),
        (Trouble::NetworkError(-105), false),
        (Trouble::Fine, false),
    ] {
        report(&shell, cx, 12, trouble);
        assert_eq!(marked(cx), expected, "{trouble:?}");
    }
}

/// The note shows while the tab shown is hung, goes with another tab shown or the page answering,
/// and never shows for a tab behind.
#[gpui::test]
fn the_hang_note_follows_the_tab_shown(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_tabs(cx, false);
    report(&shell, cx, 12, Trouble::Hung);
    assert!(
        host.take()
            .iter()
            .all(|call| !call.starts_with("hang note")),
        "12 isn't shown"
    );
    report(&shell, cx, 10, Trouble::Hung);
    assert_eq!(host.take(), ["hang note 10 shown"]);
    select(&shell, cx, 11);
    assert_eq!(host.take(), ["hang note 10 hidden"]);
    select(&shell, cx, 12);
    assert_eq!(host.take(), ["hang note 12 shown"]);
    report(&shell, cx, 12, Trouble::Fine);
    assert_eq!(host.take(), ["hang note 12 hidden"]);
}

/// A window behind another notes its page's hang once it comes in front, as the note shows over
/// the window in front (#1432).
#[gpui::test]
fn a_window_behind_notes_its_hang_once_in_front(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_tabs(cx, false);
    host.behind.set(true);
    report(&shell, cx, 10, Trouble::Hung);
    assert!(
        host.take()
            .iter()
            .all(|call| !call.starts_with("hang note")),
        "it's behind"
    );
    host.behind.set(false);
    shell.update(cx, |shell, cx| shell.became_key(cx));
    assert!(host.take().contains(&"hang note 10 shown".to_string()));
    host.behind.set(true);
    report(&shell, cx, 10, Trouble::Fine);
    assert_eq!(host.take(), ["hang note 10 hidden"]);
}
