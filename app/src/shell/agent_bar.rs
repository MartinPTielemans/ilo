//! What the strip under an agent's page watched says (#365): while the agent is at work there,
//! what it did last, with Take Over, which makes the tab the user's, and Stop; on a tab the user
//! holds, Hand Back, which gives it back. ⌘↩ is Take Over or Hand Back, and ⌘. Stop while the
//! agent is at work, with focus in the page too. The strip is the task area's (task_area.rs).

use gpui::{App, Context, SharedString};

use super::{
    Shell,
    commands::{ShellHandle, keystroke_keys},
    extensions::keystroke_text,
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

/// ⌘. for Stop while the UI has the keyboard, before it's Stop Loading. The page hands it over
/// before it sees it (`run_agent_key`).
pub(super) fn bind(cx: &mut App) {
    cx.intercept_keystrokes(|event, _, cx| {
        if crate::dialog::asking(cx) {
            return;
        }
        let Some(shell) = cx
            .try_global::<ShellHandle>()
            .map(|handle| handle.0.clone())
        else {
            return;
        };
        let keystroke = keystroke_text(&event.keystroke);
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

    /// What the agents do and what the user holds from them, as the Agents tell every window.
    pub fn set_agent_status(&mut self, status: Vec<Status>, cx: &mut Context<Self>) {
        if self.agent_status != status {
            self.agent_status = status;
            cx.notify();
        }
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

/// Stop's key, ⌘., as a chip after its words shows it.
pub(super) fn stop_keys() -> String {
    keystroke_keys(STOP_KEY).concat()
}

/// What the bar says, its title and the line under it, and the tab it's about.
pub(super) fn bar_words(bar: &Bar) -> (String, String, u64) {
    match bar {
        Bar::Working {
            name, doing, tab, ..
        } => (format!("{name} is at work"), doing.to_string(), *tab),
        Bar::Held { name, why, tab } if why.is_empty() => (
            "You have this tab".to_string(),
            format!("{name} can't act in it until you hand it back."),
            *tab,
        ),
        Bar::Held { name, why, tab } => (format!("{name} needs you"), why.to_string(), *tab),
    }
}
