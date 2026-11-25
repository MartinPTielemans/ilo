//! A new profile's first launch, on a host that records what the shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Entity, Focusable, Modifiers, TestAppContext, VisualTestContext,
    accesskit::{Role, Toggled},
    point, px,
};

use super::{END_FADE, FirstRun, STEP_MOTION, Step, Switch};
use crate::account::{self, State, Status};
use crate::import::ImportEvent;
use crate::shell::{Folders, Setting, Shell, commands::Command, recorder::Recorder};
use crate::theme::Theme;
use crate::theme_picker::{SWATCHES, swatch_name};
use crate::tokens::shell::onboarding::IMPORT_HOLD;
use crate::transparency::{self, Transparency};

/// A drawn shell showing one Space with no tab shown, as at a new profile's first launch, and first
/// run's window over it, which the context returned drives: its keys and clicks go to that window.
fn first_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    // The transparency step keeps its level where Settings does: here, a folder of the test's own.
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
        // Over no glass, as on Linux and Windows, so the flow's steps are the same on every OS;
        // the transparency step's tests give it the Mac's.
        shell.blurred = false;
    });
    cx.update(|window, cx| window.focus(&shell.focus_handle(cx), cx));
    cx.run_until_parked();
    // Opened here so the test holds its context; starting the flow takes it up.
    let first_run = cx
        .update(|_, cx| FirstRun::open(shell.clone(), cx))
        .expect("first run's window");
    shell.update(cx, |shell, _| shell.first_run = Some(first_run));
    let cx = VisualTestContext::from_window(first_run.into(), cx).into_mut();
    cx.run_until_parked();
    (shell, cx, host)
}

/// A first window over glass, as the Mac's is, where the transparency step shows.
fn glass_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = first_window(cx);
    shell.update(cx, |shell, _| shell.blurred = true);
    (shell, cx, host)
}

/// The level the transparency step shows.
fn level(shell: &Entity<Shell>, cx: &VisualTestContext) -> Option<Transparency> {
    shell.read_with(cx, |shell, _| {
        shell.onboarding.as_ref().map(|onboarding| onboarding.level)
    })
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

/// Goes on with Skip to `to`, as the user skipping each step before it does, from the welcome with
/// Return, as its Skip Setup skips the whole setup.
fn skip_to(shell: &Entity<Shell>, cx: &mut VisualTestContext, to: Step) {
    // A flow that never comes to `to` fails rather than spins.
    for _ in 0..32 {
        let Some(at) = step(shell, cx).filter(|&step| step != to) else {
            break;
        };
        match at {
            Step::Intro => cx.simulate_keystrokes("enter"),
            _ => click_skip(cx),
        }
    }
    assert_eq!(step(shell, cx), Some(to));
}

/// From the step before the keys, left with Skip or Return, the keys' step comes, which has no
/// Skip, and its button ends the flow.
fn in_through_the_keys(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    later(shell, cx, STEP_MOTION);
    assert_eq!(
        step(shell, cx),
        Some(Step::Keys),
        "the keys are the last step"
    );
    assert!(placed("skip", cx).is_none(), "the keys' step has no Skip");
    cx.simulate_keystrokes("enter");
    let ending = shell.read_with(cx, |shell, _| shell.onboarding.as_ref().unwrap().ending);
    assert!(ending.is_some(), "the keys' button ends the flow");
}

/// Moves the step shown, or the flow's end, `seconds` into the past, and draws.
/// Lets `seconds` of the executor's clock pass, as a timer the flow set waits for.
fn waited(cx: &mut VisualTestContext, seconds: f32) {
    cx.executor()
        .advance_clock(Duration::from_secs_f32(seconds));
    cx.run_until_parked();
}

/// Clicks the step's Skip, in the corner or in the sheet's foot.
fn click_skip(cx: &mut VisualTestContext) {
    let skip = placed("skip", cx).expect("Skip is drawn");
    cx.simulate_click(skip.center(), Modifiers::none());
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

/// Whether the flow's last frame asked for another, once what it asked for has drawn.
fn asks_for_frames(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        shell.onboarding.as_ref().is_some_and(|o| o.drawing)
    })
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

/// A launch that brings Spaces back never starts it: the shell has no flow until it's asked for.
#[gpui::test]
fn a_normal_window_has_no_flow(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    assert_eq!(step(&shell, cx), None);
}

/// The welcome waits for the user, however long: another key, Esc, which has no step to go back
/// to, or a click on the window leaves it be, Return or Get Started goes on, and Skip Setup ends
/// the flow there, as its last step does, as do closing the window and its shortcut.
#[gpui::test]
fn the_welcome_waits_then_goes_on_or_skips_the_setup(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    // Half a minute, as long as any step's motion many times over.
    later(&shell, cx, 30.);
    cx.simulate_keystrokes("a escape");
    cx.simulate_click(point(px(600.), px(300.)), Modifiers::none());
    assert_eq!(step(&shell, cx), Some(Step::Intro), "it waits");
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Import));

    for leave in ["skip", "close", "secondary-w"] {
        started(&shell, cx);
        match leave {
            "skip" => click_skip(cx),
            "close" => assert!(!cx.simulate_close(), "the window goes as the flow ends"),
            key => cx.simulate_keystrokes(key),
        }
        let ending = shell.read_with(cx, |shell, _| shell.onboarding.as_ref().unwrap().ending);
        assert!(ending.is_some(), "{leave} ends the flow at the welcome");
        later(&shell, cx, END_FADE);
        assert_eq!(step(&shell, cx), None, "{leave}");
    }
}

/// With the system's Reduce Motion, the welcome is all there once it has faded in, asking for no
/// more frames.
#[gpui::test]
fn reduce_motion_gives_the_still_welcome(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    started(&shell, cx);
    later(&shell, cx, STEP_MOTION);
    let settled = shell.read_with(cx, |shell, _| {
        shell.onboarding.as_ref().is_some_and(|o| o.settled)
    });
    assert!(
        settled,
        "the still welcome is all there once its words fade in"
    );
    assert!(
        !asks_for_frames(&shell, cx),
        "a still welcome draws no more frames"
    );
}

/// Clicks the sheet's Back.
fn click_back(cx: &mut VisualTestContext) {
    let back = placed("back", cx).expect("Back is drawn");
    cx.simulate_click(back.center(), Modifiers::none());
}

/// Back goes to the step before, which comes again; the welcome has none.
#[gpui::test]
fn back_goes_to_the_step_before(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    cx.run_until_parked();
    assert!(placed("back", cx).is_none(), "the welcome has no Back");
    skip_to(&shell, cx, Step::Color);
    later(&shell, cx, STEP_MOTION);
    click_back(cx);
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser));
    later(&shell, cx, STEP_MOTION);
    click_skip(cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Color),
        "and goes on again as before"
    );
}

/// A colour picked shows on the window at once, as the theme picker's does; Continue keeps it, and
/// Skip, Esc or Back take it off again with nothing kept.
#[gpui::test]
fn a_colour_picked_shows_at_once_and_goes_unless_kept(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    let shown = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.theme_of(1).map(|theme| theme.main_color())
        })
    };
    for leave in ["escape", "skip", "back", "enter"] {
        started(&shell, cx);
        skip_to(&shell, cx, Step::Color);
        later(&shell, cx, STEP_MOTION);
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
            "skip" => click_skip(cx),
            "back" => click_back(cx),
            key => cx.simulate_keystrokes(key),
        }
        let kept = host.take().iter().any(|call| call.contains("space color"));
        assert_eq!(kept, leave == "enter", "{leave}");
        if leave != "enter" {
            assert_eq!(shown(&shell, cx), None, "{leave} takes it off");
        }
        shell.update(cx, |shell, _| shell.onboarding = None);
    }
}

