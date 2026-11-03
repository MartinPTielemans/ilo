//! The note on how to leave full screen, which the window shows as it goes full screen and again
//! for each Space shown there, and where the page goes as a page's full screen ends after the
//! window left it, on a host that records what the shell asks.

use std::rc::Rc;

use gpui::{
    AppContext, Bounds, Entity, Pixels, Size, TestAppContext, VisualTestContext, point, px, size,
};

use crate::shell::{Command, Folders, Shell, recorder::Recorder};

/// Spaces 1 and 2, with tabs 10 and 20, Space 1 shown.
fn two_spaces(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into()),
                (2, "Space 2".into(), None, 0, "".into(), "".into(), "".into()),
            ],
            cx,
        );
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
    });
    (shell, host)
}

/// What the shell asked of the note since the last time.
fn notes(host: &Recorder) -> Vec<String> {
    host.take().into_iter().filter(|call| call.starts_with("full screen note")).collect()
}

#[gpui::test]
fn the_note_shows_again_for_each_space_in_full_screen(cx: &mut TestAppContext) {
    let (shell, host) = two_spaces(cx);
    notes(&host);
    shell.update(cx, |shell, cx| shell.set_full_screen(true, cx));
    assert_eq!(notes(&host), ["full screen note shown"]);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    assert_eq!(notes(&host), ["full screen note shown"]);
    // The same Space again isn't another.
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    assert!(notes(&host).is_empty());
    shell.update(cx, |shell, cx| shell.set_full_screen(false, cx));
    assert_eq!(notes(&host), ["full screen note hidden"]);
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    assert!(notes(&host).is_empty());
}

#[gpui::test]
fn escape_leaves_full_screen_when_nothing_loads(cx: &mut TestAppContext) {
    let (shell, host) = two_spaces(cx);
    shell.update(cx, |shell, cx| shell.set_full_screen(true, cx));
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    assert_eq!(host.take(), ["window button zoom"]);
    shell.update(cx, |shell, cx| {
        shell.set_full_screen(false, cx);
        shell.run(Command::Stop, "ui", cx);
    });
    assert!(!host.take().contains(&"window button zoom".to_string()));
}

const WINDOWED: Size<Pixels> = size(px(1000.), px(700.));
const SCREEN: Size<Pixels> = size(px(1440.), px(900.));

/// A windowed window showing tab 10, 1000 × 700, and its host.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![(1, "Space 1".into(), None, 0, "".into(), "".into(), "".into())], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
    });
    cx.simulate_resize(WINDOWED);
    cx.run_until_parked();
    (shell, host, cx)
}

/// AppKit takes the window into full screen or out, to its size there, but doesn't say so yet.
fn window_goes(full_screen: bool, cx: &mut VisualTestContext) {
    cx.update(|window, _| {
        if window.is_fullscreen() != full_screen {
            window.toggle_fullscreen();
        }
    });
    cx.simulate_resize(if full_screen { SCREEN } else { WINDOWED });
}

/// Linux's window manager takes the window into full screen or out, which the shell hears of as
/// the window resizes.
fn window_manager_takes(
    full_screen: bool,
    shell: &Entity<Shell>,
    host: &Recorder,
    cx: &mut VisualTestContext,
) {
    host.full_screen.set(full_screen);
    cx.simulate_resize(if full_screen { SCREEN } else { WINDOWED });
    shell.update(cx, |shell, cx| shell.bounds_changed(cx));
}

/// The core says the page went full screen, or left.
fn page_goes(full_screen: bool, shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update_in(cx, |shell, window, cx| shell.set_page_fullscreen(full_screen, window, cx));
}

