//! One physical two-finger swipe, given to the app as AppKit's phased events and to the site's
//! demo as a browser's wheel events for the same fingers, to compare where each puts the page.
//! `SWIPE_MODEL_CSV=path cargo test --release --lib write_swipe_model -- --ignored` writes the
//! page's place over time for each, to plot, and `swipe_model_stats` tries many gestures. The
//! trackpad's rate, its noise and AppKit's momentum are assumed; the demo's `?trace=1` records a
//! real swipe's wheel events to check them against, and `replay_wheel_trace` replays one.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use web_time::Instant;

use gpui::{
    Entity, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
};

use crate::shell::{SWIPE_TRACKING, Shell, recorder::Recorder};

/// A trackpad's events, and AppKit's momentum after the fingers lift, 120 a second.
const EVENT_MS: f64 = 1000. / 120.;
/// AppKit's momentum falls away as e^(−t/τ), like UIScrollView's normal deceleration.
const MOMENTUM_TAU_MS: f64 = 400.;

/// What an event of AppKit's is.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// Fingers touching, with no move yet.
    Down,
    Fingers,
    Lift,
    Momentum,
    MomentumEnd,
}

#[derive(Clone, Copy, Debug)]
struct Event {
    at_ms: f64,
    /// Points across, positive with the fingers going right.
    dx: f32,
    /// Points along, as the fingers drift up or down.
    dy: f32,
    kind: Kind,
}

/// A swipe whose fingers go at `speed(t)` points a millisecond for `lift_ms`, then lift into
/// AppKit's momentum at the speed they lifted at. A trackpad's own events come a little unevenly,
/// and its fingers' deltas by a quarter either way, from `seed`; the momentum's are smooth.
fn gesture(lift_ms: f64, seed: u64, speed: impl Fn(f64) -> f64) -> Vec<Event> {
    gesture_falling(lift_ms, seed, MOMENTUM_TAU_MS, speed)
}

/// The same, with the momentum falling away as e^(−t/`tau_ms`).
fn gesture_falling(lift_ms: f64, seed: u64, tau_ms: f64, speed: impl Fn(f64) -> f64) -> Vec<Event> {
    let mut random = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut noise = move || {
        random = random
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (random >> 33) as f64 / (1u64 << 31) as f64 * 2. - 1.
    };
    let mut events = vec![Event {
        at_ms: 0.,
        dx: 0.,
        dy: 0.,
        kind: Kind::Down,
    }];
    let mut at = 0.;
    while at + EVENT_MS <= lift_ms {
        let next = (at + EVENT_MS + 1.5 * noise()).min(lift_ms);
        // The way since the last event, by the midpoint's speed.
        let dx = speed((at + next) / 2.) * (next - at) * (1. + 0.25 * noise());
        at = next;
        events.push(Event {
            at_ms: at,
            dx: dx as f32,
            dy: 0.,
            kind: Kind::Fingers,
        });
    }
    events.push(Event {
        at_ms: lift_ms,
        dx: 0.,
        dy: 0.,
        kind: Kind::Lift,
    });
    let lifted = speed(lift_ms);
    let mut t = 0.;
    loop {
        t += EVENT_MS;
        let dx = lifted * EVENT_MS * (-t / tau_ms).exp();
        if dx.abs() < 0.1 {
            break;
        }
        events.push(Event {
            at_ms: lift_ms + t,
            dx: dx as f32,
            dy: 0.,
            kind: Kind::Momentum,
        });
    }
    events.push(Event {
        at_ms: lift_ms + t,
        dx: 0.,
        dy: 0.,
        kind: Kind::MomentumEnd,
    });
    events
}

/// `first`, its momentum stopped by fingers coming down again at `touch_ms`, and `then` from there.
fn and_then(first: Vec<Event>, touch_ms: f64, then: Vec<Event>) -> Vec<Event> {
    let before = first.into_iter().filter(|event| event.at_ms < touch_ms);
    let after = then.into_iter().map(|event| Event {
        at_ms: event.at_ms + touch_ms,
        ..event
    });
    before.chain(after).collect()
}

/// Smoothly from `from` to `to` points a millisecond between `start` and `end` ms.
fn ramp(t: f64, start: f64, end: f64, from: f64, to: f64) -> f64 {
    let s = ((t - start) / (end - start)).clamp(0., 1.);
    from + (to - from) * (0.5 - 0.5 * (std::f64::consts::PI * s).cos())
}

/// A tenth of a second toward the next Space, lifting at its fastest.
fn flick(seed: u64) -> Vec<Event> {
    gesture(100., seed, |t| -ramp(t, 0., 100., 0., 1.))
}

/// The gestures, by name, each toward the next Space, so with the fingers going left.
fn gestures() -> Vec<(&'static str, Vec<Event>)> {
    vec![
        // Steady at 0.3 points a millisecond, slowing to 0.1 as the fingers lift.
        (
            "slow drag and let go",
            gesture(700., 1, |t| {
                -ramp(t, 0., 150., 0., 0.3) + ramp(t, 450., 700., 0., 0.2)
            }),
        ),
        ("short flick", flick(2)),
        // Toward the next Space, then back past where it began, lifting on the way back.
        (
            "drag that reverses",
            gesture(800., 3, |t| {
                -ramp(t, 0., 120., 0., 0.35) + ramp(t, 300., 520., 0., 0.65)
            }),
        ),
        // The second as the first's momentum still goes, 250 ms after its fingers lift.
        ("two flicks", and_then(flick(4), 350., flick(5))),
    ]
}

