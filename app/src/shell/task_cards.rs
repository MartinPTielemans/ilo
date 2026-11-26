//! The window's agent tasks as cards in Today's list, among the tabs (spec-thread-card): one
//! outlined card a task, in its home Space, standing above the Today tab that was on top when it
//! came, so the tabs opened after it go above it. A card is never a tab: the walkers, drag and the
//! saved session pass it by, and it has no ✕. Its form is its task's state alone: at work, waiting
//! for the user, done and not yet seen, ended; done or ended and seen, it folds to a line, as one
//! of several at work does. A task that waits for the user lists in every other Space of the window
//! too, first in Today. While ⌘T asks for a task, Today holds an empty slot where its card will
//! come, which the bar folds into. A card done and seen stays until ⌘W in its conversation, or
//! until the profile's Today tabs' ageing takes it as it takes a tab. Nothing draws while the list
//! is at rest: only a card's motion on the display's frames, and the ring at work on the loading
//! dots' clock.

mod card;
mod glyph;
mod motion;
#[cfg(test)]
pub(super) mod tests;

use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use gpui::{
    AnyElement, Bounds, Context, Hsla, Pixels, Role, SharedString, Window, accesskit::Live, canvas,
    div, point, prelude::*, px, rgb,
};

use super::agent_bar;
use super::page_ends::StripStop;
use super::{
    Curve as RowCurve, Edit, FAVICON_X, HOVERED, PLATE_HEIGHT, PLATE_INSET, PLATE_RADIUS,
    ROW_HEIGHT, STATUS_AGENT, Shell, Slot, TITLE, TITLE_END_INSET, TITLE_GAP, What, title,
};
use crate::agents::runner::Cli;
use crate::agents::sign_in::Offer;
use crate::agents::task::{Card, Dot, NEEDS_YOU, TaskAction, Word};
use crate::agents::tier::Tier;
use crate::needs_you::Told;
use crate::palette::FIELD_PLATE as URL_FIELD;
use crate::tokens::shell::task::motion::{
    CONTENT, CROSSFADE, FADE, FOLD, GHOST, GHOST_CONTENT, GHOST_DELAY, GHOST_LAND, GHOST_STAGGER,
};
use crate::tokens::shell::task::{CHIP_HEIGHT, GHOST_RADIUS, GLYPH, STATUS, STOPPED};
use crate::tokens::shell::task_card::{
    BUTTON_HEIGHT, COUNT_LINE, DONE_LINE, FINDING_LINES, GAP, GLYPH as CARD_GLYPH, GLYPH_GAP, ICON,
    LINE, LINE_AROUND, LIVE_LINE, PADDING, TITLE_LINE, UNDER,
};
use glyph::{Look, State};
use motion::{Curve, Ease};

/// Now, for the cards' motion. Tests hold this clock still and move it on themselves, so how far a
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

/// Sets the cards' clock to `at`, where it stays until set again.
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

/// The slot's number among the cards', which no task has.
const SLOT: u64 = u64::MAX - 1;

/// How long Open's pages may take to reach the list before their flight is let go.
const OPENING_WAIT: Duration = Duration::from_secs(3);

/// The one chip on an ended task's card: an act that is the user's and has no page.
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

/// The line a card says its task's state on, but at work and waiting: who works on the task, or
/// Full access where it has that, and its state, in ilo's words alone. The agent's own words are
/// the title's (#3690). The state is the task's one word, in the fuller form its card has for it
/// where that names the same state: "Stopped after 6 steps", "Working · 3 helpers".
fn state_line(card: &Card) -> SharedString {
    let state = match card.fuller {
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

/// What a task's page is to it now, in the words every place that names the page says: its tile's
/// head line, and the waiting card's page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PageWord {
    /// The task works in it now.
    Working,
    Done,
    /// Its task waits for the user here: the page it handed them.
    NeedsYou,
    /// The user took it over: the bar's title on that page.
    Driving,
    /// Its task waits at a refused step or broke off, or the user holds it for a task that went
    /// on.
    Paused,
    /// The user stopped its task: every page of it says so.
    Stopped,
}

impl PageWord {
    pub(super) fn label(self) -> &'static str {
        match self {
            PageWord::Working => "Working",
            PageWord::Done => "Done",
            PageWord::NeedsYou => NEEDS_YOU,
            PageWord::Driving => agent_bar::DRIVING,
            PageWord::Paused => "Paused",
            PageWord::Stopped => "Stopped",
        }
    }
}

/// A card's form, from its task's state alone, whether its conversation has shown since it
/// finished, and whether another task in the same list is at work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Form {
    Working,
    Waits,
    DoneUnseen,
    /// Stopped, interrupted or at its limit, and not yet seen.
    Ended,
    /// At work beside another at work.
    WorkingLine,
    DoneLine,
    EndedLine,
}

