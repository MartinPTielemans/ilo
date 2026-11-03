//! The address of the link the pointer is on, in a plate in the card's bottom corner (docs/ux-spec.md
//! › Link target). The core reports the link of a tab's page; the shell keeps it for the tab the
//! window shows and says where the plate goes, and the plate's window draws it (`crate::link_target`).

use gpui::{Context, WindowTextSystem};

use super::Shell;
use crate::link_target::{self, Plate, Side, Target};

use crate::tokens::shell::link::MIN_WIDTH;

/// The link the plate shows.
pub(super) struct Link {
    tab: u64,
    plate: Plate,
    side: Side,
}

impl Shell {
    /// The pointer is on the link `target` in `tab`'s page, or has left the link (`None`). Only
    /// the page the window shows has a plate, and not in the page's full screen or over the
    /// Library.
    pub fn link_target_changed(
        &mut self,
        tab: u64,
        target: Option<Target>,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = target else {
            if self.link.as_ref().is_some_and(|link| link.tab == tab) {
                self.hide_link(cx);
            }
            return;
        };
        if self.page_fullscreen || self.library.is_some() || !self.shows(tab) {
            return;
        }
        let (card, _) = self.card_frames(self.viewport);
        // Half the card at most, so the pointer has room beside it.
        let max = (f32::from(card.size.width) / 2.).clamp(MIN_WIDTH, link_target::WINDOW_WIDTH);
        let system = self
            .link_system
            .get_or_insert_with(|| WindowTextSystem::new(cx.text_system().clone()));
        let system: &WindowTextSystem = system;
        let plate = link_target::plate(&target, max, |text| link_target::text_width(text, system));
        // It stays on the side it moved to while the pointer is on links, and starts on the left.
        let side = self.link.take().map_or(Side::Left, |link| link.side);
        self.link = Some(Link { tab, plate, side });
        self.show_link(cx);
    }

    /// The pointer is at (`x`, `y`) in the window while the plate shows: near it, the plate goes to
    /// the card's other corner, as Chrome's status bubble does. Near both, it stays.
    pub fn link_pointer(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let (card, _) = self.card_frames(self.viewport);
        let Some(link) = &mut self.link else {
            return;
        };
        let width = link.plate.width;
        let here = link_target::near(link_target::place(card, width, link.side), x, y);
        let there = link_target::near(link_target::place(card, width, link.side.other()), x, y);
        if here && !there {
            link.side = link.side.other();
            self.show_link(cx);
        }
    }

    /// Takes the plate away, if it shows.
    pub(super) fn hide_link(&mut self, cx: &mut Context<Self>) {
        if self.link.take().is_some() {
            self.browser.link_target(None, cx);
        }
    }

    /// The tab shown changed: a plate for a page the window no longer shows goes.
    pub(super) fn link_tab_changed(&mut self, cx: &mut Context<Self>) {
        if self.link.as_ref().is_some_and(|link| !self.shows(link.tab)) {
            self.hide_link(cx);
        }
    }

    /// Puts the plate in its corner of the card as it is now, which moves with the sidebar.
    pub(super) fn show_link(&mut self, cx: &mut Context<Self>) {
        let (card, _) = self.card_frames(self.viewport);
        let Some(link) = &self.link else {
            return;
        };
        let frame = link_target::place(card, link.plate.width, link.side);
        self.browser
            .link_target(Some((link.plate.clone(), frame, link.side)), cx);
    }
}
