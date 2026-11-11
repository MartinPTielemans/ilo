//! One physical two-finger swipe, given to the app as AppKit's phased events and to the site's
//! demo as a browser's wheel events for the same fingers, to compare where each puts the page.
//! `SWIPE_MODEL_CSV=path cargo test --release --lib write_swipe_model -- --ignored` writes the
//! page's place over time for each, to plot, and `swipe_model_stats` tries many gestures. The
//! trackpad's rate, its noise and AppKit's momentum are assumed; the demo's `?trace=1` records a
//! real swipe's wheel events to check them against.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    Entity, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
};

use crate::shell::{SWIPE_TRACKING, Shell, recorder::Recorder, wheel_gestures::{GAP, LONG}};

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
    kind: Kind,
}

/// A swipe whose fingers go at `speed(t)` points a millisecond for `lift_ms`, then lift into
/// AppKit's momentum at the speed they lifted at. A trackpad's own events come a little unevenly,
/// and its fingers' deltas by a quarter either way, from `seed`; the momentum's are smooth.
fn gesture(lift_ms: f64, seed: u64, speed: impl Fn(f64) -> f64) -> Vec<Event> {
    let mut random = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut noise = move || {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (random >> 33) as f64 / (1u64 << 31) as f64 * 2. - 1.
    };
    let mut events = vec![Event { at_ms: 0., dx: 0., kind: Kind::Down }];
    let mut at = 0.;
    while at + EVENT_MS <= lift_ms {
        let next = (at + EVENT_MS + 1.5 * noise()).min(lift_ms);
        // The way since the last event, by the midpoint's speed.
        let dx = speed((at + next) / 2.) * (next - at) * (1. + 0.25 * noise());
        at = next;
        events.push(Event { at_ms: at, dx: dx as f32, kind: Kind::Fingers });
    }
    events.push(Event { at_ms: lift_ms, dx: 0., kind: Kind::Lift });
    let lifted = speed(lift_ms);
    let mut t = 0.;
    loop {
        t += EVENT_MS;
        let dx = lifted * EVENT_MS * (-t / MOMENTUM_TAU_MS).exp();
        if dx.abs() < 0.1 {
            break;
        }
        events.push(Event { at_ms: lift_ms + t, dx: dx as f32, kind: Kind::Momentum });
    }
    events.push(Event { at_ms: lift_ms + t, dx: 0., kind: Kind::MomentumEnd });
    events
}

/// `first`, its momentum stopped by fingers coming down again at `touch_ms`, and `then` from there.
fn and_then(first: Vec<Event>, touch_ms: f64, then: Vec<Event>) -> Vec<Event> {
    let before = first.into_iter().filter(|event| event.at_ms < touch_ms);
    let after = then.into_iter().map(|event| Event { at_ms: event.at_ms + touch_ms, ..event });
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
        ("slow drag and let go", gesture(700., 1, |t| {
            -ramp(t, 0., 150., 0., 0.3) + ramp(t, 450., 700., 0., 0.2)
        })),
        ("short flick", flick(2)),
        // Toward the next Space, then back past where it began, lifting on the way back.
        ("drag that reverses", gesture(800., 3, |t| {
            -ramp(t, 0., 120., 0., 0.35) + ramp(t, 300., 520., 0., 0.65)
        })),
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
    /// summed into it's, and its delta.
    fn wheels(self, events: &[Event]) -> Vec<(f64, f64, f32)> {
        let moving = events.iter().filter(|event| event.dx != 0.);
        match self {
            Delivery::Direct => moving.map(|event| (event.at_ms, event.at_ms, event.dx)).collect(),
            Delivery::Whole => moving
                .map(|event| (event.at_ms, event.at_ms, event.dx.round()))
                .filter(|&(_, _, dx)| dx != 0.)
                .collect(),
            Delivery::Frames(hz) => {
                let frame = 1000. / hz;
                let mut wheels: Vec<(f64, f64, f32)> = Vec::new();
                for event in moving {
                    // Frames a third of one out of step with the trackpad's events.
                    let at = ((event.at_ms - frame / 3.) / frame).ceil() * frame + frame / 3.;
                    match wheels.last_mut() {
                        Some((last, stamp, dx)) if *last == at => {
                            *stamp = event.at_ms;
                            *dx += event.dx;
                        }
                        _ => wheels.push((at, event.at_ms, event.dx)),
                    }
                }
                wheels
            }
        }
    }
}

/// Five Spaces, showing the third, as the core leaves them.
fn five_spaces(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space =
            |id: u64| (id, format!("Space {id}").into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces((1..=5).map(space).collect(), cx);
        shell.set_space(3, cx);
        shell.settle = None;
    });
    cx.run_until_parked();
    (shell, host, cx)
}