impl Form {
    fn of(card: &Card, unseen: bool, several: bool) -> Self {
        match (card.dot, unseen, several) {
            (Dot::Working, _, false) => Form::Working,
            (Dot::Working, _, true) => Form::WorkingLine,
            (Dot::Waiting, ..) => Form::Waits,
            (Dot::Done, true, _) => Form::DoneUnseen,
            (Dot::Done, false, _) => Form::DoneLine,
            (Dot::Ended, true, _) => Form::Ended,
            (Dot::Ended, false, _) => Form::EndedLine,
        }
    }

    pub(super) fn line(self) -> bool {
        matches!(self, Form::WorkingLine | Form::DoneLine | Form::EndedLine)
    }
}

/// The room a card in `form` takes in the list, the room under it with it: its padding, its title's
/// line, its second line, or a finding's `lines`, and, where it has one, its third, a gap between
/// each.
fn room(card: &Card, form: Form, lines: usize) -> f32 {
    if form.line() {
        return LINE + 2. * LINE_AROUND;
    }
    let second = match form {
        Form::DoneUnseen => DONE_LINE * lines as f32,
        _ => LIVE_LINE,
    };
    let pages = !card.pages.is_empty();
    let visited = !card.visited.is_empty();
    let third = match form {
        Form::Waits if pages => BUTTON_HEIGHT,
        Form::Working if pages => ICON.max(COUNT_LINE),
        Form::DoneUnseen if visited => ICON,
        Form::Ended if chip(card).is_some() => CHIP_HEIGHT.max(ICON),
        Form::Ended if visited => ICON,
        _ => 0.,
    };
    let third = if third > 0. { GAP + third } else { 0. };
    2. * PADDING + TITLE_LINE + GAP + second + third + UNDER
}

/// How many lines a card's `finding` takes in a list `width` wide, at most FINDING_LINES: what
/// the card's second row has to its right, at small type.
fn finding_lines(finding: &str, width: f32, cx: &gpui::App) -> usize {
    if finding.is_empty() {
        return 1;
    }
    let room = width - 2. * PLATE_INSET - 2. * PADDING - CARD_GLYPH - GLYPH_GAP;
    let font = gpui::font(crate::face::UI);
    let mut wrapper = cx
        .text_system()
        .line_wrapper(font, px(crate::tokens::text::SMALL));
    let fragments = [gpui::LineFragment::text(finding)];
    let indent = gpui::IndentAdjustment::NoIndent;
    let breaks = wrapper.wrap_line(&fragments, px(room), indent).count();
    (breaks + 1).min(FINDING_LINES as usize)
}

/// The room ⌘T's slot takes: a card at work's, before it has a page.
fn slot_room() -> f32 {
    2. * PADDING + TITLE_LINE + GAP + LIVE_LINE + UNDER
}

/// A card's place in a Space's list (`Space::placed`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Placed {
    /// Its task, or nothing for ⌘T's slot.
    pub(super) agent: SharedString,
    pub(super) n: u64,
    /// The Today tab it stands above, or none at Today's end.
    pub(super) above: Option<u64>,
    /// Listed first in Today, over the cards that stand: a waiting card away from its home Space,
    /// then ⌘T's slot.
    pub(super) lead: Option<u8>,
    pub(super) form: Form,
    pub(super) room: f32,
}

/// Where each of `placed` goes among `today`, a Space's Today rows: before the row at its index,
/// or after them all at their count, with its number and room, top first. A card above a tab that
/// isn't listed goes at Today's end, and those above one tab go newest first.
pub(super) fn blocks(placed: &[Placed], today: &[u64]) -> Vec<(usize, u64, f32)> {
    let at = |card: &Placed| match card.lead {
        Some(_) => 0,
        None => (card.above)
            .and_then(|tab| today.iter().position(|&each| each == tab))
            .unwrap_or(today.len()),
    };
    let mut blocks: Vec<_> = placed
        .iter()
        .map(|card| {
            let at = at(card);
            (
                (at, card.lead.map_or(1, |_| 0), card.lead, Reverse(card.n)),
                (at, card.n, card.room),
            )
        })
        .collect();
    blocks.sort_by_key(|(key, _)| *key);
    blocks.into_iter().map(|(_, block)| block).collect()
}

/// After a Space's Today went from `was` to `today`: a card keeps its place among the rows. Above
/// a tab that left, or one dragged elsewhere, it stands above the next one down that stayed, or at
/// the end. The tabs that stayed are the most that kept their order, so a drag moves only the tab.
pub(super) fn stand_again(placed: &mut [Placed], was: &[u64], today: &[u64]) {
    let stayed = kept_order(was, today);
    for card in placed.iter_mut() {
        let Some(tab) = card.above.filter(|tab| !stayed.contains(tab)) else {
            continue;
        };
        let after = was.iter().skip_while(|&&each| each != tab).skip(1);
        card.above = after.copied().find(|each| stayed.contains(each));
    }
}

