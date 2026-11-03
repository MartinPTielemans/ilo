//! How long the toast stays, on a host that records what the toast asks of its window.

use std::cell::RefCell;

use gpui::{Bounds, Pixels, TestAppContext, hsla};

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>, RefCell<Vec<Bounds<Pixels>>>);

impl Host for Recorder {
    fn show(&self) {
        self.0.borrow_mut().push("show".into());
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }

    fn take_clicks(&self, takes: bool) {
        self.0.borrow_mut().push(format!("clicks {takes}"));
    }

    fn plate(&self, bounds: Bounds<Pixels>) {
        self.1.borrow_mut().push(bounds);
    }
}

const TIME: Duration = Duration::from_secs(5);

fn open_toast(cx: &mut TestAppContext) -> Rc<Recorder> {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| open(host.clone(), |_| (hsla(0., 0., 0., 1.), Default::default()), cx)).expect("no toast window");
    host
}

fn show_deleted(cx: &mut TestAppContext) {
    cx.update(|cx| show("Space deleted! Use", Some(("⌘Z", "to undo.")), TIME, cx));
    cx.run_until_parked();
}

/// Makes the toast's window `width` wide, as the host makes it the page's width.
fn resize(cx: &mut TestAppContext, width: f32) {
    let window = cx.update(|cx| cx.global::<ToastWindow>().0);
    gpui::VisualTestContext::from_window(window.into(), cx)
        .simulate_resize(size(px(width), px(WINDOW_HEIGHT)));
}

/// Draws the toast's window, through the any-window handle, since drawing renders the toast the
/// other would lease.
fn draw(cx: &mut TestAppContext) {
    let window = cx.update(|cx| cx.global::<ToastWindow>().0);
    cx.update(|cx| {
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap()
    });
}

/// Puts the toast showing at rest, as if it came three seconds ago.
fn rest(cx: &mut TestAppContext) {
    let window = cx.update(|cx| cx.global::<ToastWindow>().0);
    cx.update(|cx| {
        window
            .update(cx, |toast, _, _| {
                let shown = toast.shown.as_mut().unwrap();
                shown.shown = Some(Instant::now() - Duration::from_secs(3));
            })
            .unwrap()
    });
}

/// Moves the clock on by `seconds` and returns what the toast asked of the host meanwhile.
fn after(cx: &mut TestAppContext, host: &Recorder, seconds: f32) -> Vec<String> {
    cx.executor().advance_clock(Duration::from_secs_f32(seconds));
    cx.run_until_parked();
    host.0.take()
}

#[gpui::test]
fn it_leaves_once_its_time_is_up(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    show_deleted(cx);
    assert_eq!(host.0.take(), ["show"]);
    assert!(after(cx, &host, 4.9).is_empty());
    // It hides once it has shrunk and faded away.
    assert!(after(cx, &host, 0.1).is_empty());
    assert_eq!(after(cx, &host, LEAVE), ["hide"]);
}

#[gpui::test]
fn another_toast_starts_the_time_again(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    show_deleted(cx);
    after(cx, &host, 4.);
    show_deleted(cx);
    assert_eq!(host.0.take(), ["show"]);
    // The first would have left at 5 s.
    assert!(after(cx, &host, 4.).is_empty());
    assert_eq!(after(cx, &host, 1. + LEAVE), ["hide"]);
}

/// Asked for again while it's in place, as a second ⌘↩ in the command bar asks for New Tab Created
/// again, it stays where it is and bumps to 1.044 of its size in 40 ms and back by 0.2 s, rather
/// than come in again; not before it's on screen, nor under Reduce Motion.
#[gpui::test]
fn a_toast_asked_for_again_in_place_bumps(cx: &mut TestAppContext) {
    let rest_pose = Pose { scale: 1., opacity: 1. };
    assert_eq!(rest_pose.bumped(0.), rest_pose);
    assert!((rest_pose.bumped(BUMP_UP).scale - BUMP).abs() < 1e-4);
    let falling = rest_pose.bumped((BUMP_UP + BUMP_TIME) / 2.).scale;
    assert!(falling > 1. && falling < BUMP, "{falling}");
    assert_eq!(rest_pose.bumped(BUMP_TIME), rest_pose);
    assert_eq!(bump(BUMP_TIME), None);

    open_toast(cx);
    let bumped = |cx: &mut TestAppContext| {
        let window = cx.update(|cx| cx.global::<ToastWindow>().0);
        cx.update(|cx| window.read(cx).unwrap().shown.as_ref().unwrap().bumped.is_some())
    };
    show_deleted(cx);
    // As AppKit hasn't put its window on screen yet.
    let window = cx.update(|cx| cx.global::<ToastWindow>().0);
    let unseen = |toast: &mut Toast, _: &mut Window, _: &mut Context<Toast>| {
        toast.shown.as_mut().unwrap().shown = None;
    };
    cx.update(|cx| window.update(cx, unseen)).unwrap();
    show_deleted(cx);
    assert!(!bumped(cx), "not yet on screen, it comes in once");
    rest(cx);
    show_deleted(cx);
    assert!(bumped(cx), "in place, it bumps");
    cx.update(|cx| cx.set_reduce_motion(true));
    show_deleted(cx);
    assert!(!bumped(cx), "under Reduce Motion it doesn't scale");
}