/// Skip skips each step in turn to the keys, whose button is the way in, and the flow fades out,
/// with the first Space's welcome under it.
#[gpui::test]
fn skip_goes_on_and_the_last_step_leaves_the_flow(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    cx.simulate_keystrokes("enter");
    for next in [Step::DefaultBrowser, Step::Color] {
        later(&shell, cx, STEP_MOTION);
        click_skip(cx);
        assert_eq!(step(&shell, cx), Some(next));
    }
    later(&shell, cx, STEP_MOTION);
    click_skip(cx);
    later(&shell, cx, STEP_MOTION);
    in_through_the_keys(&shell, cx);
    later(&shell, cx, END_FADE);
    assert_eq!(step(&shell, cx), None);
    assert!(
        shell.read_with(cx, |shell, _| shell.welcome),
        "the welcome is under it"
    );
}

/// Esc is Back, as in the drawing's window: it goes to the step before, keeping nothing the step
/// shows, and at the welcome, which has none, it does nothing.
#[gpui::test]
fn escape_goes_back_a_step(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    later(&shell, cx, STEP_MOTION);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::DefaultBrowser));
    later(&shell, cx, STEP_MOTION);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::Import));
    later(&shell, cx, STEP_MOTION);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::Intro));
    later(&shell, cx, STEP_MOTION);
    cx.simulate_keystrokes("escape");
    assert_eq!(step(&shell, cx), Some(Step::Intro), "the welcome stays");
    let ending = shell.read_with(cx, |shell, _| shell.onboarding.as_ref().unwrap().ending);
    assert!(ending.is_none(), "and the flow with it");
}