/// The tabs of `was` still in `today` that kept their order: the longest run of them whose places
/// in `today` rise, so one tab dragged is the one left out.
fn kept_order(was: &[u64], today: &[u64]) -> Vec<u64> {
    let at: Vec<(u64, usize)> = (was.iter())
        .filter_map(|tab| Some((*tab, today.iter().position(|each| each == tab)?)))
        .collect();
    // Patience sorting: `ends[k]` is the index in `at` of the least last place of a rising run
    // k + 1 long, and `back` each one's place before it in its run.
    let mut ends: Vec<usize> = Vec::new();
    let mut back: Vec<Option<usize>> = vec![None; at.len()];
    for (index, &(_, place)) in at.iter().enumerate() {
        let k = ends.partition_point(|&end| at[end].1 < place);
        back[index] = k.checked_sub(1).map(|before| ends[before]);
        match ends.get_mut(k) {
            Some(end) => *end = index,
            None => ends.push(index),
        }
    }
    let mut run = Vec::with_capacity(ends.len());
    let mut next = ends.last().copied();
    while let Some(index) = next {
        run.push(at[index].0);
        next = back[index];
    }
    run.reverse();
    run
}

/// Whether a regular window lists `card` in `space`'s Today: in its home Space, and a task that
/// waits for the user in every other Space too. `homes` are the window's Spaces.
pub(super) fn lists_in(card: &Card, space: u64, homes: &[u64]) -> bool {
    homes.contains(&card.home) && (card.home == space || card.dot == Dot::Waiting)
}

/// A page Open sent to Today, flying from `from` in the sidebar to its row from `start`.
struct Ghost {
    tab: u64,
    from: Bounds<Pixels>,
    start: Instant,
}

/// Open's pages, which fly to their rows once the list has them.
struct Opening {
    tabs: Vec<u64>,
    from: Bounds<Pixels>,
    at: Instant,
}

/// What the cards keep in the window, past what each Space's list places.
pub(super) struct TaskList {
    /// ⌘T's ask shows over the window, so Today holds its slot.
    slot: bool,
    /// The landing plate's fill and its content: nothing until the bar lands in the slot.
    fill: Ease,
    content: Ease,
    /// Tasks that finished while their conversation didn't show and that it hasn't shown since,
    /// whose cards stay whole, with the dot.
    unseen: HashSet<SharedString>,
    /// When each finished task was last seen, on the shell's clock, which the Today tabs' ageing
    /// counts from.
    seen_at: HashMap<SharedString, Instant>,
    /// The next ageing's wait.
    ageing: Option<gpui::Task<()>>,
    /// The task whose waiting card was clicked away from its home Space, whose conversation shows
    /// once that Space does, or the page it waits on where the click was its Show Page.
    going: Option<(SharedString, bool)>,
    /// Each card as it fades in.
    coming: HashMap<SharedString, Ease>,
    /// The window has drawn its list, so what changes on it from now on moves.
    primed: bool,
    still: bool,
    ghosts: Vec<Ghost>,
    opening: Option<Opening>,
    /// Where each card was last drawn in the window, by its number, for check.sh and for the page
    /// watched zooming out of it.
    plates_drawn: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// Where each was last logged at rest, by its log name.
    rested: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// Where the strip under a task's page drew its Open, in the window.
    pub(super) strip_open: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// When the ring's turns began.
    epoch: Instant,
    /// How many lines each card's finding takes at the sidebar's width at rest, by its task.
    pub(super) finding_lines: HashMap<SharedString, usize>,
}

impl Default for TaskList {
    fn default() -> Self {
        Self {
            slot: false,
            fill: Ease::rest(0.),
            content: Ease::rest(0.),
            unseen: HashSet::new(),
            seen_at: HashMap::new(),
            ageing: None,
            going: None,
            coming: HashMap::new(),
            primed: false,
            still: false,
            ghosts: Vec::new(),
            opening: None,
            plates_drawn: Rc::default(),
            rested: Rc::default(),
            strip_open: Rc::default(),
            epoch: clock(),
            finding_lines: HashMap::new(),
        }
    }
}

impl Shell {
    /// What a screen reader says of a task as it comes to need the user or to be done, wherever
    /// its cursor is, in the words its sidebar line and its notification have: the Mac's
    /// AccessKit announces a live region's value, and Windows' and Linux's its name (#3236).
    pub(super) fn task_announcer(&self) -> Option<impl IntoElement + use<>> {
        let (telling, said) = self.task_told.clone().filter(|_| self.a11y)?;
        Some(
            div()
                .id(("task-told", telling))
                .absolute()
                .size_0()
                .role(Role::Status)
                .aria_label(said.clone())
                .aria_value(said)
                .a11y_synthetic_children(|builder| builder.parent_node().set_live(Live::Polite)),
        )
    }

