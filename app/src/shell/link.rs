//! The address of the link the pointer is on, in a plate in the card's bottom corner (docs/ux-spec.md
//! › Link target). The core reports the link of a tab's page; the shell keeps it for the tab the
//! window shows and says where the plate goes, and the plate's window draws it (`crate::link_target`).

use gpui::{Context, Task, WindowTextSystem};

use super::Shell;
use crate::link_target::{self, Plate, Side, Target};

use crate::tokens::shell::link::MIN_WIDTH;

/// The link the plate shows.
pub(super) struct Link {
    tab: u64,
    target: Target,
    plate: Plate,
    side: Side,
    /// Whether the plate has the whole address: the pointer rested on a link for WHOLE_AFTER. It
    /// stays so for the next link while the plate shows, as Chrome's status bubble does.
    whole: bool,
    /// The one-shot timer to the whole address, while the plate is cut. Dropping it cancels it.
    growing: Option<Task<()>>,
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
        let last = self.link.take();
        // The same link again keeps its plate, and its wait for the whole address.
        if last
            .as_ref()
            .is_some_and(|link| link.tab == tab && link.target == target)
        {
            self.link = last;
            self.show_link(cx);
            return;
        }
        // It stays on the side it moved to while the pointer is on links, and starts on the left.
        let side = last.as_ref().map_or(Side::Left, |link| link.side);
        let whole = last.is_some_and(|link| link.whole);
        let plate = self.link_plate(&target, whole, cx);
        let growing = (!whole && plate.text() != target.text.as_ref()).then(|| {
            cx.spawn(async move |shell, cx| {
                cx.background_executor()
                    .timer(link_target::WHOLE_AFTER)
                    .await;
                shell.update(cx, |shell, cx| shell.whole_link(cx)).ok();
            })
        });
        self.link = Some(Link {
            tab,
            target,
            plate,
            side,
            whole,
            growing,
        });
        self.show_link(cx);
    }

    /// The plate for `target` in the card as it is now: half of it at most, so the pointer has
    /// room beside it, or with `whole` all its width inside the plate's margins.
    fn link_plate(&mut self, target: &Target, whole: bool, cx: &mut Context<Self>) -> Plate {
        let (card, _) = self.card_frames(self.viewport);
        let width = f32::from(card.size.width);
        let max = match whole {
            true => (width - 2. * link_target::MARGIN).clamp(MIN_WIDTH, link_target::WINDOW_WIDTH),
            false => (width / 2.).clamp(MIN_WIDTH, link_target::RESTING_WIDTH),
        };
        let system = self
            .text_system
            .get_or_insert_with(|| WindowTextSystem::new(cx.text_system().clone()));
        let system: &WindowTextSystem = system;
        link_target::plate(target, max, |text| link_target::text_width(text, system))
    }

    /// The pointer has rested on the link for WHOLE_AFTER: its plate grows to the whole address,
    /// in the same corner. It snaps, as the plate's other changes do.
    fn whole_link(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.link.as_ref().map(|link| link.target.clone()) else {
            return;
        };
        let plate = self.link_plate(&target, true, cx);
        // check.sh reads this line.
        log!(
            "browser: link target whole {} {:.0}",
            plate.text(),
            plate.width
        );
        if let Some(link) = &mut self.link {
            link.plate = plate;
            link.whole = true;
            link.growing = None;
        }
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
