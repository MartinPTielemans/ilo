//! The window's agent tasks at the sidebar's foot, over its media players (Agent task): a plate
//! each, of a player's shape and fill, oldest lowest, so the list's rows above never move as tasks
//! come and go. The favicon's column holds the state's glyph; the title is a row's, over a status
//! line where each step rolls in, the agent's question, or its result's line; its chips sit under
//! that: the answers, Retry, Copy or Open. ✕ shows under the pointer. Three show, waiting first,
//! then working, then done, and a fourth plate says how many more there are and opens the foot to
//! all of them. While ⌘T asks for a task, the foot holds an empty slot a plate tall, which the bar
//! folds into and the card fills. The foot is the window's, as the players are, so switching Spaces
//! never hides a task. It draws frames only while something on it moves: its plates' motion on the
//! display's frames, and a working task's circling dot and shimmer on the loading dots' clock, so a
//! foot at rest draws nothing.

mod glyph;
mod motion;
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;
use web_time::Instant;

use gpui::{
    AnyElement, Bounds, Context, FontWeight, HighlightStyle, Hsla, MouseButton, Pixels,
    ScrollHandle, SharedString, StyledText, TextRun, Window, WindowTextSystem, canvas, div, point,
    prelude::*, px, rgb,
};

use super::library::LIBRARY_BACK_INSET;
use super::{
    Edit, FAVICON_SIZE, FAVICON_X, FOOTER_HEIGHT, HOVERED, NAV, NAV_RADIUS, NAV_SIZE, PLATE_HEIGHT,
    PLATE_INSET, PLATE_RADIUS, Palette, ROW_HEIGHT, SECONDARY, STATUS_AGENT, Shell, Slot, TILE,
    TILE_RADIUS, TITLE, TITLE_END_INSET, TITLE_GAP, What, cubic_bezier, dashed_plate,
    selected_plate, title,
};
use crate::agents::task::{Card, Dot, TaskAction, open_label, status_lead};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::tokens::shell::task::motion::{
    ARCHIVE, ARCHIVE_CLOSE, ARCHIVE_CLOSE_DELAY, ARCHIVE_CONTENT, ARCHIVE_EASE, ARCHIVE_HOLD,
    ARCHIVE_LEAVE, ARCHIVE_OPEN, BUMP, BUMP_EASE, BUMP_GLYPH, BUMP_PEAK, BUMP_SCALE, CHIPS,
    CHIPS_DELAY, CHIPS_RISE, CHIPS_STAGGER, CONTENT, CROSSFADE, FADE, FOLD, GHOST, GHOST_CONTENT,
    GHOST_DELAY, GHOST_LAND, GHOST_STAGGER, HEIGHT as GROW, ROLL, ROLL_RISE, SHIMMER,
    TITLE as TITLE_FADE,
};
use crate::tokens::shell::task::{
    BOTTOM, CHIP_END, CHIP_GAP, CHIP_HEIGHT, CHIP_X, CHIPS_BOTTOM, CHIPS_TOP, FOOT_SHARE, GAP,
    GHOST_RADIUS, GLYPH, GLYPH_TOP, HEIGHT, LINE, QUESTION_LINE, SHIMMER_FROM, SHIMMER_HIGH,
    SHIMMER_REACH, SHIMMER_TO, SLOT_DASH, STATUS, STATUS_GAP, STATUS_LINE, STOPPED, TOP,
};
use crate::tokens::shell::{MEDIA_BUTTON, URL_FIELD};
use crate::tokens::text;
use crate::tooltip::text_width;
use glyph::{Glyph, State};
use motion::{Curve, Ease};

/// A task's number, from its agent's id: `task-<n>`, or `session-<n>` for an outside agent's
/// session, which are numbered with the tasks.
pub(super) fn number(agent: &str) -> u64 {
    (agent.strip_prefix("task-"))
        .or_else(|| agent.strip_prefix("session-"))
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// `seconds` as a duration.
fn seconds(seconds: f32) -> Duration {
    Duration::from_secs_f32(seconds.max(0.))
}

/// The most tasks the foot shows before a plate says how many more there are, the slot counting
/// as one, so the foot never takes more than four plates' room unless the user opens it.
const MOST: usize = 3;

/// The rows a card's chips wrap to, at most.
const CHIP_ROWS: usize = 2;

/// The column the title and the status start on, from the plate's left edge.
const COLUMN: f32 = FAVICON_X - PLATE_INSET + FAVICON_SIZE + TITLE_GAP;

/// ✕'s inset from the plate's top and right edges, as a row's.
const CLOSE_INSET: f32 = (ROW_HEIGHT - MEDIA_BUTTON) / 2.;

/// Where the title ends under the pointer, clear of ✕.
const HOVER_END: f32 = CLOSE_INSET + MEDIA_BUTTON;

/// How long Open's pages may take to reach the list before their flight is let go.
const OPENING_WAIT: Duration = Duration::from_secs(3);

/// How a card lays out in the sidebar: its status's lines, and its chips, by their place in
/// `chips`, row by row.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Shape {
    lines: usize,
    rows: Vec<Vec<usize>>,
}

/// A chip on a card: its words, and what it does.
#[derive(Clone, Debug, PartialEq)]
enum Chip {
    Answer(SharedString),
    Retry,
    Copy(&'static str),
    Open(SharedString),
    /// An ask to point's (#2352), by its id: they go once the user points, the bar having them.
    NotNow(u64),
    Point(u64),
}

impl Chip {
    fn label(&self) -> SharedString {
        match self {
            Chip::Answer(answer) => answer.clone(),
            Chip::Retry => "Retry".into(),
            Chip::Copy(_) => "Copy".into(),
            Chip::Open(label) => label.clone(),
            Chip::NotNow(_) => "Not Now".into(),
            Chip::Point(_) => "Point".into(),
        }
    }
}

/// A card's chips: the ask's answers, Limit's Retry, Copy for the command that fixes a failure, or
/// Done's Open <site> or Open 3 pages.
fn chips(card: &Card) -> Vec<Chip> {
    let mut chips: Vec<Chip> = card.answers.iter().cloned().map(Chip::Answer).collect();
    if let Some(point) = card.point.filter(|point| !point.pointing) {
        chips.extend([Chip::NotNow(point.id), Chip::Point(point.id)]);
    }
    if card.retry {
        chips.push(Chip::Retry);
    }
    if let Some(command) = card.command {
        chips.push(Chip::Copy(command));
    }
    if let Some(label) = open_label(&card.open) {
        chips.push(Chip::Open(label));
    }
    chips
}

/// How `card` lays out in a sidebar `width` wide, with `measure` giving how wide a text is at a
/// size and weight. The agent's question takes a second line if it needs one. The chips wrap to
/// two rows; past that, or with an answer wider than the room, none show, and the card's click
/// opens the conversation, where they do. A chip of its own, such as Open, is cut to the room.
fn shape(
    card: &Card,
    width: f32,
    measure: &dyn Fn(&SharedString, f32, FontWeight) -> f32,
) -> Shape {
    let plate = width - 2. * PLATE_INSET;
    let status = plate - COLUMN - TITLE_END_INSET;
    let lines = match card.asks && measure(&card.status, text::SMALL, FontWeight::NORMAL) > status {
        true => 2,
        false => 1,
    };
    let room = plate - CHIP_END - (COLUMN - CHIP_X);
    let chips = chips(card);
    if chips.len() == 1 && !matches!(chips[0], Chip::Answer(_)) {
        return Shape {
            lines,
            rows: vec![vec![0]],
        };
    }
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut left = room;
    for (index, chip) in chips.iter().enumerate() {
        let wide = measure(&chip.label(), text::SMALL, FontWeight::MEDIUM).ceil() + 2. * CHIP_X;
        if wide > room {
            return Shape {
                lines,
                rows: Vec::new(),
            };
        }
        match rows.last_mut() {
            Some(row) if wide + CHIP_GAP <= left => {
                row.push(index);
                left -= wide + CHIP_GAP;
            }
            _ => {
                rows.push(vec![index]);
                left = room - wide;
            }
        }
    }
    if rows.len() > CHIP_ROWS {
        rows.clear();
    }
    Shape { lines, rows }
}

/// The status's height: a line, or the question's lines while it asks.
fn status_height(asks: bool, lines: usize) -> f32 {
    match asks {
        true => lines as f32 * QUESTION_LINE,
        false => STATUS_LINE,
    }
}

/// How far down the plate a card's chips start.
fn chips_top(asks: bool, lines: usize) -> f32 {
    TOP + LINE + STATUS_GAP + status_height(asks, lines) + CHIPS_TOP
}

/// A plate's height for `card` in `shape`: its title, its status, and its chips.
fn content_height(card: &Card, shape: &Shape) -> f32 {
    let chips = match shape.rows.len() {
        0 => 0.,
        rows => CHIPS_TOP + rows as f32 * CHIP_HEIGHT + (rows - 1) as f32 * CHIP_GAP + CHIPS_BOTTOM,
    };
    TOP + LINE + STATUS_GAP + status_height(card.asks, shape.lines) + BOTTOM + chips
}

/// The room over a plate `height` tall, which shrinks with it as it comes or goes.
fn gap(height: f32) -> f32 {
    GAP * (height / HEIGHT).clamp(0., 1.)
}

/// Where a task ranks for a place in the foot: waiting on the user first, then working, then done
/// or stopped.
fn rank(dot: Dot) -> u8 {
    match dot {
        Dot::Waiting => 0,
        Dot::Working => 1,
        Dot::Done | Dot::Ended => 2,
    }
}

/// The tasks of `cards` the foot shows, and how many more it has: all of them while it's opened or
/// they fit beside the slot, else as many as do, by rank and newest first.
fn choose(cards: &[&Card], all: bool, slot: bool) -> (Vec<SharedString>, usize) {
    let room = MOST - usize::from(slot);
    let more = cards.len().saturating_sub(room);
    if all || more == 0 {
        return (cards.iter().map(|card| card.agent.clone()).collect(), more);
    }
    let mut ranked: Vec<&Card> = cards.to_vec();
    ranked.sort_by_key(|card| (rank(card.dot), Reverse(number(&card.agent))));
    (
        ranked
            .into_iter()
            .take(room)
            .map(|card| card.agent.clone())
            .collect(),
        more,
    )
}

/// Each character of `text` at the status's size, by its bytes, with its middle's x, and the
/// line's width: where the shimmer's highlight is on each.
fn marks(text: &SharedString, system: &WindowTextSystem) -> (Vec<(Range<usize>, f32)>, f32) {
    let run = TextRun {
        len: text.len(),
        font: gpui::font(crate::face::UI),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = system.shape_line(text.clone(), px(text::SMALL), &[run], None);
    let marks = (text.char_indices())
        .map(|(at, each)| {
            let end = at + each.len_utf8();
            let middle = (f32::from(line.x_for_index(at)) + f32::from(line.x_for_index(end))) / 2.;
            (at..end, middle)
        })
        .collect();
    (marks, f32::from(line.width))
}

/// How lit the shimmer has `x` along a line `width` wide, `phase` of the way through its sweep:
/// 1 under its middle, nothing out of its reach.
fn shimmer(x: f32, width: f32, phase: f32) -> f32 {
    let middle = width * (SHIMMER_FROM + (SHIMMER_TO - SHIMMER_FROM) * phase);
    (1. - (x - middle).abs() / (SHIMMER_REACH * width)).max(0.)
}

/// What a plate at the foot holds.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Holds {
    /// "3 more tasks", which opens the foot to all of them, or, opened, "Show less".
    More,
    /// ⌘T's empty slot, which its task's card fills.
    Slot,
    Task(SharedString),
}

impl Holds {
    /// Its place, top first: the count over the slot over the tasks, newest first.
    fn key(&self) -> u64 {
        match self {
            Holds::More => u64::MAX,
            Holds::Slot => u64::MAX - 1,
            Holds::Task(agent) => number(agent),
        }
    }
}

/// A line of text, and the one it took over from, going since when, with the lines each took and
/// whether each was a question.
struct Line {
    text: SharedString,
    lines: usize,
    asks: bool,
    was: Option<(SharedString, usize, bool, Instant)>,
}

impl Line {
    fn new(text: SharedString, lines: usize, asks: bool) -> Self {
        Self {
            text,
            lines,
            asks,
            was: None,
        }
    }

