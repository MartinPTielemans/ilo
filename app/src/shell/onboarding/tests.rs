//! A new profile's first launch, the card in first run's window, on a host that records what the
//! shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Entity, Focusable, Modifiers, TestAppContext, VisualTestContext,
    accesskit::{Action, Role, Toggled},
    point, px,
};

use super::{END_FADE, FirstRun, Onboarding, STEPS, Step, Switch, Tidy, stop_words};
use crate::account::{self, State, Status};
use crate::agents::runner::Cli;
use crate::agents::sign_in::{Offer, Shown};
use crate::import::ImportEvent;
use crate::settings_rows::spoken::{press, stops};
use crate::shell::{Broom, Folders, Shell, commands::Command, recorder::Recorder};
use crate::theme::Theme;
use crate::theme_picker::{SWATCHES, swatch_name};
use crate::tokens::shell::onboarding::card as t;
use crate::transparency;

const DAY: f64 = 24. * 3600.;
const SERVER: &str = "https://sync.example.org";

/// A drawn shell showing one Space with no tab shown, as at a new profile's first launch, and first
/// run's window over it, which the context returned drives: its keys and clicks go to that window.
fn first_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    // The Space step keeps its level where Settings does, and ⌘Q the step it was at: here, a
    // folder of the test's own.
    let dir = std::env::temp_dir().join(format!(
        "ilo-onboarding-{}-{}",
        std::process::id(),
        cx.test_function_name().unwrap_or("test")
    ));
    _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| transparency::kept_in(dir.to_string_lossy().into_owned(), cx));
    cx.update(|cx| super::resume::kept_in(dir.to_string_lossy().into_owned(), cx));
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
        // Over no glass, as on Linux and Windows, so the steps are the same on every OS.
        shell.blurred = false;
    });
    cx.update(|window, cx| window.focus(&shell.focus_handle(cx), cx));
    cx.run_until_parked();
    // Opened here so the test holds its context; starting the card takes it up.
    let first_run = cx
        .update(|_, cx| FirstRun::open(shell.clone(), cx))
        .expect("first run's window");
    shell.update(cx, |shell, _| shell.first_run = Some(first_run));
    let cx = VisualTestContext::from_window(first_run.into(), cx).into_mut();
    cx.run_until_parked();
    (shell, cx, host)
}

/// The step the card shows, if it shows.
fn step(shell: &Entity<Shell>, cx: &VisualTestContext) -> Option<Step> {
    shell.read_with(cx, |shell, _| {
        shell.onboarding.as_ref().map(|onboarding| onboarding.step)
    })
}

/// The steps this run shows.
fn steps(shell: &Entity<Shell>, cx: &VisualTestContext) -> Vec<Step> {
    shell
        .read_with(cx, |shell, _| {
            shell.onboarding.as_ref().map(|o| o.steps.clone())
        })
        .unwrap_or_default()
}

/// Reads the card's state.
fn card<R>(
    shell: &Entity<Shell>,
    cx: &VisualTestContext,
    read: impl FnOnce(&Onboarding) -> R,
) -> R {
    shell.read_with(cx, |shell, _| {
        read(shell.onboarding.as_ref().expect("the card shows"))
    })
}

/// Starts the card and draws it, its entry playing.
fn begun(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| {
        shell.set_welcome(cx);
        shell.start_onboarding(None, cx);
    });
    cx.run_until_parked();
}

/// Starts the card past its entry, the first step all there, as once the entry has played.
fn started(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    begun(shell, cx);
    shell.update(cx, |shell, cx| {
        shell.onboarding.as_mut().unwrap().end_entry();
        cx.notify();
    });
    settle(shell, cx);
}

/// Moves what the card times `seconds` into the past, and draws.
fn later(shell: &Entity<Shell>, cx: &mut VisualTestContext, seconds: f32) {
    shell.update(cx, |shell, cx| {
        let onboarding = shell.onboarding.as_mut().expect("the card shows");
        let back = |at: Instant| at - Duration::from_secs_f32(seconds);
        onboarding.since = back(onboarding.since);
        onboarding.progress.2 = back(onboarding.progress.2);
        onboarding.entry = onboarding.entry.map(back);
        onboarding.ending = onboarding.ending.map(back);
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

/// Lets the step shown come all the way, and its progress knot get there.
fn settle(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    later(shell, cx, t::PROGRESS_MOTION.max(t::STEP_FADE));
}

/// Whether the card's last frame asked for another, once what it asked for has drawn.
fn asks_for_frames(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        shell.onboarding.as_ref().is_some_and(|o| o.drawing)
    })
}

/// Where the step shown draws `name` (Shell::place).
fn placed(name: &str, cx: &mut VisualTestContext) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.run_until_parked();
    let first_run = cx
        .update(|window, _| window.root::<FirstRun>().flatten())
        .expect("first run's window");
    let shell = first_run.read_with(cx, |first_run, _| first_run.shell());
    shell.read_with(cx, |shell, _| {
        let onboarding = shell.onboarding.as_ref()?;
        onboarding.places.borrow().get(name).copied()
    })
}

/// Clicks what the step shown draws as `name`.
fn click(name: &str, cx: &mut VisualTestContext) {
    let at = placed(name, cx).unwrap_or_else(|| panic!("{name} is drawn"));
    cx.simulate_click(at.center(), Modifiers::none());
}

/// Goes on with ↩ or a step's way on to `to`, settling each step.
fn on_to(shell: &Entity<Shell>, cx: &mut VisualTestContext, to: Step) {
    // A card that never comes to `to` fails rather than spins.
    for _ in 0..16 {
        let Some(at) = step(shell, cx).filter(|&step| step != to) else {
            break;
        };
        match at {
            Step::Account => click("not-now", cx),
            Step::Assistant => click("just-the-browser", cx),
            Step::Import if placed("skip", cx).is_some() => click("skip", cx),
            _ => cx.simulate_keystrokes("enter"),
        }
        settle(shell, cx);
    }
    assert_eq!(step(shell, cx), Some(to));
}

/// The account as the core says it is, as the core hands it over before the card.
fn synced(cx: &mut VisualTestContext, status: Status) {
    let state = State {
        status,
        server: SERVER.into(),
        default_server: true,
        ..State::default()
    };
    cx.update(|_, cx| cx.set_global(account::Synced(state)));
    cx.run_until_parked();
}

/// What the Welcome asked of the core since the last look.
fn account_calls(host: &Recorder) -> Vec<String> {
    host.account.calls.take()
}

/// The role and name of what has the keyboard, as a screen reader has it.
fn keyed(cx: &mut VisualTestContext) -> Option<(Role, String)> {
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let (_, node) = tree.nodes.iter().find(|(id, _)| *id == tree.focus)?;
    Some((node.role(), node.label().unwrap_or_default().to_string()))
}

/// A launch that brings Spaces back never starts it: the shell has no card until it's asked for.
#[gpui::test]
fn a_normal_window_has_no_card(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    assert_eq!(step(&shell, cx), None);
}

/// Each step shows only when it has something to ask (v9 §0), in the design's order, and the last
/// always.
#[gpui::test]
fn the_steps_are_those_with_something_to_ask(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    assert_eq!(
        steps(&shell, cx),
        [Step::Import, Step::Space, Step::Done],
        "a browser to import from, and nothing else to ask"
    );
    shell.update(cx, |shell, _| shell.onboarding = None);

    host.sync_server.set(true);
    host.threads.set(Some(false));
    host.archive_after.set(Some(DAY / 2.));
    synced(cx, Status::SignedOut);
    started(&shell, cx);
    assert_eq!(
        steps(&shell, cx),
        [
            Step::Account,
            Step::Assistant,
            Step::Import,
            Step::Space,
            Step::Tidy,
            Step::Done
        ]
    );
    let order = |step| STEPS.iter().position(|&each| each == step);
    let shown = steps(&shell, cx);
    assert!(
        shown.windows(2).all(|two| order(two[0]) < order(two[1])),
        "in the design's order"
    );
    shell.update(cx, |shell, _| shell.onboarding = None);

    // Signed in, no browser found: neither the Welcome nor the import. No agent program found
    // here, as a test's shell has no Agents, leaves Other apps out.
    synced(cx, Status::On);
    host.no_browsers.set(true);
    // As at a launch on a machine with none: a replay lists the browsers through the host it had.
    shell.update(cx, |shell, _| shell.importing = None);
    started(&shell, cx);
    let shown = steps(&shell, cx);
    assert!(!shown.contains(&Step::Account), "signed in already");
    assert!(!shown.contains(&Step::Import), "no browser found");
    assert!(!shown.contains(&Step::OtherApps), "no agent program found");
    assert_eq!(shown.last(), Some(&Step::Done));
}

/// The card's steps are numbered as the design and its stderr lines name them.
#[test]
fn each_step_is_named_by_its_number() {
    let numbers: Vec<&str> = STEPS.iter().map(|step| step.number()).collect();
    assert_eq!(numbers, ["1", "2", "3", "3b", "4", "5", "6", "7"]);
}

