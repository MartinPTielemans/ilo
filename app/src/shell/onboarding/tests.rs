//! A new profile's first launch, on a host that records what the shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{Entity, Focusable, Modifiers, TestAppContext, VisualTestContext, point, px};

use super::{Drawn, END_FADE, INTRO_END, OptIn, STEP_MOTION, Step};
use crate::import::ImportEvent;
use crate::shell::{Folders, Shell, commands::Command, recorder::Recorder};
use crate::theme::Theme;
use crate::theme_picker::SWATCHES;
use crate::tokens::shell::onboarding::SKIP_MARGIN;

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
/// and from the last, which leaves the flow. The import's buttons are its own, so Return there is
/// the view's.
#[gpui::test]
fn return_and_skip_go_on_from_a_step(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    for (go_on, from, next) in [
        ("return", Step::Import, Some(Step::Import)),
        ("skip", Step::Import, Some(Step::DefaultBrowser)),
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

/// The import step is the import view Settings shows: an import begun goes on to the next step
/// while it runs, which the flow then says quietly, and so does what it brought in. Its Done goes
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
        Some(Step::DefaultBrowser),
        "an import begun goes on"
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

/// An import begun brings in the favorites step, with the sites its history visited most, each
/// ticked; a click unticks one, and ↩ adds those still ticked. One that offers none goes on past
/// it.
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
    shown.update(cx, |view, cx| view.offer(offered, cx));
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Started));
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
    shell.update(cx, |shell, cx| {
        shell.onboarding_tick("https://example.com/", cx)
    });
    assert_eq!(ticked(&shell, cx), ["https://news.example.org/"]);
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser));

    started(&shell, cx);
    skip_to(&shell, cx, Step::Import);
    shown.update(cx, |view, cx| view.offer(Vec::new(), cx));
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Started));
    assert_eq!(
        step(&shell, cx),
        Some(Step::DefaultBrowser),
        "none offered, no step"
    );
}
