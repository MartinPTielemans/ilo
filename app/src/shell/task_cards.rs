//! The window's agent tasks at the sidebar's foot, over its media players (Agent task): a group
//! each, oldest lowest, so the list's rows above never move as tasks come and go. A group's header
//! is a sidebar row: the favicon's column holds the state's glyph, the title is a row's, and the
//! line under it says who works on the task and its state, in ilo's words. The pages of the task
//! shown are rows under its header, one step in beside a hairline, each a favicon, the page's
//! title and one word; the other groups are closed to their header. Nothing has a plate at rest:
//! a header or a page's row takes the sidebar's two, under the pointer and on what the page area
//! shows, so one thing in the sidebar is shown at a time. An act that is the user's and has no
//! page, Retry or its assistant's Sign in to … or Get …, is one quiet chip under the header.
//! ✕ shows under the pointer. Three show, waiting first, then working, then done, and a row over
//! them says how many more there are and opens the foot to all of them. While ⌘T asks for a task,
//! the foot holds an empty slot a row tall, which the bar folds into and the header fills. The foot
//! is the window's, as the players are, so switching Spaces never hides a task. It draws frames
//! only while something on it moves: its groups' motion on the display's frames, and a working
//! task's circling dot and shimmer on the loading dots' clock, so a foot at rest draws nothing.

mod glyph;
mod motion;
#[cfg(test)]
pub(super) mod tests;

use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use gpui::{
    AnyElement, Bounds, Context, FontWeight, HighlightStyle, Hsla, MouseButton, Pixels,
    ScrollHandle, SharedString, StyledText, TextRun, Window, WindowTextSystem, canvas, div, point,
    prelude::*, px, rgb,
};

use super::ShowStarted;
use super::agent_bar;
use super::library::LIBRARY_BACK_INSET;
use super::page_ends::StripStop;
use super::{
    Edit, FAVICON_SIZE, FAVICON_X, FOOTER_HEIGHT, HOVERED, NAV, NAV_RADIUS, NAV_SIZE, PLATE_HEIGHT,
    PLATE_INSET, PLATE_RADIUS, ROW_HEIGHT, SECONDARY, STATUS_AGENT, Shell, Slot, TILE, TITLE,
    TITLE_END_INSET, TITLE_GAP, What, cubic_bezier, dashed_plate, selected_plate, title,
};
use crate::agents::runner::Cli;
use crate::agents::sign_in::Offer;
use crate::agents::task::{Card, Dot, NEEDS_YOU, TaskAction, Word};
use crate::agents::tier::Tier;
use crate::palette::FIELD_PLATE as URL_FIELD;
use crate::tokens::shell::task::motion::{
    ARCHIVE, ARCHIVE_CLOSE, ARCHIVE_CLOSE_DELAY, ARCHIVE_CONTENT, ARCHIVE_EASE, ARCHIVE_HOLD,
    ARCHIVE_LEAVE, ARCHIVE_OPEN, BUMP, BUMP_EASE, BUMP_GLYPH, BUMP_PEAK, BUMP_SCALE, CONTENT,
    CROSSFADE, FADE, FOLD, GHOST, GHOST_CONTENT, GHOST_DELAY, GHOST_LAND, GHOST_STAGGER,
    HEIGHT as GROW, ROLL, ROLL_RISE, SHIMMER, TITLE as TITLE_FADE,
};
use crate::tokens::shell::task::{
    CHIP_HEIGHT, CHIP_X, FOOT_SHARE, GHOST_RADIUS, GLYPH, LINE, PAGE_DOT, PAGE_DOT_GAP, PAGE_END,
    PAGE_FAVICON, PAGE_GAP, PAGE_HEIGHT, PAGE_INK, PAGE_RADIUS, PAGE_START, PAGE_TITLE, PAGE_WORD,
    PAGE_WORD_WORKING, PAGES_BOTTOM, PAGES_GAP, PAGES_LINE, PAGES_LINE_INK, PAGES_TOP,
    SHIMMER_FROM, SHIMMER_HIGH, SHIMMER_REACH, SHIMMER_TO, SLOT_DASH, STATUS, STATUS_GAP,
    STATUS_LINE, STOPPED,
};
use crate::tokens::shell::{FAVICON_DRAWN, MEDIA_BUTTON};
use crate::tokens::text;
use glyph::{Glyph, State};
use motion::{Curve, Ease};

/// Now, for the foot's motion. Tests hold this clock still and move it on themselves, so how far a
/// fold has got is where the test put it, not wherever a slow runner's clock had got to (#3462).
#[cfg(not(test))]
fn clock() -> Instant {
    Instant::now()
}

#[cfg(test)]
thread_local! {
    static CLOCK: Cell<Option<Instant>> = const { Cell::new(None) };
}

#[cfg(test)]
fn clock() -> Instant {
    CLOCK.with(Cell::get).unwrap_or_else(Instant::now)
}

/// Sets the foot's clock to `at`, where it stays until set again.
#[cfg(test)]
fn set_clock(at: Instant) {
    CLOCK.with(|clock| clock.set(Some(at)));
}

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

/// The most tasks the foot shows before a row says how many more there are, the slot counting as
/// one, so the foot never holds more than four headers unless the user opens it.
const MOST: usize = 3;

/// The column the title and the state's line start on, from the header's plate's left edge.
const COLUMN: f32 = FAVICON_X - PLATE_INSET + FAVICON_SIZE + TITLE_GAP;

/// A header's plate's top in its row, as a tab's row's.
const PLATE_TOP: f32 = (ROW_HEIGHT - PLATE_HEIGHT) / 2.;

/// The title's top in the header's plate, the two lines centred in it.
const TEXT_TOP: f32 = (PLATE_HEIGHT - LINE - STATUS_GAP - STATUS_LINE) / 2.;

/// ✕'s inset from the header's plate's top and right edges.
const CLOSE_INSET: f32 = (PLATE_HEIGHT - MEDIA_BUTTON) / 2.;

/// Where the title ends under the pointer, clear of ✕.
const HOVER_END: f32 = CLOSE_INSET + MEDIA_BUTTON;

/// The pages' hairline's left, from the sidebar's: under the middle of the favicon's column.
const PAGES_X: f32 = FAVICON_X + FAVICON_SIZE / 2. - PAGES_LINE / 2.;

/// The room the chip takes under its header.
const CHIP_ROOM: f32 = PAGES_TOP + CHIP_HEIGHT + PAGES_BOTTOM;

/// How long Open's pages may take to reach the list before their flight is let go.
const OPENING_WAIT: Duration = Duration::from_secs(3);

/// The one chip under a header: an act that is the user's and has no page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Chip {
    Retry,
    /// A failure's fix, its assistant's row's own button.
    Assistant(Cli, Offer),
}

impl Chip {
    fn label(self) -> SharedString {
        match self {
            Chip::Retry => "Retry".into(),
            Chip::Assistant(cli, offer) => offer.label(cli).unwrap_or_default().into(),
        }
    }
}

/// A card's chip: a failure's fix from its assistant's row, else Limit's Retry.
fn chip(card: &Card) -> Option<Chip> {
    (card.fix.map(|(cli, offer)| Chip::Assistant(cli, offer))).or(card.retry.then_some(Chip::Retry))
}

/// The room a card's chip takes under its header.
fn chip_room(card: &Card) -> f32 {
    chip(card).map_or(0., |_| CHIP_ROOM)
}

