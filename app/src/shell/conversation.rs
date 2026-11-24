//! A task's conversation, which the page area shows in the page's place (Agent task,
//! task_area.rs). A click on a task's card shows it, coming in from the card's side. Its title and
//! where it is head it, its pages' pictures under them, taken only while it shows, then what was
//! asked with its page and what each side said, with the work between two messages a trace, and
//! the reply field at its foot. A picture's click zooms its page out of it. Esc steps back to the
//! Space's tab, ⌘W closes the card, and ⌘. stops the task.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
    Bounds, Context, Corners, ElementId, Entity, Focusable, FontStyle, FontWeight, HighlightStyle,
    Hsla, Image, InteractiveText, IntoElement, MouseButton, ObjectFit, Pixels, Radians, Render,
    RenderImage, ScrollHandle, SharedString, Size, StyledText, Transformation, UnderlineStyle,
    WeakEntity, Window, actions, canvas, div, img, point, prelude::*, px, rgb, size, svg,
};

use self::markdown::{Block, Inline};
use self::motion::{Answered, Follow, Key, Landing, Motion, Run, eased, linear};
use super::{
    NAV_RADIUS, SECONDARY, Shell, TITLE,
    agent_bar::stop_keys,
    browser::{Browser, Page},
    picture::{PagePicture, next_request},
    row_spring,
    task_cards::PageWord,
};
use crate::agents::Status;
use crate::agents::page::clip;
use crate::agents::task::ilo::{self, IloButton, IloKind, IloLine};
use crate::agents::task::{
    ArchivedTask, Card, Conversation, Dot, Line, Opens, TaskAction, Trace, TraceItem, TraceStep,
};
use crate::palette::{Ground, STATE_HOVERED, STATE_PRESSED};
use crate::text_input::TextInput;
use crate::tokens::command_bar::{CHIP as BAR_CHIP, HINT_KEY_RIM, HINT_KEY_X};
use crate::tokens::find_bar::NOT_FOUND;
use crate::tokens::shell::FORM_PLACEHOLDER;
use crate::tokens::shell::agent_pointer::{COLOR as SIGNAL, RING};
use crate::tokens::shell::conversation::ilo::{
    ARM, BELOW as ILO_BELOW, BUTTONS_GAP, BUTTONS_TOP, COMPOSER_CHEVRON, COMPOSER_CHEVRON_GAP,
    COMPOSER_TOP, GLYPH as ILO_GLYPH, INK as ILO_INK, NOTE_INK, NOTE_TOP, QUOTED_CHARS,
    TEXT_LINE as ILO_TEXT_LINE, TEXT_TOP,
};
use crate::tokens::shell::conversation::motion::{CHEVRON, LAND, PICK_FADE, RISE, STEPS_FADE};
use crate::tokens::shell::conversation::{
    BLOCK_GAP, BUBBLE_BOTTOM, BUBBLE_FILL, BUBBLE_TOP, BUBBLE_WIDTH, BUBBLE_X, CHIP_KEY,
    CHIPS_BELOW, CHIPS_TOP, CODE_FILL, COLUMN, HEADER_BOTTOM, INSET, ITEM_GAP, LINES_BELOW,
    LINK_LINE, LINK_UNDERLINE, LIST_MARKER, META_GAP, META_LINE, OPEN_BELOW, OPEN_TOP,
    PAGE_CHIP_BELOW, PAGE_CHIP_FAVICON, PAGE_CHIP_FILL, PAGE_CHIP_GAP, PAGE_CHIP_HANG,
    PAGE_CHIP_HEIGHT, PAGE_CHIP_X, PARAGRAPH_GAP, PICTURE_EMPTY_GLYPH, PICTURE_HEIGHT,
    PICTURE_RADIUS, PICTURE_WIDTH, RADIUS, REPLY_BOTTOM, REPLY_FILL, REPLY_GAP, REPLY_HEIGHT,
    REPLY_INSET, REPLY_KEY_END, REPLY_RIM, SHIMMER, SHIMMER_BAND, SHIMMER_PERIOD, SITE_GAP,
    TEXT_LINE, TITLE_LINE, TOP, TRACE_BELOW, TRACE_CHEVRON, TRACE_GAP, TRACE_GLYPH,
    TRACE_GLYPH_ALPHA, TRACE_HANG, TRACE_HOVER, TRACE_INDENT, TRACE_LINE, TRACE_NOTE,
    TRACE_PICTURE_HEIGHT, TRACE_PICTURE_RADIUS, TRACE_PICTURE_WIDTH, TRACE_RULE, TRACE_RULE_FILL,
    TRACE_RULE_X, TRACE_TITLE,
};
use crate::tokens::shell::empty::KEYS_FILL;
use crate::tokens::shell::task::motion::{CHIPS, CHIPS_RISE, HEIGHT};
use crate::tokens::shell::task::{
    RAIL_BREAKPOINT, RAIL_DOT, RAIL_FAVICON, RAIL_GAP, RAIL_HEAD, RAIL_WIDTH, STATUS,
};
use crate::tokens::text;
use crate::tokens::toast::{
    CHIP_FILL, CHIP_GAP, CHIP_HEIGHT, CHIP_RADIUS, CHIP_X, KEY_COLOR, KEY_GAP,
};

