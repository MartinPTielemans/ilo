//! The hover card: resting the pointer on a sidebar tab shows a card beside its row, with a picture
//! of the tab's page as it last looked, its title and host, and buttons that pin or unpin it, make
//! it a favorite and open it in a split view (#430; ours, from docs/ux-principles.md and the design
//! tokens). It crosses the sidebar's edge onto the page, which GPUI can't draw over, so as the
//! tooltip is, it's a window of its own that the host shows over the browser window, as its child,
//! and it never takes the keyboard. What it asks of its window goes through `Host`.
//!
//! The pictures are the core's (`bc_tab_picture`), taken after a tab is hidden, never on the switch
//! that hides it, at most twice the card's size, and held here in memory alone, by tab, until the
//! tab unloads or closes or the system is short of memory.

use std::{collections::HashMap, rc::Rc, sync::Arc, time::Duration};
use web_time::Instant;

use gpui::{
    App, BackgroundExecutor, Bounds, Context, DispatchPhase, FontWeight, Global, Hsla, Image,
    ImageSource, MouseExitEvent, MouseMoveEvent, ObjectFit, Pixels, RenderImage, Role,
    SharedString, Size, Task, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, canvas, div, img, point, prelude::*, px, size, svg,
};

use crate::elevation::{Level, WINDOW_MARGIN};
use crate::palette::{
    Palette, STATE_HOVERED, STATE_PRESSED, STATE_SELECTED, STATE_SELECTED_HOVERED,
};
use crate::shell::{PagePicture, cubic_bezier, scaled::scaled, spring};
use crate::tokens::hover_card::{
    BUTTON, BUTTON_GAP, BUTTON_INSET, BUTTON_RADIUS, FAVICON, GLYPH, GLYPH_ALPHA, GROUND, HOST,
    HOST_SIZE, PADDING, PAUSED_GAP, PICTURE_DEFAULT, PICTURE_LEAST, PICTURE_MOST, PICTURE_RADIUS,
    RADIUS, RIM, SPLIT_GAP, SPLIT_RING, TEXT_TOP, TEXT_X, TITLE, TITLE_LINE, TITLE_SIZE,
};
pub use crate::tokens::hover_card::{EDGE, GAP, WIDTH};

/// How long the pointer rests on a row before its card shows: Dia's, sooner than the tooltip's
/// delay (docs/ux-spec.md › Tooltip).
pub const DWELL: Duration = Duration::from_millis(500);
/// How long the card stays after the pointer leaves its row, for the pointer to cross the gap onto
/// it (ours).
pub const LEAVE: Duration = Duration::from_millis(150);
/// The picture's box is as wide as the card inside its padding.
pub const INNER: f32 = WIDTH - 2. * PADDING;
/// The pixels the core scales a picture down to fit: the picture's box at its tallest, at the
/// window's `scale`. One is kept for each hidden tab, so a 1× screen's are a quarter of a Retina
/// screen's, which needs the twice (#2138).
pub fn picture_pixels(scale: f32) -> (i32, i32) {
    let scale = scale.max(1.);
    let side = |points: f32| (points * scale).ceil() as i32;
    (side(INNER), side(INNER * PICTURE_MOST))
}
/// The card's window, as big as the card at its tallest and the margin its shadow draws in.
pub const WINDOW_WIDTH: f32 = WIDTH + 2. * WINDOW_MARGIN;
pub const WINDOW_HEIGHT: f32 = PADDING + INNER * PICTURE_MOST + TEXT + 2. * WINDOW_MARGIN;
/// Under the picture: the title's line, then the row of buttons with the host beside them, inset
/// from the card's bottom as from its right.
const TEXT: f32 = TEXT_TOP + TITLE_LINE + BUTTON + BUTTON_INSET;

/// Dia's motion (docs/ux-spec.md › Motion › Hover card in, out): the card grows from 0.92 of its
/// size on a spring of 0.25 s and ζ 0.75 as it fades in, and springs back to 0.92 on 0.2 s and
/// ζ 0.6 as it fades out, each fade over 100 ms on the same curve. Its window hides as the fade
/// out ends, since nothing of it shows after.
const FROM: f32 = 0.92;
const IN_RESPONSE: f32 = 0.25;
const IN_DAMPING: f32 = 0.75;
const OUT_RESPONSE: f32 = 0.2;
const OUT_DAMPING: f32 = 0.6;
pub const FADE: Duration = Duration::from_millis(100);

/// How big the card is drawn, from FROM to its size, and how opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    scale: f32,
    opacity: f32,
}

