//! What the command bar learns of the user, per profile: the commands they pick (#1395) and the
//! tabs they switch to (#1396), which go first among equal matches. A pick is kept in the profile's
//! file, which the browser reads and writes, and a switch only while its tab is open and the app
//! runs. A private window learns nothing, nor does an agent's Space, and opening Clear Browsing
//! Data forgets the profile's.

use std::collections::HashMap;

use gpui::{App, Global};

use super::{Browser, Command, Shell, ShellHandle};
use crate::usage::{self, Usage};

#[derive(Default)]
struct BarUsage {
    /// Each profile's command picks, by the command's name, once read.
    picks: HashMap<u64, Usage<String>>,
    /// Each profile's tab switches, by the tab.
    switches: HashMap<u64, Usage<u64>>,
}

impl Global for BarUsage {}

impl Shell {
    /// The profile whose use the command bar learns from in this window, None in a private one.
    fn learning_profile(&self) -> Option<u64> {
        let space = self.shown_space().filter(|_| !self.one_space())?;
        Some(space.profile)
    }

    /// Reads the profile's picks the first time its command bar opens.
    pub(super) fn read_bar_picks(&self, cx: &mut App) {
        if let Some(profile) = self.learning_profile() {
            let picks = cx.default_global::<BarUsage>().picks.entry(profile);
            picks.or_insert_with(|| Usage::parse(&self.browser.bar_picks(profile)));
        }
    }

    /// The command picks of the profile, to rank equal matches by.
    pub(super) fn bar_picks<'a>(&self, cx: &'a App) -> Option<&'a Usage<String>> {
        let profile = self.learning_profile()?;
        cx.try_global::<BarUsage>()?.picks.get(&profile)
    }

    /// The user switched to `tab`, unless in an agent's Space: the agent's use isn't the user's.
    pub(super) fn switched_to(&self, tab: u64, cx: &mut App) {
        let Some(profile) = self.learning_profile() else {
            return;
        };
        let agents = |space: &&super::Space| !space.agent.is_empty();
        let in_agent_space = (self.spaces.iter().chain(&self.task_spaces))
            .filter(agents)
            .any(|space| space.pinned.contains(&tab) || space.today.contains(&tab));
        if !in_agent_space {
            let switches = cx.default_global::<BarUsage>().switches.entry(profile);
            switches.or_default().used(tab, usage::now());
        }
    }

    /// How much the switches to `tab` count, for ranking the tabs that match what's typed.
    pub(super) fn switch_score(&self, tab: u64, cx: &App) -> f32 {
        let Some(usage) = cx.try_global::<BarUsage>() else {
            return 0.0;
        };
        let now = usage::now();
        (usage.switches.values())
            .map(|switches| switches.score(&tab, now))
            .sum()
    }

    /// A closed tab's switches go with it. A private window hears only its own tabs, so it leaves
    /// the others' alone.
    pub(super) fn forget_switches(&self, open: impl Fn(&u64) -> bool, cx: &mut App) {
        if self.one_space() {
            return;
        }
        if cx.has_global::<BarUsage>() {
            let switches = cx.global_mut::<BarUsage>().switches.values_mut();
            switches.for_each(|switches| switches.retain(&open));
        }
    }

    /// The user is clearing the browsing data of the window's profile.
    pub(super) fn forget_bar_usage(&self, cx: &mut App) {
        if let Some(profile) = self.learning_profile() {
            forget(profile, &*self.browser, cx);
        }
    }
}

/// The user took `name`'s row for `command` in the command bar, which counts it for the profile.
pub fn picked_in_bar(name: &str, command: Command, cx: &mut App) {
    // Clear Browsing Data forgets what it would count.
    if command == Command::ClearBrowsingData {
        return;
    }
    let Some(shell) = (cx.try_global::<ShellHandle>()).and_then(|handle| handle.0.upgrade()) else {
        return;
    };
    let shell = shell.read(cx);
    let (profile, browser) = (shell.learning_profile(), shell.browser.clone());
    let Some(profile) = profile else {
        return;
    };
    let picks = cx.default_global::<BarUsage>().picks.entry(profile);
    let picks = picks.or_insert_with(|| Usage::parse(&browser.bar_picks(profile)));
    picks.used(name.to_string(), usage::now());
    browser.set_bar_picks(profile, &picks.to_text());
}

/// The user is clearing `profile`'s browsing data, which the core doesn't say when it's done, so
/// what the command bar learned of the profile goes once they open the dialog that clears it.
pub fn forget_bar_usage(profile: u64, cx: &mut App) {
    if let Some(shell) = (cx.try_global::<ShellHandle>()).and_then(|handle| handle.0.upgrade()) {
        let browser = shell.read(cx).browser.clone();
        forget(profile, &*browser, cx);
    }
}

fn forget(profile: u64, browser: &dyn Browser, cx: &mut App) {
    let usage = cx.default_global::<BarUsage>();
    usage.switches.remove(&profile);
    usage.picks.insert(profile, Usage::default());
    browser.set_bar_picks(profile, "");
}

#[cfg(test)]
pub(super) fn picks_of(profile: u64, cx: &App) -> Option<String> {
    let usage = cx.try_global::<BarUsage>()?;
    usage.picks.get(&profile).map(Usage::to_text)
}