/// The room `pages` rows take under a header.
fn pages_room(pages: usize) -> f32 {
    match pages {
        0 => 0.,
        pages => PAGES_TOP + pages as f32 * PAGE_HEIGHT + PAGES_BOTTOM,
    }
}

/// A group's height for `card`: its header's row, its chip, and, `open`, its pages' rows.
fn group_height(card: &Card, open: bool) -> f32 {
    let pages = if open {
        pages_room(card.pages.len())
    } else {
        0.
    };
    ROW_HEIGHT + chip_room(card) + pages
}

/// The line under a header's title: who works on the task, or Full access where it has that, and
/// its state, in ilo's words alone. The agent's own words are the title's (#3690). The state is
/// the task's one word, in the fuller form its card has for it where that names the same state:
/// "Stopped after 6 steps", a session's "Done · 10:42", "Working · 3 helpers".
fn state_line(card: &Card) -> SharedString {
    let fuller = match card.word {
        Word::Stopped => card.status.starts_with("Stopped"),
        Word::Done => !card.replies && card.status.starts_with("Done"),
        Word::Working => card.status.starts_with("Working · "),
        _ => false,
    };
    let state = match fuller {
        true => card.status.as_ref(),
        false => card.word.after_name(),
    };
    let lead = match card.tier {
        Tier::Full => Tier::Full.label(),
        _ => card.name.as_ref(),
    };
    format!("{lead} · {state}").into()
}

/// How much of a state's line is its lead, Full access, which is in the title's ink.
fn state_lead(line: &str) -> usize {
    let full = Tier::Full.label();
    let leads = (line.strip_prefix(full)).is_some_and(|rest| rest.starts_with(" · "));
    if leads { full.len() } else { 0 }
}

/// A task's page's word and whether it has the outline, as `Shell::page_mark` says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PageMark {
    pub word: PageWord,
    pub outline: bool,
}

/// What a task's page is to it now, in the words every place that names the page says: the end of
/// its row at the sidebar's foot, and its tile's head line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PageWord {
    /// The task works in it now, which Signal's dot before the word says too.
    Working,
    Done,
    /// Its task waits for the user here: the page it handed them. In the title's ink, as the
    /// glyph is while the task asks.
    NeedsYou,
    /// The user took it over: the bar's title on that page, in the title's ink too.
    Driving,
    /// Its task waits, stopped or broke off, or the user holds it for a task that went on.
    Paused,
}

