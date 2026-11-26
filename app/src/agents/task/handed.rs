//! A tab of the user's, handed to a thread. Only the user's send hands one: the composer's own
//! chips are the one source of the tabs (`Picked::of_chips`), a thread's start and a reply to it
//! are the two callers of the give, and no tool, page or typed text reaches either. A handed tab
//! is one of its thread's pages, under the thread's level as any other, until it goes back. Which
//! tabs a thread holds is kept in memory alone: never in the kept list, which holds nothing of a
//! user's tab.

use std::sync::mpsc;

use gpui::{App, Context, SharedString};
use serde_json::Value;

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
    /// Its thread, which closed it.
    Thread,
    /// Its thread's end: it stopped being under way, by its answer, the user's Stop or any other
    /// way, or goes.
    Ended,
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
    /// no tab's id reaches it from an agent's call, a page or typed text. It's the library's own,
    /// as the composer is: nothing outside it makes a pick.
    // Uncalled until the composer's send is wired.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn of_chips(space_in_view: u64, tabs: Vec<u64>) -> Self {
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

/// Which of a thread's replies that wait for its turn to end one is: its own for as long as the
/// thread lasts, and never another's after it. Made only as a reply starts to wait and read only
/// from `held_replies`, so nothing typed, called or on a page names one. In memory alone.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Waiting(u64);

/// That it is one, never which: no log line or failed test carries it.
impl std::fmt::Debug for Waiting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Waiting")
    }
}

/// The title of a thread that has none yet and no tab's to take (table Z2b's word).
pub const NEW_THREAD: &str = "New Thread";

/// The title a thread reads wherever one is drawn, on its card, in its panel and in its row in
/// the Archive and in History: its `own`, or while it has none `first`, the first tab its first
/// message handed (`Sends::first`), else NEW_THREAD. So no row of a thread reads an empty title,
/// and each reads what the others do.
pub fn titled(own: SharedString, first: Option<&SharedString>) -> SharedString {
    match (own.is_empty(), first) {
        (false, _) => own,
        (true, Some(first)) => first.clone(),
        (true, None) => SharedString::new_static(NEW_THREAD),
    }
}

/// What a thread's messages of the user's carried of their tabs. In memory alone, and gone with
/// the thread.
#[derive(Default)]
pub(in crate::agents) struct Sends {
    /// The chips of each message that had any, by which of the user's messages it is: 0 the one
    /// that started the thread, then each reply in turn.
    chips: Vec<(usize, Vec<Chip>)>,
    /// The first tab the message that started the thread handed, by its title as its chip shows
    /// it, held as a title is: what the thread is titled by until its assistant names it. None
    /// where that title is empty or no tab went.
    first: Option<SharedString>,
    /// The replies held until the turn under way ends, in the order sent, each with its tabs'
    /// titles as they were at the user's send: one with a tab of theirs, sent while the thread
    /// was at work, and every reply sent after it.
    held: Vec<(Waiting, Reply, Vec<SharedString>)>,
    /// How many replies have waited in this thread: the next one's `Waiting`.
    waited: u64,
    /// Those never given, as the thread was no longer under way before the turn ended: the
    /// user's to have back, words and picks.
    unsent: Vec<Reply>,
    /// The tabs that went back to the user as the thread was last no longer under way, each with
    /// the site it came back on: what their next reply to it shows picked again.
    came_back: Vec<(u64, Option<String>)>,
}

impl Sends {
    /// Whether a reply with `tabs` to the thread at work waits for the turn under way to end: one
    /// with a tab of the user's does, as nothing is handed mid-turn, unless the thread
    /// `waits_on_user` then; and so does any sent while one waits, which goes after it.
    pub(super) fn holds(&self, tabs: &Picked, waits_on_user: bool) -> bool {
        !self.held.is_empty() || (!tabs.is_empty() && !waits_on_user)
    }

    /// `reply` waits for the turn under way to end, after those that wait already.
    pub(super) fn hold(&mut self, reply: Reply, titles: Vec<SharedString>) {
        self.held.push((Waiting(self.waited), reply, titles));
        self.waited += 1;
    }

    /// The turn under way ended: the replies held for it, to give in their order.
    pub(super) fn take_held(&mut self) -> Vec<(Reply, Vec<SharedString>)> {
        let held = std::mem::take(&mut self.held).into_iter();
        held.map(|(_, reply, titles)| (reply, titles)).collect()
    }