/// Return is the step's button, and Skip, at the window's bottom right, goes on too, to the keys,
/// whose Return leaves the flow. At the import Return is its view's button, which
/// shows the browser's choices, and keeps the step.
#[gpui::test]
fn return_and_skip_go_on_from_a_step(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    for (go_on, from, next) in [
        // Skip first: the window keeps its import view, which Return leaves in the choices.
        ("skip", Step::Import, Some(Step::DefaultBrowser)),
        ("return", Step::Import, Some(Step::Import)),
        ("return", Step::DefaultBrowser, Some(Step::Color)),
        ("return", Step::Color, Some(Step::Keys)),
        ("skip", Step::Color, Some(Step::Keys)),
        ("return", Step::Keys, None),
    ] {
        started(&shell, cx);
        skip_to(&shell, cx, from);
        later(&shell, cx, STEP_MOTION);
        match go_on {
            "return" => cx.simulate_keystrokes("enter"),
            _ => {
                click_skip(cx);
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
    for (key, asked) in [("skip", false), ("enter", true)] {
        started(&shell, cx);
        skip_to(&shell, cx, Step::DefaultBrowser);
        later(&shell, cx, STEP_MOTION);
        host.take();
        match key {
            "skip" => click_skip(cx),
            key => cx.simulate_keystrokes(key),
        }
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

/// Not Now, the default browser step's Skip, goes on without asking the system, which leaves
/// Settings' notice that it isn't the default.
#[gpui::test]
fn not_now_goes_on_without_asking(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::DefaultBrowser);
    host.take();
    click_skip(cx);
    assert_ne!(step(&shell, cx), Some(Step::DefaultBrowser));
    assert!(!host.take().contains(&"make default browser".to_string()));
}

/// A browser that's already the default has no step asking to make it one.
#[gpui::test]
fn a_default_browser_leaves_its_step_out(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.default_browser.set(true);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Import);
    click_skip(cx);
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
    click_skip(cx);
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

/// The roles and names a screen reader has of what the tree holds, once one asks.
fn read(cx: &mut VisualTestContext) -> Vec<(Role, String, Option<Toggled>)> {
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let named = tree.nodes.iter().filter_map(|(_, node)| {
        let label = node.label()?.to_string();
        Some((node.role(), label, node.toggled()))
    });
    named.collect()
}

/// A screen reader reads each step as a dialog called by its title, its line said after, and
/// each control on it by its role and name: the button, Skip, and the colours as one choice
/// that says which is picked (#3151). The step sliding out is in the tree as nothing.
#[gpui::test]
fn a_screen_reader_reads_each_steps_title_and_controls(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    let dialogs = |said: &[(Role, String, Option<Toggled>)]| {
        let dialogs = said.iter().filter(|(role, ..)| *role == Role::Dialog);
        dialogs.map(|(_, name, _)| name.clone()).collect::<Vec<_>>()
    };
    let welcome = concat!("Welcome to ", env!("APP_NAME"), ".");
    assert_eq!(dialogs(&read(cx)), [welcome], "the welcome, by its title");

    skip_to(&shell, cx, Step::Color);
    let said = read(cx);
    assert_eq!(
        dialogs(&said),
        ["Pick a color for your first Space."],
        "the step it went on to only, as the one before slides out"
    );
    let line = cx
        .a11y_tree()
        .unwrap()
        .nodes
        .into_iter()
        .find_map(|(_, node)| {
            (node.role() == Role::Dialog).then(|| node.description().map(str::to_string))
        });
    assert!(
        line.flatten()
            .is_some_and(|line| line.starts_with("The window takes it.")),
        "its line is said after its title"
    );
    for button in ["Continue", "Skip"] {
        assert!(
            said.contains(&(Role::Button, button.into(), None)),
            "no button {button}: {said:?}"
        );
    }
    assert!(
        said.iter()
            .any(|(role, name, _)| *role == Role::RadioGroup && name == "Color")
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
}

/// The flow ends in the first Space with the command bar up, as ⌘T has it, and a line on the card
/// under it until a page shows.
#[gpui::test]
fn it_ends_with_the_command_bar_up(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    click_skip(cx);
    in_through_the_keys(&shell, cx);
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
    click_skip(cx);
    in_through_the_keys(&shell, cx);
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
    click_skip(cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Keys),
        "with neither offered, the keys come after the colour"
    );
    in_through_the_keys(&shell, cx);
    later(&shell, cx, END_FADE);

    // With no count's server named, only the crash reports' switch shows. Outside apps' agents
    // have none until their own setting (#4260), with agents on or off.
    host.crash_reports.set(Some(false));
    host.agents.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    assert_eq!(offered(&shell, cx), vec![(Switch::CrashReports, false)]);
    host.take();
    shell.update(cx, |shell, cx| {
        shell.onboarding_switch(Switch::CrashReports, cx)
    });
    cx.run_until_parked();
    assert_eq!(host.take(), vec!["crash reports true".to_string()]);
    assert_eq!(offered(&shell, cx), vec![(Switch::CrashReports, true)]);
    click_skip(cx);
    in_through_the_keys(&shell, cx);
    assert_eq!(
        host.crash_reports.get(),
        Some(true),
        "Skip leaves a switch as it was turned"
    );
    assert_eq!(
        host.agents.get(),
        Some(false),
        "and the agents' as they were"
    );
    later(&shell, cx, END_FADE);

    host.crash_reports.set(Some(false));
    host.daily_count.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    let both = vec![(Switch::DailyCount, false), (Switch::CrashReports, false)];
    assert_eq!(offered(&shell, cx), both, "both off to begin with");
}

/// With both opt-ins offered, in first run's fixed window, each switch lies inside its row, and a
/// click at its drawn centre turns it on; its sentence once pushed it out of the row, where the
/// click missed (#1286).
#[gpui::test]
fn a_click_on_each_opt_in_switch_turns_it_on(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.crash_reports.set(Some(false));
    host.daily_count.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    later(&shell, cx, STEP_MOTION);
    let frame = |shell: &Entity<Shell>, cx: &VisualTestContext, opt_in, part| {
        shell.read_with(cx, |shell, _| {
            let onboarding = shell.onboarding.as_ref().expect("the flow shows");
            onboarding.frames.borrow().get(&(opt_in, part)).copied()
        })
    };
    // The count's is set only as the step is left (the_count_is_a_switch_set_as_the_step_is_left).
    for (opt_in, set) in [
        (Switch::DailyCount, None),
        (Switch::CrashReports, Some("crash reports true")),
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
        cx.run_until_parked();
        let said: Vec<String> = set.into_iter().map(str::to_string).collect();
        assert_eq!(host.take(), said, "a click on {opt_in:?}'s switch");
    }
    let both = vec![(Switch::DailyCount, true), (Switch::CrashReports, true)];
    assert_eq!(shell.update(cx, |shell, cx| shell.opt_ins(cx)), both);
    // The switches line up, in one column at the box's right edge.
    let right = |opt_in| frame(&shell, cx, opt_in, "switch").map(|switch| switch.right());
    assert_eq!(right(Switch::DailyCount), right(Switch::CrashReports));
}

/// The daily usage count is the opt-ins' first switch while the build names a server, off, lined
/// up with the others. Turned on, nothing is set while the step shows; Continue sets it, which
/// sends the first count (#2978).
#[gpui::test]
fn the_count_is_a_switch_set_as_the_step_is_left(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.crash_reports.set(Some(false));
    host.daily_count.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    later(&shell, cx, STEP_MOTION);
    assert_eq!(
        offered(&shell, cx),
        vec![(Switch::DailyCount, false), (Switch::CrashReports, false)]
    );
    let row = opt_in_frame(&shell, cx, Switch::DailyCount, "row").expect("its row is drawn");
    let switch = opt_in_frame(&shell, cx, Switch::DailyCount, "switch").expect("its switch");
    assert!(
        switch.left() >= row.left() && switch.right() <= row.right(),
        "the count's switch at {switch:?} is outside its row at {row:?}"
    );
    let crash = opt_in_frame(&shell, cx, Switch::CrashReports, "switch").expect("crash reports'");
    assert_eq!(switch.right(), crash.right(), "the switches line up");
    host.take();
    cx.simulate_click(switch.center(), Modifiers::none());
    assert_eq!(offered(&shell, cx)[0], (Switch::DailyCount, true));
    assert!(
        host.take().is_empty(),
        "turned on, nothing is set while the step shows"
    );
    cx.simulate_keystrokes("enter");
    assert_eq!(host.take(), vec!["daily count true".to_string()]);
    assert_eq!(host.daily_count.get(), Some(true));
}

/// Turned on and off again before the step is left, the count is as it was: nothing is set.
#[gpui::test]
fn the_count_turned_on_and_off_again_sets_nothing(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.daily_count.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    for _ in 0..2 {
        shell.update(cx, |shell, cx| {
            shell.onboarding_switch(Switch::DailyCount, cx)
        });
    }
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Keys), "Continue left the step");
    assert!(
        host.take().is_empty(),
        "turned on and off again, the count was set"
    );
    assert_eq!(host.daily_count.get(), Some(false));
}

/// Skip leaves the step, so it leaves the count as it stands when pressed: off untouched, and on
/// once turned on, which sends the first count as Continue does.
#[gpui::test]
fn skip_leaves_the_count_as_it_stands(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    for (leave, turned, set) in [
        ("skip", false, None),
        ("skip", true, Some("daily count true")),
    ] {
        host.daily_count.set(Some(false));
        started(&shell, cx);
        skip_to(&shell, cx, Step::OptIns);
        later(&shell, cx, STEP_MOTION);
        if turned {
            shell.update(cx, |shell, cx| {
                shell.onboarding_switch(Switch::DailyCount, cx)
            });
        }
        host.take();
        click_skip(cx);
        assert_eq!(step(&shell, cx), Some(Step::Keys), "{leave} left the step");
        let said: Vec<String> = set.into_iter().map(str::to_string).collect();
        assert_eq!(
            host.take(),
            said,
            "{leave} with the count turned on {turned}"
        );
        assert_eq!(host.daily_count.get(), Some(turned));
        later(&shell, cx, END_FADE);
    }
}

/// The step at its switches, the count's and crash reports', both off, the motion done.
fn at_the_switches(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = first_window(cx);
    host.daily_count.set(Some(false));
    host.crash_reports.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    later(&shell, cx, STEP_MOTION);
    (shell, cx, host)
}

/// ⇥ goes along the switches in order and round again, ⇧⇥ back, and Space turns the one
/// the keyboard is on, as a click on its row does; an assistive app reads each as its switch
/// (#2985).
#[gpui::test]
fn tab_and_space_turn_each_opt_in_switch(cx: &mut TestAppContext) {
    use crate::settings_rows::spoken::{press, stops};
    let (shell, cx, host) = at_the_switches(cx);
    // Round from the window, as the step's Continue has the keyboard as it comes.
    cx.update(|window, cx| window.blur(cx));
    let names: Vec<String> = stops(cx).into_iter().map(|(_, name, _)| name).collect();
    assert_eq!(
        names,
        [
            "Send a daily usage count",
            "Send crash reports",
            "Back",
            "Skip",
            "Continue"
        ],
        "the stops ⇥ goes along, the step's buttons too, as a screen reader's user finds them (#3151)"
    );
    cx.update(|window, cx| window.blur(cx));
    host.take();
    for (at, said) in [(0, None), (1, Some("crash reports true"))] {
        cx.simulate_keystrokes("tab");
        press("space", cx);
        assert!(offered(&shell, cx)[at].1, "Space turned switch {at} on");
        let said: Vec<String> = said.into_iter().map(str::to_string).collect();
        assert_eq!(host.take(), said, "Space on switch {at}");
    }
    // Round past the sheet's foot, Back, Skip and Continue, to the first, and back to the last
    // switch.
    cx.simulate_keystrokes("tab tab tab tab");
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab shift-tab");
    press("space", cx);
    assert_eq!(
        offered(&shell, cx),
        vec![(Switch::DailyCount, true), (Switch::CrashReports, false)],
        "⇧⇥ from the first went back past the buttons to the last switch"
    );
}

/// The opt-ins rest on Continue, the last of the sheet's foot, and the switches are a ⇥ from it:
/// ⇥ goes round to the count's, the first, and ⇧⇥ back past Skip and Back to crash reports', the
/// last (#3498).
#[gpui::test]
fn the_switches_are_a_tab_from_continue(cx: &mut TestAppContext) {
    for (keys, at) in [
        ("tab", "Send a daily usage count"),
        ("shift-tab", "Skip"),
        ("shift-tab shift-tab shift-tab", "Send crash reports"),
    ] {
        let (_, cx, _) = at_the_switches(cx);
        cx.simulate_a11y_activation();
        assert_eq!(keyed(cx), Some((Role::Button, "Continue".into())));
        cx.simulate_keystrokes(keys);
        let role = if at == "Skip" {
            Role::Button
        } else {
            Role::Switch
        };
        assert_eq!(keyed(cx), Some((role, at.into())), "{keys} from Continue");
    }
}

/// With a switch the keyboard is on, ↩ is still Continue and Esc still Back: neither turns it. ↩
/// sets the count as the step is left, as it stands (#2978); Esc goes back and sets nothing.
#[gpui::test]
fn return_and_escape_leave_from_a_focused_switch(cx: &mut TestAppContext) {
    use crate::settings_rows::spoken::press;
    for (leave, turned) in [
        ("enter", true),
        ("escape", true),
        ("enter", false),
        ("escape", false),
    ] {
        let (shell, cx, host) = at_the_switches(cx);
        // From Continue, where the step rests, round to the count's switch.
        cx.simulate_keystrokes("tab");
        if turned {
            press("space", cx);
        }
        host.take();
        press(leave, cx);
        if leave == "escape" {
            assert_ne!(step(&shell, cx), Some(Step::OptIns), "Esc went back");
            assert_eq!(host.take(), Vec::<String>::new(), "turned on {turned}");
            assert_eq!(host.daily_count.get(), Some(false), "Esc set nothing");
            continue;
        }
        assert_eq!(step(&shell, cx), Some(Step::Keys), "{leave} left the step");
        let said: Vec<String> = turned
            .then(|| "daily count true".to_string())
            .into_iter()
            .collect();
        assert_eq!(
            host.take(),
            said,
            "{leave} with the count turned on {turned}"
        );
        assert_eq!(
            host.daily_count.get(),
            Some(turned),
            "{leave} turned the switch"
        );
    }
}

/// A build that names no server for the count doesn't offer it.
#[gpui::test]
fn a_build_without_a_count_server_doesnt_offer_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.crash_reports.set(Some(false));
    started(&shell, cx);
    skip_to(&shell, cx, Step::OptIns);
    assert_eq!(offered(&shell, cx), vec![(Switch::CrashReports, false)]);
}

/// A day, in the seconds Settings' times are in.
const DAY: f64 = 24. * 3600.;

/// A first window whose host has the tidy step's time, `after`, and Settings' `settings` as given.
fn with_settings<'a>(
    cx: &'a mut TestAppContext,
    after: Option<f64>,
    settings: &[(Setting, bool)],
) -> (Entity<Shell>, &'a mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = first_window(cx);
    host.archive_after.set(after);
    host.settings.borrow_mut().extend(settings.iter().copied());
    (shell, cx, host)
}

