//! A new profile's first launch, the card in first run's window, on a host that records what the
//! shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Entity, Focusable, Modifiers, TestAppContext, VisualTestContext,
    accesskit::{Role, Toggled},
    point, px,
};

use super::{END_FADE, FirstRun, Onboarding, STEPS, Step, Switch, Tidy, stop_words};
use crate::account::{self, State, Status};
use crate::agents::runner::Cli;
use crate::agents::sign_in::{Offer, Shown};
use crate::import::ImportEvent;
use crate::settings_rows::spoken::{press, stops};
use crate::shell::{Broom, Folders, Shell, commands::Command, recorder::Recorder};
use crate::tokens::shell::onboarding::card as t;
use crate::tokens::theme_panel::PALETTE_NAMES;
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

/// The account as the core says it is, with `problem`.
fn synced_with(cx: &mut VisualTestContext, status: Status, problem: Option<account::Problem>) {
    let state = State {
        status,
        problem,
        server: SERVER.into(),
        default_server: true,
        ..State::default()
    };
    cx.update(|_, cx| cx.set_global(account::Synced(state)));
    cx.run_until_parked();
}

/// What the Welcome shows of the account.
fn welcome(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> super::Welcome {
    cx.run_until_parked();
    shell.read_with(cx, |shell, cx| shell.welcome_shows(cx))
}

/// The moment "Your Spaces are syncing." holds, gone by.
fn syncing_held(cx: &mut VisualTestContext) {
    cx.executor()
        .advance_clock(Duration::from_secs_f32(t::SYNCING_HOLD));
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
    started(&shell, cx);
    assert!(placed("back", cx).is_none(), "the first step has no Back");
    on_to(&shell, cx, Step::Space);
    cx.simulate_keystrokes("right right");
    type_name("Lisbon", &shell, cx);
    assert_eq!(space_shown(&shell, cx), (Some(3), "Lisbon".into()));
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Done));
    cx.simulate_keystrokes("secondary-[");
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Space), "⌘[ went back");
    assert_eq!(
        space_shown(&shell, cx),
        (Some(3), "Lisbon".into()),
        "the palette and the name stay"
    );
    click("back", cx);
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Import), "Back went back");
}

/// The palette the Space step's editor shows chosen, and its name's field's text.
fn space_shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Option<usize>, String) {
    shell.read_with(cx, |shell, cx| {
        let editor = (shell.onboarding.as_ref())
            .and_then(|onboarding| onboarding.space.as_ref())
            .expect("the Space step's editor");
        (
            editor.picker.read(cx).palette_shown(),
            editor.name.read(cx).text().to_string(),
        )
    })
}

/// Puts `name` in the Space step's name field, as typing over its text does.
fn type_name(name: &str, shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    use gpui::EntityInputHandler as _;
    let field = shell.read_with(cx, |shell, _| {
        let editor = (shell.onboarding.as_ref()).and_then(|onboarding| onboarding.space.as_ref());
        editor.expect("the Space step's editor").name.clone()
    });
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            let all = field.text().encode_utf16().count();
            field.replace_text_in_range(Some(0..all), name, window, cx)
        })
    });
    cx.run_until_parked();
}

/// Your first Space comes with the fifth mock's look on the window, kept by nothing, and the
/// field holding Home; the arrows go along the palettes, each kept as it's picked, as the theme
/// panel keeps a pick, ← to none taking the colours away; Next names the Space.
#[gpui::test]
fn the_space_step_is_the_theme_panels_parts_for_the_first_space(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Space);
    // Just the Browser, on the way, turned threads off.
    host.take();
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.theme_preview.is_some()),
        "the mock's look shows on the window"
    );
    assert_eq!(
        space_shown(&shell, cx),
        (Some(t::SPACE_START[0] as usize), "Home".into()),
        "the mock's palette, and Home"
    );
    assert!(
        settings_set(host.take()).is_empty(),
        "shown on the window, kept by nothing yet"
    );
    cx.simulate_keystrokes("right");
    assert_eq!(space_shown(&shell, cx).0, Some(2));
    let calls = host.take();
    assert!(
        calls.iter().any(|call| call.starts_with("space color 1 ")),
        "the pick is kept as it's made: {calls:?}"
    );
    cx.simulate_keystrokes("left left left");
    assert_eq!(space_shown(&shell, cx).0, Some(0), "← stops at None");
    let calls = host.take();
    assert!(
        calls.iter().any(|call| call.starts_with("space color 1 ")),
        "← to none took the colours away, as the panel's first palette does: {calls:?}"
    );
    cx.simulate_keystrokes("right");
    host.take_edits();
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    let edits = host.take_edits();
    assert!(
        edits.contains(&"rename space 1 Home".to_string()),
        "{edits:?}"
    );
    assert_eq!(step(&shell, cx), Some(Step::Done));
}

/// The look the step comes with is only shown: Back or Next with nothing picked leaves the
/// Space's look as it was, kept and on the window.
#[gpui::test]
fn leaving_the_space_step_untouched_keeps_no_look(cx: &mut TestAppContext) {
    for how in ["back", "next"] {
        let (shell, cx, host) = first_window(cx);
        started(&shell, cx);
        on_to(&shell, cx, Step::Space);
        host.take();
        match how {
            "back" => click("back", cx),
            _ => cx.simulate_keystrokes("enter"),
        }
        settle(&shell, cx);
        let to = if how == "back" {
            Step::Import
        } else {
            Step::Done
        };
        assert_eq!(step(&shell, cx), Some(to), "{how}");
        assert_eq!(settings_set(host.take()), Vec::<String>::new(), "{how}");
        assert!(
            shell.read_with(cx, |shell, _| shell.theme_preview.is_none()
                && shell.glass_preview.is_none()),
            "{how}: the window shows the Space's own look again"
        );
    }
}

/// An empty name is Home, and one longer than the field keeps is cut to it.
#[gpui::test]
fn the_space_steps_name_is_home_when_empty_and_cut_to_the_fields_most(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Space);
    type_name("   ", &shell, cx);
    host.take_edits();
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    assert!(
        host.take_edits()
            .contains(&"rename space 1 Home".to_string())
    );
    cx.simulate_keystrokes("secondary-[");
    settle(&shell, cx);
    let long = "Lisbon trip, the long one";
    type_name(long, &shell, cx);
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    let most: String = long.chars().take(t::SPACE_NAME as usize).collect();
    assert!(
        host.take_edits()
            .contains(&format!("rename space 1 {most}")),
        "cut to {most:?}"
    );
}

