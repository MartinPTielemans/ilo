//! The whole of an agent's request (#3548), one press away from its question's card: for each
//! change, the agent's sentence whole, what the browser knows from its method, its address, its
//! headers and what it sends, laid out to be read. It sits beside the card, over the page, which
//! GPUI can't draw over, so it's a window of its own that the host shows over the browser window,
//! as its child, with the keyboard, as the other overlays over the page are. Escape closes it and
//! gives the keyboard back to the card's "See the whole request"; a click outside closes it before
//! the click goes on, so one on Deny or Allow answers. It never answers the question itself, and it
//! goes when the question does. What it asks of its window goes through `Host`.

use std::rc::Rc;

use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, FontWeight, Global, KeyBinding, ScrollHandle,
    SharedString, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, accesskit::Role, actions, canvas, div, point, prelude::*, px, size,
};

use crate::agents::request::{Request, Sent};
use crate::elevation::Level;
use crate::tokens::shell::permission::{
    DETAIL, FACTS_LINE, LINE, WHOLE_INDENT, WHOLE_INSIDE, WHOLE_NAME, WHOLE_PADDING, WHOLE_RIM,
    WHOLE_SECTION, WHOLE_WIDTH,
};
use crate::tokens::text;

pub use crate::tokens::shell::permission::RADIUS;

actions!(agent_request, [Dismiss, Up, Down, PageUp, PageDown]);

const KEYS: &str = "AgentRequest";
/// What an arrow key scrolls.
const STEP: f32 = 40.;

/// What the whole request asks of its window.
pub trait Host {
    /// Shows its window, `height` tall and WIDTH wide, its top left at (`left`, `top`) in browser
    /// window `window`'s content, with the keyboard, or moves it there if it shows.
    fn show(&self, window: u64, left: f32, top: f32, height: f32);
    /// Makes it `height` tall, keeping its top.
    fn set_height(&self, height: f32);
    /// Hides it, and the browser window gets the keyboard back.
    fn hide(&self);
}

pub const WIDTH: f32 = WHOLE_WIDTH;

/// How it closed: by Escape, which gives the keyboard back to the card's button, or by the user
/// leaving it, such as with a click outside, which goes on to what it was on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Closed {
    Escape,
    Left,
}

/// A question's requests to show whole, and where.
#[derive(Clone)]
pub struct Shown {
    /// The browser window whose card it's beside.
    pub window: u64,
    /// The question's id, by which the shell closes it when the question goes.
    pub question: u64,
    pub agent: SharedString,
    pub site: SharedString,
    pub changes: Vec<Request>,
    /// Its top left in the window's content, and the most it may be tall from there.
    pub left: f32,
    pub top: f32,
    pub room: f32,
    pub closed: Rc<dyn Fn(Closed, &mut App)>,
}

pub struct AgentRequest {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    shown: Option<Shown>,
    /// How tall its window is, to follow what it lays out.
    height: f32,
}

struct RequestWindow(WindowHandle<AgentRequest>);

impl Global for RequestWindow {}

pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<AgentRequest>> {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some(KEYS)),
        KeyBinding::new("up", Up, Some(KEYS)),
        KeyBinding::new("down", Down, Some(KEYS)),
        KeyBinding::new("pageup", PageUp, Some(KEYS)),
        KeyBinding::new("pagedown", PageDown, Some(KEYS)),
        KeyBinding::new("space", PageDown, Some(KEYS)),
    ]);
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(400.)));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show: false,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The host draws its material and shadow, so the window around the text is clear.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|cx| AgentRequest {
            host,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            shown: None,
            height: 0.,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(RequestWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the whole request's window: {error}");
            None
        }
    }
}

fn update(
    cx: &mut App,
    f: impl FnOnce(&mut AgentRequest, &mut Window, &mut Context<AgentRequest>),
) {
    if let Some(window) = cx.try_global::<RequestWindow>().map(|shown| shown.0) {
        window
            .update(cx, |view, window, cx| f(view, window, cx))
            .ok();
    }
}

/// Shows a question's requests whole.
pub fn show(shown: Shown, cx: &mut App) {
    crate::lazy_window::ready::<AgentRequest>(cx);
    update(cx, |view, window, cx| view.show(shown, window, cx));
}