/// The tidy and private steps come after the Space's look, in the drawing's order, while the host
/// has their settings, and are left out, the count going down, while it has none.
#[gpui::test]
fn the_tidy_and_private_steps_come_where_their_settings_are(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    let none = steps(&shell, cx);
    assert!(!none.contains(&Step::Tidy) && !none.contains(&Step::Private));

    let (shell, cx, _) = with_settings(
        cx,
        Some(DAY / 2.),
        &[(Setting::RestoreWindows, true), (Setting::AdBlocking, true)],
    );
    started(&shell, cx);
    let all = steps(&shell, cx);
    let at = |step| all.iter().position(|&each| each == step);
    assert!(at(Step::Color) < at(Step::Tidy), "{all:?}");
    assert!(at(Step::Tidy) < at(Step::Private), "{all:?}");
    assert!(at(Step::Private) < at(Step::Keys), "{all:?}");
    assert_eq!(all.len(), none.len() + 2);
}

/// The tidy step's bar starts at the profile's own time and the arrows move it a stop, setting
/// nothing; Continue sets the time it's at, once, and only when it moved. Skip and Back set nothing.
#[gpui::test]
fn the_tidy_bar_sets_its_time_as_continue_goes_on(cx: &mut TestAppContext) {
    let at = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let tidy = shell.onboarding.as_ref()?.tidy.as_ref()?;
            Some(tidy.stops[tidy.at].1)
        })
    };
    for (keys, leave, set) in [
        ("right", "enter", Some("archive after 1 86400")),
        ("right right left", "enter", Some("archive after 1 86400")),
        ("", "enter", None),
        ("right", "skip", None),
        ("left", "enter", None),
    ] {
        let (shell, cx, host) = with_settings(cx, Some(DAY / 2.), &[]);
        started(&shell, cx);
        skip_to(&shell, cx, Step::Tidy);
        later(&shell, cx, STEP_MOTION);
        assert_eq!(at(&shell, cx), Some("12 hours"), "the profile's time");
        host.take();
        if !keys.is_empty() {
            cx.simulate_keystrokes(keys);
        }
        assert!(
            host.take().is_empty(),
            "{keys} set something before Continue"
        );
        match leave {
            "skip" => click_skip(cx),
            _ => cx.simulate_keystrokes(leave),
        }
        assert_ne!(step(&shell, cx), Some(Step::Tidy), "{leave} left the step");
        let said: Vec<String> = set.into_iter().map(str::to_string).collect();
        assert_eq!(host.take(), said, "{keys} then {leave}");
    }
}

/// A click on the bar puts it at the stop nearest, and a drag along it follows the pointer.
#[gpui::test]
fn a_click_on_the_tidy_bar_picks_the_stop_nearest(cx: &mut TestAppContext) {
    let (shell, cx, _) = with_settings(cx, Some(DAY / 2.), &[]);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Tidy);
    later(&shell, cx, STEP_MOTION);
    let bar = placed("tidy", cx).expect("the bar is drawn");
    let at = |shell: &Entity<Shell>, cx: &VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.onboarding.as_ref()?.tidy.as_ref().map(|t| t.at)
        })
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

/// Settings' times are named in the drawing's words, and a time between them is shown at the
/// stop nearest, Never past every one.
#[test]
fn the_tidy_stops_are_settings_times_in_the_drawings_words() {
    let offered = [DAY / 2., DAY, 7. * DAY, 30. * DAY, -1.];
    let tidy = super::Tidy::of(DAY / 2., &offered).expect("a bar");
    let words: Vec<&str> = tidy.stops.iter().map(|&(_, words)| words).collect();
    assert_eq!(words, ["12 hours", "1 day", "1 week", "1 month", "Never"]);
    assert_eq!(tidy.at, 0);
    let at = |was| super::Tidy::of(was, &offered).map(|tidy| tidy.at);
    assert_eq!(at(-1.), Some(4));
    assert_eq!(at(2. * DAY), Some(1));
    assert_eq!(at(365. * DAY), Some(3));
    assert_eq!(
        super::Tidy::of(DAY, &[3600.]),
        None,
        "a time with no word has no stop"
    );
}

/// The private step shows Settings' switches as they are, each turned at once, for the Space's
/// profile, and Skip leaves them as they stand.
#[gpui::test]
fn the_private_step_shows_settings_switches_as_they_are(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_settings(
        cx,
        None,
        &[(Setting::AdBlocking, true), (Setting::SavePasswords, true)],
    );
    started(&shell, cx);
    skip_to(&shell, cx, Step::Private);
    later(&shell, cx, STEP_MOTION);
    let shown = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.switches(Step::Private, cx))
    };
    let ad = Switch::Setting(Setting::AdBlocking);
    let passwords = Switch::Setting(Setting::SavePasswords);
    assert_eq!(shown(&shell, cx), vec![(ad, true), (passwords, true)]);
    host.take();
    let switch = opt_in_frame(&shell, cx, ad, "switch").expect("its switch is drawn");
    cx.simulate_click(switch.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), vec!["setting 1 AdBlocking false".to_string()]);
    assert_eq!(shown(&shell, cx), vec![(ad, false), (passwords, true)]);
    click_skip(cx);
    assert!(host.take().is_empty(), "Skip set nothing");
    assert_eq!(
        host.settings.borrow().get(&Setting::AdBlocking),
        Some(&false)
    );
}

/// Skip Setup from the welcome leaves every one of the steps' settings as it was.
#[gpui::test]
fn skip_setup_leaves_the_tidy_and_private_settings(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_settings(
        cx,
        Some(DAY / 2.),
        &[(Setting::RestoreWindows, true), (Setting::AdBlocking, true)],
    );
    started(&shell, cx);
    host.take();
    shell.update(cx, |shell, cx| shell.onboarding_skip_setup("skip", cx));
    later(&shell, cx, END_FADE);
    let set: Vec<String> = (host.take().into_iter())
        .filter(|call| call.starts_with("setting") || call.starts_with("archive"))
        .collect();
    assert!(set.is_empty(), "{set:?}");
}