/// Where the page is at `now`, in Spaces from the first's place: 2 has the third in its place.
fn slide(shell: &Shell, now: Instant) -> f32 {
    let index = |id| shell.spaces.iter().position(|space| space.id == id).unwrap_or(0) as f32;
    let width = shell.width();
    if let Some(swipe) = shell.swipe.as_ref().filter(|swipe| swipe.across == Some(true)) {
        return index(swipe.from) - shell.swiped_to(swipe) / width;
    }
    match &shell.settle {
        Some(settle) => index(settle.space) - settle.offset_at(now).unwrap_or(0.) / width,
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
    shown: Vec<u64>,
    /// When the shell first asked for another Space.
    first_shown_ms: Option<f64>,
}

/// The fingers' and momentum's events, each `(ms, what)` as `send` gives it to the shell, which
/// shows a Space as soon as it asks to, as the core does, with the page's place each step.
fn run(
    cx: &mut TestAppContext,
    end_ms: f64,
    mut events: Vec<(f64, Send)>,
) -> Run {
    let (shell, host, cx) = five_spaces(cx);
    let start = Instant::now();
    let at = |ms: f64| start + Duration::from_secs_f64(ms / 1000.);
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut events = events.into_iter().peekable();
    let mut slides = Vec::new();
    let mut first_shown_ms = None;
    let mut ms = 0.;
    while ms <= end_ms {
        while let Some((when, send)) = events.next_if(|(when, _)| *when <= ms) {
            let now = at(when);
            shell.update_in(cx, |shell, window, cx| send(shell, now, window, cx));
            let asked = host.shown.borrow().last().copied();
            if asked.is_some() && first_shown_ms.is_none() {
                first_shown_ms = Some(when);
            }
            shell.update(cx, |shell, cx| {
                if let Some(space) = asked.filter(|&space| space != shell.space) {
                    shell.set_space(space, cx);
                }
            });
        }
        slides.push(shell.read_with(cx, |shell, _| slide(shell, at(ms))));
        ms += STEP_MS;
    }
    let shown = host.shown.borrow().clone();
    Run { slides, shown, first_shown_ms }
}

/// An event over the sidebar, `dx` across as GPUI has it.
fn scroll(touch_phase: TouchPhase, dx: f32) -> ScrollEvent {
    ScrollEvent {
        position: point(px(110.), px(500.)),
        delta: ScrollDelta::Pixels(point(px(dx), px(0.))),
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
                && events.iter().find(|other| other.kind == Kind::Fingers).map(|e| e.at_ms)
                    == Some(event.at_ms);
            let phase = if first { TouchPhase::Started } else { phase };
            let send: Send = Box::new(move |shell, now, window, cx| {
                    shell.scroll_at(&scroll(phase, event.dx), now, window, cx);
                });
            (event.at_ms, send)
        })
        .collect();
    run(cx, end_ms, sends)
}

/// The site's demo: the browser's wheel events, each at its stamp, and the quiet timer after each.
fn in_demo(cx: &mut TestAppContext, events: &[Event], delivery: Delivery, end_ms: f64) -> Run {
    in_demo_wheels(cx, delivery.wheels(events), end_ms)
}

/// The site's demo given `wheels`, each `(ms it comes, ms its stamp says, dx)`.
fn in_demo_wheels(cx: &mut TestAppContext, wheels: Vec<(f64, f64, f32)>, end_ms: f64) -> Run {
    let gap = GAP.as_secs_f64() * 1000.;
    let mut sends: Vec<(f64, Send)> = Vec::new();
    for (i, &(at_ms, stamp_ms, dx)) in wheels.iter().enumerate() {
        let stamped = Duration::from_secs_f64((at_ms - stamp_ms) / 1000.);
        sends.push((at_ms, Box::new(move |shell, now, window, cx| {
            shell.web_scroll(&scroll(TouchPhase::Moved, dx), now - stamped, window, cx);
        })));
        let next = wheels.get(i + 1).map_or(f64::INFINITY, |next| next.0);
        for wait in [gap, LONG.as_secs_f64() * 1000.] {
            if at_ms + wait < next {
                sends.push((at_ms + wait, Box::new(|shell, now, window, cx| {
                    shell.web_quiet(&scroll(TouchPhase::Ended, 0.), now, window, cx);
                })));
            }
        }
    }
    run(cx, end_ms, sends)
}

/// The deliveries the demo is compared in.
fn deliveries() -> [Delivery; 4] {
    [Delivery::Direct, Delivery::Whole, Delivery::Frames(60.), Delivery::Frames(120.)]
}

/// Each gesture, in each way a browser may hand it on, shows the Spaces in the demo that it does
/// in the app, starting no sooner than the fingers lift there and soon after.
#[gpui::test]
fn the_demo_swipes_as_the_app_does(cx: &mut TestAppContext) {
    for (name, events) in gestures() {
        let end = events.last().map_or(0., |event| event.at_ms) + 400.;
        let app = in_app(cx, &events, end);
        for delivery in deliveries() {
            let demo = in_demo(cx, &events, delivery, end);
            let way = format!("{name}, {}", delivery.name());
            assert_eq!(demo.shown, app.shown, "{way}: the Spaces shown");
            let lag = demo.first_shown_ms.zip(app.first_shown_ms).map(|(demo, app)| demo - app);
            assert!(lag.is_some_and(|lag| (0. ..300.).contains(&lag)), "{way}: {lag:?} ms after the app");
        }
    }
}

