//! Saved reads (#3819, #3822): an agent proposes a read of its tab's site with propose_read, the
//! user saves it from the agent's card, and connected agents then read it with use_read, with no
//! tab, on the profile's own session (bc_profile_read), and list them with list_reads. A read is
//! of one origin, its definition's, and a parameter never steers it off its path
//! (crate::saved_reads). What a read answered never reaches the agent but as rows or a state:
//! never the address it landed on, nor where a redirect went.

pub mod rows;

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::mpsc;

use gpui::{Context, SharedString};
use serde_json::{Map, Value, json};

use super::saved_read::{self, Answer as CoreAnswer};
use super::{Act, Agents, Asking, Fenced, PageAction, Question, Show, ToolResult, Waits};
use crate::saved_reads::{self, Definition, Outcome, Row, Saved, State, Store};

/// The rows a proposal's card shows at most.
pub const SAMPLE_ROWS: usize = 3;

/// A read an agent proposed, with what reading it once gave, for the user's card.
#[derive(Debug)]
pub struct Proposal {
    pub read: Definition,
    /// The values the agent gave its parameters for this read, by name.
    pub sample: Vec<(String, String)>,
    /// The rows that read gave, which the card shows the first of.
    pub rows: Vec<Row>,
    /// What it changes of the saved read of its name, or empty for a new one: "columns", "url"…
    pub changes: Vec<&'static str>,
    profile: u64,
}

impl Proposal {
    /// The site as the card and the log say it: its host.
    pub fn site(&self) -> String {
        shown(&self.read.site)
    }

    /// "Sample for crate: serde, days: 7. Agents can use other values.", or None without
    /// parameters.
    pub fn sample_line(&self) -> Option<String> {
        if self.sample.is_empty() {
            return None;
        }
        let shown = |text: &str| saved_reads::text(text, saved_reads::definition::NAME_MOST);
        let values: Vec<String> = (self.sample.iter())
            .map(|(name, value)| format!("{}: {}", shown(name), shown(value)))
            .collect();
        Some(format!(
            "Sample for {}. Agents can use other values.",
            values.join(", ")
        ))
    }

    /// A proposal of the read `arguments` describe on `origin`, which gave `rows`, as a card shows.
    #[cfg(test)]
    pub(crate) fn of(arguments: &Value, origin: &str, rows: Vec<Row>) -> Proposal {
        Proposal {
            read: definition(arguments, origin).expect("not a read"),
            sample: Vec::new(),
            rows,
            changes: Vec::new(),
            profile: 1,
        }
    }

    /// The read block's name for a screen reader: "{name}, {n} columns: {columns}".
    pub fn heard(&self) -> String {
        let columns: Vec<&str> = (self.read.columns.iter())
            .map(|column| column.name.as_str())
            .collect();
        let count = match columns.len() {
            1 => "1 column".to_string(),
            n => format!("{n} columns"),
        };
        format!("{}, {count}: {}", self.read.name, columns.join(", "))
    }
}

/// A use of a saved read waiting its turn: uses of one read go one at a time (#3822).
struct Use {
    agent: String,
    /// The connection the use came on: when it closes, its uses waiting go with it.
    connection: u64,
    name: String,
    site: String,
    address: String,
    reply: mpsc::Sender<ToolResult>,
}

/// The profiles' saved reads, as they're asked about, and what waits on them.
#[derive(Default)]
pub(super) struct Reads {
    /// Each profile's, read from its folder the first time: None for one with no folder, as a
    /// private window's.
    stores: HashMap<u64, Option<Store>>,
    /// The uses of each read, by profile and name in lower case: the first is the one under way.
    waiting: HashMap<(u64, String), VecDeque<Use>>,
    /// The reads the user removed, newest last, each with its profile and where it was, for ⌘Z.
    removed: Vec<(u64, Saved, usize)>,
}

/// The agent's text for a read of a site on the user's Always ask first list, which stops it.
fn paused(site: &str) -> String {
    format!("paused: {site} is on Always ask first, so saved reads for it are stopped.")
}

/// The agent's text for a proposal on a site on that list, which can't be saved.
fn not_allowed(site: &str) -> String {
    format!(
        "not_allowed: {site} is on Always ask first, so reads can't be saved for it. Open {site} \
         in a tab to ask the user instead."
    )
}