/// The opt-ins the step offers, each with whether it's on.
fn offered(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<(Switch, bool)> {
    shell.update(cx, |shell, cx| shell.opt_ins(cx))
}

/// Where `opt_in`'s `part`, "row" or "switch", is drawn.
fn opt_in_frame(
    shell: &Entity<Shell>,
    cx: &VisualTestContext,
    opt_in: Switch,
    part: &'static str,
) -> Option<gpui::Bounds<gpui::Pixels>> {
    shell.read_with(cx, |shell, _| {
        let onboarding = shell.onboarding.as_ref().expect("the flow shows");
        onboarding.frames.borrow().get(&(opt_in, part)).copied()
    })
}

/// Help › Welcome to ilo plays it again in first run's window, whether the window shows a page or
/// not, as the flow no longer draws over the browser's.
#[gpui::test]
fn welcome_to_ilo_plays_it_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    for selected in [None, Some(1)] {
        if let Some(tab) = selected {
            shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
            cx.run_until_parked();
        }
        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::Welcome, "menu", cx));
        cx.run_until_parked();
        assert_eq!(step(&shell, cx), Some(Step::Intro), "{selected:?}");
        assert!(
            host.take().contains(&"first run shown".to_string()),
            "{selected:?}: in its window"
        );
        click_skip(cx);
        later(&shell, cx, END_FADE);
        assert_eq!(step(&shell, cx), None);
        assert!(host.take().contains(&"first run ended".to_string()));
    }
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
    click_skip(cx);
    in_through_the_keys(&shell, cx);
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
    cx.simulate_keystrokes("enter");
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
    click_skip(cx);
    assert_eq!(step(&shell, cx), Some(Step::OptIns));
}

/// Skip goes on from the account step without signing in.
#[gpui::test]
fn skip_leaves_the_account_step_signed_out(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
    host.crash_reports.set(Some(false));
    synced(cx, Status::SignedOut, "https://sync.example.org");
    started(&shell, cx);
    skip_to(&shell, cx, Step::Account);
    later(&shell, cx, STEP_MOTION);
    click_skip(cx);
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
    assert_eq!(step(&shell, cx), Some(Step::Keys), "signed in, it went on");
}

/// The account's first device shows its recovery words, which show once, so neither Esc nor Skip
/// goes on from them; their Done does.
#[gpui::test]
fn the_first_devices_words_go_on_with_done(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.sync_server.set(true);
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
    // Taller than the sheet's room, the words scroll in it above the foot, Done at their end.
    let done = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| view.drawn("done"))
            .expect("Done is drawn")
    };
    let foot = placed("back", cx).expect("Back is drawn");
    let over_the_words = point(done(cx).center().x, foot.top() - px(40.));
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: over_the_words,
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-400.))),
        ..Default::default()
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    let done = done(cx);
    assert!(
        done.bottom() <= foot.top(),
        "scrolled to their end, Done at {done:?} is above the foot at {foot:?}"
    );
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
/// the list "Bring your stuff." with Import from the browser used last and Skip; a browser's
/// choices, which a wait for access ends on, "What comes from Arc.", with Import and Back, and its
/// view has no buttons of its own; Esc goes back to the list, keeping the step, and Esc there goes
/// back a step.
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
        format!(
            "Spaces, pinned tabs, folders and favorites from the browser you used. It all \
             stays on {here}."
        )
    );
    assert_eq!(list.button.as_deref(), Some("Import from Arc"));
    assert_eq!(list.corner, Corner::Skip);
    view.update(cx, |view, cx| view.pick(0, cx));
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
    assert_eq!(
        step(&shell, cx),
        Some(Step::Intro),
        "from the list, to the step before"
    );
}

/// ↩ at the list imports the browser picked at once, as the drawing's Import from … does: once
/// read, the flow goes on by itself while the rest comes in.
#[gpui::test]
fn return_brings_the_browser_picked_and_imports_it(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
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

/// A profile that hasn't opened keeps the step, named in its title, with Try Again as its button,
/// which ↩ presses; Skip, in its corner, skips the step, and the import stops waiting.
#[gpui::test]
fn a_stalled_import_names_its_profile_on_the_step(cx: &mut TestAppContext) {
    use crate::import::Corner;
    use crate::import::tests::Browser;
    let browser = Rc::new(Browser::new(None));
    let (shell, cx, view) = importing_from(browser, cx);
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
    click_skip(cx);
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
    std::fs::remove_dir_all(home).unwrap();
}

/// The wait for Full Disk Access says Back in its corner, which Esc does, back to the list; Esc
/// there goes back a step, as from any step.
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
    assert_eq!(
        step(&shell, cx),
        Some(Step::Intro),
        "from the list, to the step before"
    );
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
    let (shell, cx, _) = importing_from(browser.clone(), cx);
    let weak = shell.downgrade();
    browser.reads_shell(move |cx| {
        if let Some(shell) = weak.upgrade() {
            shell.read(cx);
        }
    });
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

/// What has the keyboard, as a screen reader reads it: its role and name. Inside a stop with an
/// active one, as the colours have, that one.
fn keyed(cx: &mut VisualTestContext) -> Option<(Role, String)> {
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let (_, node) = tree.nodes.iter().find(|(id, _)| *id == tree.focus)?;
    Some((node.role(), node.label().unwrap_or_default().to_string()))
}

/// A key pressed and let go, as a keyboard sends it: Space and Return click on the release.
fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    let keystroke = gpui::Keystroke::parse(key).unwrap();
    cx.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke }),
            cx,
        );
    });
    cx.run_until_parked();
}

/// The keyboard comes with each step, so a screen reader, which follows it, reads the step by its
/// title on the way in. ⇥ goes along the step's stops, each read by its name, the colours' saying
/// the one picked as the arrows move, and ↩ on Skip is Skip's press alone (#3151).
#[gpui::test]
fn a_screen_reader_follows_the_keyboard_through_each_step(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    assert_eq!(
        keyed(cx),
        Some((Role::Button, "Get Started".into())),
        "the welcome rests on its button, as a step with no choice does"
    );

    skip_to(&shell, cx, Step::DefaultBrowser);
    // On the sheet, ⇥ from the step's button goes round to Back, then Skip, which this step,
    // asking, calls Not Now.
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Back".into())));
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Not Now".into())));
    press("enter", cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Color),
        "↩ on Not Now skipped one step"
    );
    assert_eq!(
        keyed(cx),
        Some((Role::RadioGroup, "Color".into())),
        "the step it went on to has the keyboard"
    );
    cx.simulate_keystrokes("right");
    let first = swatch_name(SWATCHES[0]).to_string();
    assert_eq!(
        keyed(cx),
        Some((Role::RadioButton, first)),
        "→ picks the first, which it reads"
    );
    // The sheet's foot, in its reading order: Back, Skip, then the step's button.
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Back".into())));
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Skip".into())));
    cx.simulate_keystrokes("tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Continue".into())));
    cx.simulate_keystrokes("shift-tab");
    assert_eq!(keyed(cx), Some((Role::Button, "Skip".into())));
    host.take();
    press("enter", cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Keys),
        "↩ on Skip left the step"
    );
    let colored = host
        .take()
        .iter()
        .any(|call| call.starts_with("space color 1 "));
    assert!(!colored, "as Skip, keeping the Space's colour");
}

