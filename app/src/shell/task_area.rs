//! A task the user asked from ⌘T (docs/ux-spec.md › Spaces › Agent task). Its Space is an
//! agent's that the window never lists as a Space, and the page area shows the task in place of
//! the Space's tab while the sidebar, the Space, the selection and the URL field stay the user's.
//! First its conversation, a view the task draws, in the card's place at once, as a tab switch
//! is (docs/ux-principles.md › A switch stays instant), the page laid out out of the window. Then,
//! from a thumbnail in it, one of the task's pages, which zooms out of the thumbnail to fill the
//! area and shrinks back into it, with the Agent Space's bar (#365) laid under it as a strip, as
//! GPUI can't draw over a page. The core shows the task's tab only while its page is watched, and
//! the user's again as it goes. Where the core shows pages in tiles (the Mac), a watched page is
//! its tile grown to the area instead, at its own size, and the window shows the task's tab only
//! once the user holds it (#3788). Nothing here runs for a window that shows no task.

use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use std::f32::consts::FRAC_PI_2;

use gpui::{
    AnyElement, AnyView, Bounds, Context, Corners, FontWeight, Hsla, MouseButton, Pixels, Radians,
    SharedString, Size, Task, Transformation, Window, canvas, div, point, prelude::*, px, rgb,
    size, svg,
};

use super::{
    CARD_INSET, Command, DialogKind, Folders, NAV_RADIUS, PagePicture, Palette, SECONDARY,
    SIDEBAR_WIDTH, Shell, Space, SpaceEntry, TITLE, Theme,
    agent_bar::{Bar, bar_words, stop_keys},
    card,
    conversation::{page_chip, page_chip_keyed},
    cubic_bezier,
    keys::user_keys,
    live_tiles::{LiveTiles, TILE_RADIUS, page_in},
    permission::{Look, card_listener},
    picture::next_request,
    task_cards::{PageWord, act},
};
use crate::agents::task::{Card, Dot, TaskAction, status_lead};
use crate::link_preview::{CLOSE, OPEN};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::tokens::shell::agent_pointer::{COLOR, RING};
use crate::tokens::shell::conversation::ilo::ARM;
use crate::tokens::shell::permission::LINE;
use crate::tokens::shell::task::STATUS;
use crate::tokens::shell::task_area::{
    BACK, BACK_GLYPH, BESIDE, BESIDE_GAP, GAP, PAGE_ROOM, PRIMARY_KEY, STRIP,
};
use crate::tokens::text;
use crate::tokens::toast::{CHIP_FILL, CHIP_GAP, CHIP_GLYPH, CHIP_HEIGHT, CHIP_RADIUS, CHIP_X};

use super::page_ends::StripStop;

/// What the page area shows of a task, at most one per window.
pub(super) struct TaskArea {
    space: u64,
    /// The conversation, which the task draws.
    view: AnyView,
    page: Option<Watched>,
    /// A page of the task's that came to need the user while the one watched went back, which ilo
    /// opens once that one is in its thumbnail, never over it on its way.
    opens: Option<u64>,
}

/// A task's view as it stood when the user asked the window to close or the browser to quit,
/// which a page of theirs may still refuse (#4202). The core never says a close was refused: the
/// page's question answered with staying is that word, and brings the view back.
pub(super) struct Closing {
    /// The page watched then, which went back to the conversation for the close.
    watched: Option<Watched>,
    /// The area, out of the window while a page of the user's asks whether to leave it: no
    /// page's dialog is drawn over a task's conversation.
    aside: Option<TaskArea>,
}
/// How long, in seconds, a page going back waits for a picture of itself as it is to shrink from,
/// as the one it came out of its thumbnail with is the conversation's, smaller and older: past it,
/// it goes back with that one. The core answers in a frame or two where the page draws.
pub(super) const PICTURE_WAIT: f32 = 0.1;

/// A task's page watched in the page area.
struct Watched {
    tab: u64,
    /// The thumbnail it zoomed out of, which it shrinks back into.
    thumb: Bounds<Pixels>,
    picture: Option<PagePicture>,
    /// Whether `picture` is one this window asked for, and so its own to let go of.
    asked: bool,
    zoom: Zoom,
    /// Its tile, where the core shows the page in one: the page is never the window's while it's
    /// only watched (#3788).
    tile: Option<WatchTile>,
    /// Whether ilo opened it by itself, as at a handoff, rather than the user: it never takes the
    /// keyboard, and until ARM has passed (`guarded`) it stays out of the window behind its
    /// picture, so a click meant for what stood there a moment ago never reaches the page.
    by_ilo: bool,
    /// Held behind its picture since ilo opened it, till a timer of ARM lets it in place.
    guarded: bool,
    /// While it waits to go back for a picture of itself as it is: the one wait that bounds it,
    /// which the answer ends by dropping it.
    waiting: Option<Task<()>>,
}

/// A page watched as its tile grown to the area: it keeps its own size, takes no input and hears
/// nothing of where it shows, until the user takes it over and it's the window's page, the user's
/// tab. Handed back, it's a tile again.
struct WatchTile {
    /// The conversation's tiles, which lent it and take it back as the watch ends.
    tiles: Rc<LiveTiles>,
    /// Whether the core shows the page in it now, rather than as the window's page.
    shown: bool,
    /// Where it was put last, and its corners' radius.
    placed: Option<(Bounds<Pixels>, f32)>,
    /// When a press on it took the page over. It stays a tile till ARM after, so the press and any
    /// that follow it, a double-click's second, land on nothing the agent left under the pointer.
    pressed: Option<Instant>,
    /// When the agent made the page the user's while it was watched, at a handoff or an ask. It
    /// stays a tile till ARM after, so a click meant for what stood there reaches nothing, then
    /// it's the window's page with the keyboard left where it was: the user's first click in the
    /// page gives it the keyboard.
    handed: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Zoom {
    /// Out of the thumbnail since then, the page laid out out of the window at its size meanwhile.
    Out(Instant),
    /// In place, filling the area.
    Rest,
    /// Going back, waiting for the picture asked with this request, of the page as it is.
    Asked(u64),
    /// Into the thumbnail since then, the Space's page shown again out of the window.
    In(Instant),
}

impl Zoom {
    /// How far the page has come out of its thumbnail at `now`: 0 in it, 1 filling the area.
    fn at(self, now: Instant) -> f32 {
        let seconds = |start: Instant| now.saturating_duration_since(start).as_secs_f32();
        match self {
            Zoom::Out(start) => cubic_bezier(0., 0., 0.58, 1., (seconds(start) / OPEN).min(1.)),
            Zoom::Rest | Zoom::Asked(_) => 1.,
            Zoom::In(start) => {
                1. - cubic_bezier(0.42, 0., 1., 1., (seconds(start) / CLOSE).min(1.))
            }
        }
    }
}

/// The size a window opens at by default, the app's (window_opening.rs, WINDOW_SIZE), which a test
/// reads there.
const DEFAULT_WINDOW: (f32, f32) = (1280., 800.);

/// An agent's page's own size, which the core gives it (core/window.cc, kAgentPageSize) and a tile
/// never changes: the area a page watched has in a window at its default size, so a watch there
/// shows it at 1:1 (#3788).
pub(super) fn agent_page_size() -> Size<Pixels> {
    size(
        px(DEFAULT_WINDOW.0 - SIDEBAR_WIDTH - CARD_INSET),
        px(DEFAULT_WINDOW.1 - 2. * CARD_INSET - PAGE_ROOM),
    )
}

/// Where an agent's page shows whole in `area`: at its own shape, by the one factor that fits it
/// both ways and never past its own size, its top at the area's and centred across.
fn fitted(area: Bounds<Pixels>) -> Bounds<Pixels> {
    let own = agent_page_size();
    let scale = (f32::from(area.size.width) / f32::from(own.width))
        .min(f32::from(area.size.height) / f32::from(own.height))
        .min(1.);
    let shown = size(own.width * scale, own.height * scale);
    Bounds::new(
        point(
            area.left() + (area.size.width - shown.width) / 2.,
            area.top(),
        ),
        shown,
    )
}

/// Where a watched page goes in the card: its top, the card's width, with room under it for the
/// strip.
fn watched(card: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        card.origin,
        size(card.size.width, card.size.height - px(PAGE_ROOM)),
    )
}

/// The card split for standing on a page with the conversation beside it: the page's part on its
/// left, and the conversation's panel at its right.
fn split(card: Bounds<Pixels>) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let wide = px(BESIDE.min(f32::from(card.size.width) / 2.));
    let page = Bounds::new(
        card.origin,
        size(card.size.width - wide - px(BESIDE_GAP), card.size.height),
    );
    let panel = Bounds::new(
        point(card.right() - wide, card.top()),
        size(wide, card.size.height),
    );
    (page, panel)
}

fn pushed(bounds: Bounds<Pixels>, by: Pixels) -> Bounds<Pixels> {
    Bounds::new(point(bounds.left() + by, bounds.top()), bounds.size)
}

fn between(from: Bounds<Pixels>, to: Bounds<Pixels>, way: f32) -> Bounds<Pixels> {
    let at = |from: Pixels, to: Pixels| from + (to - from) * way;
    Bounds::new(
        point(at(from.left(), to.left()), at(from.top(), to.top())),
        size(
            at(from.size.width, to.size.width),
            at(from.size.height, to.size.height),
        ),
    )
}

