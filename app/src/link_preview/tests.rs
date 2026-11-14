//! The link preview's geometry, keys and clicks, on a host that records what it asks of its window
//! and the core.

use std::cell::{Cell, RefCell};

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;

#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    /// Whether the page shows under the window, as on Windows and Linux.
    under: Cell<bool>,
}

impl Recorder {
    fn push(&self, call: String) {
        self.calls.borrow_mut().push(call);
    }
}

fn describe(frame: Bounds<Pixels>) -> String {
    let (x, y) = (f32::from(frame.origin.x), f32::from(frame.origin.y));
    format!(
        "{x} {y} {} {}",
        f32::from(frame.size.width),
        f32::from(frame.size.height)
    )
}

impl Host for Recorder {
    fn place(&self, tab: u64, page: Bounds<Pixels>, radius: f32) {
        self.push(format!("place {tab} {} r{radius}", describe(page)));
    }
    fn show(&self, window: u64) {
        self.push(format!("show {window}"));
    }
    // A frame of the motion, which tells nothing of what the user did, but for the hole a page
    // under the window shows through.
    fn drew(&self, page: Bounds<Pixels>, _: f32, _: f32) {
        if self.under.get() {
            self.push(format!("hole {}", describe(page)));
        }
    }
    fn page_under(&self) -> bool {
        self.under.get()
    }
    fn release(&self) {
        self.push("release".into());
    }
    fn hide(&self) {
        self.push("hide".into());
    }
    fn close(&self, tab: u64) {
        self.push(format!("close {tab}"));
    }
    fn keep(&self, window: u64, tab: u64, archived: Option<u64>) {
        let archived =
            archived.map_or(String::new(), |archived| format!(" of archived {archived}"));
        self.push(format!("keep {window} {tab}{archived}"));
    }
    fn kept(&self, window: u64, archived: Option<u64>, _: &mut App) {
        if let Some(archived) = archived {
            self.push(format!("kept archived {archived} in {window}"));
        }
    }
    fn closed(&self, window: u64, closed: Closed, undone: bool, _: &mut App) {
        let (x, y) = (f32::from(closed.link.x), f32::from(closed.link.y));
        let undone = if undone { " undone" } else { "" };
        self.push(format!(
            "closed {window} over {} {} {x} {y}{undone}",
            closed.over, closed.url
        ));
    }
    fn tooltip(&self, tip: Option<(Tip, Anchor)>, _: &mut App) {
        match tip {
            // What it says, and the middle of the button's bottom edge it hangs from.
            Some((tip, anchor)) => self.push(format!(
                "tooltip {} {} under {} {}",
                tip.text,
                tip.keys.concat(),
                anchor.x,
                anchor.bottom
            )),
            None => self.push("tooltip none".into()),
        }
    }
}

const WINDOW: u64 = 1;
const TAB: u64 = 7;
/// The pinned tab whose link it opened from.
const OVER: u64 = 3;
/// What closing it tells the window, for ⌘Z and ⇧⌘T to open it again.
const CLOSED: &str = "closed 1 over 3 https://www.example.org/a 400 200";

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// The window's page, right of a 240 pt sidebar in an 800 by 600 window, inset 10 as the card is.
fn page() -> Bounds<Pixels> {
    bounds(240., 10., 550., 580.)
}

// Over that page the card is inset 55, 10% of its width, and 40 more at the top for the controls
// over it: from x 295 to 735 and y 105 to 535, with the page 1 in, inside its rim. The controls are
// 32 tall, from y 65 to 97: the site at the left, keep and close at the right.
const PLACED: &str = "place 7 296 106 438 428 r16";
const DIM_SPOT: (f32, f32) = (100., 300.);
const SITE: (f32, f32) = (305., 81.);
const KEEP_BUTTON: (f32, f32) = (679., 81.);
const CLOSE_BUTTON: (f32, f32) = (719., 81.);

/// A preview opened from a link at 400, 200 in the window.
struct Shown {
    cx: VisualTestContext,
    host: Rc<Recorder>,
}

impl Shown {
    fn new(cx: &mut TestAppContext) -> Self {
        let mut shown = Self::on(Recorder::default(), cx);
        assert_eq!(shown.calls(), [PLACED, "show 1"]);
        shown
    }