/// Each step's keyboard rests on a control as it comes, never on the step's dialog, whose title
/// and line a screen reader says on the way in. Orca drops the arrival of a step left by a key
/// while its place is a dialog (#3498). The colours and engines have it, else the step's button,
/// the opt-ins' too, never a switch. ↩ there does what it does anywhere in the step: it's the
/// step's button, pressed once on its way down and not again on its way up.
#[gpui::test]
fn each_step_rests_on_a_control(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    with_engines(&host);
    host.sync_server.set(true);
    host.daily_count.set(Some(false));
    host.crash_reports.set(Some(false));
    synced(cx, Status::SignedOut, "https://sync.example.org");
    cx.simulate_a11y_activation();
    let button = |name: &str| Some((Role::Button, name.to_string()));
    let signing = |host: &Recorder| {
        let calls = account_calls(host);
        calls.iter().filter(|call| *call == "sign in").count()
    };
    started(&shell, cx);
    skip_to(&shell, cx, Step::DefaultBrowser);
    assert_eq!(keyed(cx), button("Make Default"));
    host.take();
    press("enter", cx);
    assert_eq!(step(&shell, cx), Some(Step::SearchEngine), "↩ went on");
    let asked = host.take();
    let asked = asked.iter().filter(|call| *call == "make default browser");
    assert_eq!(asked.count(), 1, "↩ on Make Default asked the system once");

    assert_eq!(
        keyed(cx),
        Some((Role::RadioButton, "DuckDuckGo".into())),
        "the engines, read by the one picked"
    );
    click_skip(cx);
    assert_eq!(step(&shell, cx), Some(Step::Color));
    assert_eq!(keyed(cx), Some((Role::RadioGroup, "Color".into())));
    press("enter", cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Account),
        "↩ on the colours went on"
    );

    assert_eq!(keyed(cx), button("Sign In"));
    signing(&host);
    click_skip(cx);
    assert_eq!(step(&shell, cx), Some(Step::OptIns));
    assert_eq!(signing(&host), 0, "Skip didn't sign in");
    // Not on a switch, each a yes to sending something, which a stray Space would give.
    assert_eq!(keyed(cx), button("Continue"));
    host.take();
    press("space", cx);
    assert_eq!(
        step(&shell, cx),
        Some(Step::Keys),
        "Space on Continue pressed it"
    );
    assert_eq!(host.daily_count.get(), Some(false), "and turned no switch");
    assert_eq!(
        host.crash_reports.get(),
        Some(false),
        "and turned no switch"
    );

    started(&shell, cx);
    skip_to(&shell, cx, Step::Account);
    signing(&host);
    press("enter", cx);
    assert_eq!(signing(&host), 1, "↩ on Sign In signed in once");
}

/// At the import the keyboard rests on its view's button, and ↩ presses it once: the list's
/// press brings the browser's choices, and only the next imports it.
#[gpui::test]
fn return_at_the_import_presses_its_button_once(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let browser = Rc::new(Browser::new(None));
    let (shell, cx, view) = importing_from(browser, cx);
    cx.simulate_a11y_activation();
    let guide = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.guide());
    let words = guide(cx).button.expect("the list's button");
    assert_eq!(keyed(cx), Some((Role::Button, words)));
    press("enter", cx);
    let choices = guide(cx).title;
    assert_ne!(choices, "Here it comes.", "↩ pressed it twice");
    press("enter", cx);
    assert_eq!(
        guide(cx).title,
        "Here it comes.",
        "and its choices' imports"
    );
    assert_eq!(step(&shell, cx), Some(Step::Import));
}

/// A host offering three engines, DuckDuckGo the default and Google last, as the core lists them.
fn with_engines(host: &Recorder) {
    let engines = [
        ("duckduckgo.com", "DuckDuckGo"),
        ("bing.com", "Bing"),
        ("google.com", "Google"),
    ];
    *host.search_engines.borrow_mut() = (
        engines
            .iter()
            .map(|&(keyword, name)| (keyword.into(), name.into()))
            .collect(),
        "duckduckgo.com".into(),
    );
}

/// The engine picked at the search engine step, if one is.
fn engine(shell: &Entity<Shell>, cx: &VisualTestContext) -> Option<usize> {
    shell.read_with(cx, |shell, _| {
        shell.onboarding.as_ref().and_then(|o| o.engine)
    })
}

/// The step comes after Make Default with the default picked, and the arrows move the pick. Nothing
/// is set until Continue: Skip after a moved pick leaves the profile's engine as it was, Continue
/// on the default sets none either, as it stays the core's fallback, and Continue on another
/// sets that one.
#[gpui::test]
fn the_search_engine_step_sets_nothing_until_continue(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    with_engines(&host);
    let set = |calls: Vec<String>| {
        (calls.into_iter())
            .filter(|call| call.starts_with("search engine "))
            .collect::<Vec<_>>()
    };
    started(&shell, cx);
    skip_to(&shell, cx, Step::DefaultBrowser);
    click_skip(cx);
    assert_eq!(step(&shell, cx), Some(Step::SearchEngine));
    assert_eq!(engine(&shell, cx), Some(0), "the default is picked");
    cx.simulate_keystrokes("down down down");
    assert_eq!(engine(&shell, cx), Some(2), "↓ stops at the last");
    cx.simulate_keystrokes("up");
    assert_eq!(engine(&shell, cx), Some(1));
    host.take();
    click_skip(cx);
    assert_eq!(step(&shell, cx), Some(Step::Color));
    assert_eq!(set(host.take()), Vec::<String>::new(), "Skip sets nothing");

    started(&shell, cx);
    skip_to(&shell, cx, Step::SearchEngine);
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Color));
    assert_eq!(
        set(host.take()),
        Vec::<String>::new(),
        "the default stays unset"
    );

    started(&shell, cx);
    skip_to(&shell, cx, Step::SearchEngine);
    cx.simulate_keystrokes("down down");
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(set(host.take()), ["search engine 1 google.com"]);
}

/// With no engines to offer, as while a policy picks the default, the step is left out.
#[gpui::test]
fn no_engines_leaves_the_search_engine_step_out(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::DefaultBrowser);
    click_skip(cx);
    assert_eq!(step(&shell, cx), Some(Step::Color));
}

/// A new profile's engines load after the flow starts: the step is planned, and its list is the
/// one the core has when the step comes.
#[gpui::test]
fn engines_loaded_after_the_start_come_with_their_step(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    started(&shell, cx);
    assert!(steps(&shell, cx).contains(&Step::SearchEngine));
    with_engines(&host);
    skip_to(&shell, cx, Step::SearchEngine);
    assert_eq!(step(&shell, cx), Some(Step::SearchEngine));
    assert_eq!(engine(&shell, cx), Some(0), "the default is picked");
}

/// A screen reader has the engines as one choice called Search engine, a radio button each by its
/// name, the one picked checked.
#[gpui::test]
fn a_screen_reader_reads_the_engines_as_one_choice(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    with_engines(&host);
    cx.simulate_a11y_activation();
    started(&shell, cx);
    skip_to(&shell, cx, Step::SearchEngine);
    let said = read(cx);
    assert!(
        said.contains(&(Role::Dialog, "Pick a search engine.".into(), None)),
        "{said:?}"
    );
    let line = cx
        .a11y_tree()
        .unwrap()
        .nodes
        .into_iter()
        .find_map(|(_, node)| {
            (node.role() == Role::Dialog).then(|| node.description().map(str::to_string))
        });
    assert_eq!(
        line,
        Some(Some("You can change it later in Settings.".to_string())),
        "the step's title is read with its line (P-search)"
    );
    assert!(
        said.iter()
            .any(|(role, name, _)| *role == Role::RadioGroup && name == "Search engine")
    );
    let engines: Vec<_> = (said.iter())
        .filter(|(role, ..)| *role == Role::RadioButton)
        .map(|(_, name, on)| (name.as_str(), *on))
        .collect();
    assert_eq!(
        engines,
        [
            ("DuckDuckGo", Some(Toggled::True)),
            ("Bing", Some(Toggled::False)),
            ("Google", Some(Toggled::False)),
        ]
    );
}

/// Over glass the transparency step comes right after the colour; over none it's left out, as
/// there's nothing behind the window to show through.
#[gpui::test]
fn the_transparency_step_follows_the_colour_over_glass(cx: &mut TestAppContext) {
    let (shell, cx, _) = glass_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Color);
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Transparency));
}

