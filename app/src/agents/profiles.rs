//! The user's profiles an agent works in (#4376). An outside app's tab opens in the profile it
//! names, else the one whose history last visited the site, else the user's window's; a thread's in
//! the one it names, else its own Space's. Each app, and threads together, may use only the
//! profiles the user leaves checked in Settings, which no tool reads or writes. Why a tab opened
//! where it did is the user's alone: its card and the log say it, an agent's answer never does.
//! With one profile, nothing names one anywhere.
//!
//! The level, Off limits and the sensitive sites are the same in every profile: nothing here
//! touches them.

use std::collections::{BTreeSet, HashMap, HashSet};

use gpui::{App, Context};
use serde_json::{Value, json};

use super::{Act, Agents, Refused, is_task, name};
use crate::product::PRODUCT;

/// A profile of the user's, as the core names it: its number this run, its folder, which names it
/// from one run to the next, and the name the user gave it.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub id: u64,
    pub folder: String,
    pub name: String,
}

/// The key of the threads' list, as an app's is its id.
pub const THREADS: &str = "threads";

/// The title over an app's checkmarks (PR1), and over the threads' (PR7).
pub const MAY_USE: &str = "Profiles it may use";
pub const THREADS_MAY_USE: &str = "Profiles threads may use";

/// Why a tab opened in its profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The profile's history visited the site last (the core's pick).
    History,
    /// The user's window's (a thread's: its own Space's).
    Window,
    /// The window's profile is off for the agent, so the first it may use (PR3b).
    First,
    /// The agent named it.
    Asked,
}

/// Where an agent's tab opened and why, in words: its card's and the log's, never an answer's.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenedIn {
    pub profile: String,
    pub why: String,
}

impl OpenedIn {
    /// The line under the step that opened the tab (PR5).
    pub fn card_line(&self) -> String {
        format!("In {}, {}", self.profile, self.why)
    }

    /// The log's entry of the open (PR3): `app`'s, or with None a thread's, which the log's rows
    /// are already.
    pub fn log_line(&self, app: Option<&str>, site: &str) -> String {
        match app {
            Some(app) => format!("{app} opened {site} in {} · {}", self.profile, self.why),
            None => format!("Opened {site} in {} · {}", self.profile, self.why),
        }
    }
}

/// The hint beside an app's last checkmark still on (PR1a), `app` None for an unnamed one.
pub fn one_stays_on(app: Option<&str>) -> String {
    match app {
        Some(app) => format!("One stays on. To stop {app}, disconnect it."),
        None => "One stays on. To stop this app, disconnect it.".into(),
    }
}

/// The hint beside the threads' last checkmark still on (PR7).
pub fn threads_one_stays_on() -> String {
    format!("One stays on. To stop threads, turn off “Let your assistant use {PRODUCT}”.")
}

/// The answer to an open in a profile the agent may not use, the same whether or not one of that
/// name exists (PR2), which the agent passes on to the user.
pub fn cant_use(thread: bool, profile: &str) -> String {
    match thread {
        true => format!(
            "This task can't use “{profile}”. The user chooses the profiles tasks may use in \
             Settings › Threads."
        ),
        false => format!(
            "This app can't use “{profile}”. The user chooses the profiles it may use in \
             Settings › Other apps."
        ),
    }
}

/// What open_tab's profile argument says it is.
pub const PROFILE_ARGUMENT: &str = "profile is a profile's name, from list_profiles.";

/// The profile the next open goes to, and why, between its choice and its open.
#[derive(Clone, Debug)]
struct Chosen {
    profile: Profile,
    why: Why,
}

