//! A task's conversation, which the page area shows in the page's place (Agent task,
//! task_area.rs). A click on a task's card shows it, coming in from the card's side. Its title and
//! where it is head it, its pages' pictures under them, taken only while it shows, then what was
//! asked with its page and what each side said, with the work between two messages a trace, and
//! the reply field at its foot. A picture's click zooms its page out of it. Esc steps back to the
//! Space's tab, ⌘W closes the card, and ⌘. stops the task.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Bounds, Context, Corners, ElementId, Entity, Focusable, FontStyle, FontWeight, HighlightStyle,
    Hsla, Image, InteractiveText, IntoElement, MouseButton, ObjectFit, Pixels, Radians, Render,
    RenderImage, ScrollHandle, SharedString, Size, StyledText, Transformation, UnderlineStyle,
    WeakEntity, Window, actions, canvas, div, img, point, prelude::*, px, rgba, size, svg,
};

use gpui::{Role, accesskit::Live as Telling};

use self::markdown::{Block, Inline};
use self::motion::{Answered, Follow, Key, Landing, Motion, Run, eased, linear};
use super::{
    NAV_RADIUS, SECONDARY, Shell, TITLE, Trouble,
    agent_bar::stop_keys,
    browser::{Browser, Page},
    live_tiles::LiveTiles,
    menus::{MenuAction, MenuEntry},
    page_ends::StripStop,
    picture::{PagePicture, next_request},
    ring, row_spring,
    task_cards::PageWord,
};
use crate::agents::Status;
use crate::agents::files::words as files_words;
use crate::agents::kept::cards::Plate;
use crate::agents::page::clip;
use crate::agents::task::ilo::{self, IloButton, IloKind, IloLine};
use crate::agents::task::{
    ArchivedTask, Card, Conversation, Dot, Line, Live, Opens, TaskAction, Trace, TraceItem,
    TraceStep, told_line,
};
use crate::agents::tier::Tier;
use crate::command_bar::choices;
use crate::palette::{Ground, STATE_HOVERED, STATE_PRESSED, Tone};
use crate::text_input::TextInput;
use crate::tokens::command_bar::CHIP as BAR_CHIP;
use crate::tokens::command_bar::CHOICES_MENU_DROP;
use crate::tokens::mark::INK as MARK_INK;
use crate::tokens::shell::FAVICON_DRAWN;
use crate::tokens::shell::conversation::ilo::{
    ARM, BELOW as ILO_BELOW, GLYPH as ILO_GLYPH, HELD_BUTTON, HELD_BUTTON_FILL, HELD_BUTTON_HOVER,
    HELD_BUTTON_RADIUS, HELD_BUTTON_TEXT, HELD_BUTTON_TEXT_LINE, HELD_BUTTON_X, HELD_BUTTONS_GAP,
    HELD_GAP, HELD_NOTE, HELD_NOTE_LINE, HELD_ON_PRIMARY, HELD_PRIMARY, HELD_REASON,
    HELD_REASON_LINE, HELD_TOP, INK as ILO_INK, LIGHT_HELD_BUTTON_FILL, LIGHT_HELD_ON_PRIMARY,
    LIGHT_HELD_PRIMARY, NOTE_INK, NOTE_TOP, QUOTED_CHARS, TEXT_LINE as ILO_TEXT_LINE, TEXT_TOP,
};
use crate::tokens::shell::conversation::motion::{CHEVRON, LAND, PICK_FADE, RISE, STEPS_FADE};
use crate::tokens::shell::conversation::panel;
use crate::tokens::shell::conversation::{
    ANSWER_FOOT, ANSWER_FOOT_GAP, ANSWER_FOOT_TOP, ANSWER_TIME, ANSWER_TIME_LINE, BLOCK_GAP,
    BUBBLE_BOTTOM, BUBBLE_FILL, BUBBLE_MOST, BUBBLE_TOP, BUBBLE_X, CHIP_KEY, CHIPS_BELOW,
    CHIPS_TOP, CODE_FILL, COLUMN, COMPOSER_ARROW, COMPOSER_FIELD, COMPOSER_FIELD_X, COMPOSER_LINE,
    COMPOSER_RADIUS, COMPOSER_RIM, COMPOSER_ROUND, COMPOSER_ROW, COMPOSER_ROW_GAP,
    COMPOSER_ROW_UNDER, COMPOSER_ROW_X, COMPOSER_SQUARE, COMPOSER_SQUARE_RADIUS, COMPOSER_TEXT,
    CONNECTOR_ABOVE, CONNECTOR_HEIGHT, CONNECTOR_INK, CONNECTOR_WIDTH, CONNECTOR_X, COPY_GLYPH,
    FOLD_DONE_GAP, FOLD_GAP, FOLD_HEAD, FOLD_ICON, FOLD_ICONS_GAP, HEADER_BOTTOM, INSET, ITEM_GAP,
    LIGHT_BUBBLE_FILL, LIGHT_TEXT_3, LINES_BELOW, LINK_LINE, LINK_UNDERLINE, LIST_MARKER, META_GAP,
    META_LINE, OPEN_BELOW, OPEN_TOP, PAGE_CHIP_BELOW, PAGE_CHIP_FAVICON, PAGE_CHIP_FILL,
    PAGE_CHIP_GAP, PAGE_CHIP_HANG, PAGE_CHIP_HEIGHT, PAGE_CHIP_X, PAGE_LINE, PARAGRAPH_GAP,
    PICTURE_CRASHED_WASH, PICTURE_EMPTY_GLYPH, PICTURE_HEIGHT, PICTURE_WIDTH, RADIUS, REPLY_BOTTOM,
    REPLY_FILL, REPLY_GAP, REPLY_HEIGHT, SAID_GAP, SAID_INK, SAID_RULE, SAYS_TOP, SHIMMER,
    SHIMMER_BAND, SHIMMER_PERIOD, SITE_GAP, STEP_RING, STEP_RING_STROKE, STRETCH_GAP, TEXT, TEXT_3,
    TEXT_LINE, TITLE_LINE, TITLE_SIZE, TOP, TRACE_BELOW, TRACE_CHEVRON, TRACE_GAP, TRACE_GLYPH,
    TRACE_GLYPH_ALPHA, TRACE_HANG, TRACE_HOVER, TRACE_LINE, TRACE_NOTE, TRACE_PICTURE_HEIGHT,
    TRACE_PICTURE_RADIUS, TRACE_PICTURE_WIDTH, TRACE_TEXT, TRACE_TEXT_LINE, TRACE_TITLE,
};
use crate::tokens::shell::task::motion::{CHIPS, CHIPS_RISE, HEIGHT, RING_TURN};
use crate::tokens::shell::task::{
    LIGHT_TILE_FILL, LIGHT_TILE_LINE, LIGHT_TILE_LINE_ON, RAIL_BREAKPOINT, RAIL_EDGE, RAIL_FAVICON,
    RAIL_GAP, RAIL_HEAD, RAIL_HEAD_GAP, RAIL_HEAD_X, RAIL_TITLE, RAIL_TITLE_LINE, RAIL_WIDTH,
    RAIL_WORD, RAIL_WORD_LINE, RING_TRACK, STATUS, TILE_FILL, TILE_LINE, TILE_LINE_ON,
    TILE_PICTURE, TILE_RADIUS, TILE_RING, TILE_RING_STROKE,
};
use crate::tokens::shell::task_card::FROM_LINE;
use crate::tokens::shell::{HAIRLINE, ICON_INK};
use crate::tokens::text;
use crate::tokens::toast::{CHIP_FILL, CHIP_GAP, CHIP_HEIGHT, CHIP_RADIUS, CHIP_X, KEY_GAP};

actions!(conversation, [SendReply, StopTask]);

mod ilo_keys;
mod markdown;
mod motion;
pub(crate) mod pages;
mod picks;
pub(crate) mod words;

/// The assistant's said words, in the answer and between a stretch's steps alike: the text's ink.
fn said_ink(palette: &super::Palette) -> Hsla {
    palette.text(TITLE)
}

/// The stage's fill, the conversation's ground in the page area: also the fill of the shown
/// thread's card, a shape of its own beside it (task_cards.rs).
pub(super) fn stage_fill(palette: &super::Palette) -> gpui::Rgba {
    palette.page
}

/// A chip at the page's scale, the toast's: its words, and its key after them, fainter. The
/// conversation's and the page strip's buttons are these.
pub(super) fn page_chip(
    palette: super::Palette,
    id: impl Into<ElementId>,
    words: impl Into<SharedString>,
    key: Option<String>,
) -> gpui::Stateful<gpui::Div> {
    page_chip_keyed(palette, id, words, key, palette.text(CHIP_KEY))
}

/// A page chip whose key is in `key_color`, as on a primary's fill.
pub(super) fn page_chip_keyed(
    palette: super::Palette,
    id: impl Into<ElementId>,
    words: impl Into<SharedString>,
    key: Option<String>,
    key_color: Hsla,
) -> gpui::Stateful<gpui::Div> {
    chip_ground(palette, id)
        .child(words.into())
        .children(key.map(|key| div().text_color(key_color).child(key)))
}

/// A page chip's ground, with nothing on it yet.
fn chip_ground(palette: super::Palette, id: impl Into<ElementId>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(CHIP_HEIGHT))
        .px(px(CHIP_X))
        .flex()
        .items_center()
        .gap(px(KEY_GAP))
        .rounded(px(CHIP_RADIUS))
        .bg(palette.ground_plate(Ground::Chip, CHIP_FILL))
        .hover(move |style| style.bg(palette.ground_state(Ground::Chip, CHIP_FILL, STATE_HOVERED)))
        .active(move |style| style.bg(palette.ground_state(Ground::Chip, CHIP_FILL, STATE_PRESSED)))
        .whitespace_nowrap()
        .text_size(px(text::BODY))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.text(TITLE))
}

/// Sends the windows' `shells` what the Agents show as they change: their status, the task cards,
/// the tasks in the Archive, those of them a run before left that Latest tasks lists (`kept`) and,
/// to a window showing a task's conversation, that conversation, as `conversation` makes it for
/// the task's agent. Each is made before any shell changes, since making one reads its pages
/// from a shell (the core's `Browser::tab`), which can't be read while it's being changed.
pub fn send_agents(
    status: Vec<Status>,
    cards: Vec<Card>,
    archived: Vec<ArchivedTask>,
    kept: Vec<Card>,
    conversation: impl Fn(&str, &gpui::App) -> Option<Conversation>,
    shells: &[WeakEntity<Shell>],
    cx: &mut gpui::App,
) {
    let shells: Vec<Entity<Shell>> = shells.iter().filter_map(WeakEntity::upgrade).collect();
    let talks: Vec<_> = (shells.iter())
        .map(|shell| {
            let agent = shell.read(cx).talk_agent()?;
            let made = conversation(&agent, cx);
            Some((agent, made))
        })
        .collect();
    for (shell, talk) in shells.into_iter().zip(talks) {
        shell.update(cx, |shell, cx| {
            shell.set_agent_status(status.clone(), cx);
            shell.set_archived_tasks(archived.clone(), cx);
            shell.set_kept_cards(kept.clone(), cx);
            shell.set_task_cards(cards.clone(), cx);
            shell.assistants_learned(cx);
            // Only a window that still shows the task's conversation is sent it.
            if let Some((agent, made)) = talk
                && shell.talk_agent() == Some(agent)
            {
                shell.set_conversation(made, cx);
            }
        });
    }
}

/// Sends the windows' `shells` what stands in for the titles of this run's threads that have
/// none, by their pasts' ids (`Agents::past_stand_ins`), for History's rows of them.
pub fn send_past_stand_ins(
    titles: &[(String, gpui::SharedString)],
    shells: &[WeakEntity<Shell>],
    cx: &mut gpui::App,
) {
    for shell in shells.iter().filter_map(WeakEntity::upgrade) {
        shell.update(cx, |shell, cx| {
            shell.set_past_stand_ins(titles.to_vec(), cx)
        });
    }
}

/// The key context of the conversation, whose Return sends the reply ahead of the browser's
/// keys. Esc, ⌘W and ⌘. are the task area's, wherever the keyboard is.
const KEY_CONTEXT: &str = "Conversation";

pub fn bind_keys(cx: &mut gpui::App) {
    ilo_keys::bind(cx);
    let keys = [super::keys::view_binding(
        "enter",
        Box::new(SendReply),
        KEY_CONTEXT,
        cx,
    )];
    cx.bind_keys(keys);
}

/// The conversation a window shows, or showed last.
pub(super) struct Talk {
    pub(super) agent: SharedString,
    space: u64,
    view: Entity<TalkView>,
}

impl Talk {
    /// Its pages leave their tiles, as a page watched over it hides it and it isn't drawn.
    pub(super) fn end_tiles(&self, cx: &mut gpui::App) {
        let tiles = self.view.read(cx).tiles.clone();
        tiles.end_all(cx);
    }

    /// Whether `tab` is one the user handed its thread, whose page's menu offers the way back.
    pub(super) fn hands(&self, tab: u64, cx: &gpui::App) -> bool {
        let shown = self.view.read(cx).shown.as_ref();
        shown.is_some_and(|shown| shown.handed.contains(&tab))
    }

    /// The tiles its pages show live in, which a watch borrows one of.
    pub(super) fn tiles(&self, cx: &gpui::App) -> Rc<LiveTiles> {
        self.view.read(cx).tiles.clone()
    }
}

/// The view the page area draws.
pub struct TalkView {
    shell: WeakEntity<Shell>,
    agent: SharedString,
    shown: Option<Conversation>,
    reply: Entity<TextInput>,
    /// Whether the user replies here: a task, and not an outside agent's session, whose user talks
    /// to it where they started it, which a quiet line in the field's place says.
    replies: bool,
    /// Its pages' pictures, by tab: the request out for each, and the picture once it came.
    pictures: Vec<(u64, u64, Option<PagePicture>)>,
    /// The requests out for pages whose picture could have been given to an agent when asked,
    /// which a kept card may keep if it still could when the picture comes.
    keeps_asked: Vec<u64>,
    /// The pages the core last answered with no picture of, which the next action asks again.
    unpictured: Vec<u64>,
    /// Where each picture was drawn last, in the window, for its page to zoom out of.
    drawn: Rc<RefCell<Vec<(u64, Bounds<Pixels>)>>>,
    /// Where the glyph of each dead page's picture was drawn last, in the window.
    crashed_drawn: Rc<RefCell<Vec<(u64, Bounds<Pixels>)>>>,
    /// Its pages, live in their pictures' places where the core can.
    tiles: Rc<LiveTiles>,
    /// Where each picture was drawn when that was logged last.
    drawn_logged: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// What each tile's word said when that was logged last, then its page's state's word, which a
    /// dead page's tile leaves out.
    said_logged: RefCell<HashMap<u64, (&'static str, &'static str)>>,
    /// The thread's pages the user has stood on, which its panel's head shows at full strength.
    visited: HashSet<u64>,
    /// Where Stop was drawn when that was logged last.
    stop_logged: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Where the composer's round button was drawn last, Stop or Send, for a test.
    round_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Its pages changed while one was watched over it, so they're pictured again once it shows.
    stale: bool,
    /// Whether its step under way shimmered as it last drew, which check.sh hears of.
    shimmering: bool,
    /// Its lines, which keep the newest in view as they come while the user is at the end.
    scroll: ScrollHandle,
    /// Its rail of pages, which brings a tile into sight once as its word turns to Needs you.
    rail_scroll: ScrollHandle,
    /// The window's size as last drawn, and whether the lines stood in the panel beside a page:
    /// a change of either is a change of the lines' own column, which keeps them at their end if
    /// they were.
    viewport: (Size<Pixels>, bool),
    /// What the user opened or folded of its traces and their lines, which wins over what the
    /// trace would do by itself.
    opened: HashMap<Fold, bool>,
    /// Whether each was open as last drawn, and since when each has unfolded or folded.
    was_open: HashMap<Fold, bool>,
    folding: HashMap<Fold, Instant>,
    /// How tall what each opens to is, as last laid out, for it to unfold from and fold to.
    heights: Rc<RefCell<HashMap<Fold, f32>>>,
    /// When it was made, which the shimmer's band sweeps from.
    made: Instant,
    /// What moves as it changes, and where what it moves from and to was drawn last, in the
    /// window: the reply field, the chips of a question's answers, the plate of what the user sent
    /// as it lands, and the view.
    motion: Motion,
    reply_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Where an outside app's line was drawn last in the panel's head, for a test.
    from_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    answers_drawn: Rc<RefCell<HashMap<usize, Bounds<Pixels>>>>,
    /// Each ilo line's button, by its line's key, where it was drawn last and since when it has
    /// stood there: one that widens what the agent may do takes no press until it has stood a
    /// moment (`armed`).
    ilo_drawn: Rc<RefCell<HashMap<(usize, IloButton), (Bounds<Pixels>, Instant)>>>,
    /// Whether the pointer rests on such a button, which holds the lines still as more come.
    ilo_hovered: Rc<Cell<bool>>,
    /// The keyboard on a line's buttons, while the user's Focus command has it there, and the
    /// buttons' handles, in their order on the line (ilo_keys.rs).
    ilo_keys: Option<ilo_keys::IloKeys>,
    ilo_stops: [gpui::FocusHandle; ilo_keys::MOST],
    /// GPUI's focus still on a button whose keys ended, for the next frame to take from it.
    ilo_left: Option<ilo_keys::Left>,
    /// Where each of ilo's lines' words were last drawn, and their layout, by its line.
    ilo_words_drawn: WordsDrawn,
    /// Where each ilo line's link was drawn, by the line's key.
    ilo_links: Rc<RefCell<HashMap<usize, Bounds<Pixels>>>>,
    /// Where the composer's level trigger was drawn, if the last frame drew it, and its model's.
    level_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    model_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The composer's trigger whose menu is open, which wears its plate while it is.
    menu_open: Option<&'static str>,
    /// Where the folder's chip under the reply field was drawn.
    folder_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The plus before the level's chip, the add menu's (`crate::plus`).
    plus: crate::plus::Plus,
    /// The user's tabs picked for the reply being written, by their clicks in the add menu
    /// alone, and the watch on the window's tabs kept while there is one (picks.rs).
    picks: crate::tab_picks::Picks,
    picks_watch: Option<gpui::Subscription>,
    /// Where the last frame drew Take Over on the lines: once, on the live line.
    take_over_drawn: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    /// The page row's case as last logged, with the page it was for.
    row_logged: Cell<Option<(u64, &'static str)>>,
    /// Where the last frame drew the live line's Hand Back, while the user has the task's page.
    hand_back_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Where the panel's head drew the page's speaker this frame.
    speaker_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    landed: Rc<Cell<Option<Bounds<Pixels>>>>,
    drawn_at: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Whether the view was last drawn as the panel beside a page, so `drawn_at` is the panel's.
    drawn_standing: Cell<bool>,
    /// Whether an assistive app reads the window, as the last frame found: its lines have roles
    /// and names only then, so with none running a frame costs what it did (#4244).
    a11y: bool,
    /// What a screen reader is told as it comes while the conversation shows, and which telling
    /// it is: a line of ilo's in its own words, or the answer's first line. Each is a node of its
    /// own, so the same words coming again are told again.
    spoken: Option<(usize, SharedString)>,
    /// Its task finished, so the answer is told as the conversation comes with it.
    answer_due: bool,
}

/// What opens in a conversation: a trace, a line of one, or a turn's same step on several tabs,
/// by its key.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Fold {
    Trace(usize),
    Step(usize),
    Group(usize),
    /// A page in a stretch, by its first step's key.
    Page(usize),
}

impl Shell {
    /// A link in a thread's answer to anywhere but its own pages, at the user's click: it opens
    /// as a tab of the user's Space, and shows, as it always has, ⌘ held or not.
    pub(in crate::shell) fn open_link(&mut self, url: &str, cx: &mut Context<Self>) {
        // ⌘ takes this road too: opening behind lands in the Space the window shows, the thread's
        // while one of its pages is, and no call yet opens behind into the user's Space by its id.
        // The address is the user's to see, not the log's.
        log!("browser: task link out");
        self.leave_task_area(cx);
        self.browser.open_page(url, cx);
    }

    /// The task whose conversation the page area shows, for the Agents to send.
    pub fn talk_agent(&self) -> Option<SharedString> {
        let talk = self.talk.as_ref()?;
        (self.task_open() == Some(talk.space)).then(|| talk.agent.clone())
    }

    /// Where the conversation shown drew `tab`'s picture last, and the picture.
    pub(super) fn talk_thumb(
        &self,
        tab: u64,
        cx: &gpui::App,
    ) -> Option<(Bounds<Pixels>, Option<PagePicture>)> {
        self.talk.as_ref()?.view.read(cx).thumb(tab)
    }

    /// Where the conversation shown drew its reply field, for a test outside this file.
    #[cfg(test)]
    pub(super) fn reply_drawn(&self, cx: &gpui::App) -> Option<Bounds<Pixels>> {
        self.talk.as_ref()?.view.read(cx).reply_drawn.get()
    }

    /// The task whose conversation is kept for `space`'s area.
    pub(super) fn talk_in(&self, space: u64) -> Option<SharedString> {
        self.talk
            .as_ref()
            .filter(|talk| talk.space == space)
            .map(|talk| talk.agent.clone())
    }

    /// The task whose conversation is kept had no Space of its own, as one a run before left has
    /// none until its run opens a page, and its card has one the window knows now: the Space.
    pub(super) fn talk_takes_space(&mut self) -> Option<u64> {
        let talk = self.talk.as_mut().filter(|talk| talk.space == 0)?;
        let card = (self.task_cards.iter()).find(|card| card.agent == talk.agent)?;
        let space = card.space;
        let known = space != 0 && self.task_spaces.iter().any(|each| each.id == space);
        known.then(|| {
            talk.space = space;
            space
        })
    }

    /// The card's click: its conversation in the page area, or, shown already, the Space's tab
    /// again.
    /// A click on the notification a task's need posted (needs_you): its card's click, which never
    /// puts the task away if it shows already, as the user came to it.
    /// An ended task's Start Again: the command bar opens as it does for a new tab, with what the
    /// task was first asked in its field, as an ask. Nothing is sent until the user sends it.
    pub fn ask_again(&mut self, asked: SharedString, cx: &mut Context<Self>) {
        self.run(super::Command::NewTab, "start again", cx);
        self.browser.ask_again(asked, cx);
    }