    /// Takes `text`, the old one going from `now` if `animate`. Returns whether the words changed.
    fn set(
        &mut self,
        text: SharedString,
        lines: usize,
        asks: bool,
        now: Instant,
        animate: bool,
    ) -> bool {
        let changed = text != self.text;
        if changed {
            let old = std::mem::replace(&mut self.text, text);
            self.was = animate.then_some((old, self.lines, self.asks, now));
        }
        (self.lines, self.asks) = (lines, asks);
        changed
    }

    /// The old line and how far it has gone in `length` seconds, while it goes.
    fn going(&self, now: Instant, length: f32) -> Option<(&SharedString, usize, bool, f32)> {
        let (text, lines, asks, since) = self.was.as_ref()?;
        let t = now.saturating_duration_since(*since).as_secs_f32() / length;
        (t < 1.).then_some((text, *lines, *asks, t))
    }
}

/// Chips a card no longer has, fading where they were as the plate folds round them.
struct Leaving {
    chips: Vec<Chip>,
    rows: Vec<Vec<usize>>,
    top: f32,
    since: Instant,
}

/// What a task's plate shows.
struct Face {
    card: Card,
    shape: Shape,
    title: Line,
    status: Line,
    chips: Vec<Chip>,
    /// Since when its chips rise in, one after another.
    chips_in: Option<Instant>,
    chips_out: Option<Leaving>,
    glyph: Glyph,
    /// The status's characters and the line's width, for the shimmer.
    marks: Vec<(Range<usize>, f32)>,
    width: f32,
}

impl Face {
    fn new(
        card: &Card,
        shape: Shape,
        now: Instant,
        still: bool,
        system: &WindowTextSystem,
    ) -> Self {
        let (marks, width) = marks(&card.status, system);
        Self {
            title: Line::new(card.title.clone(), 1, false),
            status: Line::new(card.status.clone(), shape.lines, card.asks),
            chips: chips(card),
            chips_in: None,
            chips_out: None,
            glyph: Glyph::new(card.dot.into(), now, still),
            marks,
            width,
            card: card.clone(),
            shape,
        }
    }

    /// Takes `card`'s changes: a new title crossfades, a new status rolls in, chips that go fade
    /// where they were, unless Open sent them off already, and new ones rise in.
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        card: &Card,
        shape: Shape,
        now: Instant,
        still: bool,
        animate: bool,
        opened: bool,
        system: &WindowTextSystem,
    ) {
        self.title.set(card.title.clone(), 1, false, now, animate);
        if self
            .status
            .set(card.status.clone(), shape.lines, card.asks, now, animate)
        {
            (self.marks, self.width) = marks(&card.status, system);
        }
        let chips = chips(card);
        if chips != self.chips {
            let old = std::mem::replace(&mut self.chips, chips);
            let leaves = animate && !opened && !old.is_empty() && !self.shape.rows.is_empty();
            self.chips_out = leaves.then(|| Leaving {
                chips: old,
                rows: self.shape.rows.clone(),
                top: chips_top(self.card.asks, self.shape.lines),
                since: now,
            });
            self.chips_in = (animate && !self.chips.is_empty()).then_some(now);
        }
        self.glyph.set(card.dot.into(), now, still);
        self.card = card.clone();
        self.shape = shape;
    }

    /// How long its chips take to rise in, the last one's included.
    fn chips_length(&self, still: bool) -> f32 {
        match still {
            true => CROSSFADE,
            false => {
                CHIPS_DELAY + self.chips.len().saturating_sub(1) as f32 * CHIPS_STAGGER + CHIPS
            }
        }
    }

    fn moving(&self, now: Instant, still: bool) -> bool {
        let since = |at: Instant| now.saturating_duration_since(at).as_secs_f32();
        let roll = if still { CROSSFADE } else { ROLL };
        let title = if still { CROSSFADE } else { TITLE_FADE };
        let out = if still { CROSSFADE } else { CHIPS };
        self.title.going(now, title).is_some()
            || self.status.going(now, roll).is_some()
            || self
                .chips_in
                .is_some_and(|at| since(at) < self.chips_length(still))
            || self
                .chips_out
                .as_ref()
                .is_some_and(|leaving| since(leaving.since) < out)
            || self.glyph.moving(now)
    }

    /// Whether it draws on the loading dots' clock: its dot circling and its status shimmering.
    fn live(&self) -> bool {
        self.glyph.circling()
    }
}

/// A plate at the foot.
struct Plate {
    holds: Holds,
    /// Its height, which grows from nothing as it comes, to its content's as that changes, and to
    /// nothing as it goes, and its opacity.
    height: Ease,
    opacity: Ease,
    /// Its fill and its content: the slot's are nothing until its card lands in it.
    fill: Ease,
    content: Ease,
    leaving: bool,
    /// Since when a copy of it flies into the Archive button, from where it was in the sidebar,
    /// as it goes.
    archived: Option<(Instant, Bounds<Pixels>)>,
    face: Option<Face>,
}

impl Plate {
    /// A plate coming in: from nothing, fading in, unless the window hasn't drawn its foot yet. A
    /// slot comes as its dashes, with no fill.
    fn new(holds: Holds, now: Instant, still: bool, animate: bool) -> Self {
        let empty = holds == Holds::Slot;
        let mut plate = Plate {
            holds,
            height: Ease::rest(0.),
            opacity: Ease::rest(1.),
            fill: Ease::rest(if empty { 0. } else { 1. }),
            content: Ease::rest(if empty { 0. } else { 1. }),
            leaving: false,
            archived: None,
            face: None,
        };
        if animate {
            plate.opacity = Ease::rest(0.);
            let (length, curve) = if still {
                (CROSSFADE, Curve::Linear)
            } else {
                (FADE, Curve::Ease)
            };
            plate.opacity.go(1., now, length, 0., curve);
        }
        plate
    }

