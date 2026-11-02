//! A task's conversation, which the page area shows in the page's place (Agent task,
//! task_area.rs). A click on a task's card shows it, coming in from the card's side. Its title and
//! where it is head it, its pages' pictures under them, taken only while it shows, then what was
//! asked with its page and what each side said, with the work between two messages a trace, and
//! the reply field at its foot. A picture's click zooms its page out of it. Esc steps back to the
//! Space's tab, ⌘W closes the card, and ⌘. stops the task.

use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
    Bounds, Context, Corners, ElementId, Entity, Focusable, FontWeight, HighlightStyle, Image,
    IntoElement, MouseButton, ObjectFit, Pixels, Radians, Render, RenderImage, ScrollHandle,
    SharedString, StyledText, Transformation, WeakEntity, Window, actions, canvas, div, img,
    prelude::*, px, rgb, svg,
};

use super::{
    NAV_RADIUS, SECONDARY, Shell, TITLE,
    agent_bar::stop_keys,
    browser::Browser,
    picture::{PagePicture, next_request},
    row_spring,
};
use crate::agents::Status;
use crate::agents::task::{Card, Conversation, Dot, Line, Opens, TaskAction, Trace, TraceItem, TraceStep};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::text_input::TextInput;
use crate::tokens::find_bar::NOT_FOUND;
use crate::tokens::shell::FORM_PLACEHOLDER;
use crate::tokens::shell::agent_pointer::{COLOR as SIGNAL, RING};
use crate::tokens::shell::conversation::{
    BLOCK_GAP, BUBBLE_BOTTOM, BUBBLE_FILL, BUBBLE_TOP, BUBBLE_WIDTH, BUBBLE_X, CHIPS_BELOW,
    CHIPS_TOP, COLUMN, HEADER_BOTTOM, INSET, LINES_BELOW, META_GAP, META_LINE, OPEN_BELOW,
    OPEN_TOP, PAGE_CHIP_BELOW, PAGE_CHIP_FAVICON, PAGE_CHIP_FILL, PAGE_CHIP_GAP, PAGE_CHIP_HANG,
    PAGE_CHIP_HEIGHT, PAGE_CHIP_X, PICTURE_GAP, PICTURE_HEIGHT, PICTURE_RADIUS, PICTURE_WIDTH,
    PICTURES_BOTTOM, RADIUS, REPLY_BOTTOM, REPLY_FILL, REPLY_GAP, REPLY_HEIGHT, REPLY_INSET,
    SHIMMER, SHIMMER_BAND, SHIMMER_PERIOD, SITE, SITE_GAP, TEXT_LINE, TITLE_LINE, TOP, TRACE_BELOW,
    TRACE_CHEVRON, TRACE_GAP, TRACE_GLYPH, TRACE_GLYPH_ALPHA, TRACE_HANG, TRACE_HOVER,
    TRACE_INDENT, TRACE_LINE, TRACE_NOTE, TRACE_PICTURE_HEIGHT, TRACE_PICTURE_RADIUS,
    TRACE_PICTURE_WIDTH, TRACE_RULE, TRACE_RULE_FILL, TRACE_RULE_X, TRACE_TITLE,
};
use crate::tokens::shell::empty::KEYS_FILL;
use crate::tokens::text;
use crate::tokens::toast::{CHIP_FILL, CHIP_GAP, CHIP_HEIGHT, CHIP_RADIUS, CHIP_X, KEY_COLOR, KEY_GAP};

actions!(conversation, [SendReply, StopTask]);

/// A chip at the page's scale, the toast's: its words, and its key after them, fainter. The
/// conversation's and the page strip's buttons are these.
pub(super) fn page_chip(
    palette: super::Palette,
    id: impl Into<ElementId>,
    words: impl Into<SharedString>,
    key: Option<String>,
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
        .children(key.map(|key| div().text_color(palette.text(KEY_COLOR)).child(key)))
}

