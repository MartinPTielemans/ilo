//! A new profile's first launch, on a host that records what the shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{Entity, Focusable, Modifiers, TestAppContext, VisualTestContext, point, px};

use super::{Drawn, END_FADE, INTRO_END, OptIn, STEP_MOTION, STILL_END, STILL_FADE, Step};
use crate::account::{self, State, Status};
use crate::import::ImportEvent;
use crate::shell::{Folders, Shell, commands::Command, recorder::Recorder};
use crate::theme::Theme;
use crate::theme_picker::SWATCHES;
use crate::tokens::shell::onboarding::{IMPORT_HOLD, SKIP_MARGIN};

/// A drawn shell showing one Space with no tab shown, as at a new profile's first launch, with the
/// keyboard in the UI.
fn first_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            )],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![1], Vec::new(), Folders::default(), cx);
    });
    cx.update(|window, cx| window.focus(&shell.focus_handle(cx), cx));
    cx.run_until_parked();
    (shell, cx, host)
}

/// The step the flow shows, if it shows.
fn step(shell: &Entity<Shell>, cx: &VisualTestContext) -> Option<Step> {
    shell.read_with(cx, |shell, _| {
        shell.onboarding.as_ref().map(|onboarding| onboarding.step)
    })
}

/// Starts the flow, and draws it once, as a launch with nothing to bring back does.
fn started(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| {
        shell.set_welcome(cx);
        shell.start_onboarding(None, cx);
    });
    cx.run_until_parked();
}

/// Goes on with Esc to `to`, as the user skipping each step before it does.
fn skip_to(shell: &Entity<Shell>, cx: &mut VisualTestContext, to: Step) {
    while step(shell, cx).is_some_and(|step| step != to) {
        cx.simulate_keystrokes("escape");
    }
    assert_eq!(step(shell, cx), Some(to));
}

/// Moves the step shown, or the flow's end, `seconds` into the past, and draws.
/// Lets `seconds` of the executor's clock pass, as a timer the flow set waits for.
fn waited(cx: &mut VisualTestContext, seconds: f32) {
    cx.executor()
        .advance_clock(Duration::from_secs_f32(seconds));
    cx.run_until_parked();
}

fn later(shell: &Entity<Shell>, cx: &mut VisualTestContext, seconds: f32) {
    shell.update(cx, |shell, cx| {
        let onboarding = shell.onboarding.as_mut().expect("the flow shows");
        let back = |at: Instant| at - Duration::from_secs_f32(seconds);
        onboarding.since = back(onboarding.since);
        onboarding.ending = onboarding.ending.map(back);
        cx.notify();
    });
    cx.run_until_parked();
}

/// The wordmark draws as a hand writes "ilo": the i's stem, the l's, then the ring, each from
/// nothing; the dot lands only once the ring is closed, takes the green after, and the line comes
/// last. A still intro has all of the wordmark from its first frame, only fading in.
#[test]
fn the_intro_draws_the_wordmark_then_its_line() {
    let at = |t| Drawn::at(t, false);
    assert_eq!(
        at(0.),
        Drawn {
            stem_i: 0.,
            stem_l: 0.,
            ring: 0.,
            dot: 0.,
            signal: 0.,
            tagline: 0.
        }
    );
    let early = at(0.4);
    assert!(
        early.stem_i > early.stem_l && early.stem_l > 0.,
        "the stems in order: {early:?}"
    );
    assert_eq!(
        early.ring, 0.,
        "the ring waits for the stems to be under way"
    );
    // A frame after each part ends, which is when the next may start.
    let ring_closed = at(1.11);
    assert_eq!(ring_closed.ring, 1.);
    assert!(
        ring_closed.dot < 0.1,
        "the dot lands once the ring closes: {ring_closed:?}"
    );
    let landed = at(1.35);
    assert_eq!(landed.dot, 1.);
    assert!(landed.signal < 0.1, "the dot lands in the ink: {landed:?}");
    let green = at(1.65);
    assert_eq!(green.signal, 1.);
    assert!(
        green.tagline < 0.1,
        "it takes the green before the line comes: {green:?}"
    );
    assert_eq!(at(2.05).tagline, 1.);

    let still = Drawn::at(0., true);
    assert_eq!(
        (still.stem_i, still.ring, still.dot, still.signal),
        (1., 1., 1., 1.)
    );
    assert_eq!(
        still.tagline, 0.,
        "with Reduce Motion it fades in rather than drawing"
    );
}

/// A launch that brings Spaces back never starts it: the shell has no flow until it's asked for.
#[gpui::test]
fn a_normal_window_has_no_flow(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    assert_eq!(step(&shell, cx), None);
}