actions!(conversation, [SendReply, StopTask]);

mod ilo_keys;
mod markdown;
mod motion;
mod words;

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
        .child(words.into())
        .children(key.map(|key| div().text_color(key_color).child(key)))
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
    /// The pages the core last answered with no picture of, which the next action asks again.
    unpictured: Vec<u64>,
    /// Where each picture was drawn last, in the window, for its page to zoom out of.
    drawn: Rc<RefCell<Vec<(u64, Bounds<Pixels>)>>>,
    /// Where each picture was drawn when that was logged last.
    drawn_logged: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// What each tile's word said when that was logged last.
    said_logged: RefCell<HashMap<u64, &'static str>>,
    /// Where Stop was drawn when that was logged last.
    stop_logged: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Its pages changed while one was watched over it, so they're pictured again once it shows.
    stale: bool,
    /// Whether its step under way shimmered as it last drew, which check.sh hears of.
    shimmering: bool,
    /// Its lines, which keep the newest in view as they come while the user is at the end.
    scroll: ScrollHandle,
    /// Its rail of pages, which brings a tile into sight once as its word turns to Needs you.
    rail_scroll: ScrollHandle,
    /// The window's size as last drawn: a change keeps the lines at their end if they were.
    viewport: Size<Pixels>,
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
    /// Where each of ilo's lines' words were last drawn, and their layout, by its line.
    ilo_words_drawn: WordsDrawn,
    /// Where each ilo line's link was drawn, by the line's key.
    ilo_links: Rc<RefCell<HashMap<usize, Bounds<Pixels>>>>,
    /// Where the level's chip under the reply field was drawn, if the last frame drew it.
    level_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Where the last frame drew Take Over on the lines: once, on the live line.
    take_over_drawn: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    landed: Rc<Cell<Option<Bounds<Pixels>>>>,
    drawn_at: Rc<Cell<Option<Bounds<Pixels>>>>,
}

/// What opens in a conversation: a trace, a line of one, or a turn's same step on several tabs,
/// by its key.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Fold {
    Trace(usize),
    Step(usize),
    Group(usize),
}

