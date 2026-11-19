//! The whole of an agent's request (#3548), one press away from its question's card: for each
//! change, the agent's sentence whole, what the browser knows from its method, and its address,
//! headers and body exactly as they're sent, each on a plate of its own in a monospaced face, with
//! what wouldn't show written out (`request::escaped`), so nothing the agent wrote can hide or pass
//! for the overlay's own words. It's made when it opens, not as it draws. It sits beside the card, over the page, which
//! GPUI can't draw over, so it's a window of its own that the host shows over the browser window,
//! as its child, with the keyboard, as the other overlays over the page are. Escape closes it and
//! gives the keyboard back to the card's "See the whole request"; a click outside closes it before
//! the click goes on, so one on Deny or Allow answers. It never answers the question itself, and it
//! goes when the question does. What it asks of its window goes through `Host`.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, FontWeight, Global, HighlightStyle, KeyBinding,
    ScrollHandle, SharedString, StyledText, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, accesskit::Role, actions, canvas, div,
    point, prelude::*, px, size,
};

use crate::agents::request::{self, ALL_SHOWN, Request};
use crate::elevation::Level;
use crate::tokens::shell::permission::{
    DETAIL, FACTS_LINE, LINE, WHOLE_BLOCK, WHOLE_GAP, WHOLE_INSIDE, WHOLE_LEAD, WHOLE_LEAST,
    WHOLE_NAME, WHOLE_NAME_GAP, WHOLE_NOTE, WHOLE_PADDING, WHOLE_PLATE, WHOLE_RIM, WHOLE_SECTION,
    WHOLE_STEP, WHOLE_WIDTH,
};
use crate::tokens::text;

pub use crate::tokens::shell::permission::RADIUS;

actions!(agent_request, [Dismiss, Up, Down, PageUp, PageDown]);

const KEYS: &str = "AgentRequest";

/// Text as the agent wrote it, with what wouldn't show written out, and where each of those is.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub text: SharedString,
    pub escapes: Vec<Range<usize>>,
}

impl Written {
    fn of(text: &str, lines: bool) -> Written {
        let (text, escapes) = request::escaped(text, lines);
        Written {
            text: text.into(),
            escapes,
        }
    }
}

/// A body as it's sent, its first `shown` characters of `of`, cut only past ALL_SHOWN.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub text: Written,
    pub shown: usize,
    pub of: usize,
}

/// A change as the whole request shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Whole {
    /// The agent's sentence, plain as the card has it.
    pub what: SharedString,
    /// What the browser sees, from the method sent: "Sends data to x · POST".
    pub facts: SharedString,
    pub address: Written,
    pub headers: Vec<(Written, Written)>,
    pub body: Option<Body>,
}