/// A window that opens with no page has given its keyboard to Chromium's view, so the flow takes
/// it for the UI as it comes on screen, and only then, or each frame would take it from the import
/// view's own controls.
#[gpui::test]
fn the_flow_takes_the_keyboard_as_it_shows(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.take();
    started(&shell, cx);
    let taken = |calls: Vec<String>| calls.iter().filter(|call| *call == "focus ui").count();
    assert_eq!(taken(host.take()), 1, "the flow's first frame takes it");
    later(&shell, cx, 0.5);
    cx.simulate_keystrokes("a");
    later(&shell, cx, STEP_MOTION);
    assert_eq!(taken(host.take()), 0, "the frames and steps after leave it");
}

/// Any key skips the intro to the first step, and so does a click anywhere.
#[gpui::test]
fn any_key_or_click_skips_the_intro(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Intro));
    cx.simulate_keystrokes("a");
    assert_eq!(step(&shell, cx), Some(Step::Import));

    started(&shell, cx);
    let middle = shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport).0.center());
    cx.simulate_click(middle, Modifiers::none());
    assert_eq!(step(&shell, cx), Some(Step::Import));
}

/// Left alone, the intro holds once it's drawn, then the first step comes.
#[gpui::test]
fn the_intro_goes_on_by_itself(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    later(&shell, cx, INTRO_END - 0.5);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Intro),
        "it holds after it's drawn"
    );
    later(&shell, cx, 0.5);
    assert_eq!(step(&shell, cx), Some(Step::Import));
}

/// With the system's Reduce Motion, the intro is the still one, all there once its line has faded
/// in, long before the drawn one's line comes, and goes on at its shorter end. Turned off while it
/// shows, as the system says it changed, the intro holds until the full one's end.
#[gpui::test]
fn reduce_motion_gives_the_still_intro(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    started(&shell, cx);
    let settled = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.onboarding.as_ref().is_some_and(|o| o.settled)
        })
    };
    later(&shell, cx, STILL_FADE);
    assert!(
        settled(&shell, cx),
        "the still intro is all there once its line fades in"
    );
    later(&shell, cx, STILL_END - STILL_FADE);
    assert_eq!(step(&shell, cx), Some(Step::Import));

    started(&shell, cx);
    cx.update(|_, cx| cx.set_reduce_motion(false));
    cx.run_until_parked();
    later(&shell, cx, STILL_END);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Intro),
        "it holds past the still one's end"
    );
    later(&shell, cx, INTRO_END - STILL_END);
    assert_eq!(step(&shell, cx), Some(Step::Import));
}

/// The timer the intro holds on can wake the window before the end by the flow's own clock, as
/// Windows' did (#1026); the intro then waits out what's left, with nothing else drawing it.
#[gpui::test]
fn the_intro_goes_on_after_a_timer_that_wakes_it_early(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    later(&shell, cx, INTRO_END - 0.5);
    // The timer ends, while the flow's clock has half a second to go.
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Intro));
    // The half second passes, and only a timer wakes the window.
    shell.update(cx, |shell, _| {
        let onboarding = shell.onboarding.as_mut().expect("the flow shows");
        onboarding.since -= Duration::from_secs_f32(0.5);
    });
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import));
}

/// Esc skips each step in turn, and after the last the flow fades out from over the first
/// Space's welcome, which then shows alone.
#[gpui::test]
fn escape_goes_on_and_the_last_step_leaves_the_flow(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    cx.simulate_keystrokes("a");
    for next in [Step::DefaultBrowser, Step::Color] {
        later(&shell, cx, STEP_MOTION);
        cx.simulate_keystrokes("escape");
        assert_eq!(step(&shell, cx), Some(next));
    }
    later(&shell, cx, STEP_MOTION);
    cx.simulate_keystrokes("escape");
    let ending = shell.read_with(cx, |shell, _| shell.onboarding.as_ref().unwrap().ending);
    assert!(ending.is_some(), "Esc on the last step leaves the flow");
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert!(
        shell.read_with(cx, |shell, _| shell.welcome),
        "the welcome is under it"
    );
}

/// Return is the step's button, and Skip, at the window's bottom right, goes on too, from a step
/// and from the last, which leaves the flow. At the import Return is its view's button, which
/// shows the browser's choices, and keeps the step.
#[gpui::test]
fn return_and_skip_go_on_from_a_step(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    for (go_on, from, next) in [
        // Skip first: the window keeps its import view, which Return leaves in the choices.
        ("skip", Step::Import, Some(Step::DefaultBrowser)),
        ("return", Step::Import, Some(Step::Import)),
        ("return", Step::DefaultBrowser, Some(Step::Color)),
        ("return", Step::Color, None),
        ("skip", Step::Color, None),
    ] {
        started(&shell, cx);
        skip_to(&shell, cx, from);
        later(&shell, cx, STEP_MOTION);
        match go_on {
            "return" => cx.simulate_keystrokes("enter"),
            _ => {
                let viewport = shell.read_with(cx, |shell, _| shell.viewport);
                let skip = point(px(SKIP_MARGIN + 30.), px(SKIP_MARGIN + 10.));
                cx.simulate_click(
                    point(viewport.width - skip.x, viewport.height - skip.y),
                    Modifiers::none(),
                );
            }
        }
        if next.is_some() {
            assert_eq!(step(&shell, cx), next, "{go_on} went on from {from:?}");
            continue;
        }
        let ending = shell.read_with(cx, |shell, _| shell.onboarding.as_ref().unwrap().ending);
        assert!(ending.is_some(), "{go_on} left the last step");
        later(&shell, cx, END_FADE);
    }
}

