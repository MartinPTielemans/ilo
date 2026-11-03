//! What moves in a conversation as it changes, and when, from the changes alone.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::*;

fn at(start: Instant, seconds: f32) -> Instant {
    start + Duration::from_secs_f32(seconds)
}

fn close(a: Instant, b: Instant) -> bool {
    a.max(b).duration_since(a.min(b)) < Duration::from_millis(1)
}

fn step(key: usize, title: &str, running: bool) -> TraceStep {
    TraceStep {
        key,
        glyph: "icons/globe.svg",
        title: title.to_string().into(),
        description: "".into(),
        running,
        failed: false,
        opens: Opens::Nothing,
    }
}

fn trace(running: bool, steps: usize) -> Line {
    let items = (0..steps)
        .map(|key| TraceItem::Step(step(key, "Opened maat.pt", false)))
        .collect();
    Line::Trace(Trace {
        key: 0,
        running,
        took: (!running).then_some(4),
        items,
    })
}

fn conversation(lines: Vec<Line>) -> Conversation {
    Conversation {
        agent: "task-1".into(),
        name: "Claude",
        title: "Museums in Lisbon".into(),
        about: "Claude · Haiku · Working".into(),
        asked: "what's on at maat".into(),
        page: None,
        lines,
        tabs: Vec::new(),
        on: None,
        stoppable: true,
        open: None,
        answers: Vec::new(),
        writing: false,
    }
}

#[test]
fn under_reduce_motion_what_comes_fades_in_its_place() {
    let start = Instant::now();
    let halfway = at(start, 0.1);
    let (opacity, by) = rise(Some(start), halfway, 0.2, 4., true);
    assert!(opacity > 0. && opacity < 1., "it still fades: {opacity}");
    assert_eq!(by, 0.);
    let [(old, old_by), (new, new_by)] = roll(start, halfway, true);
    assert!(
        old > 0. && new > 0.,
        "the old and new crossfade: {old}, {new}"
    );
    assert_eq!((old_by, new_by), (0., 0.));

    let (_, by) = rise(Some(start), halfway, 0.2, 4., false);
    assert!(by > 0. && by < 4., "with motion it rises: {by}");
    let [(_, old_by), (_, new_by)] = roll(start, halfway, false);
    assert!(
        old_by < 0. && new_by > 0.,
        "the old go up as the new come from under: {old_by}, {new_by}"
    );
}

#[test]
fn a_whole_message_comes_a_block_at_a_time_fifty_ms_apart() {
    let start = Instant::now();
    let runs = whole(3, start);
    let starts: Vec<_> = runs.iter().map(|run| (run.block, run.char)).collect();
    assert_eq!(starts, [(0, 0), (1, 0), (2, 0)]);
    assert!(close(runs[1].since, at(start, 0.05)) && close(runs[2].since, at(start, 0.1)));
}

#[test]
fn a_turns_answer_comes_once_its_trace_has_folded() {
    let start = Instant::now();
    let mut motion = Motion {
        live: true,
        ..Motion::default()
    };
    let old = conversation(vec![Line::You("what's on".into()), trace(true, 2)]);
    let answer = "Two shows.\n\nBoth close at six.".into();
    let new = Conversation {
        about: "Claude · Haiku · Done 13:31".into(),
        stoppable: false,
        ..conversation(vec![
            Line::You("what's on".into()),
            trace(false, 2),
            Line::Said(answer),
        ])
    };
    motion.changed(&old, &new, &HashMap::new(), start);

    assert_eq!(
        motion.folded(0),
        Some(start),
        "the trace folds as its turn ends"
    );
    let runs = motion.runs(2);
    assert_eq!(runs.len(), 2, "a run for each of its blocks");
    assert!(
        close(runs[0].since, at(start, HEIGHT)),
        "it starts once the height has closed"
    );
    assert!(close(runs[1].since, at(start, HEIGHT + STAGGER)));
    assert!(
        motion
            .swapped(Fold::Trace(0))
            .is_some_and(|was| was.title == "Working")
    );
    assert!(motion.status().is_some_and(|(was, _)| was == "Working"));
}

#[test]
fn under_reduce_motion_a_turns_answer_doesnt_wait_for_the_fold() {
    let start = Instant::now();
    let mut motion = Motion {
        live: true,
        still: true,
        ..Motion::default()
    };
    let old = conversation(vec![trace(true, 1)]);
    let new = conversation(vec![trace(false, 1), Line::Said("Done.".into())]);
    motion.changed(&old, &new, &HashMap::new(), start);
    assert!(close(motion.runs(1)[0].since, start));
}

#[test]
fn a_trace_the_user_opened_stays_open_and_its_answer_comes_at_once() {
    let start = Instant::now();
    let mut motion = Motion {
        live: true,
        ..Motion::default()
    };
    let old = conversation(vec![trace(true, 1)]);
    let new = conversation(vec![trace(false, 1), Line::Said("Done.".into())]);
    let opened = HashMap::from([(Fold::Trace(0), true)]);
    motion.changed(&old, &new, &opened, start);
    assert_eq!(motion.folded(0), None);
    assert!(close(motion.runs(1)[0].since, start));
}