    /// The Agents' cards, newest first, none of which a private window shows, as it offers no
    /// tasks. A new one takes the ask's slot once ⌘T ran it, so nothing in the list moves.
    pub fn set_task_cards(&mut self, mut cards: Vec<Card>, cx: &mut Context<Self>) {
        if self.one_space() {
            cards.clear();
        }
        if cards == self.task_cards {
            return;
        }
        let mut waits = false;
        // What a screen reader is told of a task that came to need the user or to be done.
        let (mut told, mut answered) = (None, false);
        let read = self.a11y && self.window_active;
        let drawn = (read.then(|| self.talk_agent()).flatten())
            .filter(|_| self.watched_task_page().is_none());
        let now = self.clock.now();
        let shown = self.talk_agent();
        let list = &mut self.task_list;
        for card in &cards {
            let was = self.task_cards.iter().find(|old| old.agent == card.agent);
            waits |= card.dot == Dot::Waiting && was.is_none_or(|was| was.dot != Dot::Waiting);
            // The window in front tells it, and nothing where its notification was posted.
            if read
                && was.is_none_or(|was| was.dot != card.dot)
                && let Some((came, false)) = crate::needs_you::came(&card.agent, cx)
            {
                match drawn.as_ref() == Some(&card.agent) {
                    // Its conversation is drawn: its own line says a need, and the answer a finish.
                    true => answered |= came == Told::Done && card.replies,
                    false => told = Some(crate::needs_you::said(card, came)),
                }
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
            // A task that finishes out of sight keeps its whole card until its conversation shows;
            // one that comes finished, brought back after a quit, has been seen.
            let ended = |dot: Dot| matches!(dot, Dot::Done | Dot::Ended);
            match (was.map(|was| ended(was.dot)), ended(card.dot)) {
                (Some(false), true) if shown.as_ref() != Some(&card.agent) => {
                    list.unseen.insert(card.agent.clone());
                }
                (Some(false) | None, true) => {
                    list.seen_at.insert(card.agent.clone(), now);
                }
                (_, false) => {
                    list.unseen.remove(&card.agent);
                    list.seen_at.remove(&card.agent);
                }
                (Some(true), true) => {}
            }
        }
        let kept = |agent: &SharedString| cards.iter().any(|card| &card.agent == agent);
        list.unseen.retain(|agent| kept(agent));
        list.seen_at.retain(|agent, _| kept(agent));
        list.coming.retain(|agent, _| kept(agent));
        let was: Vec<SharedString> = (self.task_cards.iter())
            .map(|card| card.agent.clone())
            .collect();
        let fresh = |card: &&Card| !was.contains(&card.agent);
        let ran = list.slot && list.fill.end() > 0.;
        let took = ran
            .then(|| {
                cards
                    .iter()
                    .filter(fresh)
                    .max_by_key(|card| number(&card.agent))
            })
            .flatten()
            .map(|card| card.agent.clone());
        if took.is_some() {
            list.slot = false;
        }
        // A card comes in fading, unless it fills the slot the bar landed in, or the window hasn't
        // drawn its list yet.
        if list.primed {
            let still = crate::motion::still(cx);
            let (length, curve) = match still {
                true => (CROSSFADE, Curve::Linear),
                false => (FADE, Curve::Ease),
            };
            let at = clock();
            for card in cards.iter().filter(fresh) {
                if Some(&card.agent) != took.as_ref() {
                    let mut ease = Ease::rest(0.);
                    ease.go(1., at, length, 0., curve);
                    list.coming.insert(card.agent.clone(), ease);
                }
            }
        }
        let before = std::mem::replace(&mut self.task_cards, cards);
        self.keep_plates(&before);
        self.area_takes_space();
        if let Some(said) = told {
            let telling = self
                .task_told
                .as_ref()
                .map_or(0, |(telling, _)| telling + 1);
            self.task_told = Some((telling, said.into()));
        }
        if answered {
            self.talk_answered(cx);
        }
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
        self.place_cards(cx);
        self.age_cards(cx);
        // The task the Archive brought back shows its conversation as its card comes back.
        if let Some(agent) = self.show_restored.take()
            && self.task_cards.iter().any(|card| card.agent == agent)
        {
            self.show_task(agent, cx);
        }
        self.show_started_task(Some(was.as_slice()), cx);
        cx.notify();
    }

    /// ⌘T's ask shows over this window, or no longer does: Today opens an empty slot for the
    /// task's card at its top, or closes it.
    pub fn set_ask_slot(&mut self, shown: bool, cx: &mut Context<Self>) {
        if shown == self.task_list.slot {
            return;
        }
        log!("browser: ask slot {}", shown as u8);
        self.task_list.slot = shown;
        if shown {
            self.task_list.fill = Ease::rest(0.);
            self.task_list.content = Ease::rest(0.);
        }
        self.place_cards(cx);
        cx.notify();
    }

    /// The page area shows another task's view, or none: a finished card shown loses its dot and
    /// folds to its line.
    pub(super) fn reshape_cards(&mut self, cx: &mut Context<Self>) {
        self.place_cards(cx);
    }

    /// The page area has a task's view back as it stood, after a close a page refused: the list is
    /// as it stood too, at once (#4202).
    pub(super) fn restore_cards(&mut self, cx: &mut Context<Self>) {
        let primed = std::mem::replace(&mut self.task_list.primed, false);
        self.place_cards(cx);
        self.task_list.primed = primed;
    }

    /// Places each card in the lists of the Spaces that list it, each with the form its task's state
    /// gives it, and moves the rows of the Space shown to make room. A card new to a Space stands
    /// above the Today tab on top; one already there keeps its place.
    pub(super) fn place_cards(&mut self, cx: &mut Context<Self>) {
        if let Some(agent) = self.talk_agent() {
            self.task_list.unseen.remove(&agent);
        }
        let before = self.layout();
        let homes: Vec<u64> = self.spaces.iter().map(|space| space.id).collect();
        let regular = !self.one_space();
        let width = self.sidebar_width;
        self.task_list.finding_lines = (self.task_cards.iter())
            .filter(|card| !card.finding.is_empty())
            .map(|card| (card.agent.clone(), finding_lines(&card.finding, width, cx)))
            .collect();
        let shown = self.space;
        let list = &self.task_list;
        let lines = |agent: &SharedString| list.finding_lines.get(agent).copied().unwrap_or(1);
        for space in &mut self.spaces {
            let listed: Vec<&Card> = (self.task_cards.iter())
                .filter(|card| regular && lists_in(card, space.id, &homes))
                .collect();
            let several = listed
                .iter()
                .filter(|card| card.dot == Dot::Working)
                .count()
                > 1;
            let top = space.today.first().copied();
            let mut placed: Vec<Placed> = listed
                .into_iter()
                .map(|card| {
                    let form = Form::of(card, list.unseen.contains(&card.agent), several);
                    let away = card.home != space.id;
                    let stood = (space.placed.iter()).find(|was| was.agent == card.agent);
                    Placed {
                        agent: card.agent.clone(),
                        n: number(&card.agent),
                        above: stood.map_or(top, |was| was.above),
                        lead: away.then_some(0),
                        form,
                        room: room(card, form, lines(&card.agent)),
                    }
                })
                .collect();
            if list.slot && space.id == shown {
                placed.push(Placed {
                    agent: SharedString::default(),
                    n: SLOT,
                    above: None,
                    lead: Some(1),
                    form: Form::Working,
                    room: slot_room(),
                });
            }
            space.placed = placed;
        }
        if self.task_list.primed && self.layout() != before {
            self.move_rows(before, RowCurve::Slide, cx);
        }
    }

    /// Takes the finished cards the user has seen whose wait is up, as the profile's ageing takes a
    /// Today tab no window shows, into the Archive, and waits for the next one due. A card at work,
    /// waiting or not yet seen is never taken, nor one whose conversation shows.
    fn age_cards(&mut self, cx: &mut Context<Self>) {
        let now = self.clock.now();
        let shown = self.talk_agent();
        let mut due = Vec::new();
        let mut next: Option<Instant> = None;
        for card in &self.task_cards {
            let list = &self.task_list;
            if !card.finished()
                || list.unseen.contains(&card.agent)
                || shown.as_ref() == Some(&card.agent)
            {
                continue;
            }
            let Some(&seen) = list.seen_at.get(&card.agent) else {
                continue;
            };
            // Its Space's profile's time, as a Today tab's; the core's Never is below nothing.
            let home = (self.spaces.iter()).find(|space| space.id == card.home);
            let Some(after) = (home.and_then(|space| self.browser.archive_after(space.id)))
                .map(|(after, _)| after)
                .filter(|after| *after >= 0.)
            else {
                continue;
            };
            let at = seen + Duration::from_secs_f64(after);
            match at <= now {
                true => due.push(card.agent.clone()),
                false => next = Some(next.map_or(at, |next| next.min(at))),
            }
        }
        for agent in due {
            log!("browser: task {agent} aged into the archive");
            self.task_list.seen_at.remove(&agent);
            self.browser.task(agent, TaskAction::Dismiss, cx);
        }
        self.task_list.ageing = next.map(|at| {
            let wait = at.saturating_duration_since(now);
            cx.spawn(async move |shell, cx| {
                cx.background_executor().timer(wait).await;
                shell.update(cx, |shell, cx| shell.age_cards(cx)).ok();
            })
        });
    }

    /// The Space shown changed: a click on a waiting card away from its home Space went there, and
    /// its conversation shows now, or its Show Page's page while the task still waits.
    pub(super) fn card_space_shown(&mut self, cx: &mut Context<Self>) {
        // ⌘T's slot is in the Space shown alone.
        if self.task_list.slot {
            self.place_cards(cx);
        }
        let Some((agent, page)) = self.task_list.going.take() else {
            return;
        };
        let Some(card) = (self.task_cards.iter()).find(|card| card.agent == agent) else {
            return;
        };
        if card.home != self.space {
            return;
        }
        if page {
            if card.dot == Dot::Waiting {
                self.card_button(agent, cx);
            }
        } else if self.talk_agent() != Some(agent.clone()) {
            self.show_task(agent, cx);
        }
    }

    /// The glyph's colours: Signal, a title's ink and a state line's, and the stopped ring's, the
    /// ink at `stopped`, under the floor text keeps, as it's a mark beside words that say the task
    /// stopped.
    fn glyph_inks(&self) -> glyph::Inks {
        glyph::Inks {
            signal: rgb(STATUS_AGENT).into(),
            title: self.palette.text(TITLE),
            status: self.palette.gray(STATUS),
            stopped: Hsla::from(self.palette.ink).opacity(STOPPED),
        }
    }

    /// A finished task's mark at rest, as the Library shows it: Done's tick, or the grey ring of
    /// one stopped or failed, in the glyph's box.
    pub(super) fn status_mark(&self, dot: Dot) -> impl IntoElement + use<> {
        let look = Look::of(State::from(dot));
        let inks = self.glyph_inks();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| glyph::paint(look, bounds, inks, false, window),
        )
        .size(px(GLYPH))
    }

