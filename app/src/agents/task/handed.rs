//! A tab of the user's, handed to a thread. Only the user's send hands one: the composer's own
//! chips are the one source of the tabs (`Picked::of_chips`), a thread's start and a reply to it
//! are the two callers of the give, and no tool, page or typed text reaches either. A handed tab
//! is one of its thread's pages, under the thread's level as any other, until it goes back. Which
//! tabs a thread holds is kept in memory alone: never in the kept list, which holds nothing of a
//! user's tab.

use std::sync::mpsc;

use gpui::{App, Context, SharedString};

use crate::agents::tier::Sites;
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

    /// The tabs picked, in the chips' order: for the composer to show them again.
    pub fn tabs(&self) -> &[u64] {
        &self.tabs
    }
}

/// How many, and nothing of which: no log line or failed test names a tab of the user's.
impl std::fmt::Debug for Picked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picked({})", self.tabs.len())
    }
}

/// The user's reply to a thread: their words and the tabs of theirs picked for it, either of which
/// may be none. The composer's send alone makes one with tabs.
#[derive(Clone, Default, PartialEq)]
pub struct Reply {
    pub text: SharedString,
    pub tabs: Picked,
}

/// Its words, as a reply always read where one is recorded, then how many tabs if any.
impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.text, f)?;
        match self.tabs.is_empty() {
            true => Ok(()),
            false => write!(f, " with {:?}", self.tabs),
        }
    }
}

impl From<SharedString> for Reply {
    fn from(text: SharedString) -> Self {
        Self {
            text,
            tabs: Picked::default(),
        }
    }
}

impl From<String> for Reply {
    fn from(text: String) -> Self {
        SharedString::from(text).into()
    }
}

impl From<&str> for Reply {
    fn from(text: &str) -> Self {
        SharedString::from(text.to_string()).into()
    }
}

/// A tab of the user's picked for a message, as the message shows it: its title as it was at
/// their send, and whether it was handed to the thread. One that wasn't shows so. In memory alone,
/// with its thread: never in the kept list, a tool's answer or a log line.
#[derive(Clone, PartialEq)]
pub struct Chip {
    pub tab: u64,
    pub title: SharedString,
    pub handed: bool,
}

/// Which tab and whether it went, never its title: no failed test names a tab of the user's.
impl std::fmt::Debug for Chip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Chip({}, {})", self.tab, self.handed)
    }
}

/// What a thread's messages of the user's carried of their tabs. In memory alone, and gone with
/// the thread.
#[derive(Default)]
pub(in crate::agents) struct Sends {
    /// The chips of each message that had any, by which of the user's messages it is: 0 the one
    /// that started the thread, then each reply in turn.
    chips: Vec<(usize, Vec<Chip>)>,
    /// The replies held until the turn under way ends, in the order sent, each with its tabs'
    /// titles as they were at the user's send: one with a tab of theirs, sent while the thread
    /// was at work, and every reply sent after it.
    held: Vec<(Reply, Vec<SharedString>)>,
    /// Those never given, as the thread was no longer under way before the turn ended: the
    /// user's to have back, words and picks.
    unsent: Vec<Reply>,
    /// The tabs that went back to the user as the thread was last no longer under way, each with
    /// the site it came back on: what their next reply to it shows picked again.
    came_back: Vec<(u64, Option<String>)>,
}

impl Sends {
    /// Whether a reply with `tabs` to the thread at work waits for the turn under way to end: one
    /// with a tab of the user's does, as nothing is handed mid-turn, and so does any sent while
    /// one waits, which goes after it.
    pub(super) fn holds(&self, tabs: &Picked) -> bool {
        !tabs.is_empty() || !self.held.is_empty()
    }

    /// `reply` waits for the turn under way to end, after those that wait already.
    pub(super) fn hold(&mut self, reply: Reply, titles: Vec<SharedString>) {
        self.held.push((reply, titles));
    }

    /// The turn under way ended: the replies held for it, to give in their order.
    pub(super) fn take_held(&mut self) -> Vec<(Reply, Vec<SharedString>)> {
        std::mem::take(&mut self.held)
    }

