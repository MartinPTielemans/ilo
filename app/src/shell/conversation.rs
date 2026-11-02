//! A task's conversation, which the page area shows in the page's place (Agent task,
//! task_area.rs). A click on a task's card shows it, coming in from the card's side. Its pages'
//! pictures are along its top, taken only while it shows, then what was asked with its page, what
//! each side said with the steps between folded into a line, what it's doing now, and the reply
//! field at its foot. A picture's click zooms its page out of it. Esc steps back to the Space's
//! tab, ⌘W closes the card, and ⌘. stops the task.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    Bounds, Context, Corners, Entity, Focusable, FontWeight, IntoElement, MouseButton, Pixels,
    Render, RenderImage, SharedString, WeakEntity, Window, actions, canvas, div, prelude::*, px,
    rgba,
};

use super::{
    SECONDARY, STATUS_AGENT, STATUS_DOT, Shell, TITLE,
    browser::Browser,
    permission::card_button,
    picture::{PagePicture, next_request},
    rows::ringed_dot,
};
use crate::agents::task::{Conversation, Dot, Line, TaskAction};
use crate::text_input::TextInput;
use crate::tokens::shell::FORM_PLACEHOLDER;
use crate::tokens::shell::conversation::{
    BUBBLE_X, BUBBLE_Y, COLUMN, HEADER, INSET, LINE_GAP, PICTURE_GAP, PICTURE_HEIGHT,
    PICTURE_RADIUS, PICTURE_WIDTH, RADIUS, REPLY_BOTTOM, REPLY_HEIGHT, REPLY_INSET, SITE, TOP,
};
use crate::tokens::shell::empty::KEYS_FILL;
use crate::tokens::text;