impl Shell {
    /// A link in a task's conversation: its page opens as a tab of the user's Space, and shows.
    fn open_link(&mut self, url: &str, cx: &mut Context<Self>) {
        // check.sh reads this line.
        log!("browser: task link {url}");
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
        // It asks again at Work on pages, whatever the composer had picked.
        self.forget_composer_tier();
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

    /// The composer's Latest tasks, at most `most`: the tasks the user talks to here, the newest
    /// first, then those a run before left in the profile in front, which no other profile's
    /// Space lists, as one comes back only in its own.
    pub(super) fn latest_tasks(&self, most: usize) -> Vec<Card> {
        let profile = |space: u64| {
            let space = self.spaces.iter().find(|each| each.id == space);
            space.map(|space| space.profile)
        };
        let here = self.shown_space().map(|space| space.profile);
        let kept =
            (self.kept_cards.iter()).filter(|card| here.is_some() && profile(card.home) == here);
        (self.task_cards.iter())
            .filter(|card| card.replies)
            .chain(kept)
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
        let (space, name, replies) = (card.space, card.name.clone(), card.replies);
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
                    let stale = old.view.update(cx, |view, _| view.let_go());
                    self.stale_pictures.extend(stale);
                }
                let placeholder = self.palette.gray(FORM_PLACEHOLDER);
                let reply = cx.new(|cx| {
                    (TextInput::new(format!("Reply to {name}"), cx).placeholder_color(placeholder))
                        .placeholder_after_caret()
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
                    unpictured: Vec::new(),
                    drawn: Rc::default(),
                    drawn_logged: Rc::default(),
                    said_logged: RefCell::default(),
                    stop_logged: Rc::default(),
                    stale: false,
                    shimmering: false,
                    scroll: ScrollHandle::new(),
                    rail_scroll: ScrollHandle::new(),
                    viewport: Size::default(),
                    opened: HashMap::new(),
                    was_open: HashMap::new(),
                    folding: HashMap::new(),
                    heights: Rc::default(),
                    made: Instant::now(),
                    motion: Motion::default(),
                    reply_drawn: Rc::default(),
                    answers_drawn: Rc::default(),
                    ilo_drawn: Rc::default(),
                    ilo_hovered: Rc::default(),
                    ilo_keys: None,
                    ilo_stops,
                    ilo_words_drawn: Rc::default(),
                    ilo_links: Rc::default(),
                    level_drawn: Rc::default(),
                    take_over_drawn: Rc::default(),
                    landed: Rc::default(),
                    drawn_at: Rc::default(),
                })
            }
        };
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
        match view.update(cx, |view, cx| {
            view.set(conversation, watched, &*browser, cx)
        }) {
            Some(stale) => self.stale_pictures.extend(stale),
            None => self.close_task(cx),
        }
    }

    /// A page watched over the conversation has gone back into its picture: the pictures are
    /// taken again if its pages changed meanwhile.
    pub(super) fn retake_talk_pictures(&mut self, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        let (view, browser) = (talk.view.clone(), self.browser.clone());
        let stale = view.update(cx, |view, cx| match view.shown.clone() {
            Some(shown) if view.stale => view.set(Some(shown), false, &*browser, cx),
            _ => None,
        });
        self.stale_pictures.extend(stale.unwrap_or_default());
    }

    /// The page area no longer shows the conversation: its pictures go, taken again if it shows
    /// again.
    pub(super) fn talk_closed(&mut self, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        let view = talk.view.clone();
        let stale = view.update(cx, |view, _| view.let_go());
        self.stale_pictures.extend(stale);
    }

    /// A picture the conversation asked for came: whether it was one of its own.
    pub(super) fn talk_pictured(
        &mut self,
        request: u64,
        picture: &Option<PagePicture>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(talk) = &self.talk else { return false };
        let view = talk.view.clone();
        let Some(old) = view.update(cx, |view, cx| view.pictured(request, picture, cx)) else {
            return false;
        };
        self.stale_pictures.extend(old);
        true
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
    /// back; a finished task's folds into the Archive as the conversation goes. Says whether there
    /// was one.
    pub(super) fn dismiss_talk(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(agent) = self.talk_agent() else {
            return false;
        };
        log!("browser: command dismiss-task via conversation");
        let finished = self
            .task_cards
            .iter()
            .any(|card| card.agent == agent && card.finished());
        if !finished {
            self.dismiss_task(agent, cx);
        }
        self.close_task(cx);
        true
    }

    /// Gives the reply field the keyboard once the card's click has shown the conversation, and
    /// takes it back while a page watched covers the field: GPUI sends the keys of a field that
    /// isn't drawn to no binding of ours, so Esc stepped back from nothing.
    pub(super) fn focus_talk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            for (tab, _) in &conversation.tabs {
                let old = self.pictures.iter_mut().find(|(each, ..)| each == tab);
                let request = next_request();
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
            self.shown = Some(conversation);
            cx.notify();
        }
        Some(stale)
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
        let entry = self
            .pictures
            .iter_mut()
            .find(|(_, each, _)| *each == request)?;
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

    /// Its pictures go, as it no longer shows, and are taken again when it does.
    fn let_go(&mut self) -> Vec<Arc<RenderImage>> {
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
        if text.is_empty() {
            return;
        }
        // It goes from where it was typed to its plate, the field's box around its words.
        if let Some(field) = self.reply_drawn.get().filter(|_| !crate::motion::still(cx)) {
            let style = window.text_style();
            let words = (window.text_system())
                .shape_line(
                    text.clone().into(),
                    px(text::BODY),
                    &[style.to_run(text.len())],
                    None,
                )
                .width;
            let width = (words + px(2. * REPLY_INSET)).min(field.size.width);
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
        self.act(TaskAction::Reply(text.into()), cx);
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
        self.act(TaskAction::Reply(answer), cx);
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
        self.shell
            .update(cx, |shell, cx| {
                shell.watch_task_page(tab, from, picture, cx)
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
                    .text_size(px(text::TITLE))
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
                    .text_size(px(text::SMALL))
                    .line_height(px(META_LINE))
                    .text_color(palette.gray(STATUS))
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
                    let logged = self.stop_logged.clone();
                    let at = canvas(
                        move |bounds, _, _| {
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
                        },
                        |_, _, _, _| {},
                    )
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
        let palette = look.palette;
        let tabs = (self.shown.as_ref()).map_or_else(Vec::new, |shown| shown.tabs.clone());
        let on = self.shown.as_ref().and_then(|shown| shown.on);
        let high = RAIL_WIDTH * PICTURE_HEIGHT / PICTURE_WIDTH;
        let shell = self.shell.upgrade();
        // Each page's own step while it works on several, as with helpers: every one is ringed.
        let steps = match (&shell, &self.shown) {
            (Some(shell), Some(shown)) => shell.read(cx).task_page_steps(&shown.agent).to_vec(),
            _ => Vec::new(),
        };
        let tiles = tabs.iter().enumerate().map(|(index, (tab, site))| {
            let tab = *tab;
            let opacity = match self.motion.came(Key::Picture(tab)) {
                Some(since) if !look.still => eased(since, look.now, RISE),
                _ => 1.,
            };
            let image = (self.pictures.iter().find(|(each, ..)| *each == tab))
                .and_then(|(_, _, picture)| picture.as_ref().map(PagePicture::image));
            let (title, favicon, page, held, said) = match (&shell, &self.shown) {
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
                    let said = card.map(|card| shell.page_word(card, tab));
                    (title, favicon, shell.page(tab), shell.held_why(tab), said)
                }
                _ => (site.clone(), None, Page::default(), None, None),
            };
            let acting = rings(tab, on, &steps, held.as_ref());
            let pictured = image.is_some();
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
                        let corners = Corners::all(px(PICTURE_RADIUS));
                        window
                            .paint_image(bounds, bounds, corners, image, 0, false)
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
            let blank = (!pictured).then(|| {
                div()
                    .absolute()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(PICTURE_EMPTY_GLYPH))
            });
            let picture = div()
                .relative()
                .w(px(RAIL_WIDTH))
                .h(px(high))
                .rounded(px(PICTURE_RADIUS))
                .bg(palette.plate(gpui::rgba(KEYS_FILL).a))
                .overflow_hidden()
                .children(blank)
                .child(paint)
                .when(acting, |picture| {
                    picture.child(
                        div()
                            .absolute()
                            .size_full()
                            .rounded(px(PICTURE_RADIUS))
                            .border(px(RING))
                            .border_color(rgb(SIGNAL)),
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
            let says = said.map_or("", PageWord::label);
            if self.said_logged.borrow_mut().insert(tab, says) != Some(says) {
                // check.sh reads this line.
                log!("browser: task picture {tab} says {says}");
                // Once, as it turns: a tile the user scrolls away again stays away.
                if said == Some(PageWord::NeedsYou) {
                    self.rail_scroll.scroll_to_item(index);
                }
            }
            let word = said.map(|said| {
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(SITE_GAP))
                    // A word that asks for the user is in the title's ink; the others are quiet.
                    .text_color(match said {
                        PageWord::NeedsYou | PageWord::Driving => palette.text(TITLE),
                        _ => palette.gray(STATUS),
                    })
                    .when(said == PageWord::Working, |word| {
                        word.child(div().size(px(RAIL_DOT)).rounded_full().bg(rgb(SIGNAL)))
                    })
                    .child(said.label())
            });
            let head = div()
                .h(px(RAIL_HEAD))
                .w(px(RAIL_WIDTH))
                .flex()
                .items_center()
                .gap(px(SITE_GAP))
                .text_size(px(text::SMALL))
                .child(icon(RAIL_FAVICON))
                .children(sound)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.text(TITLE))
                        .child(title),
                )
                .children(word);
            div()
                .id(("conversation-page", tab))
                .flex_none()
                .w(px(RAIL_WIDTH))
                .flex()
                .flex_col()
                .gap(px(SITE_GAP))
                .opacity(opacity)
                .cursor_pointer()
                .child(head)
                .child(picture)
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
                underline: style.link.then_some(underline),
                ..HighlightStyle::default()
            };
            (range.clone(), highlight)
        });
        let highlights: Vec<_> =
            gpui::combine_highlights(highlights.collect::<Vec<_>>(), fades).collect();
        let text = StyledText::new(inline.text).with_highlights(highlights);
        if inline.links.is_empty() {
            return text.into_any_element();
        }
        let (ranges, urls): (Vec<_>, Vec<SharedString>) = inline.links.into_iter().unzip();
        let shell = self.shell.clone();
        InteractiveText::new(id, text)
            .on_click(ranges, move |index, _, cx| {
                let Some(url) = urls.get(index).cloned() else {
                    return;
                };
                shell.update(cx, |shell, cx| shell.open_link(&url, cx)).ok();
            })
            .into_any_element()
    }

    /// A message of the user's, on its plate at the column's right: the first with the page they
    /// asked on, as a chip over its text. One `landing` holds its place, unseen, while what was
    /// sent goes there.
    fn you(
        &self,
        text: SharedString,
        page: Option<(SharedString, Option<Arc<Image>>)>,
        landing: bool,
        look: &Look,
    ) -> gpui::AnyElement {
        let palette = look.palette;
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
                    .max_w(gpui::relative(BUBBLE_WIDTH))
                    .pt(px(BUBBLE_TOP))
                    .pb(px(BUBBLE_BOTTOM))
                    .px(px(BUBBLE_X))
                    .rounded(px(RADIUS))
                    .bg(palette.plate(BUBBLE_FILL))
                    .children(chip)
                    .child(text)
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
        let mut line = summary_line(trace);
        line.chevron = Some(self.chevron(fold, open, look));
        line.rolled = self.motion.count(trace.key).cloned();
        self.crossfade(fold, &mut line, look);
        let summary = trace_line(look, line)
            .on_click(cx.listener(move |view, _, _, cx| view.toggle(fold, running, cx)));
        let mut nest = nest(look);
        for (index, item) in trace.items.iter().enumerate() {
            nest = match item {
                TraceItem::Step(step) => nest.children(self.step(step, true, look, cx)),
                // A helper's line has no Take Over, which is the one page's the task stands on.
                TraceItem::Helper { step, name, tab } => {
                    let page = tab.and_then(|tab| look.helper_page(tab));
                    match helper_row(step, name, page) {
                        HelperRow::Known(mut line) => {
                            line.shimmers = step.running.then(|| look.shimmer(self.made));
                            self.crossfade(Fold::Step(step.key), &mut line, look);
                            let line = trace_line(look, line).into_any_element();
                            nest.child(risen(line, self.motion.came(Key::Step(step.key)), look))
                        }
                        HelperRow::Step => {
                            nest.children(self.step_line(step, true, false, look, cx))
                        }
                        HelperRow::Nothing => nest,
                    }
                }
                TraceItem::Said(text) => {
                    let said = div()
                        .py(px((TRACE_LINE - TEXT_LINE) / 2.))
                        .text_color(look.palette.gray(SECONDARY))
                        .child(self.said("trace-said", (trace.key << 16) | index, text, &[], look));
                    nest.child(risen(
                        said.into_any_element(),
                        self.motion.came(Key::Aside(trace.key, index)),
                        look,
                    ))
                }
            };
        }
        div()
            .flex()
            .flex_col()
            .items_start()
            .mb(px(TRACE_BELOW))
            .child(summary)
            .children(self.unfolding(fold, open, nest.w_full(), look))
            .into_any_element()
    }

    /// The turn the user sent, until its trace or the agent's message takes its place: Thinking,
    /// shimmering as the step under way does, or why the agent couldn't start on it, in red.
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
        self.step_line(step, glyph, glyph, look, cx)
    }

    /// As `step`, which may be the `live` line beside the page or, as a helper's, never is.
    fn step_line(
        &self,
        step: &TraceStep,
        glyph: bool,
        live: bool,
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
        line.shimmers = step.running.then(|| look.shimmer(self.made));
        self.crossfade(Fold::Step(step.key), &mut line, look);
        line.note_alpha = line.note_alpha.min(self.motion.note(step.key, look.now));
        let mut line = trace_line(look, line);
        if opens {
            line = line.on_click(cx.listener(move |view, _, _, cx| view.toggle(fold, false, cx)));
        }
        // Beside the page, the step under way on it is the live line, with Take over at its end,
        // unless the user stands on the page, whose strip has it.
        let live = (self.shown.as_ref())
            .filter(|shown| {
                look.beside && !look.standing && live && step.running && shown.stoppable
            })
            .and_then(|shown| shown.on);
        let drawn = self.take_over_drawn.clone();
        let line = match live {
            None => line.into_any_element(),
            Some(tab) => div()
                .w_full()
                .flex()
                .items_center()
                .child(div().flex_1().min_w_0().flex().child(line))
                .child(
                    page_chip(
                        look.palette,
                        "conversation-take-over",
                        words::TAKE_OVER,
                        None,
                    )
                    .relative()
                    .child(
                        canvas(
                            move |bounds, _, _| drawn.borrow_mut().push(bounds),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    )
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.act(TaskAction::TakeOver(tab), cx)),
                    ),
                )
                .into_any_element(),
        };
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
                nest(look).children(
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
        std::iter::once(line).chain(sub).chain(opened).collect()
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
        // A session was asked in its agent's terminal, so it has no ask.
        let mut lines = Vec::new();
        if !shown.asked.is_empty() {
            lines.push(self.you(shown.asked.clone(), page, false, look));
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
        for (index, line) in shown.lines.iter().enumerate() {
            // The line in the reply field's place is drawn there.
            if matches!(line, Line::Ilo(line) if ilo::in_field(&line.kind)) {
                continue;
            }
            if let Some(answered) = answered.filter(|answered| answered.at == index) {
                lines.push(self.answered(answered, look));
            }
            let line = match line {
                Line::You(text) => self.you(text.clone(), None, landing == Some(index), look),
                Line::Said(text) => div()
                    .mb(px(BLOCK_GAP))
                    .text_color(palette.text(TITLE))
                    .child(self.said("said", index, text, self.motion.runs(index), look))
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
            false => (REPLY_FILL, RADIUS, REPLY_HEIGHT, REPLY_INSET),
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
                .bg(look.palette.plate(between(fill, BUBBLE_FILL)))
                .pt(px(between((height - TEXT_LINE) / 2., BUBBLE_TOP)))
                .px(px(between(inset, BUBBLE_X)))
                .text_size(px(text::BODY))
                .line_height(px(TEXT_LINE))
                .text_color(look.palette.text(TITLE))
                .child(landing.text.clone())
                .into_any_element(),
        )
    }

    /// The reply field, the TextInput, so IME and the field's keys are a text field's. A session's
    /// conversation is in its agent's terminal, which a quiet line in its place says, on the text's
    /// column as it has no plate.
    /// With Settings › Agents' Tasks from the command bar off, the line says so in the field's
    /// place, as the task takes no more.
    fn reply(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let tasks_off = crate::agents::global(cx).is_some_and(|agents| !agents.read(cx).tasks_on());
        let quiet = match (self.replies, tasks_off) {
            (false, _) => {
                let name = self
                    .shown
                    .as_ref()
                    .map(|shown| shown.name.clone())
                    .unwrap_or_default();
                Some(format!("Talk to {name} where you started it."))
            }
            (true, true) => Some("Tasks are off in Settings › Agents.".to_string()),
            (true, false) => None,
        };
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
        // The key that sends it, in the command bar's hint key, as a button with a key shows it.
        let send = div()
            .flex_none()
            .h(px(PAGE_CHIP_HEIGHT))
            .px(px(HINT_KEY_X))
            .flex()
            .items_center()
            .rounded(px(CHIP_RADIUS))
            .bg(palette.plate(BAR_CHIP))
            .border_1()
            .border_color(palette.plate(HINT_KEY_RIM))
            .text_size(px(text::SMALL))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.text(KEY_COLOR))
            .child(super::key_name("enter", super::OFF_THE_MAC));
        div()
            .id("conversation-reply")
            .relative()
            .flex_none()
            .h(px(REPLY_HEIGHT))
            .pl(px(REPLY_INSET))
            .pr(px(REPLY_KEY_END))
            .flex()
            .items_center()
            .gap(px(REPLY_KEY_END))
            .rounded(px(RADIUS))
            .bg(palette.plate(REPLY_FILL))
            .border_1()
            .border_color(palette.plate(REPLY_RIM))
            .font_weight(FontWeight::NORMAL)
            .on_click(cx.listener(move |_, _, window, cx| {
                window.focus(&reply.focus_handle(cx), cx);
            }))
            .child(div().flex_1().min_w_0().child(self.reply.clone()))
            .child(send)
            .child(
                canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
    }

    /// The composer's row under the reply field: the task's access on a chip,
    /// which opens its menu (#4094), and Stop at the row's end.
    fn composer(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Div {
        let palette = look.palette;
        let tier = self
            .shown
            .as_ref()
            .map(|shown| shown.tier)
            .unwrap_or_default();
        let drawn = self.level_drawn.clone();
        let chip = page_chip(palette, "conversation-tier", tier.label(), None)
            .relative()
            .child(
                svg()
                    .flex_none()
                    .ml(px(COMPOSER_CHEVRON_GAP - KEY_GAP))
                    .path("icons/chevron-down.svg")
                    .size(px(COMPOSER_CHEVRON))
                    .text_color(palette.glyph(CHIP_KEY)),
            )
            .child(
                canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            );
        div()
            .mt(px(COMPOSER_TOP))
            .flex()
            .items_center()
            .child(chip)
            .child(div().flex_1())
            .children(self.stop(look, cx))
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
        let (own, name) = words::head(&line.kind, agent);
        // ilo's mark is the first word of what it says, in the display face, so its words wrap
        // under it: an ilo line has one left edge, its mark's.
        let mark = 0..words::MARK.len();
        let own = format!("{} {own}", words::MARK);
        let marked = HighlightStyle {
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        };
        // A page's words or a site are cut short, in the title's ink, so they read as a name.
        let name = name.map(|name| clip(&name, QUOTED_CHARS as usize));
        let (said, highlights) = match &name {
            Some(name) => {
                let (mut said, quoted) = words::with_name(&line.kind, &own, name);
                // What its sentence says after the name.
                said.extend(words::tail(&line.kind, agent));
                let style = HighlightStyle {
                    color: Some(palette.text(TITLE)),
                    ..Default::default()
                };
                (said, vec![(mark.clone(), marked), (quoted, style)])
            }
            None => (own, vec![(mark.clone(), marked)]),
        };
        let count = match &line.kind {
            IloKind::Stopped { count, .. } => words::count(*count),
            _ => None,
        };
        let glyph = match &line.kind {
            IloKind::Stopped { .. } | IloKind::Helpers { .. } => "icons/ban.svg",
            IloKind::Yours { .. } => "icons/user.svg",
            IloKind::TabBack { .. } => "icons/undo-2.svg",
            IloKind::SignedIn { .. } => "icons/key-round.svg",
            IloKind::TierChanged { .. } | IloKind::KeptAt { .. } => "icons/shield.svg",
            IloKind::StartedWithout {
                left: ilo::LeftOut::Folder(_),
            } => "icons/folder.svg",
            IloKind::StartedWithout { .. } => "icons/file.svg",
            IloKind::NotRestarted { .. } => "icons/ban.svg",
            IloKind::Revoked { .. } | IloKind::FolderGone { .. } => "icons/folder.svg",
            IloKind::NotCopied { .. } => "icons/file.svg",
            IloKind::Resumed { .. } => "icons/play.svg",
            IloKind::KeptHere => "icons/save.svg",
            IloKind::Gone
            | IloKind::NotKept
            | IloKind::KeepsNothing
            | IloKind::NoProfile
            | IloKind::NoSpace => "icons/message-circle.svg",
        };
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
        let body = words::body(&line.kind, name.as_deref().unwrap_or("")).map(|body| {
            div()
                .pl(under)
                .mt(px(TEXT_TOP))
                .line_height(px(ILO_TEXT_LINE))
                .text_color(ink)
                .child(body)
        });
        let link = words::link(&line.kind).map(|words| {
            let drawn = self.ilo_links.clone();
            let key = line.key;
            div()
                .id(SharedString::from(format!("ilo-{key}-link")))
                .relative()
                .pl(under)
                .mt(px(NOTE_TOP))
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
        let buttons = ilo::drawn(line, self.beside(cx));
        let note = words::note(&buttons).map(|note| {
            div()
                .pl(under)
                .mt(px(NOTE_TOP))
                .text_size(px(text::SMALL))
                .text_color(palette.gray(NOTE_INK))
                .child(note)
        });
        let row = (!buttons.is_empty()).then(|| {
            div()
                .pl(under)
                .mt(px(BUTTONS_TOP))
                .flex()
                .flex_wrap()
                .gap(px(BUTTONS_GAP))
                .children(
                    (buttons.into_iter().enumerate())
                        .map(|(at, button)| self.ilo_button(line, (at, button), look, cx)),
                )
        });
        div()
            .flex()
            .flex_col()
            .items_start()
            .mb(px(ILO_BELOW))
            .child(head)
            .children(body)
            .children(link)
            .children(row)
            .children(note)
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
        page_chip(look.palette, id, words::button(button), None)
            .relative()
            .child(
                canvas(
                    move |bounds, _, _| {
                        let mut drawn = drawn.borrow_mut();
                        if drawn.get(&key).is_none_or(|(was, _)| *was != bounds) {
                            drawn.insert(key, (bounds, Instant::now()));
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
            IloButton::AnySite => Some(TaskAction::AnySite),
            IloButton::HandBack => tab.map(TaskAction::HandBack),
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

/// What a trace, or a turn's same step on several tabs, opens to: its lines, a step in, along a
/// rule under the glyph above them.
fn nest(look: &Look) -> gpui::Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .items_start()
        .pl(px(TRACE_INDENT))
        .child(
            div()
                .absolute()
                .left(px(TRACE_RULE_X))
                .top_0()
                .bottom_0()
                .w(px(TRACE_RULE))
                .bg(look.palette.plate(TRACE_RULE_FILL)),
        )
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
            alpha: 1.,
            note_alpha: 1.,
            chevron_alpha: 1.,
            rolled: None,
            joined: false,
        }
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
fn summary_line(trace: &Trace) -> TraceLine {
    let (title, counted) = trace.summary();
    TraceLine {
        joined: true,
        ..TraceLine::new(Fold::Trace(trace.key), title, counted)
    }
}

/// Which line a `TraceLine` is: a trace's, which folds, or one of ilo's.
#[derive(Clone, Copy)]
enum LineKey {
    Fold(Fold),
    Ilo(usize),
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
        alpha,
        note_alpha,
        chevron_alpha,
        rolled,
        joined,
    } = line;
    let [lead, count, rest] = parts;
    let space = if joined { " " } else { "" };
    let (id, group): (ElementId, SharedString) = match key {
        LineKey::Ilo(key) => (("ilo", key).into(), format!("ilo-{key}").into()),
        LineKey::Fold(Fold::Trace(key)) => (("trace", key).into(), format!("trace-{key}").into()),
        LineKey::Fold(Fold::Step(key)) => (
            ("trace-step", key).into(),
            format!("trace-step-{key}").into(),
        ),
        LineKey::Fold(Fold::Group(key)) => (
            ("trace-group", key).into(),
            format!("trace-group-{key}").into(),
        ),
    };
    // A failed line is red, its glyph, its words and why, under the pointer too.
    let red: Hsla = rgb(NOT_FOUND).into();
    let note = if failed {
        red
    } else {
        palette.gray(TRACE_NOTE)
    };
    let ilos = mark.is_some();
    let words = match (failed, shimmers) {
        (true, _) => red,
        (false, _) if ilos => palette.text(ILO_INK),
        (false, Some(None)) => palette.text(TITLE),
        // At the shimmer's peak, which each part fades down from.
        (false, Some(Some(_))) => palette.lit(TRACE_TITLE, SHIMMER, 1.),
        (false, None) => palette.text(TRACE_TITLE),
    };
    let glyph = glyph.map(|glyph| {
        let (color, size) = match (failed, ilos) {
            (true, _) => (red, TRACE_GLYPH),
            (false, true) => (palette.glyph(ILO_INK), ILO_GLYPH),
            (false, false) => (palette.glyph(TRACE_GLYPH_ALPHA), TRACE_GLYPH),
        };
        svg()
            .flex_none()
            .path(glyph)
            .size(px(size))
            .text_color(color)
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
    let opens = chevron.is_some();
    let chevron = chevron.map(|open| {
        svg()
            .flex_none()
            .path("icons/chevron-down.svg")
            .size(px(TRACE_CHEVRON))
            .text_color(palette.glyph(TRACE_NOTE))
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
        .map(|title| match ilos {
            true => title
                .relative()
                .py(px((TRACE_LINE - ILO_TEXT_LINE) / 2.))
                .line_height(px(ILO_TEXT_LINE)),
            false => title.truncate(),
        })
        .opacity(alpha)
        .text_color(words)
        // A shimmering title is lit by its shimmer, under the pointer too.
        .when(
            opens && !failed && !matches!(shimmers, Some(Some(_))),
            |title| title.group_hover(hover, move |style| style.text_color(palette.text(TITLE))),
        )
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
        .group(group)
        .flex_none()
        .max_w_full()
        .flex()
        .gap(px(TRACE_GAP))
        .rounded(px(NAV_RADIUS))
        .map(|line| match ilos {
            true => line.min_h(px(TRACE_LINE)).items_start(),
            // Its plate hangs outside its text, which ilo's lines, which don't open, have none of.
            false => line
                .h(px(TRACE_LINE))
                .ml(px(-TRACE_HANG))
                .px(px(TRACE_HANG))
                .items_center()
                .whitespace_nowrap(),
        })
        .when(opens, |line| {
            line.cursor_pointer()
                .hover(move |style| style.bg(palette.plate(TRACE_HOVER)))
        })
        .children(glyph.map(|glyph| first(glyph.into_any_element())))
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
        .children(chevron)
}

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
    /// A page watched is going back into its picture over it, which hides what's under it.
    behind: bool,
    /// The conversation stands beside the page it watches (`Shell::talk_beside`).
    beside: bool,
    /// The user stands on one of its pages, the conversation in a panel beside it.
    standing: bool,
}

/// What a helper's line shows of its page: its title, which a helper its task gave no name is
/// known by, its favicon, and the word its row in the sidebar ends with.
struct HelperPage {
    tab: u64,
    title: SharedString,
    favicon: Option<Arc<Image>>,
    word: &'static str,
}

impl Look {
    fn helper_page(&self, tab: u64) -> Option<&HelperPage> {
        self.helper_pages.iter().find(|page| page.tab == tab)
    }

    /// How far through its pass the shimmer is, or None under Reduce Motion.
    fn shimmer(&self, since: Instant) -> Option<f32> {
        (!self.still).then(|| ((self.now - since).as_secs_f32() / SHIMMER_PERIOD).fract())
    }
}

impl Shell {
    /// The favicon of an open tab at `url`, if one has it.
    fn favicon_at(&self, url: &str) -> Option<Arc<Image>> {
        (self.tabs.values())
            .find(|tab| tab.page.url.as_ref() == url)
            .and_then(|tab| tab.favicon.clone())
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
        let Some(look) = self.shell.upgrade().map(|shell| {
            let shell = shell.read(cx);
            let asked_favicon = asked.and_then(|url| shell.favicon_at(&url));
            Look {
                palette: shell.palette,
                now,
                still,
                asked_favicon,
                helper_pages: shell.helper_pages(&self.agent, helpers),
                behind: shell.task_page_going_back(),
                beside: shell.talk_beside,
                standing: shell.task_standing(),
            }
        }) else {
            return div().into_any_element();
        };
        // The reply's placeholder follows the appearance, as the lines drawn in `look` do (#1659).
        let placeholder = look.palette.gray(FORM_PLACEHOLDER);
        (self.reply).update(cx, |reply, cx| reply.set_placeholder_color(placeholder, cx));
        self.settle_folds(now, still);
        // A shorter or narrower window keeps the newest line in view while the user reads at the
        // end, as the scroll keeps its distance from the top.
        let viewport = window.viewport_size();
        if std::mem::replace(&mut self.viewport, viewport) != viewport && self.at_end() {
            self.scroll.scroll_to_bottom();
        }
        self.level_drawn.set(None);
        self.take_over_drawn.borrow_mut().clear();
        let busy = self.motion.settle(now);
        self.follow_on(now);
        // What changes once it has shown moves.
        self.motion.live = true;
        // What moves does only while it shows: what opens, folds or lands on the display's frames,
        // and the shimmer, which moves all the while a step waits on the user, on the loading dots'
        // clock, as each frame draws the whole window again (#1700).
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
        let wide =
            (self.drawn_at.get()).map_or(window.viewport_size().width, |area| area.size.width);
        let across = pages && f32::from(wide) < RAIL_BREAKPOINT;
        let middle = shut(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .justify_center()
                .mb(px(REPLY_GAP))
                .child(
                    div()
                        .size_full()
                        .max_w(px(COLUMN + 2. * TRACE_HANG))
                        .child(lines),
                ),
        );
        let column = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .text_size(px(text::BODY))
            .line_height(px(TEXT_LINE))
            .text_color(palette.text(TITLE))
            .child(shut(on_column(self.header(&look, cx).into_any_element())))
            .children(across.then(|| {
                div()
                    .flex_none()
                    .w_full()
                    .mb(px(RAIL_GAP))
                    .child(self.rail(true, &look, cx))
            }))
            .child(middle)
            .child(shut(on_column(self.reply(&look, cx).into_any_element())))
            // With ilo's line and Start Again in the field's place there is no level to set, so
            // the row goes with the field (#4105).
            .children(
                (self.replies && self.in_field().is_none())
                    .then(|| shut(on_column(self.composer(&look, cx).into_any_element()))),
            );
        let rail = (pages && !across).then(|| {
            div()
                .flex_none()
                .w(px(RAIL_WIDTH))
                .h_full()
                .ml(px(RAIL_GAP))
                .child(self.rail(false, &look, cx))
        });
        if !pages {
            self.drawn.borrow_mut().clear();
        }
        let column = div().size_full().flex().child(column).children(rail);
        let drawn = self.drawn_at.clone();
        div()
            .id("conversation")
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|view, _: &SendReply, window, cx| view.send_reply(window, cx)))
            .on_action(cx.listener(Self::ilo_press))
            .on_action(cx.listener(Self::ilo_next))
            .on_action(cx.listener(Self::ilo_back))
            .on_action(cx.listener(Self::ilo_leave))
            .size_full()
            .bg(palette.page)
            .px(px(INSET))
            .pt(px(TOP))
            .pb(px(REPLY_BOTTOM))
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
            .into_any_element()
    }
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

/// Whether `tab`'s tile is ringed: ilo acts in its page now, and the user doesn't hold it (`held`).
/// A task on one page acts in the page it's `on`; one on several, as with helpers, `steps` having
/// each one's own step, acts in every page that has one, so more than one tile can be ringed.
pub(super) fn rings(
    tab: u64,
    on: Option<u64>,
    steps: &[(u64, SharedString)],
    held: Option<&SharedString>,
) -> bool {
    let acts = match steps.is_empty() {
        true => on == Some(tab),
        false => steps.iter().any(|(each, _)| *each == tab),
    };
    acts && held.is_none()
}

#[cfg(test)]
mod tests;
