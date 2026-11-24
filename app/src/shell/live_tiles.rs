//! A task's pages live in their pictures' places (#4115). Where the core can, as on the Mac, an
//! agent's page shows in its picture in the task's rail, drawing as the agent works, while the
//! picture is wholly in the rail's view; partly scrolled out, under a page watched over the
//! conversation, or wherever the core says no, the picture the UI drew stays, as it does on Windows
//! and Linux.
//!
//! The tile is the user's view alone. The core holds the rest (`bc_tab_show_tile`): the page takes
//! no keyboard, focus or click from it, keeps its own size, and neither it nor its agent hears that
//! it's watched. A click on the tile is the UI's, which watches the page as the picture's does.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{Bounds, IntoElement, ParentElement, Pixels, Styled, canvas, div, px};

use super::browser::Browser;
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
}

impl LiveTiles {
    pub(super) fn new(browser: Rc<dyn Browser>) -> Rc<Self> {
        Rc::new(Self {
            browser,
            live: RefCell::default(),
            refused: RefCell::default(),
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
            move |area, _, _, _| tiles.place(&drawn.borrow(), area, held),
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
    fn place(&self, drawn: &[(u64, Bounds<Pixels>)], area: Bounds<Pixels>, held: bool) {
        let wanted: Vec<_> = (drawn.iter())
            .filter(|_| !held)
            .filter(|(_, picture)| inside(*picture, area))
            .map(|(tab, picture)| (*tab, page_in(*picture)))
            .collect();
        let gone: Vec<u64> = (self.live.borrow().keys())
            .filter(|tab| !wanted.iter().any(|(each, _)| each == *tab))
            .copied()
            .collect();
        for tab in gone {
            self.end(tab);
        }
        self.refused
            .borrow_mut()
            .retain(|tab, _| drawn.iter().any(|(each, _)| each == tab));
        let radius = PICTURE_RADIUS - RING;
        for (tab, at) in wanted {
            if self.live.borrow().get(&tab) == Some(&at)
                || self.refused.borrow().get(&tab) == Some(&at)
            {
                continue;
            }
            if self.browser.show_tile(tab, at, radius) {
                self.refused.borrow_mut().remove(&tab);
                if self.live.borrow_mut().insert(tab, at).is_none() {
                    // A run on the Mac reads these lines to see a page go live and end.
                    log!("browser: task tile {tab} live");
                }
            } else {
                self.refused.borrow_mut().insert(tab, at);
                if self.live.borrow_mut().remove(&tab).is_some() {
                    log!("browser: task tile {tab} ended");
                }
            }
        }
    }

    fn end(&self, tab: u64) {
        if self.live.borrow_mut().remove(&tab).is_some() {
            self.browser.end_tile(tab);
            log!("browser: task tile {tab} ended");
        }
    }

    /// Ends every tile, as the conversation no longer shows: another task's shows, or the page
    /// area shows something else, such as a tab or another Space.
    pub(super) fn end_all(&self) {
        let tabs: Vec<u64> = self.live.borrow().keys().copied().collect();
        for tab in tabs {
            self.end(tab);
        }
        self.refused.borrow_mut().clear();
    }
}

impl Drop for LiveTiles {
    /// The task's conversation went, as the task ended or its window closed.
    fn drop(&mut self) {
        self.end_all();
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
fn page_in(picture: Bounds<Pixels>) -> Bounds<Pixels> {
    picture.inset(px(RING))
}

#[cfg(test)]
mod tests;