/// Where a picture of `picture`'s size goes to fill `bounds`, as wide as them at least, its top at
/// theirs, as a thumbnail shows a page's top.
pub(in crate::shell) fn covering(bounds: Bounds<Pixels>, picture: (f32, f32)) -> Bounds<Pixels> {
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let zoom = (width / picture.0).max(height / picture.1);
    let (drawn_width, drawn_height) = (picture.0 * zoom, picture.1 * zoom);
    Bounds::new(
        point(bounds.left() + px((width - drawn_width) / 2.), bounds.top()),
        size(px(drawn_width), px(drawn_height)),
    )
}

impl Shell {
    /// The agents' Spaces apart from the user's: a task's, and an outside agent's, which its
    /// sessions share (#1080). The window never lists them. Each keeps the tabs it had.
    pub(super) fn take_task_spaces(&mut self, spaces: Vec<SpaceEntry>) -> Vec<SpaceEntry> {
        let (tasks, spaces): (Vec<_>, Vec<_>) =
            spaces.into_iter().partition(|entry| !entry.5.is_empty());
        let mut old = std::mem::take(&mut self.task_spaces);
        self.task_spaces = tasks
            .into_iter()
            .map(|(id, name, color, profile, icon, agent, theme)| {
                match old.iter().position(|space| space.id == id) {
                    Some(at) => Space {
                        name,
                        agent,
                        ..old.swap_remove(at)
                    },
                    None => {
                        // check.sh reads this line.
                        log!("browser: task space {id} {agent}");
                        Space {
                            id,
                            name,
                            color,
                            theme: Theme::parse(&theme),
                            transparency: crate::space_transparency::read(&theme),
                            icon,
                            profile,
                            agent,
                            pinned: Vec::new(),
                            today: Vec::new(),
                            splits: Vec::new(),
                            folders: Folders::default(),
                            shown: 0,
                            peeked: Vec::new(),
                            live_rooms: Default::default(),
                            placed: Vec::new(),
                        }
                    }
                }
            })
            .collect();
        spaces
    }

    /// The task Spaces changed: one the page area shows that's gone takes the area with it.
    /// A task's Space the window shows, as one a launch brings back, gives way to the user's.
    pub(super) fn task_spaces_changed(&mut self, cx: &mut Context<Self>) {
        if self.area_takes_space() {
            self.reshape_cards(cx);
        }
        let area = self.task_area.as_ref();
        let gone = area.is_some_and(|area| self.space_went(area.space));
        if gone {
            self.leave_task_area(cx);
        }
        if self.task_core == 0 && self.task_space(self.space).is_some() {
            self.show_users_space(cx);
        }
        self.show_started_task(None, cx);
    }

    pub(super) fn task_space(&self, space: u64) -> Option<&Space> {
        self.task_spaces.iter().find(|each| each.id == space)
    }

    /// Whether `space` was a task's and is gone. A task a run before left has none, 0, until its
    /// run opens a page, and one with nowhere to work never has: its conversation shows without.
    fn space_went(&self, space: u64) -> bool {
        space != 0 && self.task_space(space).is_none()
    }

    /// The task whose conversation the area shows had no Space and has one now: the area is that
    /// Space's from here, set aside for a close's question or not. Says whether it took one.
    pub(super) fn area_takes_space(&mut self) -> bool {
        let Some(space) = self.talk_takes_space() else {
            return false;
        };
        let aside = (self.task_closing.as_mut()).and_then(|closing| closing.aside.as_mut());
        for area in self.task_area.iter_mut().chain(aside) {
            if area.space == 0 {
                area.space = space;
            }
        }
        true
    }

    /// The task's tabs, pinned then Today, for its thumbnails.
    pub fn task_tabs(&self, space: u64) -> Vec<u64> {
        self.task_space(space)
            .map(|space| space.pinned.iter().chain(&space.today).copied().collect())
            .unwrap_or_default()
    }

    /// Keeps a task Space's tabs. A page watched that left it leaves the area.
    pub(super) fn set_task_tabs(
        &mut self,
        space: u64,
        pinned: Vec<u64>,
        today: Vec<u64>,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.task_spaces.iter_mut().find(|each| each.id == space) else {
            return;
        };
        entry.pinned = pinned;
        entry.today = today;
        let watched = self.watched_task_page().filter(|&(at, _)| at == space);
        if let Some((_, tab)) = watched
            && !self.task_tabs(space).contains(&tab)
        {
            // check.sh reads this line.
            log!("browser: task page {tab} gone");
            if let Some(area) = &mut self.task_area {
                area.page = None;
            }
            self.take_keyboard(cx);
            self.show_users_page(cx);
        }
        self.forget_closed(cx);
        cx.notify();
    }

    /// The core shows `space`: a task's while one of its pages is watched, the user's again after,
    /// and says whether that's all, the window's Space staying the user's. A task's Space shown
    /// any other way, as one a launch brings back, gives way to the user's.
    pub(super) fn task_space_shown(&mut self, space: u64, cx: &mut Context<Self>) -> bool {
        if self.task_space(space).is_some() {
            self.task_core = space;
            // check.sh reads this line.
            log!("browser: task space {space} shown");
            if !self.watched_task_page().is_some_and(|(at, _)| at == space)
                || self.watch_tile_shown().is_some()
            {
                self.show_users_space(cx);
            }
            cx.notify();
            return true;
        }
        if self.task_core == 0 {
            if self.task_area.is_some() && space != self.space {
                self.drop_task_area(cx);
            }
            return false;
        }
        self.task_core = 0;
        if space == self.space {
            // check.sh reads this line.
            log!("browser: task space left");
            cx.notify();
            return true;
        }
        self.drop_task_area(cx);
        false
    }

    /// Has the core show the window's Space, or the first of the user's if it was a task's.
    fn show_users_space(&mut self, cx: &mut Context<Self>) {
        let listed = self.spaces.iter().any(|each| each.id == self.space);
        let space = if listed {
            self.space
        } else {
            self.spaces.first().map_or(0, |each| each.id)
        };
        if space != 0 {
            self.browser.show_space(space, cx);
        }
    }

    /// The core shows `tab`, and says whether that's all: while it shows a task's Space, that's
    /// the task's page watched, the window's selection staying the user's. Another tab shown by
    /// itself, not the user's selected, leaves the area.
    pub(super) fn task_tab_shown(&mut self, tab: u64, cx: &mut Context<Self>) -> bool {
        if self.task_core != 0 {
            // check.sh reads this line.
            log!("browser: task page {tab} shown");
            // The agent's new tab shows where it worked, as in its Space.
            let space = self.task_core;
            let theirs = tab != 0 && self.task_tabs(space).contains(&tab);
            if let Some(page) = self.task_area.as_mut().and_then(|area| area.page.as_mut())
                && theirs
            {
                page.tab = tab;
            }
            cx.notify();
            return true;
        }
        if self.task_area.is_some() && tab != self.selected {
            self.drop_task_area(cx);
        }
        false
    }

    /// The user selects `tab`, and says whether that's all: a task's page watched takes the
    /// keyboard, and another of the task's shows in its place. Any other tab leaves the area.
    pub(super) fn task_select(&mut self, tab: u64, cx: &mut Context<Self>) -> bool {
        let Some(area) = &self.task_area else {
            return false;
        };
        if !self.task_tabs(area.space).contains(&tab) {
            self.drop_task_area(cx);
            return false;
        }
        let tiled = self.watch_tile_shown();
        match self.task_area.as_mut().and_then(|area| area.page.as_mut()) {
            // A page in its tile takes no keyboard; once the user holds it, it's the window's
            // page with it (`settle_watch_tile`).
            Some(page) if page.tab == tab && tiled == Some(tab) => {}
            Some(page) if page.tab == tab => self.browser.focus_page(cx),
            Some(page) => {
                let old = page.tab;
                page.tab = tab;
                if let Some(tile) = page.tile.take() {
                    let at = (tile.placed.filter(|_| tile.shown)).map(|(at, _)| at);
                    tile.tiles.take_back(old, at);
                }
                self.browser.select(tab, cx);
            }
            // The conversation watches a page from its thumbnail (`watch_task_page`).
            None => {}
        }
        cx.notify();
        true
    }

    /// The page area shows `view`, the conversation of the task whose Space is `space`, in place of
    /// the Space's tab, at once, as a tab switch is. Another task's conversation shown is replaced
    /// where it is, as is another session's of the same agent, whose Space it shares.
    pub fn open_task(&mut self, space: u64, view: AnyView, cx: &mut Context<Self>) {
        if self.space_went(space) {
            log!("browser: no task space {space}");
            return;
        }
        let replacing = (self.task_area.as_ref())
            .is_some_and(|area| area.space != space || area.view.entity_id() != view.entity_id());
        if replacing
            && self
                .task_area
                .as_ref()
                .is_some_and(|area| area.page.is_some())
        {
            self.leave_task_page_now(cx);
        }
        match &mut self.task_area {
            Some(area) => {
                area.space = space;
                area.view = view;
            }
            None => {
                self.task_area = Some(TaskArea {
                    space,
                    view,
                    page: None,
                    opens: None,
                })
            }
        }
        // check.sh reads these lines.
        log!("browser: task area {space} opens");
        log!("browser: task area {space} open");
        self.take_keyboard(cx);
        // Its group at the sidebar's foot opens to its pages, and the one shown before closes.
        self.reshape_cards(cx);
        cx.notify();
    }

    /// The page area goes back to the Space's tab at once, a page watched with it.
    pub fn close_task(&mut self, cx: &mut Context<Self>) {
        if self.task_area.is_none() {
            return;
        }
        self.leave_task_area(cx);
        if self.selected != 0 {
            self.browser.focus_page(cx);
        }
        cx.notify();
    }

    /// Puts a task's conversation beside the page it watches (`talk_beside`).
    pub fn set_talk_beside(&mut self, beside: bool, cx: &mut Context<Self>) {
        self.talk_beside = beside;
        self.talk_redraw(cx);
        cx.notify();
    }