    /// The mark of `agent`'s card as the list draws it now, for the strip under its page watched to
    /// lead with, so the two read as one, in the glyph's box.
    pub(super) fn task_glyph(&self, agent: &SharedString, dot: Dot) -> AnyElement {
        let unseen = self.task_list.unseen.contains(agent);
        div()
            .flex_none()
            .size(px(GLYPH))
            .flex()
            .items_center()
            .justify_center()
            .child(self.card_mark(dot, unseen))
            .into_any_element()
    }

    /// The task numbered `n`.
    fn card_agent(&self, n: u64) -> Option<SharedString> {
        (self.task_cards.iter())
            .find(|card| number(&card.agent) == n)
            .map(|card| card.agent.clone())
    }

    /// The cards the list of the Space shown places, top first.
    fn shown_placed(&self) -> Vec<&Placed> {
        let Some(space) = self.shown_space() else {
            return Vec::new();
        };
        let (_, today) = space.rows();
        let order = blocks(&space.placed, &today);
        let by_n = |n: u64| space.placed.iter().find(|card| card.n == n);
        order.into_iter().filter_map(|(_, n, _)| by_n(n)).collect()
    }

    /// Whether the list draws `agent`'s card, where the ring can sit.
    pub(super) fn card_listed(&self, agent: &SharedString) -> bool {
        self.list_drawn() && self.shown_placed().iter().any(|card| &card.agent == agent)
    }

