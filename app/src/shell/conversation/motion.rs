//! What moves in a task's conversation: only what changes while it shows, each from where it
//! starts (chat-motion). Opening it, switching to it and relaunching show it as it is, at once.
//! Each change is kept with when it began, which the view reads as it draws, and goes once it has
//! run, so the view draws frames only while something moves.

use std::collections::HashMap;
use std::ops::Range;
use std::time::Duration;
use web_time::Instant;

use gpui::{Bounds, Pixels, SharedString};

use super::{Fold, markdown};
use crate::agents::task::{Conversation, Line, Opens, Trace, TraceItem, TraceStep};
use crate::shell::cubic_bezier;
use crate::tokens::shell::conversation::motion::{
    CHEVRON, LAND, PICK_FADE, PICTURE, RISE, STAGGER, STREAM, SWAP,
};
use crate::tokens::shell::task::motion::{
    CHIPS, CHIPS_DELAY, CHIPS_STAGGER, CROSSFADE, EASE, FADE, HEIGHT, ROLL, ROLL_RISE,
};

/// Runs of words that come within this of the last are one, so a run comes at most once a frame
/// on a 120 Hz display.
const FRAME: Duration = Duration::from_micros(8_333);
/// What the user sent waits this long for its line before it lands as any line would.
const SENT_WAIT: Duration = Duration::from_secs(2);

/// What rises or grows in, by what it is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Key {
    Line(usize),
    Step(usize),
    /// What the agent said along the way, by its trace's key and its place in it.
    Aside(usize, usize),
    Picture(u64),
    Answer(usize),
    Open,
}

/// A line whose words changed: the old ones, and since when.
#[derive(Clone, Debug)]
pub(super) struct Was {
    pub title: SharedString,
    pub note: SharedString,
    /// Whether it had a chevron, which otherwise comes with its new words.
    pub chevron: bool,
    pub since: Instant,
}

/// A stretch of an agent's message that fades in: from where in its blocks, and from when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Run {
    pub block: usize,
    pub char: usize,
    pub since: Instant,
}

/// A message's words as they came: how many characters each of its blocks had, and the runs still
/// fading in, in order.
#[derive(Clone, Debug, Default)]
struct Written {
    lengths: Vec<usize>,
    runs: Vec<Run>,
}

/// What the user sent, going from where they sent it to its plate in the lines.
#[derive(Clone, Debug)]
pub(super) struct Landing {
    pub text: SharedString,
    /// Where it was, in the window.
    pub from: Bounds<Pixels>,
    /// Whether it's an answer picked from its chip, whose plate it starts as.
    pub chip: bool,
    pub sent: Instant,
    /// Its line, and since when, once the conversation has it.
    pub line: Option<(usize, Instant)>,
}

/// A question's answers going as one is picked or the user replies: the row closes where it was,
/// before the line `at`, the picked one, if any, going on as the reply.
#[derive(Clone, Debug)]
pub(super) struct Answered {
    pub answers: Vec<SharedString>,
    pub picked: Option<usize>,
    pub at: usize,
    pub since: Instant,
}

/// The lines following what came at their end, from where they were, and where they were last set.
#[derive(Clone, Copy, Debug)]
pub(super) struct Follow {
    pub from: f32,
    pub since: Instant,
    pub set: f32,
}

#[derive(Default)]
pub(super) struct Motion {
    /// Whether what changes moves: once the conversation has drawn, and until it goes.
    pub live: bool,
    /// Whether motion is reduced, under which a trace folds at once and its answer doesn't wait.
    pub still: bool,
    came: HashMap<Key, Instant>,
    swaps: HashMap<Fold, Was>,
    /// When each step finished, which its note fades in after.
    notes: HashMap<usize, Instant>,
    /// Each trace's count of steps before it changed, by its key.
    counts: HashMap<usize, (SharedString, Instant)>,
    status: Option<(SharedString, Instant)>,
    stop: Option<Instant>,
    /// The traces folding as their turn ended, by their keys.
    folded: HashMap<usize, Instant>,
    written: HashMap<usize, Written>,
    pub landing: Option<Landing>,
    pub answered: Option<Answered>,
    pub follow: Option<Follow>,
}

