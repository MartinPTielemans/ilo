//! A new profile's first launch, the card in first run's window, on a host that records what the
//! shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Entity, Focusable, Modifiers, TestAppContext, VisualTestContext, accesskit::Role, point, px,
};

use super::{END_FADE, FirstRun, Onboarding, STEPS, Step, Switch, Tidy, stop_words};
use crate::account::{self, State, Status};
use crate::agents::runner::Cli;
use crate::agents::sign_in::{Offer, Shown};
use crate::import::ImportEvent;
use crate::shell::{Broom, Folders, Shell, commands::Command, recorder::Recorder};
use crate::theme::Theme;
use crate::theme_picker::SWATCHES;
use crate::tokens::shell::onboarding::card as t;
use crate::transparency;

const DAY: f64 = 24. * 3600.;
const SERVER: &str = "https://sync.example.org";

/// A drawn shell showing one Space with no tab shown, as at a new profile's first launch, and first
/// run's window over it, which the context returned drives: its keys and clicks go to that window.
fn first_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    // The Space step keeps its level where Settings does: here, a folder of the test's own.
    let dir = std::env::temp_dir().join(format!(
        "ilo-onboarding-{}-{}",
        std::process::id(),
        cx.test_function_name().unwrap_or("test")
    ));
    _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| transparency::kept_in(dir.to_string_lossy().into_owned(), cx));
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

    shell.update(cx, |shell, cx| shell.onboarding_next("test", cx));
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
    assert_eq!(now, fraction(Step::Favorites, &shell, cx));
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