/// How a browser hands AppKit's events to the page as wheel events: those with a delta, with no
/// phase, and with what came while the page was busy summed into one.
#[derive(Clone, Copy)]
enum Delivery {
    /// Each as it comes.
    Direct,
    /// Each as it comes, to the whole pixel.
    Whole,
    /// Ahead of each frame at `hz`, summed since the last.
    Frames(f64),
}

impl Delivery {
    fn name(self) -> String {
        match self {
            Delivery::Direct => "each event".into(),
            Delivery::Whole => "each event in whole px".into(),
            Delivery::Frames(hz) => format!("{hz:.0} Hz frames"),
        }
    }

    /// The wheel events the page gets: the ms it gets each at, the ms its stamp says, the last
    /// summed into it's, and its deltas across and along.
    fn wheels(self, events: &[Event]) -> Vec<Wheel> {
        let moving = events
            .iter()
            .filter(|event| event.dx != 0. || event.dy != 0.);
        match self {
            Delivery::Direct => moving
                .map(|event| (event.at_ms, event.at_ms, event.dx, event.dy))
                .collect(),
            Delivery::Whole => moving
                .map(|event| (event.at_ms, event.at_ms, event.dx.round(), event.dy.round()))
                .filter(|&(_, _, dx, dy)| dx != 0. || dy != 0.)
                .collect(),
            Delivery::Frames(hz) => {
                let frame = 1000. / hz;
                let mut wheels: Vec<Wheel> = Vec::new();
                for event in moving {
                    // Frames a third of one out of step with the trackpad's events.
                    let at = ((event.at_ms - frame / 3.) / frame).ceil() * frame + frame / 3.;
                    match wheels.last_mut() {
                        Some((last, stamp, dx, dy)) if *last == at => {
                            *stamp = event.at_ms;
                            *dx += event.dx;
                            *dy += event.dy;
                        }
                        _ => wheels.push((at, event.at_ms, event.dx, event.dy)),
                    }
                }
                wheels
            }
        }
    }
}

/// A wheel event the page gets: the ms it comes, the ms its stamp says, and its deltas across and
/// along.
type Wheel = (f64, f64, f32, f32);

/// `events` with the fingers drifting along as they go across, by `slope` of their way across and
/// up to `wobble` of it either way an event, from `seed`, as a hand's arc on a trackpad never goes
/// straight. AppKit starts a scroll only once the fingers have gone a few points, so the first
/// event after they land goes the way they do, and the momentum the way they went as they lifted.
fn drifting(events: Vec<Event>, slope: f32, wobble: f32, seed: u64) -> Vec<Event> {
    let mut before = Kind::Lift;
    let mut lifted = slope;
    let drifted = events.into_iter().enumerate().map(|(n, event)| {
        let dy = match (event.kind, before) {
            (Kind::Fingers, Kind::Down) => event.dx.abs() * slope,
            (Kind::Fingers, _) => {
                let along = slope + wobble * (2. * draw(seed, n as u64) as f32 - 1.);
                lifted = along;
                event.dx.abs() * along
            }
            (Kind::Momentum, _) => event.dx.abs() * lifted,
            _ => 0.,
        };
        before = event.kind;
        Event { dy, ..event }
    });
    drifted.collect()
}

/// Five Spaces, showing the third, as the core leaves them.
fn five_spaces(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = |id: u64| {
            (
                id,
                format!("Space {id}").into(),
                None,
                1,
                "".into(),
                "".into(),
                "".into(),
            )
        };
        shell.set_spaces((1..=5).map(space).collect(), cx);
        shell.set_space(3, cx);
        shell.settle = None;
    });
    cx.run_until_parked();
    (shell, host, cx)
}

/// Where the page is now, in Spaces from the first's place: 2 has the third in its place.
fn slide(shell: &Shell) -> f32 {
    let index = |id| {
        shell
            .spaces
            .iter()
            .position(|space| space.id == id)
            .unwrap_or(0) as f32
    };
    let width = shell.width();
    if let Some(swipe) = shell
        .swipe
        .as_ref()
        .filter(|swipe| swipe.across == Some(true))
    {
        return index(swipe.from) - shell.swiped_to(swipe) / width;
    }
    match &shell.settle {
        Some(settle) => index(settle.space) - settle.offset().unwrap_or(0.) / width,
        None => index(shell.space),
    }
}

/// Each `STEP_MS`, the page's place.
const STEP_MS: f64 = 2.;

/// An event given to the shell at the instant it's given.
type Send = Box<dyn Fn(&mut Shell, Instant, &mut gpui::Window, &mut gpui::Context<Shell>)>;

/// What a run gives: the page's place each step, and the Spaces the shell asked to show.
struct Run {
    slides: Vec<f32>,
    /// Each step, the pages drawn and where, as `pages` has them, in Spaces from the shown's place.
    pages: Vec<Vec<(u64, f32)>>,
    /// Each step, whether fingers are down on a swipe.
    down: Vec<bool>,
    shown: Vec<u64>,
    /// When the shell first asked for another Space.
    first_shown_ms: Option<f64>,
}

impl Run {
    /// The most the page moved in any 60 Hz frame, in Spaces.
    fn largest_step(&self) -> f32 {
        let frame = (1000. / 60. / STEP_MS) as usize;
        let places = self.slides.iter().zip(self.slides.iter().skip(frame));
        places
            .map(|(then, now)| (now - then).abs())
            .fold(0., f32::max)
    }
}