/// Make Default asks the system, which asks the user; Skip leaves it be.
#[gpui::test]
fn the_default_browser_step_asks_the_system(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    for (key, asked) in [("escape", false), ("enter", true)] {
        started(&shell, cx);
        skip_to(&shell, cx, Step::DefaultBrowser);
        host.take();
        cx.simulate_keystrokes(key);
        assert_eq!(step(&shell, cx), Some(Step::Color));
        assert_eq!(
            host.take().contains(&"make default browser".to_string()),
            asked,
            "{key}"
        );
    }
}

/// The step says what each system does and where its way back is, as Settings' row does; in a
/// Flatpak, which can't set it, it says where the desktop does, and Return goes on without asking.
#[gpui::test]
fn the_default_browser_step_says_each_systems_way(cx: &mut TestAppContext) {
    use crate::default_browser::Os;
    let line = |os| Step::DefaultBrowser.message(os).1.to_string();
    assert!(line(Os::Mac).contains("macOS asks you first"));
    assert!(line(Os::Windows).contains("Settings › Default apps"));
    assert!(line(Os::Linux).contains("changes at once"));
    assert!(line(Os::Flatpak).contains("GNOME Settings › Default Apps"));
    assert_eq!(Step::DefaultBrowser.message(Os::Flatpak).2, "Continue");
    let (shell, cx, host) = first_window(cx);
    host.os.set(Some(Os::Flatpak));
    started(&shell, cx);
    skip_to(&shell, cx, Step::DefaultBrowser);
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Color));
    assert!(!host.take().contains(&"make default browser".to_string()));
}

/// A browser that's already the default has no step asking to make it one.
#[gpui::test]
fn a_default_browser_leaves_its_step_out(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.default_browser.set(true);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Import);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::Color));
}

/// The arrows go along the swatches, and Return gives the first Space the one picked, as the
/// theme picker's swatch does; Skip leaves the Space as it was.
#[gpui::test]
fn the_colour_picked_is_the_first_spaces(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    let picked = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.onboarding.as_ref().and_then(|o| o.picked)
        })
    };
    let colored = |calls: Vec<String>| calls.iter().any(|call| call.starts_with("space color 1 "));

    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    assert_eq!(picked(&shell, cx), None, "none is picked for the user");
    cx.simulate_keystrokes("left");
    assert_eq!(
        picked(&shell, cx),
        Some(SWATCHES.len() - 1),
        "← starts at the last"
    );
    cx.simulate_keystrokes("right");
    assert_eq!(
        picked(&shell, cx),
        Some(SWATCHES.len() - 1),
        "it stops at the end"
    );
    cx.simulate_keystrokes("escape");
    assert!(!colored(host.take()), "Skip keeps the Space's colour");

    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    cx.simulate_keystrokes("right right left right");
    assert_eq!(picked(&shell, cx), Some(1));
    host.take();
    cx.simulate_keystrokes("enter");
    let calls = host.take();
    assert!(
        colored(calls.clone()),
        "Return gives the Space its colour: {calls:?}"
    );
    let theme = Theme::of_color(SWATCHES[1]).to_text();
    assert!(
        calls.contains(&format!("space theme 1 {theme}")),
        "{calls:?}"
    );
}

/// A page shown covers the window, so it ends the flow at once, with no command bar over it.
#[gpui::test]
fn a_page_shown_ends_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), None);
    assert!(host.command_bars.borrow().is_empty());
}

/// The flow ends in the first Space with the command bar up, as ⌘T has it, and a line on the card
/// under it until a page shows.
#[gpui::test]
fn it_ends_with_the_command_bar_up(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    cx.simulate_keystrokes("escape");
    assert!(
        host.command_bars.borrow().is_empty(),
        "not while the flow fades"
    );
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert_eq!(host.command_bars.borrow().len(), 1, "the bar is up");
    assert!(shell.read_with(cx, |shell, _| shell.in_hint));
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    assert!(
        !shell.read_with(cx, |shell, _| shell.in_hint),
        "a page shown takes the line away"
    );
}

/// A quit under way closes the window the flow fades from, so its end opens no command bar there.
#[gpui::test]
fn a_quit_under_way_brings_no_command_bar_as_the_flow_ends(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    cx.simulate_keystrokes("escape");
    crate::shell::quit_goes_ahead();
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert!(host.command_bars.borrow().is_empty());
}