/// The agent's text for a read the site wants a sign-in for.
fn needs_sign_in(site: &str) -> String {
    format!(
        "needs_sign_in: {site} requires a sign-in. Ask the user to sign in to {site} in ilo, then \
         retry use_read."
    )
}

/// What a site's answer to a read came to, from the core's answer: a read with an error is a
/// failure whatever came of its body (the orchestrator's read of the core, #3820).
enum Came {
    Outcome(Outcome),
    /// No whole answer came, such as a timeout or a connection lost.
    Failed,
}

fn came(read: &Definition, answer: &CoreAnswer) -> Came {
    if answer.error.is_some() {
        return Came::Failed;
    }
    let answered = saved_reads::Answered {
        status: answer.status,
        url: &answer.url,
        left: answer.redirect.is_some(),
        mime: &answer.kind,
        body: &answer.body,
        cut: answer.cut,
    };
    Came::Outcome(saved_reads::outcome(read, &answered))
}

/// The fields `read` changes of `saved`, as the log names them.
fn changes(saved: &Definition, read: &Definition) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if saved.kind != read.kind {
        changed.push("kind");
    }
    if saved.url != read.url || saved.params != read.params {
        changed.push("url");
    }
    if saved.columns != read.columns || saved.rows != read.rows {
        changed.push("columns");
    }
    if saved.least_rows != read.least_rows {
        changed.push("least rows");
    }
    if saved.sign_in != read.sign_in {
        changed.push("sign-in page");
    }
    changed
}

/// The rows as an agent gets them: each a map of its columns' names to their values.
fn rows_json(read: &Definition, rows: &[Row]) -> Value {
    let rows: Vec<Value> = (rows.iter())
        .map(|row| {
            let fields: Map<String, Value> = (read.columns.iter().zip(&row.0))
                .map(|(column, value)| (column.name.clone(), json!(value)))
                .collect();
            Value::Object(fields)
        })
        .collect();
    json!(rows)
}

impl Agents {
    /// The profile `agent` works in: its Space's.
    fn reads_profile(&mut self, agent: &str, cx: &mut Context<Self>) -> u64 {
        let space = self.space(agent, cx);
        self.browser.space_profile(space)
    }

    /// `profile`'s saved reads, read from its folder the first time, or None if it has none.
    fn store(&mut self, profile: u64) -> Option<&mut Store> {
        let browser = self.browser.clone();
        (self.reads.stores.entry(profile))
            .or_insert_with(|| browser.saved_reads(profile).map(|text| Store::read(&text)))
            .as_mut()
    }

    /// Keeps `profile`'s saved reads in its folder.
    fn keep_reads(&mut self, profile: u64) {
        if let Some(Some(store)) = self.reads.stores.get(&profile) {
            self.browser.set_saved_reads(profile, &store.write());
        }
    }

    /// `profile`'s saved reads, for Settings: none for a profile with no folder.
    pub fn saved_reads(&self, profile: u64) -> Vec<Saved> {
        match self.reads.stores.get(&profile) {
            Some(store) => store.as_ref().map(|store| store.list().to_vec()),
            None => {
                (self.browser.saved_reads(profile)).map(|text| Store::read(&text).list().to_vec())
            }
        }
        .unwrap_or_default()
    }

    /// Whether `profile` keeps saved reads: a private window's doesn't.
    pub fn keeps_reads(&self, profile: u64) -> bool {
        match self.reads.stores.get(&profile) {
            Some(store) => store.is_some(),
            None => self.browser.saved_reads(profile).is_some(),
        }
    }

    /// The agent the read was asked for by, by its id, whose log the user's changes to it go in.
    fn asker(&self, saved: &Saved) -> Option<String> {
        (super::Client::ALL.iter())
            .map(|client| client.id().to_string())
            .find(|id| self.agent_name(id) == saved.asked_by.as_str())
    }

    /// The user's Remove Read in Settings, with no dialog: ⌘Z brings it back (`undo_remove_read`).
    pub fn remove_read(&mut self, profile: u64, name: &str, cx: &mut Context<Self>) {
        let Some(store) = self.store(profile) else {
            return;
        };
        let Some(at) = store.at(name) else { return };
        let Some(saved) = store.remove(name) else {
            return;
        };
        self.keep_reads(profile);
        // check.sh reads this line.
        crate::log!("browser: saved read removed in profile {profile}");
        if let Some(agent) = self.asker(&saved) {
            let host = shown(&saved.read.site);
            self.note_on(&agent, 0, "remove_read", &host, &saved.read.name, "");
        }
        self.reads.removed.push((profile, saved, at));
        cx.notify();
    }

