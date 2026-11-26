//! First run's exit (first-run v9 §10) and the hint it leaves under the landing field.

use std::rc::Rc;

use gpui::{
    Entity, EntityInputHandler as _, Modifiers, TestAppContext, VisualTestContext, point, px, size,
};

use super::{END_FADE, Step, card, first_window, kept, kept_dir, later, on_to, started, step};
use crate::shell::browser::Browser as _;
use crate::shell::recorder::Recorder;
use crate::shell::{Command, Shell};
use crate::tokens::shell::onboarding::card as t;
use crate::tokens::shell::onboarding::exit as x;

/// The card at its last step, its lines heard from here.
fn at_the_end(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    host.take();
    crate::heard::listen();
    (shell, cx, host)
}

/// How many of the heard lines are `line`.
fn heard(lines: &[String], line: &str) -> usize {
    lines.iter().filter(|each| *each == line).count()
}

/// What the host was asked to shape first run's window to, in order: width, height, radius.
fn shapes(said: &[String]) -> Vec<(f32, f32, f32)> {
    said.iter()
        .filter_map(|said| said.strip_prefix("first run shaped "))
        .map(|shape| {
            let mut each = shape.split(' ').map(|n| n.parse::<f32>().unwrap());
            (each.next().unwrap(), each.next().unwrap(), each.next().unwrap())
        })
        .collect()
}

const ASKED: &str = "first run asked to grow";

/// Start Browsing asks the host once, at the press, whether it grows first run's window, and only
/// at the exit's end says first run is done, once, closes first run's window, opens the new tab
/// and puts up the hint, in that order.
#[gpui::test]
fn the_exit_ends_once_and_in_order(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(host.take(), [ASKED], "asked at the press, before any of the exit is drawn");
    later(&shell, cx, x::CONTENT / 2.);
    assert_eq!(step(&shell, cx), Some(Step::Done), "the card still shows");
    later(&shell, cx, x::CONTENT);
    assert!(host.command_bars.borrow().is_empty(), "not while it fades");
    assert!(shell.read_with(cx, |shell, _| !shell.in_hint));
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    let said = host.take();
    assert!(said.contains(&"first run ended".to_string()), "{said:?}");
    assert!(!said.contains(&ASKED.to_string()), "asked once");
    assert_eq!(host.command_bars.borrow().len(), 1, "the new tab's bar is up");
    assert!(shell.read_with(cx, |shell, _| shell.in_hint));
    let lines = crate::heard::take();
    assert_eq!(heard(&lines, "browser: onboarding done"), 1);
    let done = lines.iter().position(|line| line == "browser: onboarding done");
    let new_tab = (lines.iter()).position(|line| line == "browser: command new-tab via onboarding");
    assert!(new_tab.is_some() && done < new_tab, "done comes first: {lines:?}");
}

/// Where the host doesn't grow the window, the card fades whole over the cross-fade once its
/// content has gone, at its own size.
#[gpui::test]
fn without_a_grow_the_card_cross_fades(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    cx.simulate_keystrokes("enter");
    later(&shell, cx, x::CONTENT + x::CROSS_FADE / 2.);
    assert_eq!(card(&shell, cx, |o| o.grows), Some(false));
    assert!(
        (shapes(&host.take()).iter()).all(|&(w, h, _)| (w, h) == (t::WIDTH, t::HEIGHT)),
        "the card kept its size"
    );
    later(&shell, cx, x::CROSS_FADE);
    assert_eq!(step(&shell, cx), None);
}

/// Where the host grows it, the card grows from its own size to the window's as the window is
/// on each frame, shaped each frame, centred in it, and the exit ends as the grow does.
#[gpui::test]
fn with_a_grow_the_card_grows_to_its_window(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    host.grows_first_run.set(true);
    cx.simulate_keystrokes("enter");
    // The host grows the window a turn after it answers.
    cx.simulate_resize(size(px(1200.), px(800.)));
    later(&shell, cx, x::CONTENT);
    assert_eq!(card(&shell, cx, |o| o.grows), Some(true));
    later(&shell, cx, x::GROW / 4.);
    later(&shell, cx, x::GROW / 4.);
    let grown = shapes(&host.take());
    assert!(grown.len() >= 2, "shaped each frame: {grown:?}");
    assert!(grown.windows(2).all(|pair| pair[1].0 >= pair[0].0), "{grown:?}");
    let (w, h, _) = *grown.last().unwrap();
    assert!(w > t::WIDTH && w < 1200. && h > t::HEIGHT && h < 800., "{grown:?}");
    later(&shell, cx, x::GROW);
    assert_eq!(step(&shell, cx), None);
}

/// With Reduce Motion the exit is the cross-fade alone, and the host is never asked to grow.
#[gpui::test]
fn reduce_motion_never_asks_for_a_grow(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    host.grows_first_run.set(true);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    cx.simulate_keystrokes("enter");
    later(&shell, cx, x::CROSS_FADE / 2.);
    assert_eq!(step(&shell, cx), Some(Step::Done));
    later(&shell, cx, x::CROSS_FADE);
    assert_eq!(step(&shell, cx), None);
    assert!(!host.take().contains(&ASKED.to_string()));
}