    /// Whether the sidebar shows the list where it rests.
    fn list_drawn(&self) -> bool {
        !self.page_fullscreen
            && !self.sidebar_hidden
            && self.sidebar_motion.is_none()
            && self.library.is_none()
            && self.form().is_none()
    }

    /// The card whose conversation the page area shows, if the list draws it.
    pub(super) fn shown_card(&self) -> Option<StripStop> {
        let agent = self.talk_agent()?;
        self.card_listed(&agent)
            .then(|| StripStop::Header(number(&agent)))
    }

    /// The cards' stops for the keys' ring, top first, each with the top of its place in the
    /// list: each card, then its button or chip. None while the sidebar doesn't draw the list, so
    /// the ring never sits where it can't be seen.
    pub(super) fn card_stops(&self) -> Vec<(StripStop, f32)> {
        if !self.list_drawn() {
            return Vec::new();
        }
        let layout = self.layout();
        let mut stops = Vec::new();
        for placed in self.shown_placed() {
            let Some(card) = self
                .task_cards
                .iter()
                .find(|card| card.agent == placed.agent)
            else {
                continue;
            };
            let top = layout
                .get(&Slot::Task(placed.n))
                .copied()
                .unwrap_or_default();
            stops.push((StripStop::Header(placed.n), top));
            let button = match placed.form {
                Form::Waits => self.waiting_page(card).is_some(),
                Form::Ended => chip(card).is_some(),
                _ => false,
            };
            if button {
                stops.push((StripStop::Chip(placed.n), top));
            }
        }
        stops
    }

    /// Return on one of the cards' stops: what a click on it does.
    pub(super) fn key_card_show(&mut self, stop: StripStop, cx: &mut Context<Self>) {
        match stop {
            StripStop::Header(n) => {
                if let Some(agent) = self.card_agent(n) {
                    self.leave_strip("header", cx);
                    log!("browser: command show-task via strip keys");
                    self.show_group(agent, cx);
                }
            }
            StripStop::Chip(n) => {
                if let Some(agent) = self.card_agent(n) {
                    self.card_button(agent, cx);
                }
            }
            _ => {}
        }
    }

    /// A card's click: its task's conversation, which a page of its watched goes back to. On the
    /// card whose conversation shows, it puts the conversation away. A waiting card away from its
    /// home Space goes there first.
    fn show_group(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        let home = (self.task_cards.iter())
            .find(|card| card.agent == agent)
            .map(|card| card.home);
        if let Some(home) = home.filter(|&home| home != self.space) {
            log!("browser: task {agent} shown in its home space {home}");
            self.task_list.going = Some((agent, false));
            return self.show_space(home, cx);
        }
        if self.talk_agent().as_ref() == Some(&agent) && self.watched_task_page().is_some() {
            return self.task_page_back(cx);
        }
        self.show_task(agent, cx);
    }

    /// The button on `agent`'s card: Show Page on a waiting card, the chip on an ended one.
    fn card_button(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        let Some(card) = self.task_cards.iter().find(|card| card.agent == agent) else {
            return;
        };
        if card.dot == Dot::Waiting {
            // Away from its home Space, the page shows there, as any page of another Space does.
            if card.home != self.space {
                let home = card.home;
                log!("browser: task {agent} page shown in its home space {home}");
                self.task_list.going = Some((agent, true));
                return self.show_space(home, cx);
            }
            if let Some(tab) = self.waiting_page(card) {
                log!("browser: command show-page via task card");
                if self.talk_agent().as_ref() != Some(&agent) {
                    self.show_task(agent, cx);
                }
                self.watch_row(tab, false, cx);
            }
            return;
        }
        if let Some(chip) = chip(card) {
            self.run_chip(agent, chip, cx);
        }
    }