    /// ⌘Z in Settings › Agents: the read removed last comes back where it was. Whether one did.
    pub fn undo_remove_read(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((profile, saved, at)) = self.reads.removed.pop() else {
            return false;
        };
        let Some(store) = self.store(profile) else {
            return false;
        };
        // A read of its name saved since keeps its place.
        if store.restore(saved, at).is_err() {
            return false;
        }
        self.keep_reads(profile);
        // check.sh reads this line.
        crate::log!("browser: saved read restored in profile {profile}");
        cx.notify();
        true
    }

    /// What ⌘Z would bring back goes once Settings closes, as Links' does.
    pub fn forget_removed_reads(&mut self) {
        self.reads.removed.clear();
    }

    /// Whether the read of `origin` is of a site on the user's Always ask first list, or under one
    /// of its entries.
    pub fn fenced_read(&self, origin: &str) -> bool {
        self.fence_of(&super::site(origin)).is_some()
    }

    /// list_reads: the profile's saved reads, each its name, site, columns, parameters and state.
    pub(super) fn list_reads(
        &mut self,
        agent: &str,
        connection: u64,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let profile = self.reads_profile(agent, cx);
        let reads = match self.store(profile) {
            Some(store) => store.list().to_vec(),
            None => Vec::new(),
        };
        let listed: Vec<Value> = (reads.iter())
            .map(|saved| {
                let read = &saved.read;
                let params: Map<String, Value> = (read.params.iter())
                    .map(|(name, param)| (name.clone(), param_json(param)))
                    .collect();
                let columns: Vec<&str> = read.columns.iter().map(|c| c.name.as_str()).collect();
                json!({
                    "name": read.name,
                    "site": shown(&read.site),
                    "columns": columns,
                    "params": params,
                    "state": match self.fenced_read(&read.site) {
                        true => "paused",
                        false => saved.state.id(),
                    },
                })
            })
            .collect();
        self.note(agent, connection, "list_reads", "", "");
        cx.notify();
        _ = reply.send(ToolResult::text(json!({ "reads": listed }).to_string()));
    }