    pub fn show_needed_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        if self.talk_agent().as_ref() != Some(&agent) {
            self.show_task(agent, cx);
        }
    }

    /// The tasks a run before left that haven't been brought back, the last to end first, each
    /// with the user's Space of its profile for its home (`Agents::kept_cards`). They have no card
    /// at the foot, which is for what's live or was opened in this run. A private window, which
    /// offers no tasks, lists none.
    pub fn set_kept_cards(&mut self, mut cards: Vec<Card>, cx: &mut Context<Self>) {
        if self.one_space() {
            cards.clear();
        }
        if cards != self.kept_cards {
            self.kept_cards = cards;
            cx.notify();
        }
    }

    /// The composer's Latest tasks, at most `most`: the tasks the user talks to whose home is the
    /// Space in front, the newest first, then those a run before left there, never another
    /// Space's. A card's home is the browser's own record of where it was asked, and a run
    /// before's is its profile's Space of the user's (`Agents::kept_cards`).
    pub(super) fn latest_tasks(&self, most: usize) -> Vec<Card> {
        let Some(here) = self.shown_space().map(|space| space.id) else {
            return Vec::new();
        };
        (self.task_cards.iter())
            .filter(|card| card.replies)
            .chain(&self.kept_cards)
            .filter(|card| card.home == here)
            .take(most)
            .cloned()
            .collect()
    }

    /// A click on a card of Latest tasks: one a run before left comes back from the Archive with
    /// its conversation open, as its row there brings it; any other shows its conversation.
    pub(super) fn open_latest(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        match self.kept_cards.iter().any(|card| card.agent == agent) {
            true => self.restore_archived_task(agent, cx),
            false => self.show_task(agent, cx),
        }
    }

    pub fn show_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        // A view set aside for a close's question is left for the one asked for now.
        self.forget_closing(cx);
        // A conversation opened leaves the empty page the composer's start would have opened.
        self.show_started = None;
        let Some(card) = self.task_cards.iter().find(|card| card.agent == agent) else {
            return;
        };
        if self.talk_agent().as_ref() == Some(&agent) {
            return self.close_task(cx);
        }
        let left = self.talk_agent();
        let (space, replies) = (card.space, card.replies);
        let view = match self.talk.take() {
            // Opened again, it shows its newest line, as the task went on while it was away.
            Some(talk) if talk.agent == agent => {
                talk.view.update(cx, |view, _| {
                    view.scroll.scroll_to_bottom();
                    view.motion.reset();
                });
                talk.view
            }
            old => {
                if let Some(old) = old {
                    self.talk_let_go(&old.agent, &old.view, cx);
                }
                // The reply's placeholder follows the appearance, as the lines do (#1659).
                let colors = Self::form_colors(cx);
                let reply = cx.new(|cx| {
                    (TextInput::new(words::REPLY, cx).follows(colors)).placeholder_after_caret()
                });
                let shell = cx.weak_entity();
                let ilo_stops = std::array::from_fn(|_| cx.focus_handle());
                cx.new(|_| TalkView {
                    shell,
                    agent: agent.clone(),
                    shown: None,
                    reply,
                    replies,
                    pictures: Vec::new(),
                    keeps_asked: Vec::new(),
                    unpictured: Vec::new(),
                    drawn: Rc::default(),
                    crashed_drawn: Rc::default(),
                    tiles: LiveTiles::new(self.browser.clone()),
                    drawn_logged: Rc::default(),
                    said_logged: RefCell::default(),
                    visited: HashSet::new(),
                    stop_logged: Rc::default(),
                    round_drawn: Rc::default(),
                    stale: false,
                    shimmering: false,
                    scroll: ScrollHandle::new(),
                    rail_scroll: ScrollHandle::new(),
                    viewport: Default::default(),
                    opened: HashMap::new(),
                    was_open: HashMap::new(),
                    folding: HashMap::new(),
                    heights: Rc::default(),
                    made: Instant::now(),
                    motion: Motion::default(),
                    reply_drawn: Rc::default(),
                    from_drawn: Rc::default(),
                    answers_drawn: Rc::default(),
                    ilo_drawn: Rc::default(),
                    ilo_hovered: Rc::default(),
                    ilo_keys: None,
                    ilo_stops,
                    ilo_left: None,
                    ilo_words_drawn: Rc::default(),
                    ilo_links: Rc::default(),
                    level_drawn: Rc::default(),
                    model_drawn: Rc::default(),
                    menu_open: None,
                    folder_drawn: Rc::default(),
                    plus: crate::plus::Plus::default(),
                    picks: Default::default(),
                    picks_watch: None,
                    take_over_drawn: Rc::default(),
                    row_logged: Cell::default(),
                    hand_back_drawn: Rc::default(),
                    speaker_drawn: Rc::default(),
                    landed: Rc::default(),
                    drawn_at: Rc::default(),
                    drawn_standing: Cell::default(),
                    a11y: false,
                    spoken: None,
                    answer_due: false,
                })
            }
        };
        // Its rail pictures its pages again, so its plate's goes.
        self.drop_plate(&agent);
        // check.sh reads this line.
        log!("browser: task {agent} shown");
        self.talk = Some(Talk {
            agent: agent.clone(),
            space,
            view: view.clone(),
        });
        self.open_task(space, view.into(), cx);
        self.browser.show_task(agent, cx);
        self.focus_talk = true;
        // The conversation it replaced was left.
        if let Some(left) = left {
            self.talk_left(left, cx);
        }
        cx.notify();
    }

    /// Draws the conversation shown again, as something of its pages changed that it reads from
    /// the window, such as whether one plays.
    pub(super) fn talk_redraw(&self, cx: &mut Context<Self>) {
        if let Some(talk) = &self.talk {
            talk.view.update(cx, |_, cx| cx.notify());
        }
    }

    /// The Agents' conversation of the task shown, as it changed. Its pages are pictured again as
    /// what it's doing changes, only while it shows, and not under a page watched over it.
    pub fn set_conversation(&mut self, conversation: Option<Conversation>, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        if self.task_shown().map(|(space, _)| space) != Some(talk.space) {
            return;
        }
        let watched = self.watched_task_page().is_some();
        let (view, browser) = (talk.view.clone(), self.browser.clone());
        let tabs = conversation.as_ref().map(|shown| &shown.tabs[..]);
        let keeps = self.keeps_pictures(tabs.unwrap_or_default(), cx);
        match view.update(cx, |view, cx| {
            view.set(conversation, watched, &*browser, &keeps, cx)
        }) {
            Some(stale) => self.stale_pictures.extend(stale),
            None => self.close_task(cx),
        }
    }

    /// The task whose conversation shows finished: a screen reader is told its answer as it
    /// comes (`TalkView::tell_answer`).
    pub(super) fn talk_answered(&mut self, cx: &mut Context<Self>) {
        if let Some(talk) = &self.talk {
            talk.view.update(cx, |view, _| view.tell_answer());
        }
    }

    /// A page watched over the conversation has gone back into its picture: the pictures are
    /// taken again if its pages changed meanwhile.
    pub(super) fn retake_talk_pictures(&mut self, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        let (view, browser) = (talk.view.clone(), self.browser.clone());
        let tabs = (view.read(cx).shown.as_ref()).map(|shown| shown.tabs.clone());
        let keeps = self.keeps_pictures(&tabs.unwrap_or_default(), cx);
        let stale = view.update(cx, |view, cx| match view.shown.clone() {
            Some(shown) if view.stale => view.set(Some(shown), false, &*browser, &keeps, cx),
            _ => None,
        });
        self.stale_pictures.extend(stale.unwrap_or_default());
    }

    /// The page area no longer shows the conversation: its pictures go, taken again if it shows
    /// again.
    pub(super) fn talk_closed(&mut self, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        let (agent, view) = (talk.agent.clone(), talk.view.clone());
        self.talk_let_go(&agent, &view, cx);
    }

    /// `agent`'s conversation lets its pictures go, but for its card's page's, which its plate in
    /// Latest tasks shows from then on.
    fn talk_let_go(
        &mut self,
        agent: &SharedString,
        view: &Entity<TalkView>,
        cx: &mut Context<Self>,
    ) {
        let card = self.task_cards.iter().find(|card| card.agent == *agent);
        let tab = card.and_then(|card| card.last_tab);
        let (kept, stale) = view.update(cx, |view, _| {
            let kept = tab.and_then(|tab| view.take_picture(tab));
            (kept, view.let_go())
        });
        self.stale_pictures.extend(stale);
        if let (Some(tab), Some(picture)) = (tab, kept) {
            self.drop_plate(agent);
            self.card_plates.push((agent.clone(), tab, picture));
        }
    }

    /// `agent`'s card's plate lets its picture go.
    fn drop_plate(&mut self, agent: &SharedString) {
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.card_plates)
            .into_iter()
            .partition(|(each, ..)| each == agent);
        self.card_plates = kept;
        self.stale_pictures
            .extend(gone.into_iter().map(|(.., picture)| picture.image()));
    }

    /// The plates' pictures of cards that no longer picture that page, or are gone, go; and so does
    /// a kept card's picture, read back after a quit, that no card of `was`'s successors holds now
    /// (Remove, Remove All, a reply), from this window's atlas if it drew it.
    pub(super) fn keep_plates(&mut self, was: &[Card]) {
        let held = |plate: &&Plate| {
            (self.task_cards.iter()).any(|card| card.kept_picture.as_ref() == Some(*plate))
        };
        let gone: Vec<_> = (was.iter().filter_map(|card| card.kept_picture.as_ref()))
            .filter(|plate| !held(plate))
            .map(|plate| plate.image.clone())
            .collect();
        self.stale_pictures.extend(gone);
        let cards = &self.task_cards;
        let (kept, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut self.card_plates)
            .into_iter()
            .partition(|(agent, tab, _)| {
                (cards.iter()).any(|card| card.agent == *agent && card.last_tab == Some(*tab))
            });
        self.card_plates = kept;
        self.stale_pictures
            .extend(gone.into_iter().map(|(.., picture)| picture.image()));
    }

    /// The picture a card of Latest tasks shows on its plate, if its conversation left one, or
    /// after a quit, its kept card's.
    pub(super) fn plate_of(&self, card: &Card) -> Option<PagePicture> {
        (self.card_plates.iter())
            .find(|(agent, tab, _)| *agent == card.agent && card.last_tab == Some(*tab))
            .map(|(.., picture)| picture.clone())
            .or_else(|| card.kept_picture.as_ref().map(PagePicture::kept))
    }

    /// A picture the conversation asked for came, or the core had none: whether it was one of its
    /// own. Its task's card is told of it, to keep for its Done, if it could have been given to an
    /// agent both when it was asked and now, and told it has none if it couldn't, picture or not;
    /// with no picture of a page that could be kept, it keeps what it had. A private window tells
    /// it nothing.
    pub(super) fn talk_pictured(
        &mut self,
        request: u64,
        picture: &Option<PagePicture>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(talk) = &self.talk else { return false };
        let (agent, view) = (talk.agent.clone(), talk.view.clone());
        let asked = view.read(cx).asked(request);
        let Some(old) = view.update(cx, |view, cx| view.pictured(request, picture, cx)) else {
            return false;
        };
        self.stale_pictures.extend(old);
        if let (Some((tab, keeps)), false) = (asked, self.private) {
            match (keeps && self.keeps_picture(tab, cx), picture) {
                (false, _) => self.browser.card_pictured(agent, tab, None, cx),
                (true, Some(picture)) => {
                    let image = Some(picture.image());
                    self.browser.card_pictured(agent, tab, image, cx)
                }
                (true, None) => {}
            }
        }
        true
    }

    /// Whether a picture of `tab` taken now could have been given to an agent: by the agents'
    /// rules, and not while its page uses the camera or the microphone, shares a screen, a window
    /// or a tab, or is captured. Never in a private window.
    fn keeps_picture(&self, tab: u64, cx: &gpui::App) -> bool {
        let page = self.page(tab);
        let in_use = page.camera || page.microphone || page.sharing || page.shared;
        // The address goes with the question: this is asked inside the shell's own update, where
        // nothing may read the shell for it.
        !self.private && !in_use && self.browser.keeps_picture(tab, &page.url, cx)
    }

    /// Those of `tabs` whose picture taken now could be kept.
    fn keeps_pictures(&self, tabs: &[(u64, SharedString)], cx: &gpui::App) -> Vec<u64> {
        (tabs.iter().map(|(tab, _)| *tab))
            .filter(|tab| self.keeps_picture(*tab, cx))
            .collect()
    }

    /// ⌘. with a conversation shown: its task stops, if it works or waits on the user.
    pub(super) fn stop_talk(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(agent) = self.talk_agent() else {
            return false;
        };
        let card = self.task_cards.iter().find(|card| card.agent == agent);
        if !card.is_some_and(|card| matches!(card.dot, Dot::Working | Dot::Waiting)) {
            return false;
        }
        // check.sh reads this line.
        log!("browser: command stop-task via conversation");
        self.browser.task(agent, TaskAction::Stop, cx);
        true
    }

    /// ⌘W with a conversation shown: its card closes, dismissing the task, which ⌘Z and ⇧⌘T bring
    /// back, a finished one's included, which stays in Today until then. Says whether there was one.
    pub(super) fn dismiss_talk(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(agent) = self.talk_agent() else {
            return false;
        };
        log!("browser: command dismiss-task via conversation");
        self.dismiss_task(agent, cx);
        self.close_task(cx);
        true
    }

    /// Gives the reply field the keyboard once the card's click has shown the conversation, and
    /// takes it back while a page watched covers the field: GPUI sends the keys of a field that
    /// isn't drawn to no binding of ours, so Esc stepped back from nothing.
    pub(super) fn focus_talk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_ilo_keys_drawn(window, cx);
        let Some(talk) = &self.talk else { return };
        let view = talk.view.read(cx);
        let (reply, replies) = (view.reply.focus_handle(cx), view.takes_reply());
        // A task that ended while the user was in its field may take no reply: the field went.
        let field_went = !replies && reply.is_focused(window);
        // Standing on a page, the field is beside it and keeps the keyboard the user gives it.
        if field_went || self.watched_task_page().is_some() && !self.task_standing() {
            if reply.is_focused(window) {
                window.focus(&self.focus_handle, cx);
            }
        } else if std::mem::take(&mut self.focus_talk) {
            // A session's has no field: the window has the keyboard, for Esc and ⌘..
            match replies {
                true => window.focus(&reply, cx),
                false => window.focus(&self.focus_handle, cx),
            }
        }
    }
}

impl TalkView {
    /// Takes `conversation`, asking for its pages' pictures again if what it does changed, unless
    /// a page is `watched` over it. Returns the pictures to let go of, or None if the task went.
    fn set(
        &mut self,
        conversation: Option<Conversation>,
        watched: bool,
        browser: &dyn Browser,
        keeps: &[u64],
        cx: &mut Context<Self>,
    ) -> Option<Vec<Arc<RenderImage>>> {
        let conversation = conversation.filter(|shown| shown.agent == self.agent)?;
        let moved = self.stale
            || self.shown.as_ref().is_none_or(|shown| {
                (
                    &shown.tabs,
                    shown.on,
                    shown.lines.len(),
                    steps(&shown.lines),
                ) != (
                    &conversation.tabs,
                    conversation.on,
                    conversation.lines.len(),
                    steps(&conversation.lines),
                )
            });
        let mut stale = Vec::new();
        if moved && watched {
            self.stale = true;
        } else if moved {
            self.stale = false;
            let mut pictures = Vec::new();
            self.keeps_asked.clear();
            for (tab, _) in &conversation.tabs {
                let old = self.pictures.iter_mut().find(|(each, ..)| each == tab);
                let request = next_request();
                if keeps.contains(tab) {
                    self.keeps_asked.push(request);
                }
                pictures.push((
                    *tab,
                    request,
                    old.and_then(|(_, _, picture)| picture.take()),
                ));
                browser.task_picture(*tab, request, cx);
            }
            let gone = std::mem::replace(&mut self.pictures, pictures);
            stale.extend(
                gone.into_iter()
                    .filter_map(|(_, _, picture)| Some(picture?.image())),
            );
        }
        if self.shown.as_ref() != Some(&conversation) {
            let now = Instant::now();
            self.motion.still = crate::motion::still(cx);
            if let Some(old) = self.shown.as_ref().filter(|_| self.motion.live) {
                self.motion.changed(old, &conversation, &self.opened, now);
            }
            // The newest line stays in view as lines come, while the user reads at the end.
            let grew = self.shown.as_ref().is_none_or(|shown| {
                (
                    shown.lines.len(),
                    steps(&shown.lines),
                    &shown.answers,
                    &shown.open,
                    said(&shown.lines),
                ) != (
                    conversation.lines.len(),
                    steps(&conversation.lines),
                    &conversation.answers,
                    &conversation.open,
                    said(&conversation.lines),
                )
            });
            let pinned = self.at_end() || self.motion.follow.is_some();
            match follows(
                grew,
                self.shown.is_none(),
                pinned,
                self.motion.live && !crate::motion::still(cx),
            ) {
                Follows::Not => {}
                Follows::AtOnce => self.scroll.scroll_to_bottom(),
                Follows::Eased => {
                    let at = f32::from(self.scroll.offset().y);
                    self.motion.follow = Some(Follow {
                        from: at,
                        since: now,
                        set: at,
                    });
                }
            }
            // A line of ilo's that comes while it shows is told once, in its own words: not under
            // a page watched over it, where it isn't drawn, and not beside a notification of what
            // its task came to with it (never both).
            if self.a11y
                && !watched
                && !crate::needs_you::notified(&conversation.agent, cx)
                && let Some(old) = &self.shown
            {
                let had = |key: usize| {
                    (old.lines.iter())
                        .any(|line| matches!(line, Line::Ilo(line) if line.key == key))
                };
                // Not the line of the user's own Stop: a task they stopped isn't told at all.
                let told = |line: &IloLine| !matches!(line.kind, IloKind::YouStopped { .. });
                let came = conversation.lines.iter().rev().find_map(|line| match line {
                    Line::Ilo(line) if !had(line.key) && told(line) => Some(line),
                    _ => None,
                });
                if let Some(line) = came {
                    self.tell(ilo_spoken(line, &conversation.name).into());
                }
            }
            // The field says what typing there does: steers the thread at work, else replies.
            let at_work = conversation.stoppable && conversation.live == Live::Working;
            let placeholder = words::reply_field(at_work);
            self.reply.update(cx, |reply, cx| {
                reply.set_placeholder(placeholder.into(), cx)
            });
            self.shown = Some(conversation);
            cx.notify();
        }
        if std::mem::take(&mut self.answer_due) {
            // By the name the app has for the assistant, never one its answer gives. An answer
            // with no words is told nothing, as the mark is only ever before the assistant's own.
            let answer = self.shown.as_ref().and_then(|shown| {
                let said = (shown.lines.iter().rev()).find_map(|line| match line {
                    Line::Said(text) => Some(text),
                    _ => None,
                })?;
                Some(words::says(&shown.name, &told_line(said)?))
            });
            if let Some(first) = answer {
                self.tell(first.into());
                cx.notify();
            }
        }
        Some(stale)
    }

    /// Tells a screen reader `said`, politely, once.
    fn tell(&mut self, said: SharedString) {
        let telling = self.spoken.as_ref().map_or(0, |(telling, _)| telling + 1);
        self.spoken = Some((telling, said));
    }

    /// Its task finished while it shows (`Shell::set_task_cards`): the answer is told once as it
    /// comes, as its first line, the one its card says, after the mark that says whose words they
    /// are (`words::says`). The rest is read in order.
    fn tell_answer(&mut self) {
        self.answer_due = self.a11y;
    }

    /// What a screen reader says as it comes, wherever its cursor is: the Mac's AccessKit
    /// announces a live region's value, and Windows' and Linux's its name (#3236).
    fn announcer(&self) -> Option<impl IntoElement + use<>> {
        let (telling, said) = self.spoken.clone().filter(|_| self.a11y)?;
        Some(
            div()
                .id(("conversation-told", telling))
                .absolute()
                .size_0()
                .role(Role::Status)
                .aria_label(said.clone())
                .aria_value(said)
                .a11y_synthetic_children(|builder| builder.parent_node().set_live(Telling::Polite)),
        )
    }

    /// Whether its lines are scrolled to their end, or all show.
    fn at_end(&self) -> bool {
        let (offset, max) = (self.scroll.offset(), self.scroll.max_offset());
        -offset.y >= max.y - px(2.)
    }

    /// The picture asked with `request` came. Returns the one it replaces to let go of, or None if
    /// it wasn't asked here.
    fn pictured(
        &mut self,
        request: u64,
        picture: &Option<PagePicture>,
        cx: &mut Context<Self>,
    ) -> Option<Vec<Arc<RenderImage>>> {
        // One a tile's end asked for stands from then, unless the conversation asked again since.
        let at_end = self.tiles.asked_at_end(request);
        let entry = self
            .pictures
            .iter_mut()
            .find(|(tab, each, _)| *each == request || (at_end == Some(*tab) && *each < request));
        let Some(entry) = entry else {
            return at_end.map(|_| picture.iter().map(PagePicture::image).collect());
        };
        entry.1 = request;
        let tab = entry.0;
        self.unpictured.retain(|each| *each != tab);
        let Some(picture) = picture else {
            if entry.2.is_none() {
                self.unpictured.push(tab);
                cx.notify();
            }
            return Some(Vec::new());
        };
        let old = entry.2.replace(picture.clone());
        cx.notify();
        Some(old.map(|old| old.image()).into_iter().collect())
    }

    /// The page `request` asked for a picture of, and whether that picture could have been given
    /// to an agent when it was asked.
    fn asked(&self, request: u64) -> Option<(u64, bool)> {
        let (tab, ..) = self.pictures.iter().find(|(_, each, _)| *each == request)?;
        Some((*tab, self.keeps_asked.contains(&request)))
    }

    /// `tab`'s picture, which it no longer holds.
    fn take_picture(&mut self, tab: u64) -> Option<PagePicture> {
        let at = self.pictures.iter().position(|(each, ..)| *each == tab)?;
        self.pictures.remove(at).2
    }

    /// Its pictures go, as it no longer shows, and are taken again when it does.
    fn let_go(&mut self) -> Vec<Arc<RenderImage>> {
        self.tiles.let_go();
        self.ilo_keys_undrawn("area");
        self.stale = true;
        self.motion.reset();
        let pictures = std::mem::take(&mut self.pictures);
        pictures
            .into_iter()
            .filter_map(|(_, _, picture)| Some(picture?.image()))
            .collect()
    }

