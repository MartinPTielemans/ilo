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
use web_time::Instant;

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
    browser::Browser,
    picture::{PagePicture, next_request},
    row_spring,
};
use crate::agents::Status;
use crate::agents::task::{
    ArchivedTask, Card, Conversation, Dot, Line, Opens, TaskAction, Trace, TraceItem, TraceStep,
};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::text_input::TextInput;
use crate::tokens::command_bar::{CHIP as BAR_CHIP, HINT_KEY_RIM, HINT_KEY_X};
use crate::tokens::find_bar::NOT_FOUND;
use crate::tokens::shell::FORM_PLACEHOLDER;
use crate::tokens::shell::agent_pointer::{COLOR as SIGNAL, RING};
use crate::tokens::shell::conversation::motion::{CHEVRON, LAND, PICK_FADE, RISE, STEPS_FADE};
use crate::tokens::shell::conversation::{
    BLOCK_GAP, BUBBLE_BOTTOM, BUBBLE_FILL, BUBBLE_TOP, BUBBLE_WIDTH, BUBBLE_X, CHIP_KEY,
    CHIPS_BELOW, CHIPS_TOP, CODE_FILL, COLUMN, HEADER_BOTTOM, INSET, ITEM_GAP, LINES_BELOW,
    LINK_LINE, LINK_UNDERLINE, LIST_MARKER, META_GAP, META_LINE, OPEN_BELOW, OPEN_TOP,
    PAGE_CHIP_BELOW, PAGE_CHIP_FAVICON, PAGE_CHIP_FILL, PAGE_CHIP_GAP, PAGE_CHIP_HANG,
    PAGE_CHIP_HEIGHT, PAGE_CHIP_X, PARAGRAPH_GAP, PICTURE_EMPTY_GAP, PICTURE_EMPTY_GLYPH,
    PICTURE_GAP, PICTURE_HEIGHT, PICTURE_RADIUS, PICTURE_WIDTH, PICTURES_COLUMNS, PICTURES_ROW_GAP,
    RADIUS, REPLY_BOTTOM, REPLY_FILL, REPLY_GAP, REPLY_HEIGHT, REPLY_INSET, REPLY_KEY_END,
    REPLY_RIM, SHIMMER, SHIMMER_BAND, SHIMMER_PERIOD, SITE, SITE_GAP, TEXT_LINE, TITLE_LINE, TOP,
    TRACE_BELOW, TRACE_CHEVRON, TRACE_GAP, TRACE_GLYPH, TRACE_GLYPH_ALPHA, TRACE_HANG, TRACE_HOVER,
    TRACE_INDENT, TRACE_LINE, TRACE_NOTE, TRACE_PICTURE_HEIGHT, TRACE_PICTURE_RADIUS,
    TRACE_PICTURE_WIDTH, TRACE_RULE, TRACE_RULE_FILL, TRACE_RULE_X, TRACE_TITLE, VIEWS_GAP,
    VIEWS_INSET, VIEWS_TRACK,
};
use crate::tokens::shell::empty::KEYS_FILL;
use crate::tokens::shell::task::STATUS;
use crate::tokens::shell::task::motion::{CHIPS, CHIPS_RISE, HEIGHT};
use crate::tokens::text;
use crate::tokens::toast::{
    CHIP_FILL, CHIP_GAP, CHIP_HEIGHT, CHIP_RADIUS, CHIP_X, KEY_COLOR, KEY_GAP,
};

actions!(conversation, [SendReply, StopTask]);

mod markdown;
mod motion;

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
        .bg(palette.plate(CHIP_FILL))
        .hover(move |style| style.bg(palette.state(CHIP_FILL, STATE_HOVERED)))
        .active(move |style| style.bg(palette.state(CHIP_FILL, STATE_PRESSED)))
        .whitespace_nowrap()
        .text_size(px(text::BODY))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.text(TITLE))
        .child(words.into())
        .children(key.map(|key| div().text_color(key_color).child(key)))
}