impl Pose {
    /// `seconds` after its window was first on screen.
    fn arriving(seconds: f32) -> Self {
        Pose {
            scale: FROM + (1. - FROM) * spring(seconds, IN_RESPONSE, IN_DAMPING).unwrap_or(1.),
            opacity: fade(seconds),
        }
    }

    /// `seconds` after it started going, from where its motion was then.
    fn going(self, seconds: f32) -> Self {
        let sprung = spring(seconds, OUT_RESPONSE, OUT_DAMPING).unwrap_or(1.);
        Pose {
            scale: self.scale + (FROM - self.scale) * sprung,
            opacity: self.opacity * (1. - fade(seconds)),
        }
    }
}

/// How far a fade is `seconds` in, from 0 to 1.
fn fade(seconds: f32) -> f32 {
    cubic_bezier(
        0.25,
        0.46,
        0.45,
        0.94,
        (seconds / FADE.as_secs_f32()).min(1.),
    )
}

/// The card is going: when it started, where its motion was then, and the timer that hides its
/// window once it has faded out.
struct Going {
    at: Instant,
    from: Pose,
    _hide: Task<()>,
}

/// What the card asks of its window.
pub trait Host {
    /// Shows the card's window with its plate at (`left`, `top`) in browser window `window`, the
    /// one whose row it's for, `width` by `height`, or moves it there if it shows. The UI has kept
    /// it inside the window. Only the plate takes the mouse; the margin round it lets it through.
    fn show(&self, window: u64, left: f32, top: f32, width: f32, height: f32);
    /// Hides it at once, once the UI has drawn it going.
    fn hide(&self);
}

/// A button on the card. Each makes the call the row's menu makes for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Pin,
    Unpin,
    Favorite,
    Unfavorite,
    Split,
    /// The page sharing another tab shares this one instead.
    ShareInstead,
}

impl Action {
    /// What assistive apps call it, as the row's menu does.
    pub fn name(self) -> &'static str {
        match self {
            Action::Pin => "Pin Tab",
            Action::Unpin => "Unpin Tab",
            Action::Favorite => "Add to Favorites",
            Action::Unfavorite => "Remove from Favorites",
            Action::Split => "Open in Split View",
            Action::ShareInstead => "Share This Tab Instead",
        }
    }

    /// How the log and the tests name it.
    pub fn label(self) -> &'static str {
        match self {
            Action::Pin => "pin",
            Action::Unpin => "unpin",
            Action::Favorite => "favorite",
            Action::Unfavorite => "unfavorite",
            Action::Split => "split",
            Action::ShareInstead => "share-instead",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Action::Pin | Action::Unpin => "icons/pin.svg",
            Action::Favorite | Action::Unfavorite => "icons/star.svg",
            Action::Split => "icons/columns-2.svg",
            Action::ShareInstead => "icons/screen-share.svg",
        }
    }

    /// Whether it undoes what the tab is, and shows that on, on the selected plate.
    fn on(self) -> bool {
        matches!(self, Action::Unpin | Action::Unfavorite)
    }
}

/// What stands for a tab with no picture: its favicon, a globe if it has none, or its emoji.
#[derive(Clone)]
pub enum Icon {
    Favicon(Option<Arc<Image>>),
    Emoji(SharedString),
}

/// What a card shows of a tab.
#[derive(Clone)]
pub struct Card {
    pub tab: u64,
    pub title: SharedString,
    pub host: SharedString,
    pub icon: Icon,
    /// Whether it has a picture's box: not for the tab shown, whose page is on screen.
    pub pictured: bool,
    /// Whether Battery Saver froze its page out of sight, which it says after the host.
    pub paused: bool,
    pub actions: Vec<Action>,
    /// A favorite split's pages, each with what stands for it without a picture, which its box
    /// shows side by side, else none for a tab alone; the one focused last, and its ring's colour.
    pub panes: Vec<(u64, Icon)>,
    pub focused: u64,
    pub ring: Hsla,
}

/// The row a card goes beside, in the browser window, and the window's size, which it stays
/// inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub row: Bounds<Pixels>,
    pub window: Size<Pixels>,
    /// Whether it opens on the row's left, toward the page from a sidebar on the right (#2375).
    pub left: bool,
}

/// What the card tells the shell that asked for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// It shows, so a loaded tab's picture can be taken again, as it is now.
    Shown,
    /// A button was pressed, and the card has gone.
    Act(Action),
    /// The pointer came onto the card, or left it, which a hidden sidebar's panel counts as its
    /// own (#830).
    Pointer(bool),
}