    /// The user's reply was given: it carried what they left picked, and nothing is picked for
    /// the next.
    pub(super) fn replied(&mut self) {
        self.came_back.clear();
    }

    /// The user took back the reply `which` of those held: it is never given, and theirs to have
    /// back as one never given is. Whether it was still held.
    fn unhold(&mut self, which: Waiting) -> bool {
        let Some(at) = self.held.iter().position(|(held, ..)| *held == which) else {
            return false;
        };
        let (_, reply, _) = self.held.remove(at);
        self.unsent.push(reply);
        true
    }

    /// The thread is no longer under way: what was held is never given.
    fn not_given(&mut self) {
        let held = self.held.drain(..);
        self.unsent.extend(held.map(|(_, reply, _)| reply));
    }

    /// What a thread with no title yet is titled by (`titled`): the first tab its first message
    /// handed, if one went and had a title.
    pub(super) fn first(&self) -> Option<&SharedString> {
        self.first.as_ref()
    }

    /// The user's message `said` showed `chips`.
    pub(super) fn showed(&mut self, said: usize, chips: Vec<Chip>) {
        // The message that started the thread: its first handed tab's title is taken here, once,
        // from what the chip shows, so no later title of the page reaches the thread's.
        if said == 0 {
            let first = chips.iter().find(|chip| chip.handed);
            let title = first.map(|chip| super::as_title(chip.title.trim()));
            self.first = title.filter(|title| !title.is_empty()).map(Into::into);
        }
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
        let site = site(&self.browser.tab(tab, cx).url);
        // Its Space is made by now at a start, and here for a thread taken up again.
        let space = self.space(agent, cx);
        if !self.browser.give_tab(tab, space, cx) {
            return false;
        }
        // The page is as the user left it, with whatever they put in it, so a kept card keeps no
        // picture of it until a new document (kept/unkept.rs).
        self.unkept.mark(tab);
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
        (self.thread_space(agent)).is_some_and(|own| self.fits(own, from, tab, cx))
    }

    /// The Space of `agent`'s thread, where it's one the app hands a tab to at all.
    fn thread_space(&self, agent: &str) -> Option<u64> {
        // A ⌘T thread alone is handed a tab, and none while tasks are off.
        let thread = (self.task(agent)).filter(|task| task.cli.is_some() && task.session.is_none());
        let task = thread.filter(|_| self.threads())?;
        Some(match task.space {
            0 => task.home,
            space => space,
        })
    }

    /// Whether `tab` is one the app would hand from the user's Space `from` to a thread of the
    /// Space `own`: the user's own tab, in the Space the window shows, never a private window's
    /// and never one of another profile than the thread's, on a page on the web, as every page a
    /// thread has.
    fn fits(&self, own: u64, from: u64, tab: u64, cx: &App) -> bool {
        self.space_fits(own, from) && self.tab_fits(from, tab, cx)
    }

    /// What `fits` asks that is the same for every tab of the Space `from`.
    fn space_fits(&self, own: u64, from: u64) -> bool {
        !self.browser.private_space(from)
            && self.browser.space_profile(from) == self.browser.space_profile(own)
    }

    /// What `fits` asks of `tab` itself.
    fn tab_fits(&self, from: u64, tab: u64, cx: &App) -> bool {
        self.browser.shows(tab, from, cx) && self.browser.off_limits(tab, cx).is_none()
    }

    /// Whether the composer may offer `tab`, and keep its chip, seen from the user's Space
    /// `space_in_view`: for a reply to `agent`'s thread, or with none for a thread not yet
    /// started. As far as the app knows: the core refuses the rest at the give. A read of what
    /// the app has, which asks nothing of the tab and makes nothing happen in it; for the
    /// composer alone, when its menu of the user's tabs opens and for a chip before the send,
    /// and in no tool's answer or log line.
    pub fn still_fits(&self, agent: Option<&str>, tab: u64, space_in_view: u64, cx: &App) -> bool {
        !(self.fits_among(agent, space_in_view, &[tab], cx)).is_empty()
    }