/// The fingers' and momentum's events, each `(ms, what)` as `send` gives it to the shell, which
/// shows a Space as soon as it asks to, as the core does, with the page's place each step. The
/// window draws at `frames`, in ms, after the events up to each.
fn run(cx: &mut TestAppContext, end_ms: f64, mut events: Vec<(f64, Send)>, frames: &[f64]) -> Run {
    let (shell, host, cx) = five_spaces(cx);
    let start = cx.executor().now();
    let at = |ms: f64| start + Duration::from_secs_f64(ms / 1000.);
    // GPUI's clock is the run's, so the settle and the demo's quiet timers keep its time.
    let to = |cx: &mut gpui::VisualTestContext, ms: f64| {
        let ahead = at(ms).saturating_duration_since(cx.executor().now());
        cx.executor().advance_clock(ahead);
        cx.run_until_parked();
    };
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut events = events.into_iter().peekable();
    let mut slides = Vec::new();
    let mut pages = Vec::new();
    let mut down = Vec::new();
    let mut first_shown_ms = None;
    // The core shows a Space as soon as the shell asks, whether an event or a timer asked.
    let show = |cx: &mut gpui::VisualTestContext, when: f64, first: &mut Option<f64>| {
        let asked = host.shown.borrow().last().copied();
        if asked.is_some() && first.is_none() {
            *first = Some(when);
        }
        shell.update(cx, |shell, cx| {
            if let Some(space) = asked.filter(|&space| space != shell.space) {
                shell.set_space(space, cx);
            }
        });
    };
    // The Library's motion, among others, ends only as it's drawn: undrawn, a closed Library
    // stayed in motion, and the next swipe moved it instead of the Spaces.
    let mut frames = frames.iter().copied().peekable();
    let draw = |cx: &mut gpui::VisualTestContext, frame: f64| {
        to(cx, frame);
        cx.update(|window, cx| window.draw(cx).clear(cx));
    };
    let mut ms = 0.;
    while ms <= end_ms {
        while let Some((when, send)) = events.next_if(|(when, _)| *when <= ms) {
            // A browser hands on the events a frame heard before it draws that frame.
            while let Some(frame) = frames.next_if(|&frame| frame < when) {
                draw(cx, frame);
            }
            to(cx, when);
            show(cx, when, &mut first_shown_ms);
            let now = at(when);
            shell.update_in(cx, |shell, window, cx| send(shell, now, window, cx));
            show(cx, when, &mut first_shown_ms);
        }
        while let Some(frame) = frames.next_if(|&frame| frame <= ms) {
            draw(cx, frame);
        }
        to(cx, ms);
        show(cx, ms, &mut first_shown_ms);
        slides.push(shell.read_with(cx, |shell, _| slide(shell)));
        pages.push(shell.read_with(cx, |shell, _| {
            let width = shell.width();
            let pages = shell.pages().into_iter();
            pages.map(|(space, x)| (space, x / width)).collect()
        }));
        down.push(shell.read_with(cx, |shell, _| shell.swipe.is_some()));
        ms += STEP_MS;
    }
    let shown = host.shown.borrow().clone();
    Run {
        slides,
        pages,
        down,
        shown,
        first_shown_ms,
    }
}

/// An event over the sidebar, `dx` across and `dy` along as GPUI has them.
fn scroll(touch_phase: TouchPhase, dx: f32, dy: f32) -> ScrollEvent {
    ScrollEvent {
        position: point(px(110.), px(500.)),
        delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
        touch_phase,
        ..Default::default()
    }
}

/// The app: AppKit's phases, as gpui_macos gives them.
fn in_app(cx: &mut TestAppContext, events: &[Event], end_ms: f64) -> Run {
    let sends = events
        .iter()
        .map(|&event| {
            let phase = match event.kind {
                Kind::Down => TouchPhase::Started,
                Kind::Lift => TouchPhase::Ended,
                // The first of the fingers' with a delta is AppKit's Began.
                Kind::Fingers | Kind::Momentum | Kind::MomentumEnd => TouchPhase::Moved,
            };
            let first = event.kind == Kind::Fingers
                && events
                    .iter()
                    .find(|other| other.kind == Kind::Fingers)
                    .map(|e| e.at_ms)
                    == Some(event.at_ms);
            let phase = if first { TouchPhase::Started } else { phase };
            let send: Send = Box::new(move |shell, now, window, cx| {
                shell.scroll_at(&scroll(phase, event.dx, event.dy), now, window, cx);
            });
            (event.at_ms, send)
        })
        .collect();
    run(cx, end_ms, sends, &[])
}

/// The site's demo: the browser's wheel events, each at its stamp, and the quiet timers they set.
fn in_demo(cx: &mut TestAppContext, events: &[Event], delivery: Delivery, end_ms: f64) -> Run {
    in_demo_wheels(cx, delivery.wheels(events), end_ms)
}

/// The site's demo given `wheels`.
fn in_demo_wheels(cx: &mut TestAppContext, wheels: Vec<Wheel>, end_ms: f64) -> Run {
    let sends = wheels
        .into_iter()
        .map(|(at_ms, stamp_ms, dx, dy)| {
            let stamped = Duration::from_secs_f64((at_ms - stamp_ms) / 1000.);
            let send: Send = Box::new(move |shell, now, window, cx| {
                let event = scroll(TouchPhase::Moved, dx, dy);
                shell.web_scroll(&event, now - stamped, window, cx);
            });
            (at_ms, send)
        })
        .collect();
    run(cx, end_ms, sends, &[])
}

/// The deliveries the demo is compared in.
fn deliveries() -> [Delivery; 4] {
    [
        Delivery::Direct,
        Delivery::Whole,
        Delivery::Frames(60.),
        Delivery::Frames(120.),
    ]
}

/// Each gesture, in each way a browser may hand it on, shows the Spaces in the demo that it does
/// in the app, starting no sooner than the fingers lift there and soon after, and never moves the
/// pages further in a frame than the app does, give or take STEP_SLACK. A lift found late can't
/// catch up with the app's settle without that jump, which the eye takes for a snap.
#[gpui::test]
fn the_demo_swipes_as_the_app_does(cx: &mut TestAppContext) {
    for (name, events) in gestures() {
        swipes_as_the_app_does(cx, name, &events);
    }
}