/// The entry plays as the card first shows, the step's content still to come; a key ends it
/// where it is and does nothing else, as does a click, which presses nothing under it. Played to
/// its end, it ends by itself.
#[gpui::test]
fn the_entry_plays_and_a_key_or_click_ends_it_alone(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    let entering =
        |shell: &Entity<Shell>, cx: &VisualTestContext| card(shell, cx, |o| o.entry.is_some());
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    begun(&shell, cx);
    assert!(entering(&shell, cx));
    assert!(asks_for_frames(&shell, cx), "it moves");
    cx.simulate_keystrokes("enter");
    assert!(!entering(&shell, cx), "a key ends it");
    assert_eq!(step(&shell, cx), Some(Step::Account), "and only ends it");
    assert!(account_calls(&host).is_empty(), "↩ didn't sign in");
    shell.update(cx, |shell, _| shell.onboarding = None);

    begun(&shell, cx);
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.simulate_click(
        point(px(t::WIDTH / 2.), px(t::HEIGHT / 2.)),
        Modifiers::none(),
    );
    assert!(!entering(&shell, cx), "a click ends it");
    assert!(account_calls(&host).is_empty(), "and presses nothing");
    shell.update(cx, |shell, _| shell.onboarding = None);

    begun(&shell, cx);
    later(&shell, cx, Onboarding::entry_length(false));
    assert!(!entering(&shell, cx), "played, it ends by itself");
    settle(&shell, cx);
    assert!(!asks_for_frames(&shell, cx), "and the card stands still");
}

/// With the system's Reduce Motion the entry is a fade, over at once, and every step comes with
/// one, drawing no more frames once there.
#[gpui::test]
fn reduce_motion_makes_the_entry_a_fade(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    begun(&shell, cx);
    later(&shell, cx, t::STILL_FADE);
    assert!(
        card(&shell, cx, |o| o.entry.is_none()),
        "faded in, it's over"
    );
    assert!(card(&shell, cx, |o| o.settled));
    assert!(
        !asks_for_frames(&shell, cx),
        "a still card draws no more frames"
    );
    cx.simulate_keystrokes("enter");
    later(&shell, cx, t::STILL_FADE);
    assert!(
        !asks_for_frames(&shell, cx),
        "the next step faded in, and its progress jumped"
    );
}

/// The progress knot counts only the steps this run shows, from none drawn at the first to whole
/// at the last; the favorites share the import's place. Each step change sets it off, and it stops
/// once there.
#[gpui::test]
fn the_progress_counts_only_the_steps_shown(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    let fraction =
        |step, shell: &Entity<Shell>, cx: &VisualTestContext| card(shell, cx, |o| o.fraction(step));
    assert_eq!(steps(&shell, cx), [Step::Import, Step::Space, Step::Done]);
    assert_eq!(fraction(Step::Import, &shell, cx), 0.);
    assert_eq!(fraction(Step::Space, &shell, cx), 0.5);
    assert_eq!(fraction(Step::Done, &shell, cx), 1.);
    shell.update(cx, |shell, cx| {
        shell.import_event(&ImportEvent::Started, cx)
    });
    assert_eq!(
        steps(&shell, cx),
        [Step::Import, Step::Favorites, Step::Space, Step::Done],
        "an import begun brings the favorites after it"
    );
    assert_eq!(
        fraction(Step::Favorites, &shell, cx),
        fraction(Step::Import, &shell, cx),
        "in the import's place"
    );
    assert_eq!(
        fraction(Step::Space, &shell, cx),
        0.5,
        "counting no more steps"
    );

    shell.update(cx, |shell, cx| shell.onboarding_next(cx));
    assert!(
        asks_for_frames(&shell, cx),
        "the step changed: it fades and the knot moves"
    );
    settle(&shell, cx);
    assert!(!asks_for_frames(&shell, cx));
    let now = shell.read_with(cx, |shell, cx| {
        let still = crate::motion::still(cx);
        shell.onboarding.as_ref().unwrap().progress_now(still)
    });
    assert_eq!(
        steps(&shell, cx),
        [Step::Import, Step::Space, Step::Done],
        "an import that offers no favorites leaves their step out as the import is left"
    );
    assert_eq!(now, fraction(Step::Space, &shell, cx));
}

/// Esc cancels a wait under way and nothing else: it never passes a step, never goes back, and
/// never ends the card.
#[gpui::test]
fn escape_never_passes_or_backs_a_step(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.archive_after.set(Some(DAY / 2.));
    host.daily_count.set(Some(false));
    started(&shell, cx);
    for at in [Step::Space, Step::Tidy, Step::Done] {
        on_to(&shell, cx, at);
        host.take();
        cx.simulate_keystrokes("escape");
        settle(&shell, cx);
        assert_eq!(step(&shell, cx), Some(at), "Esc at {at:?}");
        assert!(card(&shell, cx, |o| o.ending.is_none()));
        assert!(host.take().is_empty(), "Esc set nothing at {at:?}");
    }
}

/// The card has no close: the system's close, ⌘W, ⇧⌘W and File's Close Tab and Close Window
/// leave it as it is, and close nothing else. ⌘[ is the card's Back, not a page's.
#[gpui::test]
fn the_card_has_no_close(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Space);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    host.take();
    assert!(!cx.simulate_close(), "the window stays");
    cx.simulate_keystrokes("secondary-w shift-secondary-w");
    for command in [Command::CloseTab, Command::CloseWindow, Command::Back] {
        shell.update(cx, |shell, cx| shell.run(command, "ui", cx));
        cx.run_until_parked();
    }
    assert_eq!(step(&shell, cx), Some(Step::Space));
    assert!(card(&shell, cx, |o| o.ending.is_none()));
    let calls = host.take();
    assert!(
        !calls
            .iter()
            .any(|asked| asked.starts_with("close") || asked.starts_with("back")),
        "{calls:?}"
    );
}

/// Back goes to the step before, as does ⌘[, and what was picked stays picked; the first step has
/// no Back.
#[gpui::test]
fn back_goes_to_the_step_before_and_keeps_picks(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    let picked = |shell: &Entity<Shell>, cx: &VisualTestContext| card(shell, cx, |o| o.picked);
    started(&shell, cx);
    assert!(placed("back", cx).is_none(), "the first step has no Back");
    on_to(&shell, cx, Step::Space);
    cx.simulate_keystrokes("right right");
    assert_eq!(picked(&shell, cx), Some(1));
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Done));
    cx.simulate_keystrokes("secondary-[");
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Space), "⌘[ went back");
    assert_eq!(picked(&shell, cx), Some(1), "the pick stays");
    click("back", cx);
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Import), "Back went back");
}

/// The arrows go along the swatches, each showing on the window at once, and ↩ gives the first
/// Space the one picked, as the theme picker's swatch does.
#[gpui::test]
fn the_colour_picked_is_the_first_spaces(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Space);
    assert_eq!(
        card(&shell, cx, |o| o.picked),
        None,
        "none is picked for the user"
    );
    cx.simulate_keystrokes("left");
    assert_eq!(
        card(&shell, cx, |o| o.picked),
        Some(SWATCHES.len() - 1),
        "← starts at the last"
    );
    cx.simulate_keystrokes("right");
    assert_eq!(
        card(&shell, cx, |o| o.picked),
        Some(SWATCHES.len() - 1),
        "it stops at the end"
    );
    cx.simulate_keystrokes("left left");
    host.take();
    cx.simulate_keystrokes("enter");
    let calls = host.take();
    let theme = Theme::of_color(SWATCHES[SWATCHES.len() - 3]).to_text();
    assert!(
        calls.contains(&format!("space theme 1 {theme}")),
        "{calls:?}"
    );
    assert_eq!(step(&shell, cx), Some(Step::Done));
}

/// The Welcome: ↩ is Continue, which signs in, and the step waits while it does; signed in, the
/// card goes on. Not Now goes on without signing in.
#[gpui::test]
fn the_welcome_signs_in_or_goes_on_without(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Account), "it comes first");
    click("not-now", cx);
    assert_eq!(step(&shell, cx), Some(Step::Import));
    assert!(account_calls(&host).is_empty(), "Not Now didn't sign in");
    shell.update(cx, |shell, _| shell.onboarding = None);

    started(&shell, cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(account_calls(&host), ["sign in"]);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Account),
        "it waits for the sign-in"
    );
    synced(cx, Status::SigningIn);
    cx.simulate_keystrokes("enter");
    assert!(account_calls(&host).is_empty(), "↩ doesn't sign in twice");
    synced(cx, Status::On);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "signed in, it went on"
    );
}

/// An account that exists already, asking for its words or another device's approval, brings its
/// Spaces back through sync, so the steps about a new profile's things are left out.
#[gpui::test]
fn a_returning_account_leaves_out_the_new_profiles_steps(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    host.archive_after.set(Some(DAY / 2.));
    synced(cx, Status::SignedOut);
    started(&shell, cx);
    cx.simulate_keystrokes("enter");
    synced(cx, Status::Pairing);
    assert_eq!(step(&shell, cx), Some(Step::Account));
    synced(cx, Status::On);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Done),
        "past the import, the Space and tidy"
    );
}