/// Where the page goes at the window's size: None while it's held for the window's resize, else
/// whether it fills the window or is in the card at the sidebar's edge.
fn page(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<&'static str> {
    let viewport = cx.update(|window, _| window.viewport_size());
    shell.update(cx, |shell, _| {
        let (frame, _) = shell.window_page_frame(viewport)?;
        let edge = px(shell.width());
        Some(if frame == Bounds::new(point(px(0.), px(0.)), viewport) {
            "fills"
        } else if shell.card_left() == shell.width() && frame.origin.x == edge {
            "at the sidebar"
        } else {
            "elsewhere"
        })
    })
}

/// The page goes full screen in the windowed window, which follows it.
fn page_takes_the_window(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    page_goes(true, shell, cx);
    assert_eq!(page(shell, cx), None, "the page moved before the window went full screen");
    window_goes(true, cx);
    shell.update(cx, |shell, cx| shell.set_full_screen(true, cx));
    assert_eq!(page(shell, cx), Some("fills"));
}

#[gpui::test]
fn a_page_leaving_after_its_window_left_full_screen_brings_the_card_back(cx: &mut TestAppContext) {
    let (shell, _, cx) = window(cx);
    page_takes_the_window(&shell, cx);
    // The green button, ⌃⌘F or the display going: the window is out before the core says the page
    // is, so no resize is coming to lay the page out.
    window_goes(false, cx);
    shell.update(cx, |shell, cx| shell.set_full_screen(false, cx));
    page_goes(false, &shell, cx);
    assert_eq!(page(&shell, cx), Some("at the sidebar"));

    // The page's full screen takes the window again, and its own exit brings both back.
    page_takes_the_window(&shell, cx);
    page_goes(false, &shell, cx);
    assert_eq!(page(&shell, cx), None, "the page moved before the window's resize");
    window_goes(false, cx);
    assert_eq!(page(&shell, cx), Some("at the sidebar"));
}

#[gpui::test]
fn a_page_leaving_as_its_window_starts_out_waits_for_the_resize(cx: &mut TestAppContext) {
    let (shell, _, cx) = window(cx);
    page_takes_the_window(&shell, cx);
    // The core lets the page go as AppKit starts taking the window out, while it's still full
    // screen, so the page stays put until the window's resize lays it out, once.
    page_goes(false, &shell, cx);
    assert_eq!(page(&shell, cx), None, "the page moved before the window's resize");
    window_goes(false, cx);
    assert_eq!(page(&shell, cx), Some("at the sidebar"));
    shell.update(cx, |shell, cx| shell.set_full_screen(false, cx));

    // From there, the page's full screen takes the window again.
    page_takes_the_window(&shell, cx);
}

#[gpui::test]
fn the_note_waits_to_hear_whether_a_page_took_the_window(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    notes(&host);
    window_manager_takes(true, &shell, &host, cx);
    cx.run_until_parked();
    assert_eq!(notes(&host), ["full screen note shown"]);
    window_manager_takes(false, &shell, &host, cx);
    notes(&host);
    // Chrome on Windows takes the window full screen for a page before the core says it asked.
    window_manager_takes(true, &shell, &host, cx);
    page_goes(true, &shell, cx);
    cx.run_until_parked();
    assert!(notes(&host).is_empty(), "a page's full screen showed the window's note");
}

#[gpui::test]
fn a_page_whose_window_comes_back_first_goes_to_the_card_with_the_resize(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    page_goes(true, &shell, cx);
    window_manager_takes(true, &shell, &host, cx);
    assert_eq!(page(&shell, cx), Some("fills"));
    // The page's own exit, which the window manager can finish before the core says the page left:
    // the resize lays the page out in the card, and the core's word leaves it there.
    window_manager_takes(false, &shell, &host, cx);
    assert_eq!(page(&shell, cx), Some("at the sidebar"));
    page_goes(false, &shell, cx);
    assert_eq!(page(&shell, cx), Some("at the sidebar"));

    // From there, the page's full screen takes the window again.
    page_goes(true, &shell, cx);
    assert_eq!(page(&shell, cx), None, "the page moved before the window went full screen");
}
