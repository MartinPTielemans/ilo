//! Quiet until it needs you, outside ilo (docs/ux-spec.md › Agent task, Needs you): the Dock's
//! badge, the taskbar button's overlay or the window's urgency says how many tasks wait for the
//! user, ⌘T's and outside agents' alike, and a task that comes to need the user or finishes posts
//! one system notification, as Settings › Agents' Notify says, whose click opens its task's
//! card. Nothing is kept of them past the run, and nothing else interrupts: no sound, no bounce, no
//! flash.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::{App, Global, SharedString};

use crate::agents::page::clip;
use crate::agents::task::handed::NEW_THREAD;
use crate::agents::task::{Card, Dot, Word};

/// Settings › Agents' group and row that choose when these post.
pub const NOTIFICATIONS: &str = "Notifications";
pub const WHEN_TO_NOTIFY: &str = "Notify";
const NOTIFY_AWAY: &str = concat!("When ", env!("APP_NAME"), " isn't in front");
const NOTIFY_ALWAYS: &str = "Always";
const NOTIFY_NEVER: &str = "Never";
/// The most of a task's title a notification carries, as its card's title holds at most.
const TITLE_CHARS: usize = 80;

/// When a task's notification is posted: Settings › Agents › Notifications, the app's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Notify {
    /// While ilo isn't the active app.
    #[default]
    Away,
    Always,
    Never,
}

impl Notify {
    pub const ALL: [Notify; 3] = [Notify::Away, Notify::Always, Notify::Never];

    /// As the agents' settings keep it.
    pub fn id(self) -> &'static str {
        match self {
            Notify::Away => "away",
            Notify::Always => "always",
            Notify::Never => "never",
        }
    }

    pub fn from_id(id: &str) -> Option<Notify> {
        Notify::ALL.into_iter().find(|each| each.id() == id)
    }

    /// As Notify's pop-up says it.
    pub fn title(self) -> &'static str {
        match self {
            Notify::Away => NOTIFY_AWAY,
            Notify::Always => NOTIFY_ALWAYS,
            Notify::Never => NOTIFY_NEVER,
        }
    }
}

/// What shows the count and posts the notifications: the platform layer in the browser.
pub trait Host {
    /// `count` tasks wait for the user, or none at 0.
    fn badge(&self, count: usize);
    /// Whether ilo is the active app.
    fn active(&self) -> bool;
    /// Posts notification `id`, whose click comes back as `clicked(id)`.
    fn notify(&self, id: u64, title: &str, body: &str);
}

/// What a task's notification says of it, once a run each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Told {
    Done,
    NeedsYou,
}

/// The notifications posted in this run, a bound of them: a click on an older one opens nothing.
const KEPT: usize = 64;

struct NeedsYou {
    host: Rc<dyn Host>,
    count: usize,
    /// Each task's state as last followed, so a notification is for a state it came to, not one
    /// it was in when ilo first saw it, as a finished task kept from a run before.
    last: Vec<(SharedString, Dot)>,
    /// What each task was notified of: one notification a task for each, so a task going back and
    /// forth can't post again.
    told: HashSet<(SharedString, Told)>,
    /// Each notification posted, by its id, with the task it opens.
    posted: Vec<(u64, SharedString)>,
    next: u64,
    /// What the last `follow` found each task came to, and whether its notification was posted
    /// then: what a screen reader is told of in the app, and never beside a notification
    /// (`came`).
    came: Vec<(SharedString, Told, bool)>,
}

impl Global for NeedsYou {}

/// Starts following, with `host` to show the count and post the notifications. Without it,
/// `follow` does nothing, as in the preview.
pub fn open(host: Rc<dyn Host>, cx: &mut App) {
    cx.set_global(NeedsYou {
        host,
        count: 0,
        last: Vec::new(),
        told: HashSet::new(),
        posted: Vec::new(),
        next: 1,
        came: Vec::new(),
    });
}