/// Signed in after the Welcome was passed, there's no going back to it.
#[gpui::test]
fn signed_in_by_then_the_welcome_is_left_out(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    started(&shell, cx);
    click("not-now", cx);
    settle(&shell, cx);
    assert!(
        placed("back", cx).is_some(),
        "the Welcome is before the import"
    );
    synced(cx, Status::On);
    cx.simulate_keystrokes("secondary-[");
    settle(&shell, cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "nothing before it now"
    );
    assert!(!steps(&shell, cx).contains(&Step::Account));
    assert!(placed("back", cx).is_none());
}

/// The assistants' step has no main button while nobody is signed in, so ↩ does nothing there;
/// Just the Browser turns the threads' switch off and nothing else. Once someone is signed in,
/// its button is Next.
#[gpui::test]
fn just_the_browser_turns_threads_off_alone(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.reads_shell(shell.downgrade());
    host.threads.set(Some(true));
    host.other_apps.set(Some(true));
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    assert!(placed("main", cx).is_none(), "no main button");
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Assistant), "↩ does nothing");
    assert!(host.take().is_empty());
    click("just-the-browser", cx);
    cx.run_until_parked();
    assert_eq!(host.take(), ["threads false"]);
    assert_eq!(
        host.other_apps.get(),
        Some(true),
        "Other apps' switch is as it was"
    );
    assert_eq!(step(&shell, cx), Some(Step::Import));

    shell.update(cx, |shell, _| shell.onboarding = None);
    host.threads.set(Some(true));
    started(&shell, cx);
    let signed_in = Shown {
        line: String::new(),
        offer: Offer::SignedIn,
    };
    shell.update(cx, |shell, cx| {
        shell.onboarding_show_assistants(vec![(Cli::Claude, signed_in)], None, cx)
    });
    settle(&shell, cx);
    assert!(placed("just-the-browser", cx).is_none());
    assert!(placed("main", cx).is_some(), "Next");
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Import));
    assert!(host.take().is_empty(), "Next sets nothing");
}

/// Tidy tabs' bar starts at the profile's time and sets a new one only as ↩ goes on; Back leaves it.
#[gpui::test]
fn the_tidy_bar_sets_its_time_as_next_goes_on(cx: &mut TestAppContext) {
    let at = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        card(shell, cx, |o| {
            o.tidy.as_ref().map(|tidy| tidy.stops[tidy.at].1)
        })
    };
    for (keys, leave, set) in [
        ("right", "enter", Some("archive after 1 86400")),
        ("right right left", "enter", Some("archive after 1 86400")),
        ("", "enter", None),
        ("left", "enter", None),
        ("right", "secondary-[", None),
    ] {
        let (shell, cx, host) = first_window(cx);
        host.archive_after.set(Some(DAY / 2.));
        started(&shell, cx);
        on_to(&shell, cx, Step::Tidy);
        assert_eq!(at(&shell, cx), Some("12 hours"), "the profile's time");
        host.take();
        if !keys.is_empty() {
            cx.simulate_keystrokes(keys);
        }
        assert!(host.take().is_empty(), "{keys} set something before Next");
        cx.simulate_keystrokes(leave);
        assert_ne!(step(&shell, cx), Some(Step::Tidy), "{leave} left the step");
        let said: Vec<String> = set.into_iter().map(str::to_string).collect();
        assert_eq!(host.take(), said, "{keys} then {leave}");
    }
}

/// A click on the bar puts it at the stop nearest.
#[gpui::test]
fn a_click_on_the_tidy_bar_picks_the_stop_nearest(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.archive_after.set(Some(DAY / 2.));
    started(&shell, cx);
    on_to(&shell, cx, Step::Tidy);
    let bar = placed("tidy", cx).expect("the bar is drawn");
    let at = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        card(shell, cx, |o| o.tidy.as_ref().map(|tidy| tidy.at))
    };
    cx.simulate_click(
        point(bar.right() - px(1.), bar.center().y),
        Modifiers::none(),
    );
    assert_eq!(at(&shell, cx), Some(4), "its end is Never");
    cx.simulate_click(
        point(bar.left() + px(1.), bar.center().y),
        Modifiers::none(),
    );
    assert_eq!(at(&shell, cx), Some(0), "its start is the first time");
}

/// The tidy step tells of the broom only with threads on, and only where the broom can be.
#[gpui::test]
fn the_tidy_step_tells_of_the_broom_only_where_it_can_be(cx: &mut TestAppContext) {
    for (threads, broom, shown) in [
        (true, Some(Broom::Idle), true),
        (true, None, false),
        (false, Some(Broom::Idle), false),
    ] {
        let (shell, cx, host) = first_window(cx);
        host.archive_after.set(Some(DAY / 2.));
        host.threads.set(Some(threads));
        host.broom.set(broom);
        started(&shell, cx);
        on_to(&shell, cx, Step::Tidy);
        // On the way, Just the Browser turned threads off; as they were, for the step's own case.
        host.threads.set(Some(threads));
        settle(&shell, cx);
        assert_eq!(
            placed("broom", cx).is_some(),
            shown,
            "threads {threads}, broom {broom:?}"
        );
        host.take();
        cx.simulate_keystrokes("enter");
        assert!(
            !host.take().iter().any(|call| call == "start broom"),
            "it starts nothing"
        );
    }
}

/// Settings' times are named in the card's words, a time between them shown at the stop nearest,
/// Never past every one.
#[test]
fn the_tidy_stops_are_settings_times_in_the_cards_words() {
    let offered = [DAY / 2., DAY, 7. * DAY, 30. * DAY, -1.];
    let tidy = Tidy::of(DAY / 2., &offered).expect("a bar");
    let words: Vec<&str> = tidy.stops.iter().map(|&(_, words)| words).collect();
    assert_eq!(
        words,
        ["12 hours", "24 hours", "7 days", "30 days", "Never"]
    );
    assert_eq!(tidy.at, 0);
    let at = |was| Tidy::of(was, &offered).map(|tidy| tidy.at);
    assert_eq!(at(-1.), Some(4));
    assert_eq!(at(2. * DAY), Some(1));
    assert_eq!(at(365. * DAY), Some(3));
    assert_eq!(
        Tidy::of(DAY, &[3600.]),
        None,
        "a time with no word has no stop"
    );
    assert_eq!(stop_words(3600.), None);
}

/// Help improve is one switch for the daily count and crash reports, off until turned, set only as
/// the last step is left, so nothing is sent while it shows; turned on and off again it sets
/// nothing.
#[gpui::test]
fn help_improve_is_set_as_the_last_step_is_left(cx: &mut TestAppContext) {
    for (turns, set) in [
        (1, vec!["daily count true", "crash reports true"]),
        (2, vec![]),
        (0, vec![]),
    ] {
        let (shell, cx, host) = first_window(cx);
        host.daily_count.set(Some(false));
        host.crash_reports.set(Some(false));
        started(&shell, cx);
        on_to(&shell, cx, Step::Done);
        let offered = shell.update(cx, |shell, cx| shell.switches(Step::Done, cx));
        assert_eq!(offered, [(Switch::Help, false)], "off to begin with");
        host.take();
        for _ in 0..turns {
            shell.update(cx, |shell, cx| shell.onboarding_switch(Switch::Help, cx));
        }
        assert!(host.take().is_empty(), "nothing while the step shows");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(host.take(), set, "turned {turns} times");
    }
}

/// A build that names neither a count's server nor crash reports has no Help improve.
#[gpui::test]
fn no_help_improve_without_its_settings(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    let offered = shell.update(cx, |shell, cx| shell.switches(Step::Done, cx));
    assert!(offered.is_empty());
}

/// The last step's default browser row: Make Default asks the system, which asks the user, and
/// ↩ is Start Browsing, which doesn't. A browser that's the default at the start has no row.
#[gpui::test]
fn the_default_browser_row_asks_the_system_only_when_pressed(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    host.take();
    click("make-default", cx);
    assert_eq!(host.take(), ["make default browser"]);
    assert_eq!(step(&shell, cx), Some(Step::Done), "it stays");
    cx.simulate_keystrokes("enter");
    assert!(!host.take().contains(&"make default browser".to_string()));
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);

    host.default_browser.set(true);
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    assert!(placed("make-default", cx).is_none(), "no row");
}

/// The card ends in the first Space with the command bar up, as ⌘T has it, and its window goes.
#[gpui::test]
fn it_ends_with_the_command_bar_up(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    cx.simulate_keystrokes("enter");
    assert!(
        card(&shell, cx, |o| o.ending.is_some()),
        "Start Browsing ends it"
    );
    assert!(
        host.command_bars.borrow().is_empty(),
        "not while the card fades"
    );
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert_eq!(host.command_bars.borrow().len(), 1, "the bar is up");
    assert!(host.take().contains(&"first run ended".to_string()));
    assert!(shell.read_with(cx, |shell, _| shell.in_hint));
}

/// A quit under way closes the window the card fades from, so its end opens no command bar.
#[gpui::test]
fn a_quit_under_way_brings_no_command_bar_as_the_card_ends(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    cx.simulate_keystrokes("enter");
    crate::shell::quit_goes_ahead();
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert!(host.command_bars.borrow().is_empty());
}