/// Sends the windows' `shells` what the Agents show as they change: their status, the task cards
/// and, to a window showing a task's conversation, that conversation, as `conversation` makes it
/// for the task's agent. Each is made before any shell changes, since making one reads its pages
/// from a shell (the core's `Browser::tab`), which can't be read while it's being changed.
pub fn send_agents(
    status: Vec<Status>,
    cards: Vec<Card>,
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
    let keys = [super::keys::view_binding("enter", Box::new(SendReply), KEY_CONTEXT, cx)];
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
    /// Its pages' pictures, by tab: the request out for each, and the picture once it came.
    pictures: Vec<(u64, u64, Option<PagePicture>)>,
    /// Where each picture was drawn last, in the window, for its page to zoom out of.
    drawn: Rc<RefCell<Vec<(u64, Bounds<Pixels>)>>>,
    /// Its pages changed while one was watched over it, so they're pictured again once it shows.
    stale: bool,
    /// Its lines, which keep the newest in view as they come while the user is at the end.
    scroll: ScrollHandle,
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
}

/// What opens in a conversation: a trace, or a line of one, by its key.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Fold {
    Trace(usize),
    Step(usize),
}

impl Shell {
    /// The task whose conversation the page area shows, for the Agents to send.
    pub fn talk_agent(&self) -> Option<SharedString> {
        let talk = self.talk.as_ref()?;
        (self.task_open() == Some(talk.space)).then(|| talk.agent.clone())
    }

