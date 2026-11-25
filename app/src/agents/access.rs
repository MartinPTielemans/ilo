//! How an act meets its task's access (tier.rs): the one decision point, `check`, runs what the
//! tier allows and refuses the rest at once, telling the agent and putting ilo's line in the task's
//! conversation. Nothing waits for the user, and only the user changes a tier or a task's Sites.

use gpui::Context;

use super::task::rewall::Change;
use super::tier::{self, Sites, Stop, Tier, Why};
use super::{
    Act, Agents, CHANGES_ASK, Call, DOWNLOADS, FENCED, OTHER_SITE, Refused, UNSAVED, act_of, fence,
    page, site, tools,
};

/// What the log names a change of a task's access, which its entry's target says.
pub const ACCESS_ACTION: &str = "access";

/// The actions that act on a page, which Look only refuses, as `page_tool` names them.
pub(super) const ACTS: [&str; 4] = ["click", "type", "select_option", "press_key"];

impl Agents {
    /// The access the task `agent`'s act on `connection`, for its call `call` (0 for none), is of:
    /// a ⌘T task's, else its session's, else the one a new session starts with.
    pub(super) fn tier_of(&self, agent: &str, connection: u64, call: u64) -> Tier {
        self.acting_task(agent, connection, call)
            .map_or(Tier::default(), |task| task.tier)
    }

    /// The task `act` is held to, by its access and its Sites: the one under way that holds its
    /// page, or the page that started its download, whichever of its client's connections calls,
    /// else the act's own, as an open's is. A second connection would otherwise pass the access
    /// the user set on a task, and the refusal's line would change another task's.
    pub(super) fn judged_task(&self, act: &Act) -> Option<&super::task::Task> {
        (self.page_task(&act.agent, act.tab.max(act.started_in)))
            .or_else(|| self.acting_task(&act.agent, act.connection, act.call))
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
        let Some(task) = self.judged_task(act) else {
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
        // A tab can leave the agent's Space, close or turn off limits while its page answers, as
        // before a discard: the core refuses it then, in its own words, so the agent learns
        // nothing of a tab not its own and nothing runs. One the core still calls its own but the
        // UI doesn't list, as a popup may be, sends a request's reads and refuses its changes in
        // the core's words; any other act there is refused as anywhere.
        let space = self.browser.agent_space(&act.agent);
        if act.tab != 0
            && !self
                .tabs_seen(space, cx)
                .iter()
                .any(|tab| tab.id == act.tab)
        {
            match self.may_act(&act.agent, act.tab, act.action, cx) {
                Err(refused) => return self.refuse(act, &site, refused, cx),
                Ok(()) if act.action == "request" => return self.send(act, false, cx),
                Ok(()) => {}
            }
        }
        let at = match act.asks.is_empty() {
            true => tier::plain(&site),
            false => tier::plain(&act.asks),
        };
        // The refused call came as a step of its own task before it was judged, so the line names
        // it as it is under way.
        let doing = (self.acting_task(&act.agent, act.connection, act.call))
            .and_then(|task| task.trace.iter().rfind(|step| step.call == act.call))
            .map(|step| lowered(&tools::step_title(&step.did, tools::Tense::Now)))
            .unwrap_or_default();
        let key = self.judged_task(&act).map(|task| task.agent.clone());
        if let Some(task) = key.as_deref().and_then(|key| self.task_mut(key)) {
            let (after, traced) = (task.messages.len(), task.trace.len());
            match (task.stops.iter_mut()).find(|stop| stop.same(act.tab, &at, why, tier)) {
                Some(stop) => stop.count += 1,
                None => task.stops.push(Stop {
                    tab: act.tab,
                    at: at.clone(),
                    site: tier::plain(&site),
                    doing,
                    why,
                    tier,
                    count: 1,
                    appeared: std::time::Instant::now(),
                    after,
                    traced,
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
        let Some(task) = (self.page_task(&call.agent, tab))
            .or_else(|| self.acting_task(&call.agent, call.connection, call.id))
        else {
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
    /// One with files goes up only if its wall of the new level does, and starts again inside
    /// the wall of whichever level it's set to (rewall.rs). On a task that has ended nothing
    /// starts: the level is the one its next reply starts at, walled then (`pick_up`).
    pub fn set_tier(&mut self, key: &str, tier: Tier, cx: &mut Context<Self>) {
        let Some(task) = self.task(key) else {
            return;
        };
        let (from, ended) = (task.tier, task.ended.is_some());
        if from == tier {
            return;
        }
        let raised = match ended {
            true => None,
            false => match self.wall_to_raise(key, from, tier) {
                Ok(raised) => raised,
                Err(()) => return cx.notify(),
            },
        };
        let Some(task) = self.task_mut(key) else {
            return;
        };
        task.tier = tier;
        if ended {
            task.picked_from.get_or_insert(from);
        }
        for stop in &mut task.stops {
            stop.settled = true;
        }
        let (agent, connection) = match &task.session {
            Some(session) => (session.agent.clone(), session.connection),
            None => (task.agent.clone(), 0),
        };
        let runs = task.cli.is_some();
        let made = super::lines::Happened::TierChanged { from, to: tier };
        self.place_in(key, made);
        // check.sh reads this line.
        log!(
            "browser: task {key} access {} to {}",
            from.label(),
            tier.label()
        );
        let changed = tier::changed_logged(from, tier);
        self.note_on(&agent, connection, ACCESS_ACTION, "", &changed, "");
        if runs && !ended {
            let change = Change::Level { from, raised };
            self.rewall(key, change, tier::changed_turn(from, tier), cx);
        }
        cx.notify();
    }

    /// The user sets the sites the task `key` may use: on one that has ended, those its next reply
    /// works on. The log keeps the change as it keeps a level's, by the menu's names and a count.
    pub fn set_sites(&mut self, key: &str, sites: Sites, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        if task.sites == sites {
            return;
        }
        let changed = tier::sites_logged(&task.sites, &sites);
        task.sites = sites;
        for stop in (task.stops.iter_mut()).filter(|stop| stop.why == Why::OtherSite) {
            stop.settled = true;
        }
        let (agent, connection) = match &task.session {
            Some(session) => (session.agent.clone(), session.connection),
            None => (task.agent.clone(), 0),
        };
        self.note_on(&agent, connection, ACCESS_ACTION, "", &changed, "");
        cx.notify();
    }

    /// Whether a refusal of the task `key`'s appeared too lately for a press on its line to be
    /// meant for it.
    pub(super) fn just_stopped(&self, key: &str) -> bool {
        (self.task(key).into_iter().flat_map(|task| &task.stops))
            .any(|stop| !stop.settled && stop.appeared.elapsed() < tier::PROTECT)
    }

    /// The user takes over `tab` from the task `key`'s line, as Take Over does: its refusals there
    /// are settled and its access stays as it was.
    pub(super) fn take_over_stopped(&mut self, key: &str, tab: u64, cx: &mut Context<Self>) {
        let Some(task) = self.task_mut(key) else {
            return;
        };
        for stop in (task.stops.iter_mut()).filter(|stop| stop.tab == tab) {
            stop.settled = true;
        }
        let agent = match &task.session {
            Some(session) => session.agent.clone(),
            None => task.agent.clone(),
        };
        if tab != 0 {
            self.take_over(&agent, tab, cx);
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

/// `text` with its first letter lowered, as a step's phrase reads after "from".
fn lowered(text: &str) -> String {
    let mut chars = text.chars();
    (chars.next())
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}