/// While the exit plays, a key or a click reaches no button of the card and opens nothing.
#[gpui::test]
fn the_exit_takes_no_key_or_click(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    let back = super::placed("back", cx).expect("Back on the last step");
    cx.simulate_keystrokes("enter");
    later(&shell, cx, x::CONTENT / 2.);
    cx.simulate_keystrokes("secondary-[ enter escape");
    cx.simulate_click(back.center(), Modifiers::none());
    assert_eq!(step(&shell, cx), Some(Step::Done), "nothing went back");
    assert!(host.command_bars.borrow().is_empty());
    later(&shell, cx, END_FADE);
    assert_eq!(heard(&crate::heard::take(), "browser: onboarding done"), 1);
    assert_eq!(host.command_bars.borrow().len(), 1, "one bar, the end's");
}

/// A press on the card away from its controls moves the window, but not while it leaves, when
/// the card may be the whole window. The test window panics at a move, so this one would fail.
#[gpui::test]
fn the_exit_moves_no_window(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    host.grows_first_run.set(true);
    cx.simulate_keystrokes("enter");
    cx.simulate_resize(size(px(1200.), px(800.)));
    later(&shell, cx, x::CONTENT + x::GROW / 2.);
    cx.simulate_mouse_down(point(px(6.), px(6.)), gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(point(px(6.), px(6.)), gpui::MouseButton::Left, Modifiers::none());
    later(&shell, cx, x::GROW);
    assert_eq!(step(&shell, cx), None);
}

/// First run has ended as Start Browsing is pressed: ⌘Q in the exit leaves no step to resume,
/// and the exit still ends, with no new tab for the quit.
#[gpui::test]
fn a_quit_in_the_exit_leaves_first_run_ended(cx: &mut TestAppContext) {
    let dir = kept_dir(cx);
    let (shell, cx, host) = at_the_end(cx);
    assert!(kept(&dir).is_some(), "the last step is kept until it's left");
    cx.simulate_keystrokes("enter");
    later(&shell, cx, x::CONTENT / 2.);
    crate::shell::quit_goes_ahead();
    assert_eq!(kept(&dir), None, "nothing to resume, and no exit's number");
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert!(host.command_bars.borrow().is_empty());
    assert_eq!(heard(&crate::heard::take(), "browser: onboarding done"), 1);
}

/// The card ended, its hint up.
fn ended(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = at_the_end(cx);
    cx.simulate_keystrokes("enter");
    later(&shell, cx, x::CONTENT);
    later(&shell, cx, END_FADE);
    assert!(shell.read_with(cx, |shell, _| shell.in_hint));
    (shell, cx, host)
}

/// The hint goes at the first key, not at a modifier alone, and nothing brings it back.
#[gpui::test]
fn the_hint_goes_at_the_first_key_for_good(cx: &mut TestAppContext) {
    let (shell, cx, _) = ended(cx);
    cx.simulate_keystrokes("shift");
    assert!(shell.read_with(cx, |shell, _| shell.in_hint), "not for ⇧ alone");
    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| !shell.in_hint && shell.hint_keys.is_none()));
    shell.update(cx, |shell, cx| shell.run(Command::NewTab, "menu", cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| !shell.in_hint));
}

/// An input method's first mark in a bar's field takes the hint away though no key observer
/// hears it, and nothing of what it marked is logged.
#[gpui::test]
fn a_compositions_first_mark_takes_the_hint(cx: &mut TestAppContext) {
    let (shell, cx, host) = ended(cx);
    let bar = cx.update(|window, cx| host.command_bar_in_place(window, cx));
    let field = bar.read_with(cx, |bar, _| bar.field());
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_and_mark_text_in_range(None, "ぴ", None, window, cx)
        })
    });
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| !shell.in_hint));
    assert!(
        !crate::heard::take().iter().any(|line| line.contains('ぴ')),
        "a line named what was marked"
    );
}

/// Closing ⌘T's bar the end left up takes the hint with it, as it was under the bar's field.
#[gpui::test]
fn the_hint_goes_with_the_bar(cx: &mut TestAppContext) {
    let (shell, cx, _) = ended(cx);
    shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
    assert!(shell.read_with(cx, |shell, _| !shell.in_hint));
}

/// The hint says the new tab's own key, as the user has it bound, and none where it has none.
#[gpui::test]
fn the_hint_says_the_new_tabs_own_key(cx: &mut TestAppContext) {
    let (shell, cx, _) = ended(cx);
    let key = cx.update(|_, cx| crate::shell::keys::user_keys(cx).label(Command::NewTab));
    let drawn = shell.update(cx, |shell, cx| shell.landing_hint(0., 600., cx).is_some());
    assert_eq!(drawn, key.is_some());
    assert!(drawn, "⌘T is bound by default");
}