/// ↩ that commits an input method's composition (Japanese) in the Space's name is the method's:
/// it doesn't pass the step, and the name waits for Next.
#[gpui::test]
fn return_ending_a_composition_in_the_spaces_name_passes_no_step(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler as _;
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    on_to(&shell, cx, Step::Space);
    let field = shell.read_with(cx, |shell, _| {
        let editor = (shell.onboarding.as_ref()).and_then(|onboarding| onboarding.space.as_ref());
        editor.expect("the Space step's editor").name.clone()
    });
    cx.update(|window, cx| {
        window.focus(&field.focus_handle(cx), cx);
        field.update(cx, |field, cx| {
            field.replace_and_mark_text_in_range(None, "いえ", None, window, cx)
        })
    });
    cx.run_until_parked();
    host.take_edits();
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Space),
        "↩ was the composition's"
    );
    assert!(
        !host
            .take_edits()
            .iter()
            .any(|call| call.starts_with("rename space")),
        "the name waits for Next"
    );
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
    assert_eq!(welcome(&shell, cx), super::Welcome::Syncing);
    assert_eq!(step(&shell, cx), Some(Step::Account), "it says so first");
    syncing_held(cx);
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
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    assert!(placed("main", cx).is_none(), "no main button");
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Assistant), "↩ does nothing");
    assert!(host.take().is_empty());
    click("just-the-browser", cx);
    cx.run_until_parked();
    assert_eq!(host.take(), ["threads false"], "and nothing else");
    assert_eq!(step(&shell, cx), Some(Step::Import));

    shell.update(cx, |shell, _| shell.onboarding = None);
    host.threads.set(Some(true));
    started(&shell, cx);
    let signed_in = Shown {
        line: String::new(),
        offer: Offer::SignedIn,
    };
    shell.update(cx, |shell, cx| {
        shell.onboarding_show_assistants(vec![(Cli::Claude, signed_in)], cx)
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

/// No blank plane is ever shown: a split step whose picture isn't in draws no plane, its column
/// centred in the card as Bring your stuff's is, in either look.
#[gpui::test]
fn a_step_without_its_picture_draws_no_plane(cx: &mut TestAppContext) {
    for light in [false, true] {
        let (shell, cx, host) = first_window(cx);
        host.threads.set(Some(true));
        host.archive_after.set(Some(DAY / 2.));
        started(&shell, cx);
        shell.update(cx, |shell, cx| {
            shell.onboarding.as_mut().unwrap().light = Some(light);
            cx.notify();
        });
        for to in [Step::Assistant, Step::Space, Step::Tidy] {
            on_to(&shell, cx, to);
            assert!(!to.split(), "{to:?}'s picture isn't in");
            assert_eq!(placed("plane", cx), None, "{to:?}, light {light}: no plane");
            let column = placed("column", cx).expect("the column is drawn");
            assert!(
                (f32::from(column.center().x) - t::WIDTH / 2.).abs() < 0.5,
                "{to:?}, light {light}: the column is centred, at {column:?}"
            );
        }
    }
}

/// How strong `ink` reads on the card's sheet, as a share of the text's own ink: each drawn over
/// the opaque sheet, how far it stands from it.
fn strength(ink: gpui::Rgba, inks: super::card::Inks) -> f32 {
    let sheet = inks.fill(false);
    let over = |ink: gpui::Rgba| {
        [ink.r, ink.g, ink.b]
            .into_iter()
            .zip([sheet.r, sheet.g, sheet.b])
            .map(|(ink_, sheet)| (ink_ * ink.a + sheet * (1. - ink.a) - sheet).abs())
            .fold(0., f32::max)
    };
    over(ink) / over(inks.text(1.))
}

/// No text on a first-run card reads under text-3 unless its control is off: a row's second line
/// is text-2 on the switches' rows as on the card's own, in Dark and in Light.
#[gpui::test]
fn no_text_on_the_card_reads_under_text_3(_: &mut TestAppContext) {
    for light in [false, true] {
        let inks = super::card::Inks::of(light);
        let look = super::switch_look(inks);
        for (what, ink) in [
            ("a row's name", look.ink),
            ("a row's second line", look.detail),
            ("the card's second ink", inks.text_2()),
            ("the card's third ink", inks.text(t::TEXT_3)),
        ] {
            assert!(
                strength(ink, inks) >= t::TEXT_3 - 0.01,
                "{what}, light {light}: reads at {}",
                strength(ink, inks)
            );
        }
        assert_eq!(
            look.detail,
            inks.text_2(),
            "light {light}: text-2, one of the three"
        );
    }
}

/// The main button filled with the text's ink goes a step toward its label's under the pointer
/// and a step further pressed, as every ink-filled button does, in both looks: never lighter or
/// darker by an opacity that lets the card through.
#[gpui::test]
fn the_main_button_goes_toward_its_label_under_the_pointer(_: &mut TestAppContext) {
    use crate::tokens::palette::{INK_HOVER_MIX, INK_PRESSED_MIX};
    for light in [false, true] {
        let inks = super::card::Inks::of(light);
        let [rest, hovered, pressed] = super::card::main_fills(inks);
        let label = inks.on_main();
        assert_eq!(
            rest,
            inks.text(1.),
            "light {light}: at rest, the text's ink"
        );
        for (what, fill, mix) in [
            ("hovered", hovered, INK_HOVER_MIX),
            ("pressed", pressed, INK_PRESSED_MIX),
        ] {
            assert_eq!(fill.a, 1., "light {light}: {what} is opaque");
            for (fill, rest, label) in [
                (fill.r, rest.r, label.r),
                (fill.g, rest.g, label.g),
                (fill.b, rest.b, label.b),
            ] {
                assert!(
                    (fill - (rest + (label - rest) * mix)).abs() < 1e-4,
                    "light {light}: {what} is {mix} of the way to its label"
                );
            }
        }
    }
}

/// The account's view on the card takes the switches' look whole: its signed-in dot, the look's
/// primary, is the text's ink as every mark on the card is, in both looks.
#[gpui::test]
fn the_accounts_dot_is_the_cards_ink(_: &mut TestAppContext) {
    for light in [false, true] {
        let inks = super::card::Inks::of(light);
        let look = super::switch_look(inks);
        assert_eq!(
            look.primary, look.ink,
            "light {light}: the dot is the look's ink"
        );
        assert_eq!(look.ink, inks.text(1.), "light {light}: the text's own");
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

/// Help › Welcome's replay keeps no step, going on or back: quit halfway, the next launch doesn't
/// open first run by itself.
#[gpui::test]
fn a_replay_keeps_no_step_to_resume(cx: &mut TestAppContext) {
    let dir = kept_dir(cx);
    let (shell, cx, host) = first_window(cx);
    host.archive_after.set(Some(DAY / 2.));
    shell.update(cx, |shell, cx| shell.run(Command::Welcome, "menu", cx));
    shell.update(cx, |shell, cx| {
        shell.onboarding.as_mut().unwrap().end_entry();
        cx.notify();
    });
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Import));
    on_to(&shell, cx, Step::Tidy);
    cx.simulate_keystrokes("secondary-[");
    assert_eq!(step(&shell, cx), Some(Step::Space));
    assert_eq!(kept(&dir), None, "the replay kept no step");
    assert!(!super::first_run_waiting(&dir.to_string_lossy()));
}

/// The calls of `calls` that set a switch or a setting the card offers.
fn settings_set(calls: Vec<String>) -> Vec<String> {
    const SET: [&str; 10] = [
        "threads ",
        "other apps ",
        "daily count ",
        "crash reports ",
        "archive after ",
        "space color ",
        "space theme ",
        "theme ",
        "search engine ",
        "passwords ",
    ];
    (calls.into_iter())
        .filter(|call| SET.iter().any(|set| call.starts_with(set)))
        .collect()
}

/// A first run passed through, each step left the way that takes nothing it offers (Not Now,
/// Next with an assistant signed in, Skip, ↩ with nothing picked) to Start Browsing, sets no
/// switch and no setting.
#[gpui::test]
fn passing_each_step_sets_no_switch(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    host.threads.set(Some(true));
    host.archive_after.set(Some(DAY / 2.));
    host.daily_count.set(Some(false));
    host.crash_reports.set(Some(false));
    started(&shell, cx);
    let signed_in = Shown {
        line: String::new(),
        offer: Offer::SignedIn,
    };
    shell.update(cx, |shell, cx| {
        shell.onboarding_show_assistants(vec![(Cli::Claude, signed_in)], cx)
    });
    host.take();
    let mut passed = Vec::new();
    for _ in 0..16 {
        let Some(at) = step(&shell, cx) else { break };
        if card(&shell, cx, |o| o.ending.is_some()) {
            break;
        }
        passed.push(at);
        match at {
            Step::Account => click("not-now", cx),
            Step::Import if placed("skip", cx).is_some() => click("skip", cx),
            _ => cx.simulate_keystrokes("enter"),
        }
        settle(&shell, cx);
    }
    assert_eq!(
        passed,
        [
            Step::Account,
            Step::Assistant,
            Step::Import,
            Step::Space,
            Step::Tidy,
            Step::Done
        ]
    );
    // The last settle has played the end's fade, and the card has gone.
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), None, "Start Browsing ended the card");
    assert_eq!(settings_set(host.take()), Vec::<String>::new());
}