/// Help › Welcome plays it again from its first step, in its window.
#[gpui::test]
fn welcome_plays_it_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::Welcome, "menu", cx));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import));
    assert!(host.take().contains(&"first run shown".to_string()));
}

/// The preview opens the card at a step by its number.
#[gpui::test]
fn the_preview_opens_the_card_at_a_step_by_number(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.archive_after.set(Some(DAY / 2.));
    let at = |number: &str, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.onboarding = None;
            shell.onboarding_at(Some("dark"), number, cx);
            shell.onboarding.as_ref().map(|onboarding| onboarding.step)
        })
    };
    assert_eq!(at("4", cx), Some(Step::Space));
    assert_eq!(at("5", cx), Some(Step::Tidy));
    assert_eq!(at("7", cx), Some(Step::Done));
}

/// The keyboard comes with each step: on its choice where the choice is the step, on its main
/// button where it has one, and on nothing where it has none, so ↩ does nothing there. A screen
/// reader reads each step as a dialog called by its title.
#[gpui::test]
fn each_step_rests_where_the_design_says(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    host.threads.set(Some(true));
    host.archive_after.set(Some(DAY / 2.));
    synced(cx, Status::SignedOut);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    let button = |name: &str| Some((Role::Button, name.to_string()));
    assert_eq!(keyed(cx), button("Continue"), "the Welcome");
    click("not-now", cx);
    settle(&shell, cx);
    assert_eq!(
        keyed(cx),
        Some((Role::Dialog, "Your assistants".into())),
        "nothing to press"
    );
    click("just-the-browser", cx);
    settle(&shell, cx);
    on_to(&shell, cx, Step::Space);
    assert_eq!(keyed(cx), Some((Role::RadioGroup, "Color".into())));
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Tidy),
        "↩ on the colours went on"
    );
    assert_eq!(keyed(cx).map(|(role, _)| role), Some(Role::Slider));
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    assert_eq!(keyed(cx), button("Start Browsing"));
}

/// Nothing of first run is made at a launch that doesn't start it: no card, no window of its own,
/// no import view.
#[gpui::test]
fn a_launch_after_first_run_makes_nothing_of_it(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert!(shell.onboarding.is_none());
        assert!(shell.first_run.is_none());
        assert!(shell.importing.is_none());
    });
    assert!(!host.take().iter().any(|call| call.contains("first run")));
}

/// Lets `seconds` of the executor's clock pass, as a timer the card set waits for.
fn waited(cx: &mut VisualTestContext, seconds: f32) {
    cx.executor()
        .advance_clock(Duration::from_secs_f32(seconds));
    cx.run_until_parked();
}

/// The import view the card holds.
fn import_view(shell: &Entity<Shell>, cx: &VisualTestContext) -> Entity<crate::import::ImportView> {
    shell
        .read_with(cx, |shell, _| {
            shell.importing.as_ref().map(|i| i.view.clone())
        })
        .expect("the import view")
}

/// The import step is the import view Settings shows: an import begun stays on its step, one whose
/// Spaces are in holds a moment on them and goes on while the rest runs, and its Done goes on. The
/// view outlives the card, so an import under way goes on, and Help › Welcome shows the same one.
#[gpui::test]
fn the_import_step_is_the_import_view(cx: &mut TestAppContext) {
    use crate::tokens::shell::onboarding::IMPORT_HOLD;
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Import));
    let shown = import_view(&shell, cx);
    assert!(
        !shown.read_with(cx, |view, _| view.is_empty()),
        "it found the made-up Arc"
    );
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Started));
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "not before its Spaces are in"
    );
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Placed));
    waited(cx, IMPORT_HOLD * 0.9);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "it holds on the Spaces"
    );
    waited(cx, IMPORT_HOLD * 0.2);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Space),
        "then goes on, offering no favorites"
    );

    shell.update(cx, |shell, _| shell.onboarding = None);
    started(&shell, cx);
    assert_eq!(
        import_view(&shell, cx).entity_id(),
        shown.entity_id(),
        "played again, the same view"
    );
    shown.update(cx, |_, cx| cx.emit(ImportEvent::Done));
    assert_eq!(step(&shell, cx), Some(Step::Space), "its Done goes on");
}

/// An import that has read brings in the favorites, the sites its history visited most, each
/// ticked: the button adds those still ticked, Skip none, and there's no going back over the import.
#[gpui::test]
fn an_import_offers_its_most_visited_sites_as_favorites(cx: &mut TestAppContext) {
    use crate::tokens::shell::onboarding::IMPORT_HOLD;
    let tab = |url: &str| crate::import::found::Tab {
        url: url.into(),
        name: String::new(),
    };
    for leave in ["enter", "skip"] {
        let (shell, cx, _) = first_window(cx);
        started(&shell, cx);
        let shown = import_view(&shell, cx);
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
        settle(&shell, cx);
        assert_eq!(step(&shell, cx), Some(Step::Favorites));
        assert!(
            placed("back", cx).is_none(),
            "no going back over the import"
        );
        assert!(placed("skip", cx).is_some(), "Skip, as some are ticked");
        shown.update(cx, |view, cx| {
            view.toggle_favorite("https://example.com/", cx)
        });
        let ticked = shell.read_with(cx, |shell, cx| shell.favorites_ticked(cx).len());
        assert_eq!(ticked, 1);
        match leave {
            "skip" => click("skip", cx),
            key => cx.simulate_keystrokes(key),
        }
        assert_eq!(step(&shell, cx), Some(Step::Space), "{leave}");
        if leave == "enter" {
            let left = shown.read_with(cx, |view, _| view.suggested().map(<[_]>::len));
            assert_eq!(left, Some(0), "the button took the offer");
        }
    }
}

/// The folder a test keeps first run's step in, the one `first_window` gave it.
fn kept_dir(cx: &mut TestAppContext) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ilo-onboarding-{}-{}",
        std::process::id(),
        cx.test_function_name().unwrap_or("test")
    ))
}

/// The card started again as at a launch, which reads the step kept.
fn relaunch(shell: &Entity<Shell>, cx: &mut VisualTestContext, dir: &std::path::Path) {
    let dir = dir.to_string_lossy().into_owned();
    cx.update(|_, cx| super::resume::kept_in(dir, cx));
    shell.update(cx, |shell, _| shell.onboarding = None);
    started(shell, cx);
}

/// What first run keeps for ⌘Q, if anything.
fn kept(dir: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(dir.join("first-run-step")).ok()
}

/// ⌘Q keeps the step's place, a number from 1 to 7 and nothing else, whatever was picked, and the
/// card's end takes it away.
#[gpui::test]
fn the_step_is_kept_as_its_number_alone_until_the_end(cx: &mut TestAppContext) {
    let dir = kept_dir(cx);
    let (shell, cx, host) = first_window(cx);
    host.archive_after.set(Some(DAY / 2.));
    started(&shell, cx);
    assert_eq!(kept(&dir).as_deref(), Some("3"), "the import's");
    on_to(&shell, cx, Step::Space);
    cx.simulate_keystrokes("right right");
    assert_eq!(kept(&dir).as_deref(), Some("4"), "the pick isn't kept");
    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("right");
    assert_eq!(kept(&dir).as_deref(), Some("5"), "nor the bar's");
    cx.simulate_keystrokes("secondary-[");
    assert_eq!(
        kept(&dir).as_deref(),
        Some("4"),
        "Back keeps the step it comes to"
    );
    on_to(&shell, cx, Step::Done);
    assert_eq!(kept(&dir).as_deref(), Some("7"));
    cx.simulate_keystrokes("enter");
    assert_eq!(kept(&dir), None, "ended, there's nothing to resume");
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert!(!super::first_run_waiting(&dir.to_string_lossy()));
}

/// A launch after a ⌘Q resumes at the step kept, or the next step this launch shows where that
/// one's condition no longer holds; a number past every step is the last, which always shows, and
/// one that isn't a number starts at the beginning. What was set comes back from the settings.
#[gpui::test]
fn a_launch_resumes_at_the_step_kept(cx: &mut TestAppContext) {
    let dir = kept_dir(cx);
    let (shell, cx, host) = first_window(cx);
    host.archive_after.set(Some(DAY));
    for (written, at) in [
        ("5", Step::Tidy),
        ("4", Step::Space),
        ("9", Step::Done),
        ("0", Step::Import),
        ("x", Step::Import),
        ("", Step::Import),
    ] {
        std::fs::write(dir.join("first-run-step"), written).unwrap();
        assert!(
            super::first_run_waiting(&dir.to_string_lossy()),
            "{written:?}"
        );
        relaunch(&shell, cx, &dir);
        assert_eq!(step(&shell, cx), Some(at), "{written:?}");
    }
    // At the tidy step the bar shows the time Settings has, as the profile kept it.
    std::fs::write(dir.join("first-run-step"), "5").unwrap();
    relaunch(&shell, cx, &dir);
    let shown = card(&shell, cx, |o| {
        o.tidy.as_ref().map(|tidy| tidy.stops[tidy.at].1)
    });
    assert_eq!(shown, Some("24 hours"));

    // Kept at the Welcome, signed in since: the next step.
    host.sync_server.set(true);
    synced(cx, Status::On);
    std::fs::write(dir.join("first-run-step"), "1").unwrap();
    relaunch(&shell, cx, &dir);
    assert_eq!(step(&shell, cx), Some(Step::Import));
    // Kept at the tidy step, which this launch leaves out: the next that shows.
    host.archive_after.set(None);
    std::fs::write(dir.join("first-run-step"), "5").unwrap();
    relaunch(&shell, cx, &dir);
    assert_eq!(step(&shell, cx), Some(Step::Done));

    // Help › Welcome in the same launch plays the card from its start, whatever is kept.
    std::fs::write(dir.join("first-run-step"), "5").unwrap();
    shell.update(cx, |shell, _| shell.onboarding = None);
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Import));
}

