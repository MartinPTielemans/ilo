//! The card of a window with no page, on a host that records what the shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use crate::command_bar::PLACEHOLDER_TEXT;
use crate::shell::{
    Folders, Page, ROW_HEIGHT, SECONDARY, Shell, Slot, empty::FADE, recorder::Recorder,
};

/// A drawn shell showing one Space with Today tabs 1 and 2, and no tab shown, as a fresh window.
fn window_with_no_tab(
    cx: &mut TestAppContext,
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
        shell.set_tabs(
            1,
            Vec::new(),
            vec![1, 2],
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// The middle of the card, where a click is as far from the sidebar as from the window's edge.
fn middle(shell: &Entity<Shell>, cx: &VisualTestContext) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport).0.center())
}

/// The line is there with no tab selected, from nothing to all of it over the fade, and gone with
/// the next frame once a tab is selected, without waiting for the fade to end.
#[gpui::test]
fn the_line_shows_only_while_no_tab_is_selected(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    let start = Instant::now() - Duration::from_secs(10);
    shell.update(cx, |shell, _| shell.empty_since = Some(start));
    let opacity =
        |cx: &VisualTestContext, at| shell.read_with(cx, |shell, _| shell.empty_opacity(at));
    let fade = Duration::from_secs_f32(FADE);
    assert!(opacity(cx, start) < 1e-3, "it starts from nothing");
    let halfway = opacity(cx, start + fade / 2);
    assert!(
        halfway > 0. && halfway < 1.,
        "halfway through the fade it's at {halfway}"
    );
    assert!(opacity(cx, start + fade) > 0.999);

    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.empty_since.is_none()));
    assert_eq!(
        opacity(cx, Instant::now()),
        0.,
        "a page shown has no line, not a fading one"
    );

    // The card is empty again once the page goes, and the fade starts over.
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.empty_since.is_some()));
}

/// A click anywhere on the empty card opens the command bar as ⌘T does, centred on the window.
#[gpui::test]
fn a_click_on_the_empty_card_opens_the_command_bar(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    let frame = shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport).0);
    let corner = Point::new(
        frame.right() - gpui::px(20.),
        frame.bottom() - gpui::px(20.),
    );
    for at in [middle(&shell, cx), corner] {
        host.bar_open.set(false);
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(host.command_bars.take(), [None], "a click at {at:?}");
    }
    assert!(
        shell.read_with(cx, |shell, _| shell.new_tab),
        "New Tab takes the selected plate"
    );
}

/// With a tab selected the same place is the page's, so the card takes no click.
#[gpui::test]
fn a_page_shown_takes_the_card_away(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    assert!(host.command_bars.take().is_empty());
}

/// With the bar open, a click on the card only closes it: the host has closed the bar at the press,
/// and the app reports so after, so the release mustn't open it again. Once the report is in, the
/// next click opens it.
#[gpui::test]
fn a_click_on_the_card_with_the_bar_open_only_closes_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    let at = middle(&shell, cx);
    cx.simulate_click(at, Modifiers::none());
    assert_eq!(host.command_bars.take().len(), 1);

    cx.simulate_click(at, Modifiers::none());
    assert!(
        host.command_bars.take().is_empty(),
        "the click opened the bar it closed"
    );

    // The app's report, which the click's press came before.
    host.bar_open.set(false);
    shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
    cx.simulate_click(at, Modifiers::none());
    assert_eq!(host.command_bars.take().len(), 1, "the next click opens it");
}

/// The sidebar's New Tab row and its URL field open the bar as the card does, so a click on either
/// with the bar open closes it too.
#[gpui::test]
fn the_other_targets_that_open_the_bar_only_close_it_too(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    let (new_tab, field) = shell.read_with(cx, |shell, _| {
        let row = shell.list_top_with(0) + shell.layout()[&Slot::NewTab] + ROW_HEIGHT / 2.;
        let field = shell
            .url_field_bounds()
            .expect("the URL field is drawn")
            .center();
        (point(px(100.), px(row)), field)
    });
    for at in [new_tab, field] {
        host.bar_open.set(false);
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(
            host.command_bars.take().len(),
            1,
            "a click at {at:?} opens it"
        );
        cx.simulate_click(at, Modifiers::none());
        assert!(
            host.command_bars.take().is_empty(),
            "a second click at {at:?} opened it again"
        );
    }
}

/// With no page the URL field says what typing there does, in the command bar's words and fading
/// in with the card's mark; a page shown puts its host there, and closing it brings the words back.
#[gpui::test]
fn the_url_field_says_what_to_type_only_with_no_page(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "https://www.iana.org/domains".into(),
            ..Page::default()
        };
        shell.set_page(1, page, cx);
    });
    let words = |cx: &VisualTestContext| {
        shell.read_with(cx, |shell, cx| {
            let (words, color) = shell.url_field_words(&shell.url(shell.selected), cx);
            (words.to_string(), color.a)
        })
    };
    let now = cx.executor().now();
    shell.update(cx, |shell, _| shell.empty_since = Some(now));
    let (said, alpha) = words(cx);
    assert_eq!(said, PLACEHOLDER_TEXT);
    assert!(
        alpha < 0.1,
        "it starts from nothing with the mark, at {alpha}"
    );
    shell.update(cx, |shell, _| {
        shell.empty_since = Some(now - Duration::from_secs(1))
    });
    let (_, faded_in) = words(cx);
    // Dimmer than a host where the window lets it be, and never under what text needs to read.
    let (floor, host) = shell.read_with(cx, |shell, _| {
        (shell.palette.text_floor, shell.palette.text(SECONDARY).a)
    });
    assert!(
        faded_in > floor.max(0.3) - 0.01 && faded_in <= host,
        "reads, no brighter than a host, at {faded_in}"
    );

    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    let (said, alpha) = words(cx);
    assert_eq!(said, "iana.org");
    assert!(
        alpha > faded_in,
        "the host is brighter than the placeholder"
    );

    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    assert_eq!(words(cx).0, PLACEHOLDER_TEXT);
}

/// The first run's welcome stays until a page shows, and an empty card after that has the Space's
/// name and line again, not the welcome.
#[gpui::test]
fn the_welcome_shows_once(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_welcome(cx));
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.welcome),
        "the empty card has the welcome"
    );

    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    assert!(
        !shell.read_with(cx, |shell, _| shell.welcome),
        "once a page showed, it's gone"
    );
}

/// A click on the welcome, in the card's middle, opens the command bar, as ⌘T and its button do.
#[gpui::test]
fn the_welcome_opens_a_new_tab(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_welcome(cx));
    cx.run_until_parked();
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    assert_eq!(host.command_bars.take(), [None]);
}