    /// The user's reply was given: it carried what they left picked, and nothing is picked for
    /// the next.
    pub(super) fn replied(&mut self) {
        self.came_back.clear();
    }

    /// The thread is no longer under way: what was held is never given.
    fn not_given(&mut self) {
        let held = self.held.drain(..);
        self.unsent.extend(held.map(|(reply, _)| reply));
    }

    /// The user's message `said` showed `chips`.
    pub(super) fn showed(&mut self, said: usize, chips: Vec<Chip>) {
        if !chips.is_empty() {
            self.chips.push((said, chips));
        }
    }

    /// What a new run of the thread from its own words is told of the tabs the user's message
    /// `said` handed it: how many, and nothing of which, as they went back since.
    pub(super) fn told_again(&self, said: usize) -> Option<String> {
        let chips = self.chips.iter().find(|(which, _)| *which == said);
        added(chips.map_or(0, |(_, chips)| {
            chips.iter().filter(|chip| chip.handed).count()
        }))
    }
}

/// A tab a thread holds of the user's.
struct Handed {
    agent: String,
    tab: u64,
    /// The site it was on at the give, its registrable domain.
    site: Option<String>,
}

/// The tabs the threads hold of the user's, in the order handed. In memory alone.
#[derive(Default)]
pub(in crate::agents) struct Records {
    held: Vec<Handed>,
    /// The sites the gives added to those a thread may use, each with its thread: a site that
    /// wasn't the thread's before, which leaves with the last handed tab of it.
    joined: Vec<(String, String)>,
}

impl Records {
    /// The user set the sites `agent`'s thread may use themselves: they're as set, whatever the
    /// gives had added, and no return changes them.
    pub(in crate::agents) fn sites_set(&mut self, agent: &str) {
        self.joined.retain(|(whose, _)| whose != agent);
    }
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
    let added = added(tabs.len())?;
    Some(format!("{added}\n{}", tools::tab_list(tabs, None, &[])))
}