/// The pointer rests on a row: in which browser window, what its card shows, where, and who hears
/// what happens to it.
#[derive(Clone)]
pub struct Rest {
    /// The browser window whose sidebar has the row, which the card shows over (#1432).
    pub window: u64,
    pub card: Card,
    pub anchor: Anchor,
    pub events: Rc<dyn Fn(u64, Event, &mut App)>,
}

/// What the shell asks of the card.
#[derive(Clone)]
pub enum Hover {
    /// The pointer has come onto a tab's row.
    Rest(Rest),
    /// It has left that tab's row.
    Left(u64),
    /// What a tab's card shows changed, as a rename changes its name.
    Changed(Card),
    /// A press, a scroll, a drag, a menu, a Space switch or a key ends the card at once.
    Close,
}

impl Hover {
    /// How the recorder's log names it.
    pub fn label(&self) -> String {
        match self {
            Hover::Rest(rest) => {
                let card = &rest.card;
                let actions: Vec<&str> = card.actions.iter().map(|action| action.label()).collect();
                let picture = if card.pictured { "picture" } else { "none" };
                format!("card rest {} {} {picture}", card.tab, actions.join(" "))
            }
            Hover::Left(tab) => format!("card left {tab}"),
            Hover::Changed(card) => format!("card changed {} {}", card.tab, card.title),
            Hover::Close => "card close".into(),
        }
    }
}

/// How tall the picture's box is for a picture `size` pixels, else for none yet, or 0 for a card
/// without one: the page's shape, within the least and most shares of its width.
pub fn picture_height(pictured: bool, size: Option<(f32, f32)>) -> f32 {
    if !pictured {
        return 0.;
    }
    let share = size
        .filter(|&(width, _)| width > 0.)
        .map_or(PICTURE_DEFAULT, |(w, h)| h / w);
    INNER * share.clamp(PICTURE_LEAST, PICTURE_MOST)
}

/// How tall a card is with a picture's box `picture` tall.
pub fn height(picture: f32) -> f32 {
    match picture > 0. {
        true => PADDING + picture + TEXT,
        false => TEXT,
    }
}

/// Where a card `height` tall has its top left: GAP right of its row, or left of it toward a page on
/// the left, its top at the row's, and EDGE inside the window, the top and left first if it can't be
/// both.
pub fn place(anchor: Anchor, height: f32) -> (f32, f32) {
    let (width, tall) = (
        f32::from(anchor.window.width),
        f32::from(anchor.window.height),
    );
    let left = match anchor.left {
        true => f32::from(anchor.row.left()) - GAP - WIDTH,
        false => f32::from(anchor.row.right()) + GAP,
    };
    let top = f32::from(anchor.row.top());
    (
        left.min(width - WIDTH - EDGE).max(EDGE),
        top.min(tall - height - EDGE).max(EDGE),
    )
}

/// What the window draws, which stays as it fades out after the card went.
#[derive(Clone)]
struct Drawn {
    card: Card,
    picture: Option<PagePicture>,
    /// A split's pages' pictures, in its order.
    panes: Vec<Option<PagePicture>>,
    height: f32,
}

pub struct HoverCard {
    host: Rc<dyn Host>,
    /// The card that shows.
    shown: Option<Rest>,
    /// The row the pointer rests on whose card waits for its dwell.
    resting: Option<Rest>,
    /// The dwell's timer, or the leave's once the pointer has left the shown card's row. Dropping
    /// it cancels it.
    wait: Option<Task<()>>,
    /// The pointer is on the card, which keeps it.
    on_card: bool,
    drawn: Option<Drawn>,
    /// When the card shown was first on screen, where its motion starts.
    appeared: Option<Instant>,
    going: Option<Going>,
    /// It has come to rest, which it logs once.
    settled: bool,
    /// The tabs' pictures, and the tab each picture asked for is of.
    pictures: HashMap<u64, PagePicture>,
    asked: HashMap<u64, u64>,
    /// The pictures the window last drew, and those it has no more use for, which it lets go of.
    painted: Vec<Arc<RenderImage>>,
    stale: Vec<Arc<RenderImage>>,
    /// What it logged as drawn last, for check.sh.
    logged: Option<(u64, &'static str)>,
    /// GPUI's clock, which the card's motion is timed by: the real one in the app, and in a test
    /// one only the test moves.
    clock: BackgroundExecutor,
}

struct CardWindow(WindowHandle<HoverCard>);

impl Global for CardWindow {}

/// The pictures, and the requests out for them, from before the card's window is made the first
/// time a card shows (lazy_window). The window takes them.
#[derive(Default)]
struct Early {
    pictures: HashMap<u64, PagePicture>,
    asked: HashMap<u64, u64>,
}

impl Global for Early {}

/// Opens the card's window, hidden, for `host` to show over the browser window, ahead of time as
/// the tooltip's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<HoverCard>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
        ))),
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
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let early = match cx.has_global::<Early>() {
        true => cx.remove_global::<Early>(),
        false => Early::default(),
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|cx| HoverCard {
            host,
            shown: None,
            resting: None,
            wait: None,
            on_card: false,
            drawn: None,
            appeared: None,
            going: None,
            settled: false,
            pictures: early.pictures,
            asked: early.asked,
            painted: Vec::new(),
            stale: Vec::new(),
            logged: None,
            clock: cx.background_executor().clone(),
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(CardWindow(window));
            // Any key ends it, as in Dia, in whichever of the app's windows has the keyboard. The
            // page's keys come to `close` from the core.
            cx.intercept_keystrokes(|_, _, cx| close(cx)).detach();
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the hover card's window: {error}");
            None
        }
    }
}