    /// Those of `tabs` the composer may offer, in their order: what `still_fits` answers for
    /// each, for its menu of the user's tabs as it opens, with what is the thread's and the
    /// Space's asked once for them all. The same read, under the same rule: it asks nothing of
    /// a tab, and no tool's answer or log line carries it.
    pub fn fits_among(
        &self,
        agent: Option<&str>,
        space_in_view: u64,
        tabs: &[u64],
        cx: &App,
    ) -> Vec<u64> {
        let own = match agent {
            Some(agent) => self.thread_space(agent),
            None => self.threads().then_some(space_in_view),
        };
        if !own.is_some_and(|own| self.space_fits(own, space_in_view)) {
            return Vec::new();
        }
        let fits = |tab: &u64| self.tab_fits(space_in_view, *tab, cx);
        tabs.iter().copied().filter(fits).collect()
    }

    /// Whether any of `picked` would be handed to a thread started from the user's Space `home`,
    /// as far as the app knows before there is a thread: with no words, a start needs one.
    pub(super) fn any_to_hand(&self, home: u64, picked: &Picked, cx: &App) -> bool {
        let fits = |tab: &u64| self.fits(home, picked.space, *tab, cx);
        self.threads() && picked.tabs.iter().any(fits)
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

    /// The replies to `agent`'s thread that wait for its turn under way to end, in the order sent,
    /// each with which it is for its ✕: for the composer to show as not yet given.
    pub fn held_replies(&self, agent: &str) -> Vec<(Waiting, Reply)> {
        let Some(task) = self.task(agent) else {
            return Vec::new();
        };
        let held = task.sends.held.iter();
        held.map(|(which, reply, _)| (*which, reply.clone()))
            .collect()
    }

    /// The user's press on the ✕ of the reply `which` of those that wait for `agent`'s thread's
    /// turn to end, as `held_replies` named it: it waits no longer, and `take_unsent` hands it
    /// back. Those behind it wait on. One that no longer waits does nothing, as none does once
    /// the thread is no longer under way. Nothing of it is given, handed or entered in the log.
    pub(super) fn unhold(&mut self, agent: &str, which: Waiting, cx: &mut Context<Self>) {
        if (self.task_mut(agent)).is_some_and(|task| task.sends.unhold(which)) {
            cx.notify();
        }
    }

    /// The replies to `agent`'s thread that were never given, as it was stopped or ended before
    /// its turn under way did or the user took them back from their wait: their words and picks,
    /// in the order they left it, handed back as data and no longer anywhere else. Nothing of
    /// them was given, handed or entered in the log.
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
        self.return_tab_at(agent, tab, by, None, cx)
    }