/// The lines a check waits on name a step by its number, a switch's or button's place, and how a
/// step was left; never what was picked, turned or typed (floor 8).
#[gpui::test]
fn no_line_names_a_pick(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.archive_after.set(Some(DAY / 2.));
    host.daily_count.set(Some(false));
    host.crash_reports.set(Some(false));
    crate::heard::listen();
    started(&shell, cx);
    on_to(&shell, cx, Step::Space);
    cx.simulate_keystrokes("right right right");
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    cx.simulate_keystrokes("right right");
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    shell.update(cx, |shell, cx| shell.onboarding_switch(Switch::Help, cx));
    settle(&shell, cx);
    cx.simulate_keystrokes("enter");
    later(&shell, cx, END_FADE);
    let lines = crate::heard::take();
    let ours: Vec<&String> = (lines.iter())
        .filter(|line| line.starts_with("browser: onboarding"))
        .collect();
    assert!(
        ours.iter().any(|line| *line == "browser: onboarding done"),
        "{ours:?}"
    );
    let picked = Theme::of_color(SWATCHES[2]).to_text();
    let words = [
        "hours", "days", "Never", "on", "off", "true", "false", "Standard",
    ];
    for line in ours {
        assert!(!line.contains(picked.as_str()), "{line:?} names {picked:?}");
        for word in line.split_whitespace() {
            assert!(!words.contains(&word), "{line:?} names {word:?}");
        }
    }
}

/// The host shapes the window to the card as the entry grows it and once as it ends, at the card's
/// size; nothing after, so nothing keeps the window busy once the motion has ended.
#[gpui::test]
fn the_window_is_shaped_by_the_entrys_frames_alone(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    let shaped = |host: &Recorder| -> Vec<String> {
        (host.take().into_iter())
            .filter(|call| call.starts_with("first run shaped"))
            .collect()
    };
    begun(&shell, cx);
    let length = Onboarding::entry_length(false);
    for _ in 0..5 {
        later(&shell, cx, length / 5.);
    }
    settle(&shell, cx);
    let calls = shaped(&host);
    assert!(calls.len() > 2, "it grows: {calls:?}");
    let whole = format!("first run shaped {} {} {}", t::WIDTH, t::HEIGHT, t::RADIUS);
    assert_eq!(calls.last(), Some(&whole), "{calls:?}");
    assert_eq!(
        calls.iter().filter(|call| **call == whole).count(),
        1,
        "{calls:?}"
    );
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    cx.simulate_keystrokes("secondary-[");
    settle(&shell, cx);
    assert_eq!(
        shaped(&host),
        Vec::<String>::new(),
        "a step's change shapes nothing"
    );
}

/// The roles and names a screen reader has of what the tree holds, once one asks.
fn read(cx: &mut VisualTestContext) -> Vec<(Role, String, Option<Toggled>)> {
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let named = tree.nodes.iter().filter_map(|(_, node)| {
        let label = node.label()?.to_string();
        Some((node.role(), label, node.toggled()))
    });
    named.collect()
}

/// The names of the dialogs `said` holds: the step shown, called by its title.
fn dialogs(said: &[(Role, String, Option<Toggled>)]) -> Vec<String> {
    let dialogs = said.iter().filter(|(role, ..)| *role == Role::Dialog);
    dialogs.map(|(_, name, _)| name.clone()).collect()
}

/// The line the step's dialog says after its title, if it has one.
fn dialog_line(cx: &mut VisualTestContext) -> Option<Option<String>> {
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    tree.nodes.into_iter().find_map(|(_, node)| {
        (node.role() == Role::Dialog).then(|| node.description().map(str::to_string))
    })
}

/// ⇥ until the keyboard is on `role` called `name`, going round the step's stops once.
fn tabbed_to(role: Role, name: &str, cx: &mut VisualTestContext) {
    for _ in 0..8 {
        if keyed(cx) == Some((role, name.to_string())) {
            return;
        }
        cx.simulate_keystrokes("tab");
    }
    panic!(
        "⇥ never came to {role:?} {name}, it ended on {:?}",
        keyed(cx)
    );
}

/// The import step with a browser of the test's own behind the view, reached from a Welcome that
/// came first, so the step has a Back, with a screen reader on.
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
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    click("not-now", cx);
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Import));
    let view = import_view(&shell, cx);
    (shell, cx, view)
}

/// ↩ signs in, as the step's button does, and the step waits while the sign-in does; an account
/// that exists asks the other devices to add this one, and once it's added and syncs, the card
/// goes on by itself.
#[gpui::test]
fn a_finished_sign_in_goes_on(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    started(&shell, cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(account_calls(&host), ["sign in"]);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Account),
        "it waits for the sign-in"
    );

    synced(cx, Status::SigningIn);
    cx.simulate_keystrokes("enter");
    assert!(account_calls(&host).is_empty(), "↩ doesn't sign in twice");
    synced(cx, Status::NeedsSecret);
    settle(&shell, cx);
    assert_eq!(
        account_calls(&host),
        ["request pairing"],
        "it asks the other devices, as Settings does"
    );
    synced(cx, Status::Pairing);
    assert_eq!(step(&shell, cx), Some(Step::Account));
    synced(cx, Status::On);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Done),
        "signed in, it went on, past what the account's Spaces bring"
    );
}

/// The account's first device shows its recovery words, which show once, so neither Esc nor ↩ goes
/// on from them and there's no Not Now; their Done does.
#[gpui::test]
fn the_first_devices_words_go_on_with_done(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    synced(cx, Status::FirstDevice);
    settle(&shell, cx);
    assert!(account_calls(&host).contains(&"recovery words".to_string()));
    // The core has made them, and syncs.
    synced(cx, Status::On);
    settle(&shell, cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::Account), "Esc keeps the words");
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Account), "so does ↩");
    assert!(shell.read_with(cx, |shell, cx| shell.keeping_words(cx)));
    assert!(
        placed("not-now", cx).is_none(),
        "no Not Now while they show"
    );
    // The words are taller than the card has room for, so the assistive app's press reaches Done.
    let done = cx.a11y_tree().expect("GPUI built a tree").nodes;
    let done = done
        .into_iter()
        .find_map(|(id, node)| {
            (node.role() == Role::Button && node.label() == Some("Done")).then_some(id)
        })
        .expect("Done is in the tree");
    cx.simulate_a11y_action(done, Action::Click);
    assert_eq!(step(&shell, cx), Some(Step::Import), "Done goes on");
}

/// A colour picked shows on the window at once, as the theme picker's does, and only Next keeps
/// it: Esc and Back set nothing of it.
#[gpui::test]
fn a_colour_picked_shows_at_once_and_only_next_keeps_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    let shown = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.theme_of(1).map(|theme| theme.main_color())
        })
    };
    for leave in ["escape", "back", "enter"] {
        started(&shell, cx);
        on_to(&shell, cx, Step::Space);
        host.take();
        cx.simulate_keystrokes("right");
        assert_eq!(
            shown(&shell, cx),
            Some(SWATCHES[0]),
            "{leave}: shown at once"
        );
        assert!(
            !host.take().iter().any(|call| call.contains("space color")),
            "{leave}: nothing kept while it shows"
        );
        match leave {
            "back" => click("back", cx),
            key => cx.simulate_keystrokes(key),
        }
        let kept = host.take().iter().any(|call| call.contains("space color"));
        assert_eq!(kept, leave == "enter", "{leave}");
        shell.update(cx, |shell, _| shell.onboarding = None);
    }
}

