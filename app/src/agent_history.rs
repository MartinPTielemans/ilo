//! Library › Agents (docs/ux-spec.md › Library): the past tasks and sessions of a profile's
//! agents, as their receipts showed them. Only what a receipt shows is kept: the task's title, its
//! agent, when it ran, its pages' titles and addresses, the orders and messages the user allowed
//! it, and its activity log's entries, each what it did, on which site, when, and why it was
//! refused, and for a request where it went and what it sent, without its secrets (#2326, which
//! the owner approved). Never what a page said, what else on it the agent acted on, what was
//! typed, a cookie or the conversation. Kept on this
//! computer only, in the profile's own folder as `FILE` (the owner, 2026-11-04), never synced, and
//! never for a private window, which offers no tasks. Each lives 30 days from when it ended, and
//! goes with the history the user deletes of its time.

use serde_json::{Value, json};

use crate::agents::request::Sent;

/// The file in a profile's folder its past tasks are kept in.
pub const FILE: &str = "Agent sessions";

/// How long an entry lives once its task ended: 30 days (the owner).
pub const KEEP: u64 = 30 * 24 * 60 * 60;

/// The most of a task's activity log an entry keeps, its last.
pub const STEPS: usize = 200;

/// An entry of a past task's activity log.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    /// When, in seconds since 1970.
    pub at: u64,
    /// The core's name for what it did, such as "read_page" (`agents::tools::describe`).
    pub action: String,
    /// The host of the page it did it to, or empty.
    pub site: String,
    /// Why it was refused, or empty if it wasn't.
    pub refused: String,
    /// A request's method and path, its whole address, and what it sent, or would have; a web
    /// search's count of the thread's searches, never their words; empty for any other step, and
    /// for a request kept before they were.
    pub target: String,
    pub url: String,
    pub sent: Vec<Sent>,
    /// The access its task had as it was judged, by its name, or empty for a step kept before.
    pub tier: String,
}

/// A past task or session, as its receipt showed it.
#[derive(Clone, Debug, PartialEq)]
pub struct Past {
    /// The task's own id and when it started, which a later turn of the same task keeps.
    pub id: String,
    pub title: String,
    /// Its agent's name, such as "Claude Code".
    pub agent: String,
    /// When it started and ended, in seconds since 1970.
    pub started: u64,
    pub ended: u64,
    /// Its pages, each its title and address.
    pub pages: Vec<(String, String)>,
    pub orders: usize,
    pub messages: usize,
    /// Its activity log, oldest first.
    pub steps: Vec<Step>,
    /// Whose answers for good its agent's are, the key Settings › Agents lists them under: a
    /// connected app's id, or a ⌘T CLI's (`agents::task_answers`). Empty for an entry from before
    /// they were kept.
    pub answers: String,
    /// How its last turn ended, or None for an entry from before that was kept, and for a sort.
    pub end: Option<crate::agents::kept::End>,
    /// A sort of today's tabs into folders, in place of a task: then its entry is its counts, its
    /// assistant's id in `agent`, and when it ran, and nothing else.
    pub sorted: Option<Sorted>,
}

/// What an entry keeps of a sort of today's tabs into folders (#4252): counts alone, never a
/// title, a site or a folder's name. One for each list handed over, folders 0 if it failed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sorted {
    /// The tabs handed over, the tabs put in folders, and the folders that took them.
    pub handed: usize,
    pub moved: usize,
    pub folders: usize,
}

impl Past {
    /// A sort's entry, by `assistant`'s id, such as "claude", from `started` to `ended`.
    pub fn sort(assistant: &str, started: u64, ended: u64, sorted: Sorted) -> Past {
        Past {
            id: format!("sort-{started}"),
            title: String::new(),
            agent: assistant.to_string(),
            started,
            ended,
            pages: Vec::new(),
            orders: 0,
            messages: 0,
            steps: Vec::new(),
            answers: String::new(),
            end: None,
            sorted: Some(sorted),
        }
    }

