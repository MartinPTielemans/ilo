//! A tab of the user's, handed to a thread. Only the user's send hands one: the composer's own
//! chips are the one source of the tabs (`Picked::of_chips`), a thread's start and a reply to it
//! are the two callers of the give, and no tool, page or typed text reaches either. A handed tab
//! is one of its thread's pages, under the thread's level as any other, until it goes back. Which
//! tabs a thread holds is kept in memory alone: never in the kept list, which holds nothing of a
//! user's tab.

use std::sync::mpsc;

use gpui::{App, Context};

use crate::agents::{Agents, Gone, TabInfo, ToolResult, held, site, tools};

/// The activity log's entry of a tab the user handed to a thread, as the log file names it. The
/// app's own give alone makes one, so the entry says who did it.
pub const HANDED: &str = "tab_handed";
/// The entry of a handed tab the user returned to their own tabs.
pub const TAKEN_BACK: &str = "tab_taken_back";
/// The entry of a handed tab its thread gave back: by closing it, or by being no longer under way.
pub const GIVEN_BACK: &str = "tab_given_back";

/// What a call on a tab the user returned to their own tabs is answered, and all its assistant
/// hears of the return.
pub const RETURNED: &str =
    "The user returned this tab to their own tabs. You can no longer access it.";

/// Who returns a handed tab to the user's tabs, which its entry says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum By {
    /// The user, by Return to Your Tabs, or by their own close of it.
    User,
    /// Its thread, which closed it or is no longer under way.
    Thread,
}

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
        TAKEN_BACK => Some(format!("You returned {site} to your tabs")),
        GIVEN_BACK => Some(format!("{site} returned to your tabs")),
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

    /// Returns `tab`, which `agent`'s thread holds of the user's, to the user's tabs, at the place
    /// it left and on the page it's at now, at once: a hold the user has on it ends, a call under
    /// way on it is answered that the tab is gone, and the activity log says who returned it.
    /// Whether it was the thread's to return.
    pub(in crate::agents) fn return_tab(
        &mut self,
        agent: &str,
        tab: u64,
        by: By,
        cx: &mut Context<Self>,
    ) -> bool {
        // No longer the thread's from here, whatever the core's report of the move then asks.
        if !self.forget_handed_of(agent, tab) {
            return false;
        }
        let site = site(&self.browser.tab(tab, cx).url);
        let space = self.browser.space_of(tab, cx);
        // The user's own hold on it ends with it: the tab is theirs, with nothing to hand back.
        self.let_go(tab, held::Event::Returned, cx);
        self.forget_page(tab);
        match by {
            By::User => self.returned_by_user(agent, tab),
            // Its thread knows why its own page went, and the core ends what it was doing there.
            By::Thread => {}
        }
        // Where the core can't put it back, as when the Space it left is gone, it's a tab of
        // today in a Space of the user's.
        if !self.browser.return_tab(tab, cx) && space != 0 {
            let users = self.browser.user_space(space);
            self.browser.move_tab(tab, users, cx);
        }
        let entry = match by {
            By::User => TAKEN_BACK,
            By::Thread => GIVEN_BACK,
        };
        self.note_page(agent, (0, tab), entry, &site, "");
        cx.notify();
        true
    }

    /// The user closes `tab`. One a thread holds of theirs never closes while it's the thread's:
    /// their close of it is their return of it, on the page it's at. Whether it was one, so nothing
    /// is to close.
    pub fn close_returns(&mut self, tab: u64, cx: &mut Context<Self>) -> bool {
        let held = &self.tasks.handed.held;
        let Some(agent) = (held.iter().find(|each| each.tab == tab)).map(|each| each.agent.clone())
        else {
            return false;
        };
        self.return_tab(&agent, tab, By::User, cx)
    }

    /// `agent`'s close of `tab`, by close_tab or remove_tab. One it holds of the user's never
    /// closes while it's the thread's: the close gives it back, on the page it's at, and the
    /// thread hears what its close of a page of its own says. Whether it was one.
    pub(in crate::agents) fn close_gives_back(
        &mut self,
        agent: &str,
        tab: u64,
        reply: &mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.return_tab(agent, tab, By::Thread, cx) {
            return false;
        }
        // A later call on it hears what one on a page the agent closed does.
        self.went(tab, agent, Gone::Itself);
        _ = reply.send(ToolResult::text(format!("Closed tab {tab}.")));
        true
    }

    /// The user returned `tab` from `agent`'s thread: every call on it from here is answered so,
    /// those under way too, which are dropped.
    fn returned_by_user(&mut self, agent: &str, tab: u64) {
        self.went(tab, agent, Gone::Returned);
        let gone = || ToolResult::error(RETURNED);
        for (_, reading) in self.reading.extract_if(|_, reading| reading.tab == tab) {
            _ = reading.reply.send(gone());
        }
        for (_, acting) in self.acting.extract_if(|_, acting| acting.tab == tab) {
            _ = acting.reply.send(gone());
        }
        for (_, act) in self.describing.extract_if(|_, act| act.tab == tab) {
            _ = act.reply.send(gone());
        }
    }

    /// A tab left `space`, which now has `tabs`. One a thread of that Space held of the user's and
    /// that is still open went to a Space of the user's by their own move, which returns it as
    /// Return to Your Tabs does: the core has taken it back already.
    pub(in crate::agents) fn handed_left(
        &mut self,
        space: u64,
        tabs: &[u64],
        cx: &mut Context<Self>,
    ) {
        let left: Vec<(String, u64)> = (self.tasks.handed.held.iter())
            .filter(|each| !tabs.contains(&each.tab))
            .filter(|each| {
                self.task(&each.agent)
                    .is_some_and(|task| task.space == space)
            })
            .map(|each| (each.agent.clone(), each.tab))
            .collect();
        for (agent, tab) in left {
            if self.browser.space_of(tab, cx) == 0 {
                continue;
            }
            self.forget_handed_of(&agent, tab);
            let site = site(&self.browser.tab(tab, cx).url);
            self.let_go(tab, held::Event::Returned, cx);
            self.forget_page(tab);
            self.returned_by_user(&agent, tab);
            self.note_page(&agent, (0, tab), TAKEN_BACK, &site, "");
            cx.notify();
        }
    }

    /// Whether `tab` is one the user returned to their own tabs from `agent`'s thread, and hasn't
    /// handed to a thread since.
    pub(in crate::agents) fn returned_from(&self, agent: &str, tab: u64) -> bool {
        matches!(self.gone.get(&tab), Some((whose, Gone::Returned)) if whose == agent)
    }

    /// What the app keeps of `tab` as a page of an agent's goes with it: the agent's own pin of it,
    /// the names its outline gave the page's elements, and its being the tab a call that names
    /// none acts on.
    fn forget_page(&mut self, tab: u64) {
        self.pins.remove(&tab);
        self.refs.remove(&tab);
        self.current.retain(|_, last| *last != tab);
    }

    /// `tab` is no longer one `agent`'s thread holds. Whether it was.
    fn forget_handed_of(&mut self, agent: &str, tab: u64) -> bool {
        let held = &mut self.tasks.handed.held;
        let before = held.len();
        held.retain(|each| !(each.tab == tab && each.agent == agent));
        held.len() != before
    }

    /// `tab` closed: no thread holds it.
    pub(in crate::agents) fn forget_handed(&mut self, tab: u64) {
        self.tasks.handed.held.retain(|each| each.tab != tab);
    }

    /// Whether `tab` is one `agent`'s thread holds of the user's.
    pub fn is_handed(&self, agent: &str, tab: u64) -> bool {
        let held = &self.tasks.handed.held;
        held.iter()
            .any(|each| each.tab == tab && each.agent == agent)
    }
}
