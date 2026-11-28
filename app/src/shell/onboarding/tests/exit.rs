//! First run's exit (first-run v9 §10) and the hint it leaves under the landing field.

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    AppContext as _, Entity, EntityInputHandler as _, Modifiers, TestAppContext,
    VisualContext as _, VisualTestContext, point, px, size,
};

use super::super::card::KNOTS;
use super::super::exit::{DRAWN, Plate, frame};
use super::{END_FADE, Step, card, first_window, kept, kept_dir, later, on_to, started, step};
use crate::shell::browser::Browser as _;
use crate::shell::recorder::Recorder;
use crate::shell::{Command, Shell};
use crate::theme::Appearance;
use crate::tokens::shell::onboarding::card as t;
use crate::tokens::shell::onboarding::exit as x;

/// The card at its last step, its lines heard from here.
fn at_the_end(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    host.take();
    crate::heard::listen();
    DRAWN.take();
    (shell, cx, host)
}

/// The window the host grew first run's to in these tests.
const GROWN: (f32, f32) = (1200., 800.);

/// The last step's Start Browsing pressed where the host grows first run's window, and grown.
fn pressed_and_grown(shell: &Entity<Shell>, cx: &mut VisualTestContext, host: &Recorder) {
    host.grows_first_run.set(true);
    cx.simulate_keystrokes("enter");
    // The host grows the window as it answers.
    cx.simulate_resize(size(px(GROWN.0), px(GROWN.1)));
    later(shell, cx, x::CONTENT);
}

/// The plates the layer has drawn since the press, none for a frame with no card.
fn plates() -> Vec<Option<Plate>> {
    DRAWN.with_borrow(|drawn| {
        drawn
            .iter()
            .map(|each| each.map(|(plate, ..)| plate))
            .collect()
    })
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
            (
                each.next().unwrap(),
                each.next().unwrap(),
                each.next().unwrap(),
            )
        })
        .collect()
}

/// Start Browsing asks the host once, at the press, whether it grows first run's window, and only
/// at the exit's end says first run is done, once, closes first run's window, opens the new tab
/// and puts up the hint, in that order.
#[gpui::test]
fn the_exit_ends_once_and_in_order(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(
        host.asked_to_grow.get(),
        1,
        "asked at the press, before any of it is drawn"
    );
    later(&shell, cx, x::CONTENT / 2.);
    assert_eq!(step(&shell, cx), Some(Step::Done), "the card still shows");
    later(&shell, cx, x::CONTENT);
    assert!(host.command_bars.borrow().is_empty(), "not while it fades");
    assert!(shell.read_with(cx, |shell, _| !shell.in_hint));
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    let said = host.take();
    assert!(said.contains(&"first run ended".to_string()), "{said:?}");
    assert_eq!(host.asked_to_grow.get(), 1, "asked once");
    assert_eq!(
        host.command_bars.borrow().len(),
        1,
        "the new tab's bar is up"
    );
    assert!(shell.read_with(cx, |shell, _| shell.in_hint));
    let lines = crate::heard::take();
    assert_eq!(heard(&lines, "browser: onboarding done"), 1);
    let done = lines
        .iter()
        .position(|line| line == "browser: onboarding done");
    let new_tab = (lines.iter()).position(|line| line == "browser: command new-tab via onboarding");
    assert!(
        new_tab.is_some() && done < new_tab,
        "done comes first: {lines:?}"
    );
}

/// Where the host doesn't grow the window, the card fades whole over the cross-fade alone, and
/// the host is never asked to shape it.
#[gpui::test]
fn without_a_grow_the_card_cross_fades(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    cx.simulate_keystrokes("enter");
    later(&shell, cx, x::CROSS_FADE / 2.);
    assert_eq!(card(&shell, cx, |o| o.grows), Some(false));
    later(&shell, cx, x::CROSS_FADE / 2.);
    assert_eq!(step(&shell, cx), None, "over in the cross-fade's length");
    assert!(shapes(&host.take()).is_empty(), "shaped on a no");
}