    /// Eases its height to `to`, at once if it shouldn't move.
    fn grow(&mut self, to: f32, now: Instant, moves: bool) {
        self.height
            .go(to, now, if moves { GROW } else { 0. }, 0., Curve::Ease);
    }

    /// Folds it away and fades it, or with motion reduced fades it and then takes its room.
    fn leave(&mut self, now: Instant, still: bool, animate: bool) {
        self.leaving = true;
        match (animate, still) {
            (false, _) => {
                self.height = Ease::rest(0.);
                self.opacity = Ease::rest(0.);
            }
            (true, true) => {
                self.opacity.go(0., now, CROSSFADE, 0., Curve::Linear);
                self.height.go(0., now, 0., CROSSFADE, Curve::Ease);
            }
            (true, false) => {
                self.height.go(0., now, GROW, 0., Curve::Ease);
                self.opacity.go(0., now, FADE, 0., Curve::Ease);
            }
        }
    }

    /// Comes back as it went, its copy on the way to the Archive gone.
    fn stay(&mut self, now: Instant, still: bool) {
        self.leaving = false;
        self.archived = None;
        let (length, curve) = if still {
            (CROSSFADE, Curve::Linear)
        } else {
            (FADE, Curve::Ease)
        };
        self.opacity.go(1., now, length, 0., curve);
    }

    fn moving(&self, now: Instant, still: bool) -> bool {
        [self.height, self.opacity, self.fill, self.content]
            .iter()
            .any(|ease| ease.moving(now))
            || self
                .face
                .as_ref()
                .is_some_and(|face| face.moving(now, still))
            || self
                .archived
                .is_some_and(|(since, _)| now < since + seconds(ARCHIVE))
    }

    fn settle(&mut self, now: Instant) {
        for ease in [
            &mut self.height,
            &mut self.opacity,
            &mut self.fill,
            &mut self.content,
        ] {
            ease.settle(now);
        }
    }

    /// The room it takes at `now`, with the gap over it.
    fn room(&self, now: Instant) -> f32 {
        let height = self.height.at(now);
        height + gap(height)
    }

    /// The room it will take once it's still.
    fn end_room(&self) -> f32 {
        let height = self.height.end();
        height + gap(height)
    }
}

/// A page Open sent to Today, flying from `from` in the sidebar to its row from `start`.
struct Ghost {
    agent: SharedString,
    tab: u64,
    from: Bounds<Pixels>,
    start: Instant,
}

/// Open's pages, which fly to their rows once the list has them.
struct Opening {
    agent: SharedString,
    tabs: Vec<u64>,
    from: Bounds<Pixels>,
    at: Instant,
}

/// The foot's plates and what moves on it.
pub(super) struct Foot {
    /// Top first.
    plates: Vec<Plate>,
    /// ⌘T's ask shows over the window, so the foot holds its slot.
    slot: bool,
    /// The user opened the foot to every task.
    all: bool,
    /// The plate under the pointer, by its key.
    hovered: Option<u64>,
    /// The window has drawn its foot, so what changes on it from now on moves.
    primed: bool,
    still: bool,
    ghosts: Vec<Ghost>,
    opening: Option<Opening>,
    /// Finished tasks the user has seen, which fold into the Archive once each is due and its pages
    /// have landed.
    folds: Vec<(SharedString, Instant)>,
    /// When the Archive button's glyph bumps, as a fold lands in it.
    bump: Option<Instant>,
    /// Where each plate and each chip was last drawn in the window, for Open's pages' flight and
    /// for check.sh, by the plate's key and the chip's place.
    plates_drawn: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    chips_drawn: Rc<RefCell<HashMap<(u64, usize), Bounds<Pixels>>>>,
    /// Where each was last logged at rest, by its log name: a plate's last frame on its way can
    /// already be where it rests.
    rested: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// Where the strip under a task's page drew its Open, in the window.
    pub(super) strip_open: Rc<Cell<Option<Bounds<Pixels>>>>,
    scroll: ScrollHandle,
    /// When the shimmer's sweeps began.
    epoch: Instant,
    /// The plates as last logged.
    logged: String,
}

impl Default for Foot {
    fn default() -> Self {
        Self {
            plates: Vec::new(),
            slot: false,
            all: false,
            hovered: None,
            primed: false,
            still: false,
            ghosts: Vec::new(),
            opening: None,
            folds: Vec::new(),
            bump: None,
            plates_drawn: Rc::default(),
            chips_drawn: Rc::default(),
            rested: Rc::default(),
            strip_open: Rc::default(),
            scroll: ScrollHandle::new(),
            epoch: Instant::now(),
            logged: String::new(),
        }
    }
}

impl Shell {
    /// The Agents' cards, newest first, none of which a private window shows, as it offers no
    /// tasks. A new one takes the ask's slot once ⌘T ran it, so nothing at the foot moves.
    pub fn set_task_cards(&mut self, mut cards: Vec<Card>, cx: &mut Context<Self>) {
        if self.one_space() {
            cards.clear();
        }
        if cards == self.task_cards {
            return;
        }
        let mut waits = false;
        for card in &cards {
            let was = self.task_cards.iter().find(|old| old.agent == card.agent);
            waits |= card.dot == Dot::Waiting && was.is_none_or(|was| was.dot != Dot::Waiting);
            if was.is_none_or(|was| (&was.status, was.dot) != (&card.status, card.dot)) {
                // check.sh reads this line.
                log!(
                    "browser: task card {} {:?} {}",
                    card.agent,
                    card.dot,
                    card.status
                );
            }
        }
        let fresh = |card: &&Card| !self.task_cards.iter().any(|old| old.agent == card.agent);
        let ran = self
            .foot
            .plates
            .iter()
            .any(|plate| plate.holds == Holds::Slot && !plate.leaving && plate.fill.end() > 0.);
        let took = (ran && self.foot.slot)
            .then(|| {
                cards
                    .iter()
                    .filter(fresh)
                    .max_by_key(|card| number(&card.agent))
            })
            .flatten()
            .map(|card| card.agent.clone());
        if took.is_some() {
            self.foot.slot = false;
        }
        self.task_cards = cards;
        // A card that turns to wait for the user brings a hidden sidebar out, once, as a question
        // does, so it reaches them while ilo is in front (#1080); the Dock's badge does otherwise.
        // It isn't held out, as a task may wait a long while over the page.
        if waits && self.can_reveal() {
            self.set_revealed(true, cx);
        }
        // A conversation whose card went, dismissed, goes with it.
        if let Some(agent) = self.talk_agent()
            && !self.task_cards.iter().any(|card| card.agent == agent)
        {
            self.close_task(cx);
        }
        self.sync_foot(took, cx);
        // The task the Archive brought back shows its conversation as its card comes back.
        if let Some(agent) = self.show_restored.take()
            && self.task_cards.iter().any(|card| card.agent == agent)
        {
            self.show_task(agent, cx);
        }
        cx.notify();
    }

    /// ⌘T's ask shows over this window, or no longer does: the foot opens an empty slot for the
    /// task's card, or closes it.
    pub fn set_ask_slot(&mut self, shown: bool, cx: &mut Context<Self>) {
        if shown == self.foot.slot {
            return;
        }
        log!("browser: ask slot {}", shown as u8);
        self.foot.slot = shown;
        self.sync_foot(None, cx);
        cx.notify();
    }

    /// The sidebar's width changed: the cards' text and chips wrap to it.
    pub(super) fn reshape_foot(&mut self, cx: &mut Context<Self>) {
        self.sync_foot(None, cx);
    }

    /// The stopped ring's colour: the ink at `stopped`, under the floor text keeps, as it's a mark
    /// beside words that say the task stopped.
    fn stopped_ring(&self) -> Hsla {
        Hsla::from(self.palette.ink).opacity(STOPPED)
    }

