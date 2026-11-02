//! A task the user asked from ⌘T (docs/ux-spec.md › Spaces › Agent task). Its Space is an
//! agent's that the window never lists as a Space, and the page area shows the task in place of
//! the Space's tab while the sidebar, the Space, the selection and the URL field stay the user's.
//! First its conversation, a view the task draws, which comes in from the card's side, pushing
//! the page out of the window the other way as the Library does, and leaves back that way. Then,
//! from a thumbnail in it, one of the task's pages, which zooms out of the thumbnail to fill the
//! area and shrinks back into it, with the Agent Space's bar (#365) laid under it as a strip, as
//! GPUI can't draw over a page. The core shows the task's tab only while its page is watched, and
//! the user's again as it goes. Nothing here runs for a window that shows no task.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, Bounds, Context, Corners, FontWeight, MouseButton, Pixels, Size, Window,
    canvas, div, point, prelude::*, px, rgb, rgba, size, svg,
};

use super::{
    CARD_INSET, CARD_RADIUS, Command, Folders, NAV, NAV_RADIUS, NAV_SIZE, PagePicture, Shell, Space,
    SpaceEntry, TITLE, Theme,
    agent_bar::{Bar, bar_words},
    card, cubic_bezier,
    library::{LIBRARY_DAMPING, LIBRARY_RESPONSE},
    permission::card_listener,
    picture::next_request,
    spring_to,
};
use crate::agents::is_task;
use crate::link_preview::{CLOSE, OPEN};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::tokens::shell::agent_pointer::{COLOR, RING};
use crate::tokens::shell::permission::{BUTTON_GAP, DETAIL, LINE};
use crate::tokens::shell::task_area::{BACK_GLYPH, GAP, PAGE_ROOM, STRIP};
use crate::tokens::text;

/// What the page area shows of a task, at most one per window.
pub(super) struct TaskArea {
    space: u64,
    /// The conversation, which the task draws.
    view: AnyView,
    /// Whether it's in or on its way out, and where its slide started, how far in and when. None
    /// once it rests.
    open: bool,
    slide: Option<(f32, Instant)>,
    page: Option<Watched>,
}

