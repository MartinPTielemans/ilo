//! Show Tab on a handoff's card (#4065): the handed page in front of the user, in the window the
//! press came from, as the task's Pages watch it, with the keyboard on it, since the user asked for
//! it. The agent's Space is a task's, which the window shows only through its conversation, so a
//! plain select of the tab gave way to the user's Space and showed nothing. The page shows at once,
//! as a tab switch does, with no thumbnail to zoom from; the keyboard waits while one of the app's
//! overlays has it, until it's gone. Nothing of this reaches the agent.

use gpui::Context;

use super::{Watched, Zoom};
use crate::shell::Shell;

impl Shell {
    /// Show Tab on the card of the handoff of `tab`.
    pub(in crate::shell) fn show_handed(&mut self, tab: u64, cx: &mut Context<Self>) {
        let space = (self.task_spaces.iter())
            .find(|space| space.pinned.contains(&tab) || space.today.contains(&tab))
            .map(|space| space.id);
        let agent = (self.task_cards.iter())
            .find(|card| Some(card.space) == space)
            .map(|card| card.agent.clone());
        let (Some(space), Some(agent)) = (space, agent) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: show tab {tab}");
        self.show_needed_task(agent, cx);
        if self.task_open() != Some(space) {
            return;
        }
        if self.watched_task_page() != Some((space, tab)) {
            let (frame, _) = self.page_frame(self.viewport);
            if let Some(area) = &mut self.task_area {
                if let Some(Watched {
                    picture: Some(old),
                    asked: true,
                    ..
                }) = area.page.take()
                {
                    self.stale_pictures.push(old.image());
                }
                area.page = Some(Watched {
                    tab,
                    thumb: frame,
                    picture: None,
                    asked: false,
                    zoom: Zoom::Rest,
                });
            }
            // check.sh reads this line, as for a page watched from its picture.
            log!("browser: task page {tab} watched");
            self.browser.select(tab, cx);
            self.lay_out_page(cx);
        }
        self.handed_keys = Some(tab);
        self.give_handed_keys(cx);
        cx.notify();
    }

    /// Before each frame, and after Show Tab: the handed page watched gets the keyboard Show Tab
    /// asked for, once none of the app's overlays has it.
    pub(in crate::shell) fn give_handed_keys(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.handed_keys else {
            return;
        };
        if self.browser.command_bar_open(cx) || crate::menu::showing(cx) {
            return;
        }
        self.handed_keys = None;
        if self.watched_task_page().map(|(_, watched)| watched) == Some(tab) {
            self.browser.focus_page(cx);
        }
    }
}
