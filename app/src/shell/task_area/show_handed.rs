//! Show Tab on a handoff's card (#4065): the handed page in front of the user, in the window the
//! press came from, as the task's Pages watch it, with the keyboard on it, since the user asked for
//! it. The agent's Space is a task's, which the window shows only through its conversation, so a
//! plain select of the tab gave way to the user's Space and showed nothing. The page shows at once,
//! as a tab switch does, with no thumbnail to zoom from; the keyboard waits while one of the app's
//! overlays has it, until it's gone. Nothing of this reaches the agent.

use gpui::Context;

use super::{Watched, Zoom};
use crate::agents::task::Dot;
use crate::shell::Shell;

impl Shell {
    /// Show Tab on the card of the handoff of `tab`.
    pub(in crate::shell) fn show_handed(&mut self, tab: u64, cx: &mut Context<Self>) {
        let space = (self.task_spaces.iter())
            .find(|space| space.pinned.contains(&tab) || space.today.contains(&tab))
            .map(|space| space.id);
        // Sessions of one agent share its Space: the one waiting is the one that handed the tab.
        let cards = || {
            self.task_cards
                .iter()
                .filter(|card| Some(card.space) == space)
        };
        let agent = (cards().find(|card| card.dot == Dot::Waiting))
            .or_else(|| cards().next())
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
        let watched = self.watched_task_page() == Some((space, tab));
        // A page newly watched takes the keyboard once the core shows it, or the focus lands on
        // the page it showed before.
        self.handed_keys = Some((tab, watched));
        if !watched {
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
        self.give_handed_keys(cx);
        cx.notify();
    }

    /// The core shows `tab` in the task's Space: the handed page Show Tab watched can take the
    /// keyboard.
    pub(in crate::shell) fn handed_tab_shown(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.handed_keys == Some((tab, false)) {
            self.handed_keys = Some((tab, true));
            self.give_handed_keys(cx);
        }
    }

    /// Before each frame, and after Show Tab: the handed page watched gets the keyboard Show Tab
    /// asked for, once the core shows it and none of the app's overlays has the keyboard. It's
    /// forgotten once that page isn't watched.
    pub(in crate::shell) fn give_handed_keys(&mut self, cx: &mut Context<Self>) {
        let Some((tab, shown)) = self.handed_keys else {
            return;
        };
        if self.watched_task_page().map(|(_, watched)| watched) != Some(tab) {
            self.handed_keys = None;
            return;
        }
        if !shown || self.browser.command_bar_open(cx) || crate::menu::showing(cx) {
            return;
        }
        self.handed_keys = None;
        self.browser.focus_page(cx);
    }
}
