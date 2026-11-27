//! A task's pages live in their pictures' places (#4115). Where the core can, as on the Mac, an
//! agent's page shows in its picture in the task's rail, drawing as the agent works, while the
//! picture is wholly in the rail's view; partly scrolled out, under a page watched over the
//! conversation, or wherever the core says no, the picture the UI drew stays, as it does on Windows
//! and Linux.
//!
//! The tile is the user's view alone. The core holds the rest (`bc_tab_show_tile`): the page takes
//! no keyboard, focus or click from it, keeps its own size, and neither it nor its agent hears that
//! it's watched. A click on the tile is the UI's, which watches the page as the picture's does: the
//! watch takes the tile, grows it to the area and gives it back (task_area.rs), so the page is never
//! out of a tile till the user takes it over.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{App, Bounds, IntoElement, ParentElement, Pixels, Styled, canvas, div, px};

use super::browser::Browser;
use super::picture::next_request;
use crate::tokens::shell::agent_pointer::RING;
use crate::tokens::shell::conversation::PICTURE_RADIUS;

/// Each picture's page, live where it can be. A task's conversation has one, made with it.
pub(super) struct LiveTiles {
    browser: Rc<dyn Browser>,
    /// The tiles the core shows, by tab, where it was last asked to.
    live: RefCell<HashMap<u64, Bounds<Pixels>>>,
    /// The tabs the core said no to, where it was asked, so it's asked again only once the tile
    /// moves, as it does when what it is changes.
    refused: RefCell<HashMap<u64, Bounds<Pixels>>>,
    /// The tab whose tile a watch has (`lend`), which nothing here moves or ends till it's back.
    lent: Cell<Option<u64>>,
    /// Whether the core has shown any of them, so shows tiles at all.
    ever: Cell<bool>,
    /// The tab of each picture a tile's end asked for, by request: the page as it was at that end
    /// stands in its place, not the picture from before it went live.
    asked: RefCell<HashMap<u64, u64>>,
}

/// A tile's corners: the picture's, inside the ring the picture of the page the agent is on draws.
pub(super) const TILE_RADIUS: f32 = PICTURE_RADIUS - RING;

impl LiveTiles {
    pub(super) fn new(browser: Rc<dyn Browser>) -> Rc<Self> {
        Rc::new(Self {
            browser,
            live: RefCell::default(),
            refused: RefCell::default(),
            lent: Cell::new(None),
            ever: Cell::new(false),
            asked: RefCell::default(),
        })
    }

    /// The task's rail, with its pages shown live in their pictures once it has drawn: `drawn`
    /// says where each picture was drawn this frame, in the window. None is while `held`, as
    /// while a page is watched over the conversation, which a tile would draw over.
    pub(super) fn over(
        self: &Rc<Self>,
        rail: impl IntoElement,
        drawn: Rc<RefCell<Vec<(u64, Bounds<Pixels>)>>>,
        held: bool,
    ) -> gpui::Div {
        let tiles = self.clone();
        // Over the rail and drawn after it, at the rail's place, which scrolling doesn't move.
        let place = canvas(
            |_, _, _| {},
            move |area, _, _, cx| tiles.place(&drawn.borrow(), area, held, cx),
        )
        .absolute()
        .size_full();
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(rail)
            .child(place)
    }

    /// Shows the page of each picture wholly inside `area`, moves those that moved, and ends the
    /// rest.
    fn place(
        &self,
        drawn: &[(u64, Bounds<Pixels>)],
        area: Bounds<Pixels>,
        held: bool,
        cx: &mut App,
    ) {
        let wanted: Vec<_> = (drawn.iter())
            .filter(|_| !held)
            .filter(|(tab, _)| self.lent.get() != Some(*tab))
            .filter(|(_, picture)| inside(*picture, area))
            .map(|(tab, picture)| (*tab, page_in(*picture)))
            .collect();
        let gone: Vec<u64> = (self.live.borrow().keys())
            .filter(|tab| !wanted.iter().any(|(each, _)| each == *tab))
            .copied()
            .collect();
        for tab in gone {
            self.end(tab, cx);
        }
        self.refused
            .borrow_mut()
            .retain(|tab, _| drawn.iter().any(|(each, _)| each == tab));
        for (tab, at) in wanted {
            if self.live.borrow().get(&tab) == Some(&at)
                || self.refused.borrow().get(&tab) == Some(&at)
            {
                continue;
            }
            // The rail scrolls under its pictures, so the wheel over one is the rail's.
            if self.browser.show_tile(tab, at, TILE_RADIUS, false) {
                self.ever.set(true);
                self.refused.borrow_mut().remove(&tab);
                if self.live.borrow_mut().insert(tab, at).is_none() {
                    // A run on the Mac reads these lines to see a page go live and end.
                    log!("browser: task tile {tab} live");
                }
            } else {
                self.refused.borrow_mut().insert(tab, at);
                if self.live.borrow_mut().remove(&tab).is_some() {
                    self.picture_at_end(tab, cx);
                    log!("browser: task tile {tab} ended");
                }
            }
        }
    }