/// The opt-ins step shows the switches offered, each Settings' own and set as it's turned, and
/// none offered leaves the step out.
#[gpui::test]
fn the_opt_ins_show_only_those_offered(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    let offered = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.opt_ins(cx))
    };
    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    cx.simulate_keystrokes("escape");
    later(&shell, cx, END_FADE);
    assert_eq!(
        step(&shell, cx),
        None,
        "with neither offered, the colour is the last step"
    );

    // With no collector set up, only the agents' switch shows.
    host.agents.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    assert_eq!(offered(&shell, cx), vec![(OptIn::Agents, false)]);
    host.take();
    // It's set once the Shell's update is over, as turning agents off ends their ask, which reads
    // every Shell (#2459).
    host.reads_shell(shell.downgrade());
    shell.update(cx, |shell, cx| shell.onboarding_switch(OptIn::Agents, cx));
    assert_eq!(host.take(), vec!["agents true".to_string()]);
    assert_eq!(offered(&shell, cx), vec![(OptIn::Agents, true)]);
    cx.simulate_keystrokes("escape");
    let ending = shell.read_with(cx, |shell, _| shell.onboarding.as_ref().unwrap().ending);
    assert!(ending.is_some(), "it's the last step");
    assert_eq!(
        host.agents.get(),
        Some(true),
        "Skip leaves a switch as it was turned"
    );
    later(&shell, cx, END_FADE);

    host.crash_reports.set(Some(false));
    host.agents.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    let both = vec![(OptIn::CrashReports, false), (OptIn::Agents, false)];
    assert_eq!(offered(&shell, cx), both, "both off to begin with");
}

/// With both opt-ins offered, in a window the size of the owner's at 1280 × 800, each switch lies
/// inside its row, and a click at its drawn centre turns it on; its sentence once pushed it out of
/// the row, where the click missed (#1286).
#[gpui::test]
fn a_click_on_each_opt_in_switch_turns_it_on(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    cx.simulate_resize(gpui::size(px(1280.), px(800.)));
    host.crash_reports.set(Some(false));
    host.agents.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    later(&shell, cx, STEP_MOTION);
    let frame = |shell: &Entity<Shell>, cx: &VisualTestContext, opt_in, part| {
        shell.read_with(cx, |shell, _| {
            let onboarding = shell.onboarding.as_ref().expect("the flow shows");
            onboarding.frames.borrow().get(&(opt_in, part)).copied()
        })
    };
    for (opt_in, set) in [
        (OptIn::CrashReports, "crash reports true"),
        (OptIn::Agents, "agents true"),
    ] {
        let row = frame(&shell, cx, opt_in, "row").expect("its row is drawn");
        let switch = frame(&shell, cx, opt_in, "switch").expect("its switch is drawn");
        let inside = switch.left() >= row.left()
            && switch.right() <= row.right()
            && switch.top() >= row.top()
            && switch.bottom() <= row.bottom();
        assert!(
            inside,
            "{opt_in:?}'s switch at {switch:?} is outside its row at {row:?}"
        );
        host.take();
        cx.simulate_click(switch.center(), Modifiers::none());
        assert_eq!(
            host.take(),
            vec![set.to_string()],
            "a click on {opt_in:?}'s switch"
        );
    }
    let both = vec![(OptIn::CrashReports, true), (OptIn::Agents, true)];
    assert_eq!(shell.update(cx, |shell, cx| shell.opt_ins(cx)), both);
    // The switches line up, in one column at the box's right edge.
    let right = |opt_in| frame(&shell, cx, opt_in, "switch").map(|switch| switch.right());
    assert_eq!(right(OptIn::CrashReports), right(OptIn::Agents));
}

/// Help › Welcome to ilo plays it again: here when the window shows no page, else in a new window,
/// since GPUI can't draw over a page.
#[gpui::test]
fn welcome_to_ilo_plays_it_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    shell.update(cx, |shell, cx| shell.run(Command::Welcome, "menu", cx));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Intro));
    assert!(
        !host.take().contains(&"welcome window".to_string()),
        "no new window for it"
    );

    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    shell.update(cx, |shell, cx| shell.run(Command::Welcome, "menu", cx));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), None);
    assert!(host.take().contains(&"welcome window".to_string()));
}