#[gpui::test]
fn it_overshoots_its_size_as_arcs_does(_: &mut TestAppContext) {
    let peak = (0..60).map(|frame| Pose::arriving(frame as f32 / 120.).scale).fold(0., f32::max);
    assert!((1.03..1.05).contains(&peak), "peak {peak}");
    assert_eq!(Pose::arriving(0.), Pose { scale: FROM, opacity: 0. });
    assert_eq!(Pose::arriving(1.), Pose { scale: 1., opacity: 1. });
    let gone = Pose::arriving(1.).leaving(LEAVE);
    assert!((gone.scale - FROM).abs() < 1e-3 && gone.opacity < 1e-3, "{gone:?}");
}

/// Capture's toast has buttons, so the clicks go to it rather than the page under it, until it
/// leaves; a toast without them lets them through again.
#[gpui::test]
fn a_toast_with_buttons_takes_clicks_while_it_shows(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    let save = Button { label: Label::Text("Save".into()), name: "save", run: Rc::new(|_, _| {}) };
    cx.update(|cx| show_with_buttons("Screenshot Copied to Clipboard", vec![save], TIME, cx));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["clicks true", "show"]);
    assert_eq!(after(cx, &host, 5.), ["clicks false"]);
    assert_eq!(after(cx, &host, LEAVE), ["hide"]);
    show_deleted(cx);
    assert_eq!(host.0.take(), ["show"]);
}

/// VoiceOver says a toast as it comes, wherever its cursor is, as a polite live region.
#[gpui::test]
fn voiceover_says_the_toast_as_it_comes(cx: &mut TestAppContext) {
    use gpui::{VisualTestContext, accesskit::Live};
    let host = Rc::new(Recorder::default());
    let window = cx.update(|cx| open(host.clone(), |_| (hsla(0., 0., 0., 1.), Default::default()), cx));
    let mut window = VisualTestContext::from_window(window.expect("no toast window").into(), cx);
    window.simulate_a11y_activation();
    show_deleted(cx);
    let tree = window.a11y_tree().expect("GPUI built a tree");
    let (_, toast) = tree.nodes.iter().find(|(_, node)| node.role() == Role::Status).unwrap();
    assert_eq!(toast.live(), Some(Live::Polite));
    assert_eq!(toast.value(), Some("Space deleted! Use ⌘Z to undo."));
}

/// The Undo a toast offers goes with the undo, however it was done, as does a toast naming ⌘Z,
/// but another toast stays.
#[gpui::test]
fn an_undo_toast_goes_once_the_undo_is_done_some_other_way(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    cx.update(|cx| show_undo("Removed from Favorites", Rc::new(|_| {}), TIME, cx));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["clicks true", "show"]);
    cx.update(|cx| hide_undo(cx));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["clicks false"]);
    assert_eq!(after(cx, &host, LEAVE), ["hide"]);

    let undo = crate::shell::keystroke_label("cmd-z");
    cx.update(|cx| show("Space deleted! Use", Some((&undo, "to undo.")), TIME, cx));
    host.0.take();
    cx.update(|cx| hide_undo(cx));
    cx.run_until_parked();
    assert_eq!(after(cx, &host, LEAVE), ["hide"], "Space deleted! goes with its ⌘Z");

    cx.update(|cx| show("Tab Pinned", None, TIME, cx));
    host.0.take();
    cx.update(|cx| hide_undo(cx));
    assert!(after(cx, &host, 4.9).is_empty(), "another toast stays its five seconds");
}