    /// The task's `tab` zooms out of its thumbnail, at `from` in the window and pictured by
    /// `picture` if there's one, to fill the page area, and says whether it did. With the user
    /// holding it from the agent it has the keyboard; while the agent drives, the UI keeps it.
    pub fn watch_task_page(
        &mut self,
        tab: u64,
        from: Bounds<Pixels>,
        picture: Option<PagePicture>,
        by_ilo: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        // Every door goes through the tile where the core shows them, so a page only watched is
        // never the window's: the area's conversation lends it. Only show_task opens an area, and
        // with its conversation.
        let space = self.task_area.as_ref().map(|area| area.space);
        let tiles = (space.and_then(|space| self.talk_in(space)))
            .and(self.talk.as_ref())
            .map(|talk| talk.tiles(cx));
        self.watch_task_page_from(tab, from, picture, by_ilo, tiles, cx)
    }

    /// The same, from the conversation, whose `tiles` show its pages live where the core can: the
    /// page's tile grows to the area, and the page is never the window's while it's only watched.
    pub(super) fn watch_task_page_from(
        &mut self,
        tab: u64,
        from: Bounds<Pixels>,
        picture: Option<PagePicture>,
        by_ilo: bool,
        tiles: Option<Rc<LiveTiles>>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(area) = &self.task_area else {
            return false;
        };
        if !self.task_tabs(area.space).contains(&tab) {
            return false;
        }
        let zoom = if crate::motion::still(cx) {
            Zoom::Rest
        } else {
            Zoom::Out(Instant::now())
        };
        if let Some(old) = self.task_area.as_mut().and_then(|area| area.page.take()) {
            self.let_go_of_watch(old, true);
        }
        let tile = self.watch_tile(tab, from, tiles);
        // A tile takes no input, so only a page that's the window's at once waits behind its
        // picture.
        let guarded = by_ilo && !tile.as_ref().is_some_and(|tile| tile.shown);
        self.show_watched(
            Watched {
                tab,
                thumb: from,
                picture,
                asked: false,
                zoom,
                tile,
                by_ilo,
                guarded,
                waiting: None,
            },
            cx,
        );
        true
    }

    /// The tile a watch of `tab` from `from` has, which `tiles` lend it, where the core shows them:
    /// the page its agent has stays in its tile, which grows out of its picture; one the user
    /// holds, a handoff too, is theirs at once, as the window's page, and a tile once handed back.
    fn watch_tile(
        &mut self,
        tab: u64,
        from: Bounds<Pixels>,
        tiles: Option<Rc<LiveTiles>>,
    ) -> Option<WatchTile> {
        let tiles = tiles?;
        let was = tiles.lend(tab);
        if self.task_page_held(tab) {
            if was.is_some() {
                self.browser.end_tile(tab);
            }
            return (was.is_some() || tiles.shown_any()).then_some(WatchTile {
                tiles,
                shown: false,
                placed: None,
                pressed: None,
                handed: None,
            });
        }
        let at = page_in(from);
        if self.browser.show_tile(tab, at, TILE_RADIUS) {
            return Some(WatchTile {
                tiles,
                shown: true,
                placed: Some((at, TILE_RADIUS)),
                pressed: None,
                handed: None,
            });
        }
        tiles.take_back(tab, was);
        None
    }

