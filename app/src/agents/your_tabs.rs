//! A thread's look at the user's open tabs, and its copy of one (#4403). The look gives titles
//! and sites alone, from the user's Spaces in the thread's own profile, never a private window's,
//! and never a page off the web, a sensitive site's or a site the user put off limits. The copy
//! opens the page that tab is at as the thread's own open_tab would, so that tab stays the user's.

use gpui::App;
use serde_json::{Value, json};

use super::{Agents, TabInfo, is_web, sensitive, site, tools};
use crate::broom;

/// What open_your_tab answers when the tab it names can't be copied now (YT8).
pub(super) const STALE: &str = "Not done: that tab closed or navigated to another page since you looked. Call your_tabs again.";
/// What your_tabs answers with none to list (YT7).
pub(super) const NONE: &str = "The user has no open tabs you can see.";

impl Agents {
    /// The user's tabs `agent`'s thread may see now, in their Spaces' order, at most as many as
    /// the broom reads.
    fn your_tabs_now(&self, agent: &str, cx: &App) -> Vec<TabInfo> {
        let Some(task) = self.task(agent) else {
            return Vec::new();
        };
        let profile = self.browser.space_profile(task.space);
        (self.browser.user_spaces(&profile).into_iter())
            .flat_map(|space| self.browser.space_tabs(space, cx))
            .filter(|tab| self.listable(&profile, &tab.url))
            .take(broom::MOST)
            .collect()
    }

    /// Whether a page at `url` in `profile` may be listed to a thread: a web page whose site the
    /// user neither marked sensitive nor put off limits, and no identity provider's.
    fn listable(&self, profile: &str, url: &str) -> bool {
        let host = site(url);
        is_web(url)
            && !host.is_empty()
            && !sensitive::identity(&host)
            && !self.is_sensitive(profile, &host)
            && self.fence_of(&host).is_none()
    }

    /// your_tabs' answer, each tab by its number, title and site, and how many it lists. The thread
    /// keeps which tab each number is until it looks again or ends.
    pub(super) fn look_at_your_tabs(&mut self, agent: &str, cx: &App) -> (String, usize) {
        let tabs = self.your_tabs_now(agent, cx);
        let rows: Vec<Value> = (tabs.iter().enumerate())
            .map(|(at, tab)| {
                json!({ "n": at + 1, "title": broom::cut(&tab.title), "site": site(&tab.url) })
            })
            .collect();
        if let Some(task) = self.task_mut(agent) {
            task.seen_tabs = tabs.iter().map(|tab| tab.id).collect();
        }
        let listed = match rows.is_empty() {
            true => NONE.to_string(),
            false => json!({ "tabs": rows }).to_string(),
        };
        (listed, rows.len())
    }

    /// Marks the step of `agent`'s call `call`, an open_tab made of open_your_tab, as a copy of
    /// one of the user's tabs, which its words and its entry in the log say (YT2a, YT4).
    pub(super) fn mark_copy(&mut self, agent: &str, call: u64) {
        let Some(task) = self.task_mut(agent) else {
            return;
        };
        let opens = |step: &super::task::Traced| step.call == call && step.did.tool == "open_tab";
        if let Some(at) = task.trace.iter().rposition(opens) {
            task.trace[at].did.target = tools::FROM_YOUR_TABS.into();
        }
    }

    /// Whether `agent`'s call `call` is a copy of one of the user's tabs (`mark_copy`).
    pub(super) fn is_copy(&self, agent: &str, call: u64) -> bool {
        self.task(agent).is_some_and(|task| {
            (task.trace.iter()).any(|step| {
                step.call == call
                    && step.did.tool == "open_tab"
                    && step.did.target == tools::FROM_YOUR_TABS
            })
        })
    }

    /// The address open_your_tab opens: the page the tab numbered `n` in the thread's last look is
    /// at now, read again with every rule of the look, if it came by a plain address. None for a
    /// number it wasn't given, a tab that closed or went where it can't be listed, and a page
    /// reached by sending a form, which has no address that gives it again.
    pub(super) fn your_tab_url(&self, agent: &str, arguments: &Value, cx: &App) -> Option<String> {
        let n = arguments.get("n")?.as_u64()?;
        let at = usize::try_from(n).ok()?.checked_sub(1)?;
        let tab = *self.task(agent)?.seen_tabs.get(at)?;
        let now = self
            .your_tabs_now(agent, cx)
            .into_iter()
            .find(|seen| seen.id == tab)?;
        (!self.browser.posted(tab, cx)).then(|| now.url.to_string())
    }
}