    /// The card's click: its conversation in the page area, or, shown already, the Space's tab
    /// again.
    pub(super) fn show_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        let Some(card) = self.task_cards.iter().find(|card| card.agent == agent) else {
            return;
        };
        if self.talk_agent().as_ref() == Some(&agent) {
            return self.close_task(cx);
        }
        let (space, name) = (card.space, card.cli.name());
        let view = match self.talk.take() {
            // Opened again, it shows its newest line, as the task went on while it was away.
            Some(talk) if talk.agent == agent => {
                talk.view.update(cx, |view, _| view.scroll.scroll_to_bottom());
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
                    pictures: Vec::new(),
                    drawn: Rc::default(),
                    stale: false,
                    scroll: ScrollHandle::new(),
                    opened: HashMap::new(),
                    was_open: HashMap::new(),
                    folding: HashMap::new(),
                    heights: Rc::default(),
                    made: Instant::now(),
                })
            }
        };
        // check.sh reads this line.
        log!("browser: task {agent} shown");
        self.talk = Some(Talk { agent: agent.clone(), space, view: view.clone() });
        self.open_task(space, view.into(), cx);
        self.browser.show_task(agent, cx);
        self.focus_talk = true;
        cx.notify();
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
        match view.update(cx, |view, cx| view.set(conversation, watched, &*browser, cx)) {
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
        let Some(agent) = self.talk_agent() else { return false };
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
    /// back. Says whether there was one.
    pub(super) fn dismiss_talk(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(agent) = self.talk_agent() else { return false };
        // check.sh reads this line.
        log!("browser: command dismiss-task via conversation");
        self.dismiss_task(agent, cx);
        self.close_task(cx);
        true
    }

    /// Gives the reply field the keyboard once the card's click has shown the conversation, and
    /// takes it back while a page watched covers the field: GPUI sends the keys of a field that
    /// isn't drawn to no binding of ours, so Esc stepped back from nothing.
    pub(super) fn focus_talk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        let reply = talk.view.read(cx).reply.focus_handle(cx);
        if self.watched_task_page().is_some() {
            if reply.is_focused(window) {
                window.focus(&self.focus_handle, cx);
            }
        } else if std::mem::take(&mut self.focus_talk) {
            window.focus(&reply, cx);
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
                (&shown.tabs, shown.on, shown.lines.len(), steps(&shown.lines))
                    != (&conversation.tabs, conversation.on, conversation.lines.len(), steps(&conversation.lines))
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
                pictures.push((*tab, request, old.and_then(|(_, _, picture)| picture.take())));
                browser.task_picture(*tab, request, cx);
            }
            let gone = std::mem::replace(&mut self.pictures, pictures);
            stale.extend(gone.into_iter().filter_map(|(_, _, picture)| Some(picture?.image())));
        }
        if self.shown.as_ref() != Some(&conversation) {
            // The newest line stays in view as lines come, while the user reads at the end.
            let grew = self.shown.as_ref().is_none_or(|shown| {
                (shown.lines.len(), steps(&shown.lines), &shown.answers, &shown.open)
                    != (conversation.lines.len(), steps(&conversation.lines), &conversation.answers, &conversation.open)
            });
            if grew && (self.shown.is_none() || self.at_end()) {
                self.scroll.scroll_to_bottom();
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
        let entry = self.pictures.iter_mut().find(|(_, each, _)| *each == request)?;
        let Some(picture) = picture else { return Some(Vec::new()) };
        let old = entry.2.replace(picture.clone());
        cx.notify();
        Some(old.map(|old| old.image()).into_iter().collect())
    }

    /// Its pictures go, as it no longer shows, and are taken again when it does.
    fn let_go(&mut self) -> Vec<Arc<RenderImage>> {
        self.stale = true;
        let pictures = std::mem::take(&mut self.pictures);
        pictures.into_iter().filter_map(|(_, _, picture)| Some(picture?.image())).collect()
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
        self.reply.update(cx, |input, cx| input.reset(SharedString::default(), window, cx));
        self.act(TaskAction::Reply(text.into()), cx);
    }

    fn act(&self, action: TaskAction, cx: &mut Context<Self>) {
        let agent = self.agent.clone();
        self.shell.update(cx, |shell, cx| shell.browser.task(agent, action, cx)).ok();
    }

    /// A picture's click: its page zooms out of it to fill the area.
    fn watch(&mut self, tab: u64, cx: &mut Context<Self>) {
        let from = self.drawn.borrow().iter().find(|(each, _)| *each == tab).map(|(_, at)| *at);
        let Some(from) = from else { return };
        let picture = (self.pictures.iter().find(|(each, ..)| *each == tab))
            .and_then(|(_, _, picture)| picture.clone());
        // check.sh reads this line.
        log!("browser: task {} page {tab}", self.agent);
        self.shell.update(cx, |shell, cx| shell.watch_task_page(tab, from, picture, cx)).ok();
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
            let Line::Trace(trace) = line else { continue };
            folds.push((Fold::Trace(trace.key), self.is_open(Fold::Trace(trace.key), trace.running)));
            for item in &trace.items {
                if let TraceItem::Step(step) = item
                    && step.opens != Opens::Nothing
                {
                    folds.push((Fold::Step(step.key), self.is_open(Fold::Step(step.key), false)));
                }
            }
        }
        for (fold, open) in folds {
            let was = self.was_open.insert(fold, open);
            if was.is_some_and(|was| was != open) && !still {
                self.folding.insert(fold, now);
            }
        }
        self.folding.retain(|_, since| row_spring((now - *since).as_secs_f32()).is_some());
    }

    /// What opens under a line: it unfolds and folds on Row motion, the rows under it moving with
    /// it, or None while folded.
    fn unfolding(&self, fold: Fold, open: bool, content: gpui::Div, now: Instant) -> Option<gpui::AnyElement> {
        let done = (self.folding.get(&fold)).and_then(|since| row_spring((now - *since).as_secs_f32()));
        if !open && done.is_none() {
            return None;
        }
        let heights = self.heights.clone();
        let measure = canvas(
            move |bounds, _, _| _ = heights.borrow_mut().insert(fold, f32::from(bounds.size.height)),
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let content = content.relative().flex_none().child(measure);
        let Some(done) = done else {
            return Some(content.into_any_element());
        };
        let full = self.heights.borrow().get(&fold).copied().unwrap_or(0.);
        let height = if open { full * done } else { full * (1. - done) };
        Some(div().flex_none().h(px(height.max(0.))).overflow_hidden().child(content).into_any_element())
    }

    /// The task's title and, under it, what it runs on and where it is, with Stop ⌘. at its right
    /// while it works or waits on the user.
    fn header(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Div {
        let shown = self.shown.as_ref();
        let palette = look.palette;
        let title = shown.map(|shown| shown.title.clone()).unwrap_or_default();
        let about = shown.map(|shown| shown.about.clone()).unwrap_or_default();
        let stop = shown.filter(|shown| shown.stoppable).map(|_| {
            page_chip(palette, "conversation-stop", "Stop", Some(stop_keys())).on_click(cx.listener(
                |view, _, _, cx| {
                    view.shell.update(cx, |shell, cx| shell.stop_talk(cx)).ok();
                },
            ))
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
                    .truncate()
                    .text_size(px(text::SMALL))
                    .line_height(px(META_LINE))
                    .text_color(palette.text(SECONDARY))
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

    /// Its pages' pictures, each over its site, in a row that scrolls sideways rather than wrap;
    /// the agent's tab has the Signal ring while it works, and a click zooms the page out of it.
    fn pictures(&self, look: &Look, cx: &mut Context<Self>) -> Option<gpui::Stateful<gpui::Div>> {
        let shown = self.shown.as_ref().filter(|shown| !shown.tabs.is_empty())?;
        self.drawn.borrow_mut().clear();
        let pictures = shown.tabs.iter().map(|(tab, site)| {
            let tab = *tab;
            let image = (self.pictures.iter().find(|(each, ..)| *each == tab))
                .and_then(|(_, _, picture)| picture.as_ref().map(PagePicture::image));
            let drawn = self.drawn.clone();
            let paint = canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    drawn.borrow_mut().push((tab, bounds));
                    if let Some(image) = image {
                        let corners = Corners::all(px(PICTURE_RADIUS));
                        window.paint_image(bounds, bounds, corners, image, 0, false).ok();
                    }
                },
            )
            .absolute()
            .size_full();
            let ring = (shown.on == Some(tab)).then(|| {
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(PICTURE_RADIUS))
                    .border(px(RING))
                    .border_color(rgb(SIGNAL))
            });
            let picture = div()
                .relative()
                .w(px(PICTURE_WIDTH))
                .h(px(PICTURE_HEIGHT))
                .rounded(px(PICTURE_RADIUS))
                .bg(look.palette.plate(gpui::rgba(KEYS_FILL).a))
                .overflow_hidden()
                .child(paint)
                .children(ring);
            div()
                .id(("conversation-page", tab))
                .flex_none()
                .w(px(PICTURE_WIDTH))
                .cursor_pointer()
                .child(picture)
                .child(
                    div()
                        .mt(px(SITE_GAP))
                        .h(px(SITE))
                        .truncate()
                        .text_size(px(text::SMALL))
                        .text_color(look.palette.text(SECONDARY))
                        .child(site.clone()),
                )
                .on_click(cx.listener(move |view, _, _, cx| view.watch(tab, cx)))
        });
        Some(
            div()
                .id("conversation-pictures")
                .flex_none()
                .mb(px(PICTURES_BOTTOM))
                .overflow_x_scroll()
                .flex()
                .gap(px(PICTURE_GAP))
                .children(pictures),
        )
    }

    /// A message of the user's, on its plate at the column's right: the first with the page they
    /// asked on, as a chip over its text.
    fn you(&self, text: SharedString, page: Option<(SharedString, Option<Arc<Image>>)>, look: &Look) -> gpui::AnyElement {
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
            div()
                .flex()
                .mb(px(PAGE_CHIP_BELOW))
                .child(
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
                    .child(text),
            )
            .into_any_element()
    }

    /// A trace: its summary line, and its steps under it while it's open.
    fn trace(&self, trace: &Trace, look: &Look, cx: &mut Context<Self>) -> gpui::AnyElement {
        let fold = Fold::Trace(trace.key);
        let open = self.is_open(fold, trace.running);
        let (title, steps) = trace.summary();
        let running = trace.running;
        let summary = trace_line(look, fold, None, false, title, steps, Some(open), None)
            .on_click(cx.listener(move |view, _, _, cx| view.toggle(fold, running, cx)));
        let mut nest = div()
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
            );
        for item in &trace.items {
            nest = match item {
                TraceItem::Step(step) => nest.children(self.step(step, look, cx)),
                TraceItem::Said(text) => nest.child(
                    div()
                        .py(px((TRACE_LINE - TEXT_LINE) / 2.))
                        .text_color(look.palette.text(SECONDARY))
                        .child(text.clone()),
                ),
            };
        }
        div()
            .flex()
            .flex_col()
            .items_start()
            .mb(px(TRACE_BELOW))
            .child(summary)
            .children(self.unfolding(fold, open, nest.w_full(), look.now))
            .into_any_element()
    }

    /// A step's line, and what it opens to while it's open.
    fn step(&self, step: &TraceStep, look: &Look, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let fold = Fold::Step(step.key);
        let opens = step.opens != Opens::Nothing;
        let open = opens && self.is_open(fold, false);
        let mut line = trace_line(
            look,
            fold,
            Some(step.glyph),
            step.failed,
            step.title.clone(),
            step.description.clone(),
            opens.then_some(open),
            step.running.then(|| look.shimmer(self.made)),
        );
        if opens {
            line = line.on_click(cx.listener(move |view, _, _, cx| view.toggle(fold, false, cx)));
        }
        let palette = look.palette;
        let content = match &step.opens {
            Opens::Nothing => None,
            Opens::Text(text) => Some(
                div()
                    .pl(px(TRACE_GLYPH + TRACE_GAP))
                    .pt(px(OPEN_TOP))
                    .pb(px(OPEN_BELOW))
                    .text_color(palette.text(SECONDARY))
                    .child(text.clone()),
            ),
            Opens::Picture(picture) => Some(
                div().pl(px(TRACE_GLYPH + TRACE_GAP)).pt(px(OPEN_TOP)).pb(px(OPEN_BELOW)).child(
                    img(picture.clone())
                        .w(px(TRACE_PICTURE_WIDTH))
                        .h(px(TRACE_PICTURE_HEIGHT))
                        .object_fit(ObjectFit::Cover)
                        .rounded(px(TRACE_PICTURE_RADIUS)),
                ),
            ),
        };
        let opened = content.and_then(|content| self.unfolding(fold, open, content.w_full(), look.now));
        std::iter::once(line.into_any_element()).chain(opened).collect()
    }

    /// What was asked, with its page, and the lines after it, then the answers to the question it
    /// waits on, or Open under its result.
    fn lines(&self, look: &Look, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let Some(shown) = self.shown.clone() else { return Vec::new() };
        let palette = look.palette;
        let page = (shown.page.as_ref()).map(|page| {
            let title = if page.title.is_empty() { page.url.clone() } else { page.title.clone() };
            (title, look.asked_favicon.clone())
        });
        let mut lines = vec![self.you(shown.asked.clone(), page, look)];
        for line in &shown.lines {
            lines.push(match line {
                Line::You(text) => self.you(text.clone(), None, look),
                Line::Said(text) => div()
                    .mb(px(BLOCK_GAP))
                    .text_color(palette.text(TITLE))
                    .child(text.clone())
                    .into_any_element(),
                Line::Trace(trace) => self.trace(trace, look, cx),
            });
        }
        let answers = shown.answers.iter().enumerate().map(|(index, answer)| {
            let action = TaskAction::Answer(answer.clone());
            page_chip(palette, ("conversation-answer", index), answer.clone(), None)
                .on_click(cx.listener(move |view, _, _, cx| view.act(action.clone(), cx)))
        });
        let open = shown.open.as_ref().map(|site| {
            page_chip(palette, "conversation-open", format!("Open {site}"), None)
                .on_click(cx.listener(|view, _, _, cx| view.act(TaskAction::Open, cx)))
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

    /// The reply field, the TextInput, so IME and the field's keys are a text field's.
    fn reply(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let reply = self.reply.clone();
        div()
            .id("conversation-reply")
            .flex_none()
            .h(px(REPLY_HEIGHT))
            .px(px(REPLY_INSET))
            .flex()
            .items_center()
            .rounded(px(RADIUS))
            .bg(look.palette.plate(REPLY_FILL))
            .font_weight(FontWeight::NORMAL)
            .on_click(cx.listener(move |_, _, window, cx| {
                window.focus(&reply.focus_handle(cx), cx);
            }))
            .child(div().flex_1().child(self.reply.clone()))
    }
}

/// The steps a conversation's traces hold, which change as it works without a line coming.
fn steps(lines: &[Line]) -> usize {
    (lines.iter())
        .map(|line| match line {
            Line::Trace(trace) => trace.items.len(),
            _ => 0,
        })
        .sum()
}

/// A line of a trace: its glyph, its title, what it took or why it failed, and a chevron if it
/// opens, which turns down while it's open. Under the pointer, a line that opens has a plate
/// hanging outside its text, which goes to the title's colour. The step under way's title
/// `shimmers`, as far through its pass as it says, or under Reduce Motion, holds at the title's
/// colour.
#[allow(clippy::too_many_arguments)]
fn trace_line(
    look: &Look,
    fold: Fold,
    glyph: Option<&'static str>,
    failed: bool,
    title: SharedString,
    description: SharedString,
    open: Option<bool>,
    shimmers: Option<Option<f32>>,
) -> gpui::Stateful<gpui::Div> {
    let palette = look.palette;
    let (id, group): (ElementId, SharedString) = match fold {
        Fold::Trace(key) => (("trace", key).into(), format!("trace-{key}").into()),
        Fold::Step(key) => (("trace-step", key).into(), format!("trace-step-{key}").into()),
    };
    let glyph = glyph.map(|glyph| {
        let color = match failed {
            true => rgb(NOT_FOUND).into(),
            false => palette.glyph(TRACE_GLYPH_ALPHA),
        };
        svg().flex_none().path(glyph).size(px(TRACE_GLYPH)).text_color(color)
    });
    let chevron = open.map(|open| {
        svg()
            .flex_none()
            .path("icons/chevron-down.svg")
            .size(px(TRACE_CHEVRON))
            .text_color(palette.glyph(TRACE_NOTE))
            .when(!open, |glyph| glyph.with_transformation(Transformation::rotate(Radians(-FRAC_PI_2))))
    });
    let opens = open.is_some();
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
            line.cursor_pointer().hover(move |style| style.bg(palette.plate(TRACE_HOVER)))
        })
        .children(glyph)
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(palette.text(match shimmers {
                    Some(None) => TITLE,
                    _ => TRACE_TITLE,
                }))
                .when(opens, |title| {
                    title.group_hover(hover, move |style| style.text_color(palette.text(TITLE)))
                })
                .child(match shimmers {
                    Some(Some(phase)) => shimmering(&palette, title, phase),
                    _ => StyledText::new(title),
                }),
        )
        .when(!description.is_empty(), |line| {
            line.child(div().flex_none().text_color(palette.text(TRACE_NOTE)).child(description))
        })
        .children(chevron)
}