    fn send_reply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.reply.read(cx);
        if input.composing() {
            return;
        }
        let text = input.text().trim().to_string();
        // A chip alone is a reply.
        if text.is_empty() && !self.picked() {
            return;
        }
        let tabs = self.take_picks(cx);
        // It goes from where it was typed to its plate, the field's box around its words.
        let moves = !text.is_empty() && !crate::motion::still(cx);
        if let Some(field) = self.reply_drawn.get().filter(|_| moves) {
            let style = window.text_style();
            let words = (window.text_system())
                .shape_line(
                    text.clone().into(),
                    px(text::BODY),
                    &[style.to_run(text.len())],
                    None,
                )
                .width;
            let width = (words + px(2. * COMPOSER_FIELD_X)).min(field.size.width);
            let from = Bounds {
                origin: field.origin,
                size: size(width, field.size.height),
            };
            let now = Instant::now();
            self.landed.set(None);
            self.motion.landing = Some(Landing {
                text: text.clone().into(),
                from,
                chip: false,
                sent: now,
                line: None,
            });
        }
        self.reply.update(cx, |input, cx| {
            input.reset(SharedString::default(), window, cx)
        });
        // What the user sent, and the Thinking under it, show even if they'd scrolled up, the
        // lines making room on the curve it lands on.
        match follows(
            true,
            false,
            true,
            self.motion.live && !crate::motion::still(cx),
        ) {
            Follows::Eased => {
                let at = f32::from(self.scroll.offset().y);
                self.motion.follow = Some(Follow {
                    from: at,
                    since: Instant::now(),
                    set: at,
                });
            }
            _ => self.scroll.scroll_to_bottom(),
        }
        let text = text.into();
        let reply = crate::agents::task::handed::Reply { text, tabs };
        self.act(TaskAction::Reply(reply), cx);
    }

    /// A question's answer, picked: it goes on from its chip as the user's reply, and the others
    /// go as their row closes.
    fn pick(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(answers) = self.shown.as_ref().map(|shown| shown.answers.clone()) else {
            return;
        };
        let Some(answer) = answers.get(index).cloned() else {
            return;
        };
        let from = self.answers_drawn.borrow().get(&index).copied();
        if let Some(from) = from.filter(|_| !crate::motion::still(cx)) {
            let now = Instant::now();
            self.landed.set(None);
            self.motion.landing = Some(Landing {
                text: answer.clone(),
                from,
                chip: true,
                sent: now,
                line: None,
            });
            self.motion.answered = Some(Answered {
                answers,
                picked: Some(index),
                at: 0,
                since: now,
            });
        }
        // A reply answers the ask, as the chip's words.
        self.act(TaskAction::Reply(answer.into()), cx);
    }

    /// ilo's line in the reply field's place: the task has ended and takes no reply (#4105).
    fn in_field(&self) -> Option<IloLine> {
        match self.shown.as_ref()?.lines.last()? {
            Line::Ilo(line) if ilo::in_field(&line.kind) => Some(line.clone()),
            _ => None,
        }
    }

    /// Whether the reply field shows: a task's, unless ilo's line stands in its place.
    pub(super) fn takes_reply(&self) -> bool {
        self.replies && self.in_field().is_none()
    }

    /// Start Again: the command bar opens with what the task was first asked, as an ask the user
    /// sends or changes. Once this view's own update is over, as the bar's opening reaches it.
    fn start_again(&self, cx: &mut Context<Self>) {
        let Some(asked) = self.shown.as_ref().map(|shown| shown.asked.clone()) else {
            return;
        };
        let shell = self.shell.clone();
        cx.defer(move |cx| {
            shell
                .update(cx, |shell, cx| shell.ask_again(asked, cx))
                .ok();
        });
    }

    /// The task's page in place beside the conversation, if one is: a step on it offers it back.
    fn beside(&self, cx: &gpui::App) -> Option<u64> {
        let shell = self.shell.upgrade()?;
        shell.read(cx).watched_task_page().map(|(_, tab)| tab)
    }

    /// The model trigger's menu, the bar's own, hanging under it: the thread's assistant's models
    /// alone, the one it runs on checked. Only the user's pick here changes it, from the next
    /// message.
    fn model_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(trigger), Some(menu)) = (
            self.model_drawn.get(),
            cx.try_global::<crate::menu::EntriesMenu>(),
        ) else {
            return;
        };
        let menu = menu.0.clone();
        let agent = self.agent.to_string();
        let Some((ids, items)) =
            crate::agents::global(cx).and_then(|agents| agents.read(cx).thread_model_menu(&agent))
        else {
            return;
        };
        let view = cx.entity().downgrade();
        let chosen: crate::text_input::Chosen = Box::new(move |closed, _, cx| {
            let action = (closed.chosen())
                .and_then(|item| ids.get(item))
                .map(|(id, _)| TaskAction::PickModel(id.clone()));
            let closed = |view: &mut Self, cx: &mut Context<Self>| {
                view.menu_open = None;
                cx.notify();
                if let Some(action) = action {
                    view.act(action, cx);
                }
            };
            view.update(cx, closed).ok();
        });
        self.menu_open = Some(MODEL);
        cx.notify();
        let at = point(trigger.left(), trigger.bottom() + px(CHOICES_MENU_DROP));
        menu(items, at, false, window, chosen, cx);
    }

    /// The level trigger's menu, the bar's own (choices.rs), hanging under it: the thread's level
    /// checked, and last the words table's `Settings…`, to where the levels are explained. Only
    /// the user's pick here changes it, through the press its refusal's button makes; there's no
    /// agent path to it.
    fn level_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(shown) = &self.shown else { return };
        let (Some(trigger), Some(menu)) = (
            self.level_drawn.get(),
            cx.try_global::<crate::menu::EntriesMenu>(),
        ) else {
            return;
        };
        let menu = menu.0.clone();
        let mut items = choices::level_entries(shown.tier);
        items.push(MenuEntry::line());
        items.push(MenuEntry::named(super::empty::composer::SETTINGS, true));
        let settings = items.len() - 1;
        let view = cx.entity().downgrade();
        let chosen: crate::text_input::Chosen = Box::new(move |closed, _, cx| {
            let picked = closed.chosen();
            if picked == Some(settings) {
                let shell = view.upgrade().map(|view| view.read(cx).shell.clone());
                if let Some(shell) = shell {
                    shell
                        .update(cx, |shell, cx| shell.browser.agent_access(cx))
                        .ok();
                }
            }
            let action = match picked {
                Some(item) => Tier::ALL.get(item).map(|tier| TaskAction::PickTier(*tier)),
                None => None,
            };
            let closed = |view: &mut Self, cx: &mut Context<Self>| {
                view.menu_open = None;
                cx.notify();
                if let Some(action) = action {
                    view.act(action, cx);
                }
            };
            view.update(cx, closed).ok();
        });
        self.menu_open = Some(LEVEL);
        cx.notify();
        let at = point(trigger.left(), trigger.bottom() + px(CHOICES_MENU_DROP));
        menu(items, at, false, window, chosen, cx);
    }

    fn act(&self, action: TaskAction, cx: &mut Context<Self>) {
        let agent = self.agent.clone();
        self.shell
            .update(cx, |shell, cx| shell.browser.task(agent, action, cx))
            .ok();
    }

    /// Where `tab`'s picture was drawn last, in the window, and the picture if it came: what its
    /// page zooms out of when its row at the sidebar's foot is clicked, as from a click here.
    fn thumb(&self, tab: u64) -> Option<(Bounds<Pixels>, Option<PagePicture>)> {
        let drawn = self.drawn.borrow();
        let (_, from) = drawn.iter().find(|(each, _)| *each == tab)?;
        let picture = (self.pictures.iter().find(|(each, ..)| *each == tab))
            .and_then(|(_, _, picture)| picture.clone());
        Some((*from, picture))
    }

    /// A link in its answer: the thread's page shows, as its tile's click shows it, or standing
    /// on one of its pages, the page beside the panel becomes it, as its button does.
    pub(super) fn show_page(&mut self, tab: u64, cx: &mut Context<Self>) {
        log!("browser: task link {tab}");
        let standing = (self.shell.upgrade()).is_some_and(|shell| shell.read(cx).task_standing());
        match standing {
            true => self.stand(tab, cx),
            false => self.watch(tab, cx),
        }
    }

    /// A picture's click: its page zooms out of it to fill the area.
    fn watch(&mut self, tab: u64, cx: &mut Context<Self>) {
        let from = self
            .drawn
            .borrow()
            .iter()
            .find(|(each, _)| *each == tab)
            .map(|(_, at)| *at);
        let Some(from) = from else { return };
        let picture = (self.pictures.iter().find(|(each, ..)| *each == tab))
            .and_then(|(_, _, picture)| picture.clone());
        // check.sh reads this line.
        log!("browser: task {} page {tab}", self.agent);
        let tiles = Some(self.tiles.clone());
        self.shell
            .update(cx, |shell, cx| {
                shell.watch_task_page_from(tab, from, picture, false, tiles, cx)
            })
            .ok();
    }

    /// The user's click on one of the panel's page buttons: the page beside it is `tab` at once,
    /// as a tab switch is, with no zoom out of the button.
    fn stand(&mut self, tab: u64, cx: &mut Context<Self>) {
        let picture = (self.pictures.iter().find(|(each, ..)| *each == tab))
            .and_then(|(_, _, picture)| picture.clone());
        // check.sh reads this line.
        log!("browser: task {} page {tab}", self.agent);
        let tiles = Some(self.tiles.clone());
        self.shell
            .update(cx, |shell, cx| {
                shell.stand_on_task_page(tab, picture, tiles, cx)
            })
            .ok();
    }

    /// Whether a trace or a line of one is open: as the user left it, else a trace while its
    /// stretch is under way, and a line not.
    pub(super) fn is_open(&self, fold: Fold, running: bool) -> bool {
        self.opened.get(&fold).copied().unwrap_or(running)
    }

    /// The user's click on a trace's line, which opens or folds it from then on.
    pub(super) fn toggle(&mut self, fold: Fold, running: bool, cx: &mut Context<Self>) {
        let open = self.is_open(fold, running);
        self.opened.insert(fold, !open);
        cx.notify();
    }

    /// Starts the motion of each trace or line whose open changed since it was last drawn, by the
    /// user's click or as its stretch ended.
    fn settle_folds(&mut self, now: Instant, still: bool) {
        let Some(shown) = &self.shown else { return };
        let mut folds = Vec::new();
        for line in &shown.lines {
            let steps: Vec<&TraceStep> = match line {
                Line::Trace(trace) => {
                    folds.push((
                        Fold::Trace(trace.key),
                        self.is_open(Fold::Trace(trace.key), trace.running),
                    ));
                    let paged = pages::paged(&trace.items);
                    let current = pages::current(&paged, trace.running);
                    for page in paged.iter().filter_map(|each| match each {
                        pages::Paged::Page(page) => Some(page),
                        pages::Paged::Item(..) => None,
                    }) {
                        let fold = Fold::Page(page.key);
                        folds.push((fold, self.is_open(fold, current == Some(page.key))));
                    }
                    (trace.items.iter())
                        .filter_map(|item| match item {
                            TraceItem::Step(step) | TraceItem::Helper { step, .. } => Some(step),
                            TraceItem::Said(_) => None,
                        })
                        .collect()
                }
                Line::Step(step) => vec![step],
                _ => continue,
            };
            for step in steps {
                let steps = match &step.opens {
                    Opens::Steps(steps) => {
                        folds.push((
                            Fold::Group(step.key),
                            self.is_open(Fold::Group(step.key), false),
                        ));
                        steps.as_slice()
                    }
                    _ => std::slice::from_ref(step),
                };
                for step in steps.iter().filter(|step| step.opens != Opens::Nothing) {
                    folds.push((
                        Fold::Step(step.key),
                        self.is_open(Fold::Step(step.key), false),
                    ));
                }
            }
        }
        for (fold, open) in folds {
            let was = self.was_open.insert(fold, open);
            if was.is_some_and(|was| was != open) && !still {
                self.folding.insert(fold, now);
            }
        }
        self.folding
            .retain(|_, since| row_spring((now - *since).as_secs_f32()).is_some());
    }

    /// What opens under a line: it unfolds and folds on Row motion, the rows under it moving with
    /// it, or None while folded. A trace whose turn ended folds on the task's ease instead, its
    /// steps fading first.
    fn unfolding(
        &self,
        fold: Fold,
        open: bool,
        content: gpui::Div,
        look: &Look,
    ) -> Option<gpui::AnyElement> {
        let now = look.now;
        let ended = match fold {
            Fold::Trace(key) if !open && !look.still => self.motion.folded(key),
            _ => None,
        };
        let done =
            (self.folding.get(&fold)).and_then(|since| row_spring((now - *since).as_secs_f32()));
        if !open && done.is_none() && ended.is_none() {
            return None;
        }
        let heights = self.heights.clone();
        let measure = canvas(
            move |bounds, _, _| {
                _ = heights
                    .borrow_mut()
                    .insert(fold, f32::from(bounds.size.height))
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let content = content.relative().flex_none().child(measure);
        if let Some(since) = ended {
            let full = self.heights.borrow().get(&fold).copied().unwrap_or(0.);
            let left = full * (1. - eased(since, now, HEIGHT));
            let fade = 1. - linear(since, now, STEPS_FADE);
            let folding = div()
                .flex_none()
                .h(px(left.max(0.)))
                .overflow_hidden()
                .opacity(fade);
            return Some(folding.child(content).into_any_element());
        }
        let Some(done) = done else {
            return Some(content.into_any_element());
        };
        let full = self.heights.borrow().get(&fold).copied().unwrap_or(0.);
        let height = if open {
            full * done
        } else {
            full * (1. - done)
        };
        Some(
            div()
                .flex_none()
                .h(px(height.max(0.)))
                .overflow_hidden()
                .child(content)
                .into_any_element(),
        )
    }

    /// The task's title and, under it, what it runs on and where it is, or how many pages it
    /// works on. An outside agent's session has Stop ⌘. at its right.
    fn header(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Div {
        let shown = self.shown.as_ref();
        let palette = look.palette;
        let title = shown.map(|shown| shown.title.clone()).unwrap_or_default();
        let about = shown.map(|shown| shown.about.clone()).unwrap_or_default();
        // Where it is rolls over as it changes, after what it runs on.
        let status = motion::status(&about).to_string();
        let runs = about[..about.len() - status.len()].to_string();
        let about = div()
            .flex()
            .child(div().min_w_0().truncate().child(runs))
            .child(rolled(status.into(), self.motion.status(), META_LINE, look));
        // A task's Stop is on the foot's line under its reply field; an outside agent's session
        // has no field, so its Stop stays here.
        let stop = (!self.replies).then(|| self.stop(look, cx)).flatten();
        let words = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .truncate()
                    .text_size(px(TITLE_SIZE))
                    .line_height(px(TITLE_LINE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text(TITLE))
                    .child(title),
            )
            .child(
                div()
                    .mt(px(META_GAP))
                    .min_w_0()
                    .whitespace_nowrap()
                    .text_size(px(text::BODY))
                    .line_height(px(META_LINE))
                    .text_color(palette.gray(TRACE_NOTE))
                    .child(about),
            );
        div()
            .flex_none()
            .mb(px(HEADER_BOTTOM))
            .flex()
            .items_center()
            .gap(px(BUBBLE_X))
            .child(words)
            .children(stop)
    }

    /// The head of the panel beside a page, in the stage's header's place: the thread's title, then
    /// its pages as buttons, the one shown on a plate and those the user hasn't stood on quiet. A
    /// click stands on that page.
    fn panel_head(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Div {
        let palette = look.palette;
        let shown = self.shown.as_ref();
        let title = shown.map(|shown| shown.title.clone()).unwrap_or_default();
        let tabs = shown.map_or_else(Vec::new, |shown| shown.tabs.clone());
        let on = self.beside(cx);
        let plate = match palette.tone {
            Tone::Dark => panel::PAGE_PLATE,
            Tone::Light => panel::LIGHT_PAGE_PLATE,
        };
        // Before the pages, whose buttons read the shell as they're drawn.
        let speaker = self.speaker(on, look, cx);
        let shell = self.shell.upgrade();
        let pages = tabs.into_iter().map(|(tab, site)| {
            let row = (shell.as_ref()).and_then(|shell| shell.read(cx).tabs.get(&tab).cloned());
            let name = (row.as_ref().map(|row| row.title.clone()))
                .filter(|title| !title.is_empty())
                .unwrap_or(site);
            let icon = match row.and_then(|row| row.favicon) {
                Some(image) => img(image).size(px(panel::PAGE_ICON)).into_any_element(),
                None => svg()
                    .path(PAGE_GLYPH)
                    .size(px(panel::PAGE_ICON))
                    .text_color(palette.text(SECONDARY))
                    .into_any_element(),
            };
            let shown = on == Some(tab);
            div()
                .id(("conversation-panel-page", tab))
                .when(look.a11y, |page| page.role(Role::Button).aria_label(name))
                .flex_none()
                .size(px(panel::PAGE))
                .rounded(px(panel::PAGE_RADIUS))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .when(shown, |page| page.bg(rgba(plate)))
                .when(!shown && !self.visited.contains(&tab), |page| {
                    page.opacity(panel::UNVISITED)
                })
                .child(icon)
                .on_click(cx.listener(move |view, _, _, cx| view.stand(tab, cx)))
        });
        // An outside app's session, from the kind of its connection alone, has the card's line
        // under its title: the product's mark and the name it gave.
        let name = shown.map(|shown| shown.name.clone()).unwrap_or_default();
        let drawn = self.from_drawn.clone();
        let from = (!self.replies).then(|| {
            div()
                .relative()
                .mt(px(panel::FROM_GAP))
                .h(px(FROM_LINE))
                .flex()
                .items_center()
                .gap(px(panel::FROM_NAME_GAP))
                .child(
                    canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
                .child(super::task_cards::from_mark(&palette))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(text::SMALL))
                        .line_height(px(FROM_LINE))
                        .text_color(answer_text_3(&palette))
                        .child(name),
                )
        });
        div()
            .flex_none()
            .w_full()
            .h(px(panel::HEAD))
            .pl(px(panel::HEAD_LEFT))
            .pr(px(panel::HEAD_RIGHT))
            .flex()
            .items_center()
            .gap(px(panel::HEAD_GAP))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::BODY))
                            .line_height(px(panel::TITLE_LINE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text(TITLE))
                            .child(title),
                    )
                    .children(from),
            )
            .children(speaker)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(panel::PAGES_GAP))
                    .children(pages),
            )
    }

    /// The page shown's speaker, left of the page buttons, drawn only while the page plays or is
    /// muted: crossed while it's muted, and a click mutes or unmutes it, as a row's does.
    fn speaker(
        &self,
        on: Option<u64>,
        look: &Look,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let tab = on?;
        let page = self.shell.upgrade()?.read(cx).page(tab);
        (page.audible || page.muted).then_some(())?;
        let palette = look.palette;
        let drawn = self.speaker_drawn.clone();
        Some(
            div()
                .id("conversation-speaker")
                .relative()
                .child(
                    canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
                .when(look.a11y, |speaker| {
                    let said = if page.muted { "Unmute" } else { "Mute" };
                    speaker.role(Role::Button).aria_label(said)
                })
                .flex_none()
                .size(px(panel::PAGE))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(
                    svg()
                        .path(super::rows::speaker_glyph(page.muted))
                        .size(px(panel::SPEAKER))
                        .text_color(palette.glyph(ICON_INK)),
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.shell
                        .update(cx, |shell, cx| shell.toggle_mute(tab, cx))
                        .ok();
                })),
        )
    }

    /// What the page row holds for `on`, the page stood on, `open` the words of Open while the
    /// thread has ended with that page to move. ilo's line in the field's place, which an ended
    /// thread has (#4105), leaves the row its Open, which nothing else there offers.
    pub(super) fn page_row_for(&self, on: u64, open: Option<String>) -> PageRow {
        let Some(shown) = self.shown.as_ref() else {
            return PageRow::default();
        };
        let back = shown.handed.contains(&on);
        if self.replies && self.in_field().is_some() {
            return PageRow {
                case: open.map(|open| RowCase::Ended(open.into())),
                stop: false,
                back,
            };
        }
        let case = match &shown.live {
            Live::Working if shown.stoppable && shown.on == Some(on) => {
                under_way(&shown.lines).map(|step| RowCase::Working(step.title.clone()))
            }
            Live::Yours { tab, .. } if shown.stoppable && *tab == on => {
                words::live(&shown.live).map(|said| RowCase::Yours(said.into()))
            }
            // A page the user took over is theirs on its own row, whatever the line is about.
            _ if shown.stoppable && shown.held.contains(&on) => {
                let yours = crate::agents::task::yours(on, &shown.tabs);
                words::live(&yours).map(|said| RowCase::Yours(said.into()))
            }
            _ => None,
        };
        PageRow {
            case: case.or_else(|| open.map(|open| RowCase::Ended(open.into()))),
            stop: !self.replies && shown.stoppable,
            back,
        }
    }

    /// The page's row over the panel's reply field, for the page the user stands on: while the
    /// thread works on it, the step's ring and the step under way with Take Over; on a page the
    /// user took over, what `words::live` says of it with Hand Back; once the thread has ended
    /// with the page to move, Open. An outside app's session has its Stop at the row's end, in
    /// the round button's form, as it has no composer. None with nothing to hold.
    fn page_row(&self, look: &Look, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let on = self.beside(cx)?;
        let shell = self.shell.upgrade()?;
        let row = self.page_row_for(on, shell.read(cx).task_strip_open(on));
        let case = match &row.case {
            Some(RowCase::Working(_)) => "working",
            Some(RowCase::Yours(_)) => "yours",
            Some(RowCase::Ended(_)) => "ended",
            None if row.back && !row.stop => "back",
            None => "stop",
        };
        let logged = (row != PageRow::default()).then_some((on, case));
        if self.row_logged.replace(logged) != logged
            && let Some((on, case)) = logged
        {
            // check.sh reads this line: which of its cases the row is in, and nothing of the page.
            log!("browser: page row {on} {case}");
        }
        logged?;
        let palette = look.palette;
        let ring = matches!(row.case, Some(RowCase::Working(_))).then(|| {
            ring::ring(
                STEP_RING,
                STEP_RING_STROKE,
                self.turned(look, cx),
                palette.text(RING_TRACK),
                palette.text(TITLE),
            )
        });
        let said = (row.case.as_ref()).and_then(|case| case.said().cloned());
        let words = said.clone().map(|said| {
            div()
                .id("conversation-page-words")
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(panel::LIVE))
                .line_height(px(panel::LIVE_LINE))
                .text_color(palette.gray(TRACE_NOTE))
                .child(said)
        });
        let button = row.case.as_ref().map(|case| {
            let (id, label, drawn): (_, SharedString, Box<dyn Fn(Bounds<Pixels>)>) = match case {
                RowCase::Working(_) => {
                    let drawn = self.take_over_drawn.clone();
                    let drawn = Box::new(move |at| drawn.borrow_mut().push(at));
                    ("conversation-take-over", words::TAKE_OVER.into(), drawn)
                }
                RowCase::Yours(_) => {
                    let drawn = self.hand_back_drawn.clone();
                    (
                        "conversation-hand-back",
                        words::HAND_BACK.into(),
                        Box::new(move |at| drawn.set(Some(at))),
                    )
                }
                RowCase::Ended(open) => {
                    let told = open_told(on, shell.read(cx).task_list.strip_open.clone());
                    ("conversation-open", open.clone(), Box::new(told))
                }
            };
            let click: Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App)> = match case {
                RowCase::Working(_) => Box::new(
                    cx.listener(move |view, _, _, cx| view.act(TaskAction::TakeOver(on), cx)),
                ),
                RowCase::Yours(_) => Box::new(
                    cx.listener(move |view, _, _, cx| view.act(TaskAction::HandBack(on), cx)),
                ),
                RowCase::Ended(_) => Box::new(cx.listener(move |view, _, _, cx| {
                    view.shell
                        .update(cx, |shell, cx| {
                            let (Some(agent), from) =
                                (shell.task_strip_agent(), shell.task_list.strip_open.get())
                            else {
                                return;
                            };
                            log!("browser: command open-task-pages via page row");
                            shell.open_task_pages(agent, Some(on), from, cx);
                        })
                        .ok();
                })),
            };
            (
                case.button(),
                label.clone(),
                held_button(palette, id, label, false)
                    .relative()
                    .child(
                        canvas(move |bounds, _, _| drawn(bounds), |_, _, _, _| {})
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                    )
                    .on_click(click),
            )
        });
        // An outside app's session: Stop in the composer's round button's form, as it has none.
        let stop = row.stop.then(|| {
            let form = Composer::of(true).row;
            let go: Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App)> =
                Box::new(cx.listener(|view, _, _, cx| {
                    view.shell.update(cx, |shell, cx| shell.stop_talk(cx)).ok();
                }));
            let told: Box<dyn Fn(Bounds<Pixels>)> = Box::new(stop_told(self.stop_logged.clone()));
            choices::round_button(
                form,
                self.inks(look),
                false,
                (
                    "conversation-row-stop",
                    words::STOP.into(),
                    choices::Round::Stop,
                    go,
                ),
                Some(told),
            )
        });
        let back = row
            .back
            .then(|| self.way_back_of(("conversation-way-back", on), on, look, cx));
        // Each is the ring's stop as well as the pointer's, so the keyboard reaches what's drawn.
        let shell = shell.read(cx);
        let words = words.map(|words| {
            shell.strip_stop(
                words,
                StripStop::Page,
                said.unwrap_or_default(),
                panel::PAGE_RADIUS,
            )
        });
        let button = button
            .map(|(stop, label, button)| shell.strip_stop(button, stop, label, HELD_BUTTON_RADIUS));
        let back = back.map(|back| {
            shell.strip_stop(
                back,
                StripStop::Return,
                words::RETURN_TAB.into(),
                panel::WAY_BACK_RADIUS,
            )
        });
        let stop = stop.map(|stop| {
            shell.strip_stop(
                stop,
                StripStop::Stop,
                words::STOP.into(),
                panel::COMPOSER_ROUND / 2.,
            )
        });
        let spring = words.is_none().then(|| div().flex_1());
        Some(
            div()
                .flex_none()
                .w_full()
                .h(px(panel::ROW))
                .mb(px(panel::FOOT_GAP))
                .pl(px(panel::ROW_LEFT))
                .flex()
                .items_center()
                .gap(px(panel::ROW_GAP))
                .children(ring)
                .children(words)
                .children(spring)
                .children(button)
                .children(back)
                .children(stop),
        )
    }

    /// The way back for `tab`, `id`: its click returns the tab to the user's tabs, and the pointer
    /// on it shows Z34's tip under it.
    fn way_back_of(
        &self,
        id: impl Into<ElementId>,
        tab: u64,
        look: &Look,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let view = cx.entity().downgrade();
        let shell = self.shell.clone();
        way_back(
            look.palette,
            look.a11y,
            id,
            move |cx| {
                log!("browser: command return-tab via way back");
                (view.update(cx, |view, cx| view.act(TaskAction::ReturnTab(tab), cx))).ok();
            },
            move |at, cx| {
                (shell.update(cx, |shell, cx| {
                    let tip = at.map(|at| {
                        let tip = crate::tooltip::Tip::named(words::RETURN_TAB.into());
                        (tip, crate::tooltip::Anchor::under(at, shell.viewport))
                    });
                    shell.hover_tip(tip, cx);
                }))
                .ok();
            },
        )
    }

    /// The composer row's inks, which the page row's Stop shares.
    fn inks(&self, look: &Look) -> choices::Inks {
        let palette = look.palette;
        choices::Inks {
            text: palette.text(TITLE),
            dim: palette.gray(SECONDARY),
            plate: palette.plate(BAR_CHIP),
            primary: primary(palette).0,
            on_primary: primary(palette).1,
        }
    }

    /// Stop ⌘., which comes and goes with the work it stops: at the end of a task's foot line, or
    /// of an outside agent's header.
    fn stop(&self, look: &Look, cx: &mut Context<Self>) -> Option<gpui::Stateful<gpui::Div>> {
        let palette = look.palette;
        // Stop comes and goes with the work it stops.
        let stoppable = (self.shown.as_ref()).is_some_and(|shown| shown.stoppable);
        let opacity = self.motion.stop(stoppable, look.now);
        (opacity > 0.).then(|| {
            let chip = page_chip(palette, "conversation-stop", words::STOP, Some(stop_keys()))
                .opacity(opacity);
            match stoppable {
                true => {
                    let told = stop_told(self.stop_logged.clone());
                    let at = canvas(move |bounds, _, _| told(bounds), |_, _, _, _| {})
                        .absolute()
                        .size_full();
                    chip.relative()
                        .child(at)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.shell.update(cx, |shell, cx| shell.stop_talk(cx)).ok();
                        }))
                }
                false => chip,
            }
        })
    }

    /// The task's pages, a rail of tiles beside the conversation's column, or across under its
    /// header where the area is narrow (`across`): each the page's row, its favicon, its title and
    /// its word, over its picture at the page's own shape, cut at the tile's foot. Only the page ilo
    /// acts in now is ringed. A click stands on the page; a tile never gives its page a key or a
    /// click. The pictures are taken when the agent acts (`set_conversation`), and over each the
    /// live page shows where it can (live_tiles.rs), at `drawn`.
    fn rail(&self, across: bool, look: &Look, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        self.drawn.borrow_mut().clear();
        self.crashed_drawn.borrow_mut().clear();
        let palette = look.palette;
        let tabs = (self.shown.as_ref()).map_or_else(Vec::new, |shown| shown.tabs.clone());
        // The picture's height at the page's own shape, of which the tile shows its top.
        let high = RAIL_WIDTH * PICTURE_HEIGHT / PICTURE_WIDTH;
        let shell = self.shell.upgrade();
        let tiles = tabs.iter().enumerate().map(|(index, (tab, site))| {
            let tab = *tab;
            let opacity = match self.motion.came(Key::Picture(tab)) {
                Some(since) if !look.still => eased(since, look.now, RISE),
                _ => 1.,
            };
            let image = (self.pictures.iter().find(|(each, ..)| *each == tab))
                .and_then(|(_, _, picture)| picture.as_ref().map(PagePicture::image));
            let (title, favicon, page, mark) = match (&shell, &self.shown) {
                (Some(shell), Some(shown)) => {
                    let shell = shell.read(cx);
                    let row = shell.tabs.get(&tab);
                    let title = (row.map(|row| row.title.clone()))
                        .filter(|title| !title.is_empty())
                        .unwrap_or_else(|| site.clone());
                    let favicon = row.and_then(|row| row.favicon.clone());
                    // Its row at the sidebar's foot says the same word: both ask the one function.
                    let card = shell
                        .task_cards
                        .iter()
                        .find(|card| card.agent == shown.agent);
                    let mark = card.map(|card| shell.page_mark(card, tab));
                    (title, favicon, shell.page(tab), mark)
                }
                _ => (site.clone(), None, Page::default(), None),
            };
            let said = mark.map(|mark| mark.word);
            // The outline, from the same answer as the word: the page the assistant is in, or
            // the one that needs the user at a handoff.
            let acting = mark.is_some_and(|mark| mark.outline);
            let pictured = image.is_some();
            // A page that died stands as its last picture, washed, under the crash card's glyph:
            // the state's, so both go once the page loads again, whoever loaded it.
            let crashed = page.trouble == Trouble::Crashed;
            let (drawn, logged) = (self.drawn.clone(), self.drawn_logged.clone());
            let paint = canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    drawn.borrow_mut().push((tab, bounds));
                    if logged.borrow_mut().insert(tab, bounds) != Some(bounds) {
                        // check.sh reads this line, to click the picture.
                        log!(
                            "browser: task picture {tab} at {:.0} {:.0} {:.0} {:.0}",
                            f32::from(bounds.origin.x),
                            f32::from(bounds.origin.y),
                            f32::from(bounds.size.width),
                            f32::from(bounds.size.height)
                        );
                    }
                    if let Some(image) = image {
                        // The page's own shape from the picture's top, cut at the tile's foot,
                        // whose corners it takes.
                        let shape = Bounds::new(bounds.origin, size(bounds.size.width, px(high)));
                        let corners = Corners {
                            bottom_left: px(TILE_RADIUS),
                            bottom_right: px(TILE_RADIUS),
                            ..Corners::default()
                        };
                        window
                            .paint_image(bounds, shape, corners, image, 0, false)
                            .ok();
                    }
                },
            )
            .absolute()
            .size_full();
            let icon = |size: f32| {
                let favicon = favicon.clone();
                div().flex_none().size(px(size)).child(match favicon {
                    Some(image) => img(image).size_full().into_any_element(),
                    None => svg()
                        .path("icons/globe.svg")
                        .size_full()
                        .text_color(palette.text(SECONDARY))
                        .into_any_element(),
                })
            };
            // Until its picture comes the tile is blank but for its favicon.
            let blank = (!pictured && !crashed).then(|| {
                div()
                    .absolute()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(PICTURE_EMPTY_GLYPH))
            });
            // The picture at the page's own shape, cut at the tile's foot.
            let picture = div()
                .relative()
                .w_full()
                .h(px(TILE_PICTURE))
                .overflow_hidden()
                .children(blank)
                .child(paint)
                .when(crashed, |picture| {
                    let glyph_drawn = self.crashed_drawn.clone();
                    let wash = Hsla {
                        a: PICTURE_CRASHED_WASH,
                        ..Hsla::from(palette.page)
                    };
                    picture.child(
                        div()
                            .absolute()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(pictured, |over| over.bg(wash))
                            .child(
                                div()
                                    .relative()
                                    .flex_none()
                                    .size(px(FAVICON_DRAWN))
                                    .child(
                                        canvas(
                                            |_, _, _| {},
                                            move |bounds, _, _, _| {
                                                glyph_drawn.borrow_mut().push((tab, bounds))
                                            },
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                                    .child(
                                        svg()
                                            .path(super::trouble::CARD_GLYPH)
                                            .size_full()
                                            .text_color(palette.text(SECONDARY)),
                                    ),
                            ),
                    )
                });
            // While its page plays, its speaker before its title, crossed while it's muted, as
            // an agent's tabs start: a click on it unmutes or mutes it, as a row's does (#1080).
            let sound = page.audible.then(|| {
                let shell = self.shell.clone();
                div()
                    .id(("conversation-page-sound", tab))
                    .flex_none()
                    .child(
                        svg()
                            .path(super::rows::speaker_glyph(page.muted))
                            .size(px(RAIL_FAVICON))
                            .text_color(palette.text(SECONDARY)),
                    )
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        shell
                            .update(cx, |shell, cx| shell.toggle_mute(tab, cx))
                            .ok();
                    })
                    .when(look.behind, |sound| sound.relative().child(unpressed()))
            });
            // A dead page's tile says its name alone: the word comes back once the page loads.
            let word = said.map_or("", PageWord::label);
            let says = if crashed { "" } else { word };
            let was = (self.said_logged.borrow_mut()).insert(tab, (says, word));
            if was != Some((says, word)) {
                // check.sh reads this line.
                log!("browser: task picture {tab} says {says}");
                // Once, as it turns: a tile the user scrolls away again stays away, and its page
                // opens as itself unless the user has put another of the task's pages there.
                // The panel beside it keeps the reply field, so it covers nothing the user types.
                // By the page's state, which a death and the load after it leave as it was.
                let turned = was.is_none_or(|(_, before)| before != word);
                if said == Some(PageWord::NeedsYou) && turned {
                    self.rail_scroll.scroll_to_item(index);
                    let shell = self.shell.clone();
                    cx.defer(move |cx| {
                        shell
                            .update(cx, |shell, cx| shell.open_needing_page(tab, cx))
                            .ok();
                    });
                }
            }
            // The page the assistant is on wears the ring before its word, turning while it works.
            let ring = acting.then(|| {
                let turned = match (&shell, said) {
                    (Some(shell), Some(PageWord::Working)) => {
                        let shell = shell.clone();
                        let turned = (shell.read(cx).breath)
                            .turned(Duration::from_secs_f32(RING_TURN), look.now);
                        cx.defer(move |cx| shell.update(cx, |shell, cx| shell.breath.arm(cx)));
                        turned
                    }
                    _ => 0.,
                };
                ring::ring(
                    TILE_RING,
                    TILE_RING_STROKE,
                    turned,
                    palette.text(RING_TRACK),
                    palette.text(TITLE),
                )
            });
            let word = said.filter(|_| !crashed).map(|said| {
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(SITE_GAP))
                    .text_size(px(RAIL_WORD))
                    .line_height(px(RAIL_WORD_LINE))
                    // The page the assistant is on, and a word that asks for the user, are in the
                    // text's ink; the others are quiet.
                    .text_color(match said {
                        _ if acting => palette.text(TITLE),
                        PageWord::NeedsYou | PageWord::Driving => palette.text(TITLE),
                        _ => palette.gray(STATUS),
                    })
                    .children(ring)
                    .child(said.label())
            });
            // A tab the user handed the thread has its way back at the head's end, which no page
            // the thread opened has.
            let handed = (self.shown.as_ref()).is_some_and(|shown| shown.handed.contains(&tab));
            let back = handed
                .then(|| self.way_back_of(("conversation-tile-way-back", tab), tab, look, cx));
            // Spoken, a dead page's tile is its name, then the card's own heading; a handed tab's
            // is its own words, then Z35, which nothing draws.
            let spoken = look.a11y.then(|| {
                let mut said = vec![title.to_string()];
                said.extend(crashed.then(|| super::trouble::STOPPED.to_string()));
                said.extend(handed.then(|| words::YOUR_TAB.to_string()));
                said.join(", ")
            });
            let spoken = spoken.filter(|_| crashed || handed);
            let head = div()
                .flex_none()
                .h(px(RAIL_HEAD))
                .w_full()
                .pl(px(RAIL_HEAD_X))
                .pr(px(if handed {
                    panel::WAY_BACK_HEAD_RIGHT
                } else {
                    RAIL_HEAD_X
                }))
                .flex()
                .items_center()
                .gap(px(RAIL_HEAD_GAP))
                .child(icon(RAIL_FAVICON))
                .children(sound)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(RAIL_TITLE))
                        .line_height(px(RAIL_TITLE_LINE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.text(TITLE))
                        .child(title),
                )
                .children(word)
                .children(back);
            let (fill, line) = match palette.tone {
                Tone::Dark => (TILE_FILL, if acting { TILE_LINE_ON } else { TILE_LINE }),
                Tone::Light => (
                    LIGHT_TILE_FILL,
                    if acting {
                        LIGHT_TILE_LINE_ON
                    } else {
                        LIGHT_TILE_LINE
                    },
                ),
            };
            div()
                .id(("conversation-page", tab))
                .relative()
                .flex_none()
                .w(px(RAIL_WIDTH))
                .rounded(px(TILE_RADIUS))
                .bg(rgba(fill))
                .overflow_hidden()
                .flex()
                .flex_col()
                .opacity(opacity)
                .cursor_pointer()
                .when_some(spoken, |tile, spoken| {
                    tile.role(Role::Button).aria_label(spoken)
                })
                .child(head)
                .child(picture)
                // Its inner line, over the picture.
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded(px(TILE_RADIUS))
                        .border(px(HAIRLINE))
                        .border_color(rgba(line)),
                )
                .on_click(cx.listener(move |view, _, _, cx| view.watch(tab, cx)))
        });
        let rail = div()
            .id("conversation-pages")
            .flex()
            .gap(px(RAIL_GAP))
            .track_scroll(&self.rail_scroll)
            .children(tiles);
        match across {
            true => rail.w_full().overflow_x_scroll(),
            false => rail.size_full().flex_col().overflow_y_scroll(),
        }
    }

    /// What the agent said, as its Markdown marks it: its paragraphs and lists, with bold, italic,
    /// code and links, a link's click opening its page as a tab of the user's. Each of its texts
    /// is `name` and `number`'s, which tell it from the others in the lines. The `runs` of it that
    /// came last fade in.
    fn said(
        &self,
        name: &'static str,
        number: usize,
        text: &str,
        runs: &[Run],
        look: &Look,
    ) -> gpui::Div {
        let mut column = div().flex().flex_col();
        let mut items = false;
        for (index, block) in markdown::blocks(text).into_iter().enumerate() {
            let id = ElementId::NamedInteger(name.into(), ((number as u64) << 16) | index as u64);
            let fading =
                |text: &Inline| motion::fading(runs, index, text.text.chars().count(), look.now);
            let (item, block) = match block {
                Block::Paragraph(text) => {
                    let fading = fading(&text);
                    (false, self.inline(id, text, &fading, look))
                }
                Block::Item {
                    depth,
                    marker,
                    text,
                } => {
                    let fading = fading(&text);
                    // Its marker comes with its first word.
                    let marker_opacity = fading
                        .first()
                        .filter(|(range, _)| range.start == 0)
                        .map_or(1., |(_, alpha)| *alpha);
                    let item = div()
                        .flex()
                        .pl(px(depth as f32 * LIST_MARKER))
                        .child(
                            div()
                                .flex_none()
                                .w(px(LIST_MARKER))
                                .opacity(marker_opacity)
                                .child(marker),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(self.inline(id, text, &fading, look)),
                        );
                    (true, item.into_any_element())
                }
            };
            let gap = match (index, item && items) {
                (0, _) => 0.,
                (_, true) => ITEM_GAP,
                _ => PARAGRAPH_GAP,
            };
            column = column.child(div().mt(px(gap)).child(block));
            items = item;
        }
        column
    }

    /// A block's text with its marks, and its links, if any, which take a click. The characters
    /// `fading` in show at their opacity.
    fn inline(
        &self,
        id: ElementId,
        inline: Inline,
        fading: &[(std::ops::Range<usize>, f32)],
        look: &Look,
    ) -> gpui::AnyElement {
        let palette = look.palette;
        let inline = honest(inline);
        let bytes = |char: usize| {
            inline
                .text
                .char_indices()
                .nth(char)
                .map_or(inline.text.len(), |(at, _)| at)
        };
        let fades: Vec<_> = (fading.iter())
            .map(|(range, alpha)| {
                let style = HighlightStyle {
                    fade_out: Some(1. - alpha),
                    ..HighlightStyle::default()
                };
                (bytes(range.start)..bytes(range.end), style)
            })
            .collect();
        // A link to a page of this thread shows it; one to the web opens a tab of the user's Space,
        // as a link always has; one to anything else stays plain words.
        let links: Vec<(std::ops::Range<usize>, Linked)> = (inline.links.iter())
            .filter_map(|(range, url)| Some((range.clone(), linked_to(&look.thread_pages, url)?)))
            .collect();
        let linked = |range: &std::ops::Range<usize>| {
            (links.iter()).any(|(each, _)| each.start <= range.start && range.end <= each.end)
        };
        let highlights = (inline.styled.iter()).map(|(range, style)| {
            let underline = UnderlineStyle {
                thickness: px(LINK_LINE),
                color: Some(palette.text(LINK_UNDERLINE)),
                wavy: false,
            };
            let highlight = HighlightStyle {
                font_weight: style.bold.then_some(FontWeight::SEMIBOLD),
                font_style: style.italic.then_some(FontStyle::Italic),
                background_color: style.code.then(|| palette.plate(CODE_FILL)),
                underline: (style.link && linked(range)).then_some(underline),
                ..HighlightStyle::default()
            };
            (range.clone(), highlight)
        });
        let highlights: Vec<_> =
            gpui::combine_highlights(highlights.collect::<Vec<_>>(), fades).collect();
        let text = StyledText::new(inline.text).with_highlights(highlights);
        if links.is_empty() {
            return text.into_any_element();
        }
        let (ranges, targets): (Vec<_>, Vec<Linked>) = links.into_iter().unzip();
        // Under the pointer a web link's tooltip says its address, which nothing else here shows.
        let tips: Vec<(std::ops::Range<usize>, (SharedString, SharedString))> = (ranges.iter())
            .zip(&targets)
            .filter_map(|(range, target)| match target {
                Linked::Web(url) => Some((range.clone(), link_tip(url)?)),
                Linked::Page(_) => None,
            })
            .collect();
        let tipped = self.shell.clone();
        let shell = self.shell.clone();
        InteractiveText::new(id, text)
            .when(!tips.is_empty(), |text| {
                text.on_hover(move |at, event, _, cx| {
                    let tip = at.and_then(|at| tips.iter().find(|(range, _)| range.contains(&at)));
                    let Some(shell) = tipped.upgrade() else {
                        return;
                    };
                    shell.update(cx, |shell, cx| {
                        let tip = tip.map(|(_, (host, rest))| {
                            let at = Bounds::new(event.position, size(px(1.), px(1.)));
                            let tip = crate::tooltip::Tip::detailed(host.clone(), rest.clone());
                            (tip, crate::tooltip::Anchor::under(at, shell.viewport))
                        });
                        shell.hover_tip(tip, cx);
                    });
                })
            })
            .on_click(ranges, move |index, _, cx| {
                let Some(target) = targets.get(index) else {
                    return;
                };
                let Some(shell) = shell.upgrade() else { return };
                match target {
                    Linked::Page(tab) => {
                        let view = (shell.read(cx).talk.as_ref()).map(|talk| talk.view.clone());
                        if let Some(view) = view {
                            view.update(cx, |view, cx| view.show_page(*tab, cx));
                        }
                    }
                    Linked::Web(url) => {
                        shell.update(cx, |shell, cx| shell.open_link(url, cx));
                    }
                }
            })
            .into_any_element()
    }

    /// A message of the user's, on its plate at the column's right: the first with the page they
    /// asked on, as a chip over its text. One `landing` holds its place, unseen, while what was
    /// sent goes there.
    fn you(
        &self,
        id: ElementId,
        text: SharedString,
        page: Option<(SharedString, Option<Arc<Image>>)>,
        said: usize,
        landing: bool,
        look: &Look,
    ) -> gpui::AnyElement {
        let palette = look.palette;
        let bubble = Bubble::of(look.standing);
        // Its tabs' chips, after the page asked on and over its words; all of it with no words.
        let tabs = (self.sent_chips(said, look))
            .map(|chips| chips.when(!text.is_empty(), |chips| chips.mb(px(PAGE_CHIP_BELOW))));
        let chip = page.map(|(title, favicon)| {
            let icon = match favicon {
                Some(favicon) => img(favicon).size(px(PAGE_CHIP_FAVICON)).into_any_element(),
                None => svg()
                    .path("icons/globe.svg")
                    .size(px(PAGE_CHIP_FAVICON))
                    .text_color(palette.text(SECONDARY))
                    .into_any_element(),
            };
            div().flex().mb(px(PAGE_CHIP_BELOW)).child(
                div()
                    .h(px(PAGE_CHIP_HEIGHT))
                    .max_w_full()
                    .ml(px(-PAGE_CHIP_HANG))
                    .px(px(PAGE_CHIP_X))
                    .flex()
                    .items_center()
                    .gap(px(PAGE_CHIP_GAP))
                    .rounded(px(CHIP_RADIUS))
                    .bg(palette.plate(PAGE_CHIP_FILL))
                    .text_size(px(text::SMALL))
                    .font_weight(FontWeight::MEDIUM)
                    .child(icon)
                    .child(div().min_w_0().truncate().child(title)),
            )
        });
        div()
            .flex()
            .justify_end()
            .mb(px(BLOCK_GAP))
            .child(
                div()
                    .id(id)
                    .when(look.a11y, |plate| {
                        plate.role(Role::Label).aria_label(text.clone())
                    })
                    .min_w_0()
                    .max_w(px(bubble.most))
                    .pt(px(bubble.top))
                    .pb(px(bubble.bottom))
                    .px(px(bubble.x))
                    .rounded(px(RADIUS))
                    .bg(bubble_fill(&palette))
                    .text_size(px(bubble.text))
                    .line_height(px(bubble.line))
                    .children(chip)
                    .children(tabs)
                    .children((!text.is_empty()).then_some(text))
                    .when(landing, |plate| {
                        let landed = self.landed.clone();
                        plate.relative().opacity(0.).child(
                            canvas(
                                move |bounds, _, _| landed.set(Some(bounds)),
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .size_full(),
                        )
                    }),
            )
            .into_any_element()
    }

    /// A trace: its summary line, and its steps under it while it's open.
    fn trace(&self, trace: &Trace, look: &Look, cx: &mut Context<Self>) -> gpui::AnyElement {
        let fold = Fold::Trace(trace.key);
        let open = self.is_open(fold, trace.running);
        let running = trace.running;
        let mut line = summary_line(trace, look.now);
        line.chevron = Some(self.chevron(fold, open, look));
        self.crossfade(fold, &mut line, look);
        let mut summary_icons = Vec::new();
        let paged = pages::paged(&trace.items);
        // A done stretch's fold shows the pages it visited.
        if trace.took.is_some() {
            line_icons(&mut summary_icons, &paged, look);
        }
        let current = pages::current(&paged, trace.running);
        let mut nest = nest();
        for (at, each) in paged.iter().enumerate() {
            // A run of helpers has the line that says the work split over it.
            let split = pages::split_at(&paged, at).and_then(words::split);
            if let (Some(split), pages::Paged::Item(_, TraceItem::Helper { step, .. })) =
                (split, each)
            {
                let mut line = TraceLine::new(LineKey::Split(step.key), split.into(), "".into());
                line.glyph = Some(SPLIT_GLYPH);
                let line = trace_line(look, line).into_any_element();
                nest = nest.child(risen(line, self.motion.came(Key::Step(step.key)), look));
            }
            let (index, item) = match each {
                pages::Paged::Page(page) => {
                    nest = nest.children(self.page(page, current == Some(page.key), look, cx));
                    continue;
                }
                pages::Paged::Item(index, item) => (*index, *item),
            };
            nest = match item {
                TraceItem::Step(step) => nest.children(self.step(step, true, look, cx)),
                // A helper's line has no Take Over, which is the one page's the task stands on.
                TraceItem::Helper { step, name, tab } => {
                    let page = tab.and_then(|tab| look.helper_page(tab));
                    match helper_row(step, name, page) {
                        HelperRow::Known(mut line) => {
                            // At its right, the step it's at, with the ring.
                            if step.running && !step.title.is_empty() {
                                line.note = step.title.clone();
                                line.ring_after = Some(self.turned(look, cx));
                            }
                            line.joined_above = true;
                            self.crossfade(Fold::Step(step.key), &mut line, look);
                            let line = trace_line(look, line).into_any_element();
                            let line = risen(line, self.motion.came(Key::Step(step.key)), look);
                            // Its step's note is under its own line, never the task's.
                            let says = self.says(step, TRACE_GLYPH + TRACE_GAP, look);
                            nest.child(line).children(says)
                        }
                        HelperRow::Step => nest.children(self.step_line(step, true, look, cx)),
                        HelperRow::Nothing => nest,
                    }
                }
                TraceItem::Said(text) => {
                    // Said words are in the text's ink, between steps as in the answer: only a
                    // thought is quiet, with its dots.
                    let said = div()
                        .py(px((TRACE_LINE - TEXT_LINE) / 2.))
                        .text_color(said_ink(&look.palette))
                        .child(self.said("trace-said", (trace.key << 16) | index, text, &[], look));
                    nest.child(risen(
                        said.into_any_element(),
                        self.motion.came(Key::Aside(trace.key, index)),
                        look,
                    ))
                }
            };
        }
        line.icons = summary_icons;
        let summary = trace_line(look, line)
            .on_click(cx.listener(move |view, _, _, cx| view.toggle(fold, running, cx)));
        div()
            .flex()
            .flex_col()
            .items_start()
            .mb(px(TRACE_BELOW))
            .child(summary)
            .children(self.unfolding(fold, open, nest.w_full(), look))
            .into_any_element()
    }

    /// A page's line, its icon and title, and its steps under it while it's open: the page the
    /// assistant is on, in the text's ink, open until the user folds it; a page it has left, one
    /// line in text-2 that a click opens.
    fn page(
        &self,
        page: &pages::Page,
        current: bool,
        look: &Look,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let fold = Fold::Page(page.key);
        let open = self.is_open(fold, current);
        let head = look.page_head(page.tab);
        // The title its steps saw, never the tab's now: a tab that went on to another page would
        // head every page it was with the last one's.
        let title = match page.title.is_empty() {
            true => page.site.clone(),
            false => page.title.clone(),
        };
        let mut line = TraceLine::new(fold, title, "".into());
        line.page = Some(current);
        line.icon = head.and_then(|head| head.favicon.clone());
        line.glyph = line.icon.is_none().then_some(PAGE_GLYPH);
        let line = trace_line(look, line)
            .on_click(cx.listener(move |view, _, _, cx| view.toggle(fold, current, cx)));
        let line = risen(
            line.into_any_element(),
            self.motion.came(Key::Step(page.key)),
            look,
        );
        let steps = nest().children(
            page.shown()
                .flat_map(|step| self.step(step, true, look, cx)),
        );
        std::iter::once(line)
            .chain(self.unfolding(fold, open, steps.w_full(), look))
            .collect()
    }

    /// The turn the user sent, until its trace or the agent's message takes its place: Thinking,
    /// shimmering as the step under way does, or why the agent couldn't start on it.
    fn thinking(&self, key: usize, failed: Option<SharedString>, look: &Look) -> gpui::AnyElement {
        let fold = Fold::Trace(key);
        let line = match failed {
            None => TraceLine {
                shimmers: Some(look.shimmer(self.made)),
                ..TraceLine::new(fold, "Thinking".into(), "".into())
            },
            Some(why) => TraceLine {
                failed: true,
                ..TraceLine::new(fold, "Couldn't start".into(), why)
            },
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .mb(px(TRACE_BELOW))
            .child(trace_line(look, line))
            .into_any_element()
    }

    /// How open a line's chevron is, from 0 folded to 1 open: it turns as the line opens or folds,
    /// by the user's click or as its turn ends.
    fn chevron(&self, fold: Fold, open: bool, look: &Look) -> f32 {
        let to = if open { 1. } else { 0. };
        let ended = match fold {
            Fold::Trace(key) => self.motion.folded(key),
            _ => None,
        };
        let since = ended
            .or_else(|| self.folding.get(&fold).copied())
            .filter(|_| !look.still);
        let Some(since) = since else { return to };
        let turned = eased(since, look.now, CHEVRON);
        if open { turned } else { 1. - turned }
    }

    /// A line whose words changed, as `swapped`, crossfades to them from the old: Thinking to the
    /// trace it became, Working to how long it worked, a step to its past tense. A chevron it
    /// didn't have comes with its new words.
    fn crossfade(&self, swapped: Fold, line: &mut TraceLine, look: &Look) {
        let Some(was) = self.motion.swapped(swapped) else {
            return;
        };
        let (old, alpha) = motion::swap(was.since, look.now);
        if old {
            line.title = was.title.clone();
            line.note = was.note.clone();
            line.rolled = None;
            line.shimmers = None;
        }
        line.alpha = alpha;
        line.note_alpha = alpha;
        if !was.chevron {
            line.chevron_alpha = if old { 0. } else { alpha };
        }
    }

    /// Whether something it shows moves: the step under way's shimmer while its trace is open, or
    /// a turn's Thinking, unless motion is reduced, and what opens or folds.
    fn moving(&self, still: bool) -> bool {
        let shimmers = self.shown.as_ref().is_some_and(|shown| {
            shown.lines.iter().any(|line| match line {
                Line::Trace(trace) => {
                    let under_way = |item: &TraceItem| match item {
                        TraceItem::Step(step) | TraceItem::Helper { step, .. } => step.running,
                        TraceItem::Said(_) => false,
                    };
                    trace.running
                        && self.is_open(Fold::Trace(trace.key), true)
                        && trace.items.iter().any(under_way)
                }
                Line::Thinking { failed, .. } => failed.is_none(),
                Line::Step(step) => step.running,
                _ => false,
            })
        });
        (shimmers && !still) || !self.folding.is_empty()
    }

    /// A step's line, and what it opens to while it's open. A turn's same step on several tabs
    /// opens to each tab's line, by its address under the step's title and with no glyph, so the
    /// addresses read as one column. A step rises in as it comes, and its title crossfades to its
    /// past tense as it finishes, its note fading in after.
    fn step(
        &self,
        step: &TraceStep,
        glyph: bool,
        look: &Look,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        self.step_line(step, glyph, look, cx)
    }

    /// How far round the running ring is now, on the shell's breath, which draws the next frame
    /// only while a ring is drawn.
    fn turned(&self, look: &Look, cx: &mut Context<Self>) -> f32 {
        let Some(shell) = self.shell.upgrade() else {
            return 0.;
        };
        let turned = (shell.read(cx).breath).turned(Duration::from_secs_f32(RING_TURN), look.now);
        cx.defer(move |cx| shell.update(cx, |shell, cx| shell.breath.arm(cx)));
        turned
    }

    /// As `step`, with its glyph or, as a helper's, without.
    fn step_line(
        &self,
        step: &TraceStep,
        glyph: bool,
        look: &Look,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let fold = match step.opens {
            Opens::Steps(_) => Fold::Group(step.key),
            _ => Fold::Step(step.key),
        };
        let opens = step.opens != Opens::Nothing;
        let open = opens && self.is_open(fold, false);
        let mut line = TraceLine::new(fold, step.title.clone(), step.description.clone());
        line.glyph = glyph.then_some(step.glyph);
        line.failed = step.failed;
        line.chevron = opens.then(|| self.chevron(fold, open, look));
        // The step under way wears the ring, and every step after the first joins the one above.
        line.ring = (step.running && glyph).then(|| self.turned(look, cx));
        line.shimmers = (step.running && !glyph).then(|| look.shimmer(self.made));
        line.joined_above = glyph;
        self.crossfade(Fold::Step(step.key), &mut line, look);
        line.note_alpha = line.note_alpha.min(self.motion.note(step.key, look.now));
        let mut line = trace_line(look, line);
        if opens {
            line = line.on_click(cx.listener(move |view, _, _, cx| view.toggle(fold, false, cx)));
        }
        let line = line.into_any_element();
        let line = risen(line, self.motion.came(Key::Step(step.key)), look);
        let palette = look.palette;
        // What a line opens to starts under its title.
        let under = if glyph { TRACE_GLYPH + TRACE_GAP } else { 0. };
        let content = match &step.opens {
            Opens::Nothing => None,
            Opens::Text(text) => Some(
                div()
                    .pl(px(under))
                    .pt(px(OPEN_TOP))
                    .pb(px(OPEN_BELOW))
                    .text_color(palette.gray(SECONDARY))
                    .child(text.clone()),
            ),
            Opens::Thought(text) => Some(
                div()
                    .pl(px(under))
                    .pt(px(OPEN_TOP))
                    .pb(px(OPEN_BELOW))
                    .child(assistant_words(palette, text.clone())),
            ),
            Opens::Picture(picture) => Some(
                div()
                    .pl(px(under))
                    .pt(px(OPEN_TOP))
                    .pb(px(OPEN_BELOW))
                    .child(
                        img(picture.clone())
                            .w(px(TRACE_PICTURE_WIDTH))
                            .h(px(TRACE_PICTURE_HEIGHT))
                            .object_fit(ObjectFit::Cover)
                            .rounded(px(TRACE_PICTURE_RADIUS)),
                    ),
            ),
            Opens::Steps(steps) => Some(
                nest().children(
                    steps
                        .iter()
                        .flat_map(|step| self.step(step, false, look, cx)),
                ),
            ),
        };
        let opened = content.and_then(|content| self.unfolding(fold, open, content.w_full(), look));
        // The line under its title, as the task made it (the site's notes its answer carried), in
        // the grey of a line's own detail, quieter than its title.
        let sub = (!step.under.is_empty()).then(|| {
            div()
                .pl(px(under))
                .text_color(palette.gray(TRACE_NOTE))
                .child(step.under.clone())
                .into_any_element()
        });
        let says = self.says(step, under, look);
        std::iter::once(line)
            .chain(sub)
            .chain(says)
            .chain(opened)
            .collect()
    }

    /// The assistant's own word on why it took `step`, one line under it from `under`, as the
    /// assistant's, and heard as theirs. None for a step with none.
    fn says(&self, step: &TraceStep, under: f32, look: &Look) -> Option<gpui::AnyElement> {
        (!step.says.is_empty()).then(|| {
            let name =
                (self.shown.as_ref()).map_or(SharedString::default(), |shown| shown.name.clone());
            div()
                .id(("conversation-says", step.key))
                .when(look.a11y, |says| {
                    says.role(Role::Label)
                        .aria_label(words::says(&name, &step.says))
                })
                .pl(px(under))
                .pt(px(SAYS_TOP))
                .child(
                    assistant_words(look.palette, step.says.clone())
                        .whitespace_nowrap()
                        .truncate(),
                )
                .into_any_element()
        })
    }

    /// What was asked, with its page, and the lines after it, then the answers to the question it
    /// waits on, or Open under its result. What came while it showed rises in; what the user sent
    /// hides while it lands on its plate; the answers of a question answered close where they were.
    fn lines(&self, look: &Look, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let Some(shown) = self.shown.clone() else {
            return Vec::new();
        };
        let palette = look.palette;
        let page = (shown.page.as_ref()).map(|page| {
            let title = if page.title.is_empty() {
                page.url.clone()
            } else {
                page.title.clone()
            };
            (title, look.asked_favicon.clone())
        });
        // A session was asked in its agent's terminal, so it has no ask. A thread started with
        // tabs of the user's and no words has those as its first message.
        let mut lines = Vec::new();
        if !shown.asked.is_empty() || shown.chips.iter().any(|(which, _)| *which == 0) {
            lines.push(self.you(
                "conversation-asked".into(),
                shown.asked.clone(),
                page,
                0,
                false,
                look,
            ));
        }
        let landing = (self.motion.landing.as_ref())
            .and_then(|landing| landing.line)
            .filter(|(_, since)| !look.still && eased(*since, look.now, LAND) < 1.)
            .map(|(index, _)| index);
        let answered =
            (self.motion.answered.as_ref()).filter(|_| !look.still && shown.answers.is_empty());
        // A button no longer drawn starts its wait again if it comes back.
        let beside = self.beside(cx);
        let buttons: Vec<(usize, IloButton)> = (shown.lines.iter())
            .filter_map(|line| match line {
                Line::Ilo(line) => Some(
                    ilo::drawn(line, beside)
                        .into_iter()
                        .map(|each| (line.key, each)),
                ),
                _ => None,
            })
            .flatten()
            .collect();
        self.ilo_drawn
            .borrow_mut()
            .retain(|key, _| buttons.contains(key));
        (self.ilo_words_drawn.borrow_mut()).retain(|key, _| {
            (shown.lines.iter()).any(|line| matches!(line, Line::Ilo(line) if line.key == *key))
        });
        (self.ilo_links.borrow_mut()).retain(|key, _| {
            (shown.lines.iter()).any(|line| matches!(line, Line::Ilo(line) if line.key == *key))
        });
        if !buttons.iter().any(|(_, button)| button.widens()) {
            self.ilo_hovered.set(false);
        }
        // Which of the user's messages a line is, after the one that started the thread.
        let mut said = 0;
        for (index, line) in shown.lines.iter().enumerate() {
            // The line in the reply field's place is drawn there.
            if matches!(line, Line::Ilo(line) if ilo::in_field(&line.kind)) {
                continue;
            }
            if let Some(answered) = answered.filter(|answered| answered.at == index) {
                lines.push(self.answered(answered, look));
            }
            let line = match line {
                Line::You(text) => {
                    let id = ("conversation-you", index).into();
                    said += 1;
                    self.you(id, text.clone(), None, said, landing == Some(index), look)
                }
                Line::Said(text) => div()
                    .id(("conversation-said", index))
                    .when(look.a11y, |said| {
                        said.role(Role::Label).aria_label(plain(text))
                    })
                    .mb(px(BLOCK_GAP))
                    .text_color(said_ink(&palette))
                    .child(self.said("said", index, text, self.motion.runs(index), look))
                    .children(self.answer_foot(index, &shown.finished, look, cx))
                    .into_any_element(),
                Line::Trace(trace) => self.trace(trace, look, cx),
                Line::Thinking { key, failed } => self.thinking(*key, failed.clone(), look),
                Line::Step(step) => div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .children(self.step(step, true, look, cx))
                    .into_any_element(),
                Line::Ilo(line) => self.ilo(line, &shown.name, look, cx),
            };
            lines.push(risen(line, self.motion.came(Key::Line(index)), look));
        }
        if let Some(answered) = answered.filter(|answered| answered.at >= shown.lines.len()) {
            lines.push(self.answered(answered, look));
        }
        let answers = shown.answers.iter().enumerate().map(|(index, answer)| {
            let drawn = self.answers_drawn.clone();
            let chip = page_chip(
                palette,
                ("conversation-answer", index),
                answer.clone(),
                None,
            )
            .relative()
            .child(
                canvas(
                    move |bounds, _, _| _ = drawn.borrow_mut().insert(index, bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_click(cx.listener(move |view, _, _, cx| view.pick(index, cx)));
            lifted(chip, self.motion.came(Key::Answer(index)), CHIPS, look)
        });
        let open = shown.open.as_ref().map(|label| {
            let chip = page_chip(palette, "conversation-open", label.clone(), None)
                .on_click(cx.listener(|view, _, _, cx| view.act(TaskAction::Open, cx)));
            lifted(chip, self.motion.came(Key::Open), CHIPS, look)
        });
        let chips: Vec<_> = answers.chain(open).collect();
        if !chips.is_empty() {
            lines.push(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(CHIP_GAP))
                    .mt(px(CHIPS_TOP - BLOCK_GAP))
                    .mb(px(CHIPS_BELOW))
                    .children(chips)
                    .into_any_element(),
            );
        }
        lines
    }

    /// Under an answer: the button that copies it, and the time it finished. None under the
    /// assistant's other words.
    fn answer_foot(
        &self,
        index: usize,
        finished: &[(usize, SharedString)],
        look: &Look,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let (_, time) = finished.iter().find(|(at, _)| *at == index)?;
        let quiet = answer_text_3(&look.palette);
        let copy = div()
            .id(("conversation-copy", index))
            .when(look.a11y, |copy| {
                copy.role(Role::Button).aria_label(words::COPY_ANSWER)
            })
            .flex_none()
            .cursor_pointer()
            .child(
                svg()
                    .path("icons/copy.svg")
                    .size(px(COPY_GLYPH))
                    .text_color(quiet),
            )
            .on_click(cx.listener(move |view, _, _, cx| view.copy_answer(index, cx)));
        Some(
            div()
                .mt(px(ANSWER_FOOT_TOP))
                .h(px(ANSWER_FOOT))
                .flex()
                .items_center()
                .gap(px(ANSWER_FOOT_GAP))
                .child(copy)
                .child(
                    div()
                        .text_size(px(ANSWER_TIME))
                        .line_height(px(ANSWER_TIME_LINE))
                        .text_color(quiet)
                        .child(time.clone()),
                ),
        )
    }

    /// Copies the answer at `index` as plain text, its marks taken out.
    pub(super) fn copy_answer(&self, index: usize, cx: &mut Context<Self>) {
        let answer = (self.shown.as_ref()).and_then(|shown| match shown.lines.get(index) {
            Some(Line::Said(text)) => Some(plain(text)),
            _ => None,
        });
        if let Some(answer) = answer {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(answer));
        }
    }

    /// A question's answers as it's answered: the one picked has gone on as the reply, and the
    /// others fade as their row closes.
    fn answered(&self, answered: &Answered, look: &Look) -> gpui::AnyElement {
        let left = 1. - eased(answered.since, look.now, LAND);
        let fade = 1. - linear(answered.since, look.now, PICK_FADE);
        let chips = answered.answers.iter().enumerate().map(|(index, answer)| {
            let opacity = if answered.picked == Some(index) {
                0.
            } else {
                fade
            };
            page_chip(
                look.palette,
                ("conversation-answered", index),
                answer.clone(),
                None,
            )
            .opacity(opacity)
        });
        div()
            .flex_none()
            .mt(px((CHIPS_TOP - BLOCK_GAP) * left))
            .h(px((CHIP_HEIGHT + CHIPS_BELOW) * left))
            .overflow_hidden()
            .child(div().flex().flex_wrap().gap(px(CHIP_GAP)).children(chips))
            .into_any_element()
    }

    /// What the user sent, on its way from where they sent it to its plate: the plate of the field
    /// or the chip it was, becoming the message's as it goes.
    fn ghost(&self, look: &Look) -> Option<gpui::AnyElement> {
        let landing = self.motion.landing.as_ref()?;
        let (_, since) = landing.line?;
        let (to, view) = (self.landed.get()?, self.drawn_at.get()?);
        let done = eased(since, look.now, LAND);
        if look.still || done >= 1. {
            return None;
        }
        let between = |from: f32, to: f32| from + (to - from) * done;
        let (fill, radius, height, inset) = match landing.chip {
            true => (
                look.palette.state(CHIP_FILL, STATE_PRESSED).a,
                CHIP_RADIUS,
                CHIP_HEIGHT,
                CHIP_X,
            ),
            false => (
                REPLY_FILL,
                COMPOSER_RADIUS,
                COMPOSER_FIELD,
                COMPOSER_FIELD_X,
            ),
        };
        let from = landing.from;
        let at = |from: Pixels, to: Pixels| between(f32::from(from), f32::from(to));
        Some(
            div()
                .absolute()
                .left(px(at(from.origin.x, to.origin.x) - f32::from(view.origin.x)))
                .top(px(at(from.origin.y, to.origin.y) - f32::from(view.origin.y)))
                .w(px(at(from.size.width, to.size.width)))
                .h(px(at(from.size.height, to.size.height)))
                .overflow_hidden()
                .rounded(px(between(radius, RADIUS)))
                .bg(mixed(
                    look.palette.plate(fill),
                    bubble_fill(&look.palette),
                    done,
                ))
                .pt(px(between((height - TEXT_LINE) / 2., BUBBLE_TOP)))
                .px(px(between(inset, BUBBLE_X)))
                .text_size(px(TEXT))
                .line_height(px(TEXT_LINE))
                .text_color(look.palette.text(TITLE))
                .child(landing.text.clone())
                .into_any_element(),
        )
    }

    /// The reply field, the TextInput, so IME and the field's keys are a text field's. A session's
    /// conversation is in its agent's terminal, which a quiet line in its place says, on the text's
    /// column as it has no plate.
    /// With Settings › Threads' switch off, the line says so in the field's place, as the thread
    /// takes no more.
    fn reply(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let tasks_off = crate::agents::global(cx).is_some_and(|agents| !agents.read(cx).threads());
        let name = (self.shown.as_ref()).map(|shown| shown.name.clone());
        let quiet = words::in_reply_place(self.replies, tasks_off, &name.unwrap_or_default());
        if let Some(quiet) = quiet {
            return div()
                .id("conversation-elsewhere")
                .flex_none()
                .h(px(REPLY_HEIGHT))
                .flex()
                .items_center()
                .text_color(look.palette.gray(SECONDARY))
                .child(quiet);
        }
        // An ended task that takes no reply: ilo's line says why, with Start Again (#4105).
        if let Some(line) = self.in_field() {
            let name = (self.shown.as_ref())
                .map(|shown| shown.name.clone())
                .unwrap_or_default();
            return div()
                .id("conversation-ended")
                .flex_none()
                .child(self.ilo(&line, &name, look, cx));
        }
        let reply = self.reply.clone();
        let drawn = self.reply_drawn.clone();
        let palette = look.palette;
        let form = Composer::of(look.standing);
        // The field, growing with its words, then the row of choices, in one box.
        let field = div()
            .id("conversation-field")
            .relative()
            .min_h(px(form.field))
            .px(px(form.field_x))
            .flex()
            .items_center()
            .text_size(px(form.text))
            .line_height(px(form.line))
            .font_weight(FontWeight::NORMAL)
            .on_click(cx.listener(move |_, _, window, cx| {
                window.focus(&reply.focus_handle(cx), cx);
            }))
            .child(div().flex_1().min_w_0().child(self.reply.clone()))
            .child(
                canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            );
        div()
            .id("conversation-reply")
            .flex_none()
            .flex()
            .flex_col()
            .rounded(px(form.radius))
            .bg(palette.plate(REPLY_FILL))
            .border_1()
            .border_color(palette.plate(COMPOSER_RIM))
            .children(self.picked_chips(look, cx))
            .child(field)
            .child(self.choices(&form, look, cx))
    }

    /// The composer's row under its field, the bar's own (choices.rs): the plus, disabled where a
    /// reply has nothing to add, the thread's level, which opens its menu (#4094), the folder it
    /// holds (#4101), its model, which opens its assistant's models where its program takes
    /// another for the next message and is plain words elsewhere, and the round button: Stop while
    /// it works and nothing is typed, else Send, which does nothing with nothing typed.
    fn choices(&self, form: &Composer, look: &Look, cx: &mut Context<Self>) -> gpui::Div {
        let shown = self.shown.as_ref();
        let tier = shown.map(|shown| shown.tier).unwrap_or_default();
        // A chip alone sends, as words do.
        let typed = !self.reply.read(cx).text().trim().is_empty() || self.picked();
        let stoppable = shown.is_some_and(|shown| shown.stoppable);
        let round = match (stoppable, typed) {
            (true, false) => choices::Round::Stop,
            _ => choices::Round::Go { ready: typed },
        };
        let (said, go): (
            SharedString,
            Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App)>,
        ) = match round {
            choices::Round::Stop => (
                words::STOP.into(),
                Box::new(cx.listener(|view, _, _, cx| {
                    view.shell.update(cx, |shell, cx| shell.stop_talk(cx)).ok();
                })),
            ),
            choices::Round::Go { .. } => (
                words::REPLY.into(),
                Box::new(cx.listener(|view, _, window, cx| view.send_reply(window, cx))),
            ),
        };
        let stop = (round == choices::Round::Stop).then(|| stop_told(self.stop_logged.clone()));
        let drawn = self.round_drawn.clone();
        let round_at: Box<dyn Fn(Bounds<Pixels>)> = Box::new(move |bounds| {
            drawn.set(Some(bounds));
            if let Some(stop) = &stop {
                stop(bounds);
            }
        });
        let level = choices::Trigger {
            id: LEVEL,
            at: self.level_drawn.clone(),
            click: Some(Box::new(
                cx.listener(|view, _, window, cx| view.level_menu(window, cx)),
            )),
        };
        // The model opens its menu where the thread's program takes another for its next message;
        // elsewhere it's plain words.
        let model =
            (shown.and_then(|shown| shown.model.clone())).map(|(mark, said, word, opens)| {
                let click: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App)>> =
                    opens.then(|| {
                        Box::new(cx.listener(|view, _, window, cx| view.model_menu(window, cx)))
                            as _
                    });
                let trigger = choices::Trigger {
                    id: MODEL,
                    at: self.model_drawn.clone(),
                    click,
                };
                // The panel speaks a default by its own word, as it draws it; a named model whole.
                let said = match form.row.model_word && word == crate::command_bar::DEFAULT_ROW {
                    true => word.clone(),
                    false => said,
                };
                (Some(mark), said, word, trigger)
            });
        // Where the model's own word gives way, never under its first word's width.
        let model_least = match (&model, form.row.model_word) {
            (Some((_, _, word, _)), true) => {
                let first = word.split_whitespace().next().unwrap_or_default();
                let system = gpui::WindowTextSystem::new(cx.text_system().clone());
                let first = SharedString::from(first.to_string());
                crate::tooltip::text_width(&first, form.row.text, gpui::FontWeight::NORMAL, &system)
                    .ceil()
            }
            _ => 0.,
        };
        let inks = self.inks(look);
        let plus = choices::plus_look(
            inks.dim,
            inks.plate,
            crate::tokens::command_bar::PLUS,
            crate::tokens::command_bar::CHOICES_MENU_DROP,
            look.a11y,
        );
        let plus = (self.plus).draw(
            "conversation-plus",
            self.reply_rows(cx),
            &plus,
            cx.entity_id(),
        );
        choices::row(choices::Row {
            form: form.row,
            inks,
            a11y: look.a11y,
            plus: Some(plus),
            added: Vec::new(),
            level: (tier, level),
            model,
            model_least,
            effort: None,
            held: self
                .folder_chip(look, cx)
                .map(|chip| chip.into_any_element())
                .into_iter()
                .collect(),
            open: self.menu_open,
            round: ("conversation-round", said, round, go),
            round_at: Some(round_at),
        })
    }

    /// The plus's rows under the reply field: the user's tabs, where the window lists any. A
    /// reply never carries files or a folder, which a task is given at its start only, so with
    /// no tab to list the plus is disabled.
    fn reply_rows(&self, cx: &mut Context<Self>) -> Vec<crate::plus::Row> {
        self.tabs_row(cx).into_iter().collect()
    }

    /// The folder the task holds, by its name, whose menu has its one act, Revoke: the task goes
    /// on without it at once.
    fn folder_chip(
        &self,
        look: &Look,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let palette = look.palette;
        let name = self.shown.as_ref()?.folder.clone()?;
        let drawn = self.folder_drawn.clone();
        let chip = chip_ground(palette, "conversation-folder")
            .ml(px(CHIP_GAP))
            .gap(px(PAGE_CHIP_GAP))
            .child(
                svg()
                    .flex_none()
                    .path("icons/folder.svg")
                    .size(px(PAGE_CHIP_FAVICON))
                    .text_color(palette.text(TITLE)),
            )
            .child(clip(&name, QUOTED_CHARS as usize))
            .relative()
            .child(
                canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
            .on_click(cx.listener(|view, _, window, cx| {
                let at = window.mouse_position();
                let agent = view.agent.clone();
                let revoke: MenuAction = Box::new(move |shell, cx| {
                    shell.browser.task(agent, TaskAction::RevokeFolder, cx)
                });
                let items = vec![(MenuEntry::new(files_words::REVOKE, true), revoke)];
                (view.shell)
                    .update(cx, |shell, cx| shell.show_menu(items, at, None, cx))
                    .ok();
            }));
        Some(chip)
    }

    /// One of ilo's own lines: its glyph and mark, its words with the name it took from a page
    /// apart from them, what it says more under them, and its buttons. Its words and buttons come
    /// from its kind (words.rs, `ilo::buttons`), never from the agent's or a page's text.
    fn ilo(
        &self,
        line: &IloLine,
        agent: &SharedString,
        look: &Look,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let palette = look.palette;
        let ink = palette.text(ILO_INK);
        // ilo's mark is the first word of what it says, in the display face, so its words wrap
        // under it: an ilo line has one left edge, its mark's.
        let mark = 0..words::MARK.len();
        let marked = HighlightStyle {
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        };
        let IloHead {
            said,
            name,
            quoted,
            count,
        } = ilo_head(&line.kind, agent);
        // A page's words or a site are in the title's ink, so they read as a name.
        let named = quoted.map(|quoted| {
            let style = HighlightStyle {
                color: Some(palette.text(TITLE)),
                ..Default::default()
            };
            (quoted, style)
        });
        let highlights: Vec<_> = [(mark.clone(), marked)].into_iter().chain(named).collect();
        // Every one of ilo's lines wears ilo's mark, whatever it says (the mark rule).
        let glyph = crate::product::mark_path(ILO_GLYPH);
        let head = trace_line(
            look,
            TraceLine {
                glyph: Some(glyph),
                mark: Some((mark, super::display_face::family(cx))),
                highlights,
                words_drawn: Some((line.key, self.ilo_words_drawn.clone())),
                ..TraceLine::new(
                    LineKey::Ilo(line.key),
                    said.into(),
                    count.unwrap_or_default().into(),
                )
            },
        );
        // What it says more starts under its words.
        let under = px(ILO_GLYPH + TRACE_GAP);
        let buttons = ilo::drawn(line, self.beside(cx));
        // A held step's line (one with buttons) sets what's under it as one column, 6 down: the
        // reason in text-2, the buttons, then the quiet line (spec-thread-stage.md).
        let held = !buttons.is_empty();
        let body = words::body(&line.kind, name.as_deref().unwrap_or("")).map(|body| match held {
            true => div()
                .text_size(px(HELD_REASON))
                .line_height(px(HELD_REASON_LINE))
                .text_color(palette.gray(TRACE_NOTE))
                .child(body),
            false => div()
                .pl(under)
                .mt(px(TEXT_TOP))
                .line_height(px(ILO_TEXT_LINE))
                .text_color(ink)
                .child(body),
        });
        let link = words::link(&line.kind).map(|words| {
            let drawn = self.ilo_links.clone();
            let key = line.key;
            div()
                .id(SharedString::from(format!("ilo-{key}-link")))
                .when(look.a11y, |link| link.role(Role::Link).aria_label(words))
                .relative()
                .when(!held, |link| link.pl(under).mt(px(NOTE_TOP)))
                .text_size(px(text::SMALL))
                .text_color(palette.gray(NOTE_INK))
                .cursor_pointer()
                .child(words)
                .child(
                    canvas(
                        move |bounds, _, _| _ = drawn.borrow_mut().insert(key, bounds),
                        |_, _, _, _| {},
                    )
                    // Pinned to the corner: unpinned, it would stand after the words, a line below.
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
                .on_click(cx.listener(|view, _, _, cx| view.open_off_limits(cx)))
        });
        let note = words::note(&buttons).map(|note| {
            div()
                .id(("ilo-note", line.key))
                .when(look.a11y, |under| under.role(Role::Label).aria_label(note))
                .text_size(px(HELD_NOTE))
                .line_height(px(HELD_NOTE_LINE))
                .text_color(answer_text_3(&palette))
                .child(note)
        });
        let row = held.then(|| {
            div().flex().flex_wrap().gap(px(HELD_BUTTONS_GAP)).children(
                (buttons.into_iter().enumerate())
                    .map(|(at, button)| self.ilo_button(line, (at, button), look, cx)),
            )
        });
        let under_line = match held {
            true => vec![
                div()
                    .pl(under)
                    .mt(px(HELD_TOP))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(HELD_GAP))
                    .children(body)
                    .children(link.map(|link| link.into_any_element()))
                    .children(row)
                    .children(note)
                    .into_any_element(),
            ],
            false => body
                .map(|body| body.into_any_element())
                .into_iter()
                .chain(link.map(|link| link.into_any_element()))
                .collect(),
        };
        div()
            .id(("ilo-line", line.key))
            // The line is a group by its own words as drawn, and its buttons are in it.
            .when(look.a11y, |group| {
                group.role(Role::Group).aria_label(ilo_spoken(line, agent))
            })
            .flex()
            .flex_col()
            .items_start()
            .mb(px(ILO_BELOW))
            .child(head)
            .children(under_line)
            .into_any_element()
    }

    /// An ilo line's button, which notes where it stands and since when.
    fn ilo_button(
        &self,
        line: &IloLine,
        (at, button): (usize, IloButton),
        look: &Look,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let key = (line.key, button);
        // The Focus command's keyboard, on this line's buttons (ilo_keys.rs).
        let keyed = self
            .ilo_keys
            .as_ref()
            .is_some_and(|keys| keys.line == line.key);
        let ringed = self.ilo_ring(line.key) == Some(at);
        let stop = (keyed && at < ilo_keys::MOST).then(|| self.ilo_stops[at].clone());
        let tab = ilo::tab(&line.kind);
        let drawn = self.ilo_drawn.clone();
        let hovered = self.ilo_hovered.clone();
        let id = SharedString::from(format!("ilo-{}-{button:?}", line.key));
        held_button(look.palette, id, words::button(button), at == 0)
            .when(look.a11y, |chip| {
                chip.role(Role::Button).aria_label(words::button(button))
            })
            .relative()
            .child(
                canvas(
                    move |bounds, _, _| {
                        let mut drawn = drawn.borrow_mut();
                        if drawn.get(&key).is_none_or(|(was, _)| *was != bounds) {
                            drawn.insert(key, (bounds, Instant::now()));
                            log!("{}", ilo_keys::place(key, bounds));
                        }
                    },
                    |_, _, _, _| {},
                )
                // At the chip's own corner: with no place given it would start inside the
                // chip's padding.
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .when(button.widens(), |chip| {
                chip.on_hover(move |over, _, _| hovered.set(*over))
            })
            .when_some(stop, |chip, stop| {
                chip.track_focus(&stop).key_context(ilo_keys::CONTEXT)
            })
            .when(ringed, |mut chip| {
                let style = chip.style();
                *style = crate::keyboard_ring::ring(look.palette.ink)(style.clone());
                chip
            })
            .on_click(cx.listener(move |view, _, _, cx| view.press_ilo(key, tab, cx)))
    }

    /// The link under a fence's refusal: Settings, on its Off limits list. It only opens it, so
    /// nothing it does lifts a fence or raises a task's access.
    fn open_off_limits(&mut self, cx: &mut Context<Self>) {
        self.shell
            .update(cx, |shell, cx| shell.browser.off_limits(cx))
            .ok();
    }

    /// A press on an ilo line's button. One that widens what the agent may do counts only once it
    /// has stood where it is a moment: an agent decides when its line comes, so a press meant for
    /// what was there before mustn't land on it. No key presses one: the keyboard reaches an ilo
    /// line only through the user's Focus command (#3955).
    fn press_ilo(&mut self, key: (usize, IloButton), tab: Option<u64>, cx: &mut Context<Self>) {
        let (line, button) = key;
        let stood = self.ilo_drawn.borrow().get(&key).map(|(_, since)| *since);
        if button.widens() && !stood.is_some_and(|since| armed(since, Instant::now())) {
            // check.sh reads this line.
            log!("browser: task {} {button:?} too soon", self.agent);
            return;
        }
        let action = match button {
            IloButton::TakeOver => tab.map(TaskAction::TakeOver),
            IloButton::Raise(tier) => Some(TaskAction::SetTier(tier)),
            IloButton::HandBack => tab.map(TaskAction::HandBack),
            IloButton::PagesBack => Some(TaskAction::PagesBack),
            // It only shows the page, as its picture's click does.
            IloButton::ShowPage => {
                if let Some(tab) = tab {
                    self.watch(tab, cx);
                }
                None
            }
            IloButton::NotNow => Some(TaskAction::NotNow(line)),
            IloButton::StartAgain => {
                self.start_again(cx);
                None
            }
        };
        if let Some(action) = action {
            self.act(action, cx);
        }
    }

    /// The lines follow what came at their end on the curve it lands on, until the user scrolls.
    fn follow_on(&mut self, now: Instant) {
        // What comes never moves a button that widens what the agent may do from under the
        // pointer resting on it.
        if self.ilo_hovered.get() {
            self.motion.follow = None;
            return;
        }
        let Some(follow) = self.motion.follow.as_mut() else {
            return;
        };
        let offset = self.scroll.offset();
        let to = -f32::from(self.scroll.max_offset().y);
        let done = eased(follow.since, now, LAND);
        if f32::from(offset.y) > follow.set + 1. {
            self.motion.follow = None;
        } else if done >= 1. {
            self.scroll.scroll_to_bottom();
            self.motion.follow = None;
        } else {
            let y = follow.from + (to - follow.from) * done;
            self.scroll.set_offset(point(offset.x, px(y)));
            follow.set = y;
        }
    }
}

/// Whether a button that widens what the agent may do, standing where it is `since`, takes a press
/// `now`.
fn armed(since: Instant, now: Instant) -> bool {
    now.saturating_duration_since(since).as_secs_f32() >= ARM
}

/// A line that came while the conversation showed, rising into its place.
fn risen(line: gpui::AnyElement, since: Option<Instant>, look: &Look) -> gpui::AnyElement {
    if since.is_none() {
        return line;
    }
    lifted(div().max_w_full().child(line), since, RISE, look).into_any_element()
}

/// `element` rising the chips' distance into its place over `length` from `since`.
fn lifted<E: Styled>(element: E, since: Option<Instant>, length: f32, look: &Look) -> E {
    let Some(since) = since else { return element };
    let (opacity, by) = motion::rise(Some(since), look.now, length, CHIPS_RISE, look.still);
    element.relative().top(px(by)).opacity(opacity)
}

/// Words rolling in over `old`, which go up, as they changed: as tall as `line`, and as wide as
/// the new, the old going out past it if they're longer.
fn rolled(
    new: SharedString,
    old: Option<&(SharedString, Instant)>,
    line: f32,
    look: &Look,
) -> gpui::AnyElement {
    let Some((old, since)) = old else {
        return div().flex_none().child(new).into_any_element();
    };
    let [(old_opacity, old_by), (new_opacity, new_by)] = motion::roll(*since, look.now, look.still);
    div()
        .flex_none()
        .relative()
        .overflow_y_hidden()
        .h(px(line))
        .child(
            div()
                .relative()
                .top(px(new_by))
                .opacity(new_opacity)
                .whitespace_nowrap()
                .child(new),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(old_by))
                .opacity(old_opacity)
                .whitespace_nowrap()
                .child(old.clone()),
        )
        .into_any_element()
}

/// A thread's `primary` and what's on it: the off-white with near-black words, in Light the
/// reverse; never the Space's colour, as no control is green.
fn primary(palette: super::Palette) -> (Hsla, Hsla) {
    match palette.tone == Tone::Dark {
        true => (rgba(HELD_PRIMARY).into(), rgba(HELD_ON_PRIMARY).into()),
        false => (
            rgba(LIGHT_HELD_PRIMARY).into(),
            rgba(LIGHT_HELD_ON_PRIMARY).into(),
        ),
    }
}

/// What a trace, or a turn's same step on several tabs, opens to: its lines, a step in, along a
/// rule under the glyph above them.
/// The user's message's plate: the drawing's dark plate in Dark, the ink at a card's strength in
/// Light (light-values.md, Proposed).
/// A held step's button: the first on `primary` with `on-primary` words at semibold, the second
/// on `button` at medium.
/// The way back on a tab the user handed the thread, `id`: the tab-return glyph in a box with
/// the stage's `button` plate under the pointer, named and tipped Z34. A click is `back`'s, and
/// never the tile's or the row's under it.
fn way_back(
    palette: super::Palette,
    a11y: bool,
    id: impl Into<ElementId>,
    back: impl Fn(&mut gpui::App) + 'static,
    tip: impl Fn(Option<Bounds<Pixels>>, &mut gpui::App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let plate: Hsla = rgba(palette.toned(HELD_BUTTON_FILL, LIGHT_HELD_BUTTON_FILL)).into();
    let tip = Rc::new(tip);
    let drawn: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
    let at = drawn.clone();
    div()
        .id(id)
        .relative()
        .flex_none()
        .size(px(panel::WAY_BACK))
        .rounded(px(panel::WAY_BACK_RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |style| style.bg(plate))
        .when(a11y, |button| {
            button.role(Role::Button).aria_label(words::RETURN_TAB)
        })
        .child(
            canvas(move |bounds, _, _| at.set(Some(bounds)), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .child(
            svg()
                .path(WAY_BACK_GLYPH)
                .size(px(panel::WAY_BACK_GLYPH))
                .text_color(palette.text(panel::WAY_BACK_INK)),
        )
        .on_hover({
            let tip = tip.clone();
            move |hovered, _, cx| tip(drawn.get().filter(|_| *hovered), cx)
        })
        // The tip goes with the press, as the button goes with the tab it returns.
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            tip(None, cx);
        })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            back(cx);
        })
}

/// The way back's glyph (spec-held-abilities.md, part 1).
const WAY_BACK_GLYPH: &str = "icons/tab-return.svg";

fn held_button(
    palette: super::Palette,
    id: impl Into<ElementId>,
    words: impl Into<SharedString>,
    first: bool,
) -> gpui::Stateful<gpui::Div> {
    let dark = palette.tone == Tone::Dark;
    let pick = |dark_value: u32, light_value: u32| -> Hsla {
        rgba(if dark { dark_value } else { light_value }).into()
    };
    let (fill, ink, weight) = match first {
        true => (primary(palette).0, primary(palette).1, FontWeight::SEMIBOLD),
        false => (
            pick(HELD_BUTTON_FILL, LIGHT_HELD_BUTTON_FILL),
            palette.text(TITLE),
            FontWeight::MEDIUM,
        ),
    };
    div()
        .id(id)
        .flex_none()
        .h(px(HELD_BUTTON))
        .px(px(HELD_BUTTON_X))
        .flex()
        .items_center()
        .rounded(px(HELD_BUTTON_RADIUS))
        .bg(fill)
        .hover(|style| style.opacity(HELD_BUTTON_HOVER))
        .cursor_pointer()
        .whitespace_nowrap()
        .text_size(px(HELD_BUTTON_TEXT))
        .line_height(px(HELD_BUTTON_TEXT_LINE))
        .font_weight(weight)
        .text_color(ink)
        .child(words.into())
}

/// The composer's triggers, by their elements' ids.
const LEVEL: &str = "conversation-level";
const MODEL: &str = "conversation-model";

/// Where the page row's Open for `tab` was drawn, kept in `at`, which it opens from, and logged as
/// it moves, for check.sh to click it.
fn open_told(tab: u64, at: Rc<Cell<Option<Bounds<Pixels>>>>) -> impl Fn(Bounds<Pixels>) + 'static {
    move |bounds| {
        if at.replace(Some(bounds)) != Some(bounds) {
            log!(
                "browser: page row open {tab} at {:.0} {:.0} {:.0} {:.0}",
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height)
            );
        }
    }
}

