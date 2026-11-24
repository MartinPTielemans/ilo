//! How an act meets its task's access (tier.rs): the one decision point, `check`, runs what the
//! tier allows and refuses the rest at once, telling the agent and putting ilo's line in the task's
//! conversation. Nothing waits for the user, and only the user changes a tier or a task's Sites.

use gpui::Context;

use super::tier::{self, Sites, Stop, Tier, Why};
use super::{
    Act, Agents, CHANGES_ASK, Call, DOWNLOADS, FENCED, OTHER_SITE, Refused, UNSAVED, act_of, fence,
    page, site, tools,
};

/// What the log names a change of a task's access, which its entry's target says.
pub const ACCESS_ACTION: &str = "access";

/// The actions that act on a page, which Look only refuses, as `page_tool` names them.
const ACTS: [&str; 4] = ["click", "type", "select_option", "press_key"];

impl Agents {
    /// The access the task `agent`'s act on `connection`, for its call `call` (0 for none), is of:
    /// a ⌘T task's, else its session's, else the one a new session starts with.
    pub(super) fn tier_of(&self, agent: &str, connection: u64, call: u64) -> Tier {
        self.acting_task(agent, connection, call)
            .map_or(Tier::default(), |task| task.tier)
    }

    /// Why `act` on `site` is one a tier may refuse, from what would have asked (`why`), or None
    /// for one that only opens or reads.
    pub(super) fn why_of(act: &Act, site: &str, why: Option<&'static str>) -> Option<Why> {
        let bank = || page::opening_risk(site).is_some();
        match why {
            Some(FENCED) => Some(Why::Fenced),
            Some(OTHER_SITE) => Some(Why::OtherSite),
            Some(page::ORDER) => Some(Why::Order),
            Some(page::SENDS) if bank() => Some(Why::Bank),
            Some(page::SENDS) => Some(Why::Message),
            Some(DOWNLOADS) => Some(Why::Download),
            Some(UNSAVED) => Some(Why::Discard),
            Some(CHANGES_ASK) => Some(Why::Request),
            Some(_) => Some(Why::Bank),
            None if ACTS.contains(&act.action) && bank() => Some(Why::Bank),
            None if ACTS.contains(&act.action) => Some(Why::Act),
            None => None,
        }
    }

    /// Whether the user answered Never for `agent` on `site`, an answer Settings lists and revokes.
    pub(super) fn said_never(&self, agent: &str, site: &str) -> bool {
        let answers = self.grants.get(&self.answers_of(agent));
        answers.and_then(|sites| sites.get(site)) == Some(&false)
    }

    /// Whether the task's Sites leave out where `act` goes or what it's on: an open's address, or
    /// its tab's page. A download comes from wherever the site keeps its files, so it isn't asked.
    pub(super) fn off_sites(&self, act: &Act, cx: &gpui::App) -> bool {
        let Some(task) = self.acting_task(&act.agent, act.connection, act.call) else {
            return false;
        };
        if task.sites == Sites::Any || act.download != 0 {
            return false;
        }
        let url = match act.url.is_empty() {
            true => self.browser.tab(act.tab, cx).url.to_string(),
            false => act.url.clone(),
        };
        !task.sites.allows(self.browser.url_site(&url).as_deref())
    }