    /// `return_tab` of a tab whose page is at `at`, for the window that asks from inside its own
    /// update: there the browser's `tab` can't be asked, as it reads that window. With none the
    /// address is looked up.
    fn return_tab_at(
        &mut self,
        agent: &str,
        tab: u64,
        by: By,
        at: Option<&str>,
        cx: &mut Context<Self>,
    ) -> bool {
        // No longer the thread's from here, whatever the core's report of the move then asks.
        if !self.forget_handed_of(agent, tab) {
            return false;
        }
        let site = match at {
            Some(url) => site(url),
            None => site(&self.browser.tab(tab, cx).url),
        };
        let space = self.browser.space_of(tab, cx);
        // The user's own hold on it ends with it: the tab is theirs, with nothing to hand back.
        // A call that waited on the hold hears they returned it only where they did: at their
        // Stop it hears the stop, as one that waits on any hold does.
        let hold_ends = match by {
            By::User => held::Event::Returned,
            By::Thread => held::Event::GivenBack { stopped: false },
            By::Ended => held::Event::GivenBack { stopped: true },
        };
        self.let_go(tab, hold_ends, cx);
        self.forget_page(tab);
        match by {
            By::User => self.returned_by_user(agent, tab),
            // Its thread knows why its own page went, and the core ends what it was doing there.
            By::Thread | By::Ended => {}
        }
        // Where the core can't put it back, as when the Space it left is gone, it's a tab of
        // today in a Space of the user's.
        if !self.browser.return_tab(tab, cx) && space != 0 {
            let users = self.browser.user_space(space);
            self.browser.move_tab(tab, users, cx);
        }
        let entry = match by {
            By::User => TAKEN_BACK,
            By::Thread | By::Ended => GIVEN_BACK,
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
            if self.return_tab(agent, tab, By::Ended, cx)
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
    /// is to close. Their window closes it from inside its own update, so it gives the address
    /// of a page it has (`address`), asked only of one a thread holds.
    pub fn close_returns(
        &mut self,
        tab: u64,
        address: &dyn Fn(u64) -> SharedString,
        cx: &mut Context<Self>,
    ) -> bool {
        let held = &self.tasks.handed.held;
        let Some(agent) = (held.iter().find(|each| each.tab == tab)).map(|each| each.agent.clone())
        else {
            return false;
        };
        let at = address(tab);
        self.return_tab_at(&agent, tab, By::User, Some(&at), cx)
    }

    /// The user closes `tab`, with its split view where `split`. Each pane a thread holds of theirs
    /// is returned as their close of it alone returns it, so every one has its entry and the
    /// core's own refusal to close one never has to act. The panes still to close.
    pub fn close_leaves(
        &mut self,
        tab: u64,
        split: bool,
        address: &dyn Fn(u64) -> SharedString,
        cx: &mut Context<Self>,
    ) -> Vec<u64> {
        let mut panes = match split {
            true => self.browser.split(tab, cx),
            false => Vec::new(),
        };
        if !panes.contains(&tab) {
            panes.insert(0, tab);
        }
        panes.retain(|&pane| !self.close_returns(pane, address, cx));
        panes
    }

    /// The user moves `tab` to a Space of theirs, on the product's own UI. One a thread holds of
    /// theirs is returned first, as by Return to Your Tabs, so the move is of a tab of their own.
    /// Whether it was one.
    pub fn move_returns(
        &mut self,
        tab: u64,
        address: &dyn Fn(u64) -> SharedString,
        cx: &mut Context<Self>,
    ) -> bool {
        self.close_returns(tab, address, cx)
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
    /// that is still open is the user's again: the core has taken it back already. Who moved it
    /// isn't known here, so its entry names nobody. The user's own roads return it themselves
    /// first, and say so (`close_returns`, `move_returns`).
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
            self.let_go(tab, held::Event::GivenBack { stopped: false }, cx);
            self.forget_page(tab);
            self.note_page(&agent, (0, tab), GIVEN_BACK, &site, "");
            cx.notify();
        }
    }

    /// Whether `tab` is one the user returned to their own tabs from `agent`'s thread, and hasn't
    /// handed to a thread since.
    fn returned_from(&self, agent: &str, tab: u64) -> bool {
        matches!(self.gone.get(&tab), Some((whose, Gone::Returned)) if whose == agent)
    }

    /// Whether `agent`'s thread waits on the user by a tab: a step it handed them, or a tab they
    /// took over that it asked back. A call of its own waits on the hold, so a tab of theirs the
    /// user sends it then goes at their send, as one sent with the answer to its question does.
    pub(super) fn waits_on_a_hold(&self, agent: &str) -> bool {
        let mut held = self.held.values().filter(|held| held.agent == agent);
        held.any(|held| held.reply.is_some())
    }

    /// Whether a call of `agent`'s with `arguments` names a tab the user returned from its thread.
    pub(in crate::agents) fn names_returned(&self, agent: &str, arguments: &Value) -> bool {
        let named = tools::tabs_named(arguments);
        named.iter().any(|tab| self.returned_from(agent, *tab))
    }

    /// What the app keeps of `tab` as a page of an agent's goes with it: the agent's own pin of it,
    /// the names its outline gave the page's elements, and its being the tab a call that names
    /// none acts on.
    fn forget_page(&mut self, tab: u64) {
        self.pins.remove(&tab);
        self.refs.remove(&tab);
        self.current.retain(|_, last| *last != tab);
    }

    /// `tab` is no longer one `agent`'s thread holds. Whether it was one.
    fn forget_handed_of(&mut self, agent: &str, tab: u64) -> bool {
        let held = &mut self.tasks.handed.held;
        let Some(at) = (held.iter()).position(|each| each.tab == tab && each.agent == agent) else {
            return false;
        };
        held.remove(at);
        true
    }

    /// `tab` closed: no thread holds it.
    pub(in crate::agents) fn forget_handed(&mut self, tab: u64) {
        let held = &self.tasks.handed.held;
        let whose = (held.iter().find(|each| each.tab == tab)).map(|each| each.agent.clone());
        if let Some(agent) = whose {
            self.forget_handed_of(&agent, tab);
        }
    }

    /// Whether `tab` is one `agent`'s thread holds of the user's.
    pub fn is_handed(&self, agent: &str, tab: u64) -> bool {
        let held = &self.tasks.handed.held;
        held.iter()
            .any(|each| each.tab == tab && each.agent == agent)
    }
}