/// ⌘Q at a step sets nothing turned there: the tidy bar, Help improve. They're set only as their
/// step is left. The Space's look is kept as it's picked, as the theme panel keeps it, so a quit
/// there resumes with it.
#[gpui::test]
fn a_quit_sets_nothing_turned_at_its_step(cx: &mut TestAppContext) {
    for at in [Step::Space, Step::Tidy, Step::Done] {
        let (shell, cx, host) = first_window(cx);
        host.archive_after.set(Some(DAY / 2.));
        host.daily_count.set(Some(false));
        host.crash_reports.set(Some(false));
        started(&shell, cx);
        on_to(&shell, cx, at);
        host.take();
        match at {
            Step::Space => cx.simulate_keystrokes("right"),
            Step::Tidy => cx.simulate_keystrokes("right"),
            _ => shell.update(cx, |shell, cx| shell.onboarding_switch(Switch::Help, cx)),
        }
        let picked = settings_set(host.take());
        shell.update(cx, |shell, cx| shell.run(Command::Quit, "keys", cx));
        cx.run_until_parked();
        assert_eq!(settings_set(host.take()), Vec::<String>::new(), "{at:?}");
        let kept = picked.iter().any(|call| call.starts_with("space color 1 "));
        assert_eq!(kept, at == Step::Space, "{at:?}: {picked:?}");
        assert_eq!(
            step(&shell, cx),
            Some(at),
            "{at:?}: the step stands for the quit"
        );
    }
}

/// ↩ that commits an input method's composition (Japanese) in the Welcome's recovery words is
/// the method's: it doesn't pass the step, nor press its button.
#[gpui::test]
fn return_ending_a_composition_passes_no_step(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler as _;
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    cx.simulate_keystrokes("enter");
    synced(cx, Status::Pairing);
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Account));
    // Use Recovery Words Instead, where the screen reader has it; its field takes the keyboard.
    let scale = cx.update(|window, _| window.scale_factor());
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let words = (tree.nodes.iter())
        .map(|(_, node)| node)
        .find(|node| node.role() == Role::Button && node.label() == Some("Use Recovery Words"))
        .and_then(|node| node.bounds())
        .expect("Use Recovery Words");
    let at = gpui::point(
        px(((words.x0 + words.x1) / 2.) as f32 / scale),
        px(((words.y0 + words.y1) / 2.) as f32 / scale),
    );
    cx.simulate_click(at, Modifiers::none());
    settle(&shell, cx);
    let view = shell.read_with(cx, |shell, _| {
        shell.account_view().expect("the account's view")
    });
    let field = view.read_with(cx, |view, _| view.open_field().expect("the words' field"));
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_and_mark_text_in_range(None, "ことば", None, window, cx)
        })
    });
    cx.run_until_parked();
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(
        step(&shell, cx),
        Some(Step::Account),
        "↩ was the composition's"
    );
    assert!(
        field.read_with(cx, |field, _| field.composing()),
        "the marked text is the method's"
    );
    assert!(host.take().is_empty(), "nothing was asked of the core");
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
    assert_eq!(
        keyed(cx),
        button("Next"),
        "the Space's parts are the pointer's and the arrows'"
    );
    cx.simulate_keystrokes("enter");
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Tidy), "↩ on Next went on");
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

/// What the card draws of the import view.
fn brought(
    view: &Entity<crate::import::ImportView>,
    cx: &mut VisualTestContext,
) -> crate::import::Bring {
    view.update(cx, |view, _| view.card_state(0))
}

/// The chips the card draws, as picked or as an import brings them.
fn chips(
    view: &Entity<crate::import::ImportView>,
    cx: &mut VisualTestContext,
) -> Vec<crate::import::Chip> {
    use crate::import::Bringing;
    match brought(view, cx).state {
        Bringing::Kinds(chips) | Bringing::Running { chips, .. } => chips,
        state => panic!("no chips: {state:?}"),
    }
}

/// An import from the card gets all the way: its Spaces in, then its history and addresses.
fn brought_in(browser: &crate::import::tests::Browser, cx: &mut VisualTestContext) {
    cx.update(|_, cx| browser.load(cx));
    cx.run_until_parked();
    cx.update(|_, cx| browser.added(cx));
    cx.run_until_parked();
}

/// The import step's view outlives the card, so an import under way goes on, and Help › Welcome
/// shows the same one.
#[gpui::test]
fn the_import_step_keeps_its_view(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Import));
    let shown = import_view(&shell, cx);
    assert!(
        !shown.read_with(cx, |view, _| view.is_empty()),
        "it found the made-up Arc"
    );
    shell.update(cx, |shell, _| shell.onboarding = None);
    started(&shell, cx);
    assert_eq!(
        import_view(&shell, cx).entity_id(),
        shown.entity_id(),
        "played again, the same view"
    );
}

/// An import that has read brings in the favorites, the sites its history visited most, each
/// ticked: the button adds those still ticked, Skip none, and there's no going back over the import.
/// With none ticked, Next alone goes on.
#[gpui::test]
fn an_import_offers_its_most_visited_sites_as_favorites(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let tab = |url: &str| crate::import::found::Tab {
        url: url.into(),
        name: String::new(),
    };
    for leave in ["enter", "skip", "none"] {
        let browser = Rc::new(Browser::new(None));
        let (shell, cx, view) = importing_from(browser.clone(), cx);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        brought_in(&browser, cx);
        let offered = vec![
            tab("https://example.com/"),
            tab("https://news.example.org/"),
        ];
        view.update(cx, |view, _| view.offer(offered));
        waited(cx, 10.);
        settle(&shell, cx);
        assert_eq!(step(&shell, cx), Some(Step::Favorites), "{leave}");
        assert!(
            placed("back", cx).is_none(),
            "no going back over the import"
        );
        assert!(placed("skip", cx).is_some(), "Skip, as some are ticked");
        assert!(placed("tile 1", cx).is_some() && placed("tile 2", cx).is_none());
        click("tile 0", cx);
        let ticked = shell.read_with(cx, |shell, cx| shell.favorites_ticked(cx).len());
        assert_eq!(ticked, 1, "the click left one out");
        match leave {
            "skip" => click("skip", cx),
            "none" => {
                click("tile 1", cx);
                assert!(placed("skip", cx).is_none(), "none ticked, no Skip");
                assert!(read(cx).contains(&(Role::Button, "Next".into(), None)));
                cx.simulate_keystrokes("enter");
            }
            key => cx.simulate_keystrokes(key),
        }
        assert_eq!(step(&shell, cx), Some(Step::Space), "{leave}");
        if leave == "enter" {
            let left = view.read_with(cx, |view, _| view.suggested().map(<[_]>::len));
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
    let words = [
        "hours", "days", "Never", "on", "off", "true", "false", "Standard",
    ];
    assert!(
        !(lines.iter()).any(|line| line.starts_with("browser: theme picker")),
        "the Space's editor says nothing of its picks"
    );
    for line in ours {
        for name in PALETTE_NAMES {
            assert!(!line.contains(name), "{line:?} names {name:?}");
        }
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
    syncing_held(cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Done),
        "signed in, it went on, past what the account's Spaces bring"
    );
}

/// The account's first device shows its recovery words, which show once: Esc keeps them and
/// there's no Not Now; Copy copies them through the build's own way, and their main button, I've
/// Saved Them, goes on, as ↩ does (v9 §3.3).
#[gpui::test]
fn the_first_devices_words_go_on_with_ive_saved_them(cx: &mut TestAppContext) {
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
    assert_eq!(welcome(&shell, cx), super::Welcome::Words);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::Account), "Esc keeps the words");
    assert!(account_calls(&host).is_empty(), "and asks nothing");
    assert!(
        placed("not-now", cx).is_none(),
        "no Not Now while they show"
    );
    // All 24, counted, never read out.
    let shown = shell.read_with(cx, |shell, cx| {
        (shell.account_view()).and_then(|view| view.read(cx).shown_words().map(<[String]>::len))
    });
    assert_eq!(shown, Some(24));
    click("copy", cx);
    assert_eq!(account_calls(&host), ["copy secret"]);
    let (copied, noted) = shell.read_with(cx, |shell, cx| {
        let now = cx.background_executor().now();
        (shell.account_view()).map_or((false, false), |view| view.read(cx).copied(now))
    });
    assert!(
        copied && noted,
        "Copied, with the note on where the copy went"
    );
    // The assistive app's press reaches the main button.
    let saved = cx.a11y_tree().expect("GPUI built a tree").nodes;
    assert!(
        (saved.iter()).any(|(_, node)| {
            node.role() == Role::Button && node.label() == Some("I've Saved Them")
        }),
        "I've Saved Them is in the tree"
    );
    cx.simulate_keystrokes("enter");
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "↩ goes on, as the button does"
    );
}