    /// propose_read: checks the agent's definition of a read of its tab's site, reads it once with
    /// the sample values it gave, and asks the user on its card whether to save it.
    pub(super) fn propose_read(
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
        let url = self.browser.tab(tab, cx).url.to_string();
        let host = super::site(&url);
        if let Some(why) = self.off_limits(agent, tab, cx) {
            self.note(agent, connection, "propose_read", &host, why);
            cx.notify();
            _ = reply.send(ToolResult::error(why));
            return;
        }
        if !self.owns(agent, tab, cx) {
            _ = reply.send(ToolResult::error(
                "propose_read requires one of your own tabs. Open the site with open_tab.",
            ));
            return;
        }
        let Some(origin) = crate::agents::request::origin(&url) else {
            _ = reply.send(ToolResult::error(
                "The tab isn't on a web page. Reads only work on http(s) sites.",
            ));
            return;
        };
        // A site on Always ask first asks at every step, so a read that never asks can't be saved
        // for it (#3787), even on a tab the user let stay there.
        if self.fenced_read(&origin) {
            let host = shown(&origin);
            self.note(agent, connection, "read_not_allowed", &host, "");
            cx.notify();
            _ = reply.send(ToolResult::error(not_allowed(&host)));
            return;
        }
        let read = match definition(arguments, &origin) {
            Ok(read) => read,
            Err(why) => {
                _ = reply.send(ToolResult::error(why));
                return;
            }
        };
        let profile = self.reads_profile(agent, cx);
        let saved = match self.store(profile) {
            None => {
                _ = reply.send(ToolResult::error(
                    "Can't save reads in a private window. Read the site in your tab instead.",
                ));
                return;
            }
            Some(store) => store.get(&read.name).map(|saved| saved.read.clone()),
        };
        if saved.as_ref() == Some(&read) {
            _ = reply.send(ToolResult::text(format!(
                "saved: \"{}\" is already saved as proposed. Call use_read with its name.",
                read.name
            )));
            return;
        }
        let sample = match arguments.get("sample") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(sample)) => sample.clone(),
            Some(_) => {
                _ = reply.send(ToolResult::error(
                    "sample must be an object with a value for each parameter, used to preview the read on the user's card.",
                ));
                return;
            }
        };
        let address = match saved_reads::address(&read, &sample) {
            Ok(address) => address,
            Err(saved_reads::Refused(why)) => {
                _ = reply.send(ToolResult::error(why));
                return;
            }
        };
        let sample: Vec<(String, String)> = (read.params.keys())
            .filter_map(|name| {
                let value = sample.get(name)?;
                let value = value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_string);
                Some((name.clone(), value))
            })
            .collect();
        let changes = saved
            .map(|saved| self::changes(&saved, &read))
            .unwrap_or_default();
        let agent = agent.to_string();
        // The read at the ask is the read's first: the card shows what it gave.
        self.read(
            profile,
            address.as_str(),
            saved_read::MAX_BYTES,
            saved_read::MAX_MILLISECONDS,
            move |answer, cx| {
                cx.notify();
                let host = shown(&read.site);
                let rows = match came(&read, &answer) {
                    Came::Outcome(Outcome::Rows(rows)) => rows,
                    Came::Outcome(Outcome::NeedsSignIn) => {
                        _ = reply.send(ToolResult::error(needs_sign_in(&host)));
                        return;
                    }
                    Came::Outcome(Outcome::Broken(broken)) => {
                        _ = reply.send(ToolResult::error(format!(
                            "broken: {host} didn't return matching rows. {} Fix the definition and call propose_read again.",
                            broken.0
                        )));
                        return;
                    }
                    Came::Failed => {
                        _ = reply.send(ToolResult::error(unreachable(&host)));
                        return;
                    }
                };
                let proposal = Proposal {
                    read,
                    sample,
                    rows,
                    changes,
                    profile,
                };
                // The core's answer comes while the Agents are being updated, so after it.
                cx.spawn(async move |agents, cx| {
                    _ = agents.update(cx, |agents, cx| {
                        agents.ask_to_save(&agent, connection, call, tab, proposal, reply, cx)
                    });
                })
                .detach();
            },
        );
    }

    /// Asks the user on the agent's card whether to save `proposal`, as any question asks, with
    /// its wait (ASK_WAIT).
    fn ask_to_save(
        &mut self,
        agent: &str,
        connection: u64,
        call: u64,
        tab: u64,
        proposal: Proposal,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let host = proposal.site();
        let act = Act {
            agent: agent.to_string(),
            connection,
            call,
            action: "propose_read",
            tab,
            url: String::new(),
            page: PageAction::Snapshot,
            element: 0,
            text: String::new(),
            note: String::new(),
            show: Show::Nothing,
            what: format!("save a read of {host}"),
            // Not an action on the tab's page: the site is the read's, which the agent names, so
            // the card has no "On <site>" head, the tab's own, and keeps its sentence whole.
            asks: String::new(),
            field: String::new(),
            download: 0,
            file: None,
            requests: Vec::new(),
            did: None,
            fenced: Fenced::No,
            wait: None,
            reply,
        };
        let question = Question {
            id: self.next_request,
            agent: self.agent_name(agent),
            from: agent.to_string().into(),
            what: act.what.clone().into(),
            why: SharedString::default(),
            site: host.clone().into(),
            // Saving a read asks each time, so its card has no Always or Never.
            each_time: true,
            read: Some(Rc::new(proposal)),
            ..Question::default()
        };
        self.next_request += 1;
        // check.sh reads this line.
        crate::log!("browser: agent {agent} asks {}: {}", question.id, act.what);
        let id = question.id;
        self.asking.push(Asking {
            question,
            site: host,
            act,
            _timer: cx.spawn(async move |agents, cx| {
                cx.background_executor().timer(Waits::launch().ask).await;
                _ = agents.update(cx, |agents, cx| agents.unanswered(id, cx));
            }),
        });
        self.asked(cx);
    }

    /// The user's answer to a proposal: Save Read keeps it, anything else declines it. `answered`
    /// is false when its wait ran out.
    pub(super) fn answer_read(
        &mut self,
        act: Act,
        question: Question,
        save: bool,
        answered: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(proposal) = question.read else {
            return;
        };
        let (name, host) = (proposal.read.name.clone(), proposal.site());
        if !save {
            let (action, text) = match answered {
                true => (
                    "decline_read",
                    format!(
                        "declined: the user didn't save \"{name}\". Do not propose it again now; read {host} in a tab instead."
                    ),
                ),
                false => (
                    "unanswered_read",
                    format!(
                        "declined: the request to save \"{name}\" timed out with no answer. Do not propose it again now; read {host} in a tab instead."
                    ),
                ),
            };
            self.note_on(&act.agent, act.connection, action, &host, &name, "");
            cx.notify();
            _ = act.reply.send(ToolResult::error(text));
            return;
        }
        let profile = proposal.profile;
        let asked_by = self.agent_name(&act.agent).to_string();
        let now = crate::usage::now();
        let read = proposal.read.clone();
        let changing = !proposal.changes.is_empty();
        let kept = self.store(profile).map(|store| match changing {
            true => store.change(&name, read, State::Ready, now),
            false => store.save(read, &asked_by, State::Ready, now),
        });
        let answer = match kept {
            Some(Ok(())) => {
                self.keep_reads(profile);
                match changing {
                    true => {
                        let target = format!("{name}: {}", proposal.changes.join(", "));
                        self.note_on(
                            &act.agent,
                            act.connection,
                            "change_read",
                            &host,
                            &target,
                            "",
                        );
                    }
                    false => {
                        self.note_on(&act.agent, act.connection, "save_read", &host, &name, "")
                    }
                }
                Ok(format!(
                    "saved: the user saved \"{name}\". Call use_read with its name to read {host} without a tab."
                ))
            }
            Some(Err(saved_reads::Refusal::Full)) => Err(format!(
                "The profile keeps {} saved reads, its most: the user can remove one in Settings › Agents.",
                saved_reads::store::MOST
            )),
            // Another of its name was saved while it asked, or the one it changed was removed.
            Some(Err(_)) => Err(format!(
                "\"{name}\" changed while the user was asked. Call list_reads, then propose it again."
            )),
            None => Err("Can't save reads in a private window.".to_string()),
        };
        cx.notify();
        _ = act.reply.send(match answer {
            Ok(text) => ToolResult::text(text),
            Err(text) => ToolResult::error(text),
        });
    }

    /// use_read: reads a saved read with the values the agent gives its parameters, on its
    /// profile's own session with no tab, one use of a read at a time.
    pub(super) fn use_read(
        &mut self,
        agent: &str,
        connection: u64,
        arguments: &Value,
        reply: mpsc::Sender<ToolResult>,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = arguments.get("name").and_then(Value::as_str) else {
            _ = reply.send(ToolResult::error(
                "name must be a saved read's name, as returned by list_reads.",
            ));
            return;
        };
        let params = match arguments.get("params") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(params)) => params.clone(),
            Some(_) => {
                _ = reply.send(ToolResult::error(
                    "params must be an object with a value for each parameter.",
                ));
                return;
            }
        };
        let profile = self.reads_profile(agent, cx);
        let saved = self
            .store(profile)
            .and_then(|store| store.get(name).cloned());
        let Some(saved) = saved else {
            let name = saved_reads::text(name, saved_reads::definition::NAME_MOST);
            self.note_on(agent, connection, "read_not_found", "", &name, "");
            cx.notify();
            _ = reply.send(ToolResult::error(format!(
                "not_found: no saved read named \"{name}\". Call list_reads to see saved reads, or open the site in a tab and call propose_read."
            )));
            return;
        };
        // Paused while its site is on Always ask first, and read again once it's off: nothing of
        // the read's own changes.
        if self.fenced_read(&saved.read.site) {
            let site = shown(&saved.read.site);
            self.note_on(
                agent,
                connection,
                "read_paused",
                &site,
                &saved.read.name,
                "",
            );
            cx.notify();
            _ = reply.send(ToolResult::error(paused(&site)));
            return;
        }
        let address = match saved_reads::address(&saved.read, &params) {
            Ok(address) => address,
            Err(saved_reads::Refused(why)) => {
                _ = reply.send(ToolResult::error(why));
                return;
            }
        };
        let name = saved.read.name.clone();
        let key = (profile, name.to_lowercase());
        let waiting = self.reads.waiting.entry(key.clone()).or_default();
        waiting.push_back(Use {
            agent: agent.to_string(),
            connection,
            name,
            site: shown(&saved.read.site),
            address: address.to_string(),
            reply,
        });
        if waiting.len() == 1 {
            self.read_next(key);
        }
    }

    /// Sends the first use waiting on the read `key`.
    fn read_next(&mut self, key: (u64, String)) {
        let Some(address) = (self.reads.waiting.get(&key))
            .and_then(|waiting| waiting.front())
            .map(|first| first.address.clone())
        else {
            self.reads.waiting.remove(&key);
            return;
        };
        let profile = key.0;
        let done = key.clone();
        self.read(
            profile,
            &address,
            saved_read::MAX_BYTES,
            saved_read::MAX_MILLISECONDS,
            move |answer, cx| {
                // The core's answer comes while the Agents are being updated, so after it.
                cx.spawn(async move |agents, cx| {
                    _ = agents.update(cx, |agents, cx| agents.read_used(done, answer, cx));
                })
                .detach();
            },
        );
        crate::log!("browser: saved read sent in profile {profile}");
    }

    /// The core's answer to the use under way of the read `key`: the agent gets its rows or why
    /// not, the read keeps what it came to, and the next use goes.
    fn read_used(&mut self, key: (u64, String), answer: CoreAnswer, cx: &mut Context<Self>) {
        let Some(used) = (self.reads.waiting.get_mut(&key)).and_then(VecDeque::pop_front) else {
            return;
        };
        let profile = key.0;
        let now = crate::usage::now();
        let saved = self
            .store(profile)
            .and_then(|store| store.get(&used.name).cloned());
        let Some(saved) = saved else {
            // Removed while it read: what it read goes nowhere.
            _ = used.reply.send(ToolResult::error(format!(
                "not_found: no saved read named \"{}\". Call list_reads to see saved reads, or open the site in a tab and call propose_read.",
                used.name
            )));
            return self.read_next(key);
        };
        let host = used.site.clone();
        let (state, result) = match came(&saved.read, &answer) {
            Came::Outcome(Outcome::Rows(rows)) => {
                let text = json!({
                    "name": saved.read.name,
                    "site": host,
                    "rows": rows_json(&saved.read, &rows),
                });
                (Some(State::Ready), ToolResult::text(text.to_string()))
            }
            Came::Outcome(Outcome::NeedsSignIn) => (
                Some(State::NeedsSignIn),
                ToolResult::error(needs_sign_in(&host)),
            ),
            Came::Outcome(Outcome::Broken(broken)) => {
                let text = format!(
                    "broken: {host} didn't match the saved read. {} Open {host} in a tab to inspect it and call propose_read to update it.",
                    broken.0
                );
                (Some(State::Broken(broken.0)), ToolResult::error(text))
            }
            // No whole answer says nothing of the read: its state stays.
            Came::Failed => (None, ToolResult::error(unreachable(&host))),
        };
        let action = match &state {
            Some(State::Ready) => "use_read",
            Some(State::NeedsSignIn) => "read_needs_sign_in",
            Some(State::Broken(_)) => "read_broken",
            None => "read_unreachable",
        };
        if let Some(state) = state
            && let Some(store) = self.store(profile)
        {
            _ = store.read_at(&used.name, state, now);
            self.keep_reads(profile);
        }
        // The address it read, which the agent's values filled, never where the site sent it.
        self.note_entry(
            &used.agent,
            used.connection,
            action,
            &host,
            &used.name,
            "",
            (&used.address, Vec::new(), ""),
        );
        cx.notify();
        _ = used.reply.send(result);
        self.read_next(key);
    }

    /// The agent went, or with `connection`, that connection of its: its uses waiting their turn
    /// go unanswered. The one under way finishes, as its read was sent.
    pub(super) fn forget_reads(&mut self, agent: &str, connection: Option<u64>) {
        for waiting in self.reads.waiting.values_mut() {
            let first = waiting.pop_front();
            waiting.retain(|each| {
                each.agent != agent || connection.is_some_and(|gone| gone != each.connection)
            });
            if let Some(first) = first {
                waiting.push_front(first);
            }
        }
    }

    /// The question's read, for the card.
    pub fn proposal(&self, id: u64) -> Option<Rc<Proposal>> {
        (self.asking.iter())
            .find(|asking| asking.question.id == id)
            .and_then(|asking| asking.question.read.clone())
    }
}