/// Where Stop was drawn, kept in `logged` and logged as it moves, for check.sh to click it.
fn stop_told(logged: Rc<Cell<Option<Bounds<Pixels>>>>) -> impl Fn(Bounds<Pixels>) + 'static {
    move |bounds| {
        if logged.replace(Some(bounds)) != Some(bounds) {
            // check.sh reads this line, to click it.
            log!(
                "browser: task stop at {:.0} {:.0} {:.0} {:.0}",
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height)
            );
        }
    }
}

/// The sizes the composer is drawn at: the stage's, or the panel's beside a page.
struct Composer {
    radius: f32,
    field: f32,
    field_x: f32,
    text: f32,
    line: f32,
    row: choices::Form,
}

impl Composer {
    fn of(standing: bool) -> Self {
        let stage = choices::Form {
            height: COMPOSER_ROW,
            under: COMPOSER_ROW_UNDER,
            left: COMPOSER_ROW_X,
            right: COMPOSER_ROW_X,
            gap: COMPOSER_ROW_GAP,
            round: COMPOSER_ROUND,
            arrow: COMPOSER_ARROW,
            square: COMPOSER_SQUARE,
            square_radius: COMPOSER_SQUARE_RADIUS,
            ..choices::Form::BAR
        };
        match standing {
            false => Composer {
                radius: COMPOSER_RADIUS,
                field: COMPOSER_FIELD,
                field_x: COMPOSER_FIELD_X,
                text: COMPOSER_TEXT,
                line: COMPOSER_LINE,
                row: stage,
            },
            true => Composer {
                radius: panel::COMPOSER_RADIUS,
                field: panel::COMPOSER_FIELD,
                field_x: panel::COMPOSER_FIELD_X,
                text: panel::COMPOSER_TEXT,
                line: panel::COMPOSER_LINE,
                row: choices::Form {
                    height: panel::COMPOSER_ROW,
                    left: panel::COMPOSER_ROW_X,
                    right: panel::COMPOSER_ROW_X,
                    level_x: panel::COMPOSER_LEVEL_X,
                    text: panel::COMPOSER_LEVEL,
                    line: panel::COMPOSER_LEVEL_LINE,
                    mark: false,
                    model_dim: true,
                    round: panel::COMPOSER_ROUND,
                    arrow: panel::COMPOSER_ARROW,
                    square: panel::COMPOSER_SQUARE,
                    round_space: panel::COMPOSER_ROUND_SPACE,
                    model_word: true,
                    ..stage
                },
            },
        }
    }
}

