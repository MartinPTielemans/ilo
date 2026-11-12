//! What the strip under an agent's page watched says (#365): while the agent is at work there,
//! what it did last, with Take Over, which makes the tab the user's, and Stop; on a tab the user
//! holds, Hand Back, which gives it back. ⌘↩ is Take Over or Hand Back, and ⌘. Stop while the
//! agent is at work, with focus in the page too. The strip is the task area's (task_area.rs).

#[cfg(target_os = "linux")]
use std::{cell::Cell, time::Duration};
#[cfg(target_os = "linux")]
use web_time::Instant;

use gpui::{App, Context, SharedString};

use super::{
    Shell,
    commands::{Command, keystroke_label},
    extensions::keystroke_text,
    keys::user_keys,
};
use crate::agents::Status;

/// What the bar shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Bar {
    /// The agent with the id `agent` is at work in `tab`, having last done `doing`.
    Working {
        agent: SharedString,
        name: SharedString,
        doing: SharedString,
        tab: u64,
    },
    /// The user holds the tab shown from the agent: handed to them for `why`, or taken over.
    Held {
        name: SharedString,
        why: SharedString,
        tab: u64,
    },
}

impl Bar {
    /// The agent's name, as its card has it.
    pub(super) fn name(&self) -> SharedString {
        match self {
            Bar::Working { name, .. } | Bar::Held { name, .. } => name.clone(),
        }
    }
}

/// The bar's Stop, which is Stop Loading's key elsewhere, so the bar takes it only while it shows
/// Stop. Escape, Stop Loading's other key, stays the page's, as an agent stopped by a stray escape
/// would lose its work.
const STOP_KEY: &str = "cmd-.";

/// How soon after ⌘↩ handed a page back from it the UI's ⌘↩ is the same press: GPUI reads it from
/// the same display in the same turn.
#[cfg(target_os = "linux")]
const SAME_PRESS_WITHIN: Duration = Duration::from_millis(500);