    /// What its row says under its title, with `when` it ended as the list it's in says a time:
    /// "Claude Code · 14:02 · 1 order placed · 3 pages".
    pub fn about(&self, when: &str) -> String {
        let count = |n: usize, one: &str, many: &str| match n {
            0 => None,
            1 => Some(format!("1 {one}")),
            n => Some(format!("{n} {many}")),
        };
        let parts = [
            Some(self.agent.clone()).filter(|agent| !agent.is_empty()),
            Some(when.to_string()).filter(|when| !when.is_empty()),
            count(self.orders, "order placed", "orders placed"),
            count(self.messages, "message sent", "messages sent"),
            count(self.pages.len(), "page", "pages"),
        ];
        parts.into_iter().flatten().collect::<Vec<_>>().join(" · ")
    }

    /// What its row says under its title, with `when` it ended as W4 says a time, today's time or
    /// an earlier day's date: how its last turn ended, "Done 14:02", or the time alone for an entry
    /// from before that was kept.
    pub fn ended(&self, when: &str) -> String {
        match self.end {
            Some(end) => format!("{} {when}", end.word().alone(&self.agent)),
            None => when.to_string(),
        }
    }

    /// What its page says under its title: "Claude Code · 12 min · 14:02 · 1 order placed · 3
    /// pages", as its row with how long it ran.
    pub fn page_about(&self) -> String {
        let about = self.about(&crate::agents::local_time(self.ended));
        match about
            .strip_prefix(&self.agent)
            .filter(|_| !self.agent.is_empty())
        {
            Some(rest) => format!("{} · {}{rest}", self.agent, self.length()),
            None => format!("{} · {about}", self.length()),
        }
    }

    /// How long it ran, as its page says it: "Under a minute", "12 min", "1 h 5 min".
    pub fn length(&self) -> String {
        let minutes = self.ended.saturating_sub(self.started) / 60;
        match (minutes / 60, minutes % 60) {
            (0, 0) => "Under a minute".to_string(),
            (0, minutes) => format!("{minutes} min"),
            (hours, 0) => format!("{hours} h"),
            (hours, minutes) => format!("{hours} h {minutes} min"),
        }
    }

    /// The sites it acted on, as its log names them, then its pages', each once, in the order it
    /// came to them.
    pub fn sites(&self) -> Vec<String> {
        let pages = self.pages.iter().map(|(_, url)| crate::agents::site(url));
        let mut sites: Vec<String> = Vec::new();
        for site in (self.steps.iter().map(|step| step.site.clone())).chain(pages) {
            if !site.is_empty() && !sites.contains(&site) {
                sites.push(site);
            }
        }
        sites
    }

    pub(crate) fn to_json(&self) -> Value {
        // No title or pages: a build from before sorts reads no task in the line, and skips it.
        if let Some(sorted) = self.sorted {
            return json!({
                "id": self.id, "agent": self.agent, "started": self.started, "ended": self.ended,
                "sorted": {
                    "handed": sorted.handed, "moved": sorted.moved, "folders": sorted.folders,
                },
            });
        }
        let pages: Vec<Value> = (self.pages.iter())
            .map(|(title, url)| json!({ "title": title, "url": url }))
            .collect();
        let steps: Vec<Value> = (self.steps.iter())
            .map(|step| {
                let mut kept = json!({
                    "at": step.at, "action": step.action, "site": step.site,
                    "refused": step.refused,
                });
                if !step.url.is_empty() {
                    let sent: Vec<Value> = (step.sent.iter())
                        .map(|line| json!([line.depth, line.name, line.value]))
                        .collect();
                    kept["target"] = json!(step.target);
                    kept["url"] = json!(step.url);
                    kept["sent"] = json!(sent);
                }
                // A search's entry holds its words, clipped, which its row says (AP25).
                if step.action == crate::agents::tools::SEARCHED {
                    kept["target"] = json!(step.target);
                }
                if !step.tier.is_empty() {
                    kept["tier"] = json!(step.tier);
                }
                kept
            })
            .collect();
        let mut kept = json!({
            "id": self.id, "title": self.title, "agent": self.agent,
            "started": self.started, "ended": self.ended, "pages": pages,
            "orders": self.orders, "messages": self.messages, "steps": steps,
            "answers": self.answers,
        });
        if let Some(end) = self.end {
            kept["end"] = json!(end.id());
        }
        kept
    }

