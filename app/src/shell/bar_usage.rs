//! What the command bar learns of the user, per profile: the commands they pick (#1395) and the
//! tabs they switch to (#1396), which go first among equal matches. A pick is kept on this computer
//! only, in a file in the profile's own folder, which goes when the profile is deleted and stays
//! with it through a rename; never synced, sent or put in a crash report, and the user's, so no
//! agent path reads it. A switch is kept only while its tab is open and the app runs. A private window learns nothing, nor does an agent's Space, and deleting history forgets
//! what was learned of it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    pub(super) fn learning_profile(&self) -> Option<u64> {
        let space = self.shown_space().filter(|_| !self.one_space())?;
        Some(space.profile)
    }

    /// Reads the profile's picks the first time its command bar opens.
    pub(super) fn read_bar_picks(&self, cx: &mut App) {
        if let Some(profile) = self.learning_profile() {
            let picks = cx.default_global::<BarUsage>().picks.entry(profile);
            picks.or_insert_with(|| read(&*self.browser, profile));
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
}

/// The user took `name`'s row for `command` in the command bar, which counts it for the profile.
pub fn picked_in_bar(name: &str, _command: Command, cx: &mut App) {
    let Some(shell) = (cx.try_global::<ShellHandle>()).and_then(|handle| handle.0.upgrade()) else {
        return;
    };
    let shell = shell.read(cx);
    let (profile, browser) = (shell.learning_profile(), shell.browser.clone());
    let Some(profile) = profile else {
        return;
    };
    let picks = cx.default_global::<BarUsage>().picks.entry(profile);
    let picks = picks.or_insert_with(|| read(&*browser, profile));
    picks.used(name.to_string(), usage::now());
    write(&*browser, profile, picks);
}

/// What the user deleted from a profile's history, as the core says.
#[derive(Clone, Debug, PartialEq)]
pub enum HistoryDeleted {
    All,
    /// The pages at `urls`, and every visit from `begin` to `end`, in seconds since 1970, when
    /// `begin` is before `end`, as Clear Browsing Data's Last hour.
    Some {
        begin: u64,
        end: u64,
        urls: Vec<String>,
    },
}

/// The user deleted pages from `profile`'s history, and what the command bar learned of them goes:
/// everything for all of it; for a span of time, the picks and switches made in it; and the
/// switches to the tabs at the pages deleted. A pick names a command, never a page, so only time
/// ties one to history.
pub fn history_deleted(profile: u64, deleted: HistoryDeleted, cx: &mut App) {
    let Some(shell) = (cx.try_global::<ShellHandle>()).and_then(|handle| handle.0.upgrade()) else {
        return;
    };
    let shell = shell.read(cx);
    let browser = shell.browser.clone();
    let at_deleted: Vec<u64> = match &deleted {
        HistoryDeleted::All => Vec::new(),
        HistoryDeleted::Some { urls, .. } => (shell.tabs.keys().copied())
            .filter(|&tab| urls.iter().any(|url| *url == *shell.url(tab)))
            .collect(),
    };
    // Library › Agents' past tasks go with the history of their time, as the owner asked (#1080).
    let past = crate::agent_history::read(&browser.agent_history(profile));
    let left = match &deleted {
        HistoryDeleted::All => Vec::new(),
        HistoryDeleted::Some { begin, end, .. } => {
            crate::agent_history::outside(past.clone(), *begin, *end)
        }
    };
    if left.len() != past.len() {
        browser.set_agent_history(profile, &crate::agent_history::write(&left));
    }
    let usage = cx.default_global::<BarUsage>();
    let (begin, end) = match deleted {
        HistoryDeleted::All => {
            usage.switches.remove(&profile);
            let picks = usage.picks.entry(profile).or_default();
            *picks = Usage::default();
            return write(&*browser, profile, picks);
        }
        HistoryDeleted::Some { begin, end, .. } => (begin, end),
    };
    if let Some(switches) = usage.switches.get_mut(&profile) {
        switches.forget_between(begin, end);
        switches.retain(|tab| !at_deleted.contains(tab));
    }
    if begin < end {
        let picks = usage.picks.entry(profile);
        let picks = picks.or_insert_with(|| read(&*browser, profile));
        picks.forget_between(begin, end);
        write(&*browser, profile, picks);
    }
}

/// The file in a profile's folder its picks are kept in.
pub const PICKS_FILE: &str = "Command bar picks";

/// Where `profile`'s picks are kept, or None while the browser doesn't know its folder.
pub(super) fn picks_file(browser: &dyn Browser, profile: u64) -> Option<PathBuf> {
    let folder = browser.profile_folder(profile)?;
    Some(folder.join(PICKS_FILE))
}

fn read(browser: &dyn Browser, profile: u64) -> Usage<String> {
    let path = picks_file(browser, profile);
    let text = path.and_then(|path| std::fs::read_to_string(path).ok());
    Usage::parse(&text.unwrap_or_default())
}

/// None picked leaves no file.
fn write(browser: &dyn Browser, profile: u64, picks: &Usage<String>) {
    let Some(path) = picks_file(browser, profile) else {
        return;
    };
    let saved = match picks.is_empty() {
        true => std::fs::remove_file(&path).or_else(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        }),
        false => std::fs::write(&path, picks.to_text()),
    };
    // check.sh reads these lines. The file names no pick.
    match saved {
        Ok(()) if picks.is_empty() => log!("browser: command bar picks none in {}", path.display()),
        Ok(()) => log!("browser: command bar picks kept in {}", path.display()),
        Err(error) => log!("couldn't save the command bar's picks: {error}"),
    }
}

/// Moves the picks from the one file every profile's shared before #1395's second change, `old`, a
/// line a pick, the profile's name, a tab, then the line `Usage` writes, into each named profile's
/// folder, unless one is already there, then removes it. A profile it names that's gone loses its.
pub fn move_old_picks(old: &Path, profiles: &[(String, PathBuf)]) {
    let Ok(text) = std::fs::read_to_string(old) else {
        return;
    };
    for (name, folder) in profiles {
        let new = folder.join(PICKS_FILE);
        let lines = (text.lines())
            .filter_map(|line| line.split_once('\t'))
            .filter(|(profile, _)| profile == name);
        let picks: String = lines.map(|(_, line)| format!("{line}\n")).collect();
        if !picks.is_empty() && !new.exists() && folder.is_dir() {
            if let Err(error) = std::fs::write(&new, picks) {
                log!("couldn't move the command bar's picks: {error}");
                return;
            }
        }
    }
    if let Err(error) = std::fs::remove_file(old) {
        log!("couldn't remove the command bar's old picks: {error}");
    }
}

#[cfg(test)]
pub(super) fn picks_of(profile: u64, cx: &App) -> Option<String> {
    let usage = cx.try_global::<BarUsage>()?;
    usage.picks.get(&profile).map(Usage::to_text)
}
