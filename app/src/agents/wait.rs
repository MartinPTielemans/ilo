//! What an agent waits for with wait_for_event (#3668): an event in its own tabs or task, or its seconds
//! passing. Each event already reaches the Agents, from the core or from their own state, so a
//! wait polls nothing, and with no wait there's no timer. First what needs no GPUI, the call's
//! arguments and the answer's words, then the Agents' waits.

use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::Duration;

use gpui::{Context, SharedString, Task};
use serde_json::Value;

use super::{Agents, NOT_ITS_SPACE, NOT_WEB_PAGE, ToolResult, held, off_limits, site, tools};

/// An event a wait_for_event call names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Awaited {
    Loaded,
    Navigated,
    Closed,
    Download,
    Taken,
    HandedBack,
}

/// Each event by the name the tool takes, in the schema's order.
pub const EVENTS: [(&str, Awaited); 6] = [
    ("loaded", Awaited::Loaded),
    ("navigated", Awaited::Navigated),
    ("closed", Awaited::Closed),
    ("download", Awaited::Download),
    ("taken", Awaited::Taken),
    ("handed_back", Awaited::HandedBack),
];

impl Awaited {
    pub fn name(self) -> &'static str {
        let named = EVENTS.iter().find(|(_, each)| *each == self);
        named.map_or("", |(name, _)| name)
    }
}

/// How long a wait lasts unless the call says, and at most: ASK_WAIT's five minutes, the longest
/// a call waits on the user apart from a handoff.
pub const SECONDS: u64 = 30;
pub const LONGEST: u64 = 300;

/// A call's events, each once in the order given, and its seconds; or why not.
pub fn parse(arguments: &Value) -> Result<(Vec<Awaited>, u64), String> {
    let names = EVENTS.map(|(name, _)| name).join(", ");
    let wrong = || format!("events is a list of one or more of {names}.");
    let given = (arguments.get("events").and_then(Value::as_array)).ok_or_else(wrong)?;
    let mut events = Vec::new();
    for name in given {
        let name = name.as_str().ok_or_else(wrong)?;
        let (_, event) = (EVENTS.iter().find(|(each, _)| *each == name)).ok_or_else(wrong)?;
        if !events.contains(event) {
            events.push(*event);
        }
    }
    if events.is_empty() {
        return Err(wrong());
    }
    let seconds = match arguments.get("seconds") {
        None | Some(Value::Null) => SECONDS,
        Some(seconds) => (seconds.as_f64())
            .filter(|seconds| (1.0..=LONGEST as f64).contains(seconds))
            .map(|seconds| seconds.round() as u64)
            .ok_or_else(|| format!("seconds is a number from 1 to {LONGEST}."))?,
    };
    Ok((events, seconds))
}

/// How a tab a wait was on stopped being the agent's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gone {
    Closed,
    /// It left the agent's Space for one of the user's.
    Left,
    /// The tab named isn't the agent's as the wait starts: it closed, or is in no Space of its
    /// own, which one answer says so a wait tells nothing of the user's tabs.
    NotYours,
}

/// What happened, which answers a wait for its event. A page's `url` is empty off the web.
#[derive(Clone, Debug, PartialEq)]
pub enum Happened {
    Loaded {
        tab: u64,
        url: String,
    },
    Navigated {
        tab: u64,
        url: String,
    },
    Closed {
        tab: u64,
        gone: Gone,
    },
    /// One of the agent's downloads ended, and no call of its said how: `line` does.
    Download {
        line: String,
    },
    /// The user holds the tab: they took it over, or the page asked for their passkey.
    Taken {
        tab: u64,
    },
    HandedBack {
        tab: u64,
    },
}

impl Happened {
    pub fn event(&self) -> Awaited {
        match self {
            Happened::Loaded { .. } => Awaited::Loaded,
            Happened::Navigated { .. } => Awaited::Navigated,
            Happened::Closed { .. } => Awaited::Closed,
            Happened::Download { .. } => Awaited::Download,
            Happened::Taken { .. } => Awaited::Taken,
            Happened::HandedBack { .. } => Awaited::HandedBack,
        }
    }

    /// The tab it happened on, or 0 for a download, which is the agent's wherever it began.
    pub fn tab(&self) -> u64 {
        match self {
            Happened::Loaded { tab, .. }
            | Happened::Navigated { tab, .. }
            | Happened::Closed { tab, .. }
            | Happened::Taken { tab }
            | Happened::HandedBack { tab } => *tab,
            Happened::Download { .. } => 0,
        }
    }