    /// The area watches `page`, in place of the one it watched, and the core shows it: in its
    /// tile, or as the window's page.
    fn show_watched(&mut self, page: Watched, cx: &mut Context<Self>) {
        let (tab, by_ilo) = (page.tab, page.by_ilo);
        let tiled = page.tile.as_ref().is_some_and(|tile| tile.shown);
        if let Some(area) = &mut self.task_area
            && let Some(old) = area.page.replace(page)
        {
            self.let_go_of_watch(old, true);
        }
        // check.sh reads this line.
        log!("browser: task page {tab} watched");
        if tiled {
            // A tile takes no input, so the UI has the keyboard, unless ilo opened the page by
            // itself: then the keyboard stays where it was, in the reply field too.
            if !by_ilo {
                self.take_keyboard(cx);
            }
            cx.notify();
            return;
        }
        // Shown first, at the frame the user's page had out of the window, then laid out at its own
        // size there, so the user's page keeps its size.
        self.browser.select(tab, cx);
        self.lay_out_page(cx);
        match self.agent_bar() {
            // A page ilo opened by itself leaves the keyboard where it was, in the reply field
            // too: the user's first click in the page gives it the keyboard, once it's in place.
            _ if by_ilo => {
                cx.spawn(async move |shell, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs_f32(ARM))
                        .await;
                    shell
                        .update(cx, |shell, cx| {
                            let page = (shell.task_area.as_mut())
                                .and_then(|area| area.page.as_mut())
                                .filter(|page| page.tab == tab && page.guarded);
                            if let Some(page) = page {
                                page.guarded = false;
                                shell.lay_out_page(cx);
                                cx.notify();
                            }
                        })
                        .ok();
                })
                .detach();
            }
            Some(Bar::Held { tab: held, .. }) if held == tab => self.browser.focus_page(cx),
            _ => self.take_keyboard(cx),
        }
        cx.notify();
    }

    /// Lets go of a page no longer watched, kept for a close a page might refuse: its picture, and
    /// its tile, if it still has one.
    fn let_go_of(&mut self, page: Option<Watched>) {
        if let Some(page) = page {
            self.let_go_of_watch(page, true);
        }
    }

    /// Whether the user holds `tab` from its agent: taken over, handed to them, or asked for.
    fn task_page_held(&self, tab: u64) -> bool {
        (self.agent_status.iter()).any(|status| status.held.iter().any(|(held, _)| *held == tab))
    }

    /// Whether the user took `tab` over, their own act, rather than the agent handing it to them
    /// or asking for it: by the hold's kind, never by the words an agent wrote for it.
    fn task_page_taken(&self, tab: u64) -> bool {
        (self.agent_status.iter()).any(|status| status.taken.contains(&tab))
    }

    /// Lets go of a watch that's over: the picture it asked for, and its tile, which the
    /// conversation's tiles take back where it is, to move to its picture or end, or which ends
    /// with no conversation to go back to.
    fn let_go_of_watch(&mut self, page: Watched, to_conversation: bool) {
        if let Watched {
            picture: Some(picture),
            asked: true,
            ..
        } = &page
        {
            self.stale_pictures.push(picture.image());
        }
        if let Some(tile) = page.tile {
            self.give_back_tile(page.tab, tile, to_conversation);
        }
    }

    /// `tab`'s tile, which a watch had, goes back to the conversation's tiles where it is, to move
    /// to its picture or end, or ends with no conversation to go back to.
    fn give_back_tile(&mut self, tab: u64, tile: WatchTile, to_conversation: bool) {
        let at = (tile.placed.filter(|_| tile.shown)).map(|(at, _)| at);
        if at.is_some() && !to_conversation {
            self.browser.end_tile(tab);
            log!("browser: task tile {tab} ended");
            return tile.tiles.take_back(tab, None);
        }
        tile.tiles.take_back(tab, at);
    }

    /// The watched page's tile, if it has one.
    fn watch_tile_mut(&mut self) -> Option<&mut WatchTile> {
        (self.task_area.as_mut())
            .and_then(|area| area.page.as_mut())
            .and_then(|page| page.tile.as_mut())
    }

    /// The page watched in its tile now, if it is.
    pub(super) fn watch_tile_shown(&self) -> Option<u64> {
        let page = self.task_area.as_ref()?.page.as_ref()?;
        page.tile.as_ref().filter(|tile| tile.shown)?;
        Some(page.tab)
    }

    /// Where the watched page's tile goes at `now`, and its corners: out of its picture's place as
    /// it zooms, then whole in the area, at the page's own shape.
    fn watch_tile_frame(&self, page: &Watched, now: Instant) -> (Bounds<Pixels>, f32) {
        let (card, _) = self.page_frame(self.viewport);
        let place = fitted(watched(self.page_part(card)));
        let way = page.zoom.at(now);
        let radius = TILE_RADIUS + (self.card_radius() - TILE_RADIUS) * way;
        (between(page_in(page.thumb), place, way), radius)
    }

    /// Swaps the watched page between its tile and the window as the hold changes, and keeps the
    /// tile where it goes. Held, taken over or handed to the user, it's the window's page, the
    /// user's tab, though not before ARM is past; handed back, it's a tile again. Only the user's
    /// own press or Take Over gives it the keyboard.
    fn settle_watch_tile(&mut self, now: Instant, cx: &mut Context<Self>) {
        let Some(page) = self.task_area.as_ref().and_then(|area| area.page.as_ref()) else {
            return;
        };
        let Some(tile) = &page.tile else {
            return;
        };
        let (tab, zoom, shown, pressed, handed) =
            (page.tab, page.zoom, tile.shown, tile.pressed, tile.handed);
        let within = |at: Option<Instant>| {
            at.is_some_and(|at| now.saturating_duration_since(at).as_secs_f32() < ARM)
        };
        let held = self.task_page_held(tab);
        let users = pressed.is_some() || self.task_page_taken(tab);
        if shown && held && !users && handed.is_none() {
            if let Some(tile) = self.watch_tile_mut() {
                tile.handed = Some(now);
            }
            // Once its ARM is past, the page handed to the user is the window's.
            cx.spawn(async move |shell, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs_f32(ARM))
                    .await;
                shell.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
            return self.place_watch_tile(now, cx);
        }
        if shown && held && !within(pressed) && (users || !within(handed)) {
            if let Some(tile) = self.watch_tile_mut() {
                (tile.shown, tile.placed, tile.pressed, tile.handed) = (false, None, None, None);
            }
            self.browser.end_tile(tab);
            log!("browser: task page {tab} out of its tile");
            self.browser.select(tab, cx);
            self.lay_out_page(cx);
            if users {
                self.browser.focus_page(cx);
            }
            return;
        }
        if !shown && !held && zoom == Zoom::Rest {
            if let Some(tile) = self.watch_tile_mut() {
                tile.shown = true;
            }
            self.show_users_page(cx);
        }
        if !held
            && !within(pressed)
            && let Some(tile) = self.watch_tile_mut()
        {
            // A press that took nothing over, as with the agent's session ended.
            (tile.pressed, tile.handed) = (None, None);
        }
        self.place_watch_tile(now, cx);
    }

    /// Puts the watched page's tile where it goes now, if it moved. Where the core says no, the
    /// page is the window's, as where it shows no tiles.
    fn place_watch_tile(&mut self, now: Instant, cx: &mut Context<Self>) {
        let Some(page) = self.task_area.as_ref().and_then(|area| area.page.as_ref()) else {
            return;
        };
        let Some(tile) = page.tile.as_ref().filter(|tile| tile.shown) else {
            return;
        };
        let placed = self.watch_tile_frame(page, now);
        if tile.placed == Some(placed) {
            return;
        }
        let tab = page.tab;
        if self.browser.show_tile(tab, placed.0, placed.1) {
            if let Some(tile) = self.watch_tile_mut() {
                tile.placed = Some(placed);
            }
            return;
        }
        if let Some(tile) = (self.task_area.as_mut())
            .and_then(|area| area.page.as_mut())
            .and_then(|page| page.tile.take())
        {
            tile.tiles.take_back(tab, None);
        }
        self.browser.select(tab, cx);
        self.lay_out_page(cx);
    }

    /// A press on the watched page's tile takes the page over, as Take Over does, and goes no
    /// further: neither it nor one in the ARM after it reaches the page.
    fn press_watch_tile(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let Some(page) = self.task_area.as_mut().and_then(|area| area.page.as_mut()) else {
            return;
        };
        if matches!(page.zoom, Zoom::Out(_)) {
            page.zoom = Zoom::Rest;
        }
        let tab = page.tab;
        let Some(tile) = page.tile.as_mut().filter(|tile| tile.shown) else {
            return;
        };
        cx.notify();
        if tile.pressed.is_some() {
            return;
        }
        tile.pressed = Some(now);
        self.page_pressed(tab, cx);
        // Once its ARM is past, the page the user took is the window's.
        cx.spawn(async move |shell, cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(ARM))
                .await;
            shell.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    /// The press on the watched tile, or its handing to the user, as if it were longer ago than its
    /// ARM, for a test.
    #[cfg(test)]
    pub(super) fn age_watch_press(&mut self) {
        if let Some(tile) = self.watch_tile_mut() {
            let aged = |at: Option<Instant>| at.map(|at| at - Duration::from_secs_f32(ARM + 1.));
            (tile.pressed, tile.handed) = (aged(tile.pressed), aged(tile.handed));
        }
    }

    /// Back to the conversation: the page shrinks into its thumbnail, from a picture of it as it
    /// is now.
    pub fn task_page_back(&mut self, cx: &mut Context<Self>) {
        let still = crate::motion::still(cx);
        let Some(page) = self.task_area.as_mut().and_then(|area| area.page.as_mut()) else {
            return;
        };
        let tab = page.tab;
        match page.zoom {
            Zoom::Asked(_) | Zoom::In(_) => return,
            // Turned around on its way out, it goes back from where it is, with the picture it came
            // with.
            Zoom::Out(_) if !still => {
                let now = Instant::now();
                let back = Duration::from_secs_f32((1. - page.zoom.at(now)) * CLOSE);
                page.zoom = Zoom::In(now.checked_sub(back).unwrap_or(now));
                self.show_task_page_away(cx);
            }
            _ if still => {
                self.leave_task_page_now(cx);
            }
            // A tile goes back as it is, live, with no picture to take.
            _ if page.tile.as_ref().is_some_and(|tile| tile.shown) => {
                page.zoom = Zoom::In(Instant::now());
            }
            _ => {
                let request = next_request();
                page.zoom = Zoom::Asked(request);
                // A core that never answers leaves it going back with the picture it has (#4247).
                page.waiting = Some(cx.spawn(async move |shell, cx| {
                    let wait = Duration::from_secs_f32(PICTURE_WAIT);
                    cx.background_executor().timer(wait).await;
                    (shell.update(cx, |shell, cx| shell.task_page_unpictured(request, cx))).ok();
                }));
                self.browser.page_picture(tab, request, cx);
            }
        }
        // check.sh reads this line.
        log!("browser: task page {tab} back");
        self.take_keyboard(cx);
        // The reply field has the keyboard again, as when the conversation opened.
        self.focus_talk = true;
        cx.notify();
    }

    /// The task Space whose conversation is in the page area.
    pub(super) fn task_open(&self) -> Option<u64> {
        self.task_area.as_ref().map(|area| area.space)
    }

    /// The task Space the page area shows and the tab watched there, if any.
    pub fn task_shown(&self) -> Option<(u64, Option<u64>)> {
        let area = self.task_area.as_ref()?;
        Some((area.space, self.watched_task_page().map(|(_, tab)| tab)))
    }

    /// The task Space and its tab whose page the area shows or is on its way to.
    pub(super) fn watched_task_page(&self) -> Option<(u64, u64)> {
        let area = self.task_area.as_ref()?;
        let page = area
            .page
            .as_ref()
            .filter(|page| !matches!(page.zoom, Zoom::In(_)))?;
        Some((area.space, page.tab))
    }

    /// ilo opens `tab`, a page of the task's that has come to need the user, unless one the user
    /// watches is in place. One on its way back is let finish first: `tab` opens once that one is
    /// in its thumbnail, if it still needs them.
    pub(super) fn open_needing_page(&mut self, tab: u64, cx: &mut Context<Self>) {
        let Some(area) = &mut self.task_area else {
            return;
        };
        match area.page.as_ref().map(|page| page.zoom) {
            Some(Zoom::Asked(_) | Zoom::In(_)) => area.opens = Some(tab),
            Some(Zoom::Out(_) | Zoom::Rest) => {}
            None => {
                self.watch_row(tab, true, cx);
            }
        }
    }

    /// Whether the task shown asks the user for something in `tab`, as its row and tile say.
    fn task_page_needs_user(&self, tab: u64) -> bool {
        self.task_card_shown()
            .is_some_and(|card| self.page_mark(card, tab).word == PageWord::NeedsYou)
    }

    /// Whether the page watched is going back into its picture, over a conversation the user
    /// can't see yet: only its pictures take a press meanwhile (#3496).
    pub(super) fn task_page_going_back(&self) -> bool {
        (self.task_area.as_ref())
            .and_then(|area| area.page.as_ref())
            .is_some_and(|page| matches!(page.zoom, Zoom::In(_)))
    }

    /// The page watched held where its zoom back starts, for a test to press under it.
    #[cfg(test)]
    pub(super) fn hold_task_page_going_back(&mut self) {
        if let Some(page) = self.task_area.as_mut().and_then(|area| area.page.as_mut()) {
            page.zoom = Zoom::In(Instant::now() + Duration::from_secs(60));
        }
    }

    /// The task's tab whose page is in place in the area, live, as the core shows it: in its
    /// tile or as the window's page.
    pub(super) fn task_page_live(&self) -> Option<u64> {
        let area = self.task_area.as_ref()?;
        let page = area.page.as_ref()?;
        let tiled = page.tile.as_ref().is_some_and(|tile| tile.shown);
        let live = matches!(page.zoom, Zoom::Rest | Zoom::Asked(_))
            && (tiled || self.task_core == area.space)
            && !page.guarded;
        live.then_some(page.tab)
    }

    /// The task's tab whose page is in place in the area as the window's page, not in a tile.
    fn task_page_in_window(&self) -> Option<u64> {
        let tab = self.task_page_live()?;
        (self.watch_tile_shown() != Some(tab)).then_some(tab)
    }

    /// The task's page in place in the area, and where it is in the window.
    pub(super) fn task_page_place(&self) -> Option<(u64, Bounds<Pixels>)> {
        let tab = self.task_page_live()?;
        if self.watch_tile_shown() == Some(tab) {
            let page = self.task_area.as_ref()?.page.as_ref()?;
            return Some((tab, self.watch_tile_frame(page, Instant::now()).0));
        }
        let (card, _) = self.page_frame(self.viewport);
        Some((
            tab,
            if self.page_fullscreen {
                card
            } else {
                watched(self.page_part(card))
            },
        ))
    }

    /// Whether the user stands on a task's page with its conversation beside it, rather than over
    /// it: with `talk_beside`, from the page's zoom out until it goes back, and not full screen.
    pub(super) fn task_standing(&self) -> bool {
        self.talk_beside
            && !self.page_fullscreen
            && (self.task_area.as_ref())
                .and_then(|area| area.page.as_ref())
                .is_some_and(|page| !matches!(page.zoom, Zoom::In(_)))
    }

    /// The card's part a page watched has: all of it, or its left while the user stands on it.
    fn page_part(&self, card: Bounds<Pixels>) -> Bounds<Pixels> {
        match self.task_standing() {
            true => split(card).0,
            false => card,
        }
    }

    /// Whether the window shows `tab`'s page as an agent's: a task's in place in the area.
    pub(super) fn agent_page_shows(&self, tab: u64) -> bool {
        self.task_area.is_some() && self.task_page_live() == Some(tab)
    }

    /// The task Space and its tab whose page the area draws, on its way back too.
    pub(super) fn task_page_drawn(&self) -> Option<(u64, u64)> {
        let area = self.task_area.as_ref()?;
        Some((area.space, area.page.as_ref()?.tab))
    }

    /// The agent's Space whose page the window shows, and the tab: a task's page the area draws.
    pub(super) fn agent_page(&self) -> Option<(&Space, u64)> {
        let (space, tab) = self.task_page_drawn()?;
        Some((self.task_space(space)?, tab))
    }

    /// Where the window's page goes, given the card's frame: there, unless a task has the area.
    /// Its conversation pushes the Space's page out of the window, at its size, as the Library
    /// pushes the card; a task's page watched is laid out out of the window at its size while it
    /// zooms, in place once it rests and the core shows it, and full screen if it goes full screen.
    pub(super) fn task_page_frame(
        &self,
        card: Bounds<Pixels>,
        viewport: Size<Pixels>,
    ) -> Bounds<Pixels> {
        let Some(area) = &self.task_area else {
            return card;
        };
        let away = viewport.width - card.left();
        let Some(page) = &area.page else {
            return pushed(card, away);
        };
        // A page in its tile is never the window's, which stays the user's, out of the window.
        if page.tile.as_ref().is_some_and(|tile| tile.shown) {
            return pushed(card, away);
        }
        let live = self.task_core == area.space;
        let part = self.page_part(card);
        let live = live && !page.guarded;
        match page.zoom {
            Zoom::Rest | Zoom::Asked(_) if live && self.page_fullscreen => card,
            Zoom::Rest | Zoom::Asked(_) if live => watched(part),
            Zoom::Out(_) | Zoom::Rest | Zoom::Asked(_) => pushed(watched(part), away),
            Zoom::In(_) => pushed(card, away),
        }
    }

    /// The area goes at once, and the core shows the Space's page again if it showed a task's.
    pub(super) fn leave_task_area(&mut self, cx: &mut Context<Self>) {
        let showed = self.task_core != 0;
        self.drop_task_area(cx);
        if showed {
            self.browser.show_space(self.space, cx);
        }
    }

    /// The area goes at once, the Space's page laid out back in the card, for whatever the window
    /// shows next. Laid out first, so the page shown next is laid out there once.
    pub(super) fn drop_task_area(&mut self, cx: &mut Context<Self>) {
        let Some(mut area) = self.task_area.take() else {
            return;
        };
        if let Some(page) = area.page.take() {
            self.let_go_of_watch(page, false);
        }
        // check.sh reads this line.
        log!("browser: task area {} closed", area.space);
        let left = self.talk_in(area.space);
        self.talk_closed(cx);
        self.lay_out_page(cx);
        // Its group at the sidebar's foot closes to its header.
        self.reshape_cards(cx);
        if let Some(agent) = left {
            self.talk_left(agent, cx);
        }
        cx.notify();
    }

    /// The page watched goes at once, back to the conversation.
    fn leave_task_page_now(&mut self, cx: &mut Context<Self>) {
        let Some(area) = self.task_area.as_mut() else {
            return;
        };
        // What the user does next is theirs: the page that needs them waits in its tile.
        area.opens = None;
        let Some(page) = area.page.take() else {
            return;
        };
        self.let_go_of_watch(page, true);
        // check.sh waits for this line, as at the end of the zoom back with motion.
        log!("browser: task page in its thumbnail");
        self.show_users_page(cx);
        self.retake_talk_pictures(cx);
        cx.notify();
    }

    /// The window is about to close, or the browser to quit, which the window's pages may still
    /// refuse (#4202). The conversation stands, and a page of the task's watched goes back to it
    /// first, kept for a refusal to bring back: the core shows the task's Space while one is
    /// watched, and the session keeps for the window the Space the core shows. The same close
    /// asked again keeps what the first kept: with its question up the view is aside already.
    pub(super) fn about_to_close(&mut self, cx: &mut Context<Self>) {
        let kept = self.task_closing.take();
        if kept.as_ref().is_some_and(|kept| kept.aside.is_some()) {
            self.task_closing = kept;
            return;
        }
        let kept = kept.and_then(|kept| kept.watched);
        if self.task_area.is_none() {
            return self.let_go_of(kept);
        }
        let watched = match self.back_to_close(cx) {
            // One watched since the first is the view as it stands.
            Some(page) => {
                self.let_go_of(kept);
                Some(page)
            }
            None => kept,
        };
        self.task_closing = Some(Closing {
            watched,
            aside: None,
        });
        // A page of theirs that asked already, behind the conversation, asks now.
        self.step_aside_to_ask(cx);
        cx.notify();
    }

    /// The task's page watched goes back to its conversation at once for a close, and is
    /// returned to keep unless it was on its way back already. The UI has the keyboard.
    fn back_to_close(&mut self, cx: &mut Context<Self>) -> Option<Watched> {
        let page = self.task_area.as_ref()?.page.as_ref()?;
        // One on its way back into its thumbnail is no longer watched.
        if matches!(page.zoom, Zoom::In(_)) {
            self.leave_task_page_now(cx);
        }
        let mut watched = (self.task_area.as_mut()).and_then(|area| area.page.take());
        // Its tile goes back to the conversation with it; a refused close lends it again.
        if let Some(page) = &mut watched
            && let Some(tile) = page.tile.take()
        {
            self.give_back_tile(page.tab, tile, true);
        }
        if watched.is_some() {
            // check.sh waits for this line, as wherever a page watched goes back.
            log!("browser: task page in its thumbnail");
            self.show_users_page(cx);
            self.retake_talk_pictures(cx);
        }
        self.take_keyboard(cx);
        watched
    }

    /// A page of the user's asks whether to leave it while the window closes with a task's view
    /// standing: the view steps aside, whole, and the window shows the page and its question as
    /// with no task shown. Any other dialog of a user's page waits behind the area, as ever.
    pub(super) fn step_aside_to_ask(&mut self, cx: &mut Context<Self>) {
        let waits = (self.task_closing.as_ref()).is_some_and(|closing| closing.aside.is_none());
        if !waits || self.task_area.is_none() {
            return;
        }
        let tasks = |tab: &u64| {
            (self.task_spaces.iter())
                .any(|space| space.pinned.contains(tab) || space.today.contains(tab))
        };
        let asks = (self.page_dialogs.iter())
            .any(|(tab, dialog)| dialog.kind == DialogKind::Leave && !tasks(tab));
        if !asks {
            return;
        }
        // A page of the task's the user watched since the close goes back first, as at the
        // close: the question is asked on the user's page, which the core then shows.
        if let Some(page) = self.back_to_close(cx) {
            let kept =
                (self.task_closing.as_mut()).and_then(|closing| closing.watched.replace(page));
            self.let_go_of(kept);
        }
        let area = self.task_area.take();
        if let Some(area) = &area {
            log!(
                "browser: task area {} aside for a page's question",
                area.space
            );
        }
        if let Some(closing) = &mut self.task_closing {
            closing.aside = area;
        }
        self.lay_out_page(cx);
        self.reshape_cards(cx);
        cx.notify();
    }

    /// A page of the user's refused the close: the task's view is back as it stood, the page
    /// watched with it, and the keyboard where the view has it.
    pub(super) fn close_refused(&mut self, cx: &mut Context<Self>) {
        let Some(Closing { watched, aside }) = self.task_closing.take() else {
            return;
        };
        if let Some(area) = aside {
            self.stand_again(area, cx);
        }
        let space = self.task_open();
        match (watched, space) {
            (Some(page), Some(space)) if self.task_tabs(space).contains(&page.tab) => {
                // The core shows the task's Space from this call on, and says so a turn later.
                // The window doesn't wait to hear it: a frame drawn meanwhile had the page back
                // and the sidebar, the URL field and the strip still the user's page's (#4202).
                self.task_core = space;
                // In place at once, as it stood, in its tile again where the core shows them.
                let tiles = self.talk.as_ref().map(|talk| talk.tiles(cx));
                let tile = self.watch_tile(page.tab, page.thumb, tiles);
                let page = Watched {
                    zoom: Zoom::Rest,
                    tile,
                    by_ilo: false,
                    guarded: false,
                    waiting: None,
                    ..page
                };
                self.show_watched(page, cx);
            }
            (gone, _) => {
                self.let_go_of(gone);
                if space.is_some() {
                    self.take_keyboard(cx);
                    // The reply field has the keyboard again, as when the conversation opened.
                    self.focus_talk = true;
                }
            }
        }
        cx.notify();
    }

    /// What the user does next is no answer to a close: the view kept for its refusal goes, and
    /// one set aside is left as any is.
    pub(super) fn forget_closing(&mut self, cx: &mut Context<Self>) {
        let Some(Closing { watched, aside }) = self.task_closing.take() else {
            return;
        };
        self.let_go_of(watched);
        if let Some(area) = aside
            && self.task_area.is_none()
        {
            self.task_area = Some(area);
            self.drop_task_area(cx);
        }
    }

    /// The area set aside takes the page area again, unless its task went meanwhile or another
    /// has it.
    fn stand_again(&mut self, area: TaskArea, cx: &mut Context<Self>) {
        if self.task_area.is_some() {
            return;
        }
        self.task_area = Some(area);
        if self.task_open().is_some_and(|space| self.space_went(space)) {
            return self.drop_task_area(cx);
        }
        if let Some(space) = self.task_open() {
            log!("browser: task area {space} back");
        }
        self.lay_out_page(cx);
        self.restore_cards(cx);
        // The Agents send its conversation as it is now, as they sent none while it was aside.
        if let Some(agent) = self.talk_agent() {
            self.browser.show_task(agent, cx);
        }
    }

    /// Lays the page out for the area as it is, then has the core show the Space's page again, if
    /// it showed a task's: out of the window behind the conversation, at the card's size.
    fn show_users_page(&mut self, cx: &mut Context<Self>) {
        self.lay_out_page(cx);
        if self.task_core != 0 {
            self.browser.show_space(self.space, cx);
        }
    }

    /// The page watched shrinks into its thumbnail from here, the Space's page shown again out of
    /// the window.
    fn show_task_page_away(&mut self, cx: &mut Context<Self>) {
        self.show_users_page(cx);
    }

    /// The core answers `request` with a picture of a task's page going back, and says whether the
    /// request was the area's.
    pub(super) fn task_pictured(
        &mut self,
        request: u64,
        picture: Option<PagePicture>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(page) = self.task_area.as_mut().and_then(|area| area.page.as_mut()) else {
            return false;
        };
        if page.zoom != Zoom::Asked(request) {
            return false;
        }
        page.waiting = None;
        if let Some(picture) = picture {
            if let Some(old) = page.picture.replace(picture).filter(|_| page.asked) {
                self.stale_pictures.push(old.image());
            }
            page.asked = true;
        }
        page.zoom = Zoom::In(Instant::now());
        self.show_task_page_away(cx);
        cx.notify();
        true
    }

    /// The page waited `PICTURE_WAIT` for the picture `request` asked, which never came: it goes
    /// back with the picture it has, and one that comes after is nothing to the area.
    fn task_page_unpictured(&mut self, request: u64, cx: &mut Context<Self>) {
        let Some(page) = self.task_area.as_mut().and_then(|area| area.page.as_mut()) else {
            return;
        };
        if page.zoom != Zoom::Asked(request) {
            return;
        }
        page.waiting = None;
        page.zoom = Zoom::In(Instant::now());
        // The core's fault (#4253), in the run's log, where the zoom alone wouldn't show it.
        log!("browser: task page {} back unpictured", page.tab);
        self.show_task_page_away(cx);
        cx.notify();
    }

    /// A press on the page zooming ends its zoom out at once, for the page itself to take what
    /// follows.
    fn press_task_page(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.task_area.as_mut().and_then(|area| area.page.as_mut()) else {
            return;
        };
        if matches!(page.zoom, Zoom::Out(_)) && !page.guarded {
            page.zoom = Zoom::Rest;
            cx.notify();
        }
    }

    /// Runs `command` while a task has the page area, and says whether that's all. ⌘↩ and ⌘. are
    /// the strip's, as in an agent's Space, and Escape with the UI's keyboard and ⌘W step back.
    /// The page's own commands act on the task's page in place, the window's leave the area as it
    /// is, and the rest act on the user's Space, so they leave the area first: the core shows
    /// the user's Space again before a tab opens in it. Quit, once its quit is decided, and
    /// Close Window keep the conversation, which only the window's going takes, and a page that
    /// refuses brings the view back whole (`about_to_close`).
    pub(super) fn run_in_task_area(
        &mut self,
        command: Command,
        from: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.task_area.is_none() {
            return false;
        }
        let live = self.task_page_live();
        let in_window = self.task_page_in_window();
        match command {
            Command::BackToPinnedUrl if self.agent_bar().is_some() => self.agent_bar_primary(cx),
            // A page in its tile is only watched: nothing finds in it, zooms it or prints it.
            Command::Find | Command::Print | Command::ActualSize | Command::ZoomIn | Command::ZoomOut
                if live.is_some() && in_window.is_none() => {}
            // Esc steps back, from a page watched too, whose page has the keyboard (the menu's
            // Stop comes from elsewhere).
            Command::Stop if matches!(from, "ui" | "page") => self.task_step_back(cx),
            // The conversation is its card's, selected, so ⌘W closes the card, dismissing the
            // task, as it closes a tab; from a page watched it steps back to the conversation.
            Command::CloseTab if self.watched_task_page().is_none() && self.dismiss_talk(cx) => {}
            Command::CloseTab => self.task_step_back(cx),
            // Escape and Stop from elsewhere never stop an agent, and its loads are its own.
            Command::Stop => {}
            Command::Back
            | Command::Forward
            | Command::Reload
            | Command::ForceReload
            | Command::ActualSize
            | Command::ZoomIn
            | Command::ZoomOut => {
                let Some(tab) = live else {
                    // The conversation has no page to act on.
                    return true;
                };
                match command {
                    Command::Back => self.browser.go_back(tab, cx),
                    Command::Forward => self.browser.go_forward(tab, cx),
                    Command::Reload => self.browser.reload(tab, false, cx),
                    Command::ForceReload => self.browser.reload(tab, true, cx),
                    Command::ActualSize => self.browser.zoom(tab, 0, cx),
                    Command::ZoomIn => self.browser.zoom(tab, 1, cx),
                    _ => self.browser.zoom(tab, -1, cx),
                }
            }
            // The header's commands act on the page watched, as its URL field shows it, and Edit
            // URL edits it.
            Command::CopyUrl
            | Command::CopyMarkdownLink
            | Command::SiteControls
            | Command::EditUrl
                if live.is_some() =>
            {
                return false;
            }
            // Rows chosen in the sidebar are copied as anywhere.
            Command::CopyUrl if !self.chosen.is_empty() => return false,
            // The conversation is no page, as the URL field shows: nothing to copy or control.
            Command::CopyUrl | Command::CopyMarkdownLink | Command::SiteControls => {}
            // ⌘T opens the bar over the conversation, from a page watched too, and so does Edit
            // URL with no page in place, the field showing the bar's placeholder.
            Command::NewTab | Command::SearchTabs | Command::EditUrl => {
                if self
                    .task_area
                    .as_ref()
                    .is_some_and(|area| area.page.is_some())
                {
                    self.leave_task_page_now(cx);
                    self.take_keyboard(cx);
                }
                return false;
            }
            Command::NewWindow
            | Command::NewPrivateWindow
            | Command::NewBlankWindow
            | Command::NewLittleWindow
            | Command::ToggleSidebar
            | Command::RecordTrace
            | Command::StayOnTop
            | Command::MoveSidebar
            | Command::TaskManager
            | Command::Settings
            | Command::KeyboardShortcuts
            | Command::About
            | Command::CheckForUpdates
            | Command::Appearance(_)
            | Command::CopyBuildInfo
            // Each asks first, and what it asks about isn't the view: Cancel, and the Archive
            // cleared, leave the user where they were (#4202).
            | Command::Quit
            | Command::ClearArchive
            // The core asks the window's pages before it closes, and theirs is the same Cancel.
            | Command::CloseWindow
            // The agent's card shows over the area as over the user's Space.
            | Command::FocusAgentQuestion
            // The ring goes to the task shown, at the foot: its pages' rows are there only while
            // its view is.
            | Command::FocusSidebar => return false,
            _ => {
                self.leave_task_area(cx);
                return false;
            }
        }
        true
    }

    /// Escape with the UI's keyboard, or ⌘W: from a page watched back to the conversation, and
    /// from the conversation back to the Space's tab. Never Stop, as an agent stopped by a stray
    /// escape would lose its work.
    pub(super) fn task_step_back(&mut self, cx: &mut Context<Self>) {
        if self.watched_task_page().is_some() {
            return self.task_page_back(cx);
        }
        self.close_task(cx);
    }

    /// Steps the area's motion for a frame: the page's zoom.
    pub(super) fn step_task_area(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let still = crate::motion::still(cx);
        let now = Instant::now();
        let Some(area) = &mut self.task_area else {
            return;
        };
        let mut ended = None;
        if let Some(page) = &mut area.page {
            match page.zoom {
                Zoom::Out(start) if still || now.duration_since(start).as_secs_f32() >= OPEN => {
                    page.zoom = Zoom::Rest;
                }
                Zoom::In(start) if still || now.duration_since(start).as_secs_f32() >= CLOSE => {
                    ended = area.page.take();
                }
                Zoom::Out(_) | Zoom::In(_) => window.request_animation_frame(),
                Zoom::Rest | Zoom::Asked(_) => {}
            }
        }
        if let Some(page) = ended {
            self.let_go_of_watch(page, true);
            // check.sh waits for this line.
            log!("browser: task page in its thumbnail");
            self.retake_talk_pictures(cx);
            let opens = self.task_area.as_mut().and_then(|area| area.opens.take());
            if let Some(tab) = opens.filter(|&tab| self.task_page_needs_user(tab)) {
                self.watch_row(tab, true, cx);
            }
        }
        self.settle_watch_tile(now, cx);
        let live = self.task_page_live();
        if live != self.task_live_logged {
            self.task_live_logged = live;
            // check.sh waits for this line before it looks at the page.
            if let Some(tab) = live {
                log!("browser: task page {tab} in place");
            }
        }
    }

    /// What the page area draws for a task, if it shows one: the conversation, and a page watched
    /// zooming or in place over its card, with the strip under it.
    pub(super) fn task_area_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(area) = self.task_area.as_ref() else {
            self.task_strip_drawn.set(None);
            return None;
        };
        let (card_frame, _) = self.page_frame(self.viewport);
        let now = Instant::now();
        let zoom = area.page.as_ref().map_or(0., |page| page.zoom.at(now));
        let back = self.task_page_going_back();
        let standing = self.task_standing();
        let mut layer = div().absolute().size_full();
        // The Space's page goes out of the window on its card.
        // The conversation is in the card's place, and fades under a page coming out of it. Under
        // a page going back it takes clicks from the zoom's first frame, before it shows (#3496).
        // Standing on the page, it's beside it in its panel instead, from the zoom's first frame.
        if standing || zoom < 1. || back {
            let (at, shown) = match standing {
                true => (split(card_frame).1, 1.),
                false => (card_frame, 1. - zoom),
            };
            let view = div()
                .absolute()
                .left_0()
                .top_0()
                .size_full()
                .rounded(px(self.card_radius()))
                .overflow_hidden()
                .child(area.view.clone());
            layer = layer.child(
                div()
                    .id("task-conversation")
                    .absolute()
                    .left(at.left())
                    .top(at.top())
                    .w(at.size.width)
                    .h(at.size.height)
                    .overflow_hidden()
                    .opacity(shown)
                    .block_mouse_except_scroll()
                    .child(view),
            );
        } else {
            // Not drawn, it places none of its tiles, which the core ended as the window took the
            // page: they're shown again once it's drawn.
            if let Some(talk) = &self.talk {
                talk.end_tiles(cx);
            }
        }
        let Some(page) = &area.page else {
            // Drawn again, for another page, the strip is logged again.
            self.task_strip_drawn.set(None);
            return Some(layer.into_any_element());
        };
        let part = self.page_part(card_frame);
        let place = watched(part);
        if let Some(tile) = page.tile.as_ref().filter(|tile| tile.shown) {
            let at = (tile.placed).map_or_else(|| self.watch_tile_frame(page, now).0, |(at, _)| at);
            return Some(
                layer
                    .child(self.watch_tile_ground(page.tab, place, at, zoom, cx))
                    .child(self.task_strip(part, page.tab, zoom, cx))
                    .into_any_element(),
            );
        }
        let in_place = self.task_page_live() == Some(page.tab);
        if in_place && self.page_fullscreen {
            return Some(layer.into_any_element());
        }
        // While the agent drives the page, the Signal ring of its pointer goes round it.
        let driving = in_place && self.task_drives(page.tab);
        if driving {
            let ring = place.dilate(px(RING));
            layer = layer.child(
                div()
                    .absolute()
                    .left(ring.left())
                    .top(ring.top())
                    .w(ring.size.width)
                    .h(ring.size.height)
                    .rounded(px(self.card_radius() + RING))
                    .border(px(RING))
                    .border_color(rgb(COLOR)),
            );
        }
        if in_place {
            // A page that died leaves its place to its card, as in the user's own Space.
            layer = layer
                .child(card(place, self.card_radius(), self.palette.page))
                .children(self.crash_cards(place, cx));
        } else {
            layer = layer.child(self.task_page_picture(page, between(page.thumb, place, zoom), cx));
        }
        Some(
            layer
                .child(self.task_strip(part, page.tab, zoom, cx))
                .into_any_element(),
        )
    }

    /// What the area draws under a page watched in its tile, which the core draws over it: the
    /// area's ground round the page, fading in as it zooms; the agent's ring round it while the
    /// agent drives; and where the tile is, what takes a press on it, which takes the page over.
    /// Nothing takes a scroll or a key there.
    fn watch_tile_ground(
        &self,
        tab: u64,
        place: Bounds<Pixels>,
        at: Bounds<Pixels>,
        zoom: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let radius = self.card_radius();
        let ring = (zoom >= 1. && self.task_drives(tab)).then(|| {
            let ring = at.dilate(px(RING));
            div()
                .absolute()
                .left(ring.left())
                .top(ring.top())
                .w(ring.size.width)
                .h(ring.size.height)
                .rounded(px(radius + RING))
                .border(px(RING))
                .border_color(rgb(COLOR))
        });
        // Going back, the tile takes no press: the conversation's pictures under it do, as they do
        // under a page going back into its picture (#3496, #4267).
        let press = (!self.task_page_going_back()).then(|| {
            div()
                .id("task-page-tile")
                .absolute()
                .left(at.left())
                .top(at.top())
                .w(at.size.width)
                .h(at.size.height)
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.press_watch_tile(cx);
                    }),
                )
        });
        div()
            .absolute()
            .size_full()
            .child(card(place, radius, self.palette.page).opacity(zoom))
            .children(ring)
            .children(press)
            .into_any_element()
    }

    /// The page watched as it zooms: its picture, or its card till there is one, which a press
    /// puts in place at once.
    fn task_page_picture(
        &self,
        page: &Watched,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let radius = px(self.card_radius());
        let picture = page.picture.clone().map(|picture| {
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let drawn = covering(bounds, picture.size());
                    let corners = Corners::all(radius);
                    window
                        .paint_image(bounds, drawn, corners, picture.image(), 0, false)
                        .ok();
                },
            )
            .absolute()
            .size_full()
        });
        div()
            .id("task-page-picture")
            .absolute()
            .left(bounds.left())
            .top(bounds.top())
            .w(bounds.size.width)
            .h(bounds.size.height)
            .rounded(radius)
            .bg(self.palette.page)
            // Going back, it lets a press through to the conversation under it, so a click on
            // another picture there watches that page (#3496).
            .when(!matches!(page.zoom, Zoom::In(_)), |zooming| {
                zooming.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.press_task_page(cx);
                    }),
                )
            })
            .children(picture)
            .into_any_element()
    }

    /// The dot of the task whose page the strip is under, as that page has it: the strip is the
    /// page's own, never the task's. A task that waits on the user at another page, one it handed
    /// them, is at work on this one (#4102).
    pub(super) fn task_strip_dot(&self) -> Option<Dot> {
        let card = self.task_card_shown()?;
        let shown = self.task_page_drawn().map(|(_, tab)| tab);
        let elsewhere = (self.agent_status.iter())
            .filter(|status| status.agent == card.agent)
            .flat_map(|status| status.held.iter())
            .any(|(held, why)| Some(*held) != shown && !why.is_empty());
        Some(match card.dot {
            Dot::Waiting if elsewhere => Dot::Working,
            dot => dot,
        })
    }

    /// Whether the task the area shows is at work on `tab`'s page, which the page's ring says:
    /// the page its agent is at work in, or once it works on several, each that has a step of
    /// its own.
    fn task_drives(&self, tab: u64) -> bool {
        let Some(Bar::Working { tab: at, .. }) = self.agent_bar() else {
            return false;
        };
        let Some(card) = self.task_card_shown() else {
            return false;
        };
        let own = card.page_steps.iter().any(|(each, _)| *each == tab);
        match card.page_steps.is_empty() {
            true => self.task_working() && at == tab,
            false => self.task_strip_dot() == Some(Dot::Working) && own,
        }
    }

    /// The agent of the task whose page the strip is under.
    pub(super) fn task_strip_agent(&self) -> Option<SharedString> {
        self.task_card_shown().map(|card| card.agent.clone())
    }

    /// The card of the task the page area shows.
    fn task_card_shown(&self) -> Option<&Card> {
        let agent = self.talk_in(self.task_open()?)?;
        self.task_cards.iter().find(|card| card.agent == agent)
    }

    /// What the strip under a page watched says, by its task's card: who works and on what, or
    /// that it finished and where, or that it stopped. The user holding the page has the Agent
    /// Space bar's words.
    pub(super) fn task_strip_words(&self, tab: u64) -> (String, String) {
        if let Some(bar @ Bar::Held { .. }) = self.agent_bar() {
            let (title, detail, _) = bar_words(&bar);
            return (title, detail);
        }
        let Some(card) = self.task_card_shown() else {
            return (self.title(tab).to_string(), String::new());
        };
        let name = card.name.clone();
        match self.task_strip_dot().unwrap_or(card.dot) {
            // The page's own step once the task works on several, never another page's, nor
            // what the task waits for on another: only its card and conversation say those.
            Dot::Working => {
                let own = (card.page_steps.iter()).find(|(each, _)| *each == tab);
                let detail = match (own, card.page_steps.is_empty() && card.dot == Dot::Working) {
                    (Some((_, step)), _) => step.to_string(),
                    (None, true) => card.status.to_string(),
                    (None, false) => String::new(),
                };
                (format!("{name} is working"), detail)
            }
            // What it needs the user for, without the card's "Needs you", which the title says.
            Dot::Waiting => {
                let lead = status_lead(&card.status);
                let why = card.status[lead..].trim_start_matches(" · ");
                (format!("{name} needs you"), why.to_string())
            }
            Dot::Done => (
                format!("{name} finished"),
                crate::agents::site(&self.page(tab).url).to_string(),
            ),
            Dot::Ended => (card.word.alone(&name).to_string(), String::new()),
        }
    }

    /// The strip's Open on a finished task's page: the page shown's own, of those the task left the
    /// user, and none once it moved.
    pub(super) fn task_strip_open(&self, tab: u64) -> Option<String> {
        let card = self
            .task_card_shown()
            .filter(|card| card.dot == Dot::Done)?;
        (card.open.iter())
            .find(|(each, _)| *each == tab)
            .map(|(_, site)| format!("Open {site}"))
    }

    /// The page watched's speaker, before the strip's chips while it plays: crossed while it's
    /// muted, as an agent's tabs start (#1080), and a click unmutes or mutes it, as a row's does.
    fn strip_sound(&self, tab: u64, cx: &mut Context<Self>) -> Option<gpui::Stateful<gpui::Div>> {
        let page = self.page(tab);
        page.audible.then_some(())?;
        let palette = self.palette;
        Some(
            div()
                .id("task-sound")
                .flex_none()
                .size(px(CHIP_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CHIP_RADIUS))
                .bg(palette.plate(CHIP_FILL))
                .hover(move |style| style.bg(palette.state(CHIP_FILL, STATE_HOVERED)))
                .active(move |style| style.bg(palette.state(CHIP_FILL, STATE_PRESSED)))
                .child(
                    svg()
                        .path(super::rows::speaker_glyph(page.muted))
                        .size(px(CHIP_GLYPH))
                        .text_color(palette.text(TITLE)),
                )
                .on_click(cx.listener(move |shell, _, _, cx| {
                    cx.stop_propagation();
                    shell.toggle_mute(tab, cx);
                })),
        )
    }

    /// The strip's chips, as `strip_chips` has them for the page watched: beside the
    /// conversation, Take Over is always the strip's, and Stop is the composer's. The ring's stops
    /// are these, so it never rings one that isn't drawn.
    pub(super) fn strip_shows(
        &self,
        held: bool,
        dot: Option<Dot>,
        open: bool,
        keyed: bool,
    ) -> Vec<(StripChip, Look)> {
        let standing = self.task_standing();
        (strip_chips(held, dot, open, keyed || standing).into_iter())
            .filter(|(chip, _)| !(standing && *chip == StripChip::Stop))
            .collect()
    }

    /// Whether the task the page area shows is at work, which its page's ring says.
    pub(super) fn task_working(&self) -> bool {
        self.task_card_shown()
            .is_some_and(|card| card.dot == Dot::Working)
    }

    /// The strip under a page watched: back to the conversation, the words, and the chips at its
    /// right: Take Over and Stop while the agent works, Hand Back while the user holds the page,
    /// Open <site> once a task left the user this page. It comes in with the page.
    fn task_strip(
        &self,
        card_frame: Bounds<Pixels>,
        tab: u64,
        shown: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (title, detail) = self.task_strip_words(tab);
        let palette = self.palette;
        let card = self.task_card_shown();
        let agent = card.map(|card| card.agent.clone()).unwrap_or_default();
        // ⌘↩, or off the Mac Ctrl+Enter, as a menu spells it.
        let take_over = |cx: &mut Context<Self>| user_keys(cx).label(Command::BackToPinnedUrl);
        let bar = self.agent_bar();
        let dot = self.task_strip_dot();
        let open = self.task_strip_open(tab);
        let held = matches!(bar, Some(Bar::Held { .. }));
        let keyed = self.keyed_strip.is_some();
        let shows = self.strip_shows(held, dot, open.is_some(), keyed);
        let chips: Vec<_> = (shows.into_iter())
            .map(|(chip, look)| match chip {
                // A click while it shows does what its key does, as Take Over is on screen.
                StripChip::TakeOver => self.strip_stop(
                    strip_chip(palette, "task-take-over", "Take Over", take_over(cx), look)
                        .on_click(cx.listener(|shell, _, _, cx| shell.agent_bar_primary(cx))),
                    StripStop::TakeOver,
                    "Take Over".into(),
                    CHIP_RADIUS,
                ),
                StripChip::HandBack => self.strip_stop(
                    strip_chip(palette, "task-hand-back", "Hand Back", take_over(cx), look)
                        .on_click(cx.listener(|shell, _, _, cx| shell.agent_bar_primary(cx))),
                    StripStop::HandBack,
                    "Hand Back".into(),
                    CHIP_RADIUS,
                ),
                StripChip::Stop => self.strip_stop(
                    strip_chip(palette, "task-stop", "Stop", Some(stop_keys()), look)
                        .on_click(act(agent.clone(), TaskAction::Stop, cx)),
                    StripStop::Stop,
                    "Stop".into(),
                    CHIP_RADIUS,
                ),
                StripChip::Open => {
                    let words = open.clone().unwrap_or_default();
                    let label = SharedString::from(words.clone());
                    let agent = agent.clone();
                    let chip = strip_chip(palette, "task-open", words, None, look)
                        .child(self.strip_open_listener())
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            cx.stop_propagation();
                            let from = shell.task_list.strip_open.get();
                            log!("browser: command open-task-pages via strip");
                            shell.open_task_pages(agent.clone(), Some(tab), from, cx);
                        }));
                    self.strip_stop(chip, StripStop::Open, label, CHIP_RADIUS)
                }
            })
            .collect();
        // Only a strip whose agent needs the user is heavier, as its card is.
        let needs_you = dot == Some(Dot::Waiting);
        let glyph = card
            .zip(dot)
            .map(|(card, dot)| self.task_glyph(&card.agent, dot));
        // The back's chevron lines up with the page's edge, its plate hanging outside.
        let left = card_frame.left() - px((BACK - BACK_GLYPH) / 2.);
        let back = div()
            .id("task-back")
            .flex_none()
            .size(px(BACK))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(palette.plate(STATE_HOVERED)))
            .active(|style| style.bg(palette.plate(STATE_PRESSED)))
            .on_click(cx.listener(|shell, _, _, cx| shell.task_page_back(cx)))
            .child(
                svg()
                    .path("icons/chevron-down.svg")
                    .size(px(BACK_GLYPH))
                    .with_transformation(Transformation::rotate(Radians(FRAC_PI_2)))
                    .text_color(palette.text(SECONDARY)),
            );
        let back = self.strip_stop(
            back,
            StripStop::Back,
            "Back to the conversation".into(),
            NAV_RADIUS,
        );
        let page_name = self.strip_page_name(tab, &title, &detail);
        let detail = (!detail.is_empty()).then(|| format!(" · {detail}"));
        let words = format!("{title}{}", detail.as_deref().unwrap_or_default());
        let mut said = self.task_strip_said.borrow_mut();
        if said.as_ref() != Some(&(tab, words.clone())) {
            // check.sh reads this line: the strip says who has the page, as the bar did.
            log!("browser: task strip {tab} says {words}");
            *said = Some((tab, words));
        }
        drop(said);
        div()
            .id("task-strip")
            .absolute()
            .left(left)
            .top(card_frame.bottom() - px(STRIP))
            .w(card_frame.right() - left)
            .h(px(STRIP))
            .flex()
            .items_center()
            .gap(px(GAP))
            .opacity(shown)
            .block_mouse_except_scroll()
            .text_size(px(text::BODY))
            .line_height(px(LINE))
            .child(card_listener("task strip", &self.task_strip_drawn, tab))
            .child(back)
            .child(
                // The page's stop holds the agent's dot and the words, with a chip's room round
                // them, given back on its left so the dot stays where it is beside Back.
                self.strip_stop(
                    div()
                        .id("task-words")
                        .flex_1()
                        .min_w(px(0.))
                        .h(px(CHIP_HEIGHT))
                        .px(px(CHIP_X))
                        .ml(px(-CHIP_X))
                        .flex()
                        .items_center()
                        .gap(px(GAP))
                        .children(glyph)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(palette.text(TITLE))
                                        .font_weight(match needs_you {
                                            true => FontWeight::SEMIBOLD,
                                            false => FontWeight::MEDIUM,
                                        })
                                        .child(title),
                                )
                                .children(detail.map(|detail| {
                                    div()
                                        .min_w(px(0.))
                                        .truncate()
                                        .text_color(palette.text(if needs_you {
                                            TITLE
                                        } else {
                                            STATUS
                                        }))
                                        .child(detail)
                                })),
                        ),
                    StripStop::Page,
                    page_name,
                    CHIP_RADIUS,
                ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(CHIP_GAP))
                    .children(self.strip_sound(tab, cx))
                    .children(chips),
            )
            .into_any_element()
    }
}