/// The tasks' cards, as the windows are sent them: the count follows those that wait, and each
/// that has come to wait or to be done since posts its notification, as `when` says.
pub fn follow(cards: &[Card], when: Notify, cx: &mut App) {
    if !cx.has_global::<NeedsYou>() {
        return;
    }
    let state = cx.global_mut::<NeedsYou>();
    let host = state.host.clone();
    let waiting = (cards.iter())
        .filter(|card| card.dot == Dot::Waiting)
        .count();
    if waiting != state.count {
        state.count = waiting;
        // check.sh reads this line.
        crate::log!("browser: needs you {}", state.count);
        host.badge(state.count);
    }
    let was = |agent: &SharedString| {
        (state.last.iter())
            .find(|(each, _)| each == agent)
            .map(|(_, dot)| *dot)
    };
    // A need is new as a task comes to wait, a finish only as a task ilo saw under way ends.
    let came: Vec<(&Card, Told)> = (cards.iter())
        .filter_map(|card| match (card.dot, was(&card.agent)) {
            (Dot::Waiting, last) if last != Some(Dot::Waiting) => Some((card, Told::NeedsYou)),
            (Dot::Done, Some(last)) if last != Dot::Done => Some((card, Told::Done)),
            _ => None,
        })
        .collect();
    let fresh: Vec<(&Card, Told)> = (came.iter().copied())
        .filter(|(card, told)| !state.told.contains(&(card.agent.clone(), *told)))
        .collect();
    let active = !fresh.is_empty() && when == Notify::Away && host.active();
    let mut notified = Vec::new();
    for (card, told) in fresh {
        if active {
            // check.sh reads this line.
            crate::log!("browser: needs you {} while active", card.agent);
            continue;
        }
        if when == Notify::Never {
            crate::log!("browser: needs you {} not notified, as chosen", card.agent);
            continue;
        }
        state.told.insert((card.agent.clone(), told));
        let id = state.next;
        state.next += 1;
        state.posted.push((id, card.agent.clone()));
        if state.posted.len() > KEPT {
            state.posted.remove(0);
        }
        // check.sh reads this line, which says no more of the task than the activity log does.
        crate::log!("browser: needs you notified {id} {}", card.agent);
        host.notify(id, &posted(card, told), "");
        notified.push(&card.agent);
    }
    state.came = (came.iter())
        .map(|(card, told)| (card.agent.clone(), *told, notified.contains(&&card.agent)))
        .collect();
    // A task whose card has gone is done with, so what it was told goes with it.
    state
        .told
        .retain(|(agent, _)| cards.iter().any(|card| card.agent == *agent));
    state.last = (cards.iter())
        .map(|card| (card.agent.clone(), card.dot))
        .collect();
}

/// What the last `follow` found the task `agent` came to, to wait or to be done, and whether its
/// notification was posted then. Nothing is told of it in the app beside a notification.
pub fn came(agent: &str, cx: &App) -> Option<(Told, bool)> {
    let state = cx.try_global::<NeedsYou>()?;
    let found = state.came.iter().find(|(each, ..)| each == agent)?;
    Some((found.1, found.2))
}

/// Whether the last `follow` posted a notification of what the task `agent` came to.
pub fn notified(agent: &str, cx: &App) -> bool {
    came(agent, cx).is_some_and(|(_, notified)| notified)
}

/// The user clicked notification `id`: the task whose card it opens, if it was one of this run's.
pub fn clicked(id: u64, cx: &App) -> Option<SharedString> {
    let state = cx.try_global::<NeedsYou>()?;
    let agent = (state.posted.iter())
        .find(|(posted, _)| *posted == id)?
        .1
        .clone();
    // check.sh reads this line.
    crate::log!("browser: needs you clicked {id} {agent}");
    Some(agent)
}

/// A notification's words, which a screen reader is told where none is posted, and nothing more
/// of the task: a ⌘T task's title, the user's ask or its
/// assistant's name for it, on one line and capped; an outside agent's session's is the agent's
/// own words, which never stand where ilo speaks, so it's named by its agent, as its card is.
pub fn said(card: &Card, told: Told) -> String {
    words(card, &card.title, told)
}

/// The words of the notification the system is given. A thread with no title yet is titled by a
/// tab of the user's, a page's words, which can be a mail's subject, and the system keeps its
/// notifications: so there it reads NEW_THREAD, and the tab's title stays where it is drawn.
fn posted(card: &Card, told: Told) -> String {
    match card.unnamed {
        true => words(card, NEW_THREAD, told),
        false => said(card, told),
    }
}

/// `said`'s words for `card`, titled `title`.
fn words(card: &Card, title: &str, told: Told) -> String {
    let task = match card.replies {
        // Plain text, as an ilo line takes a page's, so no direction mark in a title can turn the
        // state's word around in the system's notification.
        true => clip(&crate::agents::tier::plain(title), TITLE_CHARS),
        false => card.name.to_string(),
    };
    // Y4: its title, or an outside agent's name, then its state's word, as its card says it.
    let word = match told {
        Told::Done => Word::Done,
        Told::NeedsYou => Word::NeedsYou,
    };
    format!("{task} · {}", word.after_name())
}

#[cfg(target_os = "linux")]
pub mod desktop;

#[cfg(test)]
mod tests;