/// Esc stops a wait under way, the sign-in window or the request to the other devices, and never
/// passes the step; Not Now while the sign-in window is open closes it as it goes on.
#[gpui::test]
fn esc_stops_a_wait_and_never_passes_the_welcome(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    started(&shell, cx);
    cx.simulate_keystrokes("escape");
    assert!(
        account_calls(&host).is_empty(),
        "nothing to stop in the form"
    );
    assert_eq!(step(&shell, cx), Some(Step::Account));

    cx.simulate_keystrokes("enter");
    synced(cx, Status::SigningIn);
    account_calls(&host);
    cx.simulate_keystrokes("escape");
    assert_eq!(
        account_calls(&host),
        ["cancel"],
        "the sign-in window closes"
    );
    assert_eq!(step(&shell, cx), Some(Step::Account));

    synced(cx, Status::NeedsSecret);
    settle(&shell, cx);
    assert_eq!(account_calls(&host), ["request pairing"]);
    synced(cx, Status::Pairing);
    assert_eq!(welcome(&shell, cx), super::Welcome::Approve);
    assert!(placed("not-now", cx).is_none(), "no Not Now while it waits");
    cx.simulate_keystrokes("escape");
    assert_eq!(account_calls(&host), ["cancel"], "the request stops");
    assert_eq!(step(&shell, cx), Some(Step::Account));
    synced(cx, Status::NeedsSecret);
    assert_eq!(welcome(&shell, cx), super::Welcome::Again);
    cx.simulate_keystrokes("escape");
    assert!(account_calls(&host).is_empty(), "nothing more to stop");
    assert_eq!(step(&shell, cx), Some(Step::Account));
    shell.update(cx, |shell, _| shell.onboarding = None);

    synced(cx, Status::SignedOut);
    started(&shell, cx);
    cx.simulate_keystrokes("enter");
    synced(cx, Status::SigningIn);
    account_calls(&host);
    click("not-now", cx);
    assert_eq!(
        account_calls(&host),
        ["cancel"],
        "Not Now closes the window"
    );
    assert_eq!(step(&shell, cx), Some(Step::Import));
}

/// No device added this one: the build's line says why, Ask Again asks again, and Use Recovery
/// Words opens their field, whose ↩ is Restore's and never passes the step; restored, the
/// Welcome says the Spaces are syncing, then goes on.
#[gpui::test]
fn a_new_device_asks_again_or_restores_with_its_words(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler as _;
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    started(&shell, cx);
    cx.simulate_keystrokes("enter");
    synced(cx, Status::NeedsSecret);
    settle(&shell, cx);
    synced_with(cx, Status::NeedsSecret, Some(account::Problem::Declined));
    assert_eq!(welcome(&shell, cx), super::Welcome::Again);
    assert!(placed("not-now", cx).is_some(), "Not Now goes on from here");
    account_calls(&host);
    cx.simulate_keystrokes("enter");
    assert!(account_calls(&host).is_empty(), "↩ has no button here");
    click("ask-again", cx);
    assert_eq!(account_calls(&host), ["request pairing"]);

    click("use-words", cx);
    assert_eq!(welcome(&shell, cx), super::Welcome::Restore);
    settle(&shell, cx);
    let field = shell.read_with(cx, |shell, cx| {
        (shell.account_view()).and_then(|view| view.read(cx).open_field())
    });
    let field = field.expect("the words' field");
    assert!(
        cx.update(|window, cx| field.focus_handle(cx).is_focused(window)),
        "it takes the keyboard"
    );
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_text_in_range(None, "made up words", window, cx)
        })
    });
    account_calls(&host);
    cx.simulate_keystrokes("enter");
    assert_eq!(account_calls(&host), ["words"], "↩ in the field is Restore");
    assert_eq!(
        step(&shell, cx),
        Some(Step::Account),
        "and doesn't pass the step"
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(
        welcome(&shell, cx),
        super::Welcome::Again,
        "Esc leaves the field"
    );
    assert_eq!(step(&shell, cx), Some(Step::Account));
    click("use-words", cx);
    click("main", cx);
    assert_eq!(account_calls(&host), ["words"], "Restore, clicked");
    synced(cx, Status::On);
    assert_eq!(welcome(&shell, cx), super::Welcome::Syncing);
    syncing_held(cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Done),
        "a returning account's steps"
    );
}

/// ⌘Q while the account waits for its words or another device's approval resumes at the Welcome,
/// which shows that wait again; the kept number is the step's alone.
#[gpui::test]
fn a_quit_while_the_account_waits_resumes_at_the_welcome(cx: &mut TestAppContext) {
    let dir = kept_dir(cx);
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    started(&shell, cx);
    assert_eq!(kept(&dir).as_deref(), Some("1"));
    assert!(
        placed("sign-up", cx).is_none(),
        "one sign-in window, no Sign Up"
    );
    cx.simulate_keystrokes("enter");
    synced(cx, Status::NeedsSecret);
    settle(&shell, cx);
    assert_eq!(kept(&dir).as_deref(), Some("1"));
    synced(cx, Status::Pairing);
    relaunch(&shell, cx, &dir);
    assert_eq!(step(&shell, cx), Some(Step::Account));
    assert_eq!(welcome(&shell, cx), super::Welcome::Approve);
    synced(cx, Status::On);
    syncing_held(cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Done),
        "a returning account, past what its Spaces bring"
    );
}

/// The look the Space step shows is on the window at once, as the theme panel's is: the mock's
/// as it comes, and each palette as it's picked.
#[gpui::test]
fn the_space_steps_look_shows_on_the_window_at_once(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    let shown = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.theme_of(1).map(|theme| theme.main_color())
        })
    };
    let main = |index: usize| {
        crate::theme_picker::palette_theme(index, None).map(|theme| theme.main_color())
    };
    started(&shell, cx);
    on_to(&shell, cx, Step::Space);
    assert_eq!(
        shown(&shell, cx),
        main(t::SPACE_START[0] as usize),
        "as it comes"
    );
    cx.simulate_keystrokes("right");
    assert_eq!(
        shown(&shell, cx),
        main(t::SPACE_START[0] as usize + 1),
        "picked"
    );
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
    let (title, line) = Step::Import.message(card(&shell, cx, |o| o.os));
    assert_eq!(
        dialogs(&read(cx)),
        [title.to_string()],
        "the step it went on to only"
    );
    assert_eq!(dialog_line(cx), Some(Some(line.to_string())));

    on_to(&shell, cx, Step::Space);
    let said = read(cx);
    assert_eq!(dialogs(&said), [Step::Space.message(os).0.to_string()]);
    assert!(button(&said, "Back") && button(&said, "Next"), "{said:?}");
    let palettes = |said: &[(Role, String, Option<Toggled>)]| {
        (said.iter())
            .filter(|(role, name, _)| {
                *role == Role::RadioButton && PALETTE_NAMES.contains(&name.as_str())
            })
            .map(|(_, name, on)| (name.clone(), *on))
            .collect::<Vec<_>>()
    };
    let shown = palettes(&said);
    assert_eq!(shown.len(), PALETTE_NAMES.len(), "a radio button a palette");
    let chosen = |shown: &[(String, Option<Toggled>)]| {
        (shown.iter())
            .filter(|(_, on)| *on == Some(Toggled::True))
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    };
    let first = PALETTE_NAMES[t::SPACE_START[0] as usize].to_string();
    assert_eq!(chosen(&shown), [first], "the mock's palette");
    cx.simulate_keystrokes("right");
    let next = PALETTE_NAMES[t::SPACE_START[0] as usize + 1].to_string();
    assert_eq!(chosen(&palettes(&read(cx))), [next], "→ picks the next");

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
    assert_eq!(
        keyed(cx),
        Some((Role::Button, "Import".into())),
        "the import rests on its button"
    );

    on_to(&shell, cx, Step::Space);
    assert_eq!(
        keyed(cx),
        Some((Role::Button, "Next".into())),
        "the step rests on Next, the arrows going along the palettes"
    );
    // The foot, in its reading order: Back, then the step's button.
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
    assert!(!kept, "as Back, keeping nothing of the look shown");

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