/// Writes, for each gesture, the fingers' way and the page's place in the app and in the demo
/// for each delivery, as `gesture,path,ms,spaces`.
#[gpui::test]
#[ignore]
fn write_swipe_model(cx: &mut TestAppContext) {
    use std::fmt::Write;
    let Ok(path) = std::env::var("SWIPE_MODEL_CSV") else { return };
    let mut csv = String::from("gesture,path,ms,spaces\n");
    for (name, events) in gestures() {
        let end_ms = 1400.;
        let width = {
            let (shell, _, window) = five_spaces(cx);
            shell.read_with(window, |shell, _| shell.width())
        };
        let mut way = 0.;
        let mut fingers = events.iter().filter(|e| matches!(e.kind, Kind::Fingers)).peekable();
        let mut ms = 0.;
        while ms <= end_ms {
            while let Some(event) = fingers.next_if(|event| event.at_ms <= ms) {
                way += event.dx;
            }
            writeln!(csv, "{name},fingers,{ms},{}", 2. - SWIPE_TRACKING * way / width).unwrap();
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
            write(&format!("demo: {}", delivery.name()), in_demo(cx, &events, delivery, end_ms));
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
            let lift = first.iter().find(|event| event.kind == Kind::Lift).map_or(0., |e| e.at_ms);
            and_then(first, lift + between(5, 120., 400.), second)
        }
    }
}

/// For many gestures of each kind, how often the demo shows other Spaces than the app, and how
/// much later it first asks for one.
#[gpui::test]
#[ignore]
fn swipe_model_stats(cx: &mut TestAppContext) {
    let kinds = ["slow drag", "flick", "turn back", "two flicks"];
    for (kind, name) in kinds.iter().enumerate() {
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
    (0..24).map(|i| if i < 12 { -dx } else { -dx * decay.powi(i - 11) }).collect()
}

/// Wheels from `deltas`, meant 16 ms apart from `start`, as a page's timer sends them: each a few ms
/// late, or with chance `busy` up to 25 ms, and a seventh of those up to 25 + 200 × `busy` ms, as
/// timing `seed` draws it from `n0` on. Each is stamped when sent, or with `coalesce`, summed while
/// a frame held them.
fn timed(deltas: &[f32], start: f64, seed: u64, n0: u64, busy: f64, coalesce: bool) -> Vec<(f64, f64, f32)> {
    let mut wheels: Vec<(f64, f64, f32)> = Vec::new();
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
                _ => wheels.push((at, meant, dx)),
            }
        } else {
            if i > 0 {
                at += 16. + late;
            }
            wheels.push((at, at, dx));
        }
    }
    wheels
}

/// The live check's swipe and its back swipe 900 ms later, in timing `seed` of a page's timer with
/// `busy` and `coalesce`: the Spaces each swipe showed.
fn live_check_pair(cx: &mut TestAppContext, prior: f32, decay: f32, busy: f64, coalesce: bool, seed: u64) -> [Vec<u64>; 2] {
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
                assert!(shown.iter().all(|spaces| spaces.len() == 1), "{way}: {shown:?}");
            }
        }
    }
}

/// The same over many more timings, decays and pages busier still, `SWIPE_SEEDS` timings each: no
/// swipe moves more than one Space, though one held long enough may move none.
#[gpui::test]
#[ignore]
fn a_swipe_never_moves_more_than_one_space_in_many_timings(cx: &mut TestAppContext) {
    let seeds = std::env::var("SWIPE_SEEDS").ok().and_then(|n| n.parse().ok()).unwrap_or(100);
    let mut doubles = 0;
    for prior in [40., -40.] {
        for decay in [0.7, 0.8, 0.85, 0.9] {
            for (busy, coalesce) in [(0., false), (0.3, false), (0.6, false), (0.9, false), (0.3, true), (0.6, true), (0.9, true)] {
                let (mut more, mut none) = (Vec::new(), Vec::new());
                for seed in 0..seeds {
                    let shown = live_check_pair(cx, prior, decay, busy, coalesce, seed);
                    if shown.iter().any(|spaces| spaces.len() > 1) {
                        more.push(format!("{seed}: {shown:?}"));
                    } else if shown.iter().any(|spaces| spaces.is_empty()) {
                        none.push(seed);
                    }
                }
                eprintln!("prior {prior:>3} decay {decay:<4} busy {busy} coalesce {coalesce:>5}: more {more:?} none {none:?}");
                doubles += more.len();
            }
        }
    }
    assert_eq!(doubles, 0, "swipes that moved more than one Space");
}