/// The same with the fingers drifting along by 0.4 of their way across, ±0.4 an event, as a
/// hand's arc does (22° on average): the demo tells their swipe across from one event a browser
/// rounded or summed, or from their drift, no worse than the app tells it from AppKit's.
#[gpui::test]
fn the_demo_swipes_as_the_app_does_while_the_fingers_drift(cx: &mut TestAppContext) {
    for (name, events) in gestures() {
        for seed in 0..4 {
            let events = drifting(events.clone(), 0.4, 0.4, seed);
            swipes_as_the_app_does(cx, &format!("{name} drifting, seed {seed}"), &events);
        }
    }
}

/// Drags that turn back past where they began, which a browser summing two events a frame at 60
/// Hz once had the demo lift and land on the next Space mid-drag, where the app turns to the one
/// before: seed 13 a hand slowing evenly enough, over a few frames, to read as momentum's steady
/// fall, and seed 25 the hand braking for its turn as momentum's gentle one.
#[gpui::test]
fn the_demo_turns_back_as_the_app_does(cx: &mut TestAppContext) {
    for seed in [13, 25] {
        let events = random_gesture(2, seed);
        swipes_as_the_app_does(cx, &format!("turn back, seed {seed}"), &events);
    }
}

fn swipes_as_the_app_does(cx: &mut TestAppContext, name: &str, events: &[Event]) {
    const STEP_SLACK: f32 = 0.05;
    let end = events.last().map_or(0., |event| event.at_ms) + 400.;
    let app = in_app(cx, events, end);
    for delivery in deliveries() {
        let demo = in_demo(cx, events, delivery, end);
        let way = format!("{name}, {}", delivery.name());
        assert_eq!(demo.shown, app.shown, "{way}: the Spaces shown");
        let lag = demo
            .first_shown_ms
            .zip(app.first_shown_ms)
            .map(|(demo, app)| demo - app);
        assert!(
            lag.is_some_and(|lag| (0. ..300.).contains(&lag)),
            "{way}: {lag:?} ms after the app"
        );
        let (step, app_step) = (demo.largest_step(), app.largest_step());
        assert!(
            step <= app_step + STEP_SLACK,
            "{way}: {step} of a Space in a frame, the app {app_step}"
        );
    }
}

/// A hard flick toward the next Space, at `peak` points a millisecond as the fingers lift, with
/// momentum falling as e^(−t/`tau_ms`), seconds long.
fn hard_flick(peak: f64, tau_ms: f64, seed: u64) -> Vec<Event> {
    let lift = 70. + 50. * draw(seed, 7);
    gesture_falling(lift, seed, tau_ms, move |t| -ramp(t, 0., lift, 0., peak))
}

impl Run {
    /// The step the fingers last lifted at, after which the page only settles.
    fn released(&self) -> usize {
        self.down
            .iter()
            .rposition(|&down| down)
            .map_or(0, |step| step + 1)
    }

    /// How far the page went past the Space it ends on, toward the one after, in Spaces.
    fn past(&self) -> f32 {
        let end = self.slides.last().copied().unwrap_or(0.);
        let start = self.slides.first().copied().unwrap_or(0.);
        let ahead = (end - start).signum();
        let most = self.slides.iter().map(|place| (place - end) * ahead);
        most.fold(0., f32::max)
    }

    /// Once the fingers lifted, the first step the page went further from the place it ends at,
    /// and from how far to how far, in Spaces: the settle heading on past its place.
    fn heads_away(&self) -> Option<(f64, f32, f32)> {
        const SLACK: f32 = 0.002;
        let end = self.slides.last().copied().unwrap_or(0.);
        let mut nearest = f32::MAX;
        for (step, &place) in self.slides.iter().enumerate().skip(self.released()) {
            let off = (place - end).abs();
            if off > nearest + SLACK {
                return Some((step as f64 * STEP_MS, nearest, off));
            }
            nearest = nearest.min(off);
        }
        None
    }

    /// The first step two pages were drawn closer than a width apart, over each other.
    fn overlaps(&self) -> Option<(f64, f32)> {
        self.pages.iter().enumerate().find_map(|(step, pages)| {
            let pairs = pages.iter().zip(pages.iter().skip(1));
            let gaps = pairs.map(|((_, x), (_, y))| (x - y).abs());
            let gap = gaps.fold(f32::MAX, f32::min);
            (gap < 0.998).then_some((step as f64 * STEP_MS, gap))
        })
    }
}

/// How much further than the app's a hard flick may take the page past its Space, in Spaces: a
/// browser's lift comes some events late, and at 60 Hz the frame that straddles it can read as the
/// fingers' fastest and stretch the band a pixel.
const PAST_SLACK: f32 = 0.005;

/// A hard flick lands on the next Space, as the app's does, goes no further past it than the app's
/// give or take PAST_SLACK, and once the demo finds the fingers lifted, the page settles there
/// without heading on toward the Space after, however long the momentum runs; the two pages drawn
/// as it moves never overlap.
#[gpui::test]
fn a_hard_flick_settles_on_one_space(cx: &mut TestAppContext) {
    for (peak, tau) in [(2., 400.), (4., 400.), (6., 400.), (4., 800.)] {
        for (slope, wobble) in [(0., 0.), (0.5, 0.5)] {
            for seed in 0..3 {
                let events = drifting(hard_flick(peak, tau, seed), slope, wobble, seed);
                let end = events.last().map_or(0., |event| event.at_ms) + 400.;
                let app = in_app(cx, &events, end);
                assert_eq!(app.shown, vec![4], "the app's");
                for delivery in deliveries() {
                    let demo = in_demo(cx, &events, delivery, end);
                    let way = format!(
                        "{peak} pt/ms, τ {tau} ms, drift {slope}/{wobble}, seed {seed}, {}",
                        delivery.name()
                    );
                    assert_eq!(demo.shown, app.shown, "{way}: the Spaces shown");
                    assert_eq!(demo.heads_away(), None, "{way}: after the lift");
                    assert_eq!(demo.overlaps(), None, "{way}: pages over each other");
                    let (past, app_past) = (demo.past(), app.past());
                    assert!(
                        past <= app_past + PAST_SLACK,
                        "{way}: {past:.3} of a Space past its place, the app {app_past:.3}"
                    );
                }
            }
        }
    }
}

