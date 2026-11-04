//! Library › Agents (docs/ux-spec.md › Library): the past tasks and sessions of a profile's
//! agents, as their receipts showed them. Only what a receipt shows is kept: the task's title, its
//! agent, when it ran, its pages' titles and addresses, and the orders and messages the user
//! allowed it. Never what a page said, what was typed, a cookie or the conversation. Kept on this
//! computer only, in the profile directory under the profile's name (the owner, 2026-11-04), never
//! synced, and never for a private window, which offers no tasks. Each lives 30 days from when it
//! ended.

use serde_json::{Value, json};

/// How long an entry lives once its task ended: 30 days (the owner).
pub const KEEP: u64 = 30 * 24 * 60 * 60;

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
}

impl Past {
    /// What its row says under its title: "Claude Code · 14:02 · 1 order placed · 3 pages".
    pub fn about(&self) -> String {
        let count = |n: usize, one: &str, many: &str| match n {
            0 => None,
            1 => Some(format!("1 {one}")),
            n => Some(format!("{n} {many}")),
        };
        let parts = [
            Some(self.agent.clone()).filter(|agent| !agent.is_empty()),
            Some(crate::agents::local_time(self.ended)),
            count(self.orders, "order placed", "orders placed"),
            count(self.messages, "message sent", "messages sent"),
            count(self.pages.len(), "page", "pages"),
        ];
        parts.into_iter().flatten().collect::<Vec<_>>().join(" · ")
    }

    fn to_json(&self) -> Value {
        let pages: Vec<Value> = (self.pages.iter())
            .map(|(title, url)| json!({ "title": title, "url": url }))
            .collect();
        json!({
            "id": self.id, "title": self.title, "agent": self.agent,
            "started": self.started, "ended": self.ended, "pages": pages,
            "orders": self.orders, "messages": self.messages,
        })
    }

    fn from_json(value: &Value) -> Option<Past> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        let number = |key: &str| value.get(key).and_then(Value::as_u64);
        let pages = (value.get("pages")?.as_array()?.iter())
            .filter_map(|page| {
                let field = |key: &str| page.get(key).and_then(Value::as_str).map(str::to_string);
                Some((field("title")?, field("url")?))
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

/// `entries` without the one `id`.
pub fn deleted(entries: Vec<Past>, id: &str) -> Vec<Past> {
    entries.into_iter().filter(|each| each.id != id).collect()
}

#[cfg(test)]
mod tests;