/// ⇥ from the Welcome's Continue comes to Not Now, and ↩ there goes on without signing in: it's
/// the way the checks pass the Welcome, so a slip here would open the sign-in page on the hosted
/// server.
#[gpui::test]
fn tab_from_continue_comes_to_not_now_whose_return_signs_nothing_in(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced(cx, Status::SignedOut);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    assert_eq!(welcome(&shell, cx), super::Welcome::Form);
    assert_eq!(keyed(cx), Some((Role::Button, "Continue".into())));
    cx.simulate_keystrokes("tab");
    assert_eq!(
        keyed(cx),
        Some((Role::Button, "Not Now".into())),
        "⇥ from Continue"
    );
    account_calls(&host);
    crate::heard::listen();
    press("enter", cx);
    let lines = crate::heard::take();
    assert!(
        account_calls(&host).is_empty(),
        "↩ on Not Now signed nothing in"
    );
    let signed = lines
        .iter()
        .any(|line| line.contains("onboarding sign in via"));
    assert!(!signed, "{lines:?}");
    assert_eq!(step(&shell, cx), Some(Step::Import), "Not Now went on");
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

/// Other apps comes only where the stat at the card's start finds an agent program's files:
/// Claude Code's or Codex's, on a made-up computer; none found, or only another MCP client, and
/// the step is left out. Nothing is looked for of an assistant's program.
#[gpui::test]
fn a_found_agent_program_brings_other_apps(cx: &mut TestAppContext) {
    use crate::agents::Client;
    for (found, shown) in [
        (vec![Client::ClaudeCode], true),
        (vec![Client::Codex], true),
        (vec![], false),
    ] {
        let (shell, cx, _host) = first_window(cx);
        let (_agents, looks) =
            cx.update(|_, cx| crate::agents::tests::stand_in_with(Vec::new(), found.clone(), cx));
        started(&shell, cx);
        assert_eq!(
            steps(&shell, cx).contains(&Step::OtherApps),
            shown,
            "{found:?}"
        );
        assert_eq!(looks(), 0, "{found:?}: no program looked for");
    }
}

/// A first launch starts the card before the launch has made the Agents, and a program found
/// still brings Other apps.
#[gpui::test]
fn a_found_agent_program_brings_other_apps_before_the_launch_makes_the_agents(
    cx: &mut TestAppContext,
) {
    let (shell, cx, host) = first_window(cx);
    host.agents_found
        .replace(Some(vec![crate::agents::Client::ClaudeCode]));
    started(&shell, cx);
    assert!(steps(&shell, cx).contains(&Step::OtherApps));
}

/// Other apps' two switches are on their own step, off as it shows, each Settings' own and set at
/// once, a click on it or its card turning it either way. Either on turns the outside switch on as
/// it lands; it goes off only as the second is turned off. No other step has them, and nothing
/// else moves.
#[gpui::test]
fn other_apps_two_ways_are_off_as_they_show_and_either_turns_the_outside_switch_on(
    cx: &mut TestAppContext,
) {
    use crate::agents::Client;
    let (shell, cx, host) = first_window(cx);
    let found = vec![Client::ClaudeCode, Client::Codex];
    let (agents, heard, _) =
        cx.update(|_, cx| crate::agents::tests::stand_in_failing(found.clone(), None, cx));
    let outside =
        |cx: &mut VisualTestContext| agents.read_with(cx, |agents, _| agents.other_apps());
    started(&shell, cx);
    assert!(steps(&shell, cx).contains(&Step::OtherApps));
    let others = shell.update(cx, |shell, cx| {
        (STEPS.iter())
            .filter(|&&step| step != Step::OtherApps)
            .any(|&step| {
                (shell.switches(step, cx).iter())
                    .any(|(each, _)| matches!(each, Switch::Command | Switch::Mcp))
            })
    });
    assert!(!others, "no other step has a way in");
    on_to(&shell, cx, Step::OtherApps);
    let offered = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.switches(Step::OtherApps, cx))
    };
    assert_eq!(
        offered(&shell, cx),
        [(Switch::Command, false), (Switch::Mcp, false)],
        "both off as it shows"
    );
    let frame = |switch, part, shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let onboarding = shell.onboarding.as_ref().expect("the card shows");
            (onboarding.frames.borrow()).get(&(switch, part)).copied()
        })
    };
    let command = frame(Switch::Command, "switch", &shell, cx).expect("its switch is drawn");
    let mcp = frame(Switch::Mcp, "row", &shell, cx).expect("its card is drawn");
    let at_left = |card: gpui::Bounds<gpui::Pixels>| point(card.left() + px(4.), card.center().y);
    host.take();

    assert!(!outside(cx));
    cx.simulate_click(command.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(outside(cx), "on as the command landed");
    assert_eq!(
        offered(&shell, cx),
        [(Switch::Command, true), (Switch::Mcp, false)]
    );
    assert!(agents.read_with(cx, |agents, _| agents.is_connected(Client::Command)));
    assert_eq!(
        heard().0,
        [Client::Command],
        "no app connected by the command"
    );

    cx.simulate_click(at_left(mcp), Modifiers::none());
    cx.run_until_parked();
    assert!(outside(cx), "still on");
    assert_eq!(
        heard().0,
        [Client::Command, Client::ClaudeCode, Client::Codex]
    );
    assert_eq!(
        offered(&shell, cx),
        [(Switch::Command, true), (Switch::Mcp, true)]
    );

    cx.simulate_click(command.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(outside(cx), "on while MCP is");
    cx.simulate_click(at_left(mcp), Modifiers::none());
    cx.run_until_parked();
    assert!(!outside(cx), "off with both");
    assert_eq!(
        heard().1,
        [Client::Command, Client::ClaudeCode, Client::Codex]
    );
    assert_eq!(
        offered(&shell, cx),
        [(Switch::Command, false), (Switch::Mcp, false)],
        "off takes away what on added"
    );
    assert_eq!(
        host.threads.get(),
        None,
        "the assistants' switch is as it was"
    );
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Done));
    assert!(host.take().is_empty(), "Next sets nothing more");
    assert!(!outside(cx));
}

/// A way whose Add fails turns back off and says so in its small line: the command's, or MCP's
/// for each app that fell, on while one is connected. The outside switch is as it was before the
/// click while the other way is off, and stays on while it's on.
#[gpui::test]
fn a_way_that_fails_turns_back_off_and_says_so(cx: &mut TestAppContext) {
    use super::other_apps;
    use crate::agents::Client;
    let found = vec![Client::ClaudeCode, Client::Codex];
    let failed = |switch| match switch {
        Switch::Command => "Couldn't add the command.",
        _ => "Couldn't connect Claude Code and Codex.",
    };
    for (switch, other) in [
        (Switch::Command, Switch::Mcp),
        (Switch::Mcp, Switch::Command),
    ] {
        for other_on in [false, true] {
            let (shell, cx, _host) = first_window(cx);
            let (agents, _, fail_next) =
                cx.update(|_, cx| crate::agents::tests::stand_in_failing(found.clone(), None, cx));
            let outside =
                |cx: &mut VisualTestContext| agents.read_with(cx, |agents, _| agents.other_apps());
            started(&shell, cx);
            on_to(&shell, cx, Step::OtherApps);
            if other_on {
                shell.update(cx, |shell, cx| shell.onboarding_switch(other, cx));
                cx.run_until_parked();
                assert!(outside(cx));
            }
            // Each of the way's writes fails: MCP's both apps, so nothing of it lands.
            fail_next(2);
            shell.update(cx, |shell, cx| shell.onboarding_switch(switch, cx));
            cx.run_until_parked();
            let way = agents.read_with(cx, |agents, _| match switch {
                Switch::Command => other_apps::command(agents),
                _ => other_apps::mcp(agents),
            });
            assert!(!way.on, "{switch:?} {other_on}");
            assert_eq!(way.small.as_ref(), failed(switch), "{switch:?} {other_on}");
            assert_eq!(outside(cx), other_on, "{switch:?}: as before the click");
        }
    }
}