#[gpui::test]
fn an_undo_toast_runs_its_undo_with_a_click_on_the_button(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    let undone = Rc::new(std::cell::Cell::new(0));
    let counted = undone.clone();
    let undo = Rc::new(move |_: &mut App| counted.set(counted.get() + 1));
    cx.update(|cx| show_undo("Removed from Favorites", undo, TIME, cx));
    cx.run_until_parked();
    host.0.take();
    let run = cx.update(|cx| {
        let window = cx.global::<ToastWindow>().0;
        window.read(cx).unwrap().shown.as_ref().unwrap().buttons[0].run.clone()
    });
    cx.update(|cx| run(point(px(0.), px(0.)), cx));
    assert_eq!(undone.get(), 1);
}

#[gpui::test]
fn a_question_stays_until_a_button_answers_it(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    let answers = Rc::new(RefCell::new(Vec::new()));
    let button = |name: &'static str, send| {
        let answers = answers.clone();
        Button {
            label: Label::Text(name.into()),
            name,
            run: Rc::new(move |_, _| answers.borrow_mut().push(send)),
        }
    };
    let buttons = vec![button("Send Report", true), button("Don't Send", false)];
    cx.update(|cx| show_until_answered("ilo quit unexpectedly.", buttons, cx));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["clicks true", "show"]);
    assert!(after(cx, &host, 60.).is_empty(), "it waits for the answer");
    cx.update(|cx| {
        let window = cx.global::<ToastWindow>().0;
        window.update(cx, |toast, _, cx| toast.press(1, point(px(0.), px(0.)), cx)).unwrap();
    });
    cx.run_until_parked();
    assert_eq!(*answers.borrow(), [false]);
    assert_eq!(host.0.take(), ["clicks false"]);
    assert_eq!(after(cx, &host, LEAVE), ["hide"]);
}

/// A question asked again while it shows, as the archive's note at launch, is the one showing,
/// and once answered or replaced by another toast it's no longer.
#[gpui::test]
fn a_question_shows_until_answered_or_replaced(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    let note = "Tabs unseen for 12 hours went to the archive.";
    let ask = |cx: &mut TestAppContext| {
        let dismiss = Button {
            label: Label::Text("Dismiss".into()),
            name: "dismiss",
            run: Rc::new(|_, _| {}),
        };
        cx.update(|cx| show_until_answered(note, vec![dismiss], cx));
        cx.run_until_parked();
    };
    assert!(!cx.update(|cx| showing(note, cx)));
    ask(cx);
    assert!(cx.update(|cx| showing(note, cx)));
    assert!(!cx.update(|cx| showing("Space deleted! Use", cx)));
    cx.update(|cx| {
        let window = cx.global::<ToastWindow>().0;
        window.update(cx, |toast, _, cx| toast.press(0, point(px(0.), px(0.)), cx)).unwrap();
    });
    cx.run_until_parked();
    assert!(!cx.update(|cx| showing(note, cx)), "an answered question is leaving");
    after(cx, &host, LEAVE);
    ask(cx);
    show_deleted(cx);
    assert!(!cx.update(|cx| showing(note, cx)), "another toast took its place");
}

#[gpui::test]
fn a_resting_toast_comes_back_after_another_until_its_button_takes_it(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    let stop = Button { label: Label::Text("Stop".into()), name: "stop", run: Rc::new(|_, _| {}) };
    cx.update(|cx| show_resting("Recording a trace", vec![stop], cx));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["clicks true", "show"]);
    assert!(after(cx, &host, 60.).is_empty(), "it stays");
    show_deleted(cx);
    assert_eq!(host.0.take(), ["clicks false", "show"]);
    assert_eq!(after(cx, &host, 5. + LEAVE), ["hide", "clicks true", "show"]);
    let text = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let window = cx.global::<ToastWindow>().0;
            window.read(cx).unwrap().shown.as_ref().map(|shown| shown.text.clone())
        })
    };
    assert_eq!(text(cx).as_deref(), Some("Recording a trace"));
    cx.update(|cx| {
        let window = cx.global::<ToastWindow>().0;
        window.update(cx, |toast, _, cx| toast.press(0, point(px(0.), px(0.)), cx)).unwrap();
    });
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["clicks false"]);
    assert_eq!(after(cx, &host, LEAVE), ["hide"]);
    assert!(after(cx, &host, 10.).is_empty(), "it doesn't come back");
    assert_eq!(text(cx), None);
}

/// The toast's window ends below the plate, where its shadow falls, and the shadow fades out
/// before the window's edge. Above and right of it the window's edge is the browser window's.
#[test]
fn the_toasts_shadow_fades_out_inside_its_window() {
    let reach = Level::Popover.reach();
    for (height, top) in [(HEIGHT, TOP), (NOTE_HEIGHT, NOTE_TOP)] {
        let below = WINDOW_HEIGHT - top - height;
        assert!(below >= reach.below, "{below} below a toast of {height}, {reach:?}");
    }
}

