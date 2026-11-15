use std::rc::Rc;

use gpui::{Entity, FontWeight, TestAppContext, VisualTestContext, WindowTextSystem};

use super::{EXTENSIONS, TURN_BACK_ON};
use crate::shell::{Folders, Shell, recorder::Recorder};
use crate::tokens::shell::task::CHIP_X;
use crate::tokens::shell::{FOOTER_HEIGHT, SIDEBAR_MIN, UPDATE_BANNER_GAP};
use crate::tokens::text;
use crate::tooltip::text_width;
use crate::updates::Stage;

/// A window with one Space and two tabs, drawn, on a host whose answer to whether this launch is a
/// safe start a test changes.
fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    host.ad_blocking.set(true);
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![1, 2],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(1, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

/// The host says this launch is a safe start, and the window draws again.
fn safe(shell: &Entity<Shell>, cx: &mut VisualTestContext, host: &Recorder) {
    host.safe_start.set(true);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

fn room(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> f32 {
    shell.update(cx, |shell, _| shell.safe_start_room())
}

fn list_bottom(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> f32 {
    shell.update(cx, |shell, _| shell.list_bottom())
}

/// A normal launch has no card and takes nothing from the foot.
#[gpui::test]
fn a_normal_launch_shows_no_card(cx: &mut TestAppContext) {
    let (shell, cx, _) = shell(cx);
    assert_eq!(room(&shell, cx), 0.);
    assert_eq!(
        shell.update(cx, |shell, _| shell.over_safe_start()),
        FOOTER_HEIGHT
    );
}

/// A safe start shows the card over the footer, and the list ends over it, as does an update's
/// banner over it.
#[gpui::test]
fn a_safe_start_shows_the_card_and_the_foot_stands_over_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let bare = list_bottom(&shell, cx);
    safe(&shell, cx, &host);
    let room = room(&shell, cx);
    assert!(room > UPDATE_BANNER_GAP, "no room for the card: {room}");
    assert_eq!(list_bottom(&shell, cx), bare + room);
    cx.update(|_, cx| crate::updates::changed(Stage::Ready(Some("0.1.5".into())), cx));
    cx.run_until_parked();
    let banner = shell.update(cx, |shell, _| shell.update_banner_room());
    assert!(banner > 0., "no banner");
    assert_eq!(list_bottom(&shell, cx), bare + room + banner);
}

/// Its words wrap in a narrower sidebar, and the card grows to them rather than cutting them.
#[gpui::test]
fn a_narrower_sidebar_wraps_its_words_and_grows_the_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    safe(&shell, cx, &host);
    let wide = room(&shell, cx);
    shell.update(cx, |shell, cx| {
        shell.sidebar_width = SIDEBAR_MIN;
        cx.notify();
    });
    cx.run_until_parked();
    let narrow = room(&shell, cx);
    assert!(
        narrow > wide,
        "{narrow} at the narrowest, {wide} at the default"
    );
}

/// At the narrowest sidebar neither chip's label is cut: each chip is drawn as wide as its whole
/// label and its padding.
#[gpui::test]
fn at_the_narrowest_sidebar_neither_chip_is_cut(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    safe(&shell, cx, &host);
    shell.update(cx, |shell, cx| {
        shell.sidebar_width = SIDEBAR_MIN;
        cx.notify();
    });
    cx.run_until_parked();
    shell.update(cx, |shell, cx| {
        let system = WindowTextSystem::new(cx.text_system().clone());
        for (label, drawn) in [
            (TURN_BACK_ON, &shell.safe_start_button_drawn),
            (EXTENSIONS, &shell.safe_start_manage_drawn),
        ] {
            let drawn = drawn
                .get()
                .unwrap_or_else(|| panic!("{label} wasn't drawn"));
            let whole = text_width(&label.into(), text::SMALL, FontWeight::MEDIUM, &system);
            let width = f32::from(drawn.size.width);
            assert!(
                width + 0.5 >= whole + 2. * CHIP_X,
                "{label} is {width} wide at the narrowest, its label {whole} and padding {}",
                2. * CHIP_X
            );
        }
    });
}

/// Turn Back On restarts, which brings the windows back in a normal launch.
#[gpui::test]
fn turn_back_on_restarts(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    safe(&shell, cx, &host);
    shell.update(cx, |shell, cx| shell.turn_back_on(cx));
    assert_eq!(host.take(), ["restart"]);
}
