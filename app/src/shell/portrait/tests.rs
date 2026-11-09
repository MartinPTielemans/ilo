use std::rc::Rc;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

use super::PortraitBackground;
use crate::capture::portrait::Background;
use crate::shell::{Command, Folders, Shell, bind_keys, recorder::Recorder};

fn window(
    desktop: bool,
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    host.desktop_picture.set(desktop);
    let browser = host.clone();
    cx.update(bind_keys);
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

/// What the shell asked of Portrait Mode, leaving out the rest it asked.
fn captured(host: &Recorder) -> Vec<String> {
    (host.take().into_iter())
        .filter(|call| call.starts_with("capture portrait"))
        .map(|call| {
            call.rsplit_once(" on ")
                .map_or(call.clone(), |(_, on)| on.into())
        })
        .collect()
}

fn chosen(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Background> {
    shell.read_with(cx, |shell, _| shell.portrait_card.map(|(_, chosen)| chosen))
}

/// Capture in Portrait Mode asks on the card, whose swatch chooses the background and whose
/// Capture captures the page on it; the next card starts on the one chosen. Escape closes it.
#[gpui::test]
fn the_card_chooses_the_background_and_captures_on_it(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(false, cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::CapturePortrait, "ui", cx)
    });
    cx.run_until_parked();
    // Without the desktop's picture, white is first.
    assert_eq!(chosen(&shell, cx), Some(Background::White));
    assert_eq!(
        captured(&host),
        Vec::<String>::new(),
        "it captured before asking"
    );

    let colour = crate::tokens::theme_picker::SWATCHES[3];
    let swatch = shell.read_with(cx, |shell, _| shell.portrait_swatches_drawn[4].get());
    let swatch = swatch.expect("the card drew no swatch for the theme's fourth colour");
    cx.simulate_click(swatch.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(chosen(&shell, cx), Some(Background::Colour(colour)));

    // Capture is the right of the buttons along the card's bottom.
    let card = shell.read_with(cx, |shell, _| shell.portrait_card_drawn.get());
    let card = card.expect("Capture in Portrait Mode opened no card");
    let bottom = card.origin.y + card.size.height;
    let capture = point(card.origin.x + card.size.width * 0.75, bottom - px(26.));
    cx.simulate_click(capture, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(captured(&host), [Background::Colour(colour).name()]);
    assert_eq!(chosen(&shell, cx), None, "Capture left the card open");
    let kept = cx.update(|_, cx| cx.try_global::<PortraitBackground>().copied());
    assert_eq!(kept, Some(PortraitBackground(Background::Colour(colour))));

    shell.update(cx, |shell, cx| {
        shell.run(Command::CapturePortrait, "ui", cx)
    });
    cx.run_until_parked();
    assert_eq!(chosen(&shell, cx), Some(Background::Colour(colour)));
    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(chosen(&shell, cx), None, "escape left the card open");
    assert_eq!(captured(&host), Vec::<String>::new());
}

/// The desktop's picture comes first where the host reads one, and a kept one where it doesn't
/// falls back to white. Capture's pane is the page's.
#[gpui::test]
fn the_desktops_picture_is_first_where_theres_one(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(true, cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::CapturePortrait, "ui", cx)
    });
    assert_eq!(chosen(&shell, cx), Some(Background::Desktop));
    shell.update(cx, |shell, cx| {
        shell.run(Command::CapturePortrait, "ui", cx)
    });
    assert_eq!(
        chosen(&shell, cx),
        None,
        "the command didn't close its card"
    );

    shell.update(cx, |shell, cx| {
        shell.run(Command::CapturePortraitNow, "ui", cx)
    });
    let frame = shell
        .read_with(cx, |shell, _| shell.pane_frame(10))
        .unwrap();
    let calls = host.take();
    let call = calls
        .iter()
        .find(|call| call.starts_with("capture portrait"));
    let (x, y) = (f32::from(frame.0.origin.x), f32::from(frame.0.origin.y));
    let at = format!("capture portrait 10 at {x} {y} ");
    assert!(call.is_some_and(|call| call.starts_with(&at)), "{calls:?}");
    assert!(call.unwrap().ends_with(" on desktop"), "{calls:?}");

    host.desktop_picture.set(false);
    shell.update(cx, |shell, cx| {
        shell.run(Command::CapturePortrait, "ui", cx)
    });
    assert_eq!(chosen(&shell, cx), Some(Background::White));
}

/// ⌥ on Site Controls' Capture Full Page captures on the background kept, with no card.
#[gpui::test]
fn portrait_mode_now_captures_without_asking(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(false, cx);
    cx.update(|_, cx| cx.set_global(PortraitBackground(Background::Colour(0x7eb6d5))));
    shell.update(cx, |shell, cx| {
        shell.run(Command::CapturePortraitNow, "ui", cx)
    });
    assert_eq!(captured(&host), ["#7eb6d5"]);
    assert_eq!(chosen(&shell, cx), None);
}
