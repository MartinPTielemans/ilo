use std::rc::Rc;

use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::{Pair, PairKind};
use crate::{
    shell::{Folders, Shell, recorder::Recorder},
    tokens::shell::permission::{BUTTON_HEIGHT, PADDING},
};

/// One Space with tabs 10 and 11, tab 10 shown, in a window.
fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
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
    host.take();
    (shell, cx, host)
}

fn pair(kind: PairKind) -> Option<Pair> {
    Some(Pair {
        device: "Polar H10".into(),
        kind,
    })
}

/// What the browser was asked about pairing, without the card's other calls.
fn asked(host: &Recorder) -> Vec<String> {
    (host.take().into_iter())
        .filter(|asked| asked.contains("pair"))
        .collect()
}

/// The middle of the card's Pair button, its last row.
fn pair_button(shell: &Shell) -> Point<Pixels> {
    let card = shell
        .pair_drawn
        .get()
        .expect("the pairing card wasn't drawn");
    let foot = card.origin.y + card.size.height - px(1. + PADDING);
    point(
        card.origin.x + px(PADDING + 20.),
        foot - px(BUTTON_HEIGHT / 2.),
    )
}

/// A device that asks for its PIN gets the field, which takes the keyboard. Return pairs only once
/// the PIN has a digit, as Chrome's OK, and sends it as typed; the card goes.
#[gpui::test]
fn return_pairs_once_the_pin_has_a_digit(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_pair(10, pair(PairKind::Pin), cx));
    cx.run_until_parked();
    assert!(host.take().contains(&"focus ui".to_string()));
    cx.simulate_input("abc");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(asked(&host), Vec::<String>::new());
    cx.simulate_input("123");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(asked(&host), ["pair 10 abc123"]);
    shell.read_with(cx, |shell, _| {
        assert!(shell.pairing().is_none(), "the card stayed after pairing")
    });
}

/// Esc in the PIN field cancels, as Chrome's Cancel.
#[gpui::test]
fn escape_cancels(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_pair(10, pair(PairKind::Pin), cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(asked(&host), ["cancel pair 10"]);
}

/// A device that asks only whether to pair, or whether its passkey matches, has no field, and Pair
/// agrees, with no PIN.
#[gpui::test]
fn pair_agrees_without_a_pin(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_pair(10, pair(PairKind::Passkey("123456".into())), cx)
    });
    cx.run_until_parked();
    let asks = shell.read_with(cx, |shell, _| shell.pairing().expect("no card").pair.asks());
    assert_eq!(
        asks,
        "Please confirm that 123456 is the passkey displayed on Bluetooth device Polar H10."
    );
    let button = shell.read_with(cx, |shell, _| pair_button(shell));
    cx.simulate_click(button, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(asked(&host), ["pair 10"]);
}

/// Another tab's prompt waits for its tab, as its chooser would, and the core's end takes it.
#[gpui::test]
fn another_tabs_prompt_waits_for_it(cx: &mut TestAppContext) {
    let (shell, cx, _) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_pair(11, pair(PairKind::Confirm), cx)
    });
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert!(shell.pairing().is_none()));
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert!(shell.pairing().is_some()));
    shell.update(cx, |shell, cx| shell.set_pair(11, None, cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert!(shell.pairing().is_none()));
}