    /// The wait's answer, in one line.
    pub fn line(&self) -> String {
        match self {
            // A page off the web is named by its kind alone, as list_tabs names it.
            Happened::Loaded { tab, url } if url.is_empty() => {
                format!("Tab {tab} loaded a page that isn't http or https.")
            }
            Happened::Navigated { tab, url } if url.is_empty() => {
                format!("Tab {tab} went to a page that isn't http or https.")
            }
            Happened::Loaded { tab, url } => format!("Tab {tab} loaded {url}."),
            Happened::Navigated { tab, url } => format!("Tab {tab} went to {url}."),
            Happened::Closed { tab, gone } => match gone {
                Gone::Closed => format!("Tab {tab} closed."),
                Gone::Left => format!("Tab {tab} isn't yours any more."),
                Gone::NotYours => {
                    format!("Tab {tab} isn't one of yours: it closed, or isn't in your Space.")
                }
            },
            Happened::Download { line } => line.clone(),
            Happened::Taken { tab } => format!("The user took tab {tab} over."),
            Happened::HandedBack { tab } => format!("The user handed tab {tab} back."),
        }
    }
}

/// The answer once a wait's seconds pass.
pub fn nothing(seconds: u64) -> String {
    format!("Nothing happened in {seconds} s.")
}

/// What a wait waits for, as its step on the card says it: "maat.pt to load", "a download", "an
/// event". `on` is the named tab's site, or empty for any of the agent's tabs.
pub fn target(arguments: &Value, on: &str) -> String {
    let events = parse(arguments)
        .map(|(events, _)| events)
        .unwrap_or_default();
    let on = |none: &str| match on.is_empty() {
        true => none.to_string(),
        false => on.to_string(),
    };
    match events.as_slice() {
        [Awaited::Loaded] => format!("{} to load", on("a page")),
        [Awaited::Navigated] => format!("{} to move on", on("a page")),
        [Awaited::Closed] => format!("{} to close", on("a tab")),
        [Awaited::Download] => "a download".into(),
        [Awaited::Taken] => format!("you to take {}", on("a tab")),
        [Awaited::HandedBack] => format!("{} back", on("a tab")),
        _ => "an event".into(),
    }
}

/// A wait_for_event call, until its event or its seconds.
pub(super) struct Wait {
    agent: String,
    events: Vec<Awaited>,
    seconds: u64,
    /// The tabs it's on, each with the URL it was at when last seen: the one it names, else the
    /// agent's, those it gets while the wait lasts too.
    tabs: Vec<(u64, SharedString)>,
    /// Whether it names its tab, which no other tab joins.
    named: bool,
    reply: mpsc::Sender<ToolResult>,
    /// Answers once its seconds pass. Dropped with the wait, it never fires.
    _timer: Task<()>,
}

/// The most downloads ended after their call answered that an agent's waits keep to tell.
const UNTOLD: usize = 16;

thread_local! {
    /// Some call waits, which the core's reports of a tab's page reach the Agents for: with none,
    /// they go nowhere.
    static WAITING: Cell<bool> = const { Cell::new(false) };
}

/// Whether some call waits (wait_for_event).
pub fn waiting() -> bool {
    WAITING.get()
}