/// The import step is the import view Settings shows: an import whose Spaces are in holds a moment
/// on them, then goes on to the next step while the rest runs, which the flow then says quietly,
/// and so does what it brought in. Its Done goes
/// on too. The view outlives the flow, so an import under way goes on, and Welcome to ilo shows
/// the same one again.
#[gpui::test]
fn the_import_step_is_the_import_view(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    let view = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.importing.as_ref().map(|i| i.view.clone())
        })
    };
    let line = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, cx| shell.import_line(cx))
    };
    started(&shell, cx);
    skip_to(&shell, cx, Step::Import);
    let shown = view(&shell, cx).expect("the import view");
    assert!(
        !shown.read_with(cx, |view, _| view.is_empty()),
        "it found the made-up Arc"
    );
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Started));
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "it went on before its Spaces were in"
    );
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Placed));
    waited(cx, IMPORT_HOLD * 0.9);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "it didn't hold on the Spaces"
    );
    waited(cx, IMPORT_HOLD * 0.2);
    assert_eq!(
        step(&shell, cx),
        Some(Step::DefaultBrowser),
        "an import placed goes on"
    );
    let said: gpui::SharedString = "2 Spaces came in from Arc.".into();
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Finished(said.clone())));
    assert_eq!(line(&shell, cx), Some(said), "what came in, said quietly");
    assert_eq!(
        step(&shell, cx),
        Some(Step::DefaultBrowser),
        "only its own step goes on"
    );

    started(&shell, cx);
    assert_eq!(
        line(&shell, cx),
        None,
        "played again, it has nothing to say yet"
    );
    skip_to(&shell, cx, Step::Import);
    assert_eq!(
        view(&shell, cx).map(|v| v.entity_id()),
        Some(shown.entity_id()),
        "the same view"
    );
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Done));
    assert_eq!(
        step(&shell, cx),
        Some(Step::DefaultBrowser),
        "its Done goes on"
    );

    skip_to(&shell, cx, Step::Color);
    cx.simulate_keystrokes("escape");
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert_eq!(
        view(&shell, cx).map(|v| v.entity_id()),
        Some(shown.entity_id()),
        "it lives on"
    );
}

/// With no browser to import from, the import step is left out.
#[gpui::test]
fn no_browser_found_leaves_the_import_out(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.no_browsers.set(true);
    started(&shell, cx);
    cx.simulate_keystrokes("a");
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser));
}

/// An import that has read brings in the favorites step, with the sites its history visited most,
/// each ticked; a click unticks one, and ↩ adds those still ticked. One that offers none goes on
/// past it.
#[gpui::test]
fn an_import_offers_its_most_visited_sites_as_favorites(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    let view = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell
            .read_with(cx, |shell, _| {
                shell.importing.as_ref().map(|i| i.view.clone())
            })
            .unwrap()
    };
    let tab = |url: &str| crate::import::found::Tab {
        url: url.into(),
        name: String::new(),
    };
    started(&shell, cx);
    skip_to(&shell, cx, Step::Import);
    let shown = view(&shell, cx);
    let offered = vec![
        tab("https://example.com/"),
        tab("https://news.example.org/"),
    ];
    shown.update(cx, |view, _| view.offer(offered));
    shown.update(cx, |_, cx| {
        cx.emit(ImportEvent::Started);
        cx.emit(ImportEvent::Placed);
    });
    waited(cx, IMPORT_HOLD);
    assert_eq!(step(&shell, cx), Some(Step::Favorites));
    let ticked = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, cx| {
            shell
                .favorites_ticked(cx)
                .into_iter()
                .map(|tab| tab.url)
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(
        ticked(&shell, cx),
        ["https://example.com/", "https://news.example.org/"]
    );
    view(&shell, cx).update(cx, |view, cx| {
        view.toggle_favorite("https://example.com/", cx)
    });
    assert_eq!(ticked(&shell, cx), ["https://news.example.org/"]);
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser));
    let offered = shown.read_with(cx, |view, _| view.suggested().map(<[_]>::len));
    assert_eq!(
        offered,
        Some(0),
        "the step took the offer, so the import's end doesn't make it"
    );

    started(&shell, cx);
    skip_to(&shell, cx, Step::Import);
    shown.update(cx, |view, _| view.offer(Vec::new()));
    shown.update(cx, |_, cx| {
        cx.emit(ImportEvent::Started);
        cx.emit(ImportEvent::Placed);
    });
    waited(cx, IMPORT_HOLD);
    assert_eq!(
        step(&shell, cx),
        Some(Step::DefaultBrowser),
        "none offered, no step"
    );
}

/// The account as the core says it is, at `server`, as the core hands it over before the flow.
fn synced(cx: &mut VisualTestContext, status: Status, server: &str) {
    let state = State {
        status,
        server: server.into(),
        default_server: true,
        ..State::default()
    };
    cx.update(|_, cx| cx.set_global(account::Synced(state)));
    cx.run_until_parked();
}

/// The steps this launch shows.
fn steps(shell: &Entity<Shell>, cx: &VisualTestContext) -> Vec<Step> {
    shell
        .read_with(cx, |shell, _| {
            shell.onboarding.as_ref().map(|o| o.steps.clone())
        })
        .unwrap_or_default()
}

/// What the account step asked of the core since the last look.
fn account_calls(host: &Recorder) -> Vec<String> {
    host.account.calls.take()
}