/// Where the host grows it, the card grows from its own size to the window's as the window is
/// on each frame, the host's glass shaped after it, ending at the window's bounds and the
/// window's corners, and the exit ends with the grow.
#[gpui::test]
fn with_a_grow_the_card_grows_to_its_window(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    pressed_and_grown(&shell, cx, &host);
    assert_eq!(card(&shell, cx, |o| o.grows), Some(true));
    later(&shell, cx, x::GROW / 4.);
    later(&shell, cx, x::GROW / 4.);
    let grown = shapes(&host.take());
    assert!(grown.len() >= 2, "shaped each frame: {grown:?}");
    assert!(
        grown.windows(2).all(|pair| pair[1].0 >= pair[0].0),
        "{grown:?}"
    );
    let (w, h, _) = *grown.last().unwrap();
    assert!(
        w > t::WIDTH && w < 1200. && h > t::HEIGHT && h < 800.,
        "{grown:?}"
    );
    later(&shell, cx, x::GROW / 2. - 0.001);
    let Some(&Some((_, _, w, h, radius))) = plates().last() else {
        panic!("the card is drawn");
    };
    let [_, to_radius] = x::CORNERS;
    assert!(
        (w - 1200.).abs() < 1. && (h - 800.).abs() < 1. && (radius - to_radius).abs() < 0.1,
        "ends at {w} {h} {radius}"
    );
    assert_eq!(step(&shell, cx), None);
}

/// A grown exit ends once the card is within `rest` of its window on every side, before the
/// grow's curve reaches its length, whose tail moves it less than that.
#[gpui::test]
fn a_grown_exit_ends_at_rest_before_the_curves_length(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    pressed_and_grown(&shell, cx, &host);
    // When the curve brings the card within `rest` of the window, by the exit's own frames.
    let rest = (0..=1000)
        .map(|i| x::CONTENT + x::GROW * i as f32 / 1000.)
        .find(|&at| {
            let (_, _, w, h, _) = frame(at, true, GROWN).plate;
            (GROWN.0 - w).max(GROWN.1 - h) / 2. <= x::REST
        })
        .unwrap();
    assert!(
        rest < x::CONTENT + x::GROW - 0.05,
        "at rest well before the curve's length: {rest}"
    );
    later(&shell, cx, rest - x::CONTENT - 0.05);
    assert_eq!(step(&shell, cx), Some(Step::Done), "still growing");
    later(&shell, cx, 0.052);
    assert_eq!(step(&shell, cx), None, "ended at rest");
}

/// The frame a grown exit ends on still draws the card, the whole window: first run's window
/// stays on screen until the browser window is, and a frame of it with no card is its glass bare.
/// The cross-fade's last frame, the card faded out, draws none.
#[gpui::test]
fn a_grown_exits_last_frame_is_the_grown_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    pressed_and_grown(&shell, cx, &host);
    later(&shell, cx, x::GROW);
    assert_eq!(step(&shell, cx), None, "the exit ended");
    let [_, to_radius] = x::CORNERS;
    let whole = (0., 0., GROWN.0, GROWN.1, to_radius);
    assert_eq!(plates().last(), Some(&Some(whole)), "the frame it ends on");
}

/// First run's end reaches the host with the shell free to draw, so the Mac's host can draw the
/// browser window before it puts it on screen, and the window comes drawn rather than blank.
#[gpui::test]
fn the_browser_window_can_draw_as_first_run_ends(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    let window = cx.window_handle();
    let drew = Rc::new(Cell::new(false));
    let drawn = drew.clone();
    *host.first_run_end.borrow_mut() = Some(Box::new(move |cx| {
        cx.update_window(window, |_, window, cx| window.draw_and_present(cx))
            .unwrap();
        drawn.set(true);
    }));
    pressed_and_grown(&shell, cx, &host);
    later(&shell, cx, x::GROW);
    assert_eq!(step(&shell, cx), None, "the exit ended");
    assert!(drew.get(), "the browser window drew as first run ended");
    assert!(host.take().contains(&"first run ended".to_string()));
}

#[gpui::test]
fn a_cross_fades_last_frame_draws_no_card(cx: &mut TestAppContext) {
    let (shell, cx, _) = at_the_end(cx);
    cx.simulate_keystrokes("enter");
    later(&shell, cx, x::CROSS_FADE);
    assert_eq!(step(&shell, cx), None, "the exit ended");
    assert_eq!(plates().last(), Some(&None));
}