fn update(cx: &mut App, f: impl FnOnce(&mut HoverCard, &mut Context<HoverCard>)) {
    if let Some(window) = cx.try_global::<CardWindow>().map(|card| card.0) {
        window.update(cx, |card, _, cx| f(card, cx)).ok();
    }
}

/// What the shell asks: a card for the row the pointer rests on, after DWELL, or at once while
/// one shows, as the pointer moves from row to row; the pointer leaving that row, which ends it
/// after LEAVE unless it's onto the card; what a tab's card shows, as it changes; or the card's
/// end.
pub fn hover(hover: Hover, cx: &mut App) {
    if matches!(hover, Hover::Rest(_)) {
        crate::lazy_window::ready::<HoverCard>(cx);
    }
    update(cx, |card, cx| match hover {
        Hover::Rest(rest) => card.rest(rest, cx),
        Hover::Left(tab) => card.left(tab, cx),
        Hover::Changed(changed) => card.changed(changed, cx),
        Hover::Close => card.close(cx),
    });
}

/// The window lost the keyboard, the app went to the back, or a key went down in the page: the
/// card goes.
pub fn close(cx: &mut App) {
    hover(Hover::Close, cx);
}

/// The card's requests for pictures have this bit, so the answer finds its way here before it
/// reaches the windows, whose own requests never have it.
const CARD_REQUEST: u64 = 1 << 63;

/// Whether `request` is the card's.
pub fn is_card_request(request: u64) -> bool {
    request & CARD_REQUEST != 0
}

/// The card asks the core for a picture of `tab` with `request`, which this returns.
pub fn asking(tab: u64, cx: &mut App) -> u64 {
    let request = crate::shell::next_request() | CARD_REQUEST;
    match cx.has_global::<CardWindow>() {
        true => update(cx, |card, _| {
            card.asked.insert(request, tab);
        }),
        false => {
            cx.default_global::<Early>().asked.insert(request, tab);
        }
    }
    request
}

/// The core's answer to the card's `request`. A picture replaces the tab's last; none keeps it.
pub fn answered(request: u64, picture: Option<PagePicture>, cx: &mut App) {
    if !cx.has_global::<CardWindow>() {
        let early = cx.default_global::<Early>();
        keep(&mut early.asked, &mut early.pictures, request, picture);
        return;
    }
    update(cx, |card, cx| {
        let tab = keep(&mut card.asked, &mut card.pictures, request, picture);
        if tab.is_some_and(|tab| card.shown.as_ref().is_some_and(|rest| rest.card.tab == tab)) {
            card.draw(cx);
        }
    });
}

/// Keeps the picture that answers `request`, returning the tab it's of.
fn keep(
    asked: &mut HashMap<u64, u64>,
    pictures: &mut HashMap<u64, PagePicture>,
    request: u64,
    picture: Option<PagePicture>,
) -> Option<u64> {
    let tab = asked.remove(&request)?;
    let Some(picture) = picture else {
        // check.sh reads this line.
        log!("browser: hover card not pictured {tab}");
        return None;
    };
    pictures.insert(tab, picture);
    // check.sh reads this line.
    log!("browser: hover card pictured {tab}");
    Some(tab)
}

/// `tab`'s page unloaded or closed: its picture goes.
pub fn forget(tab: u64, cx: &mut App) {
    if cx.has_global::<Early>() {
        cx.global_mut::<Early>().pictures.remove(&tab);
    }
    update(cx, |card, cx| {
        if card.pictures.remove(&tab).is_some() {
            card.let_go(cx);
        }
    });
}

/// The system is short of memory: every picture goes.
pub fn forget_all(cx: &mut App) {
    if cx.has_global::<Early>() {
        let early = cx.global_mut::<Early>();
        let count = early.pictures.len();
        early.pictures.clear();
        log!("browser: hover card pictures dropped {count}");
    }
    update(cx, |card, cx| {
        let count = card.pictures.len();
        card.pictures.clear();
        card.let_go(cx);
        log!("browser: hover card pictures dropped {count}");
    });
}

