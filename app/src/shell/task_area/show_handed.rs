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
use crate::shell::agent_question::Does;

/// The keyboard Show Tab asked for the handed page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::shell) struct HandedKeys {
    tab: u64,
    /// Whether the core shows the tab yet.
    shown: bool,
    /// Whether the ↩ or Space that pressed Show Tab is still down.
    key_down: bool,
}

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
            // A tab of no task's Space shows as any of the user's tabs.
            return self.select(tab, cx);
        };
        // check.sh reads this line.
        log!("browser: show tab {tab}");
        // The card's keys end with the press, giving nothing back: the keyboard goes to the page
        // (#4065), and a ↩ typed next is the page's.
        self.leave_card_keys(cx);
        self.show_needed_task(agent, cx);
        if self.task_open() != Some(space) {
            return;
        }
        let watched = self.watched_task_page() == Some((space, tab));
        // A page newly watched takes the keyboard once the core shows it, or the focus lands on
        // the page it showed before.
        self.handed_keys = Some(HandedKeys {
            tab,
            shown: watched,
            key_down: self.card_key_down,
        });
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
        if let Some(keys) = &mut self.handed_keys
            && keys.tab == tab
            && !keys.shown
        {
            keys.shown = true;
            self.give_handed_keys(cx);
        }
    }

    /// ↩ or Space on the card's ringed button: the press, as a click's, but if it's Show Tab's,
    /// the page waits for the key to come up.
    pub(in crate::shell) fn press_card_by_key(&mut self, does: Does, cx: &mut Context<Self>) {
        self.card_key_down = true;
        self.press_card(does, cx);
        self.card_key_down = false;
    }

    /// A key coming up: the page Show Tab watched takes the keyboard once the key that pressed
    /// it is up, so neither its repeats nor its release reach the page.
    pub(in crate::shell) fn card_key_went_up(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(keys) = &mut self.handed_keys
            && keys.key_down
            && matches!(key, "enter" | "space")
        {
            keys.key_down = false;
            cx.notify();
        }
    }

    /// Before each frame, and after Show Tab: the handed page watched gets the keyboard Show Tab
    /// asked for, once the core shows it (posted, a turn after the select), the key that pressed
    /// it is up, and the command bar or a menu up at the press has closed. It's forgotten, not
    /// held, once anything else has the keyboard: the page not watched, the UI or the page taking
    /// it (`take_keyboard`, `page_took_keyboard`) or another app (`handed_keys_left`), so the
    /// page never takes it from where the user has gone since, nor brings ilo forward.
    pub(in crate::shell) fn give_handed_keys(&mut self, cx: &mut Context<Self>) {
        let Some(HandedKeys {
            tab,
            shown,
            key_down,
        }) = self.handed_keys
        else {
            return;
        };
        if self.watched_task_page().map(|(_, watched)| watched) != Some(tab) {
            self.handed_keys = None;
            return;
        }
        // A menu of the shell's says so in `menu_open`, one of a page's or a field's in `showing`.
        if !shown
            || key_down
            || self.browser.command_bar_open(cx)
            || self.menu_open
            || crate::menu::showing(cx)
        {
            return;
        }
        self.handed_keys = None;
        // On Windows focus_page's SetFocus can make the window active, so with another app in
        // front it isn't asked at all.
        if !self.browser.app_active() {
            log!("browser: show tab {tab} keyboard left for another app");
            return;
        }
        self.browser.focus_page(cx);
    }

    /// The keyboard left for another app, as Windows' host says: Show Tab's ask goes with it.
    pub(in crate::shell) fn handed_keys_left(&mut self) {
        if let Some(HandedKeys { tab, .. }) = self.handed_keys.take() {
            log!("browser: show tab {tab} keyboard left for another app");
        }
    }
}