/// As the card grows, the host's glass is given each frame's own shape and the card is drawn in
/// that shape on the same frame, centred in its window: the host commits the shape as it's told it,
/// so the glass goes out with the frame that draws the card in it, never a frame behind it, and the
/// fill and the glass grow together. The frame the exit ends on draws the grown card whole, its
/// glass told the same.
#[gpui::test]
fn the_exits_card_is_drawn_in_its_glass_shape_on_each_frame(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    pressed_and_grown(&shell, cx, &host);
    for _ in 0..5 {
        later(&shell, cx, x::GROW / 8.);
    }
    later(&shell, cx, x::GROW);
    assert_eq!(step(&shell, cx), None, "the exit ended");
    let drawn = DRAWN.with_borrow(|drawn| drawn.clone());
    let frames: Vec<_> = drawn
        .iter()
        .map(|each| each.unwrap_or_else(|| panic!("the card on every frame: {drawn:?}")))
        .collect();
    let mut grew = 0;
    for pair in frames.windows(2) {
        let (_, _, before) = pair[0];
        let ((x, y, w, h, radius), _, glass) = pair[1];
        assert_eq!(
            (w, h, radius),
            glass,
            "the card in its own frame's glass: {drawn:?}"
        );
        if glass.0 > before.0 {
            grew += 1;
            let centred = ((GROWN.0 - w) / 2., (GROWN.1 - h) / 2.);
            assert_eq!((x, y), centred, "centred in its window");
        }
    }
    assert!(grew >= 4, "the glass grew frame by frame: {drawn:?}");
    // The frame it ends on is the grown card and its glass the same, the last the host was told.
    let ((_, _, w, h, radius), _, glass) = *frames.last().unwrap();
    assert_eq!(glass, (w, h, radius));
    assert_eq!(shapes(&host.take()).last().copied(), Some(glass));
}

/// The step's knot fades with the card's words and the plate grows plain: the exit draws no knot
/// of its own, so each of its frames draws as many as a frame of the step at rest.
#[gpui::test]
fn the_exit_draws_no_knot_of_its_own(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    // Each frame of the layer draws the step's knot.
    let knots = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        let frames = plates().len();
        KNOTS.set(0);
        later(shell, cx, 0.001);
        (KNOTS.get() as usize, plates().len() - frames)
    };
    let (at_rest, frames) = knots(&shell, cx);
    assert!(at_rest > 0 && at_rest == frames, "one a frame at rest");
    pressed_and_grown(&shell, cx, &host);
    let (growing, frames) = knots(&shell, cx);
    assert_eq!(growing, frames, "one a frame as the card grows");
}

/// The grown card's fill ends on the browser window's tint alone, the ground the window shows
/// with none of its content, so the window takes the card's place unseen: never the tint with
/// the pages' card over it, which over the window's own material reads as the tint twice.
fn the_fill_ends_on_the_tint(appearance: Appearance, cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::Appearance(appearance), "test", cx)
    });
    cx.run_until_parked();
    let page = shell.read_with(cx, |shell, _| shell.window_tint());
    pressed_and_grown(&shell, cx, &host);
    // How far the last frame's fill is from the tint, at most, in a channel.
    let off = || {
        let (_, fill, _) = DRAWN.with_borrow(|drawn| drawn.last().copied().flatten().unwrap());
        [
            fill.r - page.r,
            fill.g - page.g,
            fill.b - page.b,
            fill.a - page.a,
        ]
        .into_iter()
        .fold(0_f32, |most, each| most.max(each.abs()))
    };
    let level = 1. / 255.;
    assert!(off() > level, "the card's own sheet as the grow starts");
    later(&shell, cx, x::MATERIAL);
    assert!(off() < level / 2., "crossed: {}", off());
    later(&shell, cx, x::GROW);
    assert!(off() < f32::EPSILON, "and on the frame it ends on");
}

#[gpui::test]
fn the_grown_cards_fill_ends_on_the_tint_in_dark(cx: &mut TestAppContext) {
    the_fill_ends_on_the_tint(Appearance::Dark, cx);
}

#[gpui::test]
fn the_grown_cards_fill_ends_on_the_tint_in_light(cx: &mut TestAppContext) {
    the_fill_ends_on_the_tint(Appearance::Light, cx);
}