#[gpui::test]
fn no_glass_leaves_the_transparency_step_out(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    let steps = shell.read_with(cx, |shell, _| {
        shell.onboarding.as_ref().unwrap().steps.clone()
    });
    assert!(!steps.contains(&Step::Transparency), "{steps:?}");
}

/// The preview opens the flow at a step by its name or its number, and at the transparency step
/// without glass too, so the step can be looked at on any host.
#[gpui::test]
fn the_preview_opens_the_flow_at_a_step_by_name_or_number(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    let at = |name: &str, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.onboarding = None;
            shell.onboarding_at(Some("dark"), name, cx);
            shell.onboarding.as_ref().map(|onboarding| onboarding.step)
        })
    };
    assert_eq!(at("7", cx), Some(Step::Color));
    assert_eq!(at("transparency", cx), Some(Step::Transparency));
    assert_eq!(at("8", cx), Some(Step::Transparency));
    assert_eq!(at("keys", cx), Some(Step::Keys));
}

/// The keyboard rests on the picker, which shows Standard, and the arrows move the pick, stopping
/// at either end.
#[gpui::test]
fn the_arrows_move_the_transparency_pick(cx: &mut TestAppContext) {
    let (shell, cx, _) = glass_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Transparency);
    later(&shell, cx, STEP_MOTION);
    let resting = cx.update(|window, cx| {
        let first = shell
            .read(cx)
            .onboarding
            .as_ref()
            .unwrap()
            .first_stop
            .clone();
        first.is_focused(window)
    });
    assert!(resting, "the keyboard rests on the picker");
    assert_eq!(level(&shell, cx), Some(Transparency::Standard));
    cx.simulate_keystrokes("left");
    assert_eq!(
        cx.update(|_, cx| transparency::level(cx)),
        Transparency::Less,
        "a pick applies at once, as Settings' does"
    );
    assert_eq!(level(&shell, cx), Some(Transparency::Less));
    cx.simulate_keystrokes("left up");
    assert_eq!(
        level(&shell, cx),
        Some(Transparency::Less),
        "it stops at Less"
    );
    cx.simulate_keystrokes("right down down");
    assert_eq!(level(&shell, cx), Some(Transparency::More), "and at More");
    cx.simulate_keystrokes("up");
    assert_eq!(level(&shell, cx), Some(Transparency::Standard));
}

/// ↩ goes on with the level shown, kept; Skip goes on with Standard, keeping nothing new.
#[gpui::test]
fn return_keeps_the_level_and_skip_puts_standard_back(cx: &mut TestAppContext) {
    let (shell, cx, _) = glass_window(cx);
    for (go_on, kept) in [
        ("enter", Transparency::More),
        ("skip", Transparency::Standard),
    ] {
        // A first run, at Standard.
        cx.update(|_, cx| transparency::set(Transparency::Standard, cx));
        started(&shell, cx);
        skip_to(&shell, cx, Step::Transparency);
        later(&shell, cx, STEP_MOTION);
        cx.simulate_keystrokes("right");
        match go_on {
            "skip" => {
                click_skip(cx);
            }
            key => cx.simulate_keystrokes(key),
        }
        assert_eq!(step(&shell, cx), Some(Step::Keys), "{go_on} went on");
        assert_eq!(level(&shell, cx), Some(kept), "{go_on}");
        assert_eq!(cx.update(|_, cx| transparency::level(cx)), kept, "{go_on}");
        later(&shell, cx, END_FADE);
    }
}

#[gpui::test]
fn the_cards_welcome_waits_under_the_flow_and_comes_as_it_fades(cx: &mut TestAppContext) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    waited(cx, 5.);
    let shown = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        let now = cx.executor().now();
        shell.read_with(cx, |shell, _| shell.card_opacity(now))
    };
    assert_eq!(
        shown(&shell, cx),
        0.,
        "the card's welcome draws nothing under the flow, which the window shows through"
    );
    click_skip(cx);
    cx.run_until_parked();
    assert!(
        shown(&shell, cx) > 0.,
        "the card's welcome shows as the flow fades"
    );
}

/// At the import on the sheet, Back goes back in its view where its state has somewhere to go back
/// to, as Esc does: from a browser's choices to the list, keeping the step; from the list, to the
/// step before.
#[gpui::test]
fn the_imports_back_goes_back_in_its_view_first(cx: &mut TestAppContext) {
    use crate::import::tests::Browser;
    let (shell, cx, view) = importing_from(Rc::new(Browser::new(None)), cx);
    let drawn = |cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.drawn());
    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    assert_eq!(drawn(cx), Some("pick"), "the browser's choices show");
    click_back(cx);
    cx.run_until_parked();
    assert_eq!(
        drawn(cx),
        Some("list"),
        "Back in a browser's choices goes back to the list"
    );
    assert_eq!(step(&shell, cx), Some(Step::Import), "keeping the step");
    click_back(cx);
    cx.run_until_parked();
    assert_eq!(
        step(&shell, cx),
        Some(Step::Intro),
        "Back at the list goes to the step before"
    );
}

/// The favorites step, from an import whose history offers `offered`, each ticked.
fn at_favorites<'a>(
    offered: &[&str],
    cx: &'a mut TestAppContext,
) -> (
    Entity<Shell>,
    &'a mut VisualTestContext,
    Entity<crate::import::ImportView>,
) {
    let (shell, cx, _) = first_window(cx);
    started(&shell, cx);
    skip_to(&shell, cx, Step::Import);
    let view = shell
        .read_with(cx, |shell, _| {
            shell.importing.as_ref().map(|i| i.view.clone())
        })
        .expect("the import view");
    let offered = (offered.iter())
        .map(|&url| crate::import::found::Tab {
            url: url.into(),
            name: String::new(),
        })
        .collect();
    view.update(cx, |view, _| view.offer(offered));
    view.update(cx, |_, cx| {
        cx.emit(ImportEvent::Started);
        cx.emit(ImportEvent::Placed);
    });
    waited(cx, IMPORT_HOLD);
    assert_eq!(step(&shell, cx), Some(Step::Favorites));
    later(&shell, cx, STEP_MOTION);
    (shell, cx, view)
}

/// The favorites step is on the sheet: its tiles on the left, which a click on one unticks, and
/// its foot's Back, Skip and button.
#[gpui::test]
fn the_favorites_step_is_on_the_sheet(cx: &mut TestAppContext) {
    let (shell, cx, view) = at_favorites(&["https://example.com/", "https://example.org/"], cx);
    let drawn = |name: &str, cx: &mut VisualTestContext| {
        cx.run_until_parked();
        view.read_with(cx, |view, _| view.drawn_place(name))
    };
    let tile = drawn("import-favorite example.com", cx).expect("the first site's tile is drawn");
    cx.simulate_click(tile, gpui::Modifiers::none());
    cx.run_until_parked();
    let ticked = shell.read_with(cx, |shell, cx| {
        (shell.favorites_ticked(cx).into_iter())
            .map(|tab| tab.url)
            .collect::<Vec<_>>()
    });
    assert_eq!(
        ticked,
        ["https://example.org/"],
        "a click on a tile on the sheet unticks it"
    );
    assert!(
        placed("skip", cx).is_some() && placed("back", cx).is_some(),
        "the sheet's foot has Back and Skip"
    );
}

/// The assistant's step comes after the account's, on the sheet, its foot with Back and Skip, and
/// under its rows the room kept for the sign-in's switch, a row high, with nothing in it.
#[gpui::test]
fn the_assistant_step_is_on_the_sheet_with_room_for_its_switch(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.agents.set(Some(false));
    started(&shell, cx);
    assert_eq!(
        steps(&shell, cx).get(1),
        Some(&Step::Assistant),
        "the assistant's step comes after the welcome: {:?}",
        steps(&shell, cx)
    );
    skip_to(&shell, cx, Step::Assistant);
    later(&shell, cx, STEP_MOTION);
    assert!(
        placed("skip", cx).is_some() && placed("back", cx).is_some(),
        "the sheet's foot has Back and Skip"
    );
    let room = placed("switch-place", cx).expect("the switch's room is laid out");
    assert_eq!(
        room.size.height,
        px(crate::tokens::settings::ROW_HEIGHT),
        "a row high"
    );
    click_skip(cx);
    assert_eq!(step(&shell, cx), Some(Step::Import), "Skip goes on");
}