/// For hard flicks of each speed and momentum, drifting or not, in each way a browser hands them
/// on: in how many the demo shows other Spaces than the app, the page heads away from its place
/// after the lift, or goes further past it than the app's, and the most it went past in each.
#[gpui::test]
#[ignore]
fn hard_flick_stats(cx: &mut TestAppContext) {
    for (peak, tau) in [
        (2., 400.),
        (3., 400.),
        (4., 400.),
        (6., 400.),
        (4., 800.),
        (6., 800.),
    ] {
        for (slope, wobble) in [(0., 0.), (0.5, 0.5)] {
            for delivery in deliveries() {
                let (mut shown, mut away, mut further) = (0, 0, 0);
                let (mut app_past, mut demo_past) = (0f32, 0f32);
                for seed in 0..12 {
                    let events = drifting(hard_flick(peak, tau, seed), slope, wobble, seed);
                    let end = events.last().map_or(0., |event| event.at_ms) + 400.;
                    let app = in_app(cx, &events, end);
                    let demo = in_demo(cx, &events, delivery, end);
                    shown += usize::from(demo.shown != app.shown);
                    away += usize::from(demo.heads_away().is_some());
                    further += usize::from(demo.past() > app.past() + PAST_SLACK);
                    app_past = app_past.max(app.past());
                    demo_past = demo_past.max(demo.past());
                }
                eprintln!(
                    "{peak} pt/ms τ {tau:>3} drift {slope}/{wobble} {:>22}: other Spaces {shown:>2}/12, heads away after the lift {away:>2}/12, further past {further:>2}/12, most past app {app_past:.3} demo {demo_past:.3}",
                    delivery.name()
                );
            }
        }
    }
}

/// For each gesture with its fingers drifting along by each slope and wobble, in each way a
/// browser hands it on, whether the demo shows other Spaces than the app.
#[gpui::test]
#[ignore]
fn swipe_drift_stats(cx: &mut TestAppContext) {
    let mut wrong = 0;
    for (slope, wobble) in [
        (0.2, 0.3),
        (0.2, 0.6),
        (0.4, 0.4),
        (0.4, 0.8),
        (0.7, 0.3),
        (0.7, 0.6),
    ] {
        for (name, events) in gestures() {
            for seed in 0..4 {
                let events = drifting(events.clone(), slope, wobble, seed);
                let end = events.last().map_or(0., |event| event.at_ms) + 400.;
                let app = in_app(cx, &events, end);
                for delivery in deliveries() {
                    let demo = in_demo(cx, &events, delivery, end);
                    if demo.shown != app.shown {
                        wrong += 1;
                        eprintln!(
                            "slope {slope} wobble {wobble}, {name}, seed {seed}, {}: {:?} vs the app's {:?}",
                            delivery.name(),
                            demo.shown,
                            app.shown
                        );
                    }
                }
            }
        }
    }
    eprintln!(
        "{wrong} of {} runs show other Spaces than the app",
        6 * 4 * 4 * 4
    );
}

/// Writes, for each gesture, the fingers' way and the page's place in the app and in the demo
/// for each delivery, as `gesture,path,ms,spaces`.
#[gpui::test]
#[ignore]
fn write_swipe_model(cx: &mut TestAppContext) {
    use std::fmt::Write;
    let Ok(path) = std::env::var("SWIPE_MODEL_CSV") else {
        return;
    };
    let mut csv = String::from("gesture,path,ms,spaces\n");
    for (name, events) in gestures() {
        let end_ms = 1400.;
        let width = {
            let (shell, _, window) = five_spaces(cx);
            shell.read_with(window, |shell, _| shell.width())
        };
        let mut way = 0.;
        let mut fingers = events
            .iter()
            .filter(|e| matches!(e.kind, Kind::Fingers))
            .peekable();
        let mut ms = 0.;
        while ms <= end_ms {
            while let Some(event) = fingers.next_if(|event| event.at_ms <= ms) {
                way += event.dx;
            }
            writeln!(
                csv,
                "{name},fingers,{ms},{}",
                2. - SWIPE_TRACKING * way / width
            )
            .unwrap();
            ms += STEP_MS;
        }
        let mut write = |path: &str, run: Run| {
            for (step, spaces) in run.slides.iter().enumerate() {
                writeln!(csv, "{name},{path},{},{spaces}", step as f64 * STEP_MS).unwrap();
            }
            eprintln!("{name}: {path} showed {:?}", run.shown);
        };
        write("app", in_app(cx, &events, end_ms));
        for delivery in deliveries() {
            write(
                &format!("demo: {}", delivery.name()),
                in_demo(cx, &events, delivery, end_ms),
            );
        }
    }
    std::fs::write(path, csv).unwrap();
}

/// A number in [0, 1) from `seed`'s `n`th draw.
fn draw(seed: u64, n: u64) -> f64 {
    let mut x = seed.wrapping_mul(0x9E3779B97F4A7C15) ^ n.wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D049BB133111EB);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// A gesture of `kind` with its speeds and times drawn from `seed`.