/// How far a motion of `length` seconds from `since` is at `now`, 0 before it starts.
pub(super) fn linear(since: Instant, now: Instant, length: f32) -> f32 {
    match now.checked_duration_since(since) {
        None => 0.,
        Some(_) if length <= 0. => 1.,
        Some(gone) => (gone.as_secs_f32() / length).min(1.),
    }
}

/// How far a motion of `length` seconds from `since` is at `now`, on the task's ease.
pub(super) fn eased(since: Instant, now: Instant, length: f32) -> f32 {
    let [x1, y1, x2, y2] = EASE;
    cubic_bezier(x1, y1, x2, y2, linear(since, now, length))
}

/// Something rising `by` into its place over `length` from `since`, fading in as it does: its
/// opacity, and how far under its place it is. With motion reduced, it only fades.
pub(super) fn rise(
    since: Option<Instant>,
    now: Instant,
    length: f32,
    by: f32,
    still: bool,
) -> (f32, f32) {
    let Some(since) = since else { return (1., 0.) };
    let done = eased(since, now, length);
    (done, if still { 0. } else { by * (1. - done) })
}

/// New words rolling in from under old ones, which go up, from `since`: the old's opacity and how
/// far up they are, then the new's and how far under their place. With motion reduced, they
/// crossfade in place.
pub(super) fn roll(since: Instant, now: Instant, still: bool) -> [(f32, f32); 2] {
    let done = eased(since, now, ROLL);
    let by = if still { 0. } else { ROLL_RISE };
    [(1. - done, -by * done), (done, by * (1. - done))]
}

/// Words crossfading to others from `since`: the old fade over `swap`, then the new come over the
/// crossfade's rest. Whether the old still show, and the opacity of what shows.
pub(super) fn swap(since: Instant, now: Instant) -> (bool, f32) {
    let gone = now.saturating_duration_since(since).as_secs_f32();
    match gone < SWAP {
        true => (true, 1. - gone / SWAP),
        false => (false, ((gone - SWAP) / (CROSSFADE - SWAP)).min(1.)),
    }
}

/// How many characters each block of `text` shows, as its Markdown makes it.
pub(super) fn lengths(text: &str) -> Vec<usize> {
    (markdown::blocks(text).into_iter())
        .map(|block| match block {
            markdown::Block::Paragraph(inline) | markdown::Block::Item { text: inline, .. } => {
                inline.text.chars().count()
            }
        })
        .collect()
}

/// Where in the blocks of `now` the words are that `was` didn't have: the end of its last block if
/// that grew, else the start of the first block it didn't have.
fn grew(was: &[usize], now: &[usize]) -> Option<(usize, usize)> {
    let Some(&last) = was.last() else {
        return (!now.is_empty()).then_some((0, 0));
    };
    let block = was.len() - 1;
    match now.get(block) {
        Some(&length) if length > last => Some((block, last)),
        _ => (now.len() > was.len()).then_some((was.len(), 0)),
    }
}

/// A whole message's runs from `start`: a block each, `stagger` after the one before.
pub(super) fn whole(blocks: usize, start: Instant) -> Vec<Run> {
    (0..blocks)
        .map(|block| Run {
            block,
            char: 0,
            since: start + Duration::from_secs_f32(STAGGER * block as f32),
        })
        .collect()
}

/// The characters of block `block` of `chars` characters that are still fading in at `now` under
/// `runs`, and their opacity: a character takes the last run that starts at or before it.
pub(super) fn fading(
    runs: &[Run],
    block: usize,
    chars: usize,
    now: Instant,
) -> Vec<(Range<usize>, f32)> {
    let mut fading = Vec::new();
    // The run its first character is of, and where each run in it starts.
    let mut from = runs
        .iter()
        .rposition(|run| (run.block, run.char) <= (block, 0));
    let mut start = 0;
    let starts = runs
        .iter()
        .enumerate()
        .filter(|(_, run)| run.block == block && run.char > 0);
    for (index, run) in starts.chain(std::iter::once((
        usize::MAX,
        &Run {
            block,
            char: chars,
            since: now,
        },
    ))) {
        let end = run.char.min(chars);
        if let Some(of) = from.filter(|_| end > start) {
            let alpha = linear(runs[of].since, now, STREAM);
            if alpha < 1. {
                fading.push((start..end, alpha));
            }
        }
        (from, start) = (Some(index).filter(|&index| index != usize::MAX), end);
    }
    fading
}