/// Only the plate takes the clicks, so the host is told where it is in the window: the toast's
/// own size at the top right of the page-wide window, however wide its words and buttons make it,
/// once it has come to rest, since it's drawn scaled until then.
#[gpui::test]
fn the_host_is_told_where_the_plate_is_once_it_is_at_rest(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    let page = 800.;
    resize(cx, page);
    let undo = Rc::new(|_: &mut App| {});
    cx.update(|cx| show_undo("Removed from Favorites", undo, TIME, cx));
    cx.run_until_parked();
    draw(cx);
    assert!(host.1.take().is_empty(), "told while the toast was still arriving");
    rest(cx);
    draw(cx);
    draw(cx);
    let plates = host.1.take();
    // Drawn twice, and told once.
    assert_eq!(plates.len(), 1, "{plates:?}");
    let plate = plates[0];
    assert_eq!(f32::from(plate.size.height), HEIGHT);
    assert_eq!(f32::from(plate.origin.y), TOP);
    assert_eq!(f32::from(plate.right()), page - RIGHT);
    assert!(f32::from(plate.size.width) < page - RIGHT - WINDOW_MARGIN, "{plate:?}");
    // The margin where the shadow falls is left of it and under it.
    assert!(f32::from(plate.origin.x) > 0. && f32::from(plate.bottom()) < WINDOW_HEIGHT);
}

/// A toast too long for the page it shows over, as the archive's note is in Inter on a narrow
/// window, shortens its words where its window's edge cut it (#1462): its plate stays inside the
/// window with the margin its shadow falls in, at the page's corner and at its own height.
#[gpui::test]
fn a_toast_too_long_for_its_page_shortens_its_words(cx: &mut TestAppContext) {
    let host = open_toast(cx);
    let page = 480.;
    resize(cx, page);
    let button = |label: &'static str| Button {
        label: Label::Text(label.into()),
        name: label,
        run: Rc::new(|_, _| {}),
    };
    let note = "Tabs unseen for 12 hours went to the archive.";
    cx.update(|cx| show_until_answered(note, vec![button("View Archive"), button("Dismiss")], cx));
    cx.run_until_parked();
    rest(cx);
    draw(cx);
    let plate = host.1.take().pop().expect("the host wasn't told where the plate is");
    assert_eq!(f32::from(plate.origin.x), WINDOW_MARGIN, "{plate:?}");
    assert_eq!(f32::from(plate.right()), page - RIGHT);
    assert_eq!(f32::from(plate.origin.y), TOP);
    assert_eq!(f32::from(plate.size.height), HEIGHT);
}

/// A toast that is itself its button, as New Tab Created is, takes clicks and draws no chip, and
/// a click anywhere on its plate runs it, as VoiceOver can, the toast being a button.
#[gpui::test]
fn a_click_on_a_toast_that_is_its_button_runs_it(cx: &mut TestAppContext) {
    use gpui::VisualTestContext;
    let host = Rc::new(Recorder::default());
    let window = cx.update(|cx| open(host.clone(), |_| (hsla(0., 0., 0., 1.), Default::default()), cx));
    let window = window.expect("no toast window");
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_a11y_activation();
    let clicked = Rc::new(Cell::new(0));
    let counted = clicked.clone();
    let button = Button {
        label: Label::Plate,
        name: "show",
        run: Rc::new(move |_, _| counted.set(counted.get() + 1)),
    };
    cx.update(|cx| show_with_buttons("New Tab Created", vec![button], TIME, cx));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["clicks true", "show"]);
    cx.update(|cx| {
        window
            .update(cx, |toast, _, _| {
                toast.shown.as_mut().unwrap().shown = Some(Instant::now() - Duration::from_secs(3));
            })
            .unwrap()
    });
    // Through the any-window handle, since drawing renders the toast the other would lease.
    cx.update(|cx| {
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap()
    });
    let tree = visual.a11y_tree().expect("GPUI built a tree");
    let roles: Vec<_> = tree.nodes.iter().map(|(_, node)| node.role()).collect();
    assert_eq!(roles.iter().filter(|&&role| role == Role::Button).count(), 1, "{roles:?}");
    let plate = *host.1.borrow().last().expect("the plate wasn't told");
    // Near its left end, over the words rather than a chip.
    visual.simulate_click(point(plate.left() + px(20.), plate.center().y), Default::default());
    visual.run_until_parked();
    assert_eq!(clicked.get(), 1);
    assert_eq!(after(cx, &host, LEAVE), ["clicks false", "hide"]);
}