impl Agents {
    /// wait_for_event: answers at once if what a named tab waits for is so already, else once its
    /// event comes or its seconds pass.
    pub(super) fn wait_for_event(
        &mut self,
        agent: String,
        arguments: &Value,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let given =
            parse(arguments).and_then(|parsed| Ok((parsed, tools::tab_argument(arguments)?)));
        let ((events, seconds), tab) = match given {
            Ok(given) => given,
            Err(why) => {
                _ = reply.send(ToolResult::error(why));
                return;
            }
        };
        // A download no answer told of is told at once, with a tab or without: each is told once.
        if events.contains(&Awaited::Download)
            && let Some(line) = self.untold.get_mut(&agent).and_then(VecDeque::pop_front)
        {
            return self.tell_wait(&agent, Happened::Download { line }, reply, cx);
        }
        let tabs = match tab {
            Some(tab) if !self.yours(&agent, tab, cx) => {
                if events.contains(&Awaited::Closed) {
                    let gone = Gone::NotYours;
                    return self.tell_wait(&agent, Happened::Closed { tab, gone }, reply, cx);
                }
                self.note(&agent, "wait_for_event", "", NOT_ITS_SPACE);
                cx.notify();
                _ = reply.send(ToolResult::error(NOT_ITS_SPACE));
                return;
            }
            Some(tab) => {
                if let Some(now) = self.already(&agent, tab, &events, cx) {
                    return self.tell_wait(&agent, now, reply, cx);
                }
                vec![(tab, self.browser.tab(tab, cx).url)]
            }
            // Edges alone: a state such as loaded would hold on some tab at once.
            None => {
                let space = self.browser.agent_space(&agent);
                let tabs = self.browser.space_tabs(space, cx).into_iter();
                let tabs = tabs.filter(|tab| self.seen(tab.id, cx));
                tabs.map(|tab| (tab.id, tab.url)).collect()
            }
        };
        let id = self.next_request;
        self.next_request += 1;
        let timer = cx.spawn(async move |agents, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(seconds))
                .await;
            _ = agents.update(cx, |agents, cx| agents.wait_over(id, cx));
        });
        let wait = Wait {
            agent,
            events,
            seconds,
            tabs,
            named: tab.is_some(),
            reply,
            _timer: timer,
        };
        self.waits.insert(id, wait);
        WAITING.set(true);
    }

    /// Whether the agent may hear of `tab`: in its Space, and listed by list_tabs.
    fn yours(&self, agent: &str, tab: u64, cx: &gpui::App) -> bool {
        self.owns(agent, tab, cx) && self.seen(tab, cx)
    }

    /// Whether list_tabs lists `tab` in its agent's Space: a Board's never is.
    fn seen(&self, tab: u64, cx: &gpui::App) -> bool {
        matches!(self.browser.off_limits(tab, cx), None | Some(NOT_WEB_PAGE))
    }

    /// What of `events` is so of the agent's `tab` already, the first in the call's order.
    fn already(
        &self,
        agent: &str,
        tab: u64,
        events: &[Awaited],
        cx: &gpui::App,
    ) -> Option<Happened> {
        let url = shown(&self.browser.tab(tab, cx).url);
        let told = self.told.get(&(agent.to_string(), tab));
        let held = self.held.contains_key(&tab);
        events.iter().find_map(|event| match event {
            Awaited::Loaded => (!self.browser.loading(tab, cx)).then(|| Happened::Loaded {
                tab,
                url: url.clone(),
            }),
            Awaited::Navigated => {
                (told.is_some_and(|told| shown(told) != url)).then(|| Happened::Navigated {
                    tab,
                    url: url.clone(),
                })
            }
            Awaited::Taken => held.then_some(Happened::Taken { tab }),
            Awaited::HandedBack => (!held).then_some(Happened::HandedBack { tab }),
            Awaited::Closed | Awaited::Download => None,
        })
    }

    /// Answers a wait with what happened, which the activity log and check.sh's line say.
    fn tell_wait(
        &mut self,
        agent: &str,
        happened: Happened,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let (event, tab) = (happened.event().name(), happened.tab());
        // check.sh reads this line.
        log!("browser: agent {agent} waited {event} {tab}");
        // A tab not the agent's is named in the log by nothing of its page.
        let url = match happened {
            Happened::Download { .. } | Happened::Closed { .. } => SharedString::default(),
            _ => self.browser.tab(tab, cx).url,
        };
        if let Happened::Loaded { .. } | Happened::Navigated { .. } = happened {
            self.told.insert((agent.to_string(), tab), url.clone());
        }
        self.note_on(agent, "wait_for_event", &site(&url), event, "");
        cx.notify();
        // Never to land: a wait told of its event answers as if it heard nothing (#3668's red proof).
        _ = (happened.line(), reply.send(ToolResult::text(nothing(0))));
    }

    /// The wait `id`'s seconds passed with nothing it waits for.
    fn wait_over(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(wait) = self.waits.remove(&id) else {
            return;
        };
        WAITING.set(!self.waits.is_empty());
        // check.sh reads this line.
        log!("browser: agent {} waited nothing", wait.agent);
        self.note(&wait.agent, "wait_for_event", "", "");
        cx.notify();
        _ = wait.reply.send(ToolResult::text(nothing(wait.seconds)));
    }

    /// Answers each wait for what happened on one of its tabs, or of `agent`'s for a download:
    /// whether one heard it.
    fn hear(&mut self, happened: Happened, agent: &str, cx: &mut Context<Self>) -> bool {
        let (event, tab) = (happened.event(), happened.tab());
        let heard: Vec<u64> = (self.waits.iter())
            .filter(|(_, wait)| {
                wait.agent == agent
                    && wait.events.contains(&event)
                    && (tab == 0 || wait.tabs.iter().any(|(each, _)| *each == tab))
            })
            .map(|(&id, _)| id)
            .collect();
        for id in &heard {
            let wait = self.waits.remove(id).expect("it was just found");
            self.tell_wait(&wait.agent, happened.clone(), wait.reply, cx);
        }
        WAITING.set(!self.waits.is_empty());
        !heard.is_empty()
    }

    /// The agent whose Space has `tab`, while one of its calls waits, after its waits on all its
    /// tabs take a tab new to them, with its URL.
    fn waiting_owner(&mut self, tab: u64, cx: &mut Context<Self>) -> Option<String> {
        let space = self.browser.space_of(tab, cx);
        let agent = (self.waits.values())
            .map(|wait| &wait.agent)
            .find(|agent| space != 0 && self.browser.agent_space(agent) == space)?
            .clone();
        if self.seen(tab, cx) {
            let url = self.browser.tab(tab, cx).url;
            for wait in self.waits.values_mut() {
                let new = !wait.tabs.iter().any(|(each, _)| *each == tab);
                if wait.agent == agent && !wait.named && new {
                    wait.tabs.push((tab, url.clone()));
                }
            }
        }
        Some(agent)
    }

    /// `tab` loaded `url` (tabs::tab_loaded).
    pub fn wait_loaded(&mut self, tab: u64, url: &str, cx: &mut Context<Self>) {
        if let Some(agent) = self.waiting_owner(tab, cx) {
            let url = shown(url);
            self.hear(Happened::Loaded { tab, url }, &agent, cx);
        }
    }

    /// `tab`'s page is at `url` (tabs::tab_state_changed): to a wait that saw it elsewhere, it
    /// navigated.
    pub fn wait_page(&mut self, tab: u64, url: &str, cx: &mut Context<Self>) {
        let Some(agent) = self.waiting_owner(tab, cx) else {
            return;
        };
        let mut moved = Vec::new();
        for (&id, wait) in &mut self.waits {
            for (each, seen) in &mut wait.tabs {
                if *each == tab && seen.as_ref() != url {
                    *seen = url.to_string().into();
                    moved.extend(wait.events.contains(&Awaited::Navigated).then_some(id));
                }
            }
        }
        let happened = Happened::Navigated {
            tab,
            url: shown(url),
        };
        for id in moved {
            if let Some(wait) = self.waits.remove(&id) {
                self.tell_wait(&agent, happened.clone(), wait.reply, cx);
            }
        }
        WAITING.set(!self.waits.is_empty());
    }

    /// `tab` closed (tabs::tab_closed), or left `agent`'s Space: its waits hear of it, and no
    /// wait is on it any more.
    pub(super) fn wait_gone(&mut self, tab: u64, gone: Gone, agent: &str, cx: &mut Context<Self>) {
        self.hear(Happened::Closed { tab, gone }, agent, cx);
        for wait in self.waits.values_mut() {
            wait.tabs.retain(|(each, _)| *each != tab);
        }
        self.told.retain(|(_, each), _| *each != tab);
    }

    /// `tab` closed (tabs::tab_closed).
    pub(super) fn wait_closed(&mut self, tab: u64, cx: &mut Context<Self>) {
        let mut agents: Vec<String> = (self.waits.values())
            .filter(|wait| wait.tabs.iter().any(|(each, _)| *each == tab))
            .map(|wait| wait.agent.clone())
            .collect();
        agents.dedup();
        for agent in agents {
            self.wait_gone(tab, Gone::Closed, &agent, cx);
        }
        self.told.retain(|(_, each), _| *each != tab);
    }

    /// The tabs `space`'s waits are on that left it (spaces::space_changed): closed, or moved to
    /// one of the user's Spaces.
    pub(super) fn waits_left(&mut self, space: u64, cx: &mut Context<Self>) {
        if self.waits.is_empty() {
            return;
        }
        let tabs = self.browser.space_tab_ids(space, cx);
        let left: Vec<(u64, String)> = (self.waits.values())
            .filter(|wait| self.browser.agent_space(&wait.agent) == space)
            .flat_map(|wait| {
                let left = wait.tabs.iter().filter(|(tab, _)| !tabs.contains(tab));
                left.map(|(tab, _)| (*tab, wait.agent.clone()))
            })
            .collect();
        for (tab, agent) in left {
            // Closing posts space_changed before the tab is gone if its page runs unload
            // handlers, so where it is, not whether it's gone yet, says which.
            let gone = match self.browser.space_of(tab, cx) {
                0 => Gone::Closed,
                _ => Gone::Left,
            };
            self.wait_gone(tab, gone, &agent, cx);
        }
    }

    /// The user took `tab` from `agent`, or handed it back, as `held::carry` steps its hold.
    pub(super) fn wait_held(
        &mut self,
        tab: u64,
        agent: &str,
        event: held::Event,
        free: bool,
        cx: &mut Context<Self>,
    ) {
        let happened = match (event, free) {
            (held::Event::TakeOver | held::Event::Passkey, false) => Happened::Taken { tab },
            (held::Event::HandBack, true) => Happened::HandedBack { tab },
            _ => return,
        };
        if !self.waits.is_empty() {
            self.hear(happened, agent, cx);
        }
    }

    /// A download of `agent`'s ended after its call answered, or with none to answer: a wait
    /// tells it, now or later.
    pub(super) fn download_untold(&mut self, agent: &str, line: String, cx: &mut Context<Self>) {
        let happened = Happened::Download { line: line.clone() };
        if self.hear(happened, agent, cx) {
            return;
        }
        let untold = self.untold.entry(agent.to_string()).or_default();
        if untold.len() == UNTOLD {
            untold.pop_front();
        }
        untold.push_back(line);
    }

    /// The URL of the tab a call was on, as its answer left it, which a wait for navigated on the
    /// tab compares: the tab it named, else the one the agent used last.
    pub(super) fn told_on(&mut self, agent: &str, call: u64, cx: &gpui::App) {
        let named = self.on_tab.remove(&call);
        if let Some(tab) = named.or_else(|| self.current.get(agent).copied()) {
            let url = self.browser.tab(tab, cx).url;
            self.told.insert((agent.to_string(), tab), url);
        }
    }

    /// The agent went: its waits go unanswered, with what they'd tell.
    pub(super) fn forget_waits(&mut self, agent: &str) {
        self.waits.retain(|_, wait| wait.agent != agent);
        WAITING.set(!self.waits.is_empty());
        self.untold.remove(agent);
        self.told.retain(|(each, _), _| each != agent);
    }

    /// The user stopped the agent: its waits are answered so.
    pub(super) fn stop_waits(&mut self, agent: &str, stopped: &str) {
        for (_, wait) in self.waits.extract_if(|_, wait| wait.agent == agent) {
            _ = wait.reply.send(ToolResult::error(stopped));
        }
        WAITING.set(!self.waits.is_empty());
    }
}