/// Where a trace's steps are, each by its key, those of a turn's same step on several tabs too.
fn steps(trace: &Trace) -> impl Iterator<Item = &TraceStep> {
    (trace.items.iter())
        .filter_map(|item| match item {
            TraceItem::Step(step) => Some(step),
            TraceItem::Said(_) => None,
        })
        .flat_map(and_inner)
}

/// A step, and each tab's line it opens to, as a turn's same step on several tabs.
fn and_inner(step: &TraceStep) -> impl Iterator<Item = &TraceStep> {
    let inner = match &step.opens {
        Opens::Steps(steps) => steps.as_slice(),
        _ => &[],
    };
    std::iter::once(step).chain(inner)
}

/// The status at the end of what a conversation's header says it runs on: "Working".
pub(super) fn status(about: &str) -> &str {
    about.rsplit(" · ").next().unwrap_or(about)
}

impl Motion {
    /// The conversation changed from `old` to `new` while it showed, at `now`. A trace the user
    /// left `opened` or folded stays so as its turn ends.
    pub(super) fn changed(
        &mut self,
        old: &Conversation,
        new: &Conversation,
        opened: &HashMap<Fold, bool>,
        now: Instant,
    ) {
        if status(&old.about) != status(&new.about) {
            self.status = Some((status(&old.about).to_string().into(), now));
        }
        if old.stoppable != new.stoppable {
            self.stop = Some(now);
        }
        for (tab, _) in &new.tabs {
            if !old.tabs.iter().any(|(each, _)| each == tab) {
                self.came.insert(Key::Picture(*tab), now);
            }
        }
        // The answer comes once the trace above it has folded.
        let mut after = now;
        for (index, line) in new.lines.iter().enumerate() {
            match (old.lines.get(index), line) {
                (Some(was), line) if was == line => {}
                (Some(Line::Trace(was)), Line::Trace(trace)) if was.key == trace.key => {
                    let open = |trace: &Trace| {
                        opened
                            .get(&Fold::Trace(trace.key))
                            .copied()
                            .unwrap_or(trace.running)
                    };
                    let folds = open(was) && !open(trace);
                    let folded = self.trace_changed(was, trace, folds, now);
                    if let Some(folded) = folded.filter(|_| !self.still) {
                        after = after.max(folded);
                    }
                }
                (Some(Line::Step(was)), Line::Step(step)) if was.key == step.key => {
                    self.steps_changed(and_inner(was), and_inner(step), now);
                }
                (Some(Line::Thinking { key, failed: None }), Line::Trace(trace))
                    if *key == trace.key =>
                {
                    let was = Was {
                        title: "Thinking".into(),
                        note: "".into(),
                        chevron: false,
                        since: now,
                    };
                    self.swaps.insert(Fold::Trace(trace.key), was);
                    for step in steps(trace) {
                        self.came.insert(Key::Step(step.key), now);
                    }
                }
                (Some(Line::Said(was)), Line::Said(text)) if text.starts_with(was.as_str()) => {
                    self.wrote(index, text, now);
                }
                (_, Line::Said(text)) if new.writing && index + 1 == new.lines.len() => {
                    self.written.remove(&index);
                    self.wrote(index, text, now);
                }
                (_, Line::Said(text)) => {
                    let lengths = lengths(text);
                    let runs = whole(lengths.len(), after);
                    self.written.insert(index, Written { lengths, runs });
                }
                (_, Line::You(text)) => match &mut self.landing {
                    Some(landing) if landing.line.is_none() && landing.text == *text => {
                        landing.line = Some((index, now));
                    }
                    _ => _ = self.came.insert(Key::Line(index), now),
                },
                _ => _ = self.came.insert(Key::Line(index), now),
            }
        }
        // What the agent wrote under its trace goes into it as it calls a tool, where it showed.
        if let Some(Line::Said(written)) = old
            .lines
            .last()
            .filter(|_| new.lines.len() < old.lines.len())
        {
            for line in &new.lines {
                let Line::Trace(trace) = line else { continue };
                for (index, item) in trace.items.iter().enumerate() {
                    if matches!(item, TraceItem::Said(said) if said == written) {
                        self.came.remove(&Key::Aside(trace.key, index));
                    }
                }
            }
        }
        if old.answers != new.answers && !new.answers.is_empty() {
            for index in 0..new.answers.len() {
                let delay = CHIPS_DELAY + CHIPS_STAGGER * index as f32;
                self.came
                    .insert(Key::Answer(index), now + Duration::from_secs_f32(delay));
            }
        }
        if !old.answers.is_empty() && new.answers.is_empty() {
            let picked = (self.answered.take()).filter(|answered| answered.answers == old.answers);
            self.answered = Some(Answered {
                answers: old.answers.clone(),
                picked: picked.and_then(|answered| answered.picked),
                at: old.lines.len().min(new.lines.len()),
                since: now,
            });
        }
        if old.open.is_none() && new.open.is_some() {
            self.came
                .insert(Key::Open, now + Duration::from_secs_f32(CHIPS_DELAY));
        }
    }