    fn on(host: Recorder, cx: &mut TestAppContext) -> Self {
        let host = Rc::new(host);
        let window = cx.update(|cx| open(host.clone(), cx));
        let window = window.expect("no window for the link preview");
        let cx = VisualTestContext::from_window(window.into(), cx);
        cx.simulate_resize(size(px(800.), px(600.)));
        let mut shown = Shown { cx, host };
        shown.show(TAB);
        shown
    }

    fn show(&mut self, tab: u64) {
        self.show_of(tab, None);
    }

    /// Shows `tab`'s page as the preview of the archived tab `archived`, if it's one.
    fn show_of(&mut self, tab: u64, archived: Option<u64>) {
        let link = point(px(400.), px(200.));
        let url = "https://www.example.org/a".into();
        self.cx
            .cx
            .update(|cx| show(WINDOW, tab, OVER, url, (page(), 6.), link, archived, cx));
        self.cx.run_until_parked();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    fn hover(&mut self, (x, y): (f32, f32)) {
        self.cx
            .simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
        self.cx.run_until_parked();
    }

    fn click(&mut self, (x, y): (f32, f32)) {
        self.cx
            .simulate_click(point(px(x), px(y)), Modifiers::none());
        self.cx.run_until_parked();
    }

    /// What the preview asked since the last time, once it has had time to leave.
    fn calls(&mut self) -> Vec<String> {
        self.cx
            .executor()
            .advance_clock(Duration::from_secs_f32(KEEP.max(CLOSE)));
        self.cx.run_until_parked();
        self.host.calls.take()
    }
}

/// The card sits over the window's page, inset by a share of it within its bounds, with room for
/// its controls above it, and kept, the page lands exactly on the window's.
#[test]
fn the_card_sits_over_the_page_and_grows_into_it() {
    assert_eq!(card(page()), bounds(295., 105., 440., 430.));
    assert_eq!(
        card(bounds(0., 0., 200., 150.)),
        bounds(24., 64., 152., 62.)
    );
    assert_eq!(
        card(bounds(0., 0., 3000., 2000.)),
        bounds(120., 160., 2760., 1720.)
    );
    assert_eq!(page_in(card(page())), bounds(296., 106., 438., 428.));
    assert_eq!(controls_over(card(page())), bounds(295., 65., 440., 32.));
    assert_eq!(card_around(page_in(card(page()))), card(page()));
    let link = point(px(400.), px(200.));
    let rest = Pose {
        scale: 1.,
        kept: 0.,
        page: 1.,
        card: 1.,
        dim: 1.,
    };
    assert_eq!(frames(rest, page(), link).2, page_in(card(page())));
    let kept = rest.keeping(KEEP);
    assert_eq!(kept.card, 0.);
    let (card_frame, _, shown) = frames(kept, page(), link);
    assert_eq!(shown, page());
    assert_eq!(card_frame, card_around(page()));
}

/// It grows from the link, at half its size, and a link outside the card, as in the sidebar's
/// favorites, grows it from the card's nearest edge.
#[test]
fn it_grows_from_the_link() {
    let start = Pose::arriving(0.);
    assert_eq!((start.scale, start.page, start.dim), (FROM_SCALE, 0., 0.));
    let (_, about, shown) = frames(start, page(), point(px(400.), px(200.)));
    assert_eq!(about, point(px(400.), px(200.)));
    assert_eq!(shown, bounds(348., 153., 219., 214.));
    let (_, about, _) = frames(start, page(), point(px(100.), px(20.)));
    assert_eq!(about, point(px(295.), px(105.)));
    let open = Pose::arriving(OPEN);
    assert_eq!((open.scale, open.page, open.dim), (1., 1., 1.));
    let closed = open.closing(CLOSE);
    assert_eq!(
        (closed.scale, closed.page, closed.dim),
        (FROM_SCALE, 0., 0.)
    );
}

#[test]
fn its_keys() {
    // Esc is reserved, before the page, as Chromium's stop would otherwise take it.
    assert_eq!(key_action("escape", true), Some(KeyAction::Close));
    assert_eq!(key_action("cmd-enter", false), Some(KeyAction::Keep));
    assert_eq!(key_action("shift-cmd-c", false), Some(KeyAction::CopyUrl));
    assert_eq!(key_action("cmd-w", true), Some(KeyAction::Close));
    assert_eq!(key_action("escape", false), None);
    assert_eq!(key_action("cmd-l", false), None);
    assert_eq!(key_action("cmd-t", true), None);
}

/// The site it shows is the host alone: no www., port or user.
#[test]
fn it_shows_the_site() {
    assert_eq!(site_of("https://www.example.org/a?b#c"), "example.org");
    assert_eq!(
        site_of("http://user@news.example.com:8080/"),
        "news.example.com"
    );
    assert_eq!(site_of("https://example.org"), "example.org");
    assert_eq!(site_of("http://localhost:50821/b.html"), "localhost");
    assert_eq!(site_of("http://[::1]:8080/"), "[::1]");
}

/// Where its page shows under the window, it's laid out at its size from the first and never
/// resized by the motion: out of sight while the card grows with a plate in its place, under the
/// plate the frame after the card first draws at rest, and through the hole from the frame after
/// that. As it closes, the plate fills the hole the frame before the hole closes and the page
/// leaves, before the window's page has the keyboard back (#2519).
#[gpui::test]
fn a_page_under_the_window_waits_for_the_card(cx: &mut TestAppContext) {
    let host = Recorder::default();
    host.under.set(true);
    let mut shown = Shown::on(host, cx);
    let calls = shown.calls();
    assert_eq!(calls[..2], [OUT_OF_SIGHT, "show 1"]);
    assert!(calls[2..].iter().all(|call| call == NO_HOLE), "{calls:?}");
    rest(&mut shown);
    // The card's first frame at rest, which the page comes in under only once it's on screen.
    frame(&mut shown);
    assert_eq!(shown.calls(), [NO_HOLE]);
    frame(&mut shown);
    assert_eq!(shown.calls(), [PLACED, NO_HOLE]);
    frame(&mut shown);
    assert_eq!(shown.calls(), [HOLE]);
    shown.keys("escape");
    frame(&mut shown);
    frame(&mut shown);
    // The key's own frame, then the plate's in the open hole, then the hole closes and the page
    // leaves, and only then does the window's page have the keyboard back.
    let calls = shown.calls();
    assert_eq!(
        calls[..5],
        [HOLE, HOLE, NO_HOLE, OUT_OF_SIGHT, "release"],
        "{calls:?}"
    );
    assert_eq!(calls[calls.len() - 3..], ["hide", "close 7", CLOSED]);
}

/// Under Reduce Motion the card fades in where it rests, and the page waits out of sight for it
/// all the same.
#[gpui::test]
fn under_reduce_motion_a_page_under_the_window_waits_for_the_card(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let host = Recorder::default();
    host.under.set(true);
    let mut shown = Shown::on(host, cx);
    assert_eq!(shown.calls()[..2], [OUT_OF_SIGHT, "show 1"]);
    rest(&mut shown);
    frame(&mut shown);
    frame(&mut shown);
    frame(&mut shown);
    assert_eq!(shown.calls(), [NO_HOLE, PLACED, NO_HOLE, HOLE]);
}

/// Where a page under the window waits: at its size, left of the window.
const OUT_OF_SIGHT: &str = "place 7 -1438 106 438 428 r16";
const NO_HOLE: &str = "hole 400 200 0 0";
const HOLE: &str = "hole 296 106 438 428";

/// Its motion's clock is the wall's, so its window came on screen as long ago as it opens.
fn rest(shown: &mut Shown) {
    shown.cx.cx.update(|cx| {
        update(cx, |preview, _, _| {
            let shown = preview.preview.as_mut().expect("no preview");
            shown.shown = Some(Instant::now() - Duration::from_secs_f32(OPEN));
        })
    });
}

/// A frame of the preview's window.
fn frame(shown: &mut Shown) {
    shown.cx.update(|window, _| window.refresh());
    shown.cx.run_until_parked();
}

/// Esc closes it back into the link, and its page once it has left.
#[gpui::test]
fn escape_closes_it(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    shown.keys("escape");
    assert_eq!(shown.calls(), ["release", "hide", "close 7", CLOSED]);
}

/// An archived tab's preview, from the Library (#696): ⌘W and Esc close it and leave it in the
/// archive, with nothing for ⌘Z to reopen, and ⌘↩ keeps it as a tab, which restores it from the
/// archive.
#[gpui::test]
fn an_archived_tabs_preview_closes_leaving_it_and_keeps_restoring_it(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    shown.show_of(8, Some(5));
    shown.calls();
    shown.keys("cmd-w");
    assert_eq!(shown.calls(), ["release", "hide", "close 8"]);
    shown.show_of(9, Some(5));
    shown.keys("escape");
    let calls = shown.calls();
    assert_eq!(&calls[calls.len() - 3..], ["release", "hide", "close 9"]);
    shown.show_of(10, Some(5));
    shown.keys("cmd-enter");
    let calls = shown.calls();
    assert_eq!(
        &calls[calls.len() - 4..],
        [
            "release",
            "keep 1 10 of archived 5",
            "hide",
            "kept archived 5 in 1"
        ]
    );
}

/// ⌘↩ grows it into the tab's place, then keeps its page as a Today tab, as it is.
#[gpui::test]
fn cmd_enter_keeps_it(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    shown.keys("cmd-enter");
    assert_eq!(shown.calls(), ["release", "keep 1 7", "hide"]);
}

#[gpui::test]
fn its_buttons_keep_and_close_it(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    shown.click(KEEP_BUTTON);
    assert_eq!(shown.calls(), ["release", "keep 1 7", "hide"]);
    shown.show(TAB);
    shown.calls();
    // The next preview's keep button came in under the pointer, which asked for its tooltip, and
    // the click on close ends it.
    shown.click(CLOSE_BUTTON);
    assert_eq!(
        shown.calls(),
        ["tooltip none", "release", "hide", "close 7", CLOSED]
    );
}

/// Each button's tooltip names it and its key, as written here, and hangs from it: from the
/// middle of its bottom edge, 97. It goes as the pointer leaves for the dim, a click or a key
/// ends it with the preview, and the site, which does nothing on a click, has none.
#[gpui::test]
fn its_buttons_name_themselves_and_their_keys(cx: &mut TestAppContext) {
    let keys = |keystroke| crate::shell::keystroke_keys(keystroke).concat();
    let keep = format!("tooltip Open as Tab {} under 679 97", keys("cmd-enter"));
    let close = format!("tooltip Close {} under 719 97", keys("escape"));
    if !crate::shell::OFF_THE_MAC {
        assert_eq!(keep, "tooltip Open as Tab ⌘↩ under 679 97");
        assert_eq!(close, "tooltip Close ⎋ under 719 97");
    }
    let mut shown = Shown::new(cx);
    shown.hover(KEEP_BUTTON);
    assert_eq!(shown.calls(), [keep.clone()]);
    // Onto the next button, its tooltip takes the other's place, which isn't ended under it.
    shown.hover(CLOSE_BUTTON);
    assert_eq!(shown.calls(), [close.clone()]);
    shown.hover(SITE);
    assert_eq!(shown.calls(), ["tooltip none"]);
    shown.hover(DIM_SPOT);
    assert_eq!(shown.calls(), Vec::<String>::new());
    shown.hover(KEEP_BUTTON);
    shown.click(KEEP_BUTTON);
    assert_eq!(
        shown.calls(),
        [keep.as_str(), "tooltip none", "release", "keep 1 7", "hide"]
    );
    shown.show(TAB);
    shown.calls();
    shown.hover(CLOSE_BUTTON);
    shown.keys("escape");
    assert_eq!(
        shown.calls(),
        [
            close.as_str(),
            "tooltip none",
            "release",
            "hide",
            "close 7",
            CLOSED
        ]
    );
}

/// A click on the dim closes it; one on the card or the site over it doesn't.
#[gpui::test]
fn a_click_outside_closes_it(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    shown.click((400., 300.));
    shown.click(SITE);
    assert_eq!(shown.calls(), Vec::<String>::new());
    shown.click(DIM_SPOT);
    assert_eq!(shown.calls(), ["release", "hide", "close 7", CLOSED]);
}

/// Keys from its page come here and nowhere else: its own act on it, and the window's don't act
/// on the window's tab. The core reports them as the tab its window shows, 3 here.
#[gpui::test]
fn its_pages_keys_stay_in_it(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    let route = |window, tab, keystroke: &str, reserved| {
        shown
            .cx
            .cx
            .update(|cx| key(window, tab, keystroke, reserved, cx))
    };
    // Another window's page keeps its keys.
    assert_eq!(route(WINDOW + 1, 4, "escape", true), None);
    assert_eq!(route(WINDOW, 3, "cmd-l", true), Some(false));
    assert_eq!(route(WINDOW, 3, "cmd-l", false), Some(false));
    // Esc closes it before the page sees it, as its own tab's or as the window's.
    assert_eq!(route(WINDOW, 3, "escape", true), Some(true));
    // Once it's leaving, the window's page has its keys back, and its own page's do nothing.
    assert_eq!(route(WINDOW, 3, "escape", true), None);
    assert_eq!(route(0, TAB, "cmd-enter", false), Some(false));
    assert_eq!(shown.calls(), ["release", "hide", "close 7", CLOSED]);
    assert_eq!(shown.cx.cx.update(|cx| tab(cx)), None);
    shown.show(TAB);
    shown.calls();
    assert_eq!(
        shown.cx.cx.update(|cx| key(0, TAB, "escape", true, cx)),
        Some(true)
    );
    assert_eq!(shown.calls(), ["release", "hide", "close 7", CLOSED]);
}

/// Redo closes the preview over its tab, which the window's history has already, and no other.
#[gpui::test]
fn redo_closes_it_over_its_tab(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    assert!(!shown.cx.cx.update(|cx| close_over(WINDOW, OVER + 1, cx)));
    assert!(!shown.cx.cx.update(|cx| close_over(WINDOW + 1, OVER, cx)));
    assert!(shown.cx.cx.update(|cx| close_over(WINDOW, OVER, cx)));
    assert_eq!(
        shown.calls(),
        ["release", "hide", "close 7", &format!("{CLOSED} undone")]
    );
}

/// A call's link opens as a tab, and the same sites' other pages preview.
#[test]
fn meeting_links_are_known_by_site_and_path() {
    for url in [
        "https://meet.google.com/abc-defg-hij",
        "http://meet.google.com:8080/abc-defg-hij?authuser=1",
        "https://us02web.zoom.us/j/123456789?pwd=x",
        "https://zoom.us/my/someone",
        "https://teams.microsoft.com/l/meetup-join/19%3ameeting/0",
        "https://teams.live.com/meet/9876",
        "https://acme.webex.com/meet/someone",
        "https://whereby.com/room",
        "https://meet.jit.si/SomeRoom",
    ] {
        assert!(is_meeting(url), "{url}");
    }
    for url in [
        "https://meet.google.com/",
        "https://zoom.us/pricing",
        "https://teams.microsoft.com/",
        "https://www.webex.com/pricing",
        "https://example.com/meet.google.com/abc",
        "https://notzoom.us/j/1",
        "https://www.google.com/search?q=meet.google.com",
    ] {
        assert!(!is_meeting(url), "{url}");
    }
}

/// A second link's preview takes the place of the first, whose page closes at once.
#[gpui::test]
fn a_second_preview_takes_the_firsts_place(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    shown.show(8);
    assert_eq!(
        shown.calls(),
        ["hide", "close 7", "place 8 296 106 438 428 r16", "show 1"]
    );
    assert_eq!(shown.cx.cx.update(|cx| tab(cx)), Some(8));
}

/// Its tab closing, as the page closing itself does, takes it away at once.
#[gpui::test]
fn its_tab_closing_takes_it_away(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    shown.cx.cx.update(|cx| tab_closed(3, cx));
    shown.cx.cx.update(|cx| tab_closed(TAB, cx));
    assert_eq!(shown.calls(), ["hide"]);
}

/// Its window resizing moves its page with the window's.
#[gpui::test]
fn it_follows_the_window(cx: &mut TestAppContext) {
    let mut shown = Shown::new(cx);
    shown
        .cx
        .cx
        .update(|cx| page_moved(2, (bounds(0., 0., 100., 100.), 6.), cx));
    shown
        .cx
        .cx
        .update(|cx| page_moved(WINDOW, (page(), 6.), cx));
    shown
        .cx
        .cx
        .update(|cx| page_moved(WINDOW, (bounds(240., 10., 750., 580.), 6.), cx));
    assert_eq!(shown.calls(), ["place 7 299 109 632 422 r16"]);
}
