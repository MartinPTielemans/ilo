//! A tab of the user's, handed to a thread. Only the user's send hands one: the composer's own
//! chips are the one source of the tabs (`Picked::of_chips`), a thread's start and a reply to it
//! are the two callers of the give, and no tool, page or typed text reaches either. A handed tab
//! is one of its thread's pages, under the thread's level as any other, until it goes back. Which
//! tabs a thread holds is kept in memory alone: never in the kept list, which holds nothing of a
//! user's tab.

use gpui::{App, Context};

use crate::agents::{Agents, TabInfo, site, tools};

/// The activity log's entry of a tab the user handed to a thread, as the log file names it. The
/// app's own give alone makes one, so the entry says who did it.
pub const HANDED: &str = "tab_handed";

/// The user's tabs picked for a message: the tabs of the composer's chips at the user's send, and
/// the Space the window showed then. It rides the message it was picked for and nothing else.
#[derive(Clone, Default, PartialEq)]
pub struct Picked {
    space: u64,
    tabs: Vec<u64>,
}

impl Picked {
    /// The composer's alone to call, with its own chips' tabs, in their order, as the user sends:
    /// no tab's id reaches it from an agent's call, a page or typed text.
    pub fn of_chips(space_in_view: u64, tabs: Vec<u64>) -> Self {
        let mut picked = Vec::with_capacity(tabs.len());
        for tab in tabs {
            if !picked.contains(&tab) {
                picked.push(tab);
            }
        }
        Self {
            space: space_in_view,
            tabs: picked,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
}

/// How many, and nothing of which: no log line or failed test names a tab of the user's.
impl std::fmt::Debug for Picked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picked({})", self.tabs.len())
    }
}

/// A tab a thread holds of the user's.
struct Handed {
    agent: String,
    tab: u64,
}

/// The tabs the threads hold of the user's, in the order handed. In memory alone.
#[derive(Default)]
pub(in crate::agents) struct Records {
    held: Vec<Handed>,
}

/// What the activity log says of a handed tab's entry, if `action` is one: the whole sentence,
/// with the page's host as the one moved.
pub fn entry(action: &str, site: &str) -> Option<String> {
    let site = tools::stopped_site(site);
    match action {
        HANDED => Some(format!("You moved {site} to the thread")),
        _ => None,
    }
}

/// What the thread's assistant is told in the message that hands it `tabs`, which are its pages
/// by now: how many, then each as list_tabs lists it. With no words of the user's it's the whole
/// message. None for no tab.
fn told(tabs: &[TabInfo]) -> Option<String> {
    let added = match tabs.len() {
        0 => return None,
        1 => "The user added one of their tabs to your pages.".to_string(),
        n => format!("The user added {n} of their tabs to your pages."),
    };
    Some(format!("{added}\n{}", tools::tab_list(tabs, None, &[])))
}

/// `words`, the user's message as its assistant gets it, with `told` after it.
pub(super) fn with_told(words: String, told: Option<String>) -> String {
    match (words.is_empty(), told) {
        (_, None) => words,
        (true, Some(told)) => told,
        (false, Some(told)) => format!("{words}\n\n{told}"),
    }
}

impl Agents {
    /// Hands the tabs `picked` to `agent`'s thread, in their order, each whole or not at all:
    /// checked again here, given by the core and entered in the activity log by its host alone, as
    /// one step. One that isn't handed leaves nothing, no record and no entry, and the others
    /// still go. Returns what each came to. The user's send alone reaches it, through a thread's
    /// start and a reply to it.
    pub(super) fn hand_tabs(
        &mut self,
        agent: &str,
        picked: Picked,
        cx: &mut Context<Self>,
    ) -> Vec<(u64, bool)> {
        let Picked { space, tabs } = picked;
        let mut handed = Vec::with_capacity(tabs.len());
        for tab in tabs {
            handed.push((tab, self.hand_tab(agent, space, tab, cx)));
        }
        handed
    }

    fn hand_tab(&mut self, agent: &str, from: u64, tab: u64, cx: &mut Context<Self>) -> bool {
        if !self.may_hand(agent, from, tab, cx) {
            return false;
        }
        let site = site(&self.browser.tab(tab, cx).url);
        // Its Space is made by now at a start, and here for a thread taken up again.
        let space = self.space(agent, cx);
        if !self.browser.give_tab(tab, space, cx) {
            return false;
        }
        // Whatever it left an agent's Space for before, it's this thread's page now.
        self.gone.remove(&tab);
        let agent = agent.to_string();
        self.tasks.handed.held.push(Handed {
            agent: agent.clone(),
            tab,
        });
        self.note_page(&agent, (0, tab), HANDED, &site, "");
        cx.notify();
        true
    }

    /// Whether the app would hand `tab` to `agent`'s thread from the user's Space `from`. The
    /// core refuses the rest at the give, whatever asks.
    fn may_hand(&self, agent: &str, from: u64, tab: u64, cx: &App) -> bool {
        // A ⌘T thread alone is handed a tab, and none while tasks are off.
        let thread = (self.task(agent)).filter(|task| task.cli.is_some() && task.session.is_none());
        let Some(task) = thread.filter(|_| self.tasks_on()) else {
            return false;
        };
        // The user's own tab, in the Space the window showed at the send: never a private
        // window's, and never one of another profile than the thread's.
        if self.browser.private_space(from) || !self.browser.shows(tab, from, cx) {
            return false;
        }
        let own = match task.space {
            0 => task.home,
            space => space,
        };
        if self.browser.space_profile(from) != self.browser.space_profile(own) {
            return false;
        }
        // A page on the web, as every page a thread has.
        self.browser.off_limits(tab, cx).is_none()
    }

    /// Whether any of `picked` would be handed to a thread started from the user's Space `home`,
    /// as far as the app knows before there is a thread: with no words, a start needs one.
    pub(super) fn any_to_hand(&self, home: u64, picked: &Picked, cx: &App) -> bool {
        let fits = |tab: &u64| {
            !self.browser.private_space(picked.space)
                && self.browser.shows(*tab, picked.space, cx)
                && self.browser.space_profile(picked.space) == self.browser.space_profile(home)
                && self.browser.off_limits(*tab, cx).is_none()
        };
        self.tasks_on() && picked.tabs.iter().any(fits)
    }

    /// What `agent`'s assistant is told of the tabs just handed it, those of `handed` that went.
    pub(super) fn told_of_handed(
        &self,
        agent: &str,
        handed: &[(u64, bool)],
        cx: &App,
    ) -> Option<String> {
        let went = |tab: &TabInfo| handed.contains(&(tab.id, true));
        let tabs: Vec<TabInfo> = (self.tabs_listed(agent, cx).into_iter())
            .filter(went)
            .collect();
        // In the order handed, which is the order of the user's chips.
        let mut ordered = Vec::with_capacity(tabs.len());
        for (tab, _) in handed {
            ordered.extend(tabs.iter().filter(|each| each.id == *tab).cloned());
        }
        told(&ordered)
    }

    /// Whether `tab` is one `agent`'s thread holds of the user's.
    pub fn is_handed(&self, agent: &str, tab: u64) -> bool {
        let held = &self.tasks.handed.held;
        held.iter()
            .any(|each| each.tab == tab && each.agent == agent)
    }
}