/// The sheet's foot is in the same place on every step, the assistants' too with all its rows: a
/// list taller than its room scrolls in it, and the foot never moves down for it.
#[gpui::test]
fn the_sheets_foot_is_in_the_same_place_on_every_step(cx: &mut TestAppContext) {
    let (shell, cx, host) = first_window(cx);
    host.agents.set(Some(false));
    let _agents = cx.update(|_, cx| crate::agents::tests::stand_in(Vec::new(), cx));
    started(&shell, cx);
    let mut feet = Vec::new();
    // A flow that never ends fails rather than spins.
    for _ in 0..32 {
        let Some(at) = step(&shell, cx) else {
            break;
        };
        // Left from the last step, the flow is ending and its Skip goes with it.
        if shell.read_with(cx, |shell, _| {
            shell.onboarding.as_ref().unwrap().ending.is_some()
        }) {
            break;
        }
        if at != Step::Intro {
            later(&shell, cx, STEP_MOTION);
            if let Some(skip) = placed("skip", cx) {
                feet.push((at, skip.origin.y));
            }
        }
        match at {
            // The keys' step has no Skip; its button is the way in.
            Step::Intro | Step::Keys => cx.simulate_keystrokes("enter"),
            _ => click_skip(cx),
        }
    }
    assert!(
        feet.iter().any(|(at, _)| *at == Step::Assistant),
        "the assistants' step showed its foot: {feet:?}"
    );
    let (_, first) = feet[0];
    assert!(
        feet.iter().all(|(_, y)| *y == first),
        "Skip is where it is on every step: {feet:?}"
    );
}

/// The assistants' step looks for them once as it shows, off the UI thread, and each row then
/// says what was found, as Settings › Agents' does, and offers what it offers there. Nothing asks
/// again while the step stands; shown again, it looks again.
#[gpui::test]
fn the_assistant_step_looks_for_the_assistants_as_it_shows(cx: &mut TestAppContext) {
    use crate::agents::ready::{Found, Ready};
    use crate::agents::runner::Cli;
    let (shell, cx, host) = first_window(cx);
    host.agents.set(Some(false));
    let signed_out = Found {
        ready: Ready::SignedOut,
        ..Default::default()
    };
    let (agents, looks) =
        cx.update(|_, cx| crate::agents::tests::stand_in(vec![(Cli::Claude, signed_out)], cx));
    cx.simulate_a11y_activation();
    started(&shell, cx);
    let lines = |cx: &mut VisualTestContext| {
        agents.read_with(cx, |agents, _| {
            Cli::ALL.map(|cli| agents.assistant_row(cli).line)
        })
    };
    let all = steps(&shell, cx);
    let at = all
        .iter()
        .position(|&each| each == Step::Assistant)
        .unwrap();
    skip_to(&shell, cx, all[at - 1]);
    assert_eq!(looks(), 0, "nothing looked before the step");
    assert_eq!(lines(cx), ["Checking…", "Checking…"]);

    // From the welcome, whose Skip is Skip Setup, Return goes on.
    cx.simulate_keystrokes("enter");
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    later(&shell, cx, STEP_MOTION);
    assert_eq!(looks(), 1, "the step's showing looked once");
    assert_eq!(lines(cx), ["Not signed in", "Not installed"]);
    let said: Vec<String> = read(cx).into_iter().map(|(_, name, _)| name).collect();
    for offer in ["Sign In to Claude Code", "Get Codex"] {
        assert!(said.iter().any(|name| name == offer), "{offer} in {said:?}");
    }
    later(&shell, cx, 5.);
    assert_eq!(looks(), 1, "nothing looked again while it stood");

    click_skip(cx);
    later(&shell, cx, STEP_MOTION);
    click_back(cx);
    assert_eq!(step(&shell, cx), Some(Step::Assistant));
    cx.run_until_parked();
    assert_eq!(looks(), 2, "shown again, it looked again");
}

/// In the sheet's narrow half no assistant's name is cut by its button: each button is beside the
/// whole name, or, where the row has no room for both, under it.
#[gpui::test]
fn the_assistants_names_are_whole_beside_or_over_their_buttons(cx: &mut TestAppContext) {
    use crate::agents::ready::{Found, Ready};
    use crate::agents::runner::Cli;
    use crate::tokens::settings::{
        CONTROL_GAP, END_GAP, GLYPH_PLATE, GLYPH_PLATE_GAP, PADDING, ROW_X,
    };
    let (shell, cx, host) = first_window(cx);
    host.agents.set(Some(false));
    let signed_out = Found {
        ready: Ready::SignedOut,
        ..Default::default()
    };
    let (agents, _) =
        cx.update(|_, cx| crate::agents::tests::stand_in(vec![(Cli::Claude, signed_out)], cx));
    cx.simulate_a11y_activation();
    started(&shell, cx);
    skip_to(&shell, cx, Step::Assistant);
    later(&shell, cx, STEP_MOTION);
    let lines = agents.read_with(cx, |agents, _| {
        Cli::ALL.map(|cli| agents.assistant_row(cli).line)
    });
    assert_eq!(lines, ["Not signed in", "Not installed"]);

    let scale = cx.update(|window, _| window.scale_factor());
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let button = |label: &str| {
        let node = (tree.nodes.iter())
            .map(|(_, node)| node)
            .find(|node| node.role() == Role::Button && node.label().as_deref() == Some(label))
            .unwrap_or_else(|| panic!("no {label} button"));
        let bounds = node.bounds().expect("a button has bounds");
        (bounds.x0 as f32 / scale, bounds.y0 as f32 / scale)
    };
    let system = cx.update(|_, cx| gpui::WindowTextSystem::new(cx.text_system().clone()));
    for (row, name, offer) in [
        ("claude", "Claude Code", "Sign In to Claude Code"),
        ("codex", "Codex", "Get Codex"),
        ("another", "Another app", "How to Connect"),
    ] {
        let row = placed(row, cx).unwrap_or_else(|| panic!("{name}'s row wasn't drawn"));
        let whole = crate::tooltip::text_width(
            &name.into(),
            crate::tokens::text::BODY,
            gpui::FontWeight::MEDIUM,
            &system,
        );
        let name_left = f32::from(row.left()) + ROW_X + GLYPH_PLATE + GLYPH_PLATE_GAP;
        let (left, top) = button(offer);
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

/// The keys' step shows the app's keys as they are then: the key the user gave New Tab is the one
/// its row and the picture's bar show, spelled as a tooltip's chips are.
#[gpui::test]
fn the_keys_step_shows_the_keys_the_app_has(cx: &mut TestAppContext) {
    use crate::shell::{change_keys, user_keys};
    let (_, cx, _) = first_window(cx);
    let shown = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| super::frame::keys_shown(Command::NewTab, cx))
    };
    let chips =
        |cx: &mut VisualTestContext| cx.update(|_, cx| user_keys(cx).chips(Command::NewTab));
    let before = shown(cx).expect("New Tab has a key");
    assert_eq!(before, chips(cx).join(" "));
    cx.update(|_, cx| {
        change_keys(cx, |user| {
            user.set(Command::NewTab, vec!["ctrl-alt-cmd-t".to_string()])
        })
    })
    .unwrap();
    let after = shown(cx).expect("New Tab has a key");
    assert_ne!(after, before, "the key the user gave it");
    assert_eq!(after, chips(cx).join(" "));
}
