//! An agent's sign-in with a login the user saved (#4098): the core fills and submits it, and the
//! agent never sees it. The app decides whether it may be asked for at all, before the core is:
//! Settings' switch, the user's Off limits list and the task's access. The account it was filled
//! with is the user's ilo line's alone, in memory: never in the 30-day file, a log line or an
//! answer.

use std::sync::mpsc;

use gpui::{Context, SharedString};
use serde_json::Value;

use super::task::Line;
use super::task::ilo::{Account, IloKind, IloLine, SignedInWith};
use super::tier::{self, Why};
use super::{Act, Agents, Failure, Held, Refused, ToolResult, act_of, held, page, site, tools};

/// Whether Sign in for agents is on until the user changes it: in a dev build, not yet in the
/// owner's daily one, which waits on their word.
pub(super) const STARTS_ON: bool = !cfg!(feature = "prod");

/// Settings › Agents' switch, and the words beside it.
pub const HEADING: &str = "Sign in for agents";
pub const SWITCH_LINE: &str = "A page that steers an agent can get it signed in to any site you \
    have a login saved for. It is filled only on that site, and the agent never sees it.";
pub const SWITCH_OFF_LINE: &str = "Off: ilo fills no saved login for an agent.";

/// What the agent hears while Settings' switch is off.
const OFF: &str = "Signing in for agents is off in Settings › Agents, which only the user changes.";

/// What the user's card says they're to do, when a sign-in is theirs.
const YOURS: &str =
    "Sign in on this page: ilo couldn't do it for the agent. Then hand the tab back.";

/// A sign-in the core is doing, for the call that waits on it.
pub(super) struct Signing {
    agent: String,
    connection: u64,
    call: u64,
    tab: u64,
    reply: mpsc::Sender<ToolResult>,
}

/// The first key of the ilo lines a sign-in adds, each its call's place in the task's trace past
/// it, apart from the keys of the lines a kept task adds below usize::MAX.
const FILLED: usize = usize::MAX / 2;

impl Agents {
    /// Whether Settings › Agents' Sign in for agents is on.
    pub fn sign_in_on(&self) -> bool {
        self.sign_in_on
    }

    /// Settings › Agents' Sign in for agents. Off, no agent's sign_in reaches the core.
    pub fn set_sign_in_on(&mut self, on: bool, cx: &mut Context<Self>) {
        if on == self.sign_in_on {
            return;
        }
        self.sign_in_on = on;
        self.save();
        cx.notify();
    }

    /// The agent's sign_in: past Settings' switch, the core's own refusal, the Off limits list,
    /// the task's Sites and its access, the core signs the tab in. The core's refusal comes before
    /// the site's, so a tab whose page is withheld says nothing of where it is.
    pub(super) fn sign_in_call(
        &mut self,
        agent: &str,
        connection: u64,
        call: u64,
        arguments: &Value,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tab_for(agent, connection, arguments, &reply, cx) else {
            return;
        };
        // Empty for a withheld tab, as Browser::tab gives it.
        let at = site(&self.browser.tab(tab, cx).url);
        let act = Act {
            action: "sign_in",
            what: "sign in with the user's saved login".into(),
            ..act_of(agent, connection, call, tab, reply)
        };
        if !self.sign_in_on {
            return self.refuse(act, &at, Refused::of(tools::Kind::Outside, OFF), cx);
        }
        if let Err(refused) = self.may_act(agent, tab, "sign_in", cx) {
            return self.refuse(act, &at, refused, cx);
        }
        let host = site(&self.browser.shown(tab, cx).url);
        let tier = self.tier_of(agent, connection, call);
        let why = match tier::sign_in_why(page::opening_risk(&host).is_some()) {
            _ if self.fence_of(&host).is_some() => Some(Why::Fenced),
            _ if self.off_sites(&act, cx) => Some(Why::OtherSite),
            why => Some(why).filter(|why| !tier.allows(*why)),
        };
        if let Some(why) = why {
            return self.stopped(act, host, why, tier, cx);
        }
        self.note(agent, connection, "sign_in", &at, "");
        cx.notify();
        self.current.insert((agent.into(), connection), tab);
        let request = self.next_request;
        self.next_request += 1;
        // check.sh reads this line.
        log!("browser: agent {agent} signs in {tab}");
        let signing = Signing {
            agent: agent.into(),
            connection,
            call,
            tab,
            reply: act.reply,
        };
        self.signing.insert(request, signing);
        self.browser.sign_in(agent, tab, request);
    }