/// The agents' profiles, as the Agents keep them.
#[derive(Default)]
pub(super) struct Profiles {
    /// The profiles, by folder, each app by its id and threads (`THREADS`) may not use: a new
    /// profile starts allowed.
    off: HashMap<String, BTreeSet<String>>,
    /// Each open_tab's profile, by its agent and call, from its choice to its open.
    chosen: HashMap<(String, u64), Chosen>,
    /// The opens waiting on the core's pick, by request, with their site.
    picking: HashMap<u64, (Act, String)>,
    /// The opens waiting on their profile to load, which the next change of the Spaces tries again.
    loading: Vec<Act>,
    /// The agents' Spaces made in a profile that may not have loaded, until a tab opens in one.
    unopened: HashSet<u64>,
    /// Where each tab an agent opened went and why, by tab, while the user has several profiles.
    opened: HashMap<u64, OpenedIn>,
}

impl Profiles {
    /// As agents.json keeps them: each list's profiles switched off.
    pub(super) fn saved(&self) -> Value {
        let off: serde_json::Map<String, Value> = (self.off.iter())
            .filter(|(_, off)| !off.is_empty())
            .map(|(key, off)| (key.clone(), json!(off)))
            .collect();
        Value::Object(off)
    }

    pub(super) fn load(&mut self, saved: Option<&Value>) {
        let saved = saved.and_then(Value::as_object).into_iter().flatten();
        self.off = (saved.filter_map(|(key, off)| {
            let off = off.as_array()?.iter().filter_map(Value::as_str);
            Some((key.clone(), off.map(str::to_owned).collect()))
        }))
        .collect();
    }
}

/// The key of `agent`'s list: an app's id, or `THREADS` for any thread's.
fn list_key(agent: &str) -> &str {
    match is_task(agent) {
        true => THREADS,
        false => agent,
    }
}

impl Agents {
    /// Whether the user has more than one profile, without which nothing names one.
    pub fn several_profiles(&self) -> bool {
        self.browser.profiles().len() > 1
    }

    /// Each of the user's profiles, and whether `key`'s list has it on: an app's by its id, or
    /// threads' by `THREADS`.
    pub fn profile_choices(&self, key: &str) -> Vec<(Profile, bool)> {
        let allowed = self.allowed_profiles(key);
        (self.browser.profiles().into_iter())
            .map(|profile| {
                let on = allowed.contains(&profile);
                (profile, on)
            })
            .collect()
    }

    /// The profiles `key`'s list has on, in the core's order. Never none: with every one it had
    /// on gone, the first is.
    pub fn allowed_profiles(&self, key: &str) -> Vec<Profile> {
        let profiles = self.browser.profiles();
        let off = self.profiles.off.get(key);
        let allowed: Vec<Profile> = (profiles.iter())
            .filter(|profile| off.is_none_or(|off| !off.contains(&profile.folder)))
            .cloned()
            .collect();
        match allowed.is_empty() {
            true => profiles.into_iter().take(1).collect(),
            false => allowed,
        }
    }

    /// The user checks or clears the profile in `folder` for `key`'s list. The last one on stays
    /// on. Cleared, the list's agents' tabs there go to the user's Space in it and their Spaces
    /// there go.
    pub fn set_profile_allowed(
        &mut self,
        key: &str,
        folder: &str,
        on: bool,
        cx: &mut Context<Self>,
    ) {
        let allowed = self.allowed_profiles(key);
        let has = allowed.iter().any(|profile| profile.folder == folder);
        if on == has || (!on && allowed.len() <= 1) {
            return;
        }
        let off = self.profiles.off.entry(key.to_string()).or_default();
        match on {
            true => off.remove(folder),
            false => off.insert(folder.to_string()),
        };
        if !on {
            self.hand_back_profile(key, folder, cx);
        }
        self.save();
        cx.notify();
    }