/// What the assistant itself says in a trace, an opened thought or its note on a step: text-2
/// behind a thin rule, the one form no line of the product's has.
fn assistant_words(palette: super::Palette, text: SharedString) -> gpui::Div {
    div()
        .border_l(px(SAID_RULE))
        .border_color(palette.text(SAID_INK))
        .pl(px(SAID_GAP))
        .text_color(palette.gray(TRACE_NOTE))
        .child(text)
}

fn answer_text_3(palette: &super::Palette) -> Hsla {
    match palette.tone {
        Tone::Dark => rgba(TEXT_3).into(),
        Tone::Light => rgba(LIGHT_TEXT_3).into(),
    }
}

/// The user's message's measures: the stage's, or reduced in the panel beside a page, where it
/// wraps inside the narrower column.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bubble {
    most: f32,
    top: f32,
    bottom: f32,
    x: f32,
    text: f32,
    line: f32,
}

impl Bubble {
    fn of(standing: bool) -> Self {
        match standing {
            true => Bubble {
                most: panel::BUBBLE_MOST,
                top: panel::BUBBLE_Y,
                bottom: panel::BUBBLE_Y,
                x: panel::BUBBLE_X,
                text: panel::BUBBLE_TEXT,
                line: panel::BUBBLE_LINE,
            },
            false => Bubble {
                most: BUBBLE_MOST,
                top: BUBBLE_TOP,
                bottom: BUBBLE_BOTTOM,
                x: BUBBLE_X,
                text: TEXT,
                line: TEXT_LINE,
            },
        }
    }
}