    fn from_json(value: &Value) -> Option<Past> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        let number = |key: &str| value.get(key).and_then(Value::as_u64);
        if let Some(sorted) = value.get("sorted") {
            let count = |key: &str| Some(sorted.get(key)?.as_u64()? as usize);
            let sorted = Sorted {
                handed: count("handed")?,
                moved: count("moved")?,
                folders: count("folders")?,
            };
            let past = Past::sort(
                &text("agent")?,
                number("started")?,
                number("ended")?,
                sorted,
            );
            return Some(Past {
                id: text("id")?,
                ..past
            });
        }
        let pages = (value.get("pages")?.as_array()?.iter())
            .filter_map(|page| {
                let field = |key: &str| page.get(key).and_then(Value::as_str).map(str::to_string);
                Some((field("title")?, field("url")?))
            })
            .collect();
        // Entries from before steps and answers were kept have neither.
        let steps = (value
            .get("steps")
            .and_then(Value::as_array)
            .into_iter()
            .flatten())
        .filter_map(|step| {
            let field = |key: &str| step.get(key).and_then(Value::as_str).map(str::to_string);
            let sent = (step
                .get("sent")
                .and_then(Value::as_array)
                .into_iter()
                .flatten())
            .filter_map(|line| {
                let line = line.as_array()?;
                Some(Sent {
                    depth: u8::try_from(line.first()?.as_u64()?).ok()?,
                    name: line.get(1)?.as_str()?.to_string(),
                    value: line.get(2)?.as_str()?.to_string(),
                })
            })
            .collect();
            Some(Step {
                at: step.get("at")?.as_u64()?,
                action: field("action")?,
                site: field("site").unwrap_or_default(),
                refused: field("refused").unwrap_or_default(),
                target: field("target").unwrap_or_default(),
                url: field("url").unwrap_or_default(),
                sent,
                tier: field("tier").unwrap_or_default(),
            })
        })
        .collect();
        Some(Past {
            id: text("id")?,
            title: text("title")?,
            agent: text("agent")?,
            started: number("started")?,
            ended: number("ended")?,
            pages,
            orders: number("orders")? as usize,
            messages: number("messages")? as usize,
            steps,
            answers: text("answers").unwrap_or_default(),
            // An entry from before its end was kept has none, nor one a later build ended otherwise.
            end: text("end").and_then(|end| {
                (crate::agents::kept::End::ALL.into_iter()).find(|each| each.id() == end)
            }),
            sorted: None,
        })
    }
}

/// A profile's entries as the file has them, a line each, newest first. A line it can't read is
/// left out.
pub fn read(text: &str) -> Vec<Past> {
    (text.lines())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| Past::from_json(&value))
        .collect()
}

/// The entries as a profile's lines of the file.
pub fn write(entries: &[Past]) -> String {
    entries
        .iter()
        .map(|past| format!("{}\n", past.to_json()))
        .collect()
}

/// Those that haven't lived `KEEP` past their end at `now`.
pub fn living(entries: Vec<Past>, now: u64) -> Vec<Past> {
    (entries.into_iter())
        .filter(|past| past.ended.saturating_add(KEEP) > now)
        .collect()
}

/// `entries` with `past` kept, first, in place of an earlier turn of the same task, and those past
/// `KEEP` gone.
pub fn kept(entries: Vec<Past>, past: Past, now: u64) -> Vec<Past> {
    let rest: Vec<Past> = entries
        .into_iter()
        .filter(|each| each.id != past.id)
        .collect();
    living(std::iter::once(past).chain(rest).collect(), now)
}

/// `entries` without those that ran in the history the user deleted from `begin` to `end`, in
/// seconds since 1970: none when `begin` isn't before `end`.
pub fn outside(entries: Vec<Past>, begin: u64, end: u64) -> Vec<Past> {
    (entries.into_iter())
        .filter(|past| begin >= end || past.ended < begin || past.started > end)
        .collect()
}

/// `entries` without the one `id`.
pub fn deleted(entries: Vec<Past>, id: &str) -> Vec<Past> {
    entries.into_iter().filter(|each| each.id != id).collect()
}

#[cfg(test)]
mod tests;