/// A task's page watched in the page area.
struct Watched {
    tab: u64,
    /// The thumbnail it zoomed out of, which it shrinks back into.
    thumb: Bounds<Pixels>,
    picture: Option<PagePicture>,
    /// Whether `picture` is one this window asked for, and so its own to let go of.
    asked: bool,
    zoom: Zoom,
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

/// Where a watched page goes in the card: its top, the card's width, with room under it for the
/// strip.
fn watched(card: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(card.origin, size(card.size.width, card.size.height - px(PAGE_ROOM)))
}

fn pushed(bounds: Bounds<Pixels>, by: Pixels) -> Bounds<Pixels> {
    Bounds::new(point(bounds.left() + by, bounds.top()), bounds.size)
}

fn between(from: Bounds<Pixels>, to: Bounds<Pixels>, way: f32) -> Bounds<Pixels> {
    let at = |from: Pixels, to: Pixels| from + (to - from) * way;
    Bounds::new(
        point(at(from.left(), to.left()), at(from.top(), to.top())),
        size(at(from.size.width, to.size.width), at(from.size.height, to.size.height)),
    )
}

/// Where a picture of `picture`'s size goes to fill `bounds`, as wide as them at least, its top at
/// theirs, as a thumbnail shows a page's top.
fn covering(bounds: Bounds<Pixels>, picture: (f32, f32)) -> Bounds<Pixels> {
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let zoom = (width / picture.0).max(height / picture.1);
    let (drawn_width, drawn_height) = (picture.0 * zoom, picture.1 * zoom);
    Bounds::new(
        point(bounds.left() + px((width - drawn_width) / 2.), bounds.top()),
        size(px(drawn_width), px(drawn_height)),
    )
}

impl Shell {
    /// The task Spaces apart from the user's and other agents', which the window never lists. Each
    /// keeps the tabs it had.
    pub(super) fn take_task_spaces(&mut self, spaces: Vec<SpaceEntry>) -> Vec<SpaceEntry> {
        let browser = self.browser.clone();
        let (tasks, spaces): (Vec<_>, Vec<_>) = spaces
            .into_iter()
            .partition(|entry| !entry.5.is_empty() && is_task(&browser.space_agent(entry.0)));
        let mut old = std::mem::take(&mut self.task_spaces);
        self.task_spaces = tasks
            .into_iter()
            .map(|(id, name, color, profile, icon, agent, theme)| {
                match old.iter().position(|space| space.id == id) {
                    Some(at) => Space { name, agent, ..old.swap_remove(at) },
                    None => {
                        // check.sh reads this line.
                        log!("browser: task space {id} {agent}");
                        Space {
                            id,
                            name,
                            color,
                            theme: Theme::parse(&theme),
                            icon,
                            profile,
                            agent,
                            pinned: Vec::new(),
                            today: Vec::new(),
                            splits: Vec::new(),
                            folders: Folders::default(),
                            shown: 0,
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
        let area = self.task_area.as_ref();
        let gone = area.is_some_and(|area| self.task_space(area.space).is_none());
        if gone {
            self.leave_task_area(cx);
        }
        if self.task_core == 0 && self.task_space(self.space).is_some() {
            self.show_users_space(cx);
        }
    }

    pub(super) fn task_space(&self, space: u64) -> Option<&Space> {
        self.task_spaces.iter().find(|each| each.id == space)
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
            self.browser.focus_ui(cx);
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
        let space = if listed { self.space } else { self.spaces.first().map_or(0, |each| each.id) };
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
    /// the Space's tab, coming in from the card's side. Another task's conversation shown is
    /// replaced where it is.
    pub fn open_task(&mut self, space: u64, view: AnyView, cx: &mut Context<Self>) {
        if self.task_space(space).is_none() {
            log!("browser: no task space {space}");
            return;
        }
        let out = self.task_out();
        let still = cx.reduce_motion();
        let replacing = self.task_area.as_ref().is_some_and(|area| area.space != space);
        if replacing && self.task_area.as_ref().is_some_and(|area| area.page.is_some()) {
            self.leave_task_page_now(cx);
        }
        match &mut self.task_area {
            Some(area) => {
                area.space = space;
                area.view = view;
                if !area.open {
                    area.open = true;
                    area.slide = (!still).then(|| (out, Instant::now()));
                }
            }
            None => {
                let slide = (!still).then(|| (0., Instant::now()));
                self.task_area = Some(TaskArea { space, view, open: true, slide, page: None });
            }
        }
        // check.sh reads these lines.
        log!("browser: task area {space} opens");
        if self.task_area.as_ref().is_some_and(|area| area.slide.is_none()) {
            log!("browser: task area {space} open");
        }
        self.browser.focus_ui(cx);
        cx.notify();
    }

    /// The page area goes back to the Space's tab: the conversation leaves the way it came, and a
    /// page watched goes at once.
    pub fn close_task(&mut self, cx: &mut Context<Self>) {
        let Some(area) = &self.task_area else {
            return;
        };
        if area.page.is_some() || cx.reduce_motion() {
            self.leave_task_area(cx);
        } else if area.open {
            let out = self.task_out();
            if let Some(area) = &mut self.task_area {
                area.open = false;
                area.slide = Some((out, Instant::now()));
            }
            log!("browser: task area closes");
        }
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
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(area) = &self.task_area else {
            return false;
        };
        if !self.task_tabs(area.space).contains(&tab) {
            return false;
        }
        let zoom = if cx.reduce_motion() { Zoom::Rest } else { Zoom::Out(Instant::now()) };
        if let Some(area) = &mut self.task_area {
            if let Some(Watched { picture: Some(old), asked: true, .. }) = area.page.take() {
                self.stale_pictures.push(old.image());
            }
            area.page = Some(Watched { tab, thumb: from, picture, asked: false, zoom });
        }
        // check.sh reads this line.
        log!("browser: task page {tab} watched");
        // Shown first, at the frame the user's page had out of the window, then laid out at its own
        // size there, so the user's page keeps its size.
        self.browser.select(tab, cx);
        self.lay_out_page(cx);
        match self.agent_bar() {
            Some(Bar::Held { tab: held, .. }) if held == tab => self.browser.focus_page(cx),
            _ => self.browser.focus_ui(cx),
        }
        cx.notify();
        true
    }

    /// Back to the conversation: the page shrinks into its thumbnail, from a picture of it as it
    /// is now.
    pub fn task_page_back(&mut self, cx: &mut Context<Self>) {
        let still = cx.reduce_motion();
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
                self.browser.page_picture(tab, request, cx);
            }
        }
        // check.sh reads this line.
        log!("browser: task page {tab} back");
        self.browser.focus_ui(cx);
        cx.notify();
    }

    /// The task Space the page area shows and the tab watched there, if any.
    pub fn task_shown(&self) -> Option<(u64, Option<u64>)> {
        let area = self.task_area.as_ref()?;
        Some((area.space, self.watched_task_page().map(|(_, tab)| tab)))
    }

    /// The task Space and its tab whose page the area shows or is on its way to.
    pub(super) fn watched_task_page(&self) -> Option<(u64, u64)> {
        let area = self.task_area.as_ref()?;
        let page = area.page.as_ref().filter(|page| !matches!(page.zoom, Zoom::In(_)))?;
        Some((area.space, page.tab))
    }

    /// The task's tab whose page is in place in the area, live, as the core shows it.
    pub(super) fn task_page_live(&self) -> Option<u64> {
        let area = self.task_area.as_ref()?;
        let page = area.page.as_ref()?;
        let live = matches!(page.zoom, Zoom::Rest | Zoom::Asked(_)) && self.task_core == area.space;
        live.then_some(page.tab)
    }

    /// The task's page in place in the area, and where it is in the window.
    pub(super) fn task_page_place(&self) -> Option<(u64, Bounds<Pixels>)> {
        let tab = self.task_page_live()?;
        let (card, _) = self.page_frame(self.viewport);
        Some((tab, if self.page_fullscreen { card } else { watched(card) }))
    }

    /// Whether the window shows `tab`'s page as an agent's: a task's in place in the area, else a
    /// page shown in an agent's Space.
    pub(super) fn agent_page_shows(&self, tab: u64) -> bool {
        match self.task_area {
            Some(_) => self.task_page_live() == Some(tab),
            None => self.shows(tab) && self.agents_space_shown(),
        }
    }

    /// The task Space and its tab whose page the area draws, on its way back too.
    pub(super) fn task_page_drawn(&self) -> Option<(u64, u64)> {
        let area = self.task_area.as_ref()?;
        Some((area.space, area.page.as_ref()?.tab))
    }

    /// The agent's Space whose page the window shows, and the tab: a task's page the area draws,
    /// else the Space shown if it's an agent's, with the tab selected there.
    pub(super) fn agent_page(&self) -> Option<(&Space, u64)> {
        if let Some((space, tab)) = self.task_page_drawn() {
            return Some((self.task_space(space)?, tab));
        }
        let space = self.shown_space().filter(|space| !space.agent.is_empty())?;
        Some((space, self.selected))
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
            return pushed(card, away * self.task_out());
        };
        let live = self.task_core == area.space;
        match page.zoom {
            Zoom::Rest | Zoom::Asked(_) if live && self.page_fullscreen => card,
            Zoom::Rest | Zoom::Asked(_) if live => watched(card),
            Zoom::Out(_) | Zoom::Rest | Zoom::Asked(_) => pushed(watched(card), away),
            Zoom::In(_) => pushed(card, away),
        }
    }

    /// How far in the conversation is: 0 out of the area, 1 filling it.
    fn task_out(&self) -> f32 {
        let Some(area) = &self.task_area else {
            return 0.;
        };
        let to = if area.open { 1. } else { 0. };
        let Some((from, start)) = area.slide else {
            return to;
        };
        let t = start.elapsed().as_secs_f32();
        let out = spring_to(t, from, to, 0., LIBRARY_RESPONSE, LIBRARY_DAMPING);
        out.map_or(to, |(out, _)| out).clamp(0., 1.)
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
        if let Some(Watched { picture: Some(picture), asked: true, .. }) = area.page {
            self.stale_pictures.push(picture.image());
        }
        // check.sh reads this line.
        log!("browser: task area {} closed", area.space);
        self.lay_out_page(cx);
        cx.notify();
    }

    /// The page watched goes at once, back to the conversation.
    fn leave_task_page_now(&mut self, cx: &mut Context<Self>) {
        let Some(area) = &mut self.task_area else {
            return;
        };
        if let Some(Watched { picture: Some(picture), asked: true, .. }) = area.page.take() {
            self.stale_pictures.push(picture.image());
        }
        self.show_users_page(cx);
        cx.notify();
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

    /// A press on the page zooming ends its zoom out at once, for the page itself to take what
    /// follows.
    fn press_task_page(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.task_area.as_mut().and_then(|area| area.page.as_mut()) else {
            return;
        };
        if matches!(page.zoom, Zoom::Out(_)) {
            page.zoom = Zoom::Rest;
            cx.notify();
        }
    }

    /// Runs `command` while a task has the page area, and says whether that's all. ⌘↩ and ⌘. are
    /// the strip's, as in an agent's Space, and Escape with the UI's keyboard and ⌘W step back.
    /// The page's own commands act on the task's page in place, the window's leave the area as it
    /// is, and the rest act on the user's Space, so they leave the area first: the core shows
    /// the user's Space again before a tab opens in it.
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
            Command::Stop if from == "ui" => self.task_step_back(cx),
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
            // ⌘T opens the bar over the conversation, from a page watched too.
            Command::NewTab | Command::SearchTabs => {
                if self.task_area.as_ref().is_some_and(|area| area.page.is_some()) {
                    self.leave_task_page_now(cx);
                    self.browser.focus_ui(cx);
                }
                return false;
            }
            Command::NewWindow
            | Command::NewPrivateWindow
            | Command::NewLittleWindow
            | Command::CloseWindow
            | Command::ToggleSidebar
            | Command::RecordTrace
            | Command::TaskManager
            | Command::Settings
            | Command::KeyboardShortcuts
            | Command::About
            | Command::CheckForUpdates
            | Command::Appearance(_)
            | Command::CopyBuildInfo
            | Command::Quit => return false,
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
        match self.watched_task_page() {
            Some(_) => self.task_page_back(cx),
            None => self.close_task(cx),
        }
    }

    /// Steps the area's motion for a frame: the conversation's slide and the page's zoom.
    pub(super) fn step_task_area(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let still = cx.reduce_motion();
        let out = self.task_out();
        let now = Instant::now();
        let Some(area) = &mut self.task_area else {
            return;
        };
        let mut closed = false;
        if let Some((_, start)) = area.slide {
            let to = if area.open { 1. } else { 0. };
            let t = start.elapsed().as_secs_f32();
            let moving = !still && spring_to(t, out, to, 0., LIBRARY_RESPONSE, LIBRARY_DAMPING)
                .is_some_and(|_| (out - to).abs() > 0.0005);
            if moving {
                window.request_animation_frame();
            } else {
                area.slide = None;
                closed = !area.open;
                // check.sh waits for this line.
                if area.open {
                    log!("browser: task area {} open", area.space);
                }
            }
        }
        if let Some(page) = &mut area.page {
            match page.zoom {
                Zoom::Out(start) if still || now.duration_since(start).as_secs_f32() >= OPEN => {
                    page.zoom = Zoom::Rest;
                }
                Zoom::In(start) if still || now.duration_since(start).as_secs_f32() >= CLOSE => {
                    if let Some(Watched { picture: Some(picture), asked: true, .. }) =
                        area.page.take()
                    {
                        self.stale_pictures.push(picture.image());
                    }
                    // check.sh waits for this line.
                    log!("browser: task page in its thumbnail");
                }
                Zoom::Out(_) | Zoom::In(_) => window.request_animation_frame(),
                Zoom::Rest | Zoom::Asked(_) => {}
            }
        }
        if closed {
            self.drop_task_area(cx);
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
    /// zooming or in place over its card, with the strip under it.
    pub(super) fn task_area_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let area = self.task_area.as_ref()?;
        let (card_frame, _) = self.page_frame(self.viewport);
        let now = Instant::now();
        let zoom = area.page.as_ref().map_or(0., |page| page.zoom.at(now));
        let mut layer = div().absolute().size_full();
        // The Space's page goes out of the window on its card.
        let out = self.task_out();
        if area.page.is_none() && out < 1. {
            let away = self.viewport.width - card_frame.left();
            layer = layer.child(card(pushed(card_frame, away * out), self.palette.page));
        }
        // The conversation comes from the card's side as the page goes the other way, the gap
        // between them the card's inset, and fades under a page coming out of it.
        if zoom < 1. {
            let offset = -(1. - out) * (card_frame.size.width + px(CARD_INSET));
            let view = div()
                .absolute()
                .left(offset)
                .top_0()
                .size_full()
                .rounded(px(CARD_RADIUS))
                .overflow_hidden()
                .child(area.view.clone());
            layer = layer.child(
                div()
                    .id("task-conversation")
                    .absolute()
                    .left(card_frame.left())
                    .top(card_frame.top())
                    .w(card_frame.size.width)
                    .h(card_frame.size.height)
                    .overflow_hidden()
                    .opacity(1. - zoom)
                    .block_mouse_except_scroll()
                    .child(view),
            );
        }
        let Some(page) = &area.page else {
            return Some(layer.into_any_element());
        };
        let place = watched(card_frame);
        let in_place = self.task_page_live() == Some(page.tab);
        if in_place && self.page_fullscreen {
            return Some(layer.into_any_element());
        }
        // While the agent drives the page, the Signal ring of its pointer goes round it.
        let driving = in_place
            && matches!(self.agent_bar(), Some(Bar::Working { tab, .. }) if tab == page.tab);
        if driving {
            let ring = place.dilate(px(RING));
            layer = layer.child(
                div()
                    .absolute()
                    .left(ring.left())
                    .top(ring.top())
                    .w(ring.size.width)
                    .h(ring.size.height)
                    .rounded(px(CARD_RADIUS + RING))
                    .border(px(RING))
                    .border_color(rgb(COLOR)),
            );
        }
        if in_place {
            layer = layer.child(card(place, self.palette.page));
        } else {
            layer = layer.child(self.task_page_picture(page, between(page.thumb, place, zoom), cx));
        }
        Some(layer.child(self.task_strip(card_frame, page.tab, zoom, cx)).into_any_element())
    }

    /// The page watched as it zooms: its picture, or its card till there is one, which a press
    /// puts in place at once.
    fn task_page_picture(
        &self,
        page: &Watched,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let radius = px(CARD_RADIUS);
        let picture = page.picture.clone().map(|picture| {
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let drawn = covering(bounds, picture.size());
                    let corners = Corners::all(radius);
                    window.paint_image(bounds, drawn, corners, picture.image(), 0, false).ok();
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| {
                    cx.stop_propagation();
                    shell.press_task_page(cx);
                }),
            )
            .children(picture)
            .into_any_element()
    }

    /// The strip under a page watched: back to the conversation, then the Agent Space bar's words
    /// and buttons in a row, or the page's title while no agent is at work in the task. It comes
    /// in with the page.
    fn task_strip(
        &self,
        card_frame: Bounds<Pixels>,
        tab: u64,
        shown: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let bar = self.agent_bar();
        let (title, detail) = match &bar {
            Some(bar) => {
                let (title, detail, _) = bar_words(bar);
                (title, detail)
            }
            None => (self.title(tab).to_string(), String::new()),
        };
        let buttons = bar.map(|bar| self.agent_bar_buttons(&bar, cx)).unwrap_or_default();
        // The back's glyph lines up with the page's edge, its plate hanging outside.
        let left = card_frame.left() - px((NAV_SIZE - BACK_GLYPH) / 2.);
        let back = div()
            .id("task-back")
            .flex_none()
            .size(px(NAV_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
            .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
            .on_click(cx.listener(|shell, _, _, cx| shell.task_page_back(cx)))
            .child(
                svg()
                    .path("icons/arrow-left.svg")
                    .size(px(BACK_GLYPH))
                    .text_color(self.palette.glyph(NAV)),
            );
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
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_color(self.palette.text(TITLE))
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .child(div().flex_1().min_w(px(0.)).truncate().text_color(rgba(DETAIL)).child(detail))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(BUTTON_GAP))
                    .children(buttons.into_iter().map(|button| button.flex_none())),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