fn random_gesture(kind: usize, seed: u64) -> Vec<Event> {
    let between = |n, low: f64, high: f64| low + (high - low) * draw(seed, n);
    match kind {
        // A drag at a steady speed, slowing for the lift.
        0 => {
            let (speed, lift) = (between(1, 0.1, 0.6), between(2, 300., 900.));
            let slow = between(3, 0.2, 1.);
            let ease = between(4, 100., 250.);
            gesture(lift, seed, move |t| {
                -ramp(t, 0., 120., 0., speed) + ramp(t, lift - ease, lift, 0., speed * (1. - slow))
            })
        }
        // A flick.
        1 => {
            let (peak, lift) = (between(1, 0.4, 2.), between(2, 60., 160.));
            gesture(lift, seed, move |t| -ramp(t, 0., lift, 0., peak))
        }
        // A drag that turns back.
        2 => {
            let (on, back) = (between(1, 0.2, 0.5), between(2, 0.15, 0.5));
            let (turn, lift) = (between(3, 150., 350.), between(4, 200., 400.));
            gesture(turn + lift, seed, move |t| {
                -ramp(t, 0., 100., 0., on) + ramp(t, turn, turn + 150., 0., on + back)
            })
        }
        // Two flicks, the second in the first's momentum.
        _ => {
            let (first, second) = (random_gesture(1, seed), random_gesture(1, seed + 1000));
            let lift = first
                .iter()
                .find(|event| event.kind == Kind::Lift)
                .map_or(0., |e| e.at_ms);
            and_then(first, lift + between(5, 120., 400.), second)
        }
    }
}

/// For many gestures of each kind, how often the demo shows other Spaces than the app, and how
/// much later it first asks for one. `SWIPE_KIND=0` to `3` runs one kind, so the four can run
/// apart.
#[gpui::test]
#[ignore]
fn swipe_model_stats(cx: &mut TestAppContext) {
    let kinds = ["slow drag", "flick", "turn back", "two flicks"];
    let only = std::env::var("SWIPE_KIND")
        .ok()
        .and_then(|kind| kind.parse().ok());
    for (kind, name) in kinds.iter().enumerate() {
        if only.is_some_and(|only: usize| only != kind) {
            continue;
        }
        for delivery in deliveries() {
            let (mut wrong, mut lags, mut examples) = (0, Vec::new(), Vec::new());
            for seed in 0..60 {
                let events = random_gesture(kind, seed);
                let end = events.last().map_or(0., |event| event.at_ms) + 400.;
                let app = in_app(cx, &events, end);
                let demo = in_demo(cx, &events, delivery, end);
                if app.shown != demo.shown {
                    wrong += 1;
                    if examples.len() < 3 {
                        examples.push(format!("{seed}: {:?} vs {:?}", app.shown, demo.shown));
                    }
                }
                if let (Some(app), Some(demo)) = (app.first_shown_ms, demo.first_shown_ms) {
                    lags.push(demo - app);
                }
            }
            lags.sort_by(f64::total_cmp);
            let median = lags.get(lags.len() / 2).copied().unwrap_or(f64::NAN);
            let worst = lags.last().copied().unwrap_or(f64::NAN);
            let first = lags.first().copied().unwrap_or(f64::NAN);
            eprintln!(
                "{name:>10} {:>22}: {wrong:>2}/60 wrong, lag median {median:>5.0} ms, from {first:>5.0} to {worst:>5.0}  {}",
                delivery.name(),
                examples.join("; ")
            );
        }
    }
}

/// The site's live check's `swipe:x,y,dx,24`: 24 wheels 16 ms apart, the second half falling
/// away by `decay` an event, `dx` as the DOM has it (GPUI's is the other way).
fn live_swipe(dx: f32, decay: f32) -> Vec<f32> {
    (0..24)
        .map(|i| {
            if i < 12 {
                -dx
            } else {
                -dx * decay.powi(i - 11)
            }
        })
        .collect()
}

/// Wheels from `deltas`, meant 16 ms apart from `start`, as a page's timer sends them: each a few ms
/// late, or with chance `busy` up to 25 ms, and a seventh of those up to 25 + 200 × `busy` ms, as
/// timing `seed` draws it from `n0` on. Each is stamped when sent, or with `coalesce`, summed while
/// a frame held them.
fn timed(deltas: &[f32], start: f64, seed: u64, n0: u64, busy: f64, coalesce: bool) -> Vec<Wheel> {
    let mut wheels: Vec<Wheel> = Vec::new();
    let mut at = start;
    for (i, &dx) in deltas.iter().enumerate() {
        let n = n0 + i as u64 * 3;
        let roll = draw(seed, n);
        let late = if roll < busy * 0.15 {
            25. + 200. * busy * draw(seed, n + 1)
        } else if roll < busy {
            25. * draw(seed, n + 1)
        } else {
            3. * draw(seed, n + 1)
        };
        if coalesce {
            // Meant at a steady 16 ms, held by frames of the page's, summed into the next it gets.
            let meant = start + 16. * i as f64;
            at = at.max(meant) + if roll < busy { late } else { 0. };
            match wheels.last_mut() {
                Some(last) if last.0 >= meant => {
                    last.2 += dx;
                    last.1 = meant;
                }
                _ => wheels.push((at, meant, dx, 0.)),
            }
        } else {
            if i > 0 {
                at += 16. + late;
            }
            wheels.push((at, at, dx, 0.));
        }
    }
    wheels
}