/// A screen reader reads each step as a dialog called by its title, its line said after, and each
/// control on it by its role and name: the buttons, the colours as one choice that says which is
/// picked, the bar, the switch (#3151). Only the step shown is in the tree.
#[gpui::test]
fn a_screen_reader_reads_each_steps_title_and_controls(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    host.archive_after.set(Some(DAY / 2.));
    // Help improve is offered where the build has its settings, as the card starts.
    host.daily_count.set(Some(false));
    host.crash_reports.set(Some(false));
    synced(cx, Status::SignedOut);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    let os = crate::default_browser::Os::Mac;
    let button = |said: &[(Role, String, Option<Toggled>)], name: &str| {
        said.contains(&(Role::Button, name.into(), None))
    };

    let (title, line) = Step::Account.message(os);
    let said = read(cx);
    assert_eq!(
        dialogs(&said),
        [title.to_string()],
        "the Welcome, by its title"
    );
    assert_eq!(dialog_line(cx), Some(Some(line.to_string())));
    assert!(
        button(&said, "Continue") && button(&said, "Not Now"),
        "{said:?}"
    );

    click("not-now", cx);
    settle(&shell, cx);
    let import = import_view(&shell, cx).read_with(cx, |view, _| view.guide());
    assert_eq!(
        dialogs(&read(cx)),
        [import.title.clone()],
        "the step it went on to only"
    );
    assert_eq!(dialog_line(cx), Some(Some(import.line)));

    on_to(&shell, cx, Step::Space);
    let said = read(cx);
    assert_eq!(dialogs(&said), [Step::Space.message(os).0.to_string()]);
    assert!(button(&said, "Back") && button(&said, "Next"), "{said:?}");
    assert!(
        (said.iter()).any(|(role, name, _)| *role == Role::RadioGroup && name == "Color"),
        "{said:?}"
    );
    let colours = |said: &[(Role, String, Option<Toggled>)]| {
        let swatches = said.iter().filter(|(role, ..)| *role == Role::RadioButton);
        swatches
            .map(|(_, name, on)| (name.clone(), *on))
            .collect::<Vec<_>>()
    };
    let unpicked = colours(&said);
    assert_eq!(unpicked.len(), SWATCHES.len(), "a radio button a colour");
    assert!(
        unpicked.iter().all(|(_, on)| *on == Some(Toggled::False)),
        "none picked yet"
    );
    cx.simulate_keystrokes("right");
    let picked = colours(&read(cx));
    assert_eq!(
        picked[0],
        (unpicked[0].0.clone(), Some(Toggled::True)),
        "→ picks the first"
    );
    assert!(
        picked[1..]
            .iter()
            .all(|(_, on)| *on == Some(Toggled::False))
    );

    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    let (title, line) = Step::Tidy.message(os);
    let said = read(cx);
    assert_eq!(dialogs(&said), [title.to_string()]);
    assert_eq!(dialog_line(cx), Some(Some(line.to_string())));
    assert!(
        (said.iter()).any(|(role, name, _)| *role == Role::Slider && name == "Put away after"),
        "{said:?}"
    );

    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    let said = read(cx);
    assert_eq!(dialogs(&said), [Step::Done.message(os).0.to_string()]);
    assert!(button(&said, "Start Browsing") && button(&said, "Make Default"));
    let help = Switch::Help.words().0.to_string();
    let said = read(cx);
    assert!(
        (said.iter()).any(|(role, name, _)| *role == Role::Switch && *name == help),
        "{said:?}"
    );
}

/// ⇥ goes along the stops of each step and reads each by its name, the colours' saying the one
/// picked as the arrows move, and ↩ on Back is Back's press alone, which keeps nothing (#3151).
#[gpui::test]
fn a_screen_reader_follows_the_keyboard_through_each_step(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.daily_count.set(Some(false));
    host.crash_reports.set(Some(false));
    cx.simulate_a11y_activation();
    started(&shell, cx);
    let guide = import_view(&shell, cx).read_with(cx, |view, _| view.guide());
    assert_eq!(
        keyed(cx),
        Some((Role::Button, guide.button.expect("the import's button"))),
        "the import rests on its button"
    );

    on_to(&shell, cx, Step::Space);
    assert_eq!(
        keyed(cx),
        Some((Role::RadioGroup, "Color".into())),
        "the colours have the keyboard as the step comes"
    );
    cx.simulate_keystrokes("right");
    let first = swatch_name(SWATCHES[0]).to_string();
    assert_eq!(
        keyed(cx),
        Some((Role::RadioButton, first)),
        "→ picks the first, which it reads"
    );
    // The foot, in its reading order: Back, then the step's button.
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Back".into())));
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Next".into())));
    cx.simulate_keystrokes("shift-tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Back".into())));
    host.take();
    press("enter", cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "↩ on Back went back, once"
    );
    let kept = (host.take().iter()).any(|call| call.starts_with("space"));
    assert!(!kept, "as Back, keeping nothing of the colour");

    // The last step's stops: its row's button, its switch, its main button and Back.
    on_to(&shell, cx, Step::Done);
    let mut names: Vec<String> = stops(cx).into_iter().map(|(_, name, _)| name).collect();
    names.sort();
    let mut expected = vec![
        "Make Default".to_string(),
        Switch::Help.words().0.to_string(),
        "Start Browsing".to_string(),
        "Back".to_string(),
    ];
    expected.sort();
    assert_eq!(names, expected, "the stops ⇥ goes along");
}

/// ↩ at the Welcome signs in once: the main button's press is the step's ↩, on its way down, and
/// doesn't come again as the key goes up.
#[gpui::test]
fn return_presses_the_welcomes_button_once(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    assert_eq!(keyed(cx), Some((Role::Button, "Continue".into())));
    account_calls(&host);
    press("enter", cx);
    let signed_in = account_calls(&host);
    let signed_in = signed_in.iter().filter(|call| *call == "sign in").count();
    assert_eq!(signed_in, 1, "↩ on Continue signed in once");
}

/// At the last step the keyboard rests on Start Browsing; ⇥ to the default browser's row, ↩ is
/// that button's press: it asks the system once and goes nowhere.
#[gpui::test]
fn return_on_a_row_button_presses_it_and_not_the_main_button(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    assert_eq!(keyed(cx), Some((Role::Button, "Start Browsing".into())));
    tabbed_to(Role::Button, "Make Default", cx);
    host.take();
    press("enter", cx);
    assert_eq!(host.take(), ["make default browser"], "asked once");
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Done), "it stays");
    assert!(card(&shell, cx, |o| o.ending.is_none()));
}

/// The step at its switch, Help improve off, the motion done, a screen reader on.
fn at_help(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = first_window(cx);
    host.daily_count.set(Some(false));
    host.crash_reports.set(Some(false));
    cx.simulate_a11y_activation();
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    (shell, cx, host)
}

/// With the keyboard on a switch, Space turns it, ↩ is still the main button and Esc still does
/// nothing: ↩ leaves the step, setting Help improve as it stands, and Esc leaves it be.
#[gpui::test]
fn return_and_escape_leave_from_a_focused_switch(cx: &mut TestAppContext) {
    for (leave, turned) in [
        ("enter", true),
        ("escape", true),
        ("enter", false),
        ("escape", false),
    ] {
        let (shell, cx, host) = at_help(cx);
        // What the steps before set as they were left.
        host.take();
        let help = Switch::Help.words().0.to_string();
        tabbed_to(Role::Switch, &help, cx);
        if turned {
            press("space", cx);
        }
        let offered = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.update(cx, |shell, cx| shell.switches(Step::Done, cx))
        };
        assert_eq!(offered(&shell, cx), [(Switch::Help, turned)], "{leave}");
        assert!(
            host.take().is_empty(),
            "nothing is set while the step shows"
        );
        press(leave, cx);
        // Read before the card's end fade takes it away.
        let ending = card(&shell, cx, |o| o.ending.is_some());
        if leave == "escape" {
            settle(&shell, cx);
            assert!(!ending, "Esc left the step");
            assert_eq!(step(&shell, cx), Some(Step::Done));
            assert!(host.take().is_empty(), "Esc set nothing, turned {turned}");
            assert_eq!(offered(&shell, cx), [(Switch::Help, turned)]);
            continue;
        }
        assert!(ending, "↩ on the switch is the main button's");
        let said = if turned {
            vec!["daily count true", "crash reports true"]
        } else {
            vec![]
        };
        let set: Vec<String> = (host.take().into_iter())
            .filter(|call| call.starts_with("daily count") || call.starts_with("crash reports"))
            .collect();
        assert_eq!(set, said, "{leave} with Help improve turned {turned}");
    }
}

/// A click on Help improve's switch, or anywhere on its row, turns it, and nothing is set while
/// the step shows; its switch lies inside its row, as its sentence once pushed it out of it (#1286).
#[gpui::test]
fn a_click_on_the_help_switch_or_its_row_turns_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.daily_count.set(Some(false));
    host.crash_reports.set(Some(false));
    started(&shell, cx);
    on_to(&shell, cx, Step::Done);
    let frame = |part, shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let onboarding = shell.onboarding.as_ref().expect("the card shows");
            onboarding
                .frames
                .borrow()
                .get(&(Switch::Help, part))
                .copied()
        })
    };
    let row = frame("row", &shell, cx).expect("its row is drawn");
    let switch = frame("switch", &shell, cx).expect("its switch is drawn");
    let inside = switch.left() >= row.left()
        && switch.right() <= row.right()
        && switch.top() >= row.top()
        && switch.bottom() <= row.bottom();
    assert!(
        inside,
        "the switch at {switch:?} is outside its row at {row:?}"
    );
    let offered = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.switches(Step::Done, cx))
    };
    host.take();
    cx.simulate_click(switch.center(), Modifiers::none());
    assert_eq!(offered(&shell, cx), [(Switch::Help, true)], "its switch");
    cx.simulate_click(
        point(row.left() + px(4.), row.center().y),
        Modifiers::none(),
    );
    assert_eq!(offered(&shell, cx), [(Switch::Help, false)], "its row");
    cx.simulate_click(switch.center(), Modifiers::none());
    assert_eq!(offered(&shell, cx), [(Switch::Help, true)]);
    assert!(
        host.take().is_empty(),
        "nothing is set while the step shows"
    );
}