    /// `key`'s agents' tabs in the profile in `folder` go to the user's Space there, as Take Back
    /// All gives the user an agent's tabs, and each agent's Space there goes, but a thread's own.
    fn hand_back_profile(&mut self, key: &str, folder: &str, cx: &mut Context<Self>) {
        let agents: Vec<String> = match key {
            THREADS => (self.tasks.tasks.iter())
                .filter(|task| is_task(&task.agent))
                .map(|task| task.agent.clone())
                .collect(),
            agent => vec![agent.to_string()],
        };
        let user = self.browser.profile_space(folder);
        for agent in agents {
            let space = self.browser.agent_space_in(&agent, folder);
            if space == 0 {
                continue;
            }
            if user != 0 {
                for tab in self.browser.space_tab_ids(space, cx) {
                    self.browser.move_tab(tab, user, cx);
                }
            }
            if self.task(&agent).is_none_or(|task| task.space != space) {
                self.browser.delete_space(space, cx);
                self.profiles.unopened.remove(&space);
            }
        }
    }

    /// list_profiles' answer: the names of the profiles the agent may use, one a line.
    pub(super) fn profile_names(&self, agent: &str) -> String {
        let names: Vec<String> = (self.allowed_profiles(list_key(agent)).into_iter())
            .map(|profile| profile.name)
            .collect();
        names.join("\n")
    }

    /// The profile the tab is in, by name, for list_tabs, while the user has several.
    pub(super) fn tab_profile(&self, tab: u64, profiles: &[Profile], cx: &App) -> Option<String> {
        if profiles.len() < 2 {
            return None;
        }
        let folder = self.browser.space_profile(self.browser.space_of(tab, cx));
        (profiles.iter())
            .find(|profile| profile.folder == folder)
            .map(|profile| profile.name.clone())
    }

    /// Chooses the profile `act`, an open_tab at `host`, opens in, then checks it as before: the
    /// one the agent named, if it may use it; else a thread's own Space's; else for an app the
    /// core's pick by history, which answers later (`profile_picked`).
    pub(super) fn open_in_profile(
        &mut self,
        act: Act,
        host: String,
        arguments: &Value,
        cx: &mut Context<Self>,
    ) {
        let thread = is_task(&act.agent);
        let allowed = self.allowed_profiles(list_key(&act.agent));
        match arguments.get("profile") {
            None | Some(Value::Null) => {}
            Some(Value::String(named)) => {
                return match allowed.iter().find(|profile| profile.name == *named) {
                    Some(profile) => self.chosen(act, host, profile.clone(), Why::Asked, cx),
                    None => {
                        let refused = Refused::new(cant_use(thread, named));
                        self.refuse(act, &host, refused, cx)
                    }
                };
            }
            Some(_) => return self.refuse(act, &host, Refused::new(PROFILE_ARGUMENT), cx),
        }
        if thread {
            let own = self.task(&act.agent).map(|task| match task.space {
                0 => task.home,
                space => space,
            });
            let folder = own.map(|space| self.browser.space_profile(space));
            // A profile the core doesn't list is none to choose: the thread's Space is made like
            // its own, as before.
            let listed = self.browser.profiles();
            if !listed
                .iter()
                .any(|profile| Some(&profile.folder) == folder.as_ref())
            {
                let mut act = act;
                let risk = self.opening(&mut act, &host);
                return self.check(act, host, risk, cx);
            }
            let (profile, why) =
                match (allowed.iter()).find(|profile| Some(&profile.folder) == folder.as_ref()) {
                    Some(own) => (own.clone(), Why::Window),
                    None => (allowed[0].clone(), Why::First),
                };
            return self.chosen(act, host, profile, why, cx);
        }
        if !self.several_profiles() {
            let profile = allowed[0].clone();
            return self.chosen(act, host, profile, Why::Window, cx);
        }
        // An Off limits site is refused whatever the profile, so no profile's history is read for it.
        if self.fence_of(&host).is_some() {
            let (profile, why) = self.window_profile(&allowed);
            return self.chosen(act, host, profile, why, cx);
        }
        let request = self.next_request;
        self.next_request += 1;
        let ids: Vec<u64> = allowed.iter().map(|profile| profile.id).collect();
        let url = act.url.clone();
        self.profiles.picking.insert(request, (act, host));
        self.browser.pick_profile(&ids, &url, request);
    }