fn bubble_fill(palette: &super::Palette) -> Hsla {
    rgba(match palette.tone {
        Tone::Dark => BUBBLE_FILL,
        Tone::Light => LIGHT_BUBBLE_FILL,
    })
    .into()
}

/// `from` mixed `done` of the way to `to`, channel by channel.
fn mixed(from: Hsla, to: Hsla, done: f32) -> Hsla {
    let (from, to) = (from.to_rgb(), to.to_rgb());
    let at = |a: f32, b: f32| a + (b - a) * done;
    gpui::Rgba {
        r: at(from.r, to.r),
        g: at(from.g, to.g),
        b: at(from.b, to.b),
        a: at(from.a, to.a),
    }
    .into()
}

fn nest() -> gpui::Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(STRETCH_GAP))
}

/// How the lines follow what came at their end: not while the user has scrolled up to read, at
/// once as the conversation opens or with motion reduced, else on the curve what came lands on.
#[derive(Debug, PartialEq)]
enum Follows {
    Not,
    AtOnce,
    Eased,
}

fn follows(grew: bool, opening: bool, pinned: bool, moves: bool) -> Follows {
    match (grew, opening, pinned, moves) {
        (false, ..) => Follows::Not,
        (_, true, ..) => Follows::AtOnce,
        (_, _, false, _) => Follows::Not,
        (_, _, true, false) => Follows::AtOnce,
        (_, _, true, true) => Follows::Eased,
    }
}