    /// The page a waiting task waits on the user in, else its first.
    fn waiting_page(&self, card: &Card) -> Option<u64> {
        let waits = |tab: &&u64| self.page_word(card, **tab) == PageWord::NeedsYou;
        (card.pages.iter().find(waits))
            .or(card.pages.first())
            .copied()
    }

    /// Stands on `tab`, a task's page, as its picture's click in the task's view does, and nothing
    /// else. Says whether it did. The page zooms out of its picture there, or of its task's card
    /// where the view has drawn none. `by_ilo` when ilo opens it by itself, as at a handoff.
    pub(super) fn watch_row(&mut self, tab: u64, by_ilo: bool, cx: &mut Context<Self>) -> bool {
        if self
            .watched_task_page()
            .is_some_and(|(_, shown)| shown == tab)
        {
            return false;
        }
        let card = (self.task_cards.iter())
            .find(|card| card.pages.contains(&tab))
            .map(|card| number(&card.agent));
        let drawn = card.and_then(|n| self.task_list.plates_drawn.borrow().get(&n).copied());
        let (from, picture) =
            (self.talk_thumb(tab, cx)).unwrap_or_else(|| (drawn.unwrap_or_default(), None));
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

    /// What `tab` is to `card`'s task now: its word, of the six a task's page has, and whether
    /// it has the outline. A tile in the rail and the card read this one answer, so a page's word
    /// and its outline never disagree. While the task works, each of its pages says Working and the
    /// outline alone marks the one it's in; Done is a page's word only once the task is done, as a
    /// page it left may be come back to. At a handoff the outline is on the page that needs the
    /// user. A page the user took over is theirs, one it handed them and went on from is Paused, as
    /// is one where it waits at a refused step or broke off; every page of a task the user stopped
    /// says Stopped. A stopped or done task has no outline.
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
            Word::Stopped => mark(PageWord::Stopped, false),
            Word::NeedsYou | Word::Interrupted | Word::AtLimit => mark(PageWord::Paused, false),
        }
    }

    /// `tab`'s word, from `page_mark`.
    pub(super) fn page_word(&self, card: &Card, tab: u64) -> PageWord {
        self.page_mark(card, tab).word
    }

    /// Where ⌘T's bar folds to as it runs a task: the slot's plate as it will rest, in the window,
    /// if the sidebar shows it, with its corners' radius and its fill.
    pub fn fold_target(&self) -> Option<(Bounds<Pixels>, f32, Hsla)> {
        if !self.list_drawn() || !self.task_list.slot {
            return None;
        }
        let top = *self.layout().get(&Slot::Task(SLOT))?;
        let scrolled = f32::from(self.list_scroll.offset().y);
        let origin = self.sidebar_origin.get();
        let bounds = Bounds::new(
            point(
                origin.x + px(PLATE_INSET),
                origin.y + px(self.drawn_list_top() + top + scrolled),
            ),
            gpui::size(px(self.width() - 2. * PLATE_INSET), px(slot_room() - UNDER)),
        );
        let radius = crate::tokens::shell::task_card::RADIUS;
        Some((bounds, radius, self.palette.plate(super::TILE)))
    }

    /// The bar has gone for the task: folded into the slot, whose fill takes over as it lands and
    /// goes as the card fades in then, or, where it couldn't fold, hidden, and the card fades in
    /// at once.
    pub fn fold_started(&mut self, folded: bool, cx: &mut Context<Self>) {
        if !self.task_list.slot {
            return;
        }
        let now = clock();
        let still = crate::motion::still(cx);
        let list = &mut self.task_list;
        match (folded, still) {
            (true, false) => {
                list.fill.go(1., now, 0., FOLD, Curve::Ease);
                list.content.go(1., now, CONTENT, FOLD, Curve::Ease);
            }
            (_, true) => {
                list.fill.go(1., now, CROSSFADE, 0., Curve::Linear);
                list.content.go(1., now, CROSSFADE, 0., Curve::Linear);
            }
            (false, false) => {
                list.fill.go(1., now, CONTENT, 0., Curve::Ease);
                list.content.go(1., now, CONTENT, 0., Curve::Ease);
            }
        }
        cx.notify();
    }

    /// The slot's landing fill at `now`, a tile's at most, and how much of its dashes show.
    fn landing_slot(&self, now: Instant) -> (f32, f32) {
        let fill = self.task_list.fill.at(now);
        (fill * (1. - self.task_list.content.at(now)), 1. - fill)
    }

    /// Once a frame is built: starts Open's pages' flights once the list has their rows, and asks
    /// for the next frame while anything moves. The ring at work draws on the loading dots' clock
    /// instead, as it turns all the while its task works.
    pub(super) fn step_task_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = clock();
        self.task_list.still = crate::motion::still(cx);
        self.task_list.primed = true;
        if let Some(opening) = &self.task_list.opening {
            let layout = self.layout();
            if (opening.tabs.iter()).all(|&tab| layout.contains_key(&Slot::Tab(tab))) {
                let opening = self.task_list.opening.take().expect("an opening");
                let ghosts = opening.tabs.iter().enumerate().map(|(index, &tab)| Ghost {
                    tab,
                    from: opening.from,
                    start: now + seconds(GHOST_DELAY + index as f32 * GHOST_STAGGER),
                });
                self.task_list.ghosts.extend(ghosts);
            } else if now.saturating_duration_since(opening.at) > OPENING_WAIT {
                self.task_list.opening = None;
            }
        }
        let flight = seconds(GHOST + GHOST_LAND);
        self.task_list
            .ghosts
            .retain(|ghost| now < ghost.start + flight);
        self.task_list.coming.retain(|_, ease| ease.moving(now));
        for ease in [&mut self.task_list.fill, &mut self.task_list.content] {
            ease.settle(now);
        }
        if self.cards_moving(now) {
            window.request_animation_frame();
        }
        if self.cards_live() {
            self.breath.keep();
        }
    }

    /// Whether anything about the cards moves at `now`, which the display's frames draw.
    pub(super) fn cards_moving(&self, now: Instant) -> bool {
        let list = &self.task_list;
        list.opening.is_some()
            || !list.ghosts.is_empty()
            || list.coming.values().any(|ease| ease.moving(now))
            || list.fill.moving(now)
            || list.content.moving(now)
    }

    /// Whether a ring turns in the list shown, which draws on the loading dots' clock. With motion
    /// reduced it holds still.
    pub(super) fn cards_live(&self) -> bool {
        !self.task_list.still
            && self.list_drawn()
            && (self.shown_placed().iter()).any(|card| {
                matches!(card.form, Form::Working | Form::WorkingLine) && card.n != SLOT
            })
    }

    /// Whether `tab`'s row waits for its page's flight to land, which draws it meanwhile.
    pub(super) fn landing(&self, tab: u64) -> bool {
        let now = clock();
        let flight = seconds(GHOST + GHOST_LAND);
        let list = &self.task_list;
        (list.opening.as_ref()).is_some_and(|opening| opening.tabs.contains(&tab))
            || (list.ghosts.iter()).any(|ghost| ghost.tab == tab && now < ghost.start + flight)
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
        if let Some(from) = from
            && !crate::motion::still(cx)
            && !tabs.is_empty()
        {
            let origin = self.sidebar_origin.get();
            let from = Bounds::new(from.origin - origin, from.size);
            self.task_list.opening = Some(Opening {
                tabs,
                from,
                at: clock(),
            });
        }
        let action = tab.map_or(TaskAction::Open, TaskAction::OpenTab);
        self.browser.task(agent, action, cx);
        cx.notify();
    }

    /// ⌘W in its conversation: the task goes, as ⌘Z and ⇧⌘T bring back.
    pub(super) fn dismiss_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
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

    /// The user left `agent`'s conversation: a finished task they've now seen ages from now, as a
    /// Today tab ages from when it was last shown.
    pub(super) fn talk_left(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        let ended = (self.task_cards.iter())
            .any(|card| card.agent == agent && matches!(card.dot, Dot::Done | Dot::Ended));
        if ended {
            let now = self.clock.now();
            self.task_list.seen_at.insert(agent, now);
            self.age_cards(cx);
        }
    }

    /// Open's pages on their way from the strip's Open to their rows at the top of Today: each a
    /// chip's plate growing into a row's hover plate, its favicon and title fading in, the row
    /// showing as it lands.
    pub(super) fn task_ghosts(&self) -> Vec<AnyElement> {
        let now = clock();
        let layout = self.layout();
        let scrolled = f32::from(self.list_scroll.offset().y);
        let width = self.width() - 2. * PLATE_INSET;
        let list = &self.task_list;
        // Waiting for the list, they sit on the chip.
        let waiting: Vec<(u64, Bounds<Pixels>)> = match &list.opening {
            Some(opening) => (opening.tabs.iter())
                .map(|&tab| (tab, opening.from))
                .collect(),
            None => Vec::new(),
        };
        let flying = list.ghosts.iter().filter_map(|ghost| {
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
    /// Where `agent`'s card was last drawn in the window.
    #[cfg(test)]
    pub(super) fn task_plate_drawn(&self, agent: &str) -> Option<Bounds<Pixels>> {
        (self.task_list.plates_drawn.borrow())
            .get(&number(agent))
            .copied()
    }

    /// Records where the strip under a task's page draws its Open, which its page flies from.
    pub(super) fn strip_open_listener(&self) -> impl IntoElement + use<> {
        let drawn = self.task_list.strip_open.clone();
        canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
            .absolute()
            .inset_0()
    }
}

/// Records where a card is drawn in the window, and logs it once it's at rest there.
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
                SLOT => "slot".to_string(),
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