/// Where the card grows to the browser window, the press opens the window's first Space and its
/// bar at once, as ⌘T has it, its hint with it, under the card with the window, so the window
/// takes the card's place with them drawn; the exit's end opens no second.
#[gpui::test]
fn a_grown_exit_opens_the_bar_at_the_press(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    host.grows_first_run.set(true);
    cx.simulate_keystrokes("enter");
    assert_eq!(
        host.command_bars.borrow().len(),
        1,
        "the bar opened at the press"
    );
    assert!(
        shell.read_with(cx, |shell, _| shell.in_hint),
        "with its hint"
    );
    cx.simulate_resize(size(px(GROWN.0), px(GROWN.1)));
    later(&shell, cx, x::CONTENT);
    later(&shell, cx, x::GROW);
    assert_eq!(step(&shell, cx), None, "the exit ended");
    assert_eq!(host.command_bars.borrow().len(), 1, "no second at its end");
    assert!(shell.read_with(cx, |shell, _| shell.in_hint));
    assert!(host.take().contains(&"first run ended".to_string()));
    let lines = crate::heard::take();
    assert_eq!(heard(&lines, "browser: command new-tab via onboarding"), 1);
    assert_eq!(heard(&lines, "browser: onboarding done"), 1);
}

/// How far the browser window's content has come in, as its next frame draws it.
fn content_shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> f32 {
    shell.read_with(cx, |shell, _| shell.content_shown())
}

/// The browser window's own content waits through a grown exit, the window its tint alone under
/// the card, then comes in over `reveal` from the frame the window takes the card's place, and
/// stays whole after. A cross-fade's window has it whole throughout.
#[gpui::test]
fn the_windows_content_comes_in_once_it_takes_the_cards_place(cx: &mut TestAppContext) {
    let (shell, cx, host) = at_the_end(cx);
    assert_eq!(content_shown(&shell, cx), 1., "whole while the card rests");
    pressed_and_grown(&shell, cx, &host);
    assert_eq!(content_shown(&shell, cx), 0., "none as the card grows");
    later(&shell, cx, x::GROW);
    assert_eq!(step(&shell, cx), None, "the exit ended");
    let mut seen = vec![content_shown(&shell, cx)];
    for _ in 0..4 {
        shell.update(cx, |shell, _| {
            let back = Duration::from_secs_f32(x::REVEAL / 4.);
            shell.content_since = shell.content_since.map(|since| since - back);
        });
        seen.push(content_shown(&shell, cx));
    }
    assert!(
        seen[0] < 1e-6,
        "none on the frame it takes the card's place: {seen:?}"
    );
    assert!(
        seen.windows(2).all(|pair| pair[1] > pair[0]),
        "only comes in: {seen:?}"
    );
    assert_eq!(seen.last(), Some(&1.), "whole after `reveal`: {seen:?}");
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    assert!(
        shell.read_with(cx, |shell, _| shell.content_since.is_none()),
        "the frame it's whole on ends the fade"
    );
}

#[gpui::test]
fn a_cross_fades_window_has_its_content_whole(cx: &mut TestAppContext) {
    let (shell, cx, _) = at_the_end(cx);
    cx.simulate_keystrokes("enter");
    later(&shell, cx, x::CROSS_FADE / 2.);
    assert_eq!(
        content_shown(&shell, cx),
        1.,
        "whole through the cross-fade"
    );
    later(&shell, cx, x::CROSS_FADE / 2.);
    assert_eq!(step(&shell, cx), None);
    assert_eq!(content_shown(&shell, cx), 1.);
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
    assert_eq!(host.asked_to_grow.get(), 0);
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
    cx.simulate_mouse_down(
        point(px(6.), px(6.)),
        gpui::MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(6.), px(6.)),
        gpui::MouseButton::Left,
        Modifiers::none(),
    );
    later(&shell, cx, x::GROW);
    assert_eq!(step(&shell, cx), None);
}

/// First run has ended as Start Browsing is pressed: ⌘Q in the exit leaves no step to resume,
/// and the exit still ends, with no new tab for the quit.
#[gpui::test]
fn a_quit_in_the_exit_leaves_first_run_ended(cx: &mut TestAppContext) {
    let dir = kept_dir(cx);
    let (shell, cx, host) = at_the_end(cx);
    assert!(
        kept(&dir).is_some(),
        "the last step is kept until it's left"
    );
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
    assert!(
        shell.read_with(cx, |shell, _| shell.in_hint),
        "not for ⇧ alone"
    );
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