/// The live check's swipe and its back swipe 900 ms later, in timing `seed` of a page's timer with
/// `busy` and `coalesce`: the Spaces each swipe showed.
fn live_check_pair(
    cx: &mut TestAppContext,
    prior: f32,
    decay: f32,
    busy: f64,
    coalesce: bool,
    seed: u64,
) -> [Vec<u64>; 2] {
    let first = timed(&live_swipe(prior, decay), 0., seed, 0, busy, coalesce);
    let gap = first.last().map_or(0., |last| last.0) + 900.;
    let second = timed(&live_swipe(-40., decay), gap, seed, 500, busy, coalesce);
    let end = second.last().map_or(gap, |last| last.0) + 1000.;
    let alone = in_demo_wheels(cx, first.clone(), gap).shown;
    let mut both = in_demo_wheels(cx, first.into_iter().chain(second).collect(), end).shown;
    let after = both.split_off(alone.len().min(both.len()));
    [both, after]
}

/// The site's live check, a swipe and then a back swipe 900 ms later, moves one Space each however
/// late a busy page's timer sends their wheels, and whether its frames sum them.
#[gpui::test]
fn a_swipe_never_moves_more_than_one_space(cx: &mut TestAppContext) {
    for prior in [40., -40.] {
        for (busy, coalesce) in [(0., false), (0.6, false), (0.6, true)] {
            for seed in 0..6 {
                let shown = live_check_pair(cx, prior, 0.8, busy, coalesce, seed);
                let way = format!("prior {prior}, busy {busy}, coalesce {coalesce}, seed {seed}");
                assert!(
                    shown.iter().all(|spaces| spaces.len() == 1),
                    "{way}: {shown:?}"
                );
            }
        }
    }
}