/// A chip of the strip under a page watched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StripChip {
    TakeOver,
    HandBack,
    Stop,
    Open,
}

/// The strip's chips, in order, each with its look: Hand Back while the user holds the page,
/// `held`; Stop while its task works or waits; Open once it's done and left the user the page, if
/// it did. A press in the page takes it over, and ⌘↩ still does, so Take Over shows only while the
/// ring is on one of the task's stops, `keyed`: its card, back, the page, Take Over or Stop, so it
/// is there before the ring reaches it. It's the ring's own stop, as a key from the ring never
/// takes the page over (#3220, #1080). It comes before Stop, which stays where it is, the words
/// giving it room.
/// Quiet until the agent needs the user: only while its card waits on them is Hand Back, the
/// answer it waits for, the one primary.
pub(super) fn strip_chips(
    held: bool,
    dot: Option<Dot>,
    open: bool,
    keyed: bool,
) -> Vec<(StripChip, Look)> {
    match (held, dot) {
        (true, Some(Dot::Waiting)) => vec![(StripChip::HandBack, Look::Primary)],
        (true, _) => vec![(StripChip::HandBack, Look::Quiet)],
        (false, Some(Dot::Working | Dot::Waiting)) => keyed
            .then_some((StripChip::TakeOver, Look::Quiet))
            .into_iter()
            .chain([(StripChip::Stop, Look::Quiet)])
            .collect(),
        (false, Some(Dot::Done)) if open => vec![(StripChip::Open, Look::Quiet)],
        _ => Vec::new(),
    }
}

/// A strip's chip in `look`: the conversation's, or for the primary the Space's accent with its
/// label, its key fainter on it, darker under the pointer and pressed as every filled button goes.
fn strip_chip(
    palette: Palette,
    id: &'static str,
    words: impl Into<SharedString>,
    key: Option<String>,
    look: Look,
) -> gpui::Stateful<gpui::Div> {
    match look {
        Look::Primary => {
            let (hovered, pressed) = (palette.accent_hovered(), palette.accent_pressed());
            let label = Hsla::from(palette.accent_label);
            page_chip_keyed(palette, id, words, key, label.opacity(PRIMARY_KEY))
                .bg(palette.accent)
                .hover(move |style| style.bg(hovered))
                .active(move |style| style.bg(pressed))
                .text_color(label)
        }
        Look::Quiet | Look::Text => page_chip(palette, id, words, key),
    }
}

#[cfg(test)]
mod tests;