/// The account step comes after the colour while the browser isn't signed in, which it isn't until
/// the core says, and only in a build with a sync server, whose core it signs in through.
#[gpui::test]
fn the_account_step_comes_while_signed_out(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    started(&shell, cx);
    let shown = steps(&shell, cx);
    let at = |step| shown.iter().position(|&each| each == step);
    assert_eq!(
        at(Step::Account),
        at(Step::Color).map(|at| at + 1),
        "right after the colour"
    );

    synced(cx, Status::On, "https://sync.example.org");
    started(&shell, cx);
    assert!(
        !steps(&shell, cx).contains(&Step::Account),
        "signed in already"
    );

    synced(cx, Status::SignedOut, "https://sync.example.org");
    host.sync_server.set(false);
    started(&shell, cx);
    assert!(
        !steps(&shell, cx).contains(&Step::Account),
        "a build that names no server"
    );
}

/// The core reads the account a moment into the launch, after the flow may have started: signed in
/// by the colour step, the flow goes on past the account's.
#[gpui::test]
fn signed_in_by_then_the_account_step_is_left_out(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    host.crash_reports.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    synced(cx, Status::On, "https://sync.example.org");
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::OptIns));
}

/// Esc goes on from the account step without signing in, as Skip does.
#[gpui::test]
fn skip_leaves_the_account_step_signed_out(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    host.crash_reports.set(Some(false));
    synced(cx, Status::SignedOut, "https://sync.example.org");
    started(&shell, cx);
    skip_to(&shell, cx, Step::Account);
    later(&shell, cx, STEP_MOTION);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::OptIns));
    assert!(
        !account_calls(&host).contains(&"sign in".to_string()),
        "skipped, it didn't sign in"
    );
}

/// ↩ signs in, as the step's button does, and the step waits while the sign-in does; once another
/// device adds this one and the account syncs, the flow goes on by itself.
#[gpui::test]
fn a_finished_sign_in_goes_on(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    let server = "https://sync.example.org";
    synced(cx, Status::SignedOut, server);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Account);
    later(&shell, cx, STEP_MOTION);
    cx.simulate_keystrokes("enter");
    assert_eq!(account_calls(&host), ["sign in"]);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Account),
        "it waits for the sign-in"
    );

    synced(cx, Status::SigningIn, server);
    cx.simulate_keystrokes("enter");
    assert_eq!(
        account_calls(&host),
        Vec::<String>::new(),
        "↩ doesn't sign in twice"
    );
    synced(cx, Status::NeedsSecret, server);
    assert_eq!(
        account_calls(&host),
        ["request pairing"],
        "it asks the other devices, as Settings"
    );
    synced(cx, Status::Pairing, server);
    assert_eq!(step(&shell, cx), Some(Step::Account));
    synced(cx, Status::On, server);
    later(&shell, cx, END_FADE);
    assert_eq!(
        step(&shell, cx),
        None,
        "signed in, it went on, here out of the flow"
    );
}

/// The account's first device shows its recovery words, which show once, so neither Esc nor Skip
/// goes on from them; their Done does.
#[gpui::test]
fn the_first_devices_words_go_on_with_done(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    cx.simulate_resize(gpui::size(px(1280.), px(800.)));
    host.crash_reports.set(Some(false));
    let server = "https://sync.example.org";
    synced(cx, Status::SignedOut, server);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Account);
    later(&shell, cx, STEP_MOTION);
    synced(cx, Status::FirstDevice, server);
    assert_eq!(account_calls(&host), ["recovery words"]);
    // The core has made them, and syncs.
    synced(cx, Status::On, server);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::Account), "Esc keeps the words");
    let view = shell
        .read_with(cx, |shell, _| shell.account_view())
        .expect("the account shows");
    let skip = shell.read_with(cx, |shell, cx| shell.keeping_words(cx));
    assert!(skip, "no Skip while the words show");
    let done = view
        .read_with(cx, |view, _| view.drawn("done"))
        .expect("Done is drawn");
    cx.simulate_click(done.center(), Modifiers::none());
    assert_eq!(step(&shell, cx), Some(Step::OptIns));
}

/// The import step with a browser of the test's own behind the view, from its first launch.
fn importing_from(
    browser: Rc<crate::import::tests::Browser>,
    cx: &mut TestAppContext,
) -> (
    Entity<Shell>,
    &mut VisualTestContext,
    Entity<crate::import::ImportView>,
) {
    let (shell, cx, host) = first_window(cx);
    *host.import.borrow_mut() = Some(browser);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Import);
    later(&shell, cx, STEP_MOTION);
    let view = shell.read_with(cx, |shell, _| {
        shell.importing.as_ref().map(|i| i.view.clone())
    });
    (shell, cx, view.expect("the import view"))
}