/// Sends the windows' `shells` what the Agents show as they change: their status, the task cards,
/// the tasks in the Archive and, to a window showing a task's conversation, that conversation, as
/// `conversation` makes it for the task's agent. Each is made before any shell changes, since making one reads its pages
/// from a shell (the core's `Browser::tab`), which can't be read while it's being changed.
pub fn send_agents(
    status: Vec<Status>,
    cards: Vec<Card>,
    archived: Vec<ArchivedTask>,
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
            shell.set_task_cards(cards.clone(), cx);
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
    /// What each picture's status line said when that was logged last.
    said_logged: RefCell<HashMap<u64, SharedString>>,
    /// Where Stop was drawn when that was logged last.
    stop_logged: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Where Pages N was drawn when that was logged last.
    pages_logged: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Its pages changed while one was watched over it, so they're pictured again once it shows.
    stale: bool,
    /// Whether its step under way shimmered as it last drew, which check.sh hears of.
    shimmering: bool,
    /// Its lines, which keep the newest in view as they come while the user is at the end.
    scroll: ScrollHandle,
    /// The window's size as last drawn: a change keeps the lines at their end if they were.
    viewport: Size<Pixels>,
    /// Pages rather than Talk: as the user last picked, else as the task first showed.
    pages: Option<bool>,
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

    /// The task whose conversation is kept for `space`'s area.
    pub(super) fn talk_in(&self, space: u64) -> Option<SharedString> {
        self.talk
            .as_ref()
            .filter(|talk| talk.space == space)
            .map(|talk| talk.agent.clone())
    }

    /// The card's click: its conversation in the page area, or, shown already, the Space's tab
    /// again.
    /// A click on the notification a task's need posted (needs_you): its card's click, which never
    /// puts the task away if it shows already, as the user came to it.
    /// Esc on Pages: back to Talk, and whether it was there.
    pub(super) fn talk_back_to_talk(&mut self, cx: &mut Context<Self>) -> bool {
        let view = self.talk.as_ref().map(|talk| talk.view.clone());
        view.is_some_and(|view| view.update(cx, |view, cx| view.back_to_talk(cx)))
    }

    /// The task's conversation shows Pages, its grid, rather than Talk.
    pub fn talk_on_pages(&self, cx: &gpui::App) -> bool {
        (self.talk.as_ref()).is_some_and(|talk| talk.view.read(cx).on_pages())
    }

    pub fn show_needed_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        if self.talk_agent().as_ref() != Some(&agent) {
            self.show_task(agent, cx);
        }
    }

    pub fn show_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
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
                let placeholder = self.palette.text(FORM_PLACEHOLDER);
                let reply = cx.new(|cx| {
                    TextInput::new(format!("Reply to {name}…"), cx).placeholder_color(placeholder)
                });
                let shell = cx.weak_entity();
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
                    pages_logged: Rc::default(),
                    stale: false,
                    shimmering: false,
                    scroll: ScrollHandle::new(),
                    viewport: Size::default(),
                    pages: None,
                    opened: HashMap::new(),
                    was_open: HashMap::new(),
                    folding: HashMap::new(),
                    heights: Rc::default(),
                    made: Instant::now(),
                    motion: Motion::default(),
                    reply_drawn: Rc::default(),
                    answers_drawn: Rc::default(),
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
        let (reply, replies) = (view.reply.focus_handle(cx), view.replies);
        if self.watched_task_page().is_some() {
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
            self.motion.still = cx.reduce_motion();
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
                self.motion.live && !cx.reduce_motion(),
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
            // It opens on Pages when the task has two pages or more, on Talk with fewer.
            if self.pages.is_none() {
                let pages = conversation.tabs.len() >= 2;
                // check.sh reads this line.
                log!(
                    "browser: task {} opens on {}",
                    self.agent,
                    if pages { "pages" } else { "talk" }
                );
                self.pages = Some(pages);
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
        if let Some(field) = self.reply_drawn.get().filter(|_| !cx.reduce_motion()) {
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
        match follows(true, false, true, self.motion.live && !cx.reduce_motion()) {
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
        if let Some(from) = from.filter(|_| !cx.reduce_motion()) {
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
        self.act(TaskAction::Answer(answer), cx);
    }

    fn act(&self, action: TaskAction, cx: &mut Context<Self>) {
        let agent = self.agent.clone();
        self.shell
            .update(cx, |shell, cx| shell.browser.task(agent, action, cx))
            .ok();
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
                            TraceItem::Step(step) => Some(step),
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

    /// The task's title and, under it, what it runs on and where it is, with Stop ⌘. at its right
    /// while it works or waits on the user.
    fn header(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Div {
        let shown = self.shown.as_ref();
        let palette = look.palette;
        let title = shown.map(|shown| shown.title.clone()).unwrap_or_default();
        let about = shown.map(|shown| shown.about.clone()).unwrap_or_default();
        // Where it is rolls over as it changes, after what it runs on.
        let status = motion::status(&about).to_string();
        let runs = about[..about.len() - status.len()].to_string();
        // How far it has got follows, as its steps come, without rolling: the status rolls only as
        // the work's state changes.
        let progress = shown
            .map(|shown| shown.progress.clone())
            .filter(|progress| !progress.is_empty());
        let about = div()
            .flex()
            .child(div().min_w_0().truncate().child(runs))
            .child(rolled(status.into(), self.motion.status(), META_LINE, look))
            .children(progress.map(|progress| {
                div()
                    .min_w_0()
                    .truncate()
                    .child(format!("\u{a0}· {progress}"))
            }));
        // Stop comes and goes with the work it stops.
        let stoppable = shown.is_some_and(|shown| shown.stoppable);
        let opacity = self.motion.stop(stoppable, look.now);
        let stop = (opacity > 0.).then(|| {
            let chip =
                page_chip(palette, "conversation-stop", "Stop", Some(stop_keys())).opacity(opacity);
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
        });
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
                    .text_color(palette.text(STATUS))
                    .child(about),
            );
        // Talk and Pages N, once it has a page: one switch, its track a chip's shape, the view
        // shown lifted on the selected tab's plate, so it reads as a switch and not as Stop's kind.
        let count = shown.map_or(0, |shown| shown.tabs.len());
        let views = (count > 0).then(|| {
            let pages = self.on_pages();
            let chip = |id: &'static str, words: SharedString, on: bool, to: bool| {
                div()
                    .id(id)
                    .flex_none()
                    .h_full()
                    .px(px(CHIP_X - VIEWS_INSET))
                    .flex()
                    .items_center()
                    .rounded(px(CHIP_RADIUS - VIEWS_INSET))
                    .whitespace_nowrap()
                    .text_size(px(text::BODY))
                    .font_weight(FontWeight::MEDIUM)
                    .when(on, |chip| {
                        chip.bg(palette.selected(0.))
                            .shadow(palette.selected_shadow(1.))
                            .text_color(palette.text(TITLE))
                    })
                    .when(!on, |chip| {
                        chip.text_color(palette.text(STATUS))
                            .hover(move |style| style.bg(palette.plate(STATE_HOVERED)))
                            .active(move |style| style.bg(palette.plate(STATE_PRESSED)))
                    })
                    .child(words)
                    .on_click(cx.listener(move |view, _, _, cx| view.show_pages(to, cx)))
            };
            div()
                .flex_none()
                .h(px(CHIP_HEIGHT))
                .p(px(VIEWS_INSET))
                .flex()
                .gap(px(VIEWS_GAP))
                .rounded(px(CHIP_RADIUS))
                .bg(palette.plate(VIEWS_TRACK))
                .child(chip("conversation-talk", "Talk".into(), !pages, false))
                .child({
                    let logged = self.pages_logged.clone();
                    let at = canvas(
                        move |bounds, _, _| {
                            if logged.replace(Some(bounds)) != Some(bounds) {
                                // check.sh reads this line, to click it.
                                log!(
                                    "browser: task pages at {:.0} {:.0} {:.0} {:.0}",
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
                    let words = format!("Pages {count}").into();
                    chip("conversation-pages-chip", words, pages, true)
                        .relative()
                        .child(at)
                })
        });
        div()
            .flex_none()
            .mb(px(HEADER_BOTTOM))
            .flex()
            .items_center()
            .gap(px(BUBBLE_X))
            .child(words)
            .children(views)
            .children(stop)
    }

    /// Pages: its pages' pictures in a grid, up to three across as the area holds them, each over
    /// its site, the agent's page ringed Signal while it works, with its step, and one the user
    /// holds from the agent with why: "You're driving", or "Needs you · Sign in" (#1080). A pick zooms the
    /// page out of its picture. The pictures are taken when the agent acts (`set_conversation`).
    fn grid(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        self.drawn.borrow_mut().clear();
        let tabs = self
            .shown
            .as_ref()
            .map(|shown| shown.tabs.clone())
            .unwrap_or_default();
        let on = self.shown.as_ref().and_then(|shown| shown.on);
        let step = self.running_step();
        // The area's own width as it last drew, the grid taking its inset but a gap each side.
        let area = (self.drawn_at.get()).map_or(self.viewport.width, |bounds| bounds.size.width);
        let room = f32::from(area) - 2. * PICTURE_GAP;
        let columns = (((room + PICTURE_GAP) / (PICTURE_WIDTH + PICTURE_GAP)).floor())
            .clamp(1., PICTURES_COLUMNS) as usize;
        let tiles: Vec<_> = (tabs.iter().enumerate())
            .map(|(index, (tab, site))| {
                let tab = *tab;
                let opacity = match self.motion.came(Key::Picture(tab)) {
                    Some(since) if !look.still => eased(since, look.now, RISE),
                    _ => 1.,
                };
                let image = (self.pictures.iter().find(|(each, ..)| *each == tab))
                    .and_then(|(_, _, picture)| picture.as_ref().map(PagePicture::image));
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
                let working = on == Some(tab);
                // Until its picture comes, the tile says it has none yet, rather than show nothing,
                // and once the core had none to give, that the next action takes one.
                let empty = (!pictured).then(|| {
                    let pending = !self.unpictured.contains(&tab);
                    let name = self.shown.as_ref().map(|shown| shown.name.clone());
                    div()
                        .absolute()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(PICTURE_EMPTY_GAP))
                        .text_size(px(text::SMALL))
                        .text_color(look.palette.text(STATUS))
                        .child(
                            svg()
                                .path("icons/globe.svg")
                                .size(px(PICTURE_EMPTY_GLYPH))
                                .text_color(look.palette.text(SECONDARY)),
                        )
                        .child("No picture yet")
                        .children((!pending).then(|| {
                            let name = name.unwrap_or_else(|| "the agent".into());
                            format!("Taken when {name} next acts here")
                        }))
                });
                let picture = div()
                    .relative()
                    .w(px(PICTURE_WIDTH))
                    .h(px(PICTURE_HEIGHT))
                    .rounded(px(PICTURE_RADIUS))
                    .bg(look.palette.plate(gpui::rgba(KEYS_FILL).a))
                    .overflow_hidden()
                    .children(empty)
                    .child(paint)
                    .when(working, |picture| {
                        picture.child(
                            div()
                                .absolute()
                                .size_full()
                                .rounded(px(PICTURE_RADIUS))
                                .border(px(RING))
                                .border_color(rgb(SIGNAL)),
                        )
                    });
                let line = |words: SharedString, color: Hsla| {
                    div()
                        .w(px(PICTURE_WIDTH))
                        .h(px(SITE))
                        .truncate()
                        .text_size(px(text::SMALL))
                        .text_color(color)
                        .child(words)
                };
                // While its page plays, its speaker before its site, crossed while it's muted, as an
                // agent's tabs start: a click on it unmutes or mutes it, as a row's does (#1080).
                let sound = (self.shell.upgrade())
                    .map(|shell| shell.read(cx).page(tab))
                    .filter(|page| page.audible)
                    .map(|page| {
                        let shell = self.shell.clone();
                        div()
                            .id(("conversation-page-sound", tab))
                            .flex_none()
                            .mr(px(SITE_GAP))
                            .child(
                                svg()
                                    .path(super::rows::speaker_glyph(page.muted))
                                    .size(px(text::SMALL + 1.))
                                    .text_color(look.palette.text(SECONDARY)),
                            )
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                shell
                                    .update(cx, |shell, cx| shell.toggle_mute(tab, cx))
                                    .ok();
                            })
                    });
                let caption = div()
                    .mt(px(SITE_GAP))
                    .flex()
                    .items_center()
                    .children(sound)
                    .child(line(site.clone(), look.palette.text(TITLE)));
                // A page the user holds says why under its site, in the title's ink, where the
                // agent's page has its step: the bar over the page says the same.
                let held = (self.shell.upgrade()).and_then(|shell| shell.read(cx).held_why(tab));
                let status = picture_status(held.as_ref(), step.clone().filter(|_| working));
                if let Some(words) = &status {
                    let mut said = self.said_logged.borrow_mut();
                    if said.get(&tab) != Some(words) {
                        // check.sh reads this line.
                        log!("browser: task picture {tab} says {words}");
                        said.insert(tab, words.clone());
                    }
                }
                let status = status.map(|words| match held {
                    Some(_) => {
                        line(words, look.palette.text(TITLE)).font_weight(FontWeight::MEDIUM)
                    }
                    None => line(words, rgb(SIGNAL).into()),
                });
                div()
                    .id(("conversation-page", tab))
                    .flex_none()
                    .w(px(PICTURE_WIDTH))
                    .when(index % columns > 0, |tile| tile.ml(px(PICTURE_GAP)))
                    .mb(px(PICTURES_ROW_GAP))
                    .opacity(opacity)
                    .cursor_pointer()
                    .child(picture)
                    .child(caption)
                    .children(status)
                    .on_click(cx.listener(move |view, _, _, cx| view.watch(tab, cx)))
            })
            .collect();
        let width = columns as f32 * PICTURE_WIDTH + (columns as f32 - 1.) * PICTURE_GAP;
        div()
            .id("conversation-pages")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .flex()
            .justify_center()
            .child(
                div()
                    .w(px(width))
                    .flex()
                    .flex_wrap()
                    .content_start()
                    .children(tiles),
            )
    }

    /// The step under way, as its trace names it, for the agent's page's picture.
    fn running_step(&self) -> Option<SharedString> {
        let shown = self.shown.as_ref()?;
        shown.lines.iter().rev().find_map(|line| match line {
            Line::Trace(trace) => (trace.items.iter().rev()).find_map(|item| match item {
                TraceItem::Step(step) if step.running => Some(step.title.clone()),
                _ => None,
            }),
            Line::Step(step) if step.running => Some(step.title.clone()),
            _ => None,
        })
    }

    /// Whether Pages shows, rather than Talk: as the user picked, else Pages while the task had
    /// two pages or more as it opened.
    fn on_pages(&self) -> bool {
        self.pages.unwrap_or(false)
    }

    /// The header's Talk or Pages.
    fn show_pages(&mut self, pages: bool, cx: &mut Context<Self>) {
        if self.pages != Some(pages) {
            // check.sh reads this line.
            log!(
                "browser: task {} shows {}",
                self.agent,
                if pages { "pages" } else { "talk" }
            );
            self.pages = Some(pages);
            cx.notify();
        }
    }

    /// Esc on Pages goes back to Talk, and says whether it did.
    pub(super) fn back_to_talk(&mut self, cx: &mut Context<Self>) -> bool {
        let on = self.on_pages();
        if on {
            self.show_pages(false, cx);
        }
        on
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
        let (title, steps) = trace.summary();
        let running = trace.running;
        let mut line = TraceLine::new(fold, title, steps);
        line.chevron = Some(self.chevron(fold, open, look));
        line.rolled = self.motion.count(trace.key).cloned();
        self.crossfade(fold, &mut line, look);
        let summary = trace_line(look, line)
            .on_click(cx.listener(move |view, _, _, cx| view.toggle(fold, running, cx)));
        let mut nest = nest(look);
        for (index, item) in trace.items.iter().enumerate() {
            nest = match item {
                TraceItem::Step(step) => nest.children(self.step(step, true, look, cx)),
                TraceItem::Said(text) => {
                    let said = div()
                        .py(px((TRACE_LINE - TEXT_LINE) / 2.))
                        .text_color(look.palette.text(SECONDARY))
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
                    trace.running
                        && self.is_open(Fold::Trace(trace.key), true)
                        && trace
                            .items
                            .iter()
                            .any(|item| matches!(item, TraceItem::Step(step) if step.running))
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
        let line = risen(
            line.into_any_element(),
            self.motion.came(Key::Step(step.key)),
            look,
        );
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
                    .text_color(palette.text(SECONDARY))
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
        std::iter::once(line).chain(opened).collect()
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
        for (index, line) in shown.lines.iter().enumerate() {
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
                .text_color(look.palette.text(SECONDARY))
                .child(quiet);
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

    /// The lines follow what came at their end on the curve it lands on, until the user scrolls.
    fn follow_on(&mut self, now: Instant) {
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

/// A line of a trace as it draws: its glyph, its title, what it took or why it failed, and how
/// open its chevron is if it opens. The step under way's title `shimmers`, as far through its pass
/// as it says, or under Reduce Motion, holds at the title's colour. What changed on it fades or
/// rolls in by the rest.
struct TraceLine {
    fold: Fold,
    glyph: Option<&'static str>,
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
}

impl TraceLine {
    fn new(fold: Fold, title: SharedString, note: SharedString) -> Self {
        TraceLine {
            fold,
            glyph: None,
            failed: false,
            title,
            note,
            chevron: None,
            shimmers: None,
            alpha: 1.,
            note_alpha: 1.,
            chevron_alpha: 1.,
            rolled: None,
        }
    }
}

/// A trace's line, with a chevron if it opens, which turns down as it opens. Under the pointer, a
/// line that opens has a plate hanging outside its text, which goes to the title's colour.
fn trace_line(look: &Look, line: TraceLine) -> gpui::Stateful<gpui::Div> {
    let palette = look.palette;
    let TraceLine {
        fold,
        glyph,
        failed,
        title,
        note: description,
        chevron,
        shimmers,
        alpha,
        note_alpha,
        chevron_alpha,
        rolled,
    } = line;
    let (id, group): (ElementId, SharedString) = match fold {
        Fold::Trace(key) => (("trace", key).into(), format!("trace-{key}").into()),
        Fold::Step(key) => (
            ("trace-step", key).into(),
            format!("trace-step-{key}").into(),
        ),
        Fold::Group(key) => (
            ("trace-group", key).into(),
            format!("trace-group-{key}").into(),
        ),
    };
    // A failed line is red, its glyph, its words and why, under the pointer too.
    let red: Hsla = rgb(NOT_FOUND).into();
    let note = if failed {
        red
    } else {
        palette.text(TRACE_NOTE)
    };
    let words = match (failed, shimmers) {
        (true, _) => red,
        (false, Some(None)) => palette.text(TITLE),
        // At the shimmer's peak, which each part fades down from.
        (false, Some(Some(_))) => palette.lit(TRACE_TITLE, SHIMMER, 1.),
        (false, None) => palette.text(TRACE_TITLE),
    };
    let glyph = glyph.map(|glyph| {
        let color = if failed {
            red
        } else {
            palette.glyph(TRACE_GLYPH_ALPHA)
        };
        svg()
            .flex_none()
            .path(glyph)
            .size(px(TRACE_GLYPH))
            .text_color(color)
    });
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
    // A count that changed rolls its number, and the words after it stay.
    let noted = !description.is_empty();
    let description = match (rolled, description.split_once(' ')) {
        (Some(was), Some((count, rest))) => div()
            .flex_none()
            .flex()
            .child(self::rolled(
                count.to_string().into(),
                Some(&was),
                TRACE_LINE,
                look,
            ))
            .child(format!(" {rest}"))
            .into_any_element(),
        _ => div()
            .flex_none()
            .child(description.clone())
            .into_any_element(),
    };
    let hover = group.clone();
    div()
        .id(id)
        .group(group)
        .flex_none()
        .max_w_full()
        .h(px(TRACE_LINE))
        .ml(px(-TRACE_HANG))
        .px(px(TRACE_HANG))
        .flex()
        .items_center()
        .gap(px(TRACE_GAP))
        .rounded(px(NAV_RADIUS))
        .whitespace_nowrap()
        .when(opens, |line| {
            line.cursor_pointer()
                .hover(move |style| style.bg(palette.plate(TRACE_HOVER)))
        })
        .children(glyph)
        .child(
            div()
                .min_w_0()
                .truncate()
                .opacity(alpha)
                .text_color(words)
                // A shimmering title is lit by its shimmer, under the pointer too.
                .when(
                    opens && !failed && !matches!(shimmers, Some(Some(_))),
                    |title| {
                        title.group_hover(hover, move |style| style.text_color(palette.text(TITLE)))
                    },
                )
                .child(match shimmers {
                    Some(Some(phase)) => shimmering(&palette, title, phase),
                    _ => StyledText::new(title),
                }),
        )
        .when(noted, |line| {
            line.child(
                div()
                    .flex_none()
                    .opacity(note_alpha)
                    .text_color(note)
                    .child(description),
            )
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
}

impl Look {
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
}

impl Render for TalkView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let still = cx.reduce_motion();
        let asked = self
            .shown
            .as_ref()
            .and_then(|shown| shown.page.as_ref())
            .map(|page| page.url.clone());
        let Some(look) = self.shell.upgrade().map(|shell| {
            let shell = shell.read(cx);
            let asked_favicon = asked.and_then(|url| shell.favicon_at(&url));
            Look {
                palette: shell.palette,
                now,
                still,
                asked_favicon,
            }
        }) else {
            return div().into_any_element();
        };
        // The reply's placeholder follows the appearance, as the lines drawn in `look` do (#1659).
        let placeholder = look.palette.text(FORM_PLACEHOLDER);
        (self.reply).update(cx, |reply, cx| reply.set_placeholder_color(placeholder, cx));
        self.settle_folds(now, still);
        // A shorter or narrower window keeps the newest line in view while the user reads at the
        // end, as the scroll keeps its distance from the top.
        let viewport = window.viewport_size();
        if std::mem::replace(&mut self.viewport, viewport) != viewport && self.at_end() {
            self.scroll.scroll_to_bottom();
        }
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
        // The header and the reply field are the column's; Pages' grid takes the area's width.
        let on_column = |child: gpui::AnyElement| {
            div()
                .w_full()
                .flex()
                .justify_center()
                .child(div().w_full().max_w(px(COLUMN)).child(child))
        };
        let middle = match self.on_pages() {
            true => div()
                .flex_1()
                .min_h_0()
                .mx(px(PICTURE_GAP - INSET))
                .flex()
                .mb(px(REPLY_GAP))
                .child(self.grid(&look, cx)),
            false => div()
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
        };
        let column = div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(text::BODY))
            .line_height(px(TEXT_LINE))
            .text_color(palette.text(TITLE))
            .child(on_column(self.header(&look, cx).into_any_element()))
            .child(middle)
            .child(on_column(self.reply(&look, cx).into_any_element()));
        let drawn = self.drawn_at.clone();
        div()
            .id("conversation")
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|view, _: &SendReply, window, cx| view.send_reply(window, cx)))
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

/// What a page's picture says under its site: why the user holds it from the agent, `held`, as
/// "You're driving" when they took it over or "Needs you · Sign in" when the agent asked them,
/// else the agent's step while it works there.
pub(super) fn picture_status(
    held: Option<&SharedString>,
    step: Option<SharedString>,
) -> Option<SharedString> {
    match held {
        Some(why) if why.is_empty() => Some("You're driving".into()),
        Some(why) => Some(format!("Needs you · {why}").into()),
        None => step,
    }
}

#[cfg(test)]
mod tests;