impl HoverCard {
    fn rest(&mut self, rest: Rest, cx: &mut Context<Self>) {
        if self.shown.is_some() {
            self.wait = None;
            self.resting = None;
            return self.show(rest, cx);
        }
        let tab = rest.card.tab;
        let waiting = self
            .resting
            .as_ref()
            .is_some_and(|resting| resting.card.tab == tab);
        self.resting = Some(rest);
        if waiting {
            return;
        }
        self.wait = Some(cx.spawn(async move |card, cx| {
            cx.background_executor().timer(DWELL).await;
            card.update(cx, |card, cx| {
                card.wait = None;
                if let Some(rest) = card.resting.take() {
                    card.show(rest, cx)
                }
            })
            .ok();
        }));
    }

    fn left(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self
            .resting
            .as_ref()
            .is_some_and(|rest| rest.card.tab == tab)
        {
            self.resting = None;
            self.wait = None;
        }
        if self.shown.as_ref().is_some_and(|rest| rest.card.tab == tab) && !self.on_card {
            self.leave_after(cx);
        }
    }

    /// The tab of the card that shows, or waits for its dwell, changed: it shows it as it is now,
    /// not as it was when the pointer came onto its row (#1452).
    fn changed(&mut self, card: Card, cx: &mut Context<Self>) {
        let tab = card.tab;
        if let Some(resting) = self.resting.as_mut().filter(|rest| rest.card.tab == tab) {
            resting.card = card.clone();
        }
        if let Some(shown) = self.shown.as_mut().filter(|rest| rest.card.tab == tab) {
            shown.card = card;
            self.draw(cx);
        }
    }

    /// The card goes after LEAVE, unless the pointer comes onto it or another row first.
    fn leave_after(&mut self, cx: &mut Context<Self>) {
        self.wait = Some(cx.spawn(async move |card, cx| {
            cx.background_executor().timer(LEAVE).await;
            card.update(cx, |card, cx| {
                card.wait = None;
                card.close(cx)
            })
            .ok();
        }));
    }

    /// The pointer moved on the card's plate, or off it. Only leaving the card lets it go: a move
    /// off a plate it never came onto is the pointer resting on the row.
    fn on_card(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let was = self.on_card;
        self.pointer_on(hovered && self.shown.is_some(), cx);
        match hovered {
            true if self.shown.is_some() => self.wait = None,
            false if was && self.shown.is_some() => self.leave_after(cx),
            _ => {}
        }
    }