    /// Whether the core shows these pages in tiles, as it does on the Mac: it has shown one.
    pub(super) fn shown_any(&self) -> bool {
        self.ever.get()
    }

    /// Lends `tab`'s tile to a watch of its page, which grows it to the area and moves it from
    /// then on (#3788), and says where it was, if the core showed it.
    pub(super) fn lend(&self, tab: u64) -> Option<Bounds<Pixels>> {
        self.lent.set(Some(tab));
        self.refused.borrow_mut().remove(&tab);
        self.live.borrow_mut().remove(&tab)
    }

    /// Takes back the tile a watch had, which the core shows `at`, or none once the user took the
    /// page over: the next frame moves it to its picture, or ends it.
    pub(super) fn take_back(&self, tab: u64, at: Option<Bounds<Pixels>>) {
        if self.lent.get() == Some(tab) {
            self.lent.set(None);
        }
        if let Some(at) = at {
            self.live.borrow_mut().insert(tab, at);
        }
    }

    fn end(&self, tab: u64, cx: &mut App) {
        if self.live.borrow().contains_key(&tab) {
            self.picture_at_end(tab, cx);
        }
        self.end_unpictured(tab);
    }

    fn end_unpictured(&self, tab: u64) {
        if self.live.borrow_mut().remove(&tab).is_some() {
            self.browser.end_tile(tab);
            log!("browser: task tile {tab} ended");
        }
    }

    /// Asks for the picture that stands in `tab`'s tile's place from its end, while the page is
    /// still in the tile.
    fn picture_at_end(&self, tab: u64, cx: &mut App) {
        let request = next_request();
        self.asked.borrow_mut().insert(request, tab);
        self.browser.task_picture(tab, request, cx);
    }

    /// The requests out for the pictures tiles' ends asked for.
    #[cfg(test)]
    pub(super) fn asked_at_ends(&self) -> Vec<u64> {
        self.asked.borrow().keys().copied().collect()
    }

    /// The tab whose tile's end asked for the picture that answers `request`, if one did.
    pub(super) fn asked_at_end(&self, request: u64) -> Option<u64> {
        self.asked.borrow_mut().remove(&request)
    }

    /// Ends every tile, as the conversation's pictures aren't drawn: it stands in its panel beside
    /// a page, a page is watched over it, or the page area shows something else, such as a tab or
    /// another Space.
    pub(super) fn end_all(&self, cx: &mut App) {
        let tabs: Vec<u64> = self.live.borrow().keys().copied().collect();
        for tab in tabs {
            self.end(tab, cx);
        }
        self.refused.borrow_mut().clear();
    }

    /// Ends every tile, asking for no picture, as the conversation's pictures go with it and are
    /// taken again if it shows again.
    pub(super) fn let_go(&self) {
        let tabs: Vec<u64> = self.live.borrow().keys().copied().collect();
        for tab in tabs {
            self.end_unpictured(tab);
        }
        self.refused.borrow_mut().clear();
        self.asked.borrow_mut().clear();
    }
}

impl Drop for LiveTiles {
    /// The task's conversation went, as the task ended or its window closed.
    fn drop(&mut self) {
        self.let_go();
    }
}

/// Whether `picture` is wholly inside `area`.
fn inside(picture: Bounds<Pixels>, area: Bounds<Pixels>) -> bool {
    let (top_left, bottom_right) = (picture.origin, picture.bottom_right());
    top_left.x >= area.origin.x
        && top_left.y >= area.origin.y
        && bottom_right.x <= area.bottom_right().x
        && bottom_right.y <= area.bottom_right().y
}

/// The page's place in its picture: inside the ring the picture of the page the agent is on
/// draws, which the page would hide, so the page doesn't change size as the agent moves on.
pub(super) fn page_in(picture: Bounds<Pixels>) -> Bounds<Pixels> {
    picture.inset(px(RING))
}

#[cfg(test)]
mod tests;
