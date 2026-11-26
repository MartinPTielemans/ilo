//! A task the user asked from ⌘T (docs/ux-spec.md › Spaces › Agent task). Its Space is an
//! agent's that the window never lists as a Space, and the page area shows the task in place of
//! the Space's tab while the sidebar, the Space, the selection and the URL field stay the user's.
//! First its conversation, a view the task draws, in the card's place at once, as a tab switch
//! is (docs/ux-principles.md › A switch stays instant), the page laid out out of the window. Then,
//! from a thumbnail in it, one of the task's pages, which zooms out of the thumbnail to fill the
//! area and shrinks back into it, with the Agent Space's bar (#365) laid under it as a strip, as
//! GPUI can't draw over a page. The core shows the task's tab only while its page is watched, and
//! the user's again as it goes. Nothing here runs for a window that shows no task.

use std::time::Duration;
use std::time::Instant;

use gpui::{
    AnyElement, AnyView, Bounds, Context, Corners, MouseButton, Pixels, SharedString, Size, Task,
    Window, canvas, div, point, prelude::*, px, size,
};

use super::{
    Command, DialogKind, Folders, PagePicture, Shell, Space, SpaceEntry, Theme, agent_bar::Bar,
    card, cubic_bezier, picture::next_request, task_cards::PageWord,
};
use crate::agents::task::{Card, Dot};
use crate::link_preview::{CLOSE, OPEN};
use crate::tokens::shell::conversation::ilo::ARM;
use crate::tokens::shell::task_area::{BESIDE, BESIDE_GAP, BESIDE_LEAST, PAGE_LEAST};

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

/// Where a watched page goes in its part of the card: all of it, as the panel beside it holds
/// everything the page's strip under it did.
fn watched(card: Bounds<Pixels>) -> Bounds<Pixels> {
    card
}

/// The card split for standing on a page with the conversation beside it: the page's part on its
/// right, and the conversation's panel at its left, between the sidebar and the page.
fn split(card: Bounds<Pixels>) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let wide = px(panel_width(f32::from(card.size.width)));
    let panel = Bounds::new(card.origin, size(wide, card.size.height));
    let page = Bounds::new(
        point(panel.right() + px(BESIDE_GAP), card.top()),
        size(card.size.width - wide - px(BESIDE_GAP), card.size.height),
    );
    (page, panel)
}