/// Closes it if it shows, unless it's question `keep`'s, as that question is still asked.
pub fn close(keep: Option<u64>, cx: &mut App) {
    update(cx, |view, _, cx| {
        if view
            .shown
            .as_ref()
            .is_some_and(|shown| Some(shown.question) != keep)
        {
            view.hide(None, cx);
        }
    });
}

/// The question whose requests show whole, if any.
pub fn showing(cx: &App) -> Option<u64> {
    showing_over(cx).map(|(_, question)| question)
}

/// The browser window it shows over and the question whose requests it shows, if it shows.
pub fn showing_over(cx: &App) -> Option<(u64, u64)> {
    let window = cx.try_global::<RequestWindow>()?.0;
    window
        .read(cx)
        .ok()
        .and_then(|view| (view.shown.as_ref()).map(|shown| (shown.window, shown.question)))
}

/// Closes it if it shows over browser window `window` for any question but `keep`, the one that
/// window's card shows: a question answered, gone, or covered by a page's takes it along.
pub fn follow(window: u64, keep: Option<u64>, cx: &mut App) {
    if showing_over(cx).is_some_and(|(over, _)| over == window) {
        close(keep, cx);
    }
}

/// Its left in the window's content, beside the card from `card_left` to `card_right`: toward the
/// page, past the card's right with a sidebar on the left, and to its left with one on the right,
/// as the theme picker opens.
pub fn beside(card_left: f32, card_right: f32, right: bool, inset: f32) -> f32 {
    match right {
        true => card_left - 2. * inset - WIDTH,
        false => card_right + 2. * inset,
    }
}

/// The user left it, such as with a click outside, and the host has hidden it.
pub fn dismissed(cx: &mut App) {
    update(cx, |view, _, cx| {
        if let Some(shown) = view.shown.take() {
            log!("browser: agent request {} closed (left)", shown.question);
            cx.notify();
            let closed = shown.closed.clone();
            cx.defer(move |cx| closed(Closed::Left, cx));
        }
    });
}

impl AgentRequest {
    fn show(&mut self, shown: Shown, window: &mut Window, cx: &mut Context<Self>) {
        // Laid out at the most it may be, then made as tall as what it shows.
        let height = match self.height > 0. {
            true => self.height.min(shown.room),
            false => shown.room,
        };
        self.height = height;
        self.scroll.set_offset(point(px(0.), px(0.)));
        log!(
            "browser: agent request {} shown, {} changes",
            shown.question,
            shown.changes.len()
        );
        let (at, left, top) = (shown.window, shown.left, shown.top);
        self.shown = Some(shown);
        window.focus(&self.focus, cx);
        cx.notify();
        self.host.show(at, left, top, height);
    }

    fn hide(&mut self, how: Option<Closed>, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.take() else {
            return;
        };
        self.host.hide();
        cx.notify();
        match how {
            Some(how) => {
                log!("browser: agent request {} closed ({how:?})", shown.question);
                let closed = shown.closed.clone();
                cx.defer(move |cx| closed(how, cx));
            }
            None => log!("browser: agent request {} closed (gone)", shown.question),
        }
    }

    fn scroll_by(&mut self, by: f32, cx: &mut Context<Self>) {
        let offset = self.scroll.offset();
        let most = self.scroll.max_offset().y;
        let y = (offset.y - px(by)).clamp(-most, px(0.));
        self.scroll.set_offset(point(offset.x, y));
        cx.notify();
    }

    /// Lays out what it shows `height` tall: its window follows, up to its room.
    fn measured(&mut self, height: f32, cx: &mut Context<Self>) {
        let Some(shown) = &self.shown else {
            return;
        };
        let height = (height + 2. * WHOLE_PADDING).min(shown.room);
        if (height - self.height).abs() >= 0.5 {
            self.height = height;
            self.host.set_height(height);
            cx.notify();
        }
    }
}

/// The room it has: from `top` to the window's foot, kept inside it.
pub fn room(top: f32, window_height: f32) -> f32 {
    (window_height - top - WHOLE_INSIDE).max(120.)
}