    /// A trace of the conversation changed: what came in it rises, a step that finished says so,
    /// its count rolls, and as its turn ends it `folds`. Returns when it will have folded.
    fn trace_changed(
        &mut self,
        was: &Trace,
        trace: &Trace,
        folds: bool,
        now: Instant,
    ) -> Option<Instant> {
        self.steps_changed(steps(was), steps(trace), now);
        for (index, item) in trace.items.iter().enumerate() {
            let new = !matches!(was.items.get(index), Some(TraceItem::Said(_)));
            if matches!(item, TraceItem::Said(_)) && new {
                self.came.insert(Key::Aside(trace.key, index), now);
            }
        }
        if was.steps() != trace.steps() && trace.took.is_none() {
            self.counts
                .insert(trace.key, (was.steps().to_string().into(), now));
        }
        if was.took != trace.took {
            let (title, note) = was.summary();
            self.swaps.insert(
                Fold::Trace(trace.key),
                Was {
                    title,
                    note,
                    chevron: true,
                    since: now,
                },
            );
        }
        if !folds {
            return None;
        }
        self.folded.insert(trace.key, now);
        Some(now + Duration::from_secs_f32(HEIGHT))
    }

    /// Steps changed from `was`: one that came rises, and one that finished says so, its title
    /// crossfading to its past tense, then its note fading in.
    fn steps_changed<'a>(
        &mut self,
        was: impl Iterator<Item = &'a TraceStep>,
        steps: impl Iterator<Item = &'a TraceStep>,
        now: Instant,
    ) {
        let before: HashMap<usize, &TraceStep> = was.map(|step| (step.key, step)).collect();
        for step in steps {
            match before.get(&step.key) {
                None => _ = self.came.insert(Key::Step(step.key), now),
                Some(old) if old.title != step.title => {
                    let was = Was {
                        title: old.title.clone(),
                        note: old.description.clone(),
                        chevron: true,
                        since: now,
                    };
                    self.swaps.insert(Fold::Step(step.key), was);
                    if old.description != step.description {
                        self.notes.insert(step.key, now);
                    }
                }
                Some(old) if old.description != step.description => {
                    _ = self.notes.insert(step.key, now)
                }
                _ => {}
            }
        }
    }

    /// Message `line` grew to `text` as it's written: what it didn't have fades in, as one run
    /// with what came within the frame.
    fn wrote(&mut self, line: usize, text: &str, now: Instant) {
        let written = self.written.entry(line).or_default();
        let lengths = lengths(text);
        if let Some((block, char)) = grew(&written.lengths, &lengths) {
            let last = written.runs.last().map(|run| run.since);
            if !last.is_some_and(|last| now.saturating_duration_since(last) < FRAME) {
                written.runs.push(Run {
                    block,
                    char,
                    since: now,
                });
            }
        }
        written.lengths = lengths;
    }

    /// When `key` came or comes, while it's still coming in.
    pub(super) fn came(&self, key: Key) -> Option<Instant> {
        self.came.get(&key).copied()
    }

    pub(super) fn swapped(&self, fold: Fold) -> Option<&Was> {
        self.swaps.get(&fold)
    }

    /// The opacity of step `key`'s note, which fades in once its title has crossfaded.
    pub(super) fn note(&self, key: usize, now: Instant) -> f32 {
        let Some(&since) = self.notes.get(&key) else {
            return 1.;
        };
        eased(since + Duration::from_secs_f32(SWAP), now, FADE)
    }

    pub(super) fn count(&self, trace: usize) -> Option<&(SharedString, Instant)> {
        self.counts.get(&trace)
    }

    pub(super) fn status(&self) -> Option<&(SharedString, Instant)> {
        self.status.as_ref()
    }

    /// Stop's opacity as it comes, if it's showing, or goes.
    pub(super) fn stop(&self, showing: bool, now: Instant) -> f32 {
        let done = self.stop.map_or(1., |since| eased(since, now, RISE));
        if showing { done } else { 1. - done }
    }

    /// Since when trace `key` has folded as its turn ended, while it does.
    pub(super) fn folded(&self, key: usize) -> Option<Instant> {
        self.folded.get(&key).copied()
    }

    /// The runs of message `line` still fading in.
    pub(super) fn runs(&self, line: usize) -> &[Run] {
        self.written.get(&line).map_or(&[], |written| &written.runs)
    }

    /// Lets go of what has run, and says whether anything still moves.
    pub(super) fn settle(&mut self, now: Instant) -> bool {
        let over = |since: Instant, length: f32| now >= since + Duration::from_secs_f32(length);
        self.came.retain(|key, since| {
            let length = match key {
                Key::Picture(_) => PICTURE,
                Key::Answer(_) | Key::Open => CHIPS,
                _ => RISE,
            };
            !over(*since, length)
        });
        self.swaps.retain(|_, was| !over(was.since, CROSSFADE));
        self.notes.retain(|_, since| !over(*since, SWAP + FADE));
        self.counts.retain(|_, (_, since)| !over(*since, ROLL));
        self.status = self.status.take().filter(|(_, since)| !over(*since, ROLL));
        self.stop = self.stop.filter(|since| !over(*since, RISE));
        self.folded
            .retain(|_, since| !over(*since, HEIGHT.max(CHEVRON)));
        for written in self.written.values_mut() {
            let done = written
                .runs
                .iter()
                .take_while(|run| over(run.since, STREAM))
                .count();
            written.runs.drain(..done);
        }
        self.landing = self.landing.take().filter(|landing| match landing.line {
            Some((_, since)) => !over(since, LAND),
            None => now < landing.sent + SENT_WAIT,
        });
        self.answered = self
            .answered
            .take()
            .filter(|answered| !over(answered.since, LAND.max(PICK_FADE)));
        let writing = self
            .written
            .values()
            .any(|written| !written.runs.is_empty());
        !self.came.is_empty()
            || !self.swaps.is_empty()
            || !self.notes.is_empty()
            || !self.counts.is_empty()
            || self.status.is_some()
            || self.stop.is_some()
            || !self.folded.is_empty()
            || writing
            || self
                .landing
                .as_ref()
                .is_some_and(|landing| landing.line.is_some())
            || self.answered.is_some()
            || self.follow.is_some()
    }

    /// The conversation stopped showing, or shows another's: nothing it had moves any more, and
    /// what changes next shows at once.
    pub(super) fn reset(&mut self) {
        *self = Motion {
            still: self.still,
            ..Motion::default()
        };
    }
}

#[cfg(test)]
mod tests;