#[cfg(target_os = "linux")]
thread_local! {
    /// When ⌘↩ last handed back a page it was kept from (`run_agent_page_key`). Under Wayland GPUI
    /// gets every key the page gets too, after the app has answered it, and by then Hand Back has
    /// given the UI the keyboard, where that press would run Back to Pinned URL as well.
    static HANDED_BACK_AT: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Whether `keystroke`, the UI's, is GPUI's copy of the press that just handed a page back. Any
/// key the UI gets ends the wait for it, as it's the next after that press.
#[cfg(target_os = "linux")]
fn same_press_as_hand_back(keystroke: &str, cx: &App) -> bool {
    HANDED_BACK_AT
        .take()
        .is_some_and(|at| at.elapsed() < SAME_PRESS_WITHIN)
        && user_keys(cx).command_for(keystroke, false) == Some(Command::BackToPinnedUrl)
}

/// ⌘. for Stop while the UI has the keyboard, before it's Stop Loading. The page hands it over
/// before it sees it (`run_agent_key`).
pub(super) fn bind(cx: &mut App) {
    cx.intercept_keystrokes(|event, _, cx| {
        if crate::dialog::asking(cx) {
            return;
        }
        let Some(shell) = super::commands::key_shell(cx) else {
            return;
        };
        let keystroke = keystroke_text(&event.keystroke);
        #[cfg(target_os = "linux")]
        if same_press_as_hand_back(&keystroke, cx) {
            cx.stop_propagation();
            return;
        }
        if shell
            .update(cx, |shell, cx| shell.run_agent_key(&keystroke, cx))
            .unwrap_or(false)
        {
            cx.stop_propagation();
        }
    })
    .detach();
}

impl Shell {
    /// Stops the agent at work in the Space shown if `keystroke` is the bar's Stop, and says
    /// whether it did.
    pub fn run_agent_key(&mut self, keystroke: &str, cx: &mut Context<Self>) -> bool {
        if keystroke == STOP_KEY && self.stop_talk(cx) {
            return true;
        }
        let Some(Bar::Working { agent, .. }) = self.agent_bar().filter(|_| keystroke == STOP_KEY)
        else {
            return false;
        };
        self.browser.stop_agent(agent, cx);
        true
    }

    /// As `run_agent_key`, for a key the agent's page would get first: also Take Over's and Hand
    /// Back's key (⌘↩, Back to Pinned URL's), which a form in the page would otherwise take as its
    /// own, such as a comment box's Send. Not from the UI's keys, where the command bar's and the
    /// link preview's ⌘↩ are theirs.
    pub fn run_agent_page_key(&mut self, keystroke: &str, cx: &mut Context<Self>) -> bool {
        if self.run_agent_key(keystroke, cx) {
            return true;
        }
        if self.agent_bar().is_none()
            || user_keys(cx).command_for(keystroke, false) != Some(Command::BackToPinnedUrl)
        {
            return false;
        }
        #[cfg(target_os = "linux")]
        if matches!(self.agent_bar(), Some(Bar::Held { .. })) {
            HANDED_BACK_AT.set(Some(Instant::now()));
        }
        self.agent_bar_primary(cx);
        true
    }

    /// What the agents do and what the user holds from them, as the Agents tell every window.
    pub fn set_agent_status(&mut self, status: Vec<Status>, cx: &mut Context<Self>) {
        if self.agent_status != status {
            // While an agent works, a press in its page takes it over, which X11's host hears only
            // while it's asked to.
            if self.agent_status.is_empty() != status.is_empty() {
                self.browser.hear_page_presses(!status.is_empty(), cx);
            }
            self.agent_status = status;
            cx.notify();
        }
    }

    /// A press in the window's page (host_watch_page_presses). In an agent's page while it works
    /// there, the user takes the page over, as Take Over did, and the press goes on to the page
    /// (#1080). In a page the user holds, or one of the user's own, it's a press like any other.
    pub fn page_pressed(&mut self, cx: &mut Context<Self>) {
        let Some((_, shown)) = self.agent_page() else {
            return;
        };
        let Some(Bar::Working { agent, .. }) = self.agent_bar() else {
            return;
        };
        // check.sh reads this line.
        log!("browser: agent page pressed in {shown}");
        self.browser.take_over(agent, shown, cx);
    }

    /// The bar of the agent's page the window shows (`agent_page`), if the agent is at work in its
    /// Space or the user holds that page.
    pub(super) fn agent_bar(&self) -> Option<Bar> {
        let (space, shown) = self.agent_page()?;
        let status = self
            .agent_status
            .iter()
            .find(|status| status.space == space.id)?;
        let name = status.name.clone();
        if let Some((tab, why)) = status.held.iter().find(|(tab, _)| *tab == shown) {
            return Some(Bar::Held {
                name,
                why: why.clone(),
                tab: *tab,
            });
        }
        let tab = self.agents_at.get(&status.agent).copied()?;
        let doing = status.doing.clone();
        Some(Bar::Working {
            agent: status.agent.clone(),
            name,
            doing,
            tab,
        })
    }

    /// Why the user holds `tab` from its agent, if they do: what the agent asked of them, or empty
    /// if they took it over.
    pub(super) fn held_why(&self, tab: u64) -> Option<SharedString> {
        (self.agent_status.iter())
            .flat_map(|status| status.held.iter())
            .find(|(each, _)| *each == tab)
            .map(|(_, why)| why.clone())
    }

    /// Take Over, or Hand Back: the bar's first button and ⌘↩.
    pub(super) fn agent_bar_primary(&mut self, cx: &mut Context<Self>) {
        match self.agent_bar() {
            Some(Bar::Working { agent, tab, .. }) => {
                self.browser.take_over(agent, tab, cx);
                self.select(tab, cx);
            }
            Some(Bar::Held { tab, .. }) => {
                self.browser.hand_back(tab, cx);
                // A task's page watched is the agent's again, so the keys the user types next
                // aren't its.
                if self.watched_task_page().is_some() {
                    self.browser.focus_ui(cx);
                }
            }
            None => {}
        }
    }
}

/// Stop's key as a chip after its words shows it: ⌘. on the Mac, and off it Ctrl+., as a menu
/// spells it.
pub(super) fn stop_keys() -> String {
    keystroke_label(STOP_KEY)
}

/// What the bar says, its title and the line under it, and the tab it's about.
pub(super) fn bar_words(bar: &Bar) -> (String, String, u64) {
    match bar {
        Bar::Working {
            name, doing, tab, ..
        } => (format!("{name} is at work"), doing.to_string(), *tab),
        Bar::Held { name, why, tab } if why.is_empty() => (
            "You're driving".to_string(),
            format!("{name} can't act here"),
            *tab,
        ),
        Bar::Held { name, why, tab } => (format!("{name} needs you"), why.to_string(), *tab),
    }
}