/// The panel's width in a card `card` wide: its own while the page keeps its least beside it, then
/// what the page leaves it down to the panel's least, and narrower still each in proportion to its
/// least, so neither jumps as the window narrows.
fn panel_width(card: f32) -> f32 {
    let room = (card - BESIDE_GAP).max(0.);
    if room >= BESIDE + PAGE_LEAST {
        BESIDE
    } else if room >= BESIDE_LEAST + PAGE_LEAST {
        room - PAGE_LEAST
    } else {
        room * BESIDE_LEAST / (BESIDE_LEAST + PAGE_LEAST)
    }
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
            if !self.watched_task_page().is_some_and(|(at, _)| at == space) {
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
        match self.task_area.as_mut().and_then(|area| area.page.as_mut()) {
            Some(page) if page.tab == tab => self.browser.focus_page(cx),
            Some(page) => {
                page.tab = tab;
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
        self.show_watched(
            Watched {
                tab,
                thumb: from,
                picture,
                asked: false,
                zoom,
                by_ilo,
                guarded: by_ilo,
                waiting: None,
            },
            cx,
        );
        true
    }

    /// Standing on one of the task's pages, the user stands on its `tab` instead, at once, as a tab
    /// switch is: it goes back into the picture the page it replaces would have. Says whether it
    /// did.
    pub fn stand_on_task_page(
        &mut self,
        tab: u64,
        picture: Option<PagePicture>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(area) = &self.task_area else {
            return false;
        };
        let Some(thumb) = area.page.as_ref().map(|page| page.thumb) else {
            return false;
        };
        if !self.task_standing() || !self.task_tabs(area.space).contains(&tab) {
            return false;
        }
        if area.page.as_ref().is_some_and(|page| page.tab == tab) {
            return true;
        }
        self.show_watched(
            Watched {
                tab,
                thumb,
                picture,
                asked: false,
                zoom: Zoom::Rest,
                by_ilo: false,
                guarded: false,
                waiting: None,
            },
            cx,
        );
        true
    }

    /// The area watches `page`, in place of the one it watched, and the core shows it.
    fn show_watched(&mut self, page: Watched, cx: &mut Context<Self>) {
        let (tab, page_by_ilo) = (page.tab, page.by_ilo);
        if let Some(area) = &mut self.task_area {
            let old = area.page.replace(page);
            self.let_go_of(old);
        }
        // check.sh reads this line.
        log!("browser: task page {tab} watched");
        // Shown first, at the frame the user's page had out of the window, then laid out at its own
        // size there, so the user's page keeps its size.
        let by_ilo = page_by_ilo;
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

    /// Lets go of the picture a page no longer watched was asked for.
    fn let_go_of(&mut self, page: Option<Watched>) {
        if let Some(Watched {
            picture: Some(picture),
            asked: true,
            ..
        }) = page
        {
            self.stale_pictures.push(picture.image());
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

    /// The task's tab whose page is in place in the area, live, as the core shows it.
    pub(super) fn task_page_live(&self) -> Option<u64> {
        let area = self.task_area.as_ref()?;
        let page = area.page.as_ref()?;
        let live = matches!(page.zoom, Zoom::Rest | Zoom::Asked(_))
            && self.task_core == area.space
            && !page.guarded;
        live.then_some(page.tab)
    }

    /// The task's page in place in the area, and where it is in the window.
    pub(super) fn task_page_place(&self) -> Option<(u64, Bounds<Pixels>)> {
        let tab = self.task_page_live()?;
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

    /// Whether the user stands on a task's page, its conversation in the panel beside it, the one
    /// way a thread's page is watched: from the page's zoom out until it goes back, and not full
    /// screen.
    pub(super) fn task_standing(&self) -> bool {
        !self.page_fullscreen
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
        let Some(area) = self.task_area.take() else {
            return;
        };
        if let Some(Watched {
            picture: Some(picture),
            asked: true,
            ..
        }) = area.page
        {
            self.stale_pictures.push(picture.image());
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
        if let Watched {
            picture: Some(picture),
            asked: true,
            ..
        } = page
        {
            self.stale_pictures.push(picture.image());
        }
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
        let watched = (self.task_area.as_mut()).and_then(|area| area.page.take());
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
                // In place at once, as it stood.
                let page = Watched {
                    zoom: Zoom::Rest,
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
        match command {
            Command::BackToPinnedUrl if self.agent_bar().is_some() => self.agent_bar_primary(cx),
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
        let mut retake = false;
        if let Some(page) = &mut area.page {
            match page.zoom {
                Zoom::Out(start) if still || now.duration_since(start).as_secs_f32() >= OPEN => {
                    page.zoom = Zoom::Rest;
                }
                Zoom::In(start) if still || now.duration_since(start).as_secs_f32() >= CLOSE => {
                    if let Some(Watched {
                        picture: Some(picture),
                        asked: true,
                        ..
                    }) = area.page.take()
                    {
                        self.stale_pictures.push(picture.image());
                    }
                    // check.sh waits for this line.
                    log!("browser: task page in its thumbnail");
                    retake = true;
                }
                Zoom::Out(_) | Zoom::In(_) => window.request_animation_frame(),
                Zoom::Rest | Zoom::Asked(_) => {}
            }
        }
        if retake {
            self.retake_talk_pictures(cx);
            let opens = self.task_area.as_mut().and_then(|area| area.opens.take());
            if let Some(tab) = opens.filter(|&tab| self.task_page_needs_user(tab)) {
                self.watch_row(tab, true, cx);
            }
        }
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
    /// zooming or in place over its card, beside the panel.
    pub(super) fn task_area_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(area) = self.task_area.as_ref() else {
            self.task_page_at.set(None);
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
        }
        let Some(page) = &area.page else {
            // Watched again, the page's place is logged again.
            self.task_page_at.set(None);
            return Some(layer.into_any_element());
        };
        let part = self.page_part(card_frame);
        let place = watched(part);
        super::permission::logged_at("task page", &self.task_page_at, page.tab, place);
        let in_place = self.task_page_live() == Some(page.tab);
        if in_place && self.page_fullscreen {
            return Some(layer.into_any_element());
        }
        if in_place {
            // A page that died leaves its place to its card, as in the user's own Space.
            layer = layer
                .child(card(place, self.card_radius(), self.palette.page))
                .children(self.crash_cards(place, cx));
        } else {
            layer = layer.child(self.task_page_picture(page, between(page.thumb, place, zoom), cx));
        }
        Some(layer.into_any_element())
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

    /// The agent of the task whose page the strip is under.
    pub(super) fn task_strip_agent(&self) -> Option<SharedString> {
        self.task_card_shown().map(|card| card.agent.clone())
    }

    /// The card of the task the page area shows.
    pub(super) fn task_card_shown(&self) -> Option<&Card> {
        let agent = self.talk_in(self.task_open()?)?;
        self.task_cards.iter().find(|card| card.agent == agent)
    }

    /// The page row's Open on a finished task's page: the page shown's own, of those the task left
    /// the user, and none once it moved.
    pub(super) fn task_strip_open(&self, tab: u64) -> Option<String> {
        let card = self
            .task_card_shown()
            .filter(|card| card.dot == Dot::Done)?;
        (card.open.iter())
            .find(|(each, _)| *each == tab)
            .map(|(_, site)| format!("Open {site}"))
    }
}

#[cfg(test)]
mod tests;