    /// The core's answer to the sign-in `request`, in its words, which never name an account:
    /// signed in, not, refused, or the user's step, which hands them the tab. Whether it submitted
    /// is the app's alone.
    pub fn sign_in_done(
        &mut self,
        request: u64,
        signed_in: bool,
        submitted: bool,
        text: String,
        failure: Option<Failure>,
        cx: &mut Context<Self>,
    ) {
        let Some(signing) = self.signing.remove(&request) else {
            return;
        };
        log!(
            "browser: agent {} sign-in in {} answered: signed in {signed_in}, submitted {submitted}",
            signing.agent,
            signing.tab
        );
        let answer = match failure {
            Some(Failure::UserStep) => return self.sign_in_yours(signing, text, cx),
            Some(failure) => ToolResult {
                kind: failure.kind(),
                ..ToolResult::error(text)
            },
            None if signed_in => ToolResult::text(text),
            None => ToolResult::error(text),
        };
        _ = signing.reply.send(answer);
        self.advance(cx);
    }

    /// The sign-in was the user's step: the tab goes to them as a handoff does, which the call
    /// waits on, and its answer starts with the core's words. A tab whose page showed what was
    /// filled the core holds already, as Take Over holds one.
    fn sign_in_yours(&mut self, signing: Signing, text: String, cx: &mut Context<Self>) {
        let Signing {
            agent,
            connection,
            call,
            tab,
            reply,
        } = signing;
        let (life, actions) = self.held_step(tab, held::Event::HandOff);
        let handed = life == held::Life::HandedOff
            && (self.browser.held(tab) || self.browser.hand_off(&agent, tab));
        if !handed {
            _ = reply.send(ToolResult::failed(tools::Kind::Yours, text));
            return self.advance(cx);
        }
        let at = site(&self.browser.tab(tab, cx).url);
        self.note(&agent, connection, "hand_off", &at, "");
        // check.sh reads this line.
        log!("browser: agent {agent} hands off {tab}: sign-in");
        let question = self.next_request;
        self.next_request += 1;
        let held = Held {
            agent,
            connection,
            call,
            why: YOURS.into(),
            question,
            back: None,
            reply: Some(reply),
            timer: None,
            told: Some(text),
        };
        self.held.insert(tab, held);
        self.carry(tab, held::Event::HandOff, life, actions, cx);
        self.advance(cx);
    }

    /// The core filled the login saved for `account` at `site` into `agent`'s `tab`, and its form
    /// went: the activity log's line, without the account, and the task's ilo line, which keeps
    /// it in memory alone.
    pub fn signed_in(
        &mut self,
        agent: &str,
        tab: u64,
        site: &str,
        account: String,
        cx: &mut Context<Self>,
    ) {
        // Its call, whose id may be any number an MCP client picks, 0 included.
        let signing = (self.signing.values())
            .find(|signing| signing.agent == agent && signing.tab == tab)
            .map(|signing| (signing.connection, signing.call));
        let connection =
            signing.map_or_else(|| self.holder(agent, tab), |(connection, _)| connection);
        self.note(agent, connection, "filled_login", site, "");
        cx.notify();
        // A ⌘T task's by its agent, a connected client's by the session its call is in.
        let Some((_, call)) = signing else {
            return;
        };
        let Some(task) = (self.tracer(agent, call)).and_then(|key| self.task_mut(&key)) else {
            return;
        };
        let Some(at) = (task.trace.iter()).rposition(|traced| traced.call == call) else {
            return;
        };
        let line = IloKind::SignedIn {
            tab,
            site: SharedString::from(site.to_string()),
            with: SignedInWith::Saved(Account::new(account)),
        };
        task.filled.push((at, line));
    }

    /// A task's sign-in lines, each under the stretch of work its call is in.
    pub(super) fn filled_lines(&self, task: &super::task::Task, lines: &mut Vec<Line>) {
        // The last first, so each one's place still counts the lines before it.
        for (at, kind) in task.filled.iter().rev() {
            let holds = |line: &Line| match line {
                Line::Trace(trace) => (trace.items.iter()).any(
                    |item| matches!(item, super::task::TraceItem::Step(step) if step.key == *at),
                ),
                Line::Step(step) => step.key == *at,
                _ => false,
            };
            let place = lines
                .iter()
                .position(holds)
                .map_or(lines.len(), |found| found + 1);
            let line = IloLine {
                key: FILLED + at,
                kind: kind.clone(),
                settled: false,
            };
            lines.insert(place, Line::Ilo(line));
        }
    }
}