/// The same over many more timings, decays and pages busier still, `SWIPE_SEEDS` timings each: no
/// swipe moves more than one Space, though one held long enough may move none.
#[gpui::test]
#[ignore]
fn a_swipe_never_moves_more_than_one_space_in_many_timings(cx: &mut TestAppContext) {
    let seeds = std::env::var("SWIPE_SEEDS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(100);
    let mut doubles = 0;
    for prior in [40., -40.] {
        for decay in [0.7, 0.8, 0.85, 0.9] {
            for (busy, coalesce) in [
                (0., false),
                (0.3, false),
                (0.6, false),
                (0.9, false),
                (0.3, true),
                (0.6, true),
                (0.9, true),
            ] {
                let (mut more, mut none) = (Vec::new(), Vec::new());
                for seed in 0..seeds {
                    let shown = live_check_pair(cx, prior, decay, busy, coalesce, seed);
                    if shown.iter().any(|spaces| spaces.len() > 1) {
                        more.push(format!("{seed}: {shown:?}"));
                    } else if shown.iter().any(|spaces| spaces.is_empty()) {
                        none.push(seed);
                    }
                }
                eprintln!(
                    "prior {prior:>3} decay {decay:<4} busy {busy} coalesce {coalesce:>5}: more {more:?} none {none:?}"
                );
                doubles += more.len();
            }
        }
    }
    assert_eq!(doubles, 0, "swipes that moved more than one Space");
}

/// What the demo decided of a swipe: the trace's event it decided at, and the kind and what.
type Decision = (usize, String, String);

/// `trace`, as the demo's `?trace` saves it, through the demo from `space` of five, with what it
/// decided. A version 2 trace marks the events the demo took, and the rest went elsewhere on the
/// page; an earlier one gives them all.
fn replay(cx: &mut TestAppContext, trace: &serde_json::Value, space: u64) -> (Run, Vec<Decision>) {
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let marked = events.iter().any(|event| event.get("demo").is_some());
    let number = |event: &serde_json::Value, key: &str| event[key].as_f64().unwrap_or(0.);
    let first = events.first().map_or(0., |event| number(event, "heard"));
    let decided = Rc::new(RefCell::new(Vec::new()));
    let index = Rc::new(Cell::new(0));
    let (notes, at) = (decided.clone(), index.clone());
    let listen: Send = Box::new(move |shell, _, _, cx| {
        shell.set_space(space, cx);
        shell.settle = None;
        let (notes, at) = (notes.clone(), at.clone());
        // As the page's trace does, "wheel" only marks an event the demo took.
        shell.note_wheel(Rc::new(move |kind, what| {
            if kind != "wheel" {
                notes
                    .borrow_mut()
                    .push((at.get(), kind.into(), what.into()));
            }
        }));
    });
    let mut sends = vec![(0., listen)];
    for (i, event) in events.iter().enumerate() {
        if marked && event.get("demo").is_none() {
            continue;
        }
        let heard = number(event, "heard") - first;
        let stamped =
            Duration::from_secs_f64((heard + first - number(event, "stamp")).max(0.) / 1000.);
        // As gpui_web hands them on.
        let (dx, dy) = (-number(event, "x") as f32, -number(event, "y") as f32);
        let lines = number(event, "mode") == 1.;
        let index = index.clone();
        let send: Send = Box::new(move |shell, now, window, cx| {
            index.set(i);
            let mut event = scroll(TouchPhase::Moved, dx, dy);
            if lines {
                event.delta = ScrollDelta::Lines(point(dx, dy));
            }
            shell.web_scroll(&event, now - stamped, window, cx);
        });
        sends.push((heard, send));
    }
    let end = sends.last().map_or(0., |(heard, _)| *heard) + 1000.;
    let frames: Vec<f64> = trace["frames"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|frame| Some(frame.as_f64()? - first))
        .filter(|&frame| frame >= 0.)
        .collect();
    let run = run(cx, end, sends, &frames);
    let decided = decided.borrow().clone();
    (run, decided)
}

/// `WHEEL_TRACE=path cargo test --release --lib replay_wheel_trace -- --ignored --nocapture`
/// replays a trace the demo's `?trace` saved through the demo, and prints what it decides at each
/// event beside what the page decided, marking where they differ. The model decides as the page
/// did unless the page's events came at other times than they say, or the code has changed since.
/// Space moves are counted from where the swipe began. The replay starts on the Space
/// `WHEEL_TRACE_SPACE` names, 1 to 5 and the middle one by default, so give the one the page's
/// swipes began on, such as 1 for the site's first. The app can't be replayed: a browser's events
/// don't say where AppKit's phases fall.
#[gpui::test]
#[ignore]
fn replay_wheel_trace(cx: &mut TestAppContext) {
    let path = std::env::var("WHEEL_TRACE").expect("WHEEL_TRACE names a trace");
    let trace: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("the trace")).expect("JSON");
    let recorded: Vec<Decision> = trace["decisions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|decision| {
            let text = |key: &str| decision[key].as_str().unwrap_or_default().to_string();
            let event = decision["event"].as_u64().unwrap_or(0) as usize;
            (event, text("kind"), text("what"))
        })
        .collect();
    let space =
        std::env::var("WHEEL_TRACE_SPACE").map_or(3, |space| space.parse().expect("1 to 5"));
    let (run, replayed) = replay(cx, &trace, space);
    let frames: Vec<f64> = trace["frames"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|frame| frame.as_f64())
        .collect();
    let mut gaps: Vec<f64> = frames.windows(2).map(|pair| pair[1] - pair[0]).collect();
    gaps.sort_by(f64::total_cmp);
    println!(
        "{} events, {} frames, median frame {:.1} ms, version {}",
        trace["events"].as_array().map_or(0, Vec::len),
        frames.len(),
        gaps.get(gaps.len() / 2).copied().unwrap_or(0.),
        trace["version"].as_u64().unwrap_or(1),
    );
    let at = |decisions: &[Decision], event: usize| {
        let them = decisions.iter().filter(|decision| decision.0 == event);
        them.map(|(_, kind, what)| format!("{kind} {what}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut events: Vec<usize> = recorded.iter().chain(&replayed).map(|d| d.0).collect();
    events.sort_unstable();
    events.dedup();
    let mut differ = 0;
    for event in events {
        let (page, model) = (at(&recorded, event), at(&replayed, event));
        let same = page == model || recorded.is_empty();
        differ += usize::from(!same);
        let mark = if same { ' ' } else { '≠' };
        println!("{mark} event {event:>4}  page: {page:<40} model: {model}");
    }
    println!("the model showed Spaces {:?}", run.shown);
    if recorded.is_empty() {
        println!("the trace has no decisions to compare: it's from before version 2");
    } else {
        println!("{differ} events decided differently");
    }
}

/// A trace the demo's `?trace` would save of a quick flick across at 60 Hz, replayed, decides as
/// the demo does as it's swiped: across, then a lift, then the next Space.
#[gpui::test]
fn a_replayed_trace_tells_the_demos_decisions(cx: &mut TestAppContext) {
    let wheels = Delivery::Frames(60.).wheels(&flick(2));
    let events: Vec<serde_json::Value> = wheels
        .iter()
        .map(|&(heard, stamp, dx, dy)| {
            let (x, y) = (-dx, -dy);
            serde_json::json!({ "stamp": stamp, "heard": heard, "x": x, "y": y, "mode": 0 })
        })
        .collect();
    let trace = serde_json::json!({ "version": 2, "events": events });
    let (run, decided) = replay(cx, &trace, 3);
    let kinds: Vec<&str> = decided.iter().map(|(_, kind, _)| kind.as_str()).collect();
    let said = |kind: &str| decided.iter().find(|d| d.1 == kind).map(|d| d.2.as_str());
    assert_eq!(said("axis"), Some("across"), "{decided:?}");
    assert_eq!(said("space"), Some("+1"), "{decided:?}");
    let (land, axis, lift, space) = (
        kinds.iter().position(|&kind| kind == "land"),
        kinds.iter().position(|&kind| kind == "axis"),
        kinds.iter().position(|&kind| kind == "lift"),
        kinds.iter().position(|&kind| kind == "space"),
    );
    assert!(land < axis && axis < lift && lift < space, "{decided:?}");
    assert_eq!(run.shown, vec![4], "{decided:?}");
}

/// A trace of flicks that open the Library from the first Space, close it and go on to the next
/// Space, replayed with the frames the page drew, goes on to that Space as the demo does: the
/// Library at rest once closed, not still on its way (#2409).
#[gpui::test]
fn a_replayed_trace_swipes_on_after_the_library(cx: &mut TestAppContext) {
    let mut events = Vec::new();
    let mut from = 0.;
    // Toward the Library, back, then on.
    for way in [-1., 1., 1.] {
        for (heard, stamp, dx, dy) in Delivery::Frames(60.).wheels(&flick(2)) {
            let (x, y) = (-dx * way, -dy);
            let (heard, stamp) = (from + heard, from + stamp);
            events.push(
                serde_json::json!({ "stamp": stamp, "heard": heard, "x": x, "y": y, "mode": 0 }),
            );
        }
        from += 1500.;
    }
    let frames: Vec<f64> = (0..(from / (1000. / 60.)) as usize)
        .map(|frame| frame as f64 * 1000. / 60.)
        .collect();
    let trace = serde_json::json!({ "version": 2, "events": events, "frames": frames });
    let (run, decided) = replay(cx, &trace, 1);
    let said: Vec<&str> = decided
        .iter()
        .filter(|(_, kind, _)| kind == "library" || kind == "space")
        .map(|(_, _, what)| what.as_str())
        .collect();
    assert_eq!(said, ["open", "closed", "+1"], "{decided:?}");
    assert_eq!(run.shown, vec![2], "{decided:?}");
}
