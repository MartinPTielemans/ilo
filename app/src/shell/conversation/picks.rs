//! The user's tabs picked for a reply (`crate::tab_picks`): the add menu's row for them, their
//! chips over the field, and what the send takes of them.

use gpui::Context;

use super::{Look, TalkView};
use crate::agents::task::handed;
use crate::tab_picks::{self, Offered};

impl TalkView {
    /// The add menu's row for the user's tabs, where the window lists any.
    pub(super) fn tabs_row(&self, cx: &mut Context<Self>) -> Option<crate::plus::Row> {
        let shell = self.shell.upgrade()?;
        if !shell.read(cx).lists_tabs_to_offer() {
            return None;
        }
        let (agent, picked, flips) = (self.agent.clone(), self.picks.tabs(), cx.weak_entity());
        Some(tab_picks::row(
            // The add menu opened: the tabs this thread may be handed, asked now and here
            // alone, with which of them are picked.
            move |cx| (shell.read(cx).tabs_to_offer(Some(&agent), cx), picked),
            move |space, offered, cx| {
                let picked = flips.update(cx, |view, cx| view.flip_pick(space, offered, cx));
                picked.unwrap_or(false)
            },
        ))
    }

    /// The user's click on a tab's row of the menu. Whether it is picked now.
    fn flip_pick(&mut self, space: u64, offered: Offered, cx: &mut Context<Self>) -> bool {
        let picked = self.picks.flip(space, offered);
        self.picks_changed(cx);
        picked
    }

    /// The chips changed. The window's tabs are watched while there is one, and not otherwise:
    /// a tab closed, moved to another Space or gone to a page of the browser's own loses its
    /// chip, and one that stays shows as its tab is now.
    fn picks_changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
        if self.picks.is_empty() {
            self.picks_watch = None;
            return;
        }
        let Some(shell) = self.shell.upgrade().filter(|_| self.picks_watch.is_none()) else {
            return;
        };
        self.picks_watch = Some(cx.observe(&shell, |view, shell, cx| {
            let listed = shell.read(cx).tabs_listed();
            if view.picks.keep(&listed) {
                view.picks_changed(cx);
            }
        }));
    }

    /// ⌫ in the empty field takes the last chip. Whether it did.
    pub(super) fn unpick_last(&mut self, cx: &mut Context<Self>) -> bool {
        let field = self.reply.read(cx);
        if field.composing() || !field.text().is_empty() || !self.picks.unpick_last() {
            return false;
        }
        self.picks_changed(cx);
        true
    }

    /// Whether a chip is held, with which a reply sends without words.
    pub(super) fn picked(&self) -> bool {
        !self.picks.is_empty()
    }

    /// The reply's send: the tabs its chips stand for, as the window lists them now, and no
    /// pick is left.
    pub(super) fn take_picks(&mut self, cx: &mut Context<Self>) -> handed::Picked {
        if let Some(shell) = self.shell.upgrade() {
            let listed = shell.read(cx).tabs_listed();
            self.picks.keep(&listed);
        }
        self.picks_watch = None;
        self.picks.take()
    }

    fn chips_look(&self, look: &Look) -> tab_picks::Look {
        let inks = self.inks(look);
        tab_picks::Look {
            fill: inks.plate,
            ink: inks.text,
            dim: inks.dim,
            a11y: look.a11y,
        }
    }

    /// The chips over the reply field, with a pick: each ✕ unpicks its tab.
    pub(super) fn picked_chips(&self, look: &Look, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if self.picks.is_empty() {
            return None;
        }
        let view = cx.weak_entity();
        let unpick = move |tab, _: &mut gpui::Window, cx: &mut gpui::App| {
            view.update(cx, |view, cx| {
                if view.picks.unpick(tab) {
                    view.picks_changed(cx);
                }
            })
            .ok();
        };
        Some(tab_picks::chips(
            &self.picks,
            &self.chips_look(look),
            unpick,
        ))
    }

    /// The chips of the user's message `said` (0 the one that started the thread, then each
    /// reply in turn), inside its plate over its words; none for one sent with no tab.
    pub(super) fn sent_chips(&self, said: usize, look: &Look) -> Option<gpui::Div> {
        let chips = self.shown.as_ref()?.chips.iter();
        let (_, chips) = chips.clone().find(|(which, _)| *which == said)?;
        Some(tab_picks::sent(said, chips, &self.chips_look(look)))
    }
}