/// MCP's way with one app that fails and one that lands is on, says which fell, and turns the
/// outside switch on with the one that landed.
#[gpui::test]
fn a_way_that_lands_for_one_app_turns_the_outside_switch_on(cx: &mut TestAppContext) {
    use super::other_apps;
    use crate::agents::Client;
    let found = vec![Client::ClaudeCode, Client::Codex];
    let (shell, cx, _host) = first_window(cx);
    let (agents, _, _) = cx.update(|_, cx| {
        crate::agents::tests::stand_in_failing(found.clone(), Some("not installed"), cx)
    });
    started(&shell, cx);
    on_to(&shell, cx, Step::OtherApps);
    shell.update(cx, |shell, cx| shell.onboarding_switch(Switch::Mcp, cx));
    cx.run_until_parked();
    let (way, outside) = agents.read_with(cx, |agents, _| {
        (other_apps::mcp(agents), agents.other_apps())
    });
    assert!(way.on);
    assert_eq!(way.small.as_ref(), "Connected Codex. Claude Code failed.");
    assert!(outside);
}

/// Safari's files kept from the app need Full Disk Access as Safari is read: the step stays,
/// showing the way to System Settings, and Esc leaves it as it is. Once access is given and the
/// user is back, the card reads Safari and shows its kinds, and Import brings them (#1378, #1673).
#[cfg(unix)]
#[gpui::test]
fn an_import_that_needs_full_disk_access_keeps_its_step(cx: &mut TestAppContext) {
    use crate::import::Bringing;
    use crate::import::sources::{Os, Roots};
    use crate::import::tests::{Browser, let_read, safari};
    let Some(home) = safari("onboarding-access") else {
        return;
    };
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Mac))));
    let (shell, cx, view) = importing_from(browser.clone(), cx);
    assert_eq!(brought(&view, cx).state, Bringing::NoAccess);
    assert!(
        read(cx).contains(&(Role::Button, "Open System Settings".into(), None)),
        "the way to System Settings"
    );
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import), "Esc kept the step");
    assert_eq!(brought(&view, cx).state, Bringing::NoAccess, "and the wait");

    let_read(&home);
    cx.update(|_, cx| browser.activate(cx));
    cx.run_until_parked();
    assert!(matches!(brought(&view, cx).state, Bringing::Kinds(_)));
    assert_eq!(step(&shell, cx), Some(Step::Import), "it waits for Import");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.running()).is_some());
    std::fs::remove_dir_all(&home).unwrap();
}

/// Skip, beside the wait for Full Disk Access, goes on without importing.
#[cfg(unix)]
#[gpui::test]
fn skip_goes_on_from_full_disk_access(cx: &mut TestAppContext) {
    use crate::import::Bringing;
    use crate::import::sources::{Os, Roots};
    use crate::import::tests::{Browser, safari};
    let Some(home) = safari("onboarding-access-skip") else {
        return;
    };
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Mac))));
    let (shell, cx, view) = importing_from(browser, cx);
    assert_eq!(brought(&view, cx).state, Bringing::NoAccess);
    click("skip", cx);
    assert_eq!(step(&shell, cx), Some(Step::Space), "Skip went on");
    std::fs::remove_dir_all(&home).unwrap();
}

/// The step reads the browser used last as it comes, and shows a chip a kind it has, each on, for
/// a screen reader too, with Import, Skip and Back. Esc leaves it as it is (v9 §5).
#[gpui::test]
fn the_import_step_reads_the_browser_used_last_and_shows_its_kinds(cx: &mut TestAppContext) {
    use crate::import::Bringing;
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    let bring = brought(&view, cx);
    assert_eq!(bring.picked, Some(0), "the browser used last");
    let Bringing::Kinds(kinds) = bring.state else {
        panic!("its kinds: {:?}", bring.state);
    };
    assert!(
        !kinds.is_empty() && kinds.iter().all(|chip| chip.on && chip.count > 0),
        "{kinds:?}"
    );
    let os = card(&shell, cx, |o| o.os);
    let (title, line) = Step::Import.message(os);
    let said = read(cx);
    assert_eq!(dialogs(&said), [title.to_string()], "read by its title");
    assert_eq!(dialog_line(cx), Some(Some(line.to_string())));
    for name in ["Import", "Skip", "Back"] {
        assert!(
            said.contains(&(Role::Button, name.into(), None)),
            "{name}: {said:?}"
        );
    }
    for chip in &kinds {
        let named = (
            Role::CheckBox,
            chip.kind.label().to_string(),
            Some(Toggled::True),
        );
        assert!(said.contains(&named), "{named:?}: {said:?}");
    }
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import), "Esc kept the step");
    assert_eq!(chips(&view, cx), kinds, "and its kinds");
}

/// A chip clicked leaves its kind out; with none on, Import is dimmed and neither ↩ nor a click
/// does anything.
#[gpui::test]
fn a_chip_leaves_its_kind_out_and_with_none_on_import_does_nothing(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    let kinds = chips(&view, cx);
    click("chip 0", cx);
    assert!(!chips(&view, cx)[0].on, "the click left it out");
    for chip in &kinds[1..] {
        view.update(cx, |view, cx| view.toggle_kind(chip.kind, cx));
    }
    assert!(chips(&view, cx).iter().all(|chip| !chip.on));
    cx.simulate_keystrokes("enter");
    click("main", cx);
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |view, _| view.running()).is_none(),
        "nothing to import"
    );
    assert_eq!(step(&shell, cx), Some(Step::Import));
    click("chip 0", cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.running()).is_some());
}

/// While an import runs neither Back nor Skip shows, and Cancel does until its Spaces are in. The
/// card names each kind as it's written, says All here. once all are, and goes on after it.
#[gpui::test]
fn an_import_draws_each_kind_in_and_goes_on_once_all_are_here(cx: &mut TestAppContext) {
    use crate::import::Bringing;
    use crate::import::tests::Browser;
    let browser = Rc::new(Browser::new(None));
    let (shell, cx, view) = importing_from(browser.clone(), cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(matches!(brought(&view, cx).state, Bringing::Running { .. }));
    assert!(placed("back", cx).is_none() && placed("skip", cx).is_none());
    assert!(placed("cancel", cx).is_some(), "Cancel, while it can stop");
    cx.update(|_, cx| browser.load(cx));
    cx.run_until_parked();
    assert!(placed("cancel", cx).is_none(), "Spaces in, it can't");
    cx.update(|_, cx| browser.added(cx));
    cx.run_until_parked();
    assert_eq!(step(&shell, cx), Some(Step::Import), "the kinds draw in");
    let all_here = card(&shell, cx, |o| o.bring.all_here).expect("All here. is timed");
    let now = cx.executor().now();
    waited(
        cx,
        all_here.saturating_duration_since(now).as_secs_f32() + 0.01,
    );
    let bring = brought(&view, cx);
    let line = shell.read_with(cx, |shell, _| shell.bring_words(Some(&bring)));
    assert_eq!(line.as_deref(), Some("All here."));
    assert_eq!(
        step(&shell, cx),
        Some(Step::Import),
        "it holds on All here."
    );
    waited(cx, t::BRING_ON);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Space),
        "then goes on, no favorites"
    );
}