/// Help improve is offered while either of its settings is, shown on only where both are on, and
/// sets each as the step is left only if it differs.
#[gpui::test]
fn help_improve_follows_the_settings_the_build_has(cx: &mut TestAppContext) {
    for (count, crashes, shown, set) in [
        // A build that names no server for the count has the crash reports alone.
        (None, Some(false), false, vec!["crash reports true"]),
        (Some(true), Some(false), false, vec!["crash reports true"]),
        (
            Some(true),
            Some(true),
            true,
            vec!["daily count false", "crash reports false"],
        ),
    ] {
        let (shell, cx, host) = first_window(cx);
        host.daily_count.set(count);
        host.crash_reports.set(crashes);
        started(&shell, cx);
        on_to(&shell, cx, Step::Done);
        let offered = shell.update(cx, |shell, cx| shell.switches(Step::Done, cx));
        assert_eq!(offered, [(Switch::Help, shown)], "{count:?} {crashes:?}");
        host.take();
        shell.update(cx, |shell, cx| shell.onboarding_switch(Switch::Help, cx));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(host.take(), set, "{count:?} {crashes:?} turned");
    }
}

/// Other apps' switch is on its own step, Settings' own and set at once, a click on it or its row
/// turning it either way; no other step has it, and nothing else it moves.
#[gpui::test]
fn other_apps_agents_have_their_row_on_their_step(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.other_apps.set(Some(false));
    started(&shell, cx);
    // A test's shell has no Agents to find a program of, so it puts the step in as the import's
    // begun puts the favorites in.
    shell.update(cx, |shell, _| {
        let steps = &mut shell.onboarding.as_mut().unwrap().steps;
        let done = steps.iter().position(|&step| step == Step::Done).unwrap();
        steps.insert(done, Step::OtherApps);
    });
    let others = shell.update(cx, |shell, cx| {
        (STEPS.iter())
            .filter(|&&step| step != Step::OtherApps)
            .any(|&step| {
                (shell.switches(step, cx).iter()).any(|(each, _)| *each == Switch::OtherApps)
            })
    });
    assert!(!others, "no other step has a row about other apps");
    on_to(&shell, cx, Step::OtherApps);
    let offered = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.switches(Step::OtherApps, cx))
    };
    assert_eq!(offered(&shell, cx), [(Switch::OtherApps, false)], "off");
    let frame = |part, shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let onboarding = shell.onboarding.as_ref().expect("the card shows");
            (onboarding.frames.borrow())
                .get(&(Switch::OtherApps, part))
                .copied()
        })
    };
    let switch = frame("switch", &shell, cx).expect("its switch is drawn");
    let row = frame("row", &shell, cx).expect("its row is drawn");
    host.take();
    cx.simulate_click(switch.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["other apps true"], "set at once");
    assert_eq!(offered(&shell, cx), [(Switch::OtherApps, true)]);
    cx.simulate_click(
        point(row.left() + px(4.), row.center().y),
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(host.take(), ["other apps false"], "and back by its row");
    assert_eq!(
        host.threads.get(),
        None,
        "the assistants' switch is as it was"
    );
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Done));
    assert!(host.take().is_empty(), "Next sets nothing more");
}

/// Safari's files kept from the app need Full Disk Access as Safari is picked: the import step
/// stays, showing the view's way to System Settings. Once it's given and the user is back, the
/// view reads Safari and shows its choices, and Import there goes on (#1378, #1673).
#[cfg(unix)]
#[gpui::test]
fn an_import_that_needs_full_disk_access_keeps_its_step(cx: &mut TestAppContext) {
    use crate::import::sources::{Os, Roots};
    use crate::import::tests::{Browser, let_read, safari};
    use crate::tokens::shell::onboarding::IMPORT_HOLD;
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
        view.read_with(cx, |view, _| view.primary()),
        Some("Open System Settings")
    );

    // With access given, coming back reads Safari and shows its choices; Import goes on.
    let_read(&home);
    cx.update(|_, cx| browser.activate(cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.primary()), Some("Import"));
    assert_eq!(step(&shell, cx), Some(Step::Import), "it waits for Import");
    view.update(cx, |view, cx| view.import(cx));
    cx.run_until_parked();
    waited(cx, IMPORT_HOLD);
    assert_ne!(step(&shell, cx), Some(Step::Import), "imported, it went on");
    std::fs::remove_dir_all(&home).unwrap();
}

/// The wait for Full Disk Access says Back in its corner, which goes back to the list, and Esc
/// does nothing there; Skip, beside it, goes on without importing.
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
    let guide = view.read_with(cx, |view, _| view.guide());
    assert_eq!(
        guide.title,
        format!("Let {} read Safari.", env!("PRODUCT_NAME"))
    );
    assert_eq!(guide.button.as_deref(), Some("Open System Settings"));
    assert_eq!(guide.corner, crate::import::Corner::Back);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import));
    assert_eq!(
        view.read_with(cx, |view, _| view.guide().title),
        guide.title,
        "Esc left the wait as it was"
    );
    click("back", cx);
    cx.run_until_parked();
    let list = view.read_with(cx, |view, _| view.guide());
    assert_eq!(list.corner, crate::import::Corner::Skip, "back at the list");
    assert_eq!(step(&shell, cx), Some(Step::Import), "keeping the step");

    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    click("skip", cx);
    assert_eq!(step(&shell, cx), Some(Step::Space), "Skip went on");
    std::fs::remove_dir_all(&home).unwrap();
}

/// A passwords file chosen at the import step keeps the step until its end, which offers to delete
/// the file; Delete File deletes it, and Done goes on (#1379).
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
    assert_eq!(view.read_with(cx, |view, _| view.primary()), Some("Done"));
    assert!(file.exists(), "nothing is deleted unasked");
    view.update(cx, |view, cx| view.delete_passwords_file(cx));
    cx.run_until_parked();
    assert!(!file.exists(), "Delete File deleted it");
    view.update(cx, |view, cx| view.done(cx));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Space), "Done goes on");
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

/// A profile that hasn't opened keeps the step, named in its title, with Try Again as its button,
/// which ↩ presses; Skip, beside it, skips the step.
#[gpui::test]
fn a_stalled_import_names_its_profile_on_the_step(cx: &mut TestAppContext) {
    use crate::import::Corner;
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    waited(cx, crate::import::STALL.as_secs_f32());
    let guide = view.read_with(cx, |view, _| view.guide());
    assert_eq!(guide.title, "Work hasn't opened.");
    assert_eq!(guide.button.as_deref(), Some("Try Again"));
    assert_eq!(guide.corner, Corner::Skip);
    assert!(
        read(cx).contains(&(Role::Button, "Try Again".into(), None)),
        "it's the step's button"
    );
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.guide().title),
        "Here it comes.",
        "↩ tried again"
    );
    waited(cx, crate::import::STALL.as_secs_f32());
    click("skip", cx);
    assert_eq!(step(&shell, cx), Some(Step::Space));
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
    std::fs::remove_dir_all(home).unwrap();
}