    /// A finished task's mark at rest, as its card shows it: Done's tick, or the grey ring of one
    /// stopped or failed, in the glyph's box.
    pub(super) fn status_mark(&self, dot: Dot) -> impl IntoElement + use<> {
        let now = Instant::now();
        let drawn = Glyph::new(State::from(dot), now, true).drawn(now);
        let signal: Hsla = rgb(STATUS_AGENT).into();
        let stopped = self.stopped_ring();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                glyph::paint(&drawn, bounds, signal, stopped, false, window)
            },
        )
        .size(px(GLYPH))
    }

    /// The glyph of `agent`'s card as the foot draws it now, for the strip under its page watched to
    /// lead with, so the two read as one: its card's at rest if the foot doesn't show it.
    pub(super) fn task_glyph(&self, agent: &SharedString, dot: Dot) -> gpui::AnyElement {
        let now = Instant::now();
        let face = (self.foot.plates.iter())
            .find(|plate| plate.holds == Holds::Task(agent.clone()) && !plate.leaving)
            .and_then(|plate| plate.face.as_ref());
        let Some(face) = face else {
            return self.status_mark(dot).into_any_element();
        };
        let drawn = face.glyph.drawn(now);
        let signal: Hsla = rgb(STATUS_AGENT).into();
        let stopped = self.stopped_ring();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                glyph::paint(&drawn, bounds, signal, stopped, false, window)
            },
        )
        .flex_none()
        .size(px(GLYPH))
        .into_any_element()
    }

    /// Brings the plates to the cards and the slot: those no longer wanted fold away, new ones
    /// grow in, and each task's plate takes its card's changes. `took` is the new card that fills
    /// the slot's plate.
    fn sync_foot(&mut self, took: Option<SharedString>, cx: &mut Context<Self>) {
        let now = Instant::now();
        let still = cx.reduce_motion();
        let animate = self.foot.primed;
        let moves = animate && !still;
        self.foot.still = still;
        let system = (self.text_system.take())
            .unwrap_or_else(|| WindowTextSystem::new(cx.text_system().clone()));
        let width = self.width();
        let cards = self.task_cards.clone();
        let listed: Vec<&Card> = cards.iter().collect();
        let (shown, more) = choose(&listed, self.foot.all, self.foot.slot);
        if more == 0 {
            self.foot.all = false;
        }
        let mut wanted: Vec<Holds> = Vec::new();
        if more > 0 {
            wanted.push(Holds::More);
        }
        if self.foot.slot {
            wanted.push(Holds::Slot);
        }
        wanted.extend(shown.into_iter().map(Holds::Task));
        let foot = &mut self.foot;
        if let Some(agent) = took
            && wanted.contains(&Holds::Task(agent.clone()))
            && let Some(plate) =
                (foot.plates.iter_mut()).find(|plate| plate.holds == Holds::Slot && !plate.leaving)
        {
            plate.holds = Holds::Task(agent);
        }
        for plate in &mut foot.plates {
            match (wanted.contains(&plate.holds), plate.leaving) {
                (false, false) => plate.leave(now, still, animate),
                (true, true) => plate.stay(now, still),
                _ => {}
            }
        }
        for holds in &wanted {
            if !foot.plates.iter().any(|plate| &plate.holds == holds) {
                foot.plates
                    .push(Plate::new(holds.clone(), now, still, animate));
            }
        }
        foot.plates.sort_by_key(|plate| Reverse(plate.holds.key()));
        let opening = foot.opening.as_ref().map(|opening| opening.agent.clone());
        let measure = |text: &SharedString, size: f32, weight: FontWeight| {
            text_width(text, size, weight, &system)
        };
        for plate in foot.plates.iter_mut().filter(|plate| !plate.leaving) {
            let Holds::Task(agent) = plate.holds.clone() else {
                plate.grow(HEIGHT, now, moves);
                continue;
            };
            let agent = &agent;
            let Some(card) = cards.iter().find(|card| &card.agent == agent) else {
                continue;
            };
            let shape = shape(card, width, &measure);
            plate.grow(content_height(card, &shape), now, moves);
            match &mut plate.face {
                Some(face) => {
                    let opened = opening.as_ref() == Some(agent)
                        || foot.ghosts.iter().any(|ghost| &ghost.agent == agent);
                    face.update(card, shape, now, still, animate, opened, &system);
                }
                None => plate.face = Some(Face::new(card, shape, now, still, &system)),
            }
            // A slot that the bar didn't fold into, such as a task an agent asked for, shows its
            // card at once.
            if plate.fill.end() == 0. {
                let length = if animate { CONTENT } else { 0. };
                plate.fill.go(1., now, length, 0., Curve::Ease);
                plate.content.go(1., now, length, 0., Curve::Ease);
            }
        }
        self.text_system = Some(system);
        let plates: Vec<String> = (self.foot.plates.iter())
            .filter(|plate| !plate.leaving)
            .map(|plate| match &plate.holds {
                Holds::More => format!("more {more}"),
                Holds::Slot => "slot".into(),
                Holds::Task(agent) => agent.to_string(),
            })
            .collect();
        let plates = plates.join(" ");
        if plates != self.foot.logged {
            // check.sh reads this line.
            log!(
                "browser: task foot {}",
                if plates.is_empty() { "empty" } else { &plates }
            );
            self.foot.logged = plates;
        }
    }

    /// Where the media players end, over Restart to Update, the foot's plates over them.
    pub(super) fn media_top(&self) -> f32 {
        self.over_safe_start() + self.update_banner_room() + self.media_room()
    }

    /// The most room the opened foot takes before it scrolls: the closed foot never does.
    pub(super) fn foot_cap(&self) -> Option<f32> {
        self.foot
            .all
            .then(|| FOOT_SHARE * f32::from(self.viewport.height))
    }

    /// The room the foot takes at `now`.
    fn foot_room(&self, now: Instant) -> f32 {
        let total: f32 = self.foot.plates.iter().map(|plate| plate.room(now)).sum();
        self.foot_cap().map_or(total, |cap| total.min(cap))
    }

    /// The room the foot takes now, which the list ends over.
    pub(super) fn foot_height(&self) -> f32 {
        self.foot_room(Instant::now())
    }

    /// Whether the sidebar shows the foot where it rests.
    fn foot_drawn(&self) -> bool {
        !self.page_fullscreen
            && !self.sidebar_hidden
            && self.sidebar_motion.is_none()
            && self.library.is_none()
            && self.form().is_none()
    }

    /// Whether a task's plate shows `agent`'s conversation.
    fn foot_shows(&self, agent: &SharedString) -> bool {
        self.talk_agent().as_ref() == Some(agent)
    }

    /// Where ⌘T's bar folds to as it runs a task: the slot's plate as it will rest, in the window,
    /// if the sidebar shows it, with its corners' radius and its fill.
    pub fn fold_target(&self) -> Option<(Bounds<Pixels>, f32, Hsla)> {
        if !self.foot_drawn() || !self.foot.slot {
            return None;
        }
        let live = || self.foot.plates.iter().filter(|plate| !plate.leaving);
        let index = live().position(|plate| plate.holds == Holds::Slot)?;
        let total: f32 = live().map(Plate::end_room).sum();
        let room = self.foot_cap().map_or(total, |cap| total.min(cap));
        let above: f32 = live().take(index).map(Plate::end_room).sum();
        let scrolled = f32::from(self.foot.scroll.offset().y);
        let top =
            f32::from(self.viewport.height) - self.media_top() - room + scrolled + above + GAP;
        let origin = self.sidebar_origin.get();
        let bounds = Bounds::new(
            point(origin.x + px(PLATE_INSET), origin.y + px(top)),
            gpui::size(px(self.width() - 2. * PLATE_INSET), px(HEIGHT)),
        );
        Some((bounds, TILE_RADIUS, self.palette.plate(TILE)))
    }

    /// The bar has gone for the task: folded into the slot, whose fill takes over as it lands and
    /// whose card fades in then, or, where it couldn't fold, hidden, and the card fades in at once.
    pub fn fold_started(&mut self, folded: bool, cx: &mut Context<Self>) {
        let now = Instant::now();
        let still = cx.reduce_motion();
        let Some(plate) = (self.foot.plates.iter_mut())
            .find(|plate| plate.holds == Holds::Slot && !plate.leaving)
        else {
            return;
        };
        match (folded, still) {
            (true, false) => {
                plate.fill.go(1., now, 0., FOLD, Curve::Ease);
                plate.content.go(1., now, CONTENT, FOLD, Curve::Ease);
            }
            (_, true) => {
                plate.fill.go(1., now, CROSSFADE, 0., Curve::Linear);
                plate.content.go(1., now, CROSSFADE, 0., Curve::Linear);
            }
            (false, false) => {
                plate.fill.go(1., now, CONTENT, 0., Curve::Ease);
                plate.content.go(1., now, CONTENT, 0., Curve::Ease);
            }
        }
        cx.notify();
    }

    /// Once a frame is built: drops what has gone, starts Open's pages' flights once the list has
    /// their rows, and asks for the next frame while anything moves. A working task's dot and
    /// shimmer draw on the loading dots' clock instead, as they move all the while it works.
    pub(super) fn step_foot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let still = cx.reduce_motion();
        self.foot.still = still;
        self.foot.primed = true;
        if let Some(opening) = &self.foot.opening {
            let layout = self.layout();
            if opening
                .tabs
                .iter()
                .all(|&tab| layout.contains_key(&Slot::Tab(tab)))
            {
                let opening = self.foot.opening.take().expect("an opening");
                let ghosts = opening.tabs.iter().enumerate().map(|(index, &tab)| Ghost {
                    agent: opening.agent.clone(),
                    tab,
                    from: opening.from,
                    start: now + seconds(GHOST_DELAY + index as f32 * GHOST_STAGGER),
                });
                self.foot.ghosts.extend(ghosts);
            } else if now.saturating_duration_since(opening.at) > OPENING_WAIT {
                self.foot.opening = None;
            }
        }
        let flight = seconds(GHOST + GHOST_LAND);
        self.foot.ghosts.retain(|ghost| now < ghost.start + flight);
        for agent in self.due_folds(now) {
            cx.defer_in(window, move |shell, _, cx| shell.archive_task(agent, cx));
        }
        if self
            .foot
            .bump
            .is_some_and(|start| now >= start + seconds(BUMP))
        {
            self.foot.bump = None;
        }
        let before = self.foot.plates.len();
        self.foot
            .plates
            .retain(|plate| !(plate.leaving && !plate.moving(now, still)));
        if self.foot.plates.len() != before {
            self.foot.plates_drawn.borrow_mut().clear();
            self.foot.rested.borrow_mut().clear();
        }
        for plate in &mut self.foot.plates {
            plate.settle(now);
        }
        if self.foot_moving(now) {
            window.request_animation_frame();
        }
        if self.foot_live() {
            self.breath.keep();
        }
    }

    /// Whether anything on the foot moves at `now`, which the display's frames draw.
    pub(super) fn foot_moving(&self, now: Instant) -> bool {
        let still = self.foot.still;
        self.foot.opening.is_some()
            || !self.foot.ghosts.is_empty()
            || !self.foot.folds.is_empty()
            || self
                .foot
                .bump
                .is_some_and(|start| now < start + seconds(BUMP))
            || self
                .foot
                .plates
                .iter()
                .any(|plate| plate.moving(now, still))
    }

    /// Whether a working task draws on the loading dots' clock.
    pub(super) fn foot_live(&self) -> bool {
        let face = |plate: &Plate| plate.face.as_ref().is_some_and(Face::live);
        self.foot
            .plates
            .iter()
            .any(|plate| !plate.leaving && face(plate))
    }

    /// Whether `tab`'s row waits for its page's flight to land, which draws it meanwhile.
    pub(super) fn landing(&self, tab: u64) -> bool {
        let now = Instant::now();
        let flight = seconds(GHOST + GHOST_LAND);
        self.foot
            .opening
            .as_ref()
            .is_some_and(|opening| opening.tabs.contains(&tab))
            || (self.foot.ghosts.iter()).any(|ghost| ghost.tab == tab && now < ghost.start + flight)
    }

    /// Done's Open or the strip's: the pages go to Today, each flying there from `from`, the Open
    /// that was clicked, in the window, as a row grows into its place.
    pub(super) fn open_task_pages(
        &mut self,
        agent: SharedString,
        tab: Option<u64>,
        from: Option<Bounds<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.task_cards.iter().find(|card| card.agent == agent) else {
            return;
        };
        let tabs = match tab {
            Some(tab) => vec![tab],
            None => card.open.iter().map(|(tab, _)| *tab).collect(),
        };
        // Its last page opened, a finished task has nothing left on its card, which folds into the
        // Archive once its pages have landed.
        let last = card.open.iter().all(|(each, _)| tabs.contains(each));
        if card.finished() && last {
            self.fold_later(agent.clone(), 0.);
        }
        if let Some(from) = from
            && !cx.reduce_motion()
            && !tabs.is_empty()
        {
            let origin = self.sidebar_origin.get();
            let from = Bounds::new(from.origin - origin, from.size);
            self.foot.opening = Some(Opening {
                agent: agent.clone(),
                tabs,
                from,
                at: Instant::now(),
            });
        }
        let action = tab.map_or(TaskAction::Open, TaskAction::OpenTab);
        self.browser.task(agent, action, cx);
        cx.notify();
    }

    /// The count's click: the foot opens to every task, or folds back to three.
    fn toggle_foot(&mut self, cx: &mut Context<Self>) {
        self.foot.all = !self.foot.all;
        // check.sh reads this line.
        log!(
            "browser: task foot {}",
            if self.foot.all { "opened" } else { "closed" }
        );
        self.sync_foot(None, cx);
        cx.notify();
    }

    /// ✕, or ⌘W in its conversation: the task goes, as ⌘Z and ⇧⌘T bring back.
    pub(super) fn dismiss_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        self.foot.hovered = None;
        self.browser.task(agent.clone(), TaskAction::Dismiss, cx);
        self.keep(
            What::DISMISS_TASK,
            Some(Edit::RestoreTask(agent.clone())),
            cx,
        );
        self.closed_preview = None;
        self.closed_pinned = None;
        let now = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH);
        self.closed_task = Some((agent, now.unwrap_or_default().as_secs_f64() * 1000.));
    }

    /// The user left `agent`'s conversation: a finished task they've now seen folds into the
    /// Archive, so the foot keeps only what's live or unread.
    pub(super) fn talk_left(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        if self
            .task_cards
            .iter()
            .any(|card| card.agent == agent && card.finished())
        {
            self.fold_later(agent, ARCHIVE_LEAVE);
            cx.notify();
        }
    }

    /// Folds `agent`'s card into the Archive `delay` seconds from now, or once its pages land.
    fn fold_later(&mut self, agent: SharedString, delay: f32) {
        self.foot.folds.retain(|(each, _)| each != &agent);
        self.foot
            .folds
            .push((agent, Instant::now() + seconds(delay)));
    }

    /// The folds due at `now`, taken off the list: each waits for its task's pages to land first.
    fn due_folds(&mut self, now: Instant) -> Vec<SharedString> {
        let Foot {
            folds,
            ghosts,
            opening,
            ..
        } = &mut self.foot;
        let mut due = Vec::new();
        folds.retain_mut(|(agent, at)| {
            if opening
                .as_ref()
                .is_some_and(|opening| &opening.agent == agent)
            {
                return true;
            }
            let lands = (ghosts.iter())
                .filter(|ghost| &ghost.agent == agent)
                .map(|ghost| ghost.start + seconds(GHOST))
                .max();
            if let Some(lands) = lands {
                *at = (*at).max(lands + seconds(ARCHIVE_OPEN));
            }
            let ready = *at <= now;
            if ready {
                due.push(agent.clone());
            }
            !ready
        });
        due
    }

    /// A finished task the user has seen leaves the foot as ✕ takes it, which ⌘Z and ⇧⌘T bring
    /// back: its plate goes at once and the plates above close its room, as a copy
    /// flies into the Archive button, whose glyph bumps as it lands. Where the sidebar doesn't
    /// show it, or with motion reduced, it goes as ✕ has it go. Not while its conversation shows.
    pub(super) fn archive_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        let finished = self
            .task_cards
            .iter()
            .any(|card| card.agent == agent && card.finished());
        if !finished || self.talk_agent().as_ref() == Some(&agent) {
            return;
        }
        let now = Instant::now();
        let flies =
            self.foot.primed && !cx.reduce_motion() && !self.one_space() && self.foot_drawn();
        let from = self
            .foot
            .plates_drawn
            .borrow()
            .get(&number(&agent))
            .copied();
        let plate = (self.foot.plates.iter_mut())
            .find(|plate| plate.holds == Holds::Task(agent.clone()) && !plate.leaving);
        if flies
            && let Some(from) = from
            && let Some(plate) = plate
        {
            let origin = self.sidebar_origin.get();
            plate.archived = Some((now, Bounds::new(from.origin - origin, from.size)));
            plate.leaving = true;
            plate.opacity = Ease::rest(0.);
            plate
                .height
                .go(0., now, ARCHIVE_CLOSE, ARCHIVE_CLOSE_DELAY, Curve::Ease);
            self.foot.bump = Some(now + seconds(ARCHIVE));
        }
        log!("browser: task {agent} archived");
        self.dismiss_task(agent.clone(), cx);
        // The Agents take its card away; the foot doesn't wait for them, so a change meanwhile
        // can't bring its plate back.
        self.task_cards.retain(|card| card.agent != agent);
        self.sync_foot(None, cx);
        cx.notify();
    }

    /// How far the Archive button's glyph is through its bump at `now`, 0 at rest and 1 at its
    /// peak, growing to it and going back.
    pub(super) fn archive_bump(&self, now: Instant) -> f32 {
        let Some(start) = self.foot.bump else {
            return 0.;
        };
        let Some(since) = now.checked_duration_since(start) else {
            return 0.;
        };
        let t = since.as_secs_f32() / BUMP;
        if t >= 1. {
            return 0.;
        }
        let way = cubic_bezier(BUMP_EASE[0], BUMP_EASE[1], BUMP_EASE[2], BUMP_EASE[3], t);
        match way < BUMP_PEAK {
            true => way / BUMP_PEAK,
            false => 1. - (way - BUMP_PEAK) / (1. - BUMP_PEAK),
        }
    }

    /// The Archive button's glyph's scale and alpha at `now`, through its bump.
    pub(super) fn archive_glyph(&self, now: Instant) -> (f32, f32) {
        let bump = self.archive_bump(now);
        (
            1. + (BUMP_SCALE - 1.) * bump,
            NAV + (BUMP_GLYPH - NAV) * bump,
        )
    }

    /// The copies of finished tasks' plates on their way into the Archive button: each from its
    /// plate's place to the button's box, its corners to the box's, its content fading at once and
    /// its plate over the end of the flight.
    pub(super) fn archive_flights(&self) -> Vec<AnyElement> {
        let now = Instant::now();
        let to = Bounds::new(
            point(
                px(LIBRARY_BACK_INSET),
                self.viewport.height - px((FOOTER_HEIGHT - NAV_SIZE) / 2. + NAV_SIZE),
            ),
            gpui::size(px(NAV_SIZE), px(NAV_SIZE)),
        );
        let flights = self.foot.plates.iter().filter_map(|plate| {
            let (since, from) = plate.archived?;
            let face = plate.face.as_ref()?;
            let gone = now.saturating_duration_since(since).as_secs_f32();
            let t = gone / ARCHIVE;
            if t >= 1. {
                return None;
            }
            let way = cubic_bezier(
                ARCHIVE_EASE[0],
                ARCHIVE_EASE[1],
                ARCHIVE_EASE[2],
                ARCHIVE_EASE[3],
                t,
            );
            let lerp = |a: Pixels, b: Pixels| a + (b - a) * way;
            let opacity = match t < ARCHIVE_HOLD {
                true => 1.,
                false => 1. - (t - ARCHIVE_HOLD) / (1. - ARCHIVE_HOLD),
            };
            let content = 1. - (gone / ARCHIVE_CONTENT).min(1.);
            let flight = div()
                .absolute()
                .left(lerp(from.origin.x, to.origin.x))
                .top(lerp(from.origin.y, to.origin.y))
                .w(lerp(from.size.width, to.size.width))
                .h(lerp(from.size.height, to.size.height))
                .rounded(px(TILE_RADIUS + (NAV_RADIUS - TILE_RADIUS) * way))
                .overflow_hidden()
                .bg(self.palette.plate(TILE))
                .opacity(opacity)
                .children((content > 0.).then(|| self.archived_face(face, from, content, now)));
            Some(flight.into_any_element())
        });
        flights.collect()
    }

    /// A plate's content as its copy takes it into the Archive: laid out at the plate's size,
    /// clipped as the copy shrinks, at `alpha`, and taking no clicks.
    fn archived_face(
        &self,
        face: &Face,
        from: Bounds<Pixels>,
        alpha: f32,
        now: Instant,
    ) -> AnyElement {
        let drawn = face.glyph.drawn(now);
        let signal: Hsla = rgb(STATUS_AGENT).into();
        let stopped = self.stopped_ring();
        let glyph = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                glyph::paint(&drawn, bounds, signal, stopped, false, window)
            },
        )
        .absolute()
        .left(px(FAVICON_X - PLATE_INSET + FAVICON_SIZE / 2. - GLYPH / 2.))
        .top(px(GLYPH_TOP))
        .size(px(GLYPH));
        let status = div()
            .mt(px(STATUS_GAP))
            .h(px(STATUS_LINE))
            .text_size(px(text::SMALL))
            .line_height(px(STATUS_LINE))
            .text_color(self.palette.text(SECONDARY))
            .truncate()
            .child(face.status.text.clone());
        let text = div()
            .absolute()
            .left(px(COLUMN))
            .right(px(TITLE_END_INSET))
            .top(px(TOP))
            .flex()
            .flex_col()
            .child(div().h(px(LINE)).flex().items_center().child(title(
                &self.palette,
                face.title.text.clone(),
                false,
            )))
            .child(status);
        let rows = face.shape.rows.iter().map(|row| {
            let row = (row.iter())
                .filter_map(|&index| face.chips.get(index))
                .map(|chip| card_chip(&self.palette, "task-chip-archived", chip.label()));
            div().flex().gap(px(CHIP_GAP)).children(row)
        });
        let chips = div()
            .absolute()
            .top(px(chips_top(face.card.asks, face.shape.lines)))
            .left(px(COLUMN - CHIP_X))
            .right(px(CHIP_END))
            .flex()
            .flex_col()
            .gap(px(CHIP_GAP))
            .children(rows);
        div()
            .absolute()
            .left_0()
            .top_0()
            .w(from.size.width)
            .h(from.size.height)
            .opacity(alpha)
            .child(glyph)
            .child(text)
            .child(chips)
            .into_any_element()
    }

    /// The foot, over the players, as tall as the room its plates take, scrolling once opened past
    /// its share of the sidebar.
    pub(super) fn task_foot(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.foot.plates.is_empty() {
            return None;
        }
        let now = Instant::now();
        let room = self.foot_room(now);
        let total: f32 = self.foot.plates.iter().map(|plate| plate.room(now)).sum();
        let plates: Vec<AnyElement> = (self.foot.plates.iter())
            .map(|plate| self.foot_plate(plate, now, cx))
            .collect();
        let column = div()
            .id("task-foot")
            .absolute()
            .left_0()
            .w(px(self.width()))
            .bottom(px(self.media_top()))
            .h(px(room))
            .flex()
            .flex_col();
        let column = match room < total - 0.5 {
            true => column.overflow_y_scroll().track_scroll(&self.foot.scroll),
            false => column.overflow_hidden(),
        };
        Some(column.children(plates).into_any_element())
    }

    /// A plate's box: its room, its corners and its opacity.
    fn plate_box(&self, plate: &Plate, now: Instant) -> gpui::Div {
        let height = plate.height.at(now);
        // A slot not yet filled shows its place in dashes, which go as its fill comes.
        let fill = plate.fill.at(now);
        let dashes = (fill < 1.).then(|| {
            let color = self.palette.glyph(SLOT_DASH);
            dashed_plate(color.opacity(1. - fill), TILE_RADIUS)
                .absolute()
                .inset_0()
        });
        div()
            .flex_none()
            .mt(px(gap(height)))
            .h(px(height))
            .mx(px(PLATE_INSET))
            .relative()
            .overflow_hidden()
            .rounded(px(TILE_RADIUS))
            .opacity(plate.opacity.at(now))
            .children(dashes)
    }

    fn foot_plate(&self, plate: &Plate, now: Instant, cx: &mut Context<Self>) -> AnyElement {
        let key = plate.holds.key();
        let drawn = drawn_listener(
            &self.foot.plates_drawn,
            &self.foot.rested,
            key,
            !plate.moving(now, self.foot.still),
        );
        match (&plate.holds, &plate.face) {
            (Holds::Task(_), Some(face)) => self.task_plate(plate, face, now, cx),
            (Holds::More, _) => self.more_plate(plate, now, cx),
            // The slot: the bar's plate lands in it, and its fill takes over.
            _ => {
                let fill = self.palette.plate(TILE).opacity(plate.fill.at(now));
                self.plate_box(plate, now)
                    .bg(fill)
                    .child(drawn)
                    .into_any_element()
            }
        }
    }

    /// "3 more tasks", which opens the foot to every task, or, opened, "Show less".
    fn more_plate(&self, plate: &Plate, now: Instant, cx: &mut Context<Self>) -> AnyElement {
        let listed = self.task_cards.len();
        let more = listed.saturating_sub(MOST - usize::from(self.foot.slot));
        let words: SharedString = match (self.foot.all, more) {
            (true, _) => "Show less".into(),
            (false, 1) => "1 more task".into(),
            (false, more) => format!("{more} more tasks").into(),
        };
        let hovered = self.foot.hovered == Some(Holds::More.key());
        let fill = self
            .palette
            .plate(if hovered { HOVERED } else { TILE })
            .opacity(plate.fill.at(now));
        let drawn = drawn_listener(
            &self.foot.plates_drawn,
            &self.foot.rested,
            Holds::More.key(),
            !plate.moving(now, self.foot.still),
        );
        self.plate_box(plate, now)
            .bg(fill)
            .child(drawn)
            .child(
                div()
                    .id("task-more")
                    .absolute()
                    .inset_0()
                    .pl(px(COLUMN))
                    .flex()
                    .items_center()
                    .text_size(px(text::SMALL))
                    .text_color(self.palette.text(SECONDARY))
                    .child(words)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_hover(cx.listener(|shell, hovered: &bool, _, cx| {
                        shell.hover_plate(Holds::More.key(), *hovered, cx);
                    }))
                    .on_click(cx.listener(|shell, _, _, cx| shell.toggle_foot(cx))),
            )
            .into_any_element()
    }

    fn hover_plate(&mut self, key: u64, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.foot.hovered = Some(key);
        } else if self.foot.hovered == Some(key) {
            self.foot.hovered = None;
        }
        cx.notify();
    }

    /// A task's plate: the glyph in the favicon's column, the title over the status, its chips
    /// under that, and ✕ under the pointer.
    fn task_plate(
        &self,
        plate: &Plate,
        face: &Face,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let agent = face.card.agent.clone();
        let key = plate.holds.key();
        let still = self.foot.still;
        let hovered = self.foot.hovered == Some(key);
        let fill = plate.fill.at(now);
        let shown = self.foot_shows(&agent);
        let plate_box = self.plate_box(plate, now);
        // Its conversation shows, as a selected tab's page does.
        let plate_box = match (shown, hovered) {
            (true, _) => selected_plate(
                &self.palette,
                plate_box,
                fill,
                f32::from(u8::from(hovered)),
                false,
            ),
            (false, true) => plate_box.bg(self.palette.plate(HOVERED).opacity(fill)),
            (false, false) => plate_box.bg(self.palette.plate(TILE).opacity(fill)),
        };
        let glyph = {
            let drawn = face.glyph.drawn(now);
            let signal: Hsla = rgb(STATUS_AGENT).into();
            let stopped = self.stopped_ring();
            // On the selected plate's fill the ring is drawn brighter, to read as on the others.
            let paint = move |bounds, _, window: &mut Window, _: &mut gpui::App| {
                glyph::paint(&drawn, bounds, signal, stopped, shown, window)
            };
            canvas(|_, _, _| {}, paint)
                .absolute()
                .left(px(FAVICON_X - PLATE_INSET + FAVICON_SIZE / 2. - GLYPH / 2.))
                .top(px(GLYPH_TOP))
                .size(px(GLYPH))
        };
        let end = if hovered { HOVER_END } else { TITLE_END_INSET };
        let text = div()
            .absolute()
            .left(px(COLUMN))
            .right(px(end))
            .top(px(TOP))
            .flex()
            .flex_col()
            .child(self.task_title(face, shown, now, still))
            .child(self.task_status(face, now, still));
        let content = div()
            .absolute()
            .inset_0()
            .opacity(plate.content.at(now))
            .child(glyph)
            .child(text)
            .children(self.task_chips(face, now, still, !plate.moving(now, still), cx))
            .children(self.leaving_chips(face, now, still));
        let close = hovered.then(|| {
            let agent = agent.clone();
            self.media_button("task-close", number(&agent), "icons/x.svg")
                .absolute()
                .top(px(CLOSE_INSET))
                .right(px(CLOSE_INSET))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    cx.stop_propagation();
                    log!("browser: command dismiss-task via sidebar");
                    shell.dismiss_task(agent.clone(), cx);
                }))
        });
        let drawn = drawn_listener(
            &self.foot.plates_drawn,
            &self.foot.rested,
            key,
            !plate.moving(now, still),
        );
        // Its chips and ✕ take their own clicks before the plate does.
        plate_box
            .id(("task-plate", key))
            .child(drawn)
            .child(content)
            .children(close)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(
                cx.listener(move |shell, hovered: &bool, _, cx| {
                    shell.hover_plate(key, *hovered, cx)
                }),
            )
            .on_click(cx.listener(move |shell, _, _, cx| shell.show_task(agent.clone(), cx)))
            .into_any_element()
    }

    /// The title, a new one crossfading over the old.
    fn task_title(
        &self,
        face: &Face,
        shown: bool,
        now: Instant,
        still: bool,
    ) -> impl IntoElement + use<> {
        let length = if still { CROSSFADE } else { TITLE_FADE };
        let line = |words: SharedString, alpha: f32| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .opacity(alpha)
                .child(title(&self.palette, words, shown))
        };
        let (old, t) = match face.title.going(now, length) {
            Some((old, _, _, t)) => (Some(line(old.clone(), 1. - t)), t),
            None => (None, 1.),
        };
        div()
            .relative()
            .h(px(LINE))
            .children(old)
            .child(line(face.title.text.clone(), t))
    }

    /// The status: each new one rolling in from under the old, which goes up, or with motion
    /// reduced crossfading. While the task works a highlight sweeps along it.
    fn task_status(&self, face: &Face, now: Instant, still: bool) -> impl IntoElement + use<> {
        let length = if still { CROSSFADE } else { ROLL };
        let rise = if still { 0. } else { ROLL_RISE };
        let working = face.glyph.state() == State::Working && !still;
        let room = self.width() - 2. * PLATE_INSET - COLUMN - TITLE_END_INSET;
        let phase =
            (now.saturating_duration_since(self.foot.epoch).as_secs_f32() / SHIMMER).fract();
        let line = |words: SharedString,
                    lines: usize,
                    asks: bool,
                    top: f32,
                    alpha: f32,
                    shimmers: bool| {
            let height = if asks { QUESTION_LINE } else { STATUS_LINE };
            // Its lead, how far the agent has got or that it needs the user, in the title's ink.
            let lead = status_lead(&words);
            let base = div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(top))
                .opacity(alpha)
                .text_size(px(text::SMALL))
                .line_height(px(height))
                .text_color(self.palette.text(STATUS));
            let base = match lines {
                1 => base.truncate(),
                lines => base.line_clamp(lines).text_ellipsis(),
            };
            match shimmers {
                true => {
                    let width = face.width.min(room).max(1.);
                    let highlights = face.marks.iter().map(|(range, x)| {
                        let led = range.end <= lead;
                        let lit = if led { 1. } else { shimmer(*x, width, phase) };
                        let highlight = HighlightStyle {
                            font_weight: led.then_some(FontWeight::MEDIUM),
                            ..self.palette.lit_highlight(STATUS, SHIMMER_HIGH, lit)
                        };
                        (range.clone(), highlight)
                    });
                    let styled =
                        StyledText::new(words).with_highlights(highlights.collect::<Vec<_>>());
                    // At the shimmer's peak, which each part fades down from.
                    let peak = self.palette.lit(STATUS, SHIMMER_HIGH, 1.);
                    base.text_color(peak).child(styled)
                }
                false if lead > 0 => {
                    let highlight = HighlightStyle {
                        color: Some(self.palette.text(TITLE)),
                        font_weight: Some(FontWeight::MEDIUM),
                        ..HighlightStyle::default()
                    };
                    base.child(StyledText::new(words).with_highlights([(0..lead, highlight)]))
                }
                false => base.child(words),
            }
        };
        let status = &face.status;
        let (old, t) = match status.going(now, length) {
            Some((old, lines, asks, t)) => {
                let t = if still { t } else { motion::eased(t) };
                (
                    Some(line(old.clone(), lines, asks, -rise * t, 1. - t, false)),
                    t,
                )
            }
            None => (None, 1.),
        };
        let lines = if status.asks { status.lines } else { 1 };
        div()
            .relative()
            .mt(px(STATUS_GAP))
            .h(px(status_height(status.asks, status.lines)))
            .children(old)
            .child(line(
                status.text.clone(),
                lines,
                status.asks,
                rise * (1. - t),
                t,
                working,
            ))
    }

    /// The card's chips, in the rows its shape gives them, the first one's words on the title's
    /// column, each rising in after the one before as they come. Open hides as its pages fly.
    fn task_chips(
        &self,
        face: &Face,
        now: Instant,
        still: bool,
        rest: bool,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let shape = &face.shape;
        if shape.rows.is_empty() {
            return None;
        }
        let card = &face.card;
        let n = number(&card.agent);
        let flying = self
            .foot
            .opening
            .as_ref()
            .is_some_and(|opening| opening.agent == card.agent)
            || self
                .foot
                .ghosts
                .iter()
                .any(|ghost| ghost.agent == card.agent);
        let since = face
            .chips_in
            .map(|at| now.saturating_duration_since(at).as_secs_f32());
        let rows = shape.rows.iter().map(|row| {
            let row = row.iter().filter_map(|&index| {
                let chip = face.chips.get(index)?.clone();
                let (alpha, rise) = match since {
                    None => (1., 0.),
                    Some(since) if still => ((since / CROSSFADE).min(1.), 0.),
                    Some(since) => {
                        let t = ((since - CHIPS_DELAY - index as f32 * CHIPS_STAGGER) / CHIPS)
                            .clamp(0., 1.);
                        let t = motion::eased(t);
                        (t, CHIPS_RISE * (1. - t))
                    }
                };
                let alpha = if flying && matches!(chip, Chip::Open(_)) {
                    0.
                } else {
                    alpha
                };
                // Point dims where it can't start, and says why on hover.
                let can_point = match chip {
                    Chip::Point(_) => self.may_point_here(&card.agent),
                    _ => 0,
                };
                let alpha = match can_point {
                    0 => alpha,
                    _ => alpha * crate::tokens::foot_bar::DISABLED,
                };
                let id = ("task-chip", n * 8 + index as u64);
                let element = card_chip(&self.palette, id, chip.label())
                    .relative()
                    .top(px(rise))
                    .opacity(alpha);
                let drawn = drawn_listener_chip(
                    &self.foot.chips_drawn,
                    &self.foot.rested,
                    (n, index),
                    chip.label(),
                    rest,
                );
                let element = element.child(drawn);
                let agent = card.agent.clone();
                let element = match chip {
                    Chip::Answer(answer) => {
                        element.on_click(act(agent, TaskAction::Answer(answer), cx))
                    }
                    Chip::Retry => element.on_click(act(agent, TaskAction::Retry, cx)),
                    Chip::Open(_) => {
                        let chips = self.foot.chips_drawn.clone();
                        element.on_click(cx.listener(move |shell, _, _, cx| {
                            cx.stop_propagation();
                            let from = chips.borrow().get(&(n, index)).copied();
                            log!("browser: command open-task-pages via sidebar");
                            shell.open_task_pages(agent.clone(), None, from, cx);
                        }))
                    }
                    Chip::Copy(command) => element.on_click(cx.listener(move |shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.copy(command, "Command Copied", cx);
                    })),
                    Chip::NotNow(id) => element.on_click(cx.listener(move |shell, _, _, cx| {
                        cx.stop_propagation();
                        // check.sh reads this line.
                        log!("browser: point {id} not now");
                        shell.browser.answer_point(id, None, cx);
                    })),
                    Chip::Point(id) => {
                        let chips = self.foot.chips_drawn.clone();
                        element
                            .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                                let frame = chips.borrow().get(&(n, index)).copied();
                                shell.point_tip(frame, *hovered, can_point, cx);
                            }))
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                cx.stop_propagation();
                                shell.press_point(id, &agent, cx);
                            }))
                    }
                };
                Some(element.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()))
            });
            div().flex().gap(px(CHIP_GAP)).children(row)
        });
        Some(
            div()
                .absolute()
                .top(px(chips_top(card.asks, shape.lines)))
                .left(px(COLUMN - CHIP_X))
                .right(px(CHIP_END))
                .flex()
                .flex_col()
                .gap(px(CHIP_GAP))
                .children(rows),
        )
    }

    /// Chips the card no longer has, sinking and fading where they were.
    fn leaving_chips(
        &self,
        face: &Face,
        now: Instant,
        still: bool,
    ) -> Option<impl IntoElement + use<>> {
        let leaving = face.chips_out.as_ref()?;
        let length = if still { CROSSFADE } else { CHIPS };
        let t = now.saturating_duration_since(leaving.since).as_secs_f32() / length;
        if t >= 1. {
            return None;
        }
        let t = if still { t } else { motion::eased(t) };
        let rise = if still { 0. } else { CHIPS_RISE * t };
        let rows = leaving.rows.iter().map(|row| {
            let row = (row.iter())
                .filter_map(|&index| leaving.chips.get(index))
                .map(|chip| card_chip(&self.palette, "task-chip-going", chip.label()));
            div().flex().gap(px(CHIP_GAP)).children(row)
        });
        Some(
            div()
                .absolute()
                .top(px(leaving.top + rise))
                .left(px(COLUMN - CHIP_X))
                .right(px(CHIP_END))
                .opacity(1. - t)
                .flex()
                .flex_col()
                .gap(px(CHIP_GAP))
                .children(rows),
        )
    }

    /// Open's pages on their way from the chip to their rows at the top of Today: each a chip's
    /// plate growing into a row's hover plate, its favicon and title fading in, the row showing
    /// as it lands.
    pub(super) fn task_ghosts(&self) -> Vec<AnyElement> {
        let now = Instant::now();
        let layout = self.layout();
        let scrolled = f32::from(self.list_scroll.offset().y);
        let width = self.width() - 2. * PLATE_INSET;
        // Waiting for the list, they sit on the chip.
        let waiting: Vec<(u64, Bounds<Pixels>)> = match &self.foot.opening {
            Some(opening) => opening
                .tabs
                .iter()
                .map(|&tab| (tab, opening.from))
                .collect(),
            None => Vec::new(),
        };
        let flying = self.foot.ghosts.iter().filter_map(|ghost| {
            let row = layout.get(&Slot::Tab(ghost.tab))?;
            let top = self.drawn_list_top()
                + row
                + scrolled
                + self.slide(Slot::Tab(ghost.tab))
                + (ROW_HEIGHT - PLATE_HEIGHT) / 2.;
            let t =
                (now.saturating_duration_since(ghost.start).as_secs_f32() / GHOST).clamp(0., 1.);
            let to = Bounds::new(
                point(px(PLATE_INSET), px(top)),
                gpui::size(px(width), px(PLATE_HEIGHT)),
            );
            let from = ghost.from;
            let way = motion::eased(t);
            let lerp = |a: Pixels, b: Pixels| a + (b - a) * way;
            let at = Bounds::new(
                point(
                    lerp(from.origin.x, to.origin.x),
                    lerp(from.origin.y, to.origin.y),
                ),
                gpui::size(
                    lerp(from.size.width, to.size.width),
                    lerp(from.size.height, to.size.height),
                ),
            );
            let content = (now.saturating_duration_since(ghost.start).as_secs_f32()
                / GHOST_CONTENT)
                .clamp(0., 1.);
            Some((ghost.tab, at, content, way))
        });
        let mut ghosts: Vec<AnyElement> = waiting
            .into_iter()
            .map(|(tab, at)| self.ghost(tab, at, 0., 0.))
            .collect();
        ghosts.extend(flying.map(|(tab, at, content, way)| self.ghost(tab, at, way, content)));
        ghosts
    }

    /// A page's flight at `at`, `way` of the way from a chip's corners and fill to a row's hover
    /// plate's, its favicon and title at `content`.
    fn ghost(&self, tab: u64, at: Bounds<Pixels>, way: f32, content: f32) -> AnyElement {
        let radius = GHOST_RADIUS + (PLATE_RADIUS - GHOST_RADIUS) * way;
        let fill = URL_FIELD + (HOVERED - URL_FIELD) * way;
        div()
            .absolute()
            .left(at.origin.x)
            .top(at.origin.y)
            .w(at.size.width)
            .h(at.size.height)
            .rounded(px(radius))
            .overflow_hidden()
            .bg(self.palette.plate(fill))
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .gap(px(TITLE_GAP))
                    .pl(px(FAVICON_X - PLATE_INSET))
                    .pr(px(TITLE_END_INSET))
                    .opacity(content)
                    .child(self.tab_icon(tab))
                    .child(title(&self.palette, self.title(tab), false)),
            )
            .into_any_element()
    }
}