    /// Whether the pointer is on the card, which its shell hears as it changes.
    fn pointer_on(&mut self, on: bool, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.on_card, on) == on {
            return;
        }
        // The buttons light under the pointer only while it's on the card.
        cx.notify();
        if let Some(rest) = &self.shown {
            (rest.events)(rest.card.tab, Event::Pointer(on), cx);
        }
    }

    fn show(&mut self, rest: Rest, cx: &mut Context<Self>) {
        // Back before its window has hidden, it comes in anew.
        if self.going.take().is_some() {
            self.appeared = None;
            self.settled = false;
        }
        let (tab, events) = (rest.card.tab, rest.events.clone());
        let new = self
            .shown
            .as_ref()
            .is_none_or(|shown| shown.card.tab != tab);
        self.shown = Some(rest);
        let picture = self.draw(cx);
        if new {
            let actions: Vec<&str> = (self.shown.iter().flat_map(|rest| &rest.card.actions))
                .map(|action| action.label())
                .collect();
            // check.sh reads this line.
            log!(
                "browser: hover card shown {tab} {} {picture}",
                actions.join(" ")
            );
            if self.shown.as_ref().is_some_and(|rest| rest.card.paused) {
                // check.sh reads this line.
                log!("browser: hover card {tab} says paused");
            }
            events(tab, Event::Shown, cx);
        }
    }

    /// Lays out the card that shows, with its tab's picture if it has one, and shows its window
    /// there. Returns what the picture's box holds.
    fn draw(&mut self, cx: &mut Context<Self>) -> &'static str {
        let Some(rest) = &self.shown else {
            return "none";
        };
        let (window, card) = (rest.window, rest.card.clone());
        let split = !card.panes.is_empty();
        let picture = (card.pictured && !split)
            .then(|| self.pictures.get(&card.tab).cloned())
            .flatten();
        let panes: Vec<_> = (card.panes.iter())
            .map(|(tab, _)| {
                card.pictured
                    .then(|| self.pictures.get(tab).cloned())
                    .flatten()
            })
            .collect();
        // A split's box keeps the default shape, as its pages are of more than one.
        let height = height(picture_height(
            card.pictured,
            picture.as_ref().map(|p| p.size()),
        ));
        let (left, top) = place(rest.anchor, height);
        // check.sh reads this line, to see the card is beside its row.
        log!("browser: hover card at {left} {top} {WIDTH} {height}");
        let pictured = picture.is_some() || panes.iter().any(Option::is_some);
        let holds = match (pictured, card.pictured) {
            (true, _) => "picture",
            (false, true) => "favicon",
            (false, false) => "none",
        };
        self.drawn = Some(Drawn {
            card,
            picture,
            panes,
            height,
        });
        self.host.show(window, left, top, WIDTH, height);
        cx.notify();
        holds
    }

    /// The card goes: it fades out, and its window hides after.
    fn close(&mut self, cx: &mut Context<Self>) {
        self.wait = None;
        self.resting = None;
        self.pointer_on(false, cx);
        if self.shown.is_none() {
            return;
        }
        let from = self.pose();
        self.shown = None;
        let hide = cx.spawn(async move |card, cx| {
            cx.background_executor().timer(FADE).await;
            card.update(cx, |card, cx| card.hide(cx)).ok();
        });
        self.going = Some(Going {
            at: self.clock.now(),
            from,
            _hide: hide,
        });
        cx.notify();
    }

    fn hide(&mut self, cx: &mut Context<Self>) {
        self.going = None;
        self.appeared = None;
        self.settled = false;
        self.host.hide();
        // check.sh reads this line.
        log!("browser: hover card hidden");
        cx.notify();
    }

    /// Seconds since `at` on the card's clock.
    fn since(&self, at: Instant) -> f32 {
        self.clock.now().saturating_duration_since(at).as_secs_f32()
    }

    fn pose(&self) -> Pose {
        match &self.going {
            Some(going) => going.from.going(self.since(going.at)),
            None => Pose::arriving(self.appeared.map_or(0., |at| self.since(at))),
        }
    }

    /// Whether the card's motion has more to draw: until its window is on screen, then as it
    /// comes in, and while it goes.
    fn moving(&self) -> bool {
        match (&self.going, self.appeared) {
            (Some(_), _) => true,
            (None, _) if self.shown.is_none() => false,
            (None, None) => true,
            (None, Some(at)) => spring(self.since(at), IN_RESPONSE, IN_DAMPING).is_some(),
        }
    }

    fn press(&mut self, action: Action, cx: &mut Context<Self>) {
        let Some(rest) = self.shown.clone() else {
            return;
        };
        // check.sh reads this line.
        log!("browser: hover card {} {}", action.label(), rest.card.tab);
        self.close(cx);
        (rest.events)(rest.card.tab, Event::Act(action), cx);
    }

    /// A picture it holds went: the window lets go of the one it drew, if it's gone, and draws the
    /// card again without it.
    fn let_go(&mut self, cx: &mut Context<Self>) {
        let held = |image: &Arc<RenderImage>| {
            self.pictures
                .values()
                .any(|picture| Arc::ptr_eq(&picture.image(), image))
        };
        let (kept, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut self.painted)
            .into_iter()
            .partition(|image| held(image));
        self.painted = kept;
        self.stale.extend(gone);
        if self.shown.is_some() {
            self.draw(cx);
        } else if let Some(drawn) = &mut self.drawn {
            drawn.picture = None;
            drawn.panes.iter_mut().for_each(|pane| *pane = None);
        }
        cx.notify();
    }
}