impl PageWord {
    pub(super) fn label(self) -> &'static str {
        match self {
            PageWord::Working => "Working",
            PageWord::Done => "Done",
            PageWord::NeedsYou => NEEDS_YOU,
            PageWord::Driving => agent_bar::DRIVING,
            PageWord::Paused => "Paused",
        }
    }
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
/// they fit beside the slot, else as many as do, the one the page area shows first, so what's
/// shown is always in the sidebar, then by rank and newest first.
fn choose(
    cards: &[&Card],
    all: bool,
    slot: bool,
    shown: Option<&SharedString>,
) -> (Vec<SharedString>, usize) {
    let room = MOST - usize::from(slot);
    let more = cards.len().saturating_sub(room);
    if all || more == 0 {
        return (cards.iter().map(|card| card.agent.clone()).collect(), more);
    }
    let mut ranked: Vec<&Card> = cards.to_vec();
    ranked.sort_by_key(|card| {
        (
            Some(&card.agent) != shown,
            rank(card.dot),
            Reverse(number(&card.agent)),
        )
    });
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

/// What a group at the foot holds.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Holds {
    /// "3 more tasks", which opens the foot to all of them, or, opened, "Show less".
    More,
    /// ⌘T's empty slot, which its task's header fills.
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

/// A line of text, and the one it took over from, going since when.
struct Line {
    text: SharedString,
    was: Option<(SharedString, Instant)>,
}

impl Line {
    fn new(text: SharedString) -> Self {
        Self { text, was: None }
    }

    /// Takes `text`, the old one going from `now` if `animate`. Returns whether the words changed.
    fn set(&mut self, text: SharedString, now: Instant, animate: bool) -> bool {
        let changed = text != self.text;
        if changed {
            let old = std::mem::replace(&mut self.text, text);
            self.was = animate.then_some((old, now));
        }
        changed
    }

    /// The old line and how far it has gone in `length` seconds, while it goes.
    fn going(&self, now: Instant, length: f32) -> Option<(&SharedString, f32)> {
        let (text, since) = self.was.as_ref()?;
        let t = now.saturating_duration_since(*since).as_secs_f32() / length;
        (t < 1.).then_some((text, t))
    }
}

/// What a task's group shows.
struct Face {
    card: Card,
    /// Whether its pages' rows show: its task is the one the page area shows.
    open: bool,
    title: Line,
    /// The state's line.
    status: Line,
    glyph: Glyph,
    /// The state's line's characters and its width, for the shimmer.
    marks: Vec<(Range<usize>, f32)>,
    width: f32,
}

impl Face {
    fn new(card: &Card, open: bool, now: Instant, still: bool, system: &WindowTextSystem) -> Self {
        let status = state_line(card);
        let (marks, width) = marks(&status, system);
        Self {
            title: Line::new(card.title.clone()),
            status: Line::new(status),
            glyph: Glyph::new(card.dot.into(), now, still),
            marks,
            width,
            card: card.clone(),
            open,
        }
    }

    /// Takes `card`'s changes: a new title crossfades and a new state rolls in.
    fn update(
        &mut self,
        card: &Card,
        open: bool,
        now: Instant,
        still: bool,
        animate: bool,
        system: &WindowTextSystem,
    ) {
        self.title.set(card.title.clone(), now, animate);
        let status = state_line(card);
        if self.status.set(status.clone(), now, animate) {
            (self.marks, self.width) = marks(&status, system);
        }
        self.glyph.set(card.dot.into(), now, still);
        self.card = card.clone();
        self.open = open;
    }

    fn moving(&self, now: Instant, still: bool) -> bool {
        let roll = if still { CROSSFADE } else { ROLL };
        let title = if still { CROSSFADE } else { TITLE_FADE };
        self.title.going(now, title).is_some()
            || self.status.going(now, roll).is_some()
            || self.glyph.moving(now)
    }

    /// Whether it draws on the loading dots' clock: its dot circling and its state shimmering.
    fn live(&self) -> bool {
        self.glyph.circling()
    }
}

/// A group at the foot: a task's header with its chip and its pages' rows, the count's row, or the
/// slot.
struct Group {
    holds: Holds,
    /// Its height, which grows from nothing as it comes, to its header's and its pages' as they
    /// change, and to nothing as it goes, and its opacity.
    height: Ease,
    opacity: Ease,
    /// The landing plate's fill and its content: the slot's are nothing until the bar lands in it,
    /// and the fill goes again as the header comes, as a header has no plate at rest.
    fill: Ease,
    content: Ease,
    leaving: bool,
    /// Since when a copy of its header flies into the Archive button, from where it was in the
    /// sidebar, as it goes.
    archived: Option<(Instant, Bounds<Pixels>)>,
    face: Option<Face>,
}

impl Group {
    /// A group coming in: from nothing, fading in, unless the window hasn't drawn its foot yet. A
    /// slot comes as its dashes, with no fill.
    fn new(holds: Holds, now: Instant, still: bool, animate: bool) -> Self {
        let empty = holds == Holds::Slot;
        let mut group = Group {
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
            group.opacity = Ease::rest(0.);
            let (length, curve) = if still {
                (CROSSFADE, Curve::Linear)
            } else {
                (FADE, Curve::Ease)
            };
            group.opacity.go(1., now, length, 0., curve);
        }
        group
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

    /// The room it takes at `now`: groups abut, as the list's rows do.
    fn room(&self, now: Instant) -> f32 {
        self.height.at(now)
    }

    /// The room it will take once it's still.
    fn end_room(&self) -> f32 {
        self.height.end()
    }

    /// The landing plate's fill at `now`, of a tile's, and how much of the slot's dashes show.
    fn landing(&self, now: Instant) -> (f32, f32) {
        let fill = self.fill.at(now);
        (fill * (1. - self.content.at(now)), 1. - fill)
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

/// The foot's groups and what moves on it.
pub(super) struct Foot {
    /// Top first.
    groups: Vec<Group>,
    /// ⌘T's ask shows over the window, so the foot holds its slot.
    slot: bool,
    /// The user opened the foot to every task.
    all: bool,
    /// The header under the pointer, by its group's key.
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
    /// Where each header's plate was last drawn in the window, for its flight into the Archive and
    /// for check.sh, by its group's key, and each page's row, by its tab, which its page zooms
    /// out of where the task's view has no picture of it.
    plates_drawn: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    rows_drawn: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// Where each was last logged at rest, by its log name: a plate's last frame on its way can
    /// already be where it rests.
    rested: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// Where the strip under a task's page drew its Open, in the window.
    pub(super) strip_open: Rc<Cell<Option<Bounds<Pixels>>>>,
    scroll: ScrollHandle,
    /// When the shimmer's sweeps began.
    epoch: Instant,
    /// The groups as last logged.
    logged: String,
}

impl Default for Foot {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
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
            rows_drawn: Rc::default(),
            rested: Rc::default(),
            strip_open: Rc::default(),
            scroll: ScrollHandle::new(),
            epoch: clock(),
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
        // An ended task kept its group for its pages: it goes as the last of them closes.
        let mut emptied = Vec::new();
        for card in &cards {
            let was = self.task_cards.iter().find(|old| old.agent == card.agent);
            waits |= card.dot == Dot::Waiting && was.is_none_or(|was| was.dot != Dot::Waiting);
            if card.finished()
                && card.pages.is_empty()
                && was.is_some_and(|was| was.finished() && !was.pages.is_empty())
            {
                emptied.push(card.agent.clone());
            }
            if was.is_none_or(|was| (&was.status, was.dot) != (&card.status, card.dot)) {
                // check.sh reads this line.
                log!(
                    "browser: task card {} {:?} {}",
                    card.agent,
                    card.dot,
                    card.status
                );
            }
            if was.is_none_or(|was| was.title != card.title) {
                // check.sh reads this line: a title is one line, written out (#3690).
                log!("browser: task card {} titled {}", card.agent, card.title);
            }
        }
        let was: Vec<SharedString> = self
            .task_cards
            .iter()
            .map(|card| card.agent.clone())
            .collect();
        let fresh = |card: &&Card| !was.contains(&card.agent);
        let ran = self
            .foot
            .groups
            .iter()
            .any(|group| group.holds == Holds::Slot && !group.leaving && group.fill.end() > 0.);
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
        self.area_takes_space();
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
        for agent in emptied {
            self.fold_later(agent, 0.);
        }
        self.sync_foot(took, cx);
        // The task the Archive brought back shows its conversation as its card comes back.
        if let Some(agent) = self.show_restored.take()
            && self.task_cards.iter().any(|card| card.agent == agent)
        {
            self.show_task(agent, cx);
        }
        // The composer's ↩ shows the task it started, as its Space has no page to stay on; ⌘T's
        // ask over a page leaves the page, and its card comes in at the foot.
        let started = |card: &&Card| match &self.show_started {
            Some(ShowStarted::Asked(home)) => card.home == *home && !was.contains(&card.agent),
            Some(ShowStarted::Task(agent)) => card.agent == *agent,
            None => false,
        };
        if let Some(card) = self.task_cards.iter().find(started) {
            let (agent, home) = (card.agent.clone(), card.home);
            self.show_started = None;
            if self.selected == 0 && self.space() == home && self.talk_agent().is_none() {
                self.show_task(agent, cx);
            }
        }
        cx.notify();
    }

    /// ⌘T's ask shows over this window, or no longer does: the foot opens an empty slot for the
    /// task's header, or closes it.
    pub fn set_ask_slot(&mut self, shown: bool, cx: &mut Context<Self>) {
        if shown == self.foot.slot {
            return;
        }
        log!("browser: ask slot {}", shown as u8);
        self.foot.slot = shown;
        self.sync_foot(None, cx);
        cx.notify();
    }

    /// The page area shows another task's view, or none: that task's group opens to its pages and
    /// the others close to their headers.
    pub(super) fn reshape_foot(&mut self, cx: &mut Context<Self>) {
        self.sync_foot(None, cx);
    }

    /// The glyph's colours: Signal, a header's title's ink and its state line's, and the stopped
    /// ring's, the ink at `stopped`, under the floor text keeps, as it's a mark beside words that
    /// say the task stopped.
    fn glyph_inks(&self) -> glyph::Inks {
        glyph::Inks {
            signal: rgb(STATUS_AGENT).into(),
            title: self.palette.text(TITLE),
            status: self.palette.gray(STATUS),
            stopped: Hsla::from(self.palette.ink).opacity(STOPPED),
        }
    }

    /// A finished task's mark at rest, as its header shows it: Done's tick, or the grey ring of
    /// one stopped or failed, in the glyph's box.
    pub(super) fn status_mark(&self, dot: Dot) -> impl IntoElement + use<> {
        let now = clock();
        let drawn = Glyph::new(State::from(dot), now, true).drawn(now);
        let inks = self.glyph_inks();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| glyph::paint(&drawn, bounds, inks, false, window),
        )
        .size(px(GLYPH))
    }

    /// The glyph of `agent`'s header as the foot draws it now, for the strip under its page watched
    /// to lead with, so the two read as one: its header's at rest if the foot doesn't show it.
    pub(super) fn task_glyph(&self, agent: &SharedString, dot: Dot) -> gpui::AnyElement {
        let now = clock();
        let face = (self.foot.groups.iter())
            .find(|group| group.holds == Holds::Task(agent.clone()) && !group.leaving)
            .and_then(|group| group.face.as_ref());
        let Some(face) = face else {
            return self.status_mark(dot).into_any_element();
        };
        let drawn = face.glyph.drawn(now);
        let inks = self.glyph_inks();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| glyph::paint(&drawn, bounds, inks, false, window),
        )
        .flex_none()
        .size(px(GLYPH))
        .into_any_element()
    }

    /// Brings the groups to the cards and the slot: those no longer wanted fold away, new ones
    /// grow in, and each task's group takes its card's changes, open to its pages if its task is
    /// the one the page area shows. `took` is the new card that fills the slot's group.
    fn sync_foot(&mut self, took: Option<SharedString>, cx: &mut Context<Self>) {
        let now = clock();
        let still = crate::motion::still(cx);
        let animate = self.foot.primed;
        let moves = animate && !still;
        self.foot.still = still;
        let system = (self.text_system.take())
            .unwrap_or_else(|| WindowTextSystem::new(cx.text_system().clone()));
        let cards = self.task_cards.clone();
        let listed: Vec<&Card> = cards.iter().collect();
        let shown = self.talk_agent();
        let (chosen, more) = choose(&listed, self.foot.all, self.foot.slot, shown.as_ref());
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
        wanted.extend(chosen.into_iter().map(Holds::Task));
        let foot = &mut self.foot;
        if let Some(agent) = took
            && wanted.contains(&Holds::Task(agent.clone()))
            && let Some(group) =
                (foot.groups.iter_mut()).find(|group| group.holds == Holds::Slot && !group.leaving)
        {
            group.holds = Holds::Task(agent);
        }
        for group in &mut foot.groups {
            match (wanted.contains(&group.holds), group.leaving) {
                (false, false) => group.leave(now, still, animate),
                (true, true) => group.stay(now, still),
                _ => {}
            }
        }
        for holds in &wanted {
            if !foot.groups.iter().any(|group| &group.holds == holds) {
                foot.groups
                    .push(Group::new(holds.clone(), now, still, animate));
            }
        }
        foot.groups.sort_by_key(|group| Reverse(group.holds.key()));
        for group in foot.groups.iter_mut().filter(|group| !group.leaving) {
            let Holds::Task(agent) = group.holds.clone() else {
                group.grow(ROW_HEIGHT, now, moves);
                continue;
            };
            let Some(card) = cards.iter().find(|card| card.agent == agent) else {
                continue;
            };
            let open = shown.as_ref() == Some(&agent);
            group.grow(group_height(card, open), now, moves);
            match &mut group.face {
                Some(face) => face.update(card, open, now, still, animate, &system),
                None => group.face = Some(Face::new(card, open, now, still, &system)),
            }
            // A slot that the bar didn't fold into, such as a task an agent asked for, shows its
            // header at once.
            if group.fill.end() == 0. {
                let length = if animate { CONTENT } else { 0. };
                group.fill.go(1., now, length, 0., Curve::Ease);
                group.content.go(1., now, length, 0., Curve::Ease);
            }
        }
        self.text_system = Some(system);
        let groups: Vec<String> = (self.foot.groups.iter())
            .filter(|group| !group.leaving)
            .map(|group| match &group.holds {
                Holds::More => format!("more {more}"),
                Holds::Slot => "slot".into(),
                Holds::Task(agent) => agent.to_string(),
            })
            .collect();
        let groups = groups.join(" ");
        if groups != self.foot.logged {
            // check.sh reads this line.
            log!(
                "browser: task foot {}",
                if groups.is_empty() { "empty" } else { &groups }
            );
            self.foot.logged = groups;
        }
    }

    /// Where the media players end, over Restart to Update, the foot's groups over them.
    pub(super) fn media_top(&self) -> f32 {
        self.over_safe_start() + self.update_banner_room() + self.media_room()
    }

    /// The most room the foot takes before it scrolls, a share of the window's height.
    fn foot_most(&self) -> f32 {
        match f32::from(self.viewport.height) {
            height if height > 0. => FOOT_SHARE * height,
            _ => f32::INFINITY,
        }
    }

    /// The room the foot is held to, while it is: opened to every task, or an open group's pages
    /// taking it past its share. At rest closed, three headers never reach it.
    pub(super) fn foot_cap(&self) -> Option<f32> {
        let most = self.foot_most();
        let live = (self.foot.groups.iter()).filter(|group| !group.leaving);
        let total: f32 = live.map(Group::end_room).sum();
        (self.foot.all || total > most).then_some(most)
    }

    /// The room the foot takes at `now`.
    fn foot_room(&self, now: Instant) -> f32 {
        let total: f32 = self.foot.groups.iter().map(|group| group.room(now)).sum();
        total.min(self.foot_most())
    }

    /// The room the foot takes now, which the list ends over.
    pub(super) fn foot_height(&self) -> f32 {
        self.foot_room(clock())
    }

    /// Whether the sidebar shows the foot where it rests.
    fn foot_drawn(&self) -> bool {
        !self.page_fullscreen
            && !self.sidebar_hidden
            && self.sidebar_motion.is_none()
            && self.library.is_none()
            && self.form().is_none()
    }

    /// Whether the foot draws `agent`'s group, where the ring can sit.
    pub(super) fn foot_holds(&self, agent: &SharedString) -> bool {
        let task = Holds::Task(agent.clone());
        self.foot_drawn()
            && (self.foot.groups.iter()).any(|group| group.holds == task && !group.leaving)
    }

    /// What in the foot the page area shows, the one thing there with the shown plate: the row of
    /// the task's page watched, else the header of the task whose view shows. The list's own shown
    /// row has no plate meanwhile (`shown_has_plate`), so the sidebar never shows two.
    pub(super) fn shown_at_foot(&self) -> Option<StripStop> {
        let agent = self.talk_agent()?;
        Some(match self.watched_task_page() {
            Some((_, tab)) => StripStop::Row(tab),
            None => StripStop::Header(number(&agent)),
        })
    }

    /// The foot's stops for the keys' ring, top first: the count's row, and each task's header,
    /// its chip and, where its group is open, its pages' rows. None while the sidebar doesn't
    /// draw the foot, so the ring never sits where it can't be seen.
    pub(super) fn foot_stops(&self) -> Vec<StripStop> {
        if !self.foot_drawn() {
            return Vec::new();
        }
        let mut stops = Vec::new();
        for group in self.foot.groups.iter().filter(|group| !group.leaving) {
            match (&group.holds, &group.face) {
                (Holds::More, _) => stops.push(StripStop::More),
                (Holds::Task(agent), Some(face)) => {
                    let n = number(agent);
                    stops.push(StripStop::Header(n));
                    stops.extend(chip(&face.card).map(|_| StripStop::Chip(n)));
                    if face.open {
                        stops.extend(face.card.pages.iter().map(|&tab| StripStop::Row(tab)));
                    }
                }
                _ => {}
            }
        }
        stops
    }

    /// The task numbered `n`, as its header's stop names it.
    fn foot_agent(&self, n: u64) -> Option<SharedString> {
        (self.task_cards.iter())
            .find(|card| number(&card.agent) == n)
            .map(|card| card.agent.clone())
    }

    /// Return on one of the foot's stops: what a click on it does.
    pub(super) fn key_foot_show(&mut self, stop: StripStop, cx: &mut Context<Self>) {
        match stop {
            StripStop::Header(n) => {
                if let Some(agent) = self.foot_agent(n) {
                    self.leave_strip("header", cx);
                    log!("browser: command show-task via strip keys");
                    self.show_group(agent, cx);
                }
            }
            StripStop::Chip(n) => {
                let card = self.task_cards.iter().find(|card| number(&card.agent) == n);
                if let Some((agent, chip)) =
                    card.and_then(|card| Some((card.agent.clone(), chip(card)?)))
                {
                    self.run_chip(agent, chip, cx);
                }
            }
            // The ring stays on the row, whose page shows: the keys never go to an agent's page.
            StripStop::Row(tab) => {
                if self.watch_row(tab, false, cx) && self.agents_page_watched() {
                    self.focus_keys = true;
                }
            }
            StripStop::More => self.toggle_foot(cx),
            _ => {}
        }
    }

    /// A header's click: its task's view, which opens its group, and which a page of its watched
    /// goes back to. On the header whose view shows, it puts the view away.
    fn show_group(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        if self.talk_agent().as_ref() == Some(&agent) && self.watched_task_page().is_some() {
            return self.task_page_back(cx);
        }
        self.show_task(agent, cx);
    }

    /// A page's row's click: stands on that page, as its picture's click in the task's view does,
    /// and nothing else. Says whether it did. The page zooms out of its picture there, or of its
    /// row where the view has drawn none. `by_ilo` when ilo opens it by itself, as at a handoff.
    pub(super) fn watch_row(&mut self, tab: u64, by_ilo: bool, cx: &mut Context<Self>) -> bool {
        if self
            .watched_task_page()
            .is_some_and(|(_, shown)| shown == tab)
        {
            return false;
        }
        let row = self.foot.rows_drawn.borrow().get(&tab).copied();
        let (from, picture) =
            (self.talk_thumb(tab, cx)).unwrap_or_else(|| (row.unwrap_or_default(), None));
        self.watch_task_page(tab, from, picture, by_ilo, cx)
    }

    /// A chip's click: the user's own act on the task.
    fn run_chip(&mut self, agent: SharedString, chip: Chip, cx: &mut Context<Self>) {
        match chip {
            Chip::Retry => self.browser.task(agent, TaskAction::Retry, cx),
            Chip::Assistant(cli, Offer::Get) => {
                log!("browser: assistant install page {} via sidebar", cli.id());
                self.browser.open_page(cli.install_page(), cx);
            }
            Chip::Assistant(_, Offer::Cancel) => {
                self.browser.task(agent, TaskAction::CancelSignIn, cx)
            }
            Chip::Assistant(..) => self.browser.task(agent, TaskAction::SignIn, cx),
        }
    }

    /// What `tab` is to `card`'s task now: its word, of the five a task's page has, and whether
    /// it has the outline. The row at the sidebar's foot, a tile in the rail and the card all read
    /// this one answer, so a page's word and its outline never disagree. While the task works,
    /// each of its pages says Working and the outline alone marks the one it's in; Done is a
    /// page's word only once the task is done, as a page it left may be come back to. At a handoff
    /// the outline is on the page that needs the user. A page the user took over is theirs, one it
    /// handed them and went on from is Paused, as are a stopped task's; a stopped or done task has
    /// no outline.
    pub(super) fn page_mark(&self, card: &Card, tab: u64) -> PageMark {
        let mark = |word, outline| PageMark { word, outline };
        match self.held_why(tab) {
            // What the task asked of them there: a page they took over has none.
            Some(asked) if asked.is_empty() => return mark(PageWord::Driving, false),
            Some(_) if card.word == Word::NeedsYou => return mark(PageWord::NeedsYou, true),
            Some(_) => return mark(PageWord::Paused, false),
            None => {}
        }
        match card.word {
            Word::Working => {
                // Its own step of the turn under way, where it works on more than one page.
                let in_it = match card.page_steps.is_empty() {
                    false => card.page_steps.iter().any(|(each, _)| *each == tab),
                    true => card.pages.len() == 1 || self.agents_at.get(&card.agent) == Some(&tab),
                };
                mark(PageWord::Working, in_it)
            }
            Word::Done => mark(PageWord::Done, false),
            Word::NeedsYou | Word::Stopped | Word::Interrupted | Word::AtLimit => {
                mark(PageWord::Paused, false)
            }
        }
    }

    /// `tab`'s word, from `page_mark`.
    pub(super) fn page_word(&self, card: &Card, tab: u64) -> PageWord {
        self.page_mark(card, tab).word
    }

    /// Where ⌘T's bar folds to as it runs a task: the slot's plate as it will rest, a header's, in
    /// the window, if the sidebar shows it, with its corners' radius and its fill.
    pub fn fold_target(&self) -> Option<(Bounds<Pixels>, f32, Hsla)> {
        if !self.foot_drawn() || !self.foot.slot {
            return None;
        }
        let live = || self.foot.groups.iter().filter(|group| !group.leaving);
        let index = live().position(|group| group.holds == Holds::Slot)?;
        let total: f32 = live().map(Group::end_room).sum();
        let room = total.min(self.foot_most());
        let above: f32 = live().take(index).map(Group::end_room).sum();
        let scrolled = f32::from(self.foot.scroll.offset().y);
        let top = f32::from(self.viewport.height) - self.media_top() - room
            + scrolled
            + above
            + PLATE_TOP;
        let origin = self.sidebar_origin.get();
        let bounds = Bounds::new(
            point(origin.x + px(PLATE_INSET), origin.y + px(top)),
            gpui::size(px(self.width() - 2. * PLATE_INSET), px(PLATE_HEIGHT)),
        );
        Some((bounds, PLATE_RADIUS, self.palette.plate(TILE)))
    }

    /// The bar has gone for the task: folded into the slot, whose fill takes over as it lands and
    /// goes as the header fades in then, or, where it couldn't fold, hidden, and the header fades
    /// in at once.
    pub fn fold_started(&mut self, folded: bool, cx: &mut Context<Self>) {
        let now = clock();
        let still = crate::motion::still(cx);
        let Some(group) = (self.foot.groups.iter_mut())
            .find(|group| group.holds == Holds::Slot && !group.leaving)
        else {
            return;
        };
        match (folded, still) {
            (true, false) => {
                group.fill.go(1., now, 0., FOLD, Curve::Ease);
                group.content.go(1., now, CONTENT, FOLD, Curve::Ease);
            }
            (_, true) => {
                group.fill.go(1., now, CROSSFADE, 0., Curve::Linear);
                group.content.go(1., now, CROSSFADE, 0., Curve::Linear);
            }
            (false, false) => {
                group.fill.go(1., now, CONTENT, 0., Curve::Ease);
                group.content.go(1., now, CONTENT, 0., Curve::Ease);
            }
        }
        cx.notify();
    }

    /// Once a frame is built: drops what has gone, starts Open's pages' flights once the list has
    /// their rows, and asks for the next frame while anything moves. A working task's dot and
    /// shimmer draw on the loading dots' clock instead, as they move all the while it works.
    pub(super) fn step_foot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = clock();
        let still = crate::motion::still(cx);
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
        let before = self.foot.groups.len();
        self.foot
            .groups
            .retain(|group| !(group.leaving && !group.moving(now, still)));
        if self.foot.groups.len() != before {
            self.foot.plates_drawn.borrow_mut().clear();
            self.foot.rows_drawn.borrow_mut().clear();
            self.foot.rested.borrow_mut().clear();
        }
        for group in &mut self.foot.groups {
            group.settle(now);
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
                .groups
                .iter()
                .any(|group| group.moving(now, still))
    }

    /// Whether a working task draws on the loading dots' clock.
    pub(super) fn foot_live(&self) -> bool {
        let face = |group: &Group| group.face.as_ref().is_some_and(Face::live);
        self.foot
            .groups
            .iter()
            .any(|group| !group.leaving && face(group))
    }

    /// Whether `tab`'s row waits for its page's flight to land, which draws it meanwhile.
    pub(super) fn landing(&self, tab: u64) -> bool {
        let now = clock();
        let flight = seconds(GHOST + GHOST_LAND);
        self.foot
            .opening
            .as_ref()
            .is_some_and(|opening| opening.tabs.contains(&tab))
            || (self.foot.ghosts.iter()).any(|ghost| ghost.tab == tab && now < ghost.start + flight)
    }

    /// The strip's Open: the page goes to Today, flying there from `from`, the Open that was
    /// clicked, in the window, as a row grows into its place.
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
        // Its last page opened, a finished task has nothing left in its group, which folds into
        // the Archive once its pages have landed.
        let last = card.open.iter().all(|(each, _)| tabs.contains(each));
        if card.finished() && last {
            self.fold_later(agent.clone(), 0.);
        }
        if let Some(from) = from
            && !crate::motion::still(cx)
            && !tabs.is_empty()
        {
            let origin = self.sidebar_origin.get();
            let from = Bounds::new(from.origin - origin, from.size);
            self.foot.opening = Some(Opening {
                agent: agent.clone(),
                tabs,
                from,
                at: clock(),
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
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        self.closed_task = Some((agent, now.unwrap_or_default().as_secs_f64() * 1000.));
    }

    /// The user left `agent`'s conversation: a finished task they've now seen folds into the
    /// Archive, so the foot keeps only what's live or unread. One with pages still open keeps its
    /// group until the last of them closes.
    pub(super) fn talk_left(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        if (self.task_cards.iter())
            .any(|card| card.agent == agent && card.finished() && card.pages.is_empty())
        {
            self.fold_later(agent, ARCHIVE_LEAVE);
            cx.notify();
        }
    }

    /// Folds `agent`'s group into the Archive `delay` seconds from now, or once its pages land.
    fn fold_later(&mut self, agent: SharedString, delay: f32) {
        self.foot.folds.retain(|(each, _)| each != &agent);
        self.foot.folds.push((agent, clock() + seconds(delay)));
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
    /// back: its group goes at once and the groups above close its room, as a copy of its header
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
        let now = clock();
        let flies =
            self.foot.primed && !crate::motion::still(cx) && !self.one_space() && self.foot_drawn();
        let from = self
            .foot
            .plates_drawn
            .borrow()
            .get(&number(&agent))
            .copied();
        let group = (self.foot.groups.iter_mut())
            .find(|group| group.holds == Holds::Task(agent.clone()) && !group.leaving);
        if flies
            && let Some(from) = from
            && let Some(group) = group
        {
            let origin = self.sidebar_origin.get();
            group.archived = Some((now, Bounds::new(from.origin - origin, from.size)));
            group.leaving = true;
            group.opacity = Ease::rest(0.);
            group
                .height
                .go(0., now, ARCHIVE_CLOSE, ARCHIVE_CLOSE_DELAY, Curve::Ease);
            self.foot.bump = Some(now + seconds(ARCHIVE));
        }
        log!("browser: task {agent} archived");
        self.dismiss_task(agent.clone(), cx);
        // The Agents take its card away; the foot doesn't wait for them, so a change meanwhile
        // can't bring its group back.
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

    /// The copies of finished tasks' headers on their way into the Archive button: each from its
    /// header's place to the button's box, its corners to the box's, its content fading at once
    /// and its plate over the end of the flight.
    pub(super) fn archive_flights(&self) -> Vec<AnyElement> {
        let now = clock();
        let to = Bounds::new(
            point(
                px(LIBRARY_BACK_INSET),
                self.viewport.height - px((FOOTER_HEIGHT - NAV_SIZE) / 2. + NAV_SIZE),
            ),
            gpui::size(px(NAV_SIZE), px(NAV_SIZE)),
        );
        let flights = self.foot.groups.iter().filter_map(|group| {
            let (since, from) = group.archived?;
            let face = group.face.as_ref()?;
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
            // The copy has a tile's fill, as it's a thing on its way, not a row at rest.
            let flight = div()
                .absolute()
                .left(lerp(from.origin.x, to.origin.x))
                .top(lerp(from.origin.y, to.origin.y))
                .w(lerp(from.size.width, to.size.width))
                .h(lerp(from.size.height, to.size.height))
                .rounded(px(PLATE_RADIUS + (NAV_RADIUS - PLATE_RADIUS) * way))
                .overflow_hidden()
                .bg(self.palette.plate(TILE))
                .opacity(opacity)
                .children((content > 0.).then(|| self.archived_face(face, from, content, now)));
            Some(flight.into_any_element())
        });
        flights.collect()
    }

    /// A header's content as its copy takes it into the Archive: laid out at the header's size,
    /// clipped as the copy shrinks, at `alpha`, and taking no clicks.
    fn archived_face(
        &self,
        face: &Face,
        from: Bounds<Pixels>,
        alpha: f32,
        now: Instant,
    ) -> AnyElement {
        let status = div()
            .mt(px(STATUS_GAP))
            .h(px(STATUS_LINE))
            .text_size(px(text::SMALL))
            .line_height(px(STATUS_LINE))
            .text_color(self.palette.gray(SECONDARY))
            .truncate()
            .child(face.status.text.clone());
        let text = div()
            .absolute()
            .left(px(COLUMN))
            .right(px(TITLE_END_INSET))
            .top(px(TEXT_TOP))
            .flex()
            .flex_col()
            .child(div().h(px(LINE)).flex().items_center().child(title(
                &self.palette,
                face.title.text.clone(),
                false,
            )))
            .child(status);
        div()
            .absolute()
            .left_0()
            .top_0()
            .w(from.size.width)
            .h(from.size.height)
            .opacity(alpha)
            .child(self.header_glyph(face, false, now))
            .child(text)
            .into_any_element()
    }

    /// The foot, over the players, as tall as the room its groups take, scrolling past its share
    /// of the sidebar.
    pub(super) fn task_foot(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.foot.groups.is_empty() {
            return None;
        }
        let now = clock();
        let room = self.foot_room(now);
        let total: f32 = self.foot.groups.iter().map(|group| group.room(now)).sum();
        let groups: Vec<AnyElement> = (self.foot.groups.iter())
            .map(|group| self.foot_group(group, now, cx))
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
        Some(column.children(groups).into_any_element())
    }

    /// A group: its room, clipped to it as it opens and closes, and what it holds.
    fn foot_group(&self, group: &Group, now: Instant, cx: &mut Context<Self>) -> AnyElement {
        let frame = div()
            .flex_none()
            .h(px(group.height.at(now)))
            .relative()
            .overflow_hidden()
            .opacity(group.opacity.at(now));
        match (&group.holds, &group.face) {
            (Holds::Task(_), Some(face)) => frame
                .child(self.task_header(group, face, now, cx))
                .children(self.task_chip(group, face, now, cx))
                .children(self.task_pages(group, face, now, cx))
                .into_any_element(),
            (Holds::More, _) => frame
                .child(self.more_row(group, now, cx))
                .into_any_element(),
            // The slot: the bar's plate lands in it, and its fill takes over.
            _ => {
                let key = group.holds.key();
                let rest = !group.moving(now, self.foot.still);
                let drawn = drawn_listener(&self.foot.plates_drawn, &self.foot.rested, key, rest);
                let (fill, _) = group.landing(now);
                frame
                    .child(
                        self.header_plate(group, now)
                            .bg(self.palette.plate(TILE).opacity(fill))
                            .child(drawn),
                    )
                    .into_any_element()
            }
        }
    }

    /// A header's plate's box in its group, a tab's row's, with the slot's dashes while they show.
    fn header_plate(&self, group: &Group, now: Instant) -> gpui::Div {
        // A slot not yet filled shows its place in dashes, which go as its fill comes.
        let (_, dashed) = group.landing(now);
        let dashes = (dashed > 0.).then(|| {
            let color = self.palette.glyph(SLOT_DASH);
            dashed_plate(color.opacity(dashed), PLATE_RADIUS)
                .absolute()
                .inset_0()
        });
        div()
            .absolute()
            .top(px(PLATE_TOP))
            .left(px(PLATE_INSET))
            .right(px(PLATE_INSET))
            .h(px(PLATE_HEIGHT))
            .rounded(px(PLATE_RADIUS))
            .children(dashes)
    }

    /// "3 more tasks", which opens the foot to every task, or, opened, "Show less": a row's words
    /// on the title's column.
    /// What the row over the headers says: how many tasks the three places leave out, counted in
    /// headers whatever the open group's rows take, or the way back once they all show.
    fn more_words(&self) -> SharedString {
        let listed = self.task_cards.len();
        let more = listed.saturating_sub(MOST - usize::from(self.foot.slot));
        match (self.foot.all, more) {
            (true, _) => "Show less".into(),
            (false, 1) => "1 more task".into(),
            (false, more) => format!("{more} more tasks").into(),
        }
    }

    fn more_row(&self, group: &Group, now: Instant, cx: &mut Context<Self>) -> AnyElement {
        let words = self.more_words();
        let key = Holds::More.key();
        let hovered = self.foot.hovered == Some(key);
        let rest = !group.moving(now, self.foot.still);
        let drawn = drawn_listener(&self.foot.plates_drawn, &self.foot.rested, key, rest);
        let row = self
            .header_plate(group, now)
            .when(hovered, |row| row.bg(self.palette.hovered_plate()))
            .id("task-more");
        // A stop is placed in the flow; this one keeps its plate's place in its row.
        self.strip_stop(row, StripStop::More, words.clone(), PLATE_RADIUS)
            .absolute()
            .pl(px(COLUMN))
            .flex()
            .items_center()
            .text_size(px(text::SMALL))
            .text_color(match hovered {
                true => self.palette.gray_on_plate(SECONDARY),
                false => self.palette.gray(SECONDARY),
            })
            .child(drawn)
            .child(words)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                shell.hover_header(key, *hovered, cx);
            }))
            .on_click(cx.listener(|shell, _, _, cx| shell.toggle_foot(cx)))
            .into_any_element()
    }

    fn hover_header(&mut self, key: u64, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.foot.hovered = Some(key);
        } else if self.foot.hovered == Some(key) {
            self.foot.hovered = None;
        }
        cx.notify();
    }

    /// A header's glyph, in the favicon's column of its plate. On the shown plate's fill, `lit`,
    /// the ring is drawn brighter, to read as on the others.
    fn header_glyph(&self, face: &Face, lit: bool, now: Instant) -> impl IntoElement + use<> {
        let drawn = face.glyph.drawn(now);
        let inks = self.glyph_inks();
        let paint = move |bounds, _, window: &mut Window, _: &mut gpui::App| {
            glyph::paint(&drawn, bounds, inks, lit, window)
        };
        canvas(|_, _, _| {}, paint)
            .absolute()
            .left(px(FAVICON_X - PLATE_INSET + FAVICON_SIZE / 2. - GLYPH / 2.))
            .top(px((PLATE_HEIGHT - GLYPH) / 2.))
            .size(px(GLYPH))
    }

    /// A task's header, a sidebar row: the glyph in the favicon's column, the title over the
    /// state's line, and ✕ under the pointer. It has a plate only under the pointer and while
    /// its task's view is what the page area shows.
    fn task_header(
        &self,
        group: &Group,
        face: &Face,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let agent = face.card.agent.clone();
        let key = group.holds.key();
        let still = self.foot.still;
        let hovered = self.foot.hovered == Some(key);
        let shown = self.shown_at_foot() == Some(StripStop::Header(key));
        let (landed, _) = group.landing(now);
        let plate = self.header_plate(group, now);
        let plate = match (shown, hovered) {
            (true, _) => selected_plate(
                &self.palette,
                plate,
                group.fill.at(now),
                f32::from(u8::from(hovered)),
                false,
            ),
            (false, true) => plate.bg(self.palette.hovered_plate()),
            // The bar's plate as it lands, which goes as the header comes.
            (false, false) => plate.bg(self.palette.plate(TILE).opacity(landed)),
        };
        let end = if hovered { HOVER_END } else { TITLE_END_INSET };
        let text = div()
            .absolute()
            .left(px(COLUMN))
            .right(px(end))
            .top(px(TEXT_TOP))
            .flex()
            .flex_col()
            .child(self.task_title(face, shown, now, still))
            .child(self.task_status(face, shown || hovered, now, still));
        let content = div()
            .absolute()
            .inset_0()
            .opacity(group.content.at(now))
            .child(self.header_glyph(face, shown, now))
            .child(text);
        let close = hovered.then(|| {
            let agent = agent.clone();
            self.media_button("task-close", key, "icons/x.svg")
                .absolute()
                .top(px(CLOSE_INSET))
                .right(px(CLOSE_INSET))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    cx.stop_propagation();
                    log!("browser: command dismiss-task via sidebar");
                    shell.dismiss_task(agent.clone(), cx);
                }))
        });
        let rest = !group.moving(now, still);
        let drawn = drawn_listener(&self.foot.plates_drawn, &self.foot.rested, key, rest);
        let name: SharedString = format!("{}, {}", face.card.title, face.status.text).into();
        // ✕ takes its own click before the header does.
        let header = plate.id(("task-plate", key));
        self.strip_stop(header, StripStop::Header(key), name, PLATE_RADIUS)
            .absolute()
            .child(drawn)
            .child(content)
            .children(close)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                shell.hover_header(key, *hovered, cx)
            }))
            .on_click(cx.listener(move |shell, _, _, cx| shell.show_group(agent.clone(), cx)))
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
            Some((old, t)) => (Some(line(old.clone(), 1. - t)), t),
            None => (None, 1.),
        };
        div()
            .relative()
            .h(px(LINE))
            .children(old)
            .child(line(face.title.text.clone(), t))
    }

    /// The state's line: each new one rolling in from under the old, which goes up, or with motion
    /// reduced crossfading. While the task works a highlight sweeps along it. Full access, where
    /// it leads, is in the title's ink, and no other colour. `plated`, its header has a plate
    /// under it, shown or under the pointer, and the line is in the gray that reads on one.
    fn task_status(
        &self,
        face: &Face,
        plated: bool,
        now: Instant,
        still: bool,
    ) -> impl IntoElement + use<> {
        let length = if still { CROSSFADE } else { ROLL };
        let rise = if still { 0. } else { ROLL_RISE };
        let working = face.glyph.state() == State::Working && !still;
        let room = self.width() - 2. * PLATE_INSET - COLUMN - TITLE_END_INSET;
        let phase =
            (now.saturating_duration_since(self.foot.epoch).as_secs_f32() / SHIMMER).fract();
        let led = HighlightStyle {
            color: Some(self.palette.text(TITLE)),
            font_weight: Some(FontWeight::MEDIUM),
            ..HighlightStyle::default()
        };
        let gray = match plated {
            true => self.palette.gray_on_plate(STATUS),
            false => self.palette.gray(STATUS),
        };
        let line = |words: SharedString, top: f32, alpha: f32, shimmers: bool| {
            let lead = state_lead(&words);
            let base = div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(top))
                .opacity(alpha)
                .text_size(px(text::SMALL))
                .line_height(px(STATUS_LINE))
                .text_color(gray)
                .truncate();
            match shimmers {
                true => {
                    let width = face.width.min(room).max(1.);
                    let highlights = face.marks.iter().map(|(range, x)| {
                        let highlight = match range.end <= lead {
                            true => led,
                            false => {
                                let lit = shimmer(*x, width, phase);
                                self.palette.lit_highlight(STATUS, SHIMMER_HIGH, lit)
                            }
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
                    base.child(StyledText::new(words).with_highlights([(0..lead, led)]))
                }
                false => base.child(words),
            }
        };
        let status = &face.status;
        let (old, t) = match status.going(now, length) {
            Some((old, t)) => {
                let t = if still { t } else { motion::eased(t) };
                (Some(line(old.clone(), -rise * t, 1. - t, false)), t)
            }
            None => (None, 1.),
        };
        div()
            .relative()
            .mt(px(STATUS_GAP))
            .h(px(STATUS_LINE))
            .children(old)
            .child(line(status.text.clone(), rise * (1. - t), t, working))
    }

    /// The header's one chip, its words on the title's column: a row's text on the hovered plate,
    /// not a button with a fill of its own.
    fn task_chip(
        &self,
        group: &Group,
        face: &Face,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let chip = chip(&face.card)?;
        let agent = face.card.agent.clone();
        let n = number(&agent);
        // Under the pointer the plate firms by a tile's fill.
        let hover = self.palette.plate(HOVERED + TILE);
        let button = div()
            .id(("task-chip", n))
            .flex_none()
            .min_w_0()
            .h(px(CHIP_HEIGHT))
            .px(px(CHIP_X))
            .flex()
            .items_center()
            .rounded(px(PAGE_RADIUS))
            .bg(self.palette.plate(HOVERED))
            .hover(move |style| style.bg(hover))
            .text_size(px(text::SMALL))
            .font_weight(FontWeight::MEDIUM)
            .text_color(self.palette.text(TITLE));
        let button = self
            .strip_stop(button, StripStop::Chip(n), chip.label(), PAGE_RADIUS)
            .child(div().truncate().child(chip.label()))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                shell.run_chip(agent.clone(), chip, cx);
            }));
        Some(
            div()
                .absolute()
                .top(px(ROW_HEIGHT + PAGES_TOP))
                .left(px(PLATE_INSET + COLUMN - CHIP_X))
                .right(px(PLATE_INSET))
                .flex()
                .opacity(group.content.at(now))
                .child(button)
                .into_any_element(),
        )
    }

    /// The task's pages, a row each under its header, one step in beside a hairline: while its
    /// group is open, and as it closes.
    fn task_pages(
        &self,
        group: &Group,
        face: &Face,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let card = &face.card;
        if card.pages.is_empty() || !(face.open || group.height.moving(now)) {
            return None;
        }
        let line = div()
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(px(PAGES_LINE))
            .bg(self.palette.plate(PAGES_LINE_INK));
        let rows: Vec<AnyElement> = (card.pages.iter())
            .map(|&tab| self.page_row(card, tab, cx))
            .collect();
        Some(
            div()
                .absolute()
                .top(px(ROW_HEIGHT + chip_room(card) + PAGES_TOP))
                .left(px(PAGES_X))
                .right(px(PLATE_INSET))
                .pl(px(PAGES_LINE + PAGES_GAP))
                .flex()
                .flex_col()
                .opacity(group.content.at(now))
                .child(line)
                .children(rows)
                .into_any_element(),
        )
    }

    /// A page's row: its favicon, its title and one word, with Signal's dot before the word while
    /// the task works in it. It has a plate only under the pointer and while its page is what the
    /// page area shows.
    fn page_row(&self, card: &Card, tab: u64, cx: &mut Context<Self>) -> AnyElement {
        let shown = self.shown_at_foot() == Some(StripStop::Row(tab));
        let mark = self.page_mark(card, tab);
        let word = mark.word;
        // Signal's dot is on the page the task is in, where the outline is.
        let working = word == PageWord::Working && mark.outline;
        let row = div()
            .h(px(PAGE_HEIGHT))
            .flex()
            .items_center()
            .gap(px(PAGE_GAP))
            .pl(px(PAGE_START))
            .pr(px(PAGE_END))
            .rounded(px(PAGE_RADIUS));
        let row = match shown {
            true => selected_plate(&self.palette, row, 1., 0., false),
            false => row,
        };
        let hover = self.palette.hovered_plate();
        let pointed: SharedString = format!("task-page-{tab}").into();
        let row = row
            .id(("task-page", tab))
            .group(pointed.clone())
            .when(!shown, |row| row.hover(move |style| style.bg(hover)));
        // The favicon is drawn in its box whole, as a row's is drawn smaller than its column.
        let icon = div()
            .flex_none()
            .size(px(PAGE_FAVICON))
            .flex()
            .items_center()
            .justify_center()
            .child(self.tab_icon_sized(tab, PAGE_FAVICON * FAVICON_SIZE / FAVICON_DRAWN));
        let name = self.title(tab);
        let words = div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(PAGE_TITLE))
            .text_color(self.palette.text(if shown { TITLE } else { PAGE_INK }))
            .child(name.clone());
        let dot = working.then(|| {
            div()
                .flex_none()
                .size(px(PAGE_DOT))
                .rounded_full()
                .bg(rgb(STATUS_AGENT))
        });
        // A word in gray is at this, and on a plate, the shown row's or the one under the pointer,
        // in the gray that reads on one; one the user is needed at or drives is in the title's ink.
        let gray = match word {
            PageWord::NeedsYou | PageWord::Driving => None,
            PageWord::Working => Some(PAGE_WORD_WORKING),
            PageWord::Done | PageWord::Paused => Some(PAGE_WORD),
        };
        let ink = match gray {
            None => self.palette.text(TITLE),
            Some(alpha) if shown => self.palette.gray_on_plate(alpha),
            Some(alpha) => self.palette.gray(alpha),
        };
        let on_plate = gray
            .filter(|_| !shown)
            .map(|alpha| self.palette.gray_on_plate(alpha));
        let state = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(PAGE_DOT_GAP))
            .text_size(px(text::SMALL))
            .text_color(ink)
            .when_some(on_plate, |state, ink| {
                state.group_hover(pointed, move |style| style.text_color(ink))
            })
            .children(dot)
            .child(word.label());
        let drawn = {
            let rows = self.foot.rows_drawn.clone();
            canvas(
                move |bounds, _, _| {
                    rows.borrow_mut().insert(tab, bounds);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
        };
        let said: SharedString = format!("{name}, {}", word.label()).into();
        self.strip_stop(row, StripStop::Row(tab), said, PAGE_RADIUS)
            .child(drawn)
            .child(icon)
            .child(words)
            .child(state)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                shell.watch_row(tab, false, cx);
            }))
            .into_any_element()
    }

    /// Open's pages on their way from the strip's Open to their rows at the top of Today: each a
    /// chip's plate growing into a row's hover plate, its favicon and title fading in, the row
    /// showing as it lands.
    pub(super) fn task_ghosts(&self) -> Vec<AnyElement> {
        let now = clock();
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
    /// Where `agent`'s header's plate was last drawn in the window.
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

/// Records where a header's plate is drawn in the window, and logs it once it's at rest there.
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

fn logged(bounds: Bounds<Pixels>) -> String {
    format!(
        "{:.0} {:.0} {:.0} {:.0}",
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height)
    )
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