impl Focusable for AgentRequest {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for AgentRequest {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .id("agent-request")
            .key_context(KEYS)
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .overflow_hidden()
            .rounded(px(RADIUS))
            .font_family(crate::face::UI)
            .text_size(px(text::BODY))
            .line_height(px(LINE));
        let Some(shown) = &self.shown else {
            return root;
        };
        let palette = crate::palette::current(cx);
        let (ink, detail) = (palette.on_overlay(1.), palette.on_overlay(DETAIL));
        // Its own plate, as the other overlays draw theirs, under the host's shadow.
        let rim = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(RADIUS))
            .border(px(WHOLE_RIM))
            .border_color(Level::Popover.rim_in(&palette));
        let root = root.bg(palette.overlay);
        let label = |text: SharedString| {
            div()
                .mt(px(WHOLE_SECTION))
                .text_size(px(text::SMALL))
                .line_height(px(FACTS_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(detail)
                .child(text)
        };
        let row = |line: &Sent| {
            let depth = f32::from(line.depth) * WHOLE_INDENT;
            div()
                .mt(px(3.))
                .pl(px(depth))
                .flex()
                .gap(px(10.))
                .child(
                    div()
                        .w(px((WHOLE_NAME - depth).max(40.)))
                        .flex_none()
                        .text_color(detail)
                        .child(line.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(ink)
                        .child(line.value.clone()),
                )
        };
        let many = shown.changes.len() > 1;
        let changes =
            shown.changes.iter().enumerate().map(|(at, change)| {
                let sent = change.sent();
                let kind = change.kind();
                let headers = change.headers.iter().map(|(name, value)| {
                    row(&Sent {
                        depth: 0,
                        name: name.clone(),
                        value: value.clone(),
                    })
                });
                let said = format!("{} says", shown.agent);
                div()
                .when(at > 0, |change| change.mt(px(2. * WHOLE_SECTION)))
                .when(many, |change| {
                    change.child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ink)
                            .child(format!("{} of {}", at + 1, shown.changes.len())),
                    )
                })
                .child(label(said.into()))
                .child(div().mt(px(3.)).text_color(ink).child(format!("“{}”", change.what)))
                .child(label("What the browser sees".into()))
                .child(
                    div()
                        .mt(px(3.))
                        .text_color(ink)
                        .child(format!("{} · {}", kind.facts(&shown.site), change.method)),
                )
                .child(label("Address".into()))
                // Its slashes may break a line, so a long address wraps at its parts.
                .child(div().mt(px(3.)).text_color(ink).child(change.url.replace('/', "/\u{200B}")))
                .when(!change.headers.is_empty(), |change| {
                    change.child(label("Headers".into())).children(headers)
                })
                .child(
                    div()
                        .mt(px(6.))
                        .text_size(px(text::SMALL))
                        .line_height(px(FACTS_LINE))
                        .text_color(detail)
                        .child(format!(
                            "The browser adds {}'s cookies and sign-in. They're never shown, to \
                             you or to the agent.",
                            shown.site
                        )),
                )
                .child(label(match sent.is_empty() {
                    true => "Sends nothing else".into(),
                    false => "Sends".into(),
                }))
                .children(sent.iter().map(row))
            });
        let content = div()
            .relative()
            .child(
                div()
                    .text_size(px(text::TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ink)
                    .child("The whole request"),
            )
            .child(
                div()
                    .mt(px(2.))
                    .text_color(detail)
                    .child("Nothing is sent until you allow it. Opening this answers nothing."),
            )
            .children(changes)
            // Measures what it lays out, which its window's height follows.
            .child(
                canvas(|bounds, _, _| bounds.size.height, {
                    let view = cx.entity().downgrade();
                    move |_, height, _, cx| {
                        let height = f32::from(height);
                        _ = view.update(cx, |view, cx| view.measured(height, cx));
                    }
                })
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        _ = window;
        root.role(Role::Dialog)
            .aria_label("The whole request")
            .on_action(cx.listener(|view, _: &Dismiss, _, cx| view.hide(Some(Closed::Escape), cx)))
            .on_action(cx.listener(|view, _: &Up, _, cx| view.scroll_by(-STEP, cx)))
            .on_action(cx.listener(|view, _: &Down, _, cx| view.scroll_by(STEP, cx)))
            .on_action(cx.listener(|view, _: &PageUp, _, cx| {
                let page = (view.height - 2. * WHOLE_PADDING - STEP).max(STEP);
                view.scroll_by(-page, cx)
            }))
            .on_action(cx.listener(|view, _: &PageDown, _, cx| {
                let page = (view.height - 2. * WHOLE_PADDING - STEP).max(STEP);
                view.scroll_by(page, cx)
            }))
            .child(
                div()
                    .id("agent-request-scroll")
                    .size_full()
                    .p(px(WHOLE_PADDING))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(content),
            )
            .child(rim)
    }
}