impl Render for HoverCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pictures: Vec<Arc<RenderImage>> = (self.drawn.iter())
            .flat_map(|drawn| drawn.picture.iter().chain(drawn.panes.iter().flatten()))
            .map(|p| p.image())
            .collect();
        let same = |a: &[Arc<RenderImage>], b: &[Arc<RenderImage>]| {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| Arc::ptr_eq(a, b))
        };
        // The card's pictures alone are in the atlas: one, or a split's pages'.
        if !same(&self.painted, &pictures) {
            let before = std::mem::replace(&mut self.painted, pictures);
            let kept =
                |image: &Arc<RenderImage>| self.painted.iter().any(|p| Arc::ptr_eq(p, image));
            self.stale
                .extend(before.into_iter().filter(|image| !kept(image)));
        }
        for image in self.stale.drain(..) {
            window.drop_image(image).ok();
        }
        // The plate is inset by the margin, which its shadow draws in.
        let root = div()
            .size_full()
            .p(px(WINDOW_MARGIN))
            .font_family(crate::face::UI);
        let Some(drawn) = self.drawn.clone() else {
            return root;
        };
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it, so the motion starts there rather than skip its start. Until then it's
        // drawn faded out, so the host's drawing before it shows has nothing of the last card.
        if self.shown.is_some() && self.appeared.is_none() && window.is_visible() {
            self.appeared = Some(self.clock.now());
        }
        if self.moving() {
            window.request_animation_frame();
        } else if self.shown.is_some() && !self.settled {
            self.settled = true;
            // check.sh reads this line, so a capture after it shows the card whole.
            log!("browser: hover card settled");
        }
        let pose = self.pose();
        let pictured = drawn.picture.is_some() || drawn.panes.iter().any(Option::is_some);
        let holds = match (pictured, drawn.card.pictured) {
            (true, _) => "picture",
            (false, true) => "favicon",
            (false, false) => "none",
        };
        if self.logged != Some((drawn.card.tab, holds)) {
            self.logged = Some((drawn.card.tab, holds));
            // check.sh reads this line.
            log!("browser: hover card drawn {} {holds}", drawn.card.tab);
        }
        let palette = crate::palette::current(cx);
        let a11y = window.is_a11y_active();
        let on_card = self.on_card;
        let card = &drawn.card;
        let picture_h = picture_height(card.pictured, drawn.picture.as_ref().map(|p| p.size()));
        let text_top = if picture_h > 0. {
            PADDING + picture_h
        } else {
            0.
        };
        let picture = (picture_h > 0.).then(|| {
            let frame = div()
                .absolute()
                .left(px(PADDING))
                .top(px(PADDING))
                .w(px(INNER))
                .h(px(picture_h))
                .rounded(px(PICTURE_RADIUS))
                .overflow_hidden()
                .bg(palette.plate(GROUND));
            if !card.panes.is_empty() {
                return frame.child(split_pictures(&palette, card, &drawn.panes, picture_h));
            }
            match &drawn.picture {
                // A page of another shape than the box is cut to it about its middle.
                Some(picture) => frame.child(
                    img(ImageSource::Render(picture.image()))
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .rounded(px(PICTURE_RADIUS)),
                ),
                None => frame
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(&palette, &card.icon)),
            }
        });
        let title = div()
            .absolute()
            .left(px(TEXT_X))
            .top(px(text_top + TEXT_TOP))
            .w(px(WIDTH - 2. * TEXT_X))
            .h(px(TITLE_LINE))
            .truncate()
            .text_size(px(TITLE_SIZE))
            .line_height(px(TITLE_LINE))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.on_overlay(TITLE))
            .child(card.title.clone());
        let row_top = text_top + TEXT_TOP + TITLE_LINE;
        let count = card.actions.len() as f32;
        let buttons_width = count * BUTTON + (count - 1.).max(0.) * BUTTON_GAP;
        let host = div()
            .absolute()
            .left(px(TEXT_X))
            .top(px(row_top))
            .w(px((WIDTH
                - TEXT_X
                - BUTTON_INSET
                - buttons_width
                - BUTTON_GAP)
                .max(0.)))
            .h(px(BUTTON))
            .flex()
            .items_center()
            .text_size(px(HOST_SIZE))
            .text_color(palette.on_overlay(HOST))
            // Paused, the words take the host's place, which the row under the title still shows:
            // beside the buttons there's no room for both.
            .when(!card.paused, |host| {
                host.child(div().truncate().child(card.host.clone()))
            })
            .when(card.paused, |host| {
                host.child(
                    svg()
                        .path("icons/pause.svg")
                        .flex_none()
                        .mr(px(PAUSED_GAP))
                        .size(px(HOST_SIZE))
                        .text_color(palette.on_overlay(HOST)),
                )
                .child(div().truncate().child(crate::shell::PAUSED))
            });
        let buttons = div()
            .absolute()
            .right(px(BUTTON_INSET))
            .top(px(row_top))
            .flex()
            .gap(px(BUTTON_GAP))
            .children(
                (card.actions.iter()).map(|&action| button(&palette, action, a11y, on_card, cx)),
            );
        // The rim is its own layer, so the border doesn't shift what the fill's box holds.
        let rim = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(RADIUS))
            .border(px(RIM))
            .border_color(Level::Popover.rim_in(&palette));
        // Whether the pointer is on the plate, from its moves and its leaving the window alone:
        // GPUI's hover tests, at each paint, the last place the window saw the pointer, which
        // after a click is the button, wherever the pointer has gone since (#818).
        let plate = Bounds::new(corner_at(), size(px(WIDTH), px(drawn.height)));
        let card_entity = cx.entity().downgrade();
        let pointer = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let moved = card_entity.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        let on = plate.contains(&event.position);
                        moved.update(cx, |card, cx| card.on_card(on, cx)).ok();
                    }
                });
                let left = card_entity.clone();
                window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        left.update(cx, |card, cx| card.on_card(false, cx)).ok();
                    }
                });
            },
        )
        .absolute()
        .size_full();
        // It grows about its top left corner, which its row is beside.
        let corner = corner_at();
        root.child(pointer).child(scaled(
            div()
                .id("hover-card")
                .when(a11y, |plate| {
                    plate.role(Role::Group).aria_label(card.title.clone())
                })
                .relative()
                .w(px(WIDTH))
                .h(px(drawn.height))
                .rounded(px(RADIUS))
                .shadow(Level::Popover.shadow())
                .bg(palette.overlay)
                .opacity(pose.opacity)
                .children(picture)
                .child(title)
                .child(host)
                .child(buttons)
                .child(rim),
            pose.scale,
            corner,
        ))
    }
}