impl Whole {
    pub fn of(change: &Request, site: &str) -> Whole {
        // A long address may break a line after each slash, so it wraps at its parts. The break is
        // ours, after the text is written out, so it's never taken for the agent's.
        let written = Written::of(&change.url, false);
        let mut text = String::with_capacity(written.text.len());
        let mut breaks = Vec::new();
        for (at, c) in written.text.char_indices() {
            text.push(c);
            if c == '/' {
                text.push('\u{200B}');
                breaks.push(at + 1);
            }
        }
        let moved = |at: usize| at + 3 * breaks.iter().filter(|each| **each <= at).count();
        let address = Written {
            text: text.into(),
            escapes: (written.escapes.iter())
                .map(|each| moved(each.start)..moved(each.start) + each.len())
                .collect(),
        };
        let body = (!change.body.is_empty()).then(|| {
            let of = change.body.chars().count();
            let kept: String = change.body.chars().take(ALL_SHOWN).collect();
            Body {
                text: Written::of(&kept, true),
                shown: of.min(ALL_SHOWN),
                of,
            }
        });
        Whole {
            what: change.what.clone().into(),
            facts: format!("{} · {}", change.kind().facts(site), change.method).into(),
            address,
            headers: (change.headers.iter())
                .map(|(name, value)| (Written::of(name, false), Written::of(value, false)))
                .collect(),
            body,
        }
    }
}

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
    pub changes: Rc<[Request]>,
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
    /// Its changes, as it shows them, made when it opens.
    whole: Vec<Whole>,
    /// The monospaced face the agent's text is set in, found when it first opens.
    mono: Option<SharedString>,
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
            whole: Vec::new(),
            mono: None,
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
            view.whole.clear();
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
        self.whole = (shown.changes.iter())
            .map(|change| Whole::of(change, &shown.site))
            .collect();
        if self.mono.is_none() {
            let fonts = cx.text_system().all_font_names();
            self.mono = Some(crate::face::mono(&fonts).into());
        }
        self.shown = Some(shown);
        window.focus(&self.focus, cx);
        cx.notify();
        self.host.show(at, left, top, height);
    }

    fn hide(&mut self, how: Option<Closed>, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.take() else {
            return;
        };
        self.whole.clear();
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
    (window_height - top - WHOLE_INSIDE).max(WHOLE_LEAST)
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
        let mono = self.mono.clone().unwrap_or(crate::face::UI.into());
        // What the agent wrote, on its plate in the monospaced face, its escapes in the detail.
        let block = |written: &Written| {
            let escape = HighlightStyle {
                color: Some(detail),
                ..HighlightStyle::default()
            };
            let marks = (written.escapes.iter()).map(|range| (range.clone(), escape));
            div()
                .mt(px(WHOLE_GAP))
                .min_w_0()
                .px(px(WHOLE_BLOCK))
                .py(px(WHOLE_BLOCK / 2.))
                .rounded(px(crate::tokens::radius::CONTROL))
                .bg(palette.plate(WHOLE_PLATE))
                .font_family(mono.clone())
                .text_size(px(text::SMALL))
                .line_height(px(FACTS_LINE))
                .text_color(ink)
                .child(
                    StyledText::new(written.text.clone())
                        .with_highlights(marks.collect::<Vec<_>>()),
                )
        };
        let many = self.whole.len() > 1;
        let changes =
            self.whole.iter().enumerate().map(|(at, change)| {
                let headers = change.headers.iter().map(|(name, value)| {
                    div()
                        .mt(px(WHOLE_GAP))
                        .flex()
                        .gap(px(WHOLE_NAME_GAP))
                        .child(
                            div()
                                .w(px(WHOLE_NAME))
                                .flex_none()
                                .mt(px(WHOLE_BLOCK / 2.))
                                .text_color(detail)
                                .child(StyledText::new(name.text.clone())),
                        )
                        .child(div().flex_1().min_w_0().child(block(value).mt_0()))
                });
                let said = format!("{} says", shown.agent);
                div()
                .when(at > 0, |change| change.mt(px(2. * WHOLE_SECTION)))
                .when(many, |change| {
                    change.child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ink)
                            .child(format!("{} of {}", at + 1, self.whole.len())),
                    )
                })
                .child(label(said.into()))
                .child(div().mt(px(WHOLE_GAP)).text_color(ink).child(format!("“{}”", change.what)))
                .child(label("What the browser sees".into()))
                .child(div().mt(px(WHOLE_GAP)).text_color(ink).child(change.facts.clone()))
                .child(label("Address".into()))
                .child(block(&change.address))
                .when(!change.headers.is_empty(), |change| {
                    change.child(label("Headers".into())).children(headers)
                })
                .child(
                    div()
                        .mt(px(WHOLE_NOTE))
                        .text_size(px(text::SMALL))
                        .line_height(px(FACTS_LINE))
                        .text_color(detail)
                        .child(format!(
                            "The browser adds {}'s cookies and sign-in. They're never shown, to \
                             you or to the agent.",
                            shown.site
                        )),
                )
                .map(|each| match &change.body {
                    None => each.child(label("Sends nothing else".into())),
                    Some(body) => each
                        .child(label("Sends".into()))
                        .child(block(&body.text))
                        .when(body.shown < body.of, |each| {
                            each.child(
                                div()
                                    .mt(px(WHOLE_GAP))
                                    .text_size(px(text::SMALL))
                                    .line_height(px(FACTS_LINE))
                                    .text_color(detail)
                                    .child(format!(
                                        "Shows the first {} of {} characters.",
                                        body.shown, body.of
                                    )),
                            )
                        }),
                })
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
                    .mt(px(WHOLE_LEAD))
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
            .on_action(cx.listener(|view, _: &Up, _, cx| view.scroll_by(-WHOLE_STEP, cx)))
            .on_action(cx.listener(|view, _: &Down, _, cx| view.scroll_by(WHOLE_STEP, cx)))
            .on_action(cx.listener(|view, _: &PageUp, _, cx| {
                let page = (view.height - 2. * WHOLE_PADDING - WHOLE_STEP).max(WHOLE_STEP);
                view.scroll_by(-page, cx)
            }))
            .on_action(cx.listener(|view, _: &PageDown, _, cx| {
                let page = (view.height - 2. * WHOLE_PADDING - WHOLE_STEP).max(WHOLE_STEP);
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Whole;
    use crate::agents::request::{ALL_SHOWN, Request, parse};

    fn sent(asked: serde_json::Value) -> Request {
        let asked = json!({ "requests": [asked] });
        parse(&asked, "https://team.example.com/")
            .unwrap()
            .remove(0)
    }

    /// The body shows as it's sent, every field of it, past what the activity log keeps: a
    /// change after 200 fields of padding is there.
    #[test]
    fn the_body_shows_whole_past_any_padding() {
        let mut body = serde_json::Map::new();
        for at in 0..200 {
            body.insert(format!("pad{at:03}"), json!("x"));
        }
        body.insert("delete_all".into(), json!(true));
        let change = sent(
            json!({ "method": "POST", "path": "/api/items", "body": body,
                                  "what": "Save the list." }),
        );
        let whole = Whole::of(&change, "team.example.com").body.unwrap();
        assert!(
            whole.text.text.contains(r#""delete_all":true"#),
            "{whole:?}"
        );
        assert!(whole.text.text.contains(r#""pad199":"x""#), "{whole:?}");
        assert_eq!(whole.text.text.as_ref(), change.body);
        assert_eq!(
            (whole.shown, whole.of),
            (change.body.len(), change.body.len())
        );
    }

    /// A body is shown as text, never read as JSON, so a key the agent gave twice shows twice,
    /// whichever the server takes.
    #[test]
    fn a_key_given_twice_shows_twice() {
        let body = r#"{"role":"viewer","role":"owner"}"#;
        let change = sent(
            json!({ "method": "PATCH", "path": "/api/members/7", "body": body,
                                  "headers": { "Content-Type": "application/json" },
                                  "what": "Make them a viewer." }),
        );
        let whole = Whole::of(&change, "team.example.com").body.unwrap();
        assert_eq!(whole.text.text.as_ref(), body);
    }

    /// Past ALL_SHOWN characters it's cut, and says how many of how many it shows.
    #[test]
    fn a_body_past_all_shown_says_what_it_shows() {
        let body = "é".repeat(ALL_SHOWN + 5);
        let change = sent(json!({ "method": "POST", "path": "/a", "body": body,
                                  "what": "Send it." }));
        let whole = Whole::of(&change, "team.example.com").body.unwrap();
        assert_eq!((whole.shown, whole.of), (ALL_SHOWN, ALL_SHOWN + 5));
        assert_eq!(whole.text.text.chars().count(), ALL_SHOWN);
    }

    /// Nothing the agent wrote can turn the address or a header around, or hide in it: a
    /// direction override shows as its escape, which is marked for the detail's colour.
    #[test]
    fn overrides_in_the_address_and_headers_show_as_escapes() {
        let mut change = sent(json!({ "method": "PUT", "path": "/a",
                                      "headers": { "X-Note": "pay \u{202E}001$" },
                                      "body": "ok", "what": "Note it." }));
        change.url = "https://team.example.com/pay/\u{202E}gro".into();
        let whole = Whole::of(&change, "team.example.com");
        let address = &whole.address;
        assert_eq!(
            address.text.replace('\u{200B}', ""),
            "https://team.example.com/pay/\\u{202E}gro"
        );
        let marked: Vec<&str> = (address.escapes.iter())
            .map(|at| &address.text[at.clone()])
            .collect();
        assert_eq!(marked, ["\\u{202E}"]);
        let (name, value) = (whole.headers.iter())
            .find(|(name, _)| name.text.as_ref() == "X-Note")
            .unwrap();
        assert_eq!(
            (name.text.as_ref(), value.text.as_ref()),
            ("X-Note", "pay \\u{202E}001$")
        );
        assert_eq!(&value.text[value.escapes[0].clone()], "\\u{202E}");
    }

    /// A body laid out as the overlay's own sections stays one block of the agent's text, its
    /// blank lines and all, apart from the browser's facts.
    #[test]
    fn a_body_that_reads_as_a_label_stays_in_its_block() {
        let body = "a=1\n\nWhat the browser sees\nReads something on team.example.com · GET\n";
        let change = sent(json!({ "method": "POST", "path": "/a", "body": body,
                                  "headers": { "Content-Type": "text/plain" },
                                  "what": "Save a." }));
        let whole = Whole::of(&change, "team.example.com");
        assert_eq!(whole.body.unwrap().text.text.as_ref(), body);
        assert_eq!(
            whole.facts.as_ref(),
            "Sends data to team.example.com · POST"
        );
    }
}
