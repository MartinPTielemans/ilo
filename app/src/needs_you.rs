//! Quiet until it needs you, outside ilo (docs/ux-spec.md › Agent task, Needs you): the Dock's
//! badge, the taskbar button's overlay or the window's urgency says how many tasks wait for the
//! user, ⌘T's and outside agents' alike, and each new need, if ilo isn't the active app as it
//! comes, posts one system notification, whose click opens its task's card. Nothing is kept of
//! them past the run, and nothing else interrupts: no sound, no bounce, no flash.

use std::rc::Rc;

use gpui::{App, Global, SharedString};

use crate::agents::task::{Card, Dot};

/// What shows the count and posts the notifications: the platform layer in the browser.
pub trait Host {
    /// `count` tasks wait for the user, or none at 0.
    fn badge(&self, count: usize);
    /// Whether ilo is the active app.
    fn active(&self) -> bool;
    /// Posts notification `id`, whose click comes back as `clicked(id)`.
    fn notify(&self, id: u64, title: &str, body: &str);
}

/// A need, as one task waits for the user: its agent, and the question it waits on, by its id or,
/// for one the agent asks in its conversation, its words.
#[derive(Clone, Debug, PartialEq)]
struct Need {
    agent: SharedString,
    question: Option<u64>,
    words: SharedString,
}

/// The notifications posted in this run, a bound of them: a click on an older one opens nothing.
const KEPT: usize = 64;

struct NeedsYou {
    host: Rc<dyn Host>,
    waiting: Vec<Need>,
    count: usize,
    /// Each notification posted, by its id, with the task it opens.
    posted: Vec<(u64, SharedString)>,
    next: u64,
}

impl Global for NeedsYou {}

/// Starts following, with `host` to show the count and post the notifications. Without it,
/// `follow` does nothing, as in the preview.
pub fn open(host: Rc<dyn Host>, cx: &mut App) {
    cx.set_global(NeedsYou {
        host,
        waiting: Vec::new(),
        count: 0,
        posted: Vec::new(),
        next: 1,
    });
}

/// The tasks' cards, as the windows are sent them: the count follows those that wait, and each
/// that has come to wait since posts its notification, if ilo isn't the active app.
pub fn follow(cards: &[Card], cx: &mut App) {
    if !cx.has_global::<NeedsYou>() {
        return;
    }
    let needs: Vec<(Need, &Card)> = (cards.iter())
        .filter(|card| card.dot == Dot::Waiting)
        .map(|card| (need(card), card))
        .collect();
    let state = cx.global_mut::<NeedsYou>();
    let host = state.host.clone();
    if needs.len() != state.count {
        state.count = needs.len();
        // check.sh reads this line.
        crate::log!("browser: needs you {}", state.count);
        host.badge(state.count);
    }
    let fresh: Vec<&(Need, &Card)> = (needs.iter())
        .filter(|(need, _)| !state.waiting.contains(need))
        .collect();
    let active = !fresh.is_empty() && host.active();
    for (_, card) in fresh {
        if active {
            // check.sh reads this line.
            crate::log!("browser: needs you {} while active", card.agent);
            continue;
        }
        let id = state.next;
        state.next += 1;
        state.posted.push((id, card.agent.clone()));
        if state.posted.len() > KEPT {
            state.posted.remove(0);
        }
        let (title, body) = (format!("{} needs you", card.name), body(card));
        // check.sh reads this line, which says no more of the need than the activity log does.
        crate::log!("browser: needs you notified {id} {}", card.agent);
        host.notify(id, &title, &body);
    }
    state.waiting = needs.into_iter().map(|(need, _)| need).collect();
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

fn need(card: &Card) -> Need {
    Need {
        agent: card.agent.clone(),
        question: card.question.as_ref().map(|question| question.id),
        words: card.status.clone(),
    }
}

/// The step the task waits on, as the notification's body: what a handoff asks the user to do,
/// such as "Sign in to github.com", what a confirmation would do, or the agent's question.
fn body(card: &Card) -> String {
    let Some(question) = &card.question else {
        return card.status.to_string();
    };
    match question.handoff {
        Some(_) => question.why.to_string(),
        None => {
            let mut what = question.what.chars();
            what.next()
                .map(|first| first.to_uppercase().chain(what).collect())
                .unwrap_or_default()
        }
    }
}

#[cfg(target_os = "linux")]
pub mod desktop;

#[cfg(test)]
mod tests;