/// How much the agent's messages say, which grows as one is written.
fn said(lines: &[Line]) -> usize {
    (lines.iter())
        .map(|line| match line {
            Line::Said(text) => text.len(),
            _ => 0,
        })
        .sum()
}

/// The steps a conversation's traces hold, which change as it works without a line coming.
fn steps(lines: &[Line]) -> usize {
    (lines.iter())
        .map(|line| match line {
            Line::Trace(trace) => trace.items.len(),
            Line::Step(_) => 1,
            _ => 0,
        })
        .sum()
}

/// Where the words of ilo's lines were drawn and how they were laid out, by line.
type WordsDrawn = Rc<RefCell<HashMap<usize, (Bounds<Pixels>, gpui::TextLayout)>>>;

/// The colours a trace's line draws in (`TraceLine::inks`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct LineInks {
    words: Hsla,
    note: Hsla,
    glyph: Hsla,
    /// Its words under the pointer, where the line opens and no shimmer lights them.
    pointed: Option<Hsla>,
}

/// A line of a trace as it draws: its glyph, its title, what it took or why it failed, and how
/// open its chevron is if it opens. The step under way's title `shimmers`, as far through its pass
/// as it says, or under Reduce Motion, holds at the title's colour. What changed on it fades or
/// rolls in by the rest.
struct TraceLine {
    key: LineKey,
    glyph: Option<&'static str>,
    /// A page's favicon, in its glyph's place: a helper's line's (#4102).
    icon: Option<Arc<Image>>,
    /// ilo's mark, the first word of its title, and the display face's family it's set in, which
    /// makes it one of ilo's lines: its glyph and words in ilo's ink and size, its words wrapping
    /// under its mark.
    mark: Option<(std::ops::Range<usize>, SharedString)>,
    /// Its words' parts set apart, as an ilo line's name from a page.
    highlights: Vec<(std::ops::Range<usize>, HighlightStyle)>,
    /// Where its words were drawn goes here, by its line.
    words_drawn: Option<(usize, WordsDrawn)>,
    failed: bool,
    title: SharedString,
    note: SharedString,
    chevron: Option<f32>,
    shimmers: Option<Option<f32>>,
    /// The step under way: the ring in its glyph's place, this far round, its words in the
    /// text's ink.
    ring: Option<f32>,
    /// It joins the line above it with a connector, as a step under its page does.
    joined_above: bool,
    /// A helper's step under way, at its right: the ring after its words, this far round.
    ring_after: Option<f32>,
    /// A stretch's fold head, and whether its stretch is at work: its word then in the text's
    /// ink at medium, else in text-2.
    head: Option<bool>,
    /// A page's line, and whether it's the page the assistant is on: its title then in the
    /// text's ink, else in text-2. A click opens or folds its steps.
    page: Option<bool>,
    /// A done stretch's fold: the icons of the pages it visited, after its words. None for a
    /// page whose tab has none, which shows the page glyph.
    icons: Vec<Option<Arc<Image>>>,
    alpha: f32,
    note_alpha: f32,
    chevron_alpha: f32,
    /// Its count of steps before the last came, and since when, which rolls up out of it.
    rolled: Option<(SharedString, Instant)>,
    /// Its note follows its title as one string, a middle dot between them: a stretch's summary,
    /// "Worked for 40 s · 4 steps", as its task's card writes it.
    joined: bool,
}

impl TraceLine {
    fn new(key: impl Into<LineKey>, title: SharedString, note: SharedString) -> Self {
        TraceLine {
            key: key.into(),
            glyph: None,
            icon: None,
            mark: None,
            highlights: Vec::new(),
            words_drawn: None,
            failed: false,
            title,
            note,
            chevron: None,
            shimmers: None,
            ring: None,
            joined_above: false,
            ring_after: None,
            head: None,
            page: None,
            icons: Vec::new(),
            alpha: 1.,
            note_alpha: 1.,
            chevron_alpha: 1.,
            rolled: None,
            joined: false,
        }
    }

    /// Its colours. Each is the text's ink at one strength or another, and a failed line's are
    /// any other's, a cross for its glyph and why at its end: never red, at rest or under the
    /// pointer.
    fn inks(&self, palette: &super::Palette) -> LineInks {
        let ilos = self.mark.is_some();
        let words = match self.shimmers {
            _ if ilos => palette.text(ILO_INK),
            _ if self.head == Some(true) => palette.text(TITLE),
            _ if self.page == Some(true) => palette.text(TITLE),
            _ if self.page == Some(false) => palette.gray(TRACE_NOTE),
            _ if self.ring.is_some() => palette.text(TITLE),
            Some(None) => palette.text(TITLE),
            // At the shimmer's peak, which each part fades down from.
            Some(Some(_)) => palette.lit(TRACE_TITLE, SHIMMER, 1.),
            None => palette.text(TRACE_TITLE),
        };
        let opens = self.chevron.is_some() || self.page.is_some();
        // A shimmering title is lit by its shimmer, under the pointer too.
        let lit = opens && !matches!(self.shimmers, Some(Some(_)));
        LineInks {
            words,
            note: palette.gray(TRACE_NOTE),
            glyph: match ilos {
                true => palette.text(MARK_INK),
                false => palette.glyph(TRACE_GLYPH_ALPHA),
            },
            pointed: lit.then(|| palette.text(TITLE)),
        }
    }

    /// Whether what came of it sits at the column's right edge, its words filling the line: a
    /// step's line, never a fold's head, a page's line, one of ilo's or a line read as one string.
    fn note_at_right(&self) -> bool {
        self.mark.is_none() && self.head.is_none() && self.page.is_none() && !self.joined
    }

    /// Its note's words as they draw after its title, the count apart from the rest as it rolls
    /// by itself: after a middle dot on a line that's `joined`. Each part starts with its own
    /// space, so the parts in a row read as one string.
    fn note_parts(&self) -> [String; 3] {
        let lead = if self.joined { " \u{b7}" } else { "" };
        let space = if self.joined { " " } else { "" };
        match self.note.split_once(' ') {
            Some((count, rest)) => [lead.into(), format!("{space}{count}"), format!(" {rest}")],
            None => [lead.into(), format!("{space}{}", self.note), String::new()],
        }
    }

    /// The line's words as drawn, in one.
    #[cfg(test)]
    fn text(&self) -> String {
        match self.note.is_empty() {
            true => self.title.to_string(),
            false if self.joined => format!("{}{}", self.title, self.note_parts().concat()),
            false => format!("{} {}", self.title, self.note_parts().concat()),
        }
    }
}

/// What a helper's item draws.
enum HelperRow {
    /// Its own line: the words it's known by, then its page's word.
    Known(TraceLine),
    /// Its step's line, as it's known by no words of its own.
    Step,
    /// No line yet: it has no name, no page with a title and no step, so it would be a glyph
    /// alone. Its stretch's count is all that shows it until it has its first words.
    Nothing,
}

fn helper_row(step: &TraceStep, name: &SharedString, page: Option<&HelperPage>) -> HelperRow {
    match helper_line(step, name, page) {
        Some(line) => HelperRow::Known(line),
        None if step.title.is_empty() => HelperRow::Nothing,
        None => HelperRow::Step,
    }
}

/// The line of a helper known by words of its own (#4102): the name its task gave it, or with
/// none its page's title, as the page's row in the sidebar says it. Its page's word follows, as
/// that row ends, and its page's favicon stands in its step's glyph's place, which a page without
/// one keeps. Nothing on it is the helper's own text or what it found. None for one with neither
/// a name nor a titled page.
fn helper_line(
    step: &TraceStep,
    name: &SharedString,
    page: Option<&HelperPage>,
) -> Option<TraceLine> {
    let known = match name.is_empty() {
        false => name.clone(),
        true => (page.map(|page| page.title.clone())).filter(|title| !title.is_empty())?,
    };
    let word = page.map_or("", |page| page.word);
    let mut line = TraceLine::new(Fold::Step(step.key), known, word.into());
    line.icon = page.and_then(|page| page.favicon.clone());
    line.glyph = line.icon.is_none().then_some(step.glyph);
    Some(line)
}

/// A stretch's summary line: its title and what it counts, joined as one string.
fn summary_line(trace: &Trace, now: Instant) -> TraceLine {
    let (title, note) = fold_words(trace, now);
    TraceLine {
        head: Some(trace.took.is_none()),
        ..TraceLine::new(Fold::Trace(trace.key), title, note)
    }
}

/// A stretch's fold's words at `now`: at work its word and the time so far, from its first step,
/// counted on the frames its running ring draws; done, the time it took.
fn fold_words(trace: &Trace, now: Instant) -> (SharedString, SharedString) {
    let so_far = (trace.started).map_or(0, |started| {
        now.saturating_duration_since(started).as_secs()
    });
    let (title, note) = words::fold(trace.took, so_far.max(1));
    (title.into(), note.into())
}

/// Which line a `TraceLine` is: a trace's, which folds, or one of ilo's.
#[derive(Clone, Copy)]
enum LineKey {
    Fold(Fold),
    Ilo(usize),
    /// The split's line over a run of helpers, by the first one's key.
    Split(usize),
}

impl From<Fold> for LineKey {
    fn from(fold: Fold) -> Self {
        LineKey::Fold(fold)
    }
}

/// A trace's line, with a chevron if it opens, which turns down as it opens. Under the pointer, a
/// line that opens has a plate hanging outside its text, which goes to the title's colour.
fn trace_line(look: &Look, line: TraceLine) -> gpui::Stateful<gpui::Div> {
    let palette = look.palette;
    let parts = line.note_parts();
    let at_right = line.note_at_right();
    let inks = line.inks(&palette);
    let TraceLine {
        key,
        glyph,
        icon,
        mark,
        highlights,
        words_drawn,
        failed,
        title,
        note: description,
        chevron,
        shimmers,
        ring,
        joined_above,
        ring_after,
        head,
        page,
        icons,
        alpha,
        note_alpha,
        chevron_alpha,
        rolled,
        joined,
    } = line;
    let [lead, count, rest] = parts;
    let space = if joined { " " } else { "" };
    // A step reads as its words, in its place among the lines; one of ilo's is named as a whole
    // (`TalkView::ilo`).
    let read = (look.a11y && mark.is_none()).then(|| match description.is_empty() {
        true => title.to_string(),
        false => format!("{title} {description}"),
    });
    let (id, group): (ElementId, SharedString) = match key {
        LineKey::Ilo(key) => (("ilo", key).into(), format!("ilo-{key}").into()),
        LineKey::Split(key) => (
            ("trace-split", key).into(),
            format!("trace-split-{key}").into(),
        ),
        LineKey::Fold(Fold::Trace(key)) => (("trace", key).into(), format!("trace-{key}").into()),
        LineKey::Fold(Fold::Step(key)) => (
            ("trace-step", key).into(),
            format!("trace-step-{key}").into(),
        ),
        LineKey::Fold(Fold::Page(key)) => (
            ("trace-page", key).into(),
            format!("trace-page-{key}").into(),
        ),
        LineKey::Fold(Fold::Group(key)) => (
            ("trace-group", key).into(),
            format!("trace-group-{key}").into(),
        ),
    };
    let LineInks {
        words,
        note,
        glyph: glyph_ink,
        pointed,
    } = inks;
    let ilos = mark.is_some();
    let glyph = glyph.map(|glyph| {
        let size = if ilos { ILO_GLYPH } else { TRACE_GLYPH };
        let color = glyph_ink;
        let path = if failed { "icons/x.svg" } else { glyph };
        match ring {
            Some(turned) => div()
                .flex_none()
                .size(px(size))
                .flex()
                .items_center()
                .justify_center()
                .child(ring::ring(
                    STEP_RING,
                    STEP_RING_STROKE,
                    turned,
                    palette.text(RING_TRACK),
                    palette.text(TITLE),
                ))
                .into_any_element(),
            None => svg()
                .flex_none()
                .path(path)
                .size(px(size))
                .text_color(color)
                .into_any_element(),
        }
    });
    // One of ilo's lines wraps its words and never cuts them (#4105), so what stands beside them
    // keeps to their first line, which is where a trace's one line is.
    let first = |part: gpui::AnyElement| match ilos {
        true => div()
            .flex_none()
            .h(px(TRACE_LINE))
            .flex()
            .items_center()
            .child(part)
            .into_any_element(),
        false => part,
    };
    let opens = chevron.is_some() || page.is_some();
    let chevron = chevron.map(|open| {
        svg()
            .flex_none()
            .path("icons/chevron-down.svg")
            .size(px(TRACE_CHEVRON))
            .text_color(palette.glyph(TRACE_GLYPH_ALPHA))
            .opacity(chevron_alpha)
            .when(open < 1., |glyph| {
                glyph.with_transformation(Transformation::rotate(Radians(-FRAC_PI_2 * (1. - open))))
            })
    });
    // A count that changed rolls its number, and the words before and after it stay.
    let noted = !description.is_empty();
    let description = match rolled {
        Some((was, since)) if !rest.is_empty() => div()
            .flex_none()
            .flex()
            .child(lead)
            .child(self::rolled(
                count.into(),
                // The old count takes the new one's space before it, if it has one.
                Some(&(format!("{space}{was}").into(), since)),
                TRACE_LINE,
                look,
            ))
            .child(rest)
            .into_any_element(),
        _ => div()
            .flex_none()
            .child(format!("{lead}{count}{rest}"))
            .into_any_element(),
    };
    let hover = group.clone();
    let mut laid = None;
    let title = div()
        .min_w_0()
        .when(at_right, |title| title.flex_1())
        .map(|title| match ilos {
            true => title
                .relative()
                .py(px((TRACE_LINE - ILO_TEXT_LINE) / 2.))
                .line_height(px(ILO_TEXT_LINE)),
            false => title.truncate(),
        })
        .opacity(alpha)
        .text_color(words)
        .when(head == Some(true), |title| {
            title.font_weight(FontWeight::MEDIUM)
        })
        .when_some(pointed, |title, pointed| {
            title.group_hover(hover, move |style| style.text_color(pointed))
        })
        .child(match shimmers {
            Some(Some(phase)) => shimmering(&palette, title, phase),
            _ => {
                let text = StyledText::new(title)
                    .with_highlights(highlights)
                    .with_font_family_overrides(mark);
                laid = Some(text.layout().clone());
                text
            }
        })
        .children(words_drawn.zip(laid).map(|((key, drawn), laid)| {
            canvas(
                move |bounds, _, _| _ = drawn.borrow_mut().insert(key, (bounds, laid)),
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        }));
    div()
        .id(id)
        .when_some(read, |line, read| line.role(Role::Label).aria_label(read))
        .group(group)
        .flex_none()
        .max_w_full()
        .when(at_right, |line| line.w_full())
        .flex()
        .gap(px(TRACE_GAP))
        .rounded(px(NAV_RADIUS))
        .map(|line| match ilos {
            true => line.min_h(px(TRACE_LINE)).items_start(),
            // Its plate hangs outside its text, which ilo's lines, which don't open, have none of.
            false => line
                .relative()
                .h(px(match (head, page) {
                    (Some(_), _) => FOLD_HEAD,
                    (_, Some(_)) => PAGE_LINE,
                    _ => TRACE_LINE,
                }))
                .when(head.is_some(), |line| line.gap(px(FOLD_GAP)))
                .ml(px(-TRACE_HANG))
                .px(px(TRACE_HANG))
                .items_center()
                .whitespace_nowrap()
                .text_size(px(TRACE_TEXT))
                .line_height(px(TRACE_TEXT_LINE)),
        })
        .when(joined_above, |line| {
            line.child(
                div()
                    .absolute()
                    .left(px(TRACE_HANG + CONNECTOR_X - CONNECTOR_WIDTH / 2.))
                    .top(px((TRACE_LINE - TRACE_GLYPH) / 2.
                        - CONNECTOR_ABOVE
                        - CONNECTOR_HEIGHT))
                    .w(px(CONNECTOR_WIDTH))
                    .h(px(CONNECTOR_HEIGHT))
                    .bg(palette.text(CONNECTOR_INK)),
            )
        })
        .when(opens, |line| {
            line.cursor_pointer()
                .hover(move |style| style.bg(palette.plate(TRACE_HOVER)))
        })
        .children(glyph.map(first))
        .children(icon.map(|icon| {
            first(
                img(icon)
                    .flex_none()
                    .size(px(TRACE_GLYPH))
                    .into_any_element(),
            )
        }))
        .map(|line| {
            let note = noted.then(|| {
                first(
                    div()
                        .flex_none()
                        .opacity(note_alpha)
                        .text_color(note)
                        .child(description)
                        .into_any_element(),
                )
            });
            match joined {
                // One string: the note right after the title, its dot and spaces its own.
                true => line.child(
                    div()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .child(title)
                        .children(note),
                ),
                false => line.child(title).children(note),
            }
        })
        .children(ring_after.map(|turned| {
            ring::ring(
                STEP_RING,
                STEP_RING_STROKE,
                turned,
                palette.text(RING_TRACK),
                palette.text(TITLE),
            )
        }))
        .when(!icons.is_empty(), |line| {
            line.child(
                div()
                    .flex_none()
                    .ml(px(FOLD_DONE_GAP - FOLD_GAP))
                    .flex()
                    .items_center()
                    .gap(px(FOLD_ICONS_GAP))
                    .children(icons.into_iter().map(|icon| {
                        match icon {
                            Some(icon) => {
                                img(icon).flex_none().size(px(FOLD_ICON)).into_any_element()
                            }
                            None => svg()
                                .flex_none()
                                .path(PAGE_GLYPH)
                                .size(px(FOLD_ICON))
                                .text_color(palette.glyph(TRACE_GLYPH_ALPHA))
                                .into_any_element(),
                        }
                    })),
            )
        })
        .children(chevron)
}

/// A done stretch's fold's icons: each page it visited, once.
fn line_icons(icons: &mut Vec<Option<Arc<Image>>>, paged: &[pages::Paged], look: &Look) {
    icons.extend((pages::visited(paged).into_iter()).map(|page| {
        look.page_head(page.tab)
            .and_then(|head| head.favicon.clone())
    }));
}

/// The split's line's glyph.
const SPLIT_GLYPH: &str = "icons/network.svg";

/// A page's glyph where its tab has no icon.
const PAGE_GLYPH: &str = "icons/globe.svg";

/// A title a band of the full ink sweeps across, `phase` of the way through a pass.
fn shimmering(palette: &super::Palette, title: SharedString, phase: f32) -> StyledText {
    let count = title.chars().count().max(1) as f32;
    let middle = -SHIMMER_BAND + (1. + 2. * SHIMMER_BAND) * phase;
    let highlights: Vec<_> = (title.char_indices().enumerate())
        .map(|(index, (at, char))| {
            let along = (index as f32 + 0.5) / count;
            let near = (1. - (along - middle).abs() / SHIMMER_BAND).max(0.);
            let highlight = palette.lit_highlight(TRACE_TITLE, SHIMMER, near);
            (at..at + char.len_utf8(), highlight)
        })
        .collect();
    StyledText::new(title).with_highlights(highlights)
}

/// What the view takes from its window as it draws: the colours, the time, whether motion is
/// reduced, and the favicon of the page the task was asked on.
struct Look {
    palette: super::Palette,
    now: Instant,
    still: bool,
    asked_favicon: Option<Arc<Image>>,
    /// The pages its helpers' lines are of (#4102).
    helper_pages: Vec<HelperPage>,
    /// The tabs its stretches acted on, by which a page's line has its title and icon.
    pages: Vec<PageHead>,
    /// The thread's pages, each by its tab and address: the only places its words link to.
    thread_pages: Vec<(u64, SharedString)>,
    /// A page watched is going back into its picture over it, which hides what's under it.
    behind: bool,
    /// The user stands on one of its pages, the conversation in a panel beside it.
    standing: bool,
    /// An assistive app reads the window (`TalkView::a11y`).
    a11y: bool,
}

/// What a helper's line shows of its page: its title, which a helper its task gave no name is
/// known by, its favicon, and the word its row in the sidebar ends with.
struct HelperPage {
    tab: u64,
    title: SharedString,
    favicon: Option<Arc<Image>>,
    word: &'static str,
}

/// What a page's line shows of its tab: its title and favicon.
struct PageHead {
    tab: u64,
    title: SharedString,
    favicon: Option<Arc<Image>>,
}

impl Look {
    fn page_head(&self, tab: Option<u64>) -> Option<&PageHead> {
        tab.and_then(|tab| self.pages.iter().find(|page| page.tab == tab))
    }

    fn helper_page(&self, tab: u64) -> Option<&HelperPage> {
        self.helper_pages.iter().find(|page| page.tab == tab)
    }

    /// How far through its pass the shimmer is, or None under Reduce Motion.
    fn shimmer(&self, since: Instant) -> Option<f32> {
        (!self.still).then(|| ((self.now - since).as_secs_f32() / SHIMMER_PERIOD).fract())
    }
}

impl Shell {
    /// What the panel's page row holds for `tab` while the user stands on it, which the ring's
    /// stops follow, so it never rings one the row doesn't draw.
    pub(super) fn page_row_shown(&self, tab: u64, cx: &gpui::App) -> PageRow {
        let Some(talk) = self.talk.as_ref().filter(|_| self.task_standing()) else {
            return PageRow::default();
        };
        talk.view
            .read(cx)
            .page_row_for(tab, self.task_strip_open(tab))
    }

    /// The favicon of an open tab at `url`, if one has it.
    fn favicon_at(&self, url: &str) -> Option<Arc<Image>> {
        (self.tabs.values())
            .find(|tab| tab.page.url.as_ref() == url)
            .and_then(|tab| tab.favicon.clone())
    }

    /// What the lines of the pages a task acted on, `tabs`, show of the tabs still open.
    fn page_heads(&self, tabs: Vec<u64>) -> Vec<PageHead> {
        (tabs.into_iter())
            .filter_map(|tab| {
                let open = self.tabs.get(&tab)?;
                Some(PageHead {
                    tab,
                    title: open.title.clone(),
                    favicon: open.favicon.clone(),
                })
            })
            .collect()
    }

    /// What `agent`'s task's helpers' lines show of their pages, `tabs`: each one's favicon and
    /// its word, as the task's group in the sidebar has them. None for a task with no card.
    fn helper_pages(&self, agent: &SharedString, tabs: Vec<u64>) -> Vec<HelperPage> {
        let Some(card) = self.task_cards.iter().find(|card| card.agent == *agent) else {
            return Vec::new();
        };
        let page = |tab: u64| HelperPage {
            tab,
            title: (self.tabs.get(&tab))
                .map_or_else(SharedString::default, |tab| tab.title.clone()),
            favicon: self.tabs.get(&tab).and_then(|tab| tab.favicon.clone()),
            word: self.page_word(card, tab).label(),
        };
        tabs.into_iter().map(page).collect()
    }
}

/// Where a link in an answer goes.
#[derive(Clone, Debug, PartialEq)]
enum Linked {
    /// A page of the thread, its tab.
    Page(u64),
    /// Any other web address, which opens as a tab of the user's Space.
    Web(SharedString),
}

/// Where a link to `url` goes: a page among the thread's `pages`, else the web for an http(s)
/// address, else nowhere, its words plain.
fn linked_to(pages: &[(u64, SharedString)], url: &SharedString) -> Option<Linked> {
    if let Some(tab) = thread_tab(pages, url) {
        return Some(Linked::Page(tab));
    }
    let web = url.starts_with("https://") || url.starts_with("http://");
    web.then(|| Linked::Web(url.clone()))
}

/// What a web link's tooltip says: its host, then the rest of its address, each plain and cut in
/// the middle. Both come from the address as parsed, so a host in capitals is found as any is.
fn link_tip(url: &str) -> Option<(SharedString, SharedString)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = cut_middle(parsed.host_str()?, LINK_TIP_MOST);
    let rest = cut_middle(&parsed[url::Position::AfterHost..], LINK_TIP_MOST);
    Some((host.into(), rest.into()))
}