/// That the user added `count` of their tabs to the thread's pages. None for none.
fn added(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("The user added one of their tabs to your pages.".to_string()),
        n => Some(format!("The user added {n} of their tabs to your pages.")),
    }
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
        let url = self.browser.tab(tab, cx).url;
        let (site, domain) = (site(&url), self.browser.url_site(&url));
        // Its Space is made by now at a start, and here for a thread taken up again.
        let space = self.space(agent, cx);
        if !self.browser.give_tab(tab, space, cx) {
            return false;
        }
        // Whatever it left an agent's Space for before, it's this thread's page now.
        self.gone.remove(&tab);
        let agent = agent.to_string();
        self.site_joins(&agent, domain.as_deref());
        self.tasks.handed.held.push(Handed {
            agent: agent.clone(),
            tab,
            site: domain,
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
        let own = match task.space {
            0 => task.home,
            space => space,
        };
        self.fits(own, from, tab, cx)
    }

    /// Whether `tab` is one the app would hand from the user's Space `from` to a thread of the
    /// Space `own`: the user's own tab, in the Space the window shows, never a private window's
    /// and never one of another profile than the thread's, on a page on the web, as every page a
    /// thread has.
    fn fits(&self, own: u64, from: u64, tab: u64, cx: &App) -> bool {
        !self.browser.private_space(from)
            && self.browser.shows(tab, from, cx)
            && self.browser.space_profile(from) == self.browser.space_profile(own)
            && self.browser.off_limits(tab, cx).is_none()
    }

    /// Whether the composer may offer `tab`, and keep its chip, seen from the user's Space
    /// `space_in_view`: for a reply to `agent`'s thread, or with none for a thread not yet
    /// started. As far as the app knows: the core refuses the rest at the give. A read of what
    /// the app has, which asks nothing of the tab and makes nothing happen in it; for the
    /// composer alone, when its menu of the user's tabs opens and for a chip before the send,
    /// and in no tool's answer or log line.
    pub fn still_fits(&self, agent: Option<&str>, tab: u64, space_in_view: u64, cx: &App) -> bool {
        match agent {
            Some(agent) => self.may_hand(agent, space_in_view, tab, cx),
            None => self.tasks_on() && self.fits(space_in_view, space_in_view, tab, cx),
        }
    }

    /// Whether any of `picked` would be handed to a thread started from the user's Space `home`,
    /// as far as the app knows before there is a thread: with no words, a start needs one.
    pub(super) fn any_to_hand(&self, home: u64, picked: &Picked, cx: &App) -> bool {
        let fits = |tab: &u64| self.fits(home, picked.space, *tab, cx);
        self.tasks_on() && picked.tabs.iter().any(fits)
    }

    /// The titles of the tabs `picked`, in their order, as they are now: read at the user's send,
    /// for the message's chips.
    pub(super) fn titles_of(&self, picked: &Picked, cx: &App) -> Vec<SharedString> {
        let title = |tab: &u64| self.browser.tab(*tab, cx).title;
        picked.tabs.iter().map(title).collect()
    }

    /// The user's message hands `picked` to `agent`'s thread, as `hand_tabs` does. Returns the
    /// message's chips, each by its title of `titles` and with whether its tab went, and what the
    /// thread's assistant is told of those that did.
    pub(super) fn hand_with_message(
        &mut self,
        agent: &str,
        picked: Picked,
        titles: Vec<SharedString>,
        cx: &mut Context<Self>,
    ) -> (Vec<Chip>, Option<String>) {
        let handed = self.hand_tabs(agent, picked, cx);
        let told = self.told_of_handed(agent, &handed, cx);
        let chips = (handed.into_iter().zip(titles))
            .map(|((tab, handed), title)| Chip { tab, title, handed })
            .collect();
        (chips, told)
    }

    /// The chips of `agent`'s thread's messages of the user's that had any, each with which of
    /// their messages it is: 0 the one that started the thread, then each reply in turn. For the
    /// conversation to show, and nothing else: no tool's answer or log line carries it.
    pub fn message_chips(&self, agent: &str) -> Vec<(usize, Vec<Chip>)> {
        (self.task(agent)).map_or_else(Vec::new, |task| task.sends.chips.clone())
    }

    /// The replies to `agent`'s thread that wait for its turn under way to end, in the order sent:
    /// for the composer to show as not yet given.
    pub fn held_replies(&self, agent: &str) -> Vec<Reply> {
        let Some(task) = self.task(agent) else {
            return Vec::new();
        };
        let held = task.sends.held.iter();
        held.map(|(reply, _)| reply.clone()).collect()
    }

    /// The replies to `agent`'s thread that were never given, as it was stopped or ended before
    /// its turn under way did: the user's words and picks, in the order sent, handed back as data
    /// and no longer anywhere else. Nothing of them was given, handed or entered in the log.
    pub fn take_unsent(&mut self, agent: &str) -> Vec<Reply> {
        (self.task_mut(agent)).map_or_else(Vec::new, |task| std::mem::take(&mut task.sends.unsent))
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
        if !self.forget_handed_of(agent, tab, cx) {
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

    /// `agent`'s thread is no longer under way, or goes: every tab it holds of the user's returns
    /// to their tabs, in the order handed, each with its entry, before the thread's record closes
    /// and before anything closes a page of its Space. A wait on the user, an ask or a step handed
    /// to them, is still under way, and returns none. A reply held for the end of its turn is
    /// never given: it hands nothing, and is the user's to have back.
    pub(super) fn hand_all_back(&mut self, agent: &str, cx: &mut Context<Self>) {
        if let Some(task) = self.task_mut(agent) {
            task.sends.not_given();
        }
        for tab in self.handed_order(agent) {
            let came_back_on = self.browser.url_site(&self.browser.tab(tab, cx).url);
            if self.return_tab(agent, tab, By::Thread, cx)
                && let Some(task) = self.task_mut(agent)
            {
                task.sends.came_back.retain(|(each, _)| *each != tab);
                task.sends.came_back.push((tab, came_back_on));
            }
        }
    }

    /// The tabs that went back to the user as `agent`'s thread was last no longer under way, for
    /// the composer of their next reply to it to show picked again, in the order handed. Each
    /// follows its tab as it is now: one that no longer fits, seen from the Space `space_in_view`,
    /// or has left the site it came back on, isn't picked. In memory alone, for that thread's
    /// composer and nothing else: never kept, and in no tool's answer or log line.
    pub fn came_back(&self, agent: &str, space_in_view: u64, cx: &App) -> Vec<u64> {
        let Some(task) = self.task(agent) else {
            return Vec::new();
        };
        let fits = |(tab, site): &&(u64, Option<String>)| {
            self.may_hand(agent, space_in_view, *tab, cx)
                && self.browser.url_site(&self.browser.tab(*tab, cx).url) == *site
        };
        let came_back = task.sends.came_back.iter();
        came_back.filter(fits).map(|(tab, _)| *tab).collect()
    }

    /// The tabs `agent`'s thread holds of the user's, in the order handed.
    pub fn handed_order(&self, agent: &str) -> Vec<u64> {
        let held = self.tasks.handed.held.iter();
        (held.filter(|each| each.agent == agent))
            .map(|each| each.tab)
            .collect()
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
            self.forget_handed_of(&agent, tab, cx);
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

    /// `tab` is no longer one `agent`'s thread holds, and with the last of its site neither is a
    /// site the give added. Whether it was one.
    fn forget_handed_of(&mut self, agent: &str, tab: u64, cx: &mut Context<Self>) -> bool {
        let held = &mut self.tasks.handed.held;
        let Some(at) = (held.iter()).position(|each| each.tab == tab && each.agent == agent) else {
            return false;
        };
        let was = held.remove(at);
        self.site_leaves(agent, was.site.as_deref(), cx);
        true
    }

    /// `tab` closed: no thread holds it.
    pub(in crate::agents) fn forget_handed(&mut self, tab: u64, cx: &mut Context<Self>) {
        let held = &self.tasks.handed.held;
        let whose = (held.iter().find(|each| each.tab == tab)).map(|each| each.agent.clone());
        if let Some(agent) = whose {
            self.forget_handed_of(&agent, tab, cx);
        }
    }

    /// The site of a tab handed to `agent`'s thread joins the sites the thread may use, that one
    /// site, where it has only those the user gave it: the page they handed it is one they gave.
    /// The log's entry of the give says it; no change of access is the user's to read here.
    fn site_joins(&mut self, agent: &str, domain: Option<&str>) {
        let (Some(domain), Some(task)) = (domain, self.task_mut(agent)) else {
            return;
        };
        let Sites::Only(only) = &mut task.sites else {
            return;
        };
        if only.iter().any(|each| each == domain) {
            return;
        }
        only.push(domain.into());
        let joined = &mut self.tasks.handed.joined;
        joined.push((agent.into(), domain.into()));
    }

    /// A site the give added doesn't outlive the hold: once `agent`'s thread holds no handed tab
    /// of `domain`, whoever returned the last, the site is no longer one it may use. One that was
    /// the thread's before the give, or that the user set since, stays.
    fn site_leaves(&mut self, agent: &str, domain: Option<&str>, cx: &mut Context<Self>) {
        let Some(domain) = domain else {
            return;
        };
        let records = &mut self.tasks.handed;
        let of_site = |each: &Handed| each.agent == agent && each.site.as_deref() == Some(domain);
        if records.held.iter().any(of_site) {
            return;
        }
        let added = |(whose, site): &(String, String)| whose == agent && site == domain;
        let Some(at) = records.joined.iter().position(added) else {
            return;
        };
        records.joined.remove(at);
        if let Some(task) = self.task_mut(agent)
            && let Sites::Only(only) = &mut task.sites
        {
            only.retain(|each| each != domain);
            cx.notify();
        }
    }

    /// Whether `tab` is one `agent`'s thread holds of the user's.
    pub fn is_handed(&self, agent: &str, tab: u64) -> bool {
        let held = &self.tasks.handed.held;
        held.iter()
            .any(|each| each.tab == tab && each.agent == agent)
    }
}