/// Esc or Cancel stops an import before its Spaces are in, back at the browser's kinds as they
/// were picked, keeping the step; a stalled one too, so a profile that never opens can't trap it.
#[gpui::test]
fn esc_or_cancel_stops_an_import_back_at_its_kinds(cx: &mut TestAppContext) {
    use crate::import::Bringing;
    use crate::import::tests::Browser;
    for how in ["escape", "cancel", "stalled"] {
        let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
        // The last kind left out, the Spaces still on, so the import waits on them.
        let last = chips(&view, cx).last().map(|chip| chip.kind).unwrap();
        view.update(cx, |view, cx| view.toggle_kind(last, cx));
        let picked = chips(&view, cx);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(view.read_with(cx, |view, _| view.running()).is_some());
        match how {
            "cancel" => click("cancel", cx),
            "stalled" => {
                waited(cx, crate::import::STALL.as_secs_f32());
                assert!(placed("cancel", cx).is_some(), "stalled, it can stop");
                cx.simulate_keystrokes("escape");
            }
            key => cx.simulate_keystrokes(key),
        }
        cx.run_until_parked();
        assert_eq!(step(&shell, cx), Some(Step::Import), "{how}: kept the step");
        assert_eq!(
            brought(&view, cx).state,
            Bringing::Kinds(picked),
            "{how}: back at the kinds as picked"
        );
    }
}

/// A profile that hasn't opened stops the knot's lap and says the browser is taking too long where
/// the kinds were, with Try Again, which a click or ↩ presses; Cancel still stops it.
#[gpui::test]
fn a_stalled_import_names_its_profile_on_the_step(cx: &mut TestAppContext) {
    use crate::import::Bringing;
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    waited(cx, crate::import::STALL.as_secs_f32());
    let bring = brought(&view, cx);
    assert!(
        matches!(
            bring.state,
            Bringing::Running {
                stalled: Some(_),
                ..
            }
        ),
        "still running, stalled"
    );
    let name = bring.browsers[bring.picked.unwrap()].0.clone();
    assert_eq!(
        super::bring::bring_held(&bring),
        Some(format!("{name} is taking too long.")),
        "in the failure's place"
    );
    assert!(!asks_for_frames(&shell, cx), "the knot stopped its lap");
    assert!(
        read(cx).contains(&(Role::Button, "Try Again".into(), None)),
        "Try Again, for a screen reader too"
    );
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.guide().title),
        "Here it comes.",
        "↩ tried again"
    );
    waited(cx, crate::import::STALL.as_secs_f32());
    click("try-again", cx);
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.guide().title),
        "Here it comes.",
        "and so did a click"
    );
    assert!(placed("cancel", cx).is_some(), "Cancel still stops it");
    assert_eq!(step(&shell, cx), Some(Step::Import));
}

/// The browsers found are a row, the one used last first; a click on another reads it, and it
/// flies to the slot as the one before goes back.
#[gpui::test]
fn a_browser_clicked_is_read_and_flies_to_the_slot(cx: &mut TestAppContext) {
    use crate::import::Bringing;
    use crate::import::sources::{Os, Roots};
    use crate::import::tests::{Browser, chrome, firefox_dir, places_file, used};
    let home = chrome(
        "onboarding-two",
        &[("Default", "Me")],
        std::time::SystemTime::now(),
    );
    let firefox = home.join(".mozilla/firefox");
    firefox_dir(firefox.clone(), 60 * 60);
    // A places file as Firefox lays one out, so the read finds its kinds, last used an hour ago.
    let places = firefox.join("p.default/places.sqlite");
    drop(places_file(&places));
    used(&places, 60 * 60);
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let (shell, cx, view) = importing_from(browser, cx);
    let bring = brought(&view, cx);
    let names: Vec<&str> = (bring.browsers.iter())
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, ["Chrome", "Firefox"]);
    assert_eq!(bring.picked, Some(0));
    click("browser 1", cx);
    cx.run_until_parked();
    let bring = brought(&view, cx);
    assert_eq!(bring.picked, Some(1), "the click picked Firefox");
    assert!(
        matches!(bring.state, Bringing::Kinds(_)),
        "and read it, not {:?}",
        bring.state
    );
    let flight = card(&shell, cx, |o| {
        o.bring.flight.map(|(from, to, _)| (from, to))
    });
    assert_eq!(
        flight,
        Some((Some(0), 1)),
        "Chrome goes back as Firefox flies"
    );
    std::fs::remove_dir_all(home).unwrap();
}