actions!(conversation, [SendReply, StopTask]);

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
            Some(talk) if talk.agent == agent => talk.view,
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
        if self.watched_task_page().is_some() {
            return false;
        }
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

    /// Gives the reply field the keyboard once the card's click has shown the conversation.
    pub(super) fn focus_talk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.focus_talk)
            && let Some(talk) = &self.talk
        {
            let reply = talk.view.read(cx).reply.clone();
            window.focus(&reply.focus_handle(cx), cx);
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
                (&shown.doing, &shown.tabs, shown.lines.len())
                    != (&conversation.doing, &conversation.tabs, conversation.lines.len())
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
            self.shown = Some(conversation);
            cx.notify();
        }
        Some(stale)
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
        let agent = self.agent.clone();
        self.shell
            .update(cx, |shell, cx| shell.browser.task(agent, TaskAction::Reply(text.into()), cx))
            .ok();
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

    /// The agent's name and the task's title, with Stop ⌘. while it works or waits on the user.
    fn header(&self, look: &Look, cx: &mut Context<Self>) -> gpui::Div {
        let shown = self.shown.as_ref();
        let name = shown.map_or(SharedString::default(), |shown| {
            format!("{} · {}", shown.name, shown.title).into()
        });
        let stop = shown.filter(|shown| shown.stoppable).map(|_| {
            card_button(&look.palette, "conversation-stop", "Stop ⌘.").on_click(cx.listener(
                |view, _, _, cx| {
                    view.shell.update(cx, |shell, cx| shell.stop_talk(cx)).ok();
                },
            ))
        });
        div()
            .flex_none()
            .h(px(HEADER))
            .flex()
            .items_center()
            .justify_between()
            .text_color(look.palette.text(SECONDARY))
            .child(div().min_w_0().truncate().child(name))
            .children(stop)
    }

    /// Its pages' pictures, each over its site, in a row; a click zooms the page out of it.
    fn pictures(&self, look: &Look, cx: &mut Context<Self>) -> Option<gpui::Div> {
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
            let picture = div()
                .relative()
                .w(px(PICTURE_WIDTH))
                .h(px(PICTURE_HEIGHT))
                .rounded(px(PICTURE_RADIUS))
                .bg(look.palette.plate(rgba(KEYS_FILL).a))
                .overflow_hidden()
                .child(paint);
            div()
                .id(("conversation-page", tab))
                .flex_none()
                .w(px(PICTURE_WIDTH))
                .cursor_pointer()
                .child(picture)
                .child(
                    div()
                        .h(px(SITE))
                        .truncate()
                        .text_size(px(text::SMALL))
                        .text_color(look.palette.text(SECONDARY))
                        .child(site.clone()),
                )
                .on_click(cx.listener(move |view, _, _, cx| view.watch(tab, cx)))
        });
        Some(div().flex_none().flex().gap(px(PICTURE_GAP)).pt(px(LINE_GAP)).children(pictures))
    }

    /// What was asked, with its page, and the lines after it, then what it's doing now.
    fn lines(&self, look: &Look) -> Vec<gpui::AnyElement> {
        let Some(shown) = &self.shown else { return Vec::new() };
        let palette = look.palette;
        let quiet = |text: SharedString| {
            div().text_size(px(text::SMALL)).text_color(palette.text(SECONDARY)).child(text)
        };
        let you = |text: SharedString| {
            div()
                .flex()
                .justify_end()
                .child(
                    div()
                        .max_w(px(COLUMN * 0.75))
                        .px(px(BUBBLE_X))
                        .py(px(BUBBLE_Y))
                        .rounded(px(RADIUS))
                        .bg(palette.plate(rgba(KEYS_FILL).a))
                        .child(text),
                )
                .into_any_element()
        };
        let mut lines = vec![you(shown.asked.clone())];
        if let Some(page) = &shown.page {
            let title = if page.title.is_empty() { &page.url } else { &page.title };
            lines.push(div().flex().justify_end().child(quiet(title.clone())).into_any_element());
        }
        for line in &shown.lines {
            lines.push(match line {
                Line::You(text) => you(text.clone()),
                Line::Said(text) => div().child(text.clone()).into_any_element(),
                Line::Steps { last, count } => {
                    let steps = match count {
                        1 => "1 step".to_string(),
                        count => format!("{count} steps"),
                    };
                    quiet(format!("{last} · {steps}").into()).into_any_element()
                }
            });
        }
        if let Some(doing) = &shown.doing {
            let dot = ringed_dot(STATUS_AGENT, STATUS_DOT).opacity(look.breath);
            lines.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(BUBBLE_X))
                    .child(dot)
                    .child(quiet(doing.clone()))
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
            .mt(px(LINE_GAP))
            .h(px(REPLY_HEIGHT))
            .px(px(REPLY_INSET))
            .flex()
            .items_center()
            .rounded(px(RADIUS))
            .bg(look.palette.plate(rgba(KEYS_FILL).a))
            .font_weight(FontWeight::NORMAL)
            .on_click(cx.listener(move |_, _, window, cx| {
                window.focus(&reply.focus_handle(cx), cx);
            }))
            .child(div().flex_1().child(self.reply.clone()))
    }
}

/// What the view takes from its window as it draws: the colours, and how far the working dot has
/// breathed.
struct Look {
    palette: super::Palette,
    breath: f32,
}

impl Render for TalkView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(look) = self.shell.upgrade().map(|shell| {
            let shell = shell.read(cx);
            Look { palette: shell.palette, breath: shell.breath.strength() }
        }) else {
            return div().into_any_element();
        };
        let column = div()
            .size_full()
            .max_w(px(COLUMN))
            .flex()
            .flex_col()
            .text_size(px(text::BODY))
            .text_color(look.palette.text(TITLE))
            .child(self.header(&look, cx))
            .children(self.pictures(&look, cx))
            .child(
                div()
                    .id("conversation-lines")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(LINE_GAP))
                    .pt(px(LINE_GAP))
                    .children(self.lines(&look)),
            )
            .child(self.reply(&look, cx));
        div()
            .id("conversation")
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|view, _: &SendReply, window, cx| view.send_reply(window, cx)))
            .size_full()
            .bg(look.palette.page)
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