/// The plate's top left corner in its window.
fn corner_at() -> gpui::Point<Pixels> {
    point(px(WINDOW_MARGIN), px(WINDOW_MARGIN))
}

/// What stands for a tab with no picture, at the favicon's size.
/// A favorite split's pages in its card's picture box `height` tall, side by side as the window
/// shows them, each its picture cut to its share or its icon on the ground, and the page focused
/// last ringed in the Space's colour, so the card says which page a click shows focused.
fn split_pictures(
    palette: &Palette,
    card: &Card,
    pictures: &[Option<PagePicture>],
    height: f32,
) -> impl IntoElement + use<> {
    let count = card.panes.len().max(1) as f32;
    let width = (INNER - SPLIT_GAP * (count - 1.)) / count;
    div()
        .absolute()
        .inset_0()
        .flex()
        .gap(px(SPLIT_GAP))
        .children(
            card.panes
                .iter()
                .zip(pictures)
                .map(|((tab, each), picture)| {
                    let page = div()
                        .relative()
                        .w(px(width))
                        .h(px(height))
                        .flex_none()
                        .rounded(px(PICTURE_RADIUS))
                        .overflow_hidden()
                        .bg(palette.plate(GROUND));
                    let page = match picture {
                        Some(picture) => page.child(
                            img(ImageSource::Render(picture.image()))
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        ),
                        None => page
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(palette, each)),
                    };
                    page.when(*tab == card.focused, |page| {
                        page.child(
                            div()
                                .absolute()
                                .inset_0()
                                .rounded(px(PICTURE_RADIUS))
                                .border(px(SPLIT_RING))
                                .border_color(card.ring),
                        )
                    })
                }),
        )
}

fn icon(palette: &Palette, icon: &Icon) -> gpui::AnyElement {
    match icon {
        Icon::Favicon(Some(favicon)) => img(favicon.clone()).size(px(FAVICON)).into_any_element(),
        Icon::Favicon(None) => svg()
            .path("icons/globe.svg")
            .size(px(FAVICON))
            .text_color(palette.on_overlay_glyph(GLYPH_ALPHA))
            .into_any_element(),
        Icon::Emoji(emoji) => div()
            .text_size(px(FAVICON))
            .line_height(px(FAVICON))
            .child(emoji.clone())
            .into_any_element(),
    }
}

/// A button on the card: its glyph on a control's plate, which an action the tab has shows on.
fn button(
    palette: &Palette,
    action: Action,
    a11y: bool,
    on_card: bool,
    cx: &mut Context<HoverCard>,
) -> impl IntoElement + use<> {
    let (rest, hovered) = match action.on() {
        true => (STATE_SELECTED, STATE_SELECTED_HOVERED),
        false => (0., STATE_HOVERED),
    };
    let palette = *palette;
    div()
        .id(action.label())
        .when(a11y, |button| {
            button.role(Role::Button).aria_label(action.name())
        })
        .size(px(BUTTON))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(BUTTON_RADIUS))
        .bg(palette.state(0., rest))
        // Only with the pointer on the card, as GPUI's hover is the last place the window saw it.
        .when(on_card, |button| {
            button.hover(move |style| style.bg(palette.state(0., hovered)))
        })
        .active(move |style| style.bg(palette.state(0., STATE_PRESSED)))
        .on_click(cx.listener(move |card, _, _, cx| card.press(action, cx)))
        .child(
            svg()
                .path(action.glyph())
                .size(px(GLYPH))
                .text_color(palette.on_overlay_glyph(GLYPH_ALPHA)),
        )
}

#[cfg(test)]
mod tests;