    /// The core's pick for the open waiting on `request`: `profile`, whose history visited the
    /// site last, or 0 for none. With none, or one the agent may no longer use, it's the user's
    /// window's profile, else the first the agent may use.
    pub fn profile_picked(&mut self, request: u64, profile: u64, cx: &mut Context<Self>) {
        let Some((act, host)) = self.profiles.picking.remove(&request) else {
            return;
        };
        let allowed = self.allowed_profiles(list_key(&act.agent));
        let picked = allowed
            .iter()
            .find(|each| profile != 0 && each.id == profile);
        let (profile, why) = match picked {
            Some(picked) => (picked.clone(), Why::History),
            None => self.window_profile(&allowed),
        };
        self.chosen(act, host, profile, why, cx)
    }

    /// The user's window's profile, unless it's private or one the agent can't use: then the first
    /// it may use, and the log says so.
    fn window_profile(&self, allowed: &[Profile]) -> (Profile, Why) {
        let front = self.browser.front_space();
        let folder =
            (!self.browser.private_space(front)).then(|| self.browser.space_profile(front));
        match (allowed.iter()).find(|each| Some(&each.folder) == folder.as_ref()) {
            Some(window) => (window.clone(), Why::Window),
            None => (allowed[0].clone(), Why::First),
        }
    }

    /// The open goes to `profile` for `why`, once it's checked as any open is.
    fn chosen(
        &mut self,
        mut act: Act,
        host: String,
        profile: Profile,
        why: Why,
        cx: &mut Context<Self>,
    ) {
        let key = (act.agent.clone(), act.call);
        self.profiles.chosen.insert(key, Chosen { profile, why });
        let risk = self.opening(&mut act, &host);
        self.check(act, host, risk, cx);
    }

    /// Forgets the profile chosen for `act`, which won't open.
    pub(super) fn unchoose(&mut self, act: &Act) {
        self.profiles.chosen.remove(&(act.agent.clone(), act.call));
    }

    /// Opens `act`'s tab in the profile chosen for it, making the agent's Space there first: the
    /// tab, 0 if it couldn't open, or None while the Space's profile is still loading.
    pub(super) fn open_chosen(&mut self, act: &Act, cx: &mut Context<Self>) -> Option<u64> {
        let key = (act.agent.clone(), act.call);
        let Some(chosen) = self.profiles.chosen.get(&key).cloned() else {
            self.space(&act.agent, cx);
            return Some(self.browser.open_tab(&act.agent, 0, &act.url, cx));
        };
        let space = self.space_in(&act.agent, &chosen.profile, cx);
        let tab = self
            .browser
            .open_tab(&act.agent, chosen.profile.id, &act.url, cx);
        if tab == 0 && self.profiles.unopened.contains(&space) {
            return None;
        }
        self.profiles.unopened.remove(&space);
        self.profiles.chosen.remove(&key);
        if tab != 0 && self.several_profiles() {
            let why = self.why_words(&act.agent, chosen.why);
            let opened = OpenedIn {
                profile: chosen.profile.name,
                why,
            };
            self.trace_opened_in(&act.agent, act.call, &opened);
            self.profiles.opened.insert(tab, opened);
        }
        Some(tab)
    }

    /// The step of `agent`'s call that opened a tab says under it where and why (PR5).
    fn trace_opened_in(&mut self, agent: &str, call: u64, opened: &OpenedIn) {
        let line = opened.card_line();
        for task in self.tasks.tasks.iter_mut().rev() {
            let theirs = task.agent == agent
                || (task.session.as_ref()).is_some_and(|session| session.agent == agent);
            let step = (task.trace.iter_mut().rev()).find(|step| {
                step.call == call && step.did.tool == "open_tab" && step.opened_in.is_none()
            });
            if let Some(step) = step.filter(|_| theirs) {
                step.opened_in = Some(line.into());
                return;
            }
        }
    }