    /// Refuses `act` on `site` for `why` at `tier`: the agent hears why at once, the log keeps it,
    /// and the task's conversation shows it, counted on the same refusal if it's unsettled.
    pub(super) fn stopped(
        &mut self,
        act: Act,
        site: String,
        why: Why,
        tier: Tier,
        cx: &mut Context<Self>,
    ) {
        // A tab the UI doesn't list in the agent's Space is the core's to refuse, in its words,
        // so the agent learns nothing of a tab not its own.
        let space = self.browser.agent_space(&act.agent);
        if act.tab != 0
            && !self
                .tabs_seen(space, cx)
                .iter()
                .any(|tab| tab.id == act.tab)
        {
            return self.run(act, cx);
        }
        let at = match act.asks.is_empty() {
            true => tier::plain(&site),
            false => tier::plain(&act.asks),
        };
        let key = (self.acting_task(&act.agent, act.connection, act.call)).map(|t| t.agent.clone());
        if let Some(task) = key.as_deref().and_then(|key| self.task_mut(key)) {
            let after = task.messages.len();
            match (task.stops.iter_mut()).find(|stop| stop.same(act.tab, &at, why, tier)) {
                Some(stop) => stop.count += 1,
                None => task.stops.push(Stop {
                    tab: act.tab,
                    at: at.clone(),
                    site: tier::plain(&site),
                    why,
                    tier,
                    count: 1,
                    appeared: std::time::Instant::now(),
                    after,
                    settled: false,
                }),
            }
        }
        // check.sh reads this line.
        log!(
            "browser: agent {} stopped {why:?} at {}: {at}",
            act.agent,
            tier.label()
        );
        // A fenced site is named by the entry of the user's list it's under, as they wrote it.
        let named = match why {
            Why::Fenced => self.fence_of(&site).unwrap_or_else(|| site.clone()),
            _ => site.clone(),
        };
        let refused = Refused {
            told: Some(tier::told(why, tier, &act.what, &named)),
            ..Refused::of(tools::Kind::Outside, tier::why_line(why, tier, &named))
        };
        self.refuse(act, &site, refused, cx);
    }

    /// A call on `tab`, whose page is outside the task's Sites as the tab came there by itself, is
    /// refused as an open there would be; any other goes on.
    pub(super) fn sites_call(
        &mut self,
        call: Call,
        tab: u64,
        cx: &mut Context<Self>,
    ) -> Option<Call> {
        let url = self.browser.tab(tab, cx).url.to_string();
        let Some(task) = self.acting_task(&call.agent, call.connection, call.id) else {
            return Some(call);
        };
        if task.sites.allows(self.browser.url_site(&url).as_deref()) {
            return Some(call);
        }
        let site = site(&url);
        let mut act = act_of(
            &call.agent,
            call.connection,
            call.id,
            tab,
            call.reply.clone(),
        );
        act.action = fence::CAME_ACTION;
        act.what = fence::arrival(&site);
        self.check(act, site, Some(OTHER_SITE), cx);
        None
    }

    /// The user changes the task `key`'s access to `tier`: from its line or its chip, never from
    /// an agent. Its refusals are settled, the log keeps the change, and a task ilo runs is told.
    pub fn set_tier(&mut self, key: &str, tier: Tier, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(key).filter(|task| task.ended.is_none()) else {
            return;
        };
        let from = task.tier;
        if from == tier {
            return;
        }
        task.tier = tier;
        for stop in &mut task.stops {
            stop.settled = true;
        }
        let (agent, connection) = match &task.session {
            Some(session) => (session.agent.clone(), session.connection),
            None => (task.agent.clone(), 0),
        };
        let runs = task.cli.is_some();
        // check.sh reads this line.
        log!(
            "browser: task {key} access {} to {}",
            from.label(),
            tier.label()
        );
        let changed = tier::changed_logged(from, tier);
        self.note_on(&agent, connection, ACCESS_ACTION, "", &changed, "");
        if runs {
            self.tell_task(
                key,
                tier::changed_turn(from, tier),
                tier as usize > from as usize,
                cx,
            );
        }
        cx.notify();
    }

    /// The user sets the sites the task `key` may use.
    pub fn set_sites(&mut self, key: &str, sites: Sites, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(key).filter(|task| task.ended.is_none()) else {
            return;
        };
        if task.sites == sites {
            return;
        }
        task.sites = sites;
        for stop in (task.stops.iter_mut()).filter(|stop| stop.why == Why::OtherSite) {
            stop.settled = true;
        }
        cx.notify();
    }

    /// The task `key`'s refusals the user hasn't settled, oldest first.
    pub fn stops(&self, key: &str) -> Vec<Stop> {
        (self.task(key).into_iter())
            .flat_map(|task| task.stops.iter().filter(|stop| !stop.settled).cloned())
            .collect()
    }
}