/// Safari's files kept from the app need Full Disk Access as Safari is picked: the import step
/// stays, showing the view's way to System Settings, and Skip still goes on. Once it's given and
/// the user is back, the view reads Safari and shows its choices, and Import there goes on
/// (#1378, #1673).
#[cfg(unix)]
#[gpui::test]
fn an_import_that_needs_full_disk_access_keeps_its_step(cx: &mut TestAppContext) {
    use crate::import::sources::{Os, Roots};
    use crate::import::tests::{Browser, let_read, safari};
    let Some(home) = safari("onboarding-access") else {
        return;
    };
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Mac))));
    let (shell, cx, view) = importing_from(browser.clone(), cx);
    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "it waits for the user"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.drawn()),
        Some("no access")
    );

    // With access given, coming back reads Safari and shows its choices; Import goes on.
    let_read(&home);
    cx.update(|_, cx| browser.activate(cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.drawn()), Some("pick"));
    assert_eq!(step(&shell, cx), Some(Step::Import), "it waits for Import");
    view.update(cx, |view, cx| view.import(cx));
    cx.run_until_parked();
    waited(cx, IMPORT_HOLD);
    assert_ne!(step(&shell, cx), Some(Step::Import), "imported, it went on");
    std::fs::remove_dir_all(&home).unwrap();
}

/// The import step says what the state its view shows is for, with its button and its corner:
/// the list "Bring your stuff." with Import from the browser used last and Skip; ↩ shows that
/// browser's choices, "What comes from Arc.", with Import and Back, and its view has no buttons of
/// its own; Esc goes back to the list, keeping the step, and Esc there skips it.
#[gpui::test]
fn the_import_steps_words_follow_the_state_its_view_shows(cx: &mut TestAppContext) {
    use crate::import::Corner;
    use crate::import::tests::Browser;
    let browser = Rc::new(Browser::new(None));
    let (shell, cx, view) = importing_from(browser, cx);
    let guide = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.guide());
    // The fixture's Arc is where the Mac keeps it, so it says the Mac.
    let here = "this Mac";
    let list = guide(cx);
    assert_eq!(list.title, "Bring your stuff.");
    assert_eq!(
        list.line,
        format!("From the browser you used. It all stays on {here}.")
    );
    assert_eq!(list.button.as_deref(), Some("Import from Arc"));
    assert_eq!(list.corner, Corner::Skip);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import));
    let choose = guide(cx);
    assert_eq!(choose.title, "What comes from Arc.");
    assert_eq!(choose.line, "All of it, unless you turn something off.");
    assert_eq!(choose.button.as_deref(), Some("Import"));
    assert_eq!(choose.corner, Corner::Back);
    assert_eq!(view.read_with(cx, |view, _| view.primary()), Some("Import"));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "Esc went past the list"
    );
    assert_eq!(guide(cx).title, "Bring your stuff.");
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser));
}

/// ↩ at the list brings the browser picked, and ↩ at its choices imports it: once read, the flow
/// goes on by itself while the rest comes in.
#[gpui::test]
fn return_brings_the_browser_picked_and_imports_it(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let browser = Rc::new(Browser::new(None));
    let (shell, cx, view) = importing_from(browser.clone(), cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    // The step shows the Spaces arriving while Work's profile loads, and holds on them once in.
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "it went on before its Spaces were in"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.guide().title),
        "Here it comes."
    );
    cx.update(|_, cx| browser.load(cx));
    cx.run_until_parked();
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "it didn't hold on the Spaces"
    );
    waited(cx, IMPORT_HOLD);
    assert_ne!(step(&shell, cx), Some(Step::Import), "placed, it went on");
}

/// A profile that hasn't opened keeps the step, named in its title, with Try Again as its button,
/// which ↩ presses; Esc skips the step, as its corner says, and the import goes on waiting.
#[gpui::test]
fn a_stalled_import_names_its_profile_on_the_step(cx: &mut TestAppContext) {
    use crate::import::Corner;
    use crate::import::tests::Browser;
    let browser = Rc::new(Browser::new(None));
    let (shell, cx, view) = importing_from(browser, cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    waited(cx, crate::import::STALL.as_secs_f32());
    let guide = view.read_with(cx, |view, _| view.guide());
    assert_eq!(guide.title, "Work hasn't opened.");
    assert_eq!(guide.button.as_deref(), Some("Try Again"));
    assert_eq!(guide.corner, Corner::Skip);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.guide().title),
        "Here it comes.",
        "↩ didn't try again"
    );
    waited(cx, crate::import::STALL.as_secs_f32());
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser));
    assert_eq!(
        view.read_with(cx, |view, _| view.primary()),
        Some("Try Again"),
        "the import stopped waiting as the step was skipped"
    );
}

/// ↓ and ↑ move the list's pick, stopping at its ends, and the button names the browser picked.
#[gpui::test]
fn the_arrows_move_the_lists_pick(cx: &mut TestAppContext) {
    use crate::import::sources::{Os, Roots};
    use crate::import::tests::{Browser, chrome, firefox_dir};
    let home = chrome(
        "onboarding-two",
        &[("Default", "Me")],
        std::time::SystemTime::now(),
    );
    firefox_dir(home.join(".mozilla/firefox"), 60 * 60);
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let (_, cx, view) = importing_from(browser, cx);
    let button = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.guide().button);
    assert_eq!(button(cx).as_deref(), Some("Import from Chrome"));
    cx.simulate_keystrokes("down down");
    assert_eq!(button(cx).as_deref(), Some("Import from Firefox"));
    cx.simulate_keystrokes("up");
    assert_eq!(button(cx).as_deref(), Some("Import from Chrome"));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.guide().title),
        "What comes from Chrome."
    );
    std::fs::remove_dir_all(home).unwrap();
}