/// `text` without control characters, and if longer than `most` characters, its start and end
/// with an ellipsis between, `most` in all.
fn cut_middle(text: &str, most: usize) -> String {
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    let count = text.chars().count();
    if count <= most {
        return text;
    }
    let head = (most - 1) / 2;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - (most - 1 - head)).collect();
    format!("{start}\u{2026}{end}")
}

/// `url` without its `user:pass@` part, if it is an address that has one.
fn without_sign_in(url: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    if parsed.username().is_empty() && parsed.password().is_none() {
        return None;
    }
    parsed.set_username("").ok()?;
    parsed.set_password(None).ok()?;
    Some(parsed.into())
}

/// The most of an address after its host a link's tooltip says, the ellipsis one of them.
const LINK_TIP_MOST: usize = 48;

/// `inline` with each link whose words are themselves an address on another host than the link's
/// drawn with the link's real address in the words' place, so words can't pass for where it goes.
/// A link's address loses any `user:pass@` part, and words that hold one are drawn as the address
/// without it, so a click opens no sign-in the answer chose and the line shows none.
fn honest(mut inline: Inline) -> Inline {
    for (_, url) in &mut inline.links {
        if let Some(bare) = without_sign_in(url) {
            *url = bare.into();
        }
    }
    let mut links = inline.links.clone();
    links.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, url) in links {
        let Some(words) = inline.text.get(range.clone()) else {
            continue;
        };
        let address =
            words.contains("://") || (words.contains('.') && !words.contains(char::is_whitespace));
        let host = crate::agents::fence::host_of;
        let signed_in = without_sign_in(words.trim()).is_some();
        let elsewhere = match (host(words), host(url.as_ref())) {
            (Some(said), Some(real)) => address && said != real,
            _ => false,
        };
        if !(signed_in || elsewhere) {
            continue;
        }
        let grown = url.len() as isize - range.len() as isize;
        let moved = |at: usize, end: bool| match at {
            at if at <= range.start => at,
            at if at >= range.end => (at as isize + grown) as usize,
            _ if end => range.start + url.len(),
            _ => range.start,
        };
        let map = |each: &std::ops::Range<usize>| moved(each.start, false)..moved(each.end, true);
        inline.text.replace_range(range.clone(), url.as_ref());
        for (each, _) in &mut inline.styled {
            *each = map(each);
        }
        for (each, _) in &mut inline.links {
            *each = map(each);
        }
    }
    inline
}

/// The tab among a thread's `pages` that `url` is, the same address but for a closing slash or
/// fragment.
fn thread_tab(pages: &[(u64, SharedString)], url: &str) -> Option<u64> {
    let bare = |url: &str| {
        let url = url.split('#').next().unwrap_or(url);
        url.strip_suffix('/').unwrap_or(url).to_string()
    };
    let url = bare(url);
    (pages.iter())
        .find(|(_, page)| bare(page) == url)
        .map(|(tab, _)| *tab)
}

/// The tabs the steps of `lines` acted on.
fn page_tabs(lines: &[Line]) -> Vec<u64> {
    let mut tabs: Vec<u64> = (lines.iter())
        .filter_map(|line| match line {
            Line::Trace(trace) => Some(&trace.items),
            _ => None,
        })
        .flatten()
        .filter_map(|item| match item {
            TraceItem::Step(step) => step.page.as_ref().and_then(|page| page.tab),
            _ => None,
        })
        .collect();
    tabs.sort_unstable();
    tabs.dedup();
    tabs
}

/// The tabs of the helpers `lines` name.
fn helper_tabs(lines: &[Line]) -> Vec<u64> {
    let traces = lines.iter().filter_map(|line| match line {
        Line::Trace(trace) => Some(&trace.items),
        _ => None,
    });
    (traces.flatten())
        .filter_map(|item| match item {
            TraceItem::Helper { tab, .. } => *tab,
            _ => None,
        })
        .collect()
}

impl Render for TalkView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.a11y = window.is_a11y_active();
        self.step_ilo_keys(window, cx);
        let now = Instant::now();
        let still = crate::motion::still(cx);
        let asked = self
            .shown
            .as_ref()
            .and_then(|shown| shown.page.as_ref())
            .map(|page| page.url.clone());
        let helpers =
            (self.shown.as_ref()).map_or_else(Vec::new, |shown| helper_tabs(&shown.lines));
        let paged = (self.shown.as_ref()).map_or_else(Vec::new, |shown| page_tabs(&shown.lines));
        let thread_tabs: Vec<u64> = (self.shown.as_ref()).map_or_else(Vec::new, |shown| {
            shown.tabs.iter().map(|(tab, _)| *tab).collect()
        });
        let Some(look) = self.shell.upgrade().map(|shell| {
            let shell = shell.read(cx);
            let asked_favicon = asked.and_then(|url| shell.favicon_at(&url));
            Look {
                palette: shell.palette,
                now,
                still,
                asked_favicon,
                helper_pages: shell.helper_pages(&self.agent, helpers),
                pages: shell.page_heads(paged),
                thread_pages: (thread_tabs.into_iter())
                    .filter_map(|tab| Some((tab, shell.tabs.get(&tab)?.page.url.clone())))
                    .collect(),
                behind: shell.task_page_going_back(),
                standing: shell.task_standing(),
                a11y: self.a11y,
            }
        }) else {
            return div().into_any_element();
        };
        self.settle_folds(now, still);
        if look.standing
            && let Some(tab) = self.beside(cx)
        {
            self.visited.insert(tab);
        }
        // A shorter or narrower window, or the lines' column narrower or wider as a page goes
        // beside them or away, keeps the newest line in view while the user reads at the end, as
        // the scroll keeps its distance from the top; a follow under way ends there. Only the
        // user's own scroll takes the lines off their end.
        let viewport = (window.viewport_size(), look.standing);
        if std::mem::replace(&mut self.viewport, viewport) != viewport
            && (self.at_end() || self.motion.follow.is_some())
        {
            self.scroll.scroll_to_bottom();
            self.motion.follow = None;
        }
        self.level_drawn.set(None);
        self.folder_drawn.set(None);
        self.plus.forget();
        self.take_over_drawn.borrow_mut().clear();
        self.hand_back_drawn.set(None);
        self.reply_drawn.set(None);
        self.speaker_drawn.set(None);
        let busy = self.motion.settle(now);
        self.follow_on(now);
        // What changes once it has shown moves.
        self.motion.live = true;
        // What moves does only while it shows: what opens, folds or lands on the display's frames,
        // and the shimmer, which moves while a step is under way and never while one waits on the
        // user, on the loading dots' clock, as each frame draws the whole window again (#1700).
        let shimmers = self.moving(still);
        if !self.folding.is_empty() || busy {
            window.request_animation_frame();
        } else if shimmers && let Some(shell) = self.shell.upgrade() {
            shell.read(cx).breath.keep();
            cx.defer(move |cx| shell.update(cx, |shell, cx| shell.breath.arm(cx)));
        }
        if std::mem::replace(&mut self.shimmering, shimmers) != shimmers {
            // check.sh reads these lines.
            match shimmers {
                true => log!("browser: task {} shimmers", self.agent),
                false => log!("browser: task {} still", self.agent),
            }
        }
        let palette = look.palette;
        let lines = div()
            .id("conversation-lines")
            .size_full()
            .track_scroll(&self.scroll)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            // A scroll clips at its sides too, so it's wider than the column by the plates that
            // hang out of it, and its text stays on the column.
            .px(px(TRACE_HANG))
            .pb(px(LINES_BELOW))
            .children(self.lines(&look, cx));
        // The header and the reply field are the column's.
        let on_column = |child: gpui::AnyElement| {
            div()
                .w_full()
                .flex()
                .justify_center()
                .child(div().w_full().max_w(px(COLUMN)).child(child))
        };
        // Under a page going back, only the pictures take a press (#3496).
        let shut = |part: gpui::Div| {
            part.relative()
                .when(look.behind, |part| part.child(unpressed()))
        };
        // The task's pages stand in a rail at the area's right edge, or, where the column at its
        // least and the rail don't both fit, in a row under the header. With no page, no rail.
        let pages = (self.shown.as_ref()).is_some_and(|shown| !shown.tabs.is_empty());
        // Beside a page, the panel's head has the pages, and there's no rail.
        let standing = look.standing;
        // The panel's width, drawn last beside a page, is never the area's: going back, the
        // pictures would lay out across for a frame before standing in the rail.
        let was_standing = self.drawn_standing.replace(standing);
        let wide = (self.drawn_at.get())
            .filter(|_| !was_standing)
            .map_or(window.viewport_size().width, |area| area.size.width);
        let across = pages && !standing && f32::from(wide) < RAIL_BREAKPOINT;
        // Its pages show live in their tiles where the core can (live_tiles.rs), but not under a
        // page watched over them, which they'd draw over.
        let held = look.behind
            || (self.shell.upgrade())
                .is_some_and(|shell| shell.read(cx).watched_task_page().is_some());
        let middle = shut(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .justify_center()
                .mb(px(REPLY_GAP))
                // The lines' plates hang out of their text, which is BODY_X in.
                .when(standing, |middle| {
                    middle
                        .px(px(panel::BODY_X - TRACE_HANG))
                        .pt(px(panel::BODY_TOP))
                })
                .child(
                    div()
                        .size_full()
                        .max_w(px(COLUMN + 2. * TRACE_HANG))
                        .child(lines),
                ),
        );
        // Beside a page, the reply field and its row are in from the panel's sides.
        let foot = |part: gpui::Div| part.when(standing, |part| part.px(px(panel::FOOT)));
        let head = match standing {
            true => self.panel_head(&look, cx),
            false => on_column(self.header(&look, cx).into_any_element()),
        };
        let column = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .when(!standing, |column| column.px(px(INSET)).pt(px(TOP)))
            .pb(px(if standing { panel::FOOT } else { REPLY_BOTTOM }))
            .flex()
            .flex_col()
            .text_size(px(TEXT))
            .line_height(px(TEXT_LINE))
            .text_color(palette.text(TITLE))
            .child(shut(head))
            .children(across.then(|| {
                div()
                    .flex_none()
                    .w_full()
                    .mb(px(RAIL_GAP))
                    .child(
                        self.tiles
                            .over(self.rail(true, &look, cx), self.drawn.clone(), held),
                    )
            }))
            .child(middle)
            .children(
                standing
                    .then(|| self.page_row(&look, cx))
                    .flatten()
                    .map(|row| shut(foot(row))),
            )
            // With ilo's line and Start Again in the field's place there is no level to set, so
            // the row goes with the field (#4105). An outside app's session has no composer
            // beside its page: its user talks to it in its own app.
            .children(
                (self.replies || !standing)
                    .then(|| shut(foot(on_column(self.reply(&look, cx).into_any_element())))),
            );
        let rail = (pages && !across && !standing).then(|| {
            div()
                .flex_none()
                .w(px(RAIL_WIDTH))
                .h_full()
                .py(px(RAIL_EDGE))
                .mr(px(RAIL_EDGE))
                .flex()
                .flex_col()
                .child(
                    self.tiles
                        .over(self.rail(false, &look, cx), self.drawn.clone(), held),
                )
        });
        if !pages {
            self.drawn.borrow_mut().clear();
            self.crashed_drawn.borrow_mut().clear();
            self.tiles.end_all(cx);
        } else if standing {
            // Beside a page stood on there's no rail to place its tiles in, and one left live
            // would stay drawn where its picture was, over the page's part.
            self.tiles.end_all(cx);
        }
        let column = div().size_full().flex().child(column).children(rail);
        let drawn = self.drawn_at.clone();
        div()
            .id("conversation")
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|view, _: &SendReply, window, cx| view.send_reply(window, cx)))
            .capture_action(
                cx.listener(|view, _: &crate::text_input::Backspace, _, cx| {
                    if view.unpick_last(cx) {
                        cx.stop_propagation();
                    }
                }),
            )
            .on_action(cx.listener(Self::ilo_press))
            .on_action(cx.listener(Self::ilo_next))
            .on_action(cx.listener(Self::ilo_back))
            .on_action(cx.listener(Self::ilo_leave))
            .size_full()
            .bg(stage_fill(&palette))
            .flex()
            .justify_center()
            // A press here is the conversation's, not the window background's, which moves the
            // window.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .relative()
            .child(column)
            .child(
                canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .children(self.ghost(&look))
            .children(self.announcer())
            .into_any_element()
    }
}

/// The head of one of ilo's lines as drawn: its mark and words, the name it took from a page cut
/// short and where it stands in them, and how many times it came.
struct IloHead {
    said: String,
    name: Option<String>,
    quoted: Option<std::ops::Range<usize>>,
    count: Option<String>,
}

fn ilo_head(kind: &IloKind, agent: &str) -> IloHead {
    let (own, name) = words::head(kind, agent);
    let own = format!("{} {own}", words::MARK);
    // A page's words or a site are cut short.
    let name = name.map(|name| clip(&name, QUOTED_CHARS as usize));
    let (said, quoted) = match &name {
        Some(name) => {
            let (mut said, quoted) = words::with_name(kind, &own, name);
            // What its sentence says after the name.
            said.extend(words::tail(kind, agent));
            (said, Some(quoted))
        }
        None => (own, None),
    };
    let count = match kind {
        IloKind::Stopped { count, .. } => words::count(*count),
        _ => None,
    };
    IloHead {
        said,
        name,
        quoted,
        count,
    }
}

/// What a screen reader is told of one of ilo's lines: its own words as drawn, its head, how
/// many times it came and what it says more, with a page's words cut short and quoted as they are
/// on the line. Never anything an agent or a page wrote as ilo's.
fn ilo_spoken(line: &IloLine, agent: &str) -> String {
    let IloHead {
        mut said,
        name,
        count,
        ..
    } = ilo_head(&line.kind, agent);
    let body = words::body(&line.kind, name.as_deref().unwrap_or(""));
    for more in count.into_iter().chain(body) {
        said.push(' ');
        said.push_str(&more);
    }
    said
}

/// What an agent said as its text is drawn, without the marks that style it: each paragraph and
/// each item of a list, with its marker, on a line, and a link's address after its words.
fn plain(text: &str) -> String {
    let lines = markdown::blocks(text).into_iter().map(|block| match block {
        Block::Paragraph(text) => addressed(text),
        Block::Item { marker, text, .. } => format!("{marker} {}", addressed(text)),
    });
    lines.collect::<Vec<_>>().join("\n")
}

/// An inline's text with each link's address in brackets after its words, unless the words are
/// the address.
fn addressed(inline: markdown::Inline) -> String {
    let inline = honest(inline);
    let mut text = inline.text;
    let mut links = inline.links;
    links.sort_by_key(|(range, _)| std::cmp::Reverse(range.end));
    for (range, url) in links {
        if text.get(range.clone()) != Some(url.as_ref()) && text.is_char_boundary(range.end) {
            text.insert_str(range.end, &format!(" ({url})"));
        }
    }
    text
}

/// What covers a part of the conversation while a page goes back over it, which takes a press there
/// and does nothing: the user can't see yet what it would stop or change (#3496).
fn unpressed() -> gpui::Div {
    div()
        .absolute()
        .left_0()
        .top_0()
        .size_full()
        .block_mouse_except_scroll()
}

#[cfg(test)]
mod spoken_tests;
#[cfg(test)]
mod tests;

/// The step under way at the end of a task's lines, if one is: the last trace's last step while
/// it runs, or a session's last step line. A step that ended or was refused isn't.
/// What the panel's page row holds for the page stood on. One button at most, but for an outside
/// app's session, which has no composer and so its Stop at the row's right end.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct PageRow {
    pub(super) case: Option<RowCase>,
    pub(super) stop: bool,
    /// The page is a tab the user handed the thread: the way back stands after the row's button.
    pub(super) back: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum RowCase {
    /// The thread works on the page: the step under way, with Take Over.
    Working(SharedString),
    /// The user holds the page: what `words::live` says of it, with Hand Back.
    Yours(SharedString),
    /// The thread has ended with this page to move to the user's Space: Open, as its words say.
    Ended(SharedString),
}

impl RowCase {
    /// The ring's stop on the row's button.
    pub(super) fn button(&self) -> StripStop {
        match self {
            RowCase::Working(_) => StripStop::TakeOver,
            RowCase::Yours(_) => StripStop::HandBack,
            RowCase::Ended(_) => StripStop::Open,
        }
    }

    /// The row's words before its button, which the ring's page stop rings: none beside Open.
    pub(super) fn said(&self) -> Option<&SharedString> {
        match self {
            RowCase::Working(said) | RowCase::Yours(said) => Some(said),
            RowCase::Ended(_) => None,
        }
    }
}

fn under_way(lines: &[Line]) -> Option<&TraceStep> {
    let step = match lines.last()? {
        Line::Trace(trace) => match trace.items.last()? {
            TraceItem::Step(step) => step,
            _ => return None,
        },
        Line::Step(step) => step,
        _ => return None,
    };
    (step.running && !step.failed).then_some(step)
}