    /// The entry just logged, an open, says where its tab opened and why (PR3).
    pub(super) fn logged_in(&mut self, opened: Option<OpenedIn>) {
        let Some(opened) = opened else {
            return;
        };
        let id = self.logged;
        for entry in self.log.iter_mut().rev() {
            let found = match entry.id == id {
                true => Some(entry),
                false => entry.steps.iter_mut().find(|step| step.id == id),
            };
            if let Some(found) = found {
                found.opened_in = Some(opened);
                return;
            }
        }
    }

    /// The why, in the user's words (PR3a).
    fn why_words(&self, agent: &str, why: Why) -> String {
        match why {
            Why::History => "where you last used it".into(),
            Why::Window => "your current window's profile".into(),
            Why::First => "the first allowed profile".into(),
            Why::Asked if is_task(agent) => "as the thread asked".into(),
            Why::Asked if self.nameless(agent) => "as the app asked".into(),
            Why::Asked => format!("as {} asked", self.agent_name(agent)),
        }
    }

    /// The agent's Space in `profile`, made if it has none there: a thread's first in its own
    /// profile as `space` makes it.
    fn space_in(&mut self, agent: &str, profile: &Profile, cx: &mut Context<Self>) -> u64 {
        let space = self.browser.agent_space_in(agent, &profile.folder);
        if space != 0 {
            return space;
        }
        let first = self.browser.agent_spaces(agent).is_empty();
        let home = self.task(agent).map(|task| (task.name.clone(), task.home));
        let made = match home {
            Some((_, home)) if first && self.browser.space_profile(home) == profile.folder => {
                self.space(agent, cx)
            }
            Some((named, _)) => {
                let made = self.browser.make_agent_space(agent, &named, profile.id, cx);
                if first {
                    self.task_space_made(agent, made);
                }
                made
            }
            None => {
                let named = self.space_name(agent, profile);
                let made = self.browser.make_agent_space(agent, &named, profile.id, cx);
                if first {
                    self.client_space_made(agent, made);
                }
                made
            }
        };
        // The profile the core started with is loaded; another may not be yet.
        let started = self.browser.profiles().first().map(|first| first.id);
        if made != 0 && started != Some(profile.id) {
            self.profiles.unopened.insert(made);
        }
        made
    }

    /// An app's Space's name: `{app} · {profile}` while the user has several profiles (PR6).
    pub(super) fn space_name(&self, agent: &str, profile: &Profile) -> String {
        match self.several_profiles() {
            true => format!("{} · {}", name(agent), profile.name),
            false => name(agent).to_string(),
        }
    }

    /// `act` waits for its profile to load: the core sends a change of the Spaces once it has.
    pub(super) fn park_open(&mut self, act: Act) {
        self.profiles.loading.push(act);
    }

    /// The Spaces changed, as when a profile loaded: the opens waiting on one try again.
    pub fn profiles_loaded(&mut self, cx: &mut Context<Self>) {
        for act in std::mem::take(&mut self.profiles.loading) {
            self.run(act, cx);
        }
    }

    /// Forgets what of the agent's opens waits on a pick or a load, as its session went.
    pub(super) fn forget_opens(&mut self, gone: impl Fn(&str, u64) -> bool) {
        (self.profiles.picking).retain(|_, (act, _)| !gone(&act.agent, act.connection));
        (self.profiles.loading).retain(|act| !gone(&act.agent, act.connection));
    }

    /// Where `tab` opened and why, while the user has several profiles.
    pub fn opened_in(&self, tab: u64) -> Option<&OpenedIn> {
        self.profiles.opened.get(&tab)
    }

    /// Forgets a closed tab's profile.
    pub(super) fn profile_tab_closed(&mut self, tab: u64) {
        self.profiles.opened.remove(&tab);
    }
}