/// ↩ at the list imports the browser picked at once, as its Import from … does: once read, the
/// card goes on by itself while the rest comes in.
#[gpui::test]
fn return_brings_the_browser_picked_and_imports_it(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    use crate::tokens::shell::onboarding::IMPORT_HOLD;
    let browser = Rc::new(Browser::new(None));
    let (shell, cx, view) = importing_from(browser.clone(), cx);
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

/// At the import the keyboard rests on its view's button, and ↩ presses it once: the list's press
/// imports the browser at once and leaves no button for a second ↩.
#[gpui::test]
fn return_at_the_import_presses_its_button_once(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    let guide = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.guide());
    let words = guide(cx).button.expect("the list's button");
    assert_eq!(keyed(cx), Some((Role::Button, words)));
    press("enter", cx);
    assert_eq!(guide(cx).title, "Here it comes.", "↩ imports it");
    assert_eq!(
        guide(cx).button,
        None,
        "with nothing left for ↩ to press again"
    );
    assert_eq!(step(&shell, cx), Some(Step::Import));
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
    use crate::tokens::shell::onboarding::IMPORT_HOLD;
    let name = match press {
        Press::Return => "onboarding-tabs-return",
        Press::Click => "onboarding-tabs-click",
    };
    let home = chrome(name, &[("Default", "Me")], std::time::SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let (shell, cx, _) = importing_from(browser.clone(), cx);
    let weak = shell.downgrade();
    browser.reads_shell(move |cx| {
        if let Some(shell) = weak.upgrade() {
            shell.read(cx);
        }
    });
    match press {
        Press::Return => cx.simulate_keystrokes("enter"),
        Press::Click => click("main", cx),
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

/// The import step says what the state its view shows is for, as its title and line and its
/// button: the list "Bring your stuff" with Import from the browser used last and Skip, a browser's
/// choices, "What comes from Arc.", with Import and Back. Esc leaves either as it is.
#[gpui::test]
fn the_import_steps_words_follow_the_state_its_view_shows(cx: &mut TestAppContext) {
    use crate::import::Corner;
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    let guide = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.guide());
    let list = guide(cx);
    assert_eq!(list.title, "Bring your stuff");
    assert_eq!(list.button.as_deref(), Some("Import from Arc"));
    assert_eq!(list.corner, Corner::Skip);
    let said = read(cx);
    assert_eq!(dialogs(&said), [list.title.clone()], "read by its title");
    assert_eq!(dialog_line(cx), Some(Some(list.line.clone())));
    assert!(said.contains(&(Role::Button, "Import from Arc".into(), None)));
    assert!(said.contains(&(Role::Button, "Skip".into(), None)));

    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import));
    let choose = guide(cx);
    assert_eq!(choose.title, "What comes from Arc.");
    assert_eq!(choose.line, "All of it, unless you turn something off.");
    assert_eq!(choose.button.as_deref(), Some("Import"));
    assert_eq!(choose.corner, Corner::Back);
    assert_eq!(view.read_with(cx, |view, _| view.primary()), Some("Import"));
    let said = read(cx);
    assert_eq!(dialogs(&said), [choose.title.clone()]);
    assert_eq!(dialog_line(cx), Some(Some(choose.line.clone())));
    assert!(said.contains(&(Role::Button, "Import".into(), None)));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import), "Esc kept the step");
    assert_eq!(guide(cx).title, choose.title, "and the choices");
}

/// At the import, Back goes back in its view where its state has somewhere to go back to: from a
/// browser's choices to the list, keeping the step; from the list, to the step before.
#[gpui::test]
fn the_imports_back_goes_back_in_its_view_first(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    let title = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.guide().title);
    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    assert_eq!(
        title(cx),
        "What comes from Arc.",
        "the browser's choices show"
    );
    click("back", cx);
    cx.run_until_parked();
    assert_eq!(
        title(cx),
        "Bring your stuff",
        "Back in a browser's choices goes back to the list"
    );
    assert_eq!(step(&shell, cx), Some(Step::Import), "keeping the step");
    click("back", cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Account),
        "Back at the list goes to the step before"
    );
}

/// The lines of the assistants' rows as the card draws them.
fn card_lines(
    shell: &Entity<Shell>,
    agents: &Entity<crate::agents::Agents>,
    cx: &mut VisualTestContext,
) -> [String; 2] {
    use crate::agents::runner::Cli;
    let pressed = shell.read_with(cx, |shell, _| {
        (shell.onboarding.as_ref()).map_or(Vec::new(), |o| o.pressed.clone())
    });
    agents.read_with(cx, |agents, _| {
        Cli::ALL.map(|cli| super::card_row(agents, cli, pressed.contains(&cli)).line)
    })
}

/// The assistants' step runs nothing of an assistant's program as it shows, stands or shows
/// again, with the inside switch on: each row is its maker's plan and Sign In. Only a press looks,
/// for that assistant alone, and its row then says what the press learned.
#[gpui::test]
fn the_assistant_step_runs_nothing_as_it_shows(cx: &mut TestAppContext) {
    use crate::agents::ready::{Found, Ready};
    use crate::agents::runner::Cli;
    let (shell, cx, host) = first_window(cx);
    host.threads.set(Some(true));
    let signed_out = Found {
        ready: Ready::SignedOut,
        ..Default::default()
    };
    let (agents, looks) =
        cx.update(|_, cx| crate::agents::tests::stand_in(vec![(Cli::Claude, signed_out)], cx));
    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    let found =
        |cx: &mut VisualTestContext| agents.read_with(cx, |agents, _| agents.found(Cli::Claude));

    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    later(&shell, cx, 5.);
    assert_eq!(looks(), 0, "nothing looked as the step showed or stood");
    assert_eq!(found(cx), Ready::Unknown);
    assert_eq!(
        card_lines(&shell, &agents, cx),
        ["Your Claude plan", "Your ChatGPT plan"]
    );
    shell.update(cx, |shell, _| shell.onboarding = None);
    started(&shell, cx);
    later(&shell, cx, 5.);
    assert_eq!(looks(), 0, "nor as it showed again");
    assert_eq!(found(cx), Ready::Unknown);

    shell.update(cx, |shell, cx| shell.onboarding_sign_in(Cli::Claude, cx));
    cx.run_until_parked();
    assert_eq!(found(cx), Ready::SignedOut, "the press looked for Claude");
    let lines = card_lines(&shell, &agents, cx);
    assert_ne!(
        lines[0], "Your Claude plan",
        "Claude's row says what the press learned"
    );
    assert_eq!(lines[1], "Your ChatGPT plan", "ChatGPT's row is as it was");
}

/// A new profile's assistants, with the inside switch off, are each its maker's plan and a
/// Sign In, never Checking… with no button, and nothing is looked for.
#[gpui::test]
fn a_new_profiles_assistants_are_not_checking(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.threads.set(Some(false));
    let (agents, looks) = cx.update(|_, cx| crate::agents::tests::stand_in(Vec::new(), cx));
    cx.simulate_a11y_activation();
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    settle(&shell, cx);
    assert_eq!(
        card_lines(&shell, &agents, cx),
        ["Your Claude plan", "Your ChatGPT plan"]
    );
    let said = read(cx);
    let sign_ins =
        (said.iter()).filter(|(role, name, _)| *role == Role::Button && name == "Sign In");
    assert_eq!(sign_ins.count(), 2, "a Sign In on each row in {said:?}");
    assert!(!said.iter().any(|(_, name, _)| name == "Checking…"));
    assert_eq!(looks(), 0, "nothing looked");
}

/// In the card's narrow half no assistant's name is cut by its button: each button is beside the
/// whole name, or, where the row has no room for both, under it.
#[gpui::test]
fn the_assistants_names_are_whole_beside_or_over_their_buttons(cx: &mut TestAppContext) {
    use crate::agents::ready::{Found, Ready};
    use crate::agents::runner::Cli;
    use crate::tokens::settings::{
        CONTROL_GAP, END_GAP, GLYPH_PLATE, GLYPH_PLATE_GAP, PADDING, ROW_X,
    };
    let (shell, cx, host) = first_window(cx);
    // With an assistant, as its rows show only on that path.
    host.threads.set(Some(true));
    let signed_out = Found {
        ready: Ready::SignedOut,
        ..Default::default()
    };
    let (agents, _) =
        cx.update(|_, cx| crate::agents::tests::stand_in(vec![(Cli::Claude, signed_out)], cx));
    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    cx.simulate_a11y_activation();
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    // The rows draw from what the browser holds, looking for nothing.
    settle(&shell, cx);
    assert_eq!(
        card_lines(&shell, &agents, cx),
        ["Your Claude plan", "Your ChatGPT plan"]
    );

    let scale = cx.update(|window, _| window.scale_factor());
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    // The row's own button: each assistant's says Sign In.
    let button = |label: &str, row: &gpui::Bounds<gpui::Pixels>| {
        let (top, bottom) = (f32::from(row.top()), f32::from(row.bottom()));
        let bounds = (tree.nodes.iter())
            .map(|(_, node)| node)
            .filter(|node| node.role() == Role::Button && node.label() == Some(label))
            .filter_map(|node| node.bounds())
            .find(|bounds| (top..bottom).contains(&(bounds.y0 as f32 / scale)))
            .unwrap_or_else(|| panic!("no {label} button in {row:?}"));
        (bounds.x0 as f32 / scale, bounds.y0 as f32 / scale)
    };
    let system = cx.update(|_, cx| gpui::WindowTextSystem::new(cx.text_system().clone()));
    for (row, name, offer) in [
        ("claude", "Claude", "Sign In"),
        ("codex", "ChatGPT", "Sign In"),
    ] {
        let row = placed(row, cx).unwrap_or_else(|| panic!("{name}'s row wasn't drawn"));
        let whole = crate::tooltip::text_width(
            &name.into(),
            crate::tokens::text::BODY,
            gpui::FontWeight::MEDIUM,
            &system,
        );
        let name_left = f32::from(row.left()) + ROW_X + GLYPH_PLATE + GLYPH_PLATE_GAP;
        let (left, top) = button(offer, &row);
        let beside = left + 0.5 >= name_left + whole + END_GAP;
        // Under is on a line of its own, below the name's, which is at least the glyph's height,
        // not centred on a row its line made tall.
        let under = top + 0.5 >= f32::from(row.top()) + PADDING + GLYPH_PLATE + CONTROL_GAP;
        assert!(
            beside || under,
            "{offer} at ({left}, {top}) cuts {name}, {whole} wide from {name_left}, in its row at \
             {row:?}"
        );
    }
}
