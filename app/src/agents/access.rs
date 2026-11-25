//! How an act meets its task's access (tier.rs): the one decision point, `check`, runs what the
//! tier allows and refuses the rest at once, telling the agent and putting ilo's line in the task's
//! conversation. Nothing waits for the user, and only the user raises a tier: ilo lowers an outside
//! client's task only to what the user left another of its at (`starting_tier`, `left_at`).

use gpui::Context;

use super::task::rewall::Change;
use super::tier::{self, Stop, Tier, Why};
use super::{Act, Agents, CHANGES_ASK, DOWNLOADS, FENCED, Refused, UNSAVED, page, tools};

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

    /// The task `act` is held to, by its access: the one under way that holds its
    /// page, or the page that started its download, whichever of its client's connections calls,
    /// else the act's own, as an open's is. A second connection would otherwise pass the access
    /// the user set on a task, and the refusal's line would change another task's.
    pub(super) fn judged_task(&self, act: &Act) -> Option<&super::task::Task> {
        (self.page_task(&act.agent, act.page()))
            .or_else(|| self.acting_task(&act.agent, act.connection, act.call))
    }

    /// The access the log's entry names for what `agent` did on `connection` to the page in
    /// `tab`, or with 0 to none: the one its act was judged at (`judged_task`), so that of the
    /// task under way that holds the page, else the connection's own. An entry never names a
    /// level the act wasn't held to.
    pub(super) fn entry_tier(&self, agent: &str, connection: u64, tab: u64) -> Tier {
        let own = || self.tier_of(agent, connection, 0);
        (self.page_task(agent, tab)).map_or_else(own, |task| task.tier)
    }

    /// Why `act` on `site` is one a tier may refuse, from what would have asked (`why`), or None
    /// for one that only opens or reads.
    pub(super) fn why_of(act: &Act, site: &str, why: Option<&'static str>) -> Option<Why> {
        let bank = || page::opening_risk(site).is_some();
        match why {
            Some(FENCED) => Some(Why::Fenced),
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
        self.picked_past(key);
        let Some(task) = self.task_mut(key) else {
            return;
        };
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