/// The wait for Full Disk Access says Back in its corner, which Esc does, back to the list; Esc
/// there goes on, as from any step.
#[cfg(unix)]
#[gpui::test]
fn skip_goes_on_from_full_disk_access(cx: &mut TestAppContext) {
    use crate::import::sources::{Os, Roots};
    use crate::import::tests::{Browser, safari};
    let Some(home) = safari("onboarding-access-skip") else {
        return;
    };
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Mac))));
    let (shell, cx, view) = importing_from(browser, cx);
    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.drawn()),
        Some("no access")
    );
    let guide = view.read_with(cx, |view, _| view.guide());
    assert_eq!(
        guide.title,
        format!("Let {} read Safari.", env!("APP_NAME"))
    );
    assert_eq!(guide.button.as_deref(), Some("Open System Settings"));
    assert_eq!(guide.corner, crate::import::Corner::Back);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "Esc went back to the list"
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser));
    std::fs::remove_dir_all(&home).unwrap();
}

/// A passwords file chosen at the import step keeps the step until its end, which offers to delete
/// the file in the words Settings has; Delete File deletes it, and Done goes on (#1379).
#[gpui::test]
fn a_passwords_file_from_onboarding_is_offered_for_deletion(cx: &mut TestAppContext) {
    use crate::import::Passwords;
    use crate::import::tests::{Browser, PASSWORDS, home, write};
    let browser = Rc::new(Browser::new(None));
    let file = write(
        home("onboarding-passwords").join("Chrome Passwords.csv"),
        PASSWORDS,
    );
    *browser.passwords_file.borrow_mut() = Some(file.clone());
    let (shell, cx, view) = importing_from(browser.clone(), cx);
    view.update(cx, |view, cx| view.choose_passwords(cx));
    cx.run_until_parked();
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "the step stays while it imports"
    );
    let passwords = Passwords::Imported {
        imported: 1,
        conflicted: 0,
        skipped: 0,
    };
    cx.update(|_, cx| browser.imported(passwords, cx));
    cx.run_until_parked();
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "its end shows on the step"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.drawn()),
        Some("done with file")
    );
    view.update(cx, |view, cx| view.delete_passwords_file(cx));
    cx.run_until_parked();
    assert!(!file.exists(), "Delete File deleted it");
    assert_eq!(view.read_with(cx, |view, _| view.drawn()), Some("done"));
    view.update(cx, |view, cx| view.done(cx));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser), "Done goes on");
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

/// How the import step's button is pressed.
#[derive(Clone, Copy)]
enum Press {
    Return,
    Click,
}

/// Import from a browser already read, with open tabs to place, goes on through its step whether
/// Return or the button presses it. The Shell presses the step from inside its own update, and
/// placing the tabs asks the browser what it has, which reads each window's Shell for its Today
/// tabs: read there, it panicked and aborted the launch (#2428).
fn imports_open_tabs_on(press: Press, cx: &mut TestAppContext) {
    use crate::import::sources::{Os, Roots};
    use crate::import::tests::{Browser, chrome};
    let name = match press {
        Press::Return => "onboarding-tabs-return",
        Press::Click => "onboarding-tabs-click",
    };
    let home = chrome(name, &[("Default", "Me")], std::time::SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let (shell, cx, view) = importing_from(browser.clone(), cx);
    let weak = shell.downgrade();
    browser.reads_shell(move |cx| {
        if let Some(shell) = weak.upgrade() {
            shell.read(cx);
        }
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.guide().title),
        "What comes from Chrome.",
        "Chrome wasn't read"
    );
    match press {
        Press::Return => cx.simulate_keystrokes("enter"),
        Press::Click => {
            let at = shell.read_with(cx, |shell, _| {
                (shell.onboarding.as_ref()).and_then(|onboarding| onboarding.button.get())
            });
            cx.simulate_click(at.expect("the button drawn").center(), Modifiers::none());
        }
    }
    cx.run_until_parked();
    // Mail's tab is pinned, as its bookmark on the bar is.
    assert_eq!(
        browser.today(),
        ["https://open.example.com/"],
        "the open tab wasn't placed"
    );
    waited(cx, IMPORT_HOLD);
    assert_ne!(step(&shell, cx), Some(Step::Import), "placed, it went on");
    std::fs::remove_dir_all(home).unwrap();
}

#[gpui::test]
fn return_imports_open_tabs(cx: &mut TestAppContext) {
    imports_open_tabs_on(Press::Return, cx);
}

#[gpui::test]
fn the_button_imports_open_tabs(cx: &mut TestAppContext) {
    imports_open_tabs_on(Press::Click, cx);
}