/// A read's site as the user and the agent see it: its origin's host, and its port if it has one,
/// whole, as a read is of that one origin (`agents::site` drops a www., which is another origin).
pub fn shown(origin: &str) -> String {
    origin
        .split_once("://")
        .map_or(origin, |(_, host)| host)
        .to_string()
}

/// The agent's text when no whole answer came.
fn unreachable(site: &str) -> String {
    format!("unreachable: {site} didn't respond. Try use_read again later.")
}

/// A parameter as list_reads gives it.
fn param_json(param: &saved_reads::definition::Param) -> Value {
    use saved_reads::definition::Param;
    match param {
        Param::Integer { min, max } => json!({ "type": "integer", "min": min, "max": max }),
        Param::OneOf(values) => json!({ "type": "one_of", "values": values }),
        Param::Text { pattern } => json!({ "type": "text", "pattern": pattern }),
    }
}

/// The definition propose_read's arguments give, of the site `origin`, the tab's: the agent never
/// names the site, so a read is always of the origin it proposes from, its scheme too (an https
/// tab gives an https read, which HSTS's upgrade would otherwise send off its origin).
fn definition(arguments: &Value, origin: &str) -> Result<Definition, String> {
    let Some(given) = arguments.as_object() else {
        return Err(
            "propose_read arguments must be an object with name, kind, url and columns.".into(),
        );
    };
    let shown = self::shown(origin);
    if given.contains_key("site") {
        return Err(format!(
            "propose_read takes no site argument; it reads the tab's site ({shown})."
        ));
    }
    if let Some(url) = given.get("url").and_then(Value::as_str)
        && (!url.starts_with('/') || url.starts_with("//"))
    {
        return Err(format!(
            "url must be a path on the tab's site ({shown}), such as /api/items/{{id}}. Reads can't target another site or scheme."
        ));
    }
    let mut value: Map<String, Value> = (given.iter())
        .filter(|(key, _)| !matches!(key.as_str(), "tab" | "sample"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    value.insert("site".into(), json!(origin));
    Definition::parse(&Value::Object(value)).map_err(|wrong| wrong.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proposal_takes_the_tabs_origin_and_refuses_any_other() {
        let arguments = json!({
            "name": "buildings",
            "kind": "table",
            "url": "/reads/table",
            "columns": [{ "name": "Name", "type": "text", "from": "Name" }],
        });
        let read = definition(&arguments, "https://reads.test").unwrap();
        assert_eq!(read.site, "https://reads.test");

        // The same host over http, another host, a protocol-relative address, and a site.
        for url in [
            "http://reads.test/reads/table",
            "https://away.reads.test/reads/table",
            "//away.reads.test/reads/table",
        ] {
            let mut other = arguments.clone();
            other["url"] = json!(url);
            let why = definition(&other, "https://reads.test").unwrap_err();
            assert!(
                why.contains("can't target another site or scheme"),
                "{url}: {why}"
            );
        }
        let mut sited = arguments.clone();
        sited["site"] = json!("https://away.reads.test");
        let why = definition(&sited, "https://reads.test").unwrap_err();
        assert!(
            why.starts_with("propose_read takes no site argument"),
            "{why}"
        );
    }

    #[test]
    fn the_read_blocks_heard_name_says_its_columns_and_their_count() {
        let read = |columns: &[&str]| {
            let columns: Vec<Value> = (columns.iter())
                .map(|name| json!({ "name": name, "type": "text", "from": name }))
                .collect();
            let arguments = json!({ "name": "city buildings", "kind": "table", "url": "/t", "columns": columns });
            Proposal {
                read: definition(&arguments, "https://reads.test").unwrap(),
                sample: Vec::new(),
                rows: Vec::new(),
                changes: Vec::new(),
                profile: 1,
            }
        };
        assert_eq!(
            read(&["Name", "City", "Height (m)"]).heard(),
            "city buildings, 3 columns: Name, City, Height (m)"
        );
        assert_eq!(read(&["Name"]).heard(), "city buildings, 1 column: Name");
    }
}