#[test]
fn what_is_written_fades_in_only_where_it_grew_at_most_once_a_frame() {
    let start = Instant::now();
    let mut motion = Motion {
        live: true,
        ..Motion::default()
    };
    let line = |text: &str| Conversation {
        writing: true,
        ..conversation(vec![trace(true, 1), Line::Said(text.to_string().into())])
    };
    motion.changed(&line(""), &line("Two"), &HashMap::new(), start);
    motion.changed(
        &line("Two"),
        &line("Two shows"),
        &HashMap::new(),
        at(start, 0.002),
    );
    assert_eq!(
        motion.runs(1).len(),
        1,
        "what came within the frame joins the run before"
    );
    motion.changed(
        &line("Two shows"),
        &line("Two shows are on"),
        &HashMap::new(),
        at(start, 0.03),
    );
    let runs = motion.runs(1);
    assert_eq!(
        runs.iter()
            .map(|run| (run.block, run.char))
            .collect::<Vec<_>>(),
        [(0, 0), (0, 9)]
    );
    motion.changed(
        &line("Two shows are on"),
        &line("Two shows are on\n\nBoth"),
        &HashMap::new(),
        at(start, 0.06),
    );
    assert_eq!(
        motion.runs(1).last().map(|run| (run.block, run.char)),
        Some((1, 0)),
        "a new block starts its own"
    );

    // Once a run has faded in, it goes, and only the newer still fade.
    motion.settle(at(start, 0.03 + STREAM + 0.005));
    assert_eq!(motion.runs(1).len(), 1);
}

#[test]
fn a_character_fades_with_the_run_it_came_in() {
    let start = Instant::now();
    let runs = [
        Run {
            block: 0,
            char: 0,
            since: start,
        },
        Run {
            block: 0,
            char: 4,
            since: at(start, 0.1),
        },
    ];
    let now = fading(&runs, 0, 9, at(start, 0.12));
    assert_eq!(now.len(), 2);
    assert_eq!(now[0].0, 0..4);
    assert_eq!(now[1].0, 4..9);
    assert!(now[0].1 > now[1].1, "the older run is further in");
    // Once the first has come, only the second fades.
    let later = fading(&runs, 0, 9, at(start, STREAM + 0.01));
    assert_eq!(
        later
            .iter()
            .map(|(range, _)| range.clone())
            .collect::<Vec<_>>(),
        [4..9]
    );
    // A block before the runs shows as it is.
    assert!(
        fading(&runs[1..], 0, 9, at(start, 0.12))
            .iter()
            .all(|(range, _)| range.start >= 4)
    );
}

#[test]
fn what_was_written_below_the_trace_goes_into_it_without_rising_again() {
    let start = Instant::now();
    let mut motion = Motion {
        live: true,
        ..Motion::default()
    };
    let old = Conversation {
        writing: false,
        ..conversation(vec![trace(true, 1), Line::Said("Looking at maat.".into())])
    };
    let mut new = conversation(vec![trace(true, 2)]);
    if let Line::Trace(trace) = &mut new.lines[0] {
        trace
            .items
            .insert(1, TraceItem::Said("Looking at maat.".into()));
    }
    motion.changed(&old, &new, &HashMap::new(), start);
    assert_eq!(motion.came(Key::Aside(0, 1)), None);
    assert!(
        motion.came(Key::Step(1)).is_some(),
        "the step after it rises in"
    );
}

#[test]
fn thinking_becomes_the_turns_trace_and_its_steps_rise_in() {
    let start = Instant::now();
    let mut motion = Motion {
        live: true,
        ..Motion::default()
    };
    let old = conversation(vec![
        Line::You("go".into()),
        Line::Thinking {
            key: 0,
            failed: None,
        },
    ]);
    let new = conversation(vec![Line::You("go".into()), trace(true, 1)]);
    motion.changed(&old, &new, &HashMap::new(), start);
    let was = motion
        .swapped(Fold::Trace(0))
        .expect("it crossfades from Thinking");
    assert_eq!((was.title.as_ref(), was.chevron), ("Thinking", false));
    assert_eq!(motion.came(Key::Step(0)), Some(start));
    assert!(motion.settle(start), "it moves");
    assert!(
        !motion.settle(at(start, 1.)),
        "and stops, so no frames are drawn after"
    );
}

#[test]
fn chips_rise_in_one_after_another() {
    let start = Instant::now();
    let mut motion = Motion {
        live: true,
        ..Motion::default()
    };
    let old = conversation(vec![trace(true, 1)]);
    let new = Conversation {
        answers: vec!["Friday".into(), "Saturday".into()],
        ..conversation(vec![trace(true, 1)])
    };
    motion.changed(&old, &new, &HashMap::new(), start);
    assert_eq!(
        motion
            .came(Key::Answer(0))
            .map(|since| close(since, at(start, CHIPS_DELAY))),
        Some(true)
    );
    let second = at(start, CHIPS_DELAY + CHIPS_STAGGER);
    assert_eq!(
        motion
            .came(Key::Answer(1))
            .map(|since| close(since, second)),
        Some(true)
    );
}
