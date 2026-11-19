//! A tab the user holds from its agent, from the hold until the tab is the agent's again or gone,
//! as one state machine with no GPUI in it (#2584). `Agents` keeps each held tab's data and
//! carries out what `step` returns. The core keeps a set of held tabs of its own, which refuses
//! the agent by itself (bc_agent_may_act), so the browser enforces the hold even if the app is
//! wrong. The actions here name each core call that keeps it following this.

use std::ops::BitOr;

/// Where a tab is between the user and its agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Life {
    /// Nobody holds it.
    Free,
    /// The user holds it, and no call of the agent's waits on it. They took it over, the page
    /// asked for their passkey, or the agent's wait was answered.
    Held,
    /// The agent handed it to the user, and its hand_off call waits for Hand Back or
    /// HANDOFF_WAIT.
    HandedOff,
    /// The user holds it, and the agent's ask_for_tab call waits for Hand Back or Keep.
    Asked,
}

impl Life {
    /// Whether a call of the agent's waits on the tab.
    pub(super) fn waits(self) -> bool {
        matches!(self, Life::HandedOff | Life::Asked)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Event {
    /// The agent handed its tab to the user (hand_off), and the core holds it.
    HandOff,
    /// The user took the tab over, by a press in its page or by taking all of the agent's tabs.
    TakeOver,
    /// The page asked for the user's passkey, and the core holds the tab for it.
    Passkey,
    /// An agent asked for the tab back (ask_for_tab). `holder` is whether it's the agent the tab
    /// is held from.
    Ask { holder: bool },
    /// The user handed the tab back.
    HandBack,
    /// The user answered an agent's ask with Keep.
    Keep,
    /// HANDOFF_WAIT passed since the handoff.
    Timer,
    /// The user stopped an agent. `holder` is whether it's the agent the tab is held from.
    Stop { holder: bool },
    /// An agent went: its client disconnected or was disconnected, or the agents were switched
    /// off. `holder` is whether it's the agent the tab is held from.
    Gone { holder: bool },
    /// The tab closed.
    Closed,
    /// The tab left the agent's Space for one of the user's.
    Moved,
}

impl Event {
    /// The activity log's word for how the hold, or the agent's wait on it, ended by this event.
    pub(super) fn way(self) -> &'static str {
        match self {
            Event::HandBack => "handed back",
            Event::Keep => "kept",
            Event::Timer => "not handed back",
            Event::Closed => "closed",
            Event::Moved => "moved",
            // The agent's own entry says these, once for all its tabs: stopped, or disconnected.
            Event::Stop { .. } | Event::Gone { .. } => "",
            Event::HandOff | Event::TakeOver | Event::Passkey | Event::Ask { .. } => "",
        }
    }
}

/// What the host does, after it has the life `step` returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) struct Actions(u16);

impl Actions {
    pub(super) const NONE: Self = Self(0);
    /// Starts the timer that answers the handoff once HANDOFF_WAIT passes.
    pub(super) const TIMER: Self = Self(1);
    /// Asks the core to take the tab back (bc_tab_take_back): it holds the tab and ends the
    /// agent's session there, which stops what the agent was doing in it.
    pub(super) const TAKE_BACK: Self = Self(1 << 1);
    /// Tells the core the tab isn't held any more (bc_tab_hand_back).
    pub(super) const HAND_BACK: Self = Self(1 << 2);
    /// Answers the call that waited on the tab, with the event's way.
    pub(super) const ANSWER: Self = Self(1 << 3);
    /// Lets go of the call that waited, unanswered: the agent that made it is gone, and so is
    /// its connection.
    pub(super) const DROP: Self = Self(1 << 4);
    /// Refuses the agent's ask: the tab isn't held from it.
    pub(super) const NOT_HELD: Self = Self(1 << 5);
    /// Refuses the agent's ask: it waits on the user for this tab already.
    pub(super) const WAITING: Self = Self(1 << 6);
    /// Notes the event's way in the agent's activity log.
    pub(super) const NOTE: Self = Self(1 << 7);
    /// Shows the question asked now again, as the tab's question came or went.
    pub(super) const CARD: Self = Self(1 << 8);

    pub(super) fn has(self, actions: Actions) -> bool {
        self.0 & actions.0 == actions.0 && actions.0 != 0
    }
}

impl BitOr for Actions {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// The life after `event`, and what the host does.
pub(super) fn step(life: Life, event: Event) -> (Life, Actions) {
    use Event::*;
    use Life::*;
    type A = Actions;
    // A call that waited gets its one answer as the hold, or the wait, ends.
    let answer = if life.waits() { A::ANSWER } else { A::NONE };
    match (life, event) {
        (Free, HandOff) => (HandedOff, A::TIMER | A::CARD),
        (Free, TakeOver) => (Held, A::TAKE_BACK),
        (Free, Passkey) => (Held, A::NONE),
        // Held already: the core refuses a second hold, and the first stays as it is.
        (_, HandOff | TakeOver | Passkey) => (life, A::NONE),
        (Free, Ask { .. }) | (_, Ask { holder: false }) => (life, A::NOT_HELD),
        (Held, Ask { holder: true }) => (Asked, A::CARD),
        (HandedOff | Asked, Ask { holder: true }) => (life, A::WAITING),
        // The core may hold what the app doesn't, and Hand Back lets go of it there too.
        (Free, HandBack) => (Free, A::HAND_BACK),
        (_, HandBack) => (Free, A::HAND_BACK | answer | A::NOTE | A::CARD),
        (Asked, Keep) => (Held, A::ANSWER | A::NOTE | A::CARD),
        (HandedOff, Timer) => (Held, A::ANSWER | A::NOTE | A::CARD),
        (HandedOff | Asked, Stop { holder: true }) => (Held, A::ANSWER | A::CARD),
        (HandedOff | Asked, Gone { holder: true }) => (Held, A::DROP | A::CARD),
        (Free, Closed | Moved) => (Free, A::NONE),
        // A closed tab's id leaves the core's set too.
        (_, Closed) => (Free, A::HAND_BACK | answer | A::NOTE | A::CARD),
        // The core keeps holding a tab that moved: bc_tab_hand_back would take the focus from
        // its page, which is the user's now, and the core refuses agents a tab outside their
        // Space anyway.
        (_, Moved) => (Free, answer | A::NOTE | A::CARD),
        (_, Keep | Timer | Stop { .. } | Gone { .. }) => (life, A::NONE),
    }
}

#[cfg(test)]
mod tests;