/// At the import the keyboard rests on Import, and ↩ presses it once: the import runs, and leaves
/// no button for a second ↩.
#[gpui::test]
fn return_at_the_import_presses_its_button_once(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    assert_eq!(keyed(cx), Some((Role::Button, "Import".into())));
    press("enter", cx);
    assert!(
        view.read_with(cx, |view, _| view.running()).is_some(),
        "↩ imports"
    );
    assert!(placed("main", cx).is_none(), "nothing left for ↩ to press");
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
    brought_in(&browser, cx);
    waited(cx, 10.);
    assert_ne!(step(&shell, cx), Some(Step::Import), "all here, it went on");
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

/// Back leaves the import for the step before, and coming back the kinds are as they were picked.
#[gpui::test]
fn back_leaves_the_import_keeping_its_picks(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    click("chip 0", cx);
    let picked = chips(&view, cx);
    click("back", cx);
    assert_eq!(step(&shell, cx), Some(Step::Account), "the step before");
    settle(&shell, cx);
    click("not-now", cx);
    settle(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Import));
    assert_eq!(chips(&view, cx), picked, "Back kept the picks");
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

/// Each row on the card says v9's words for its state, never Settings' own: its maker's plan
/// before a press and once signed in, the wait in the place its sign-in opened, and a failed or
/// closed sign-in as one line, with Sign In again. A missing program is never "Not installed".
#[test]
fn the_assistants_rows_say_the_cards_words_in_each_state() {
    use crate::agents::ready::Ready;
    use crate::agents::sign_in::{Row, shown};
    let card = |cli, ready, plan, row: Row, learned| {
        let said = super::card_words(cli, shown(cli, ready, plan, &row), learned);
        (said.line, said.offer)
    };
    let said = |line: &str, offer| (line.to_string(), offer);
    for (cli, plan) in [
        (Cli::Claude, "Your Claude plan"),
        (Cli::Codex, "Your ChatGPT plan"),
    ] {
        for ready in [Ready::Unknown, Ready::NotFound, Ready::SignedOut] {
            let before = card(cli, ready, None, Row::Idle, false);
            assert_eq!(before, said(plan, Offer::SignIn), "{cli:?} before a press");
        }
        let missing = card(cli, Ready::NotFound, None, Row::Idle, true);
        assert_eq!(missing, said(plan, Offer::Get), "{cli:?}'s program missing");
        let signed_in = card(cli, Ready::SignedIn, Some("Plus"), Row::Idle, true);
        assert_eq!(signed_in, said(plan, Offer::SignedIn), "{cli:?} signed in");
        for row in [Row::Failed(None), Row::Failed(Some("said".into()))] {
            let failed = card(cli, Ready::SignedOut, None, row, true);
            assert_eq!(failed, said("Sign-in didn't finish.", Offer::TryAgain));
        }
    }
    assert_eq!(
        card(Cli::Claude, Ready::SignedOut, None, Row::SigningIn, true),
        said("Finish in your browser.", Offer::Cancel),
        "Claude's sign-in is in the computer's browser"
    );
    assert_eq!(
        card(Cli::Codex, Ready::SignedOut, None, Row::SigningIn, true),
        said("Finish in the new window.", Offer::Cancel),
        "ChatGPT's is in a window here"
    );
}

/// A row on the card is as tall in each of its states (FR19), as each state's line is one line in
/// the card's column: its maker's plan with Sign In, the wait with the ring and Cancel, signed in
/// with the disc and ⋯, and a sign-in that didn't finish.
#[gpui::test]
fn an_assistants_row_is_as_tall_in_each_state(cx: &mut TestAppContext) {
    use crate::agents::ready::Ready;
    use crate::agents::sign_in::{Row, shown};
    let (shell, cx, host) = first_window(cx);
    host.threads.set(Some(true));
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    let states = [
        ("at the start", Ready::SignedOut, None, Row::Idle),
        ("waiting", Ready::SignedOut, None, Row::SigningIn),
        ("signed in", Ready::SignedIn, Some("Plus"), Row::Idle),
        ("failed", Ready::SignedOut, None, Row::Failed(None)),
    ];
    for cli in Cli::ALL {
        let mut heights = Vec::new();
        for (state, ready, plan, row) in &states {
            let rows = vec![(cli, shown(cli, *ready, *plan, row))];
            shell.update(cx, |shell, cx| shell.onboarding_show_assistants(rows, cx));
            settle(&shell, cx);
            let drawn = placed(cli.id(), cx)
                .unwrap_or_else(|| panic!("{cli:?}'s row wasn't drawn {state}"));
            heights.push((*state, f32::from(drawn.size.height)));
        }
        let first = heights[0].1;
        assert!(
            heights
                .iter()
                .all(|&(_, height)| (height - first).abs() < 0.5),
            "{cli:?}'s row changes height between its states: {heights:?}"
        );
    }
}

/// A line too long for the card's column is cut where it's drawn, the row keeping its height, and
/// read whole by an assistive app; each maker's own waiting line is read whole too.
#[gpui::test]
fn a_cut_line_keeps_the_rows_height_and_is_read_whole(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.threads.set(Some(true));
    cx.simulate_a11y_activation();
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    let show = |shell: &Entity<Shell>, cx: &mut VisualTestContext, line: &str, offer| {
        let rows = vec![(
            Cli::Claude,
            Shown {
                line: line.into(),
                offer,
            },
        )];
        shell.update(cx, |shell, cx| shell.onboarding_show_assistants(rows, cx));
        settle(shell, cx);
        placed("claude", cx)
            .expect("Claude's row is drawn")
            .size
            .height
    };
    let rest = show(&shell, cx, "", Offer::SignIn);
    let long = "Finish signing in to your Claude plan in the browser window that opened just now.";
    assert_eq!(
        show(&shell, cx, long, Offer::Cancel),
        rest,
        "the cut row's height"
    );
    let read = |cx: &mut VisualTestContext| {
        let said = read(cx).into_iter();
        let labels = said.filter(|(role, ..)| *role == Role::Label);
        labels.map(|(_, name, _)| name).collect::<Vec<_>>()
    };
    let said = read(cx);
    assert!(
        said.iter().any(|name| name == long),
        "the whole line in {said:?}"
    );

    let rows = Cli::ALL.map(|cli| {
        let waiting = crate::agents::sign_in::Row::SigningIn;
        let ready = crate::agents::ready::Ready::SignedOut;
        (
            cli,
            crate::agents::sign_in::shown(cli, ready, None, &waiting),
        )
    });
    shell.update(cx, |shell, cx| {
        shell.onboarding_show_assistants(rows.to_vec(), cx)
    });
    settle(&shell, cx);
    let said = read(cx);
    for line in [
        crate::agents::sign_in::CLAUDE_WAITING,
        crate::agents::sign_in::CHATGPT_WAITING,
    ] {
        assert!(said.iter().any(|name| name == line), "{line} in {said:?}");
    }
}

/// A signed-in row on the card has the check disc and its ⋯, not Settings' mark, and the foot's
/// right is Next in Just the Browser's place.
#[gpui::test]
fn a_signed_in_row_has_its_disc_and_more_and_no_mark(cx: &mut TestAppContext) {
    use crate::agents::ready::{Found, Ready};
    let (shell, cx, host) = first_window(cx);
    host.threads.set(Some(true));
    let signed_in = Found {
        ready: Ready::SignedIn,
        ..Default::default()
    };
    let (agents, _) =
        cx.update(|_, cx| crate::agents::tests::stand_in(vec![(Cli::Claude, signed_in)], cx));
    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    cx.simulate_a11y_activation();
    started(&shell, cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    settle(&shell, cx);
    assert!(
        placed("just-the-browser", cx).is_some(),
        "nobody is signed in yet"
    );

    shell.update(cx, |shell, cx| shell.onboarding_sign_in(Cli::Claude, cx));
    cx.run_until_parked();
    settle(&shell, cx);
    let found = agents.read_with(cx, |agents, _| agents.found(Cli::Claude));
    assert_eq!(found, Ready::SignedIn, "the press looked for Claude");
    assert_eq!(
        card_lines(&shell, &agents, cx),
        ["Your Claude plan", "Your ChatGPT plan"]
    );
    let said = read(cx);
    let named = |name: &str| said.iter().any(|(_, said, _)| said == name);
    assert!(named("Claude options"), "Claude's ⋯ in {said:?}");
    assert!(
        !said
            .iter()
            .any(|(_, name, _)| name.starts_with("Run threads with")),
        "no mark in {said:?}"
    );
    assert!(placed("just-the-browser", cx).is_none());
    assert!(placed("main", cx).is_some(), "Next");
}

mod exit;

/// The account as the core says it is, its server offering Google or not.
fn synced_google(cx: &mut VisualTestContext, status: Status, google: bool) {
    let state = State {
        status,
        server: SERVER.into(),
        default_server: true,
        google,
        ..State::default()
    };
    cx.update(|_, cx| cx.set_global(account::Synced(state)));
    cx.run_until_parked();
}

/// The Welcome draws Continue with Google under Continue only while the server's discovery offers
/// it (v9 §3): its press opens the sign-in window straight on to Google, ⇥ comes to it between
/// Continue and Not Now, and ↩ on Continue still signs in the usual way.
#[gpui::test]
fn the_welcome_offers_google_only_when_the_server_does(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced_google(cx, Status::SignedOut, false);
    started(&shell, cx);
    assert!(placed("main", cx).is_some());
    assert!(placed("google", cx).is_none(), "no Google from this server");
    shell.update(cx, |shell, _| shell.onboarding = None);

    synced_google(cx, Status::SignedOut, true);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    let (main, google) = (placed("main", cx).unwrap(), placed("google", cx).unwrap());
    assert!(google.top() > main.bottom(), "under Continue");
    assert_eq!(google.size, main.size, "the same size");
    tabbed_to(Role::Button, "Continue", cx);
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, account::GOOGLE.to_string())));
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Not Now".to_string())));

    account_calls(&host);
    click("google", cx);
    assert_eq!(account_calls(&host), ["sign in google"]);
    synced_google(cx, Status::SigningIn, true);
    assert_eq!(welcome(&shell, cx), super::Welcome::SigningIn);
    assert!(
        placed("google", cx).is_none(),
        "Cancel alone while it waits"
    );
}

/// A Google sign-in whose window closes without finishing brings the form back saying so (v9 §3),
/// and Not Now still goes on without an account.
#[gpui::test]
fn a_google_sign_in_that_didnt_finish_brings_the_form_back(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    synced_google(cx, Status::SignedOut, true);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    click("google", cx);
    synced_google(cx, Status::SigningIn, true);
    synced_google(cx, Status::SignedOut, true);
    assert_eq!(welcome(&shell, cx), super::Welcome::Form);
    let unfinished = shell.read_with(cx, |shell, cx| {
        (shell.account_view()).is_some_and(|view| view.read(cx).google_unfinished())
    });
    assert!(unfinished, "the form says Google's sign-in didn't finish");
    assert!(placed("google", cx).is_some(), "it can be tried again");
    account_calls(&host);
    click("not-now", cx);
    assert!(account_calls(&host).is_empty(), "nothing to stop");
    assert_eq!(step(&shell, cx), Some(Step::Import));
}