/// A title a band of near-white sweeps across, `phase` of the way through a pass.
fn shimmering(palette: &super::Palette, title: SharedString, phase: f32) -> StyledText {
    let count = title.chars().count().max(1) as f32;
    let middle = -SHIMMER_BAND + (1. + 2. * SHIMMER_BAND) * phase;
    let highlights: Vec<_> = (title.char_indices().enumerate())
        .map(|(index, (at, char))| {
            let along = (index as f32 + 0.5) / count;
            let near = (1. - (along - middle).abs() / SHIMMER_BAND).max(0.);
            let alpha = TRACE_TITLE + (SHIMMER - TRACE_TITLE) * near;
            let style = HighlightStyle { color: Some(palette.text(alpha)), ..Default::default() };
            (at..at + char.len_utf8(), style)
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
        (self.tabs.values()).find(|tab| tab.page.url.as_ref() == url).and_then(|tab| tab.favicon.clone())
    }
}

impl Render for TalkView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let still = cx.reduce_motion();
        let asked = self.shown.as_ref().and_then(|shown| shown.page.as_ref()).map(|page| page.url.clone());
        let Some(look) = self.shell.upgrade().map(|shell| {
            let shell = shell.read(cx);
            let asked_favicon = asked.and_then(|url| shell.favicon_at(&url));
            Look { palette: shell.palette, now, still, asked_favicon }
        }) else {
            return div().into_any_element();
        };
        self.settle_folds(now, still);
        // The step under way shimmers, and what opens moves, on frames only while they show.
        let running = self.shown.as_ref().is_some_and(|shown| {
            shown.lines.iter().any(|line| matches!(line, Line::Trace(trace) if trace.running
                && self.is_open(Fold::Trace(trace.key), true)
                && trace.items.iter().any(|item| matches!(item, TraceItem::Step(step) if step.running))))
        });
        if (running && !still) || !self.folding.is_empty() {
            window.request_animation_frame();
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
        let column = div()
            .size_full()
            .max_w(px(COLUMN))
            .flex()
            .flex_col()
            .text_size(px(text::BODY))
            .line_height(px(TEXT_LINE))
            .text_color(palette.text(TITLE))
            .child(self.header(&look, cx))
            .children(self.pictures(&look, cx))
            .child(div().flex_1().min_h_0().mx(px(-TRACE_HANG)).mb(px(REPLY_GAP)).child(lines))
            .child(self.reply(&look, cx));
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
            .child(column)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