/// A page's URL as a wait tells it: none off the web, as list_tabs gives none.
fn shown(url: &str) -> String {
    match off_limits(url, "") {
        None => url.to_string(),
        Some(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_wait_takes_each_event_once_and_seconds_up_to_five_minutes() {
        let (events, seconds) =
            parse(&json!({ "events": ["closed", "loaded", "closed"] })).unwrap();
        assert_eq!(events, [Awaited::Closed, Awaited::Loaded]);
        assert_eq!(seconds, SECONDS);
        let (_, seconds) = parse(&json!({ "events": ["taken"], "seconds": 300 })).unwrap();
        assert_eq!(seconds, 300);
        for wrong in [
            json!({}),
            json!({ "events": [] }),
            json!({ "events": "loaded" }),
            json!({ "events": ["answered"] }),
            json!({ "events": ["loaded"], "seconds": 301 }),
            json!({ "events": ["loaded"], "seconds": 0 }),
            json!({ "events": ["loaded"], "seconds": "30" }),
        ] {
            assert!(parse(&wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn every_event_has_its_name_and_its_line() {
        for (name, event) in EVENTS {
            assert_eq!(event.name(), name);
        }
        let closed = |gone| Happened::Closed { tab: 7, gone }.line();
        assert_eq!(closed(Gone::Closed), "Tab 7 closed.");
        assert_eq!(closed(Gone::Left), "Tab 7 isn't yours any more.");
        assert_eq!(
            Happened::Taken { tab: 7 }.line(),
            "The user took tab 7 over."
        );
        assert_eq!(nothing(30), "Nothing happened in 30 s.");
    }

    #[test]
    fn the_card_says_what_a_wait_is_for() {
        let target = |events: Value, on| target(&json!({ "events": events }), on);
        assert_eq!(target(json!(["loaded"]), "maat.pt"), "maat.pt to load");
        assert_eq!(target(json!(["loaded"]), ""), "a page to load");
        assert_eq!(target(json!(["download"]), "maat.pt"), "a download");
        assert_eq!(target(json!(["loaded", "closed"]), "maat.pt"), "an event");
    }
}