impl Shell {
    /// Where `agent`'s plate was last drawn in the window.
    #[cfg(test)]
    pub(super) fn task_plate_drawn(&self, agent: &str) -> Option<Bounds<Pixels>> {
        self.foot.plates_drawn.borrow().get(&number(agent)).copied()
    }

    /// Records where the strip under a task's page draws its Open, which its page flies from.
    pub(super) fn strip_open_listener(&self) -> impl IntoElement + use<> {
        let drawn = self.foot.strip_open.clone();
        canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
            .absolute()
            .inset_0()
    }
}

/// Records where a plate is drawn in the window, and logs it once it's at rest there.
fn drawn_listener(
    drawn: &Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    rested: &Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    key: u64,
    rest: bool,
) -> impl IntoElement + use<> {
    let (drawn, rested) = (drawn.clone(), rested.clone());
    canvas(
        move |bounds, _, _| {
            drawn.borrow_mut().insert(key, bounds);
            let name = match key {
                u64::MAX => "more".to_string(),
                key if key == u64::MAX - 1 => "slot".to_string(),
                key => format!("task-{key}"),
            };
            if rest && rested.borrow_mut().insert(format!("plate {name}"), bounds) != Some(bounds) {
                // check.sh reads this line.
                log!("browser: task plate {name} at {}", logged(bounds));
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// Records where a chip is drawn in the window, for Open's pages' flight, and logs it at rest.
fn drawn_listener_chip(
    drawn: &Rc<RefCell<HashMap<(u64, usize), Bounds<Pixels>>>>,
    rested: &Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    key: (u64, usize),
    label: SharedString,
    rest: bool,
) -> impl IntoElement + use<> {
    let (drawn, rested) = (drawn.clone(), rested.clone());
    canvas(
        move |bounds, _, _| {
            drawn.borrow_mut().insert(key, bounds);
            let name = format!("chip task-{} {label}", key.0);
            if rest && rested.borrow_mut().insert(name, bounds) != Some(bounds) {
                log!(
                    "browser: task chip task-{} {label} at {}",
                    key.0,
                    logged(bounds)
                );
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

fn logged(bounds: Bounds<Pixels>) -> String {
    format!(
        "{:.0} {:.0} {:.0} {:.0}",
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height)
    )
}

/// A chip on a card, the sidebar's scale: the URL field's plate at a control's corners, as wide
/// as its words, cut to the room it has.
pub(super) fn card_chip(
    palette: &Palette,
    id: impl Into<gpui::ElementId>,
    label: SharedString,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .min_w_0()
        .h(px(CHIP_HEIGHT))
        .px(px(CHIP_X))
        .flex()
        .items_center()
        .rounded(px(NAV_RADIUS))
        .bg(palette.plate(URL_FIELD))
        .hover(|style| style.bg(palette.state(URL_FIELD, STATE_HOVERED)))
        .active(|style| style.bg(palette.state(URL_FIELD, STATE_PRESSED)))
        .text_size(px(text::SMALL))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.text(TITLE))
        .child(div().truncate().child(label))
}

/// A chip's click, which does `action` to the task.
pub(super) fn act(
    agent: SharedString,
    action: TaskAction,
    cx: &mut Context<Shell>,
) -> impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static + use<> {
    let shell = cx.weak_entity();
    move |_, _, cx| {
        cx.stop_propagation();
        let act = |shell: &mut Shell, cx: &mut Context<Shell>| {
            shell.browser.task(agent.clone(), action.clone(), cx);
        };
        shell.update(cx, act).ok();
    }
}
