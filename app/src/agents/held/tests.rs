//! Every order of the events around one tab in agent A's Space, with A and another agent B
//! connected. The world is the core's copy, the app's, the tab and A's calls.

use std::collections::{HashSet, VecDeque};

use super::*;

/// Whether the core lets go of an action the agent started when a hand_off holds the tab, or the
/// tab moves out of the agent's Space. It doesn't yet (#2601), so an action already started
/// finishes on a tab that's the user's. Set this once the core's change for #2601 lands, and the
/// walk then asserts no order lets one finish.
const CORE_LETS_GO: bool = false;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Place {
    /// In A's Space.
    Agents,
    /// Moved to one of the user's Spaces.
    Users,
    Closed,
}

/// A's call that waits on the tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Call {
    HandOff,
    Ask,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct World {
    tab: Place,
    /// The tab closed or moved, and the app hasn't heard yet.
    unheard: bool,
    /// The core: whether it holds the tab, and whether an action of A's it let through runs
    /// there.
    core_held: bool,
    acting: bool,
    /// The app.
    life: Life,
    /// The handoff's timer runs, as its held entry keeps it.
    timer: bool,
    /// A's call that waits on the tab, unanswered.
    waiting: Option<Call>,
}

impl World {
    fn start() -> World {
        World {
            tab: Place::Agents,
            unheard: false,
            core_held: false,
            acting: false,
            life: Life::Free,
            timer: false,
            waiting: None,
        }
    }

    /// The core's bc_agent_may_act for A.
    fn may_act(&self) -> bool {
        self.tab == Place::Agents && !self.core_held
    }

    /// Steps the app by `event` and carries out what it returns, as `Agents` does, checking each
    /// answer and log entry as it goes.
    fn app(&mut self, event: Event) -> Actions {
        let before = self.life;
        let (life, actions) = step(before, event);
        let answered = actions.has(Actions::ANSWER) || actions.has(Actions::DROP);
        if answered {
            assert!(
                self.waiting.take().is_some(),
                "{event:?} answered a call when none waited, from {before:?}"
            );
        }
        if before.waits() && !life.waits() {
            assert!(
                answered,
                "{event:?} ended a wait without answering its call"
            );
        }
        let left =
            before != Life::Free && (life == Life::Free || (before.waits() && !life.waits()));
        let noted_per_agent = matches!(event, Event::Stop { .. } | Event::Gone { .. });
        if left && !noted_per_agent {
            assert!(
                actions.has(Actions::NOTE) && !event.way().is_empty(),
                "{event:?} let the tab go from {before:?} without a log entry saying how"
            );
        }
        if actions.has(Actions::TAKE_BACK) {
            // bc_tab_take_back: the session ends, and with it what the agent was doing.
            self.core_held = true;
            self.acting = false;
        }
        if actions.has(Actions::HAND_BACK) {
            self.core_held = false;
        }
        if actions.has(Actions::TIMER) {
            self.timer = true;
        }
        if life == Life::Free {
            // The entry goes, with its timer.
            self.timer = false;
        }
        self.life = life;
        actions
    }

    /// Each event the world can take next, named.
    fn next(&self) -> Vec<(&'static str, World)> {
        let mut next = Vec::new();
        let mut go = |name: &'static str, change: &dyn Fn(&mut World)| {
            let mut world = self.clone();
            change(&mut world);
            next.push((name, world));
        };
        if self.may_act() && !self.acting {
            go("A acts", &|w| w.acting = true);
        }
        if self.acting {
            go("A's action finishes", &|w| {
                let users = w.core_held || w.tab != Place::Agents;
                assert!(
                    !users || !CORE_LETS_GO,
                    "an action of A's finished on a tab that's the user's: {w:?}"
                );
                w.acting = false;
            });
        }
        if self.may_act() {
            go("A hands off", &|w| {
                // bc_agent_hand_off holds it, and today lets what runs there go on.
                w.core_held = true;
                if CORE_LETS_GO {
                    w.acting = false;
                }
                // A hold the app has already answers the call at once, as the tab is the user's.
                if w.app(Event::HandOff).has(Actions::TIMER) {
                    w.waiting = Some(Call::HandOff);
                }
            });
        }
        if self.tab == Place::Agents && !self.unheard {
            go("the user takes it over", &|w| _ = w.app(Event::TakeOver));
        }
        if self.may_act() {
            go("the page asks for a passkey", &|w| {
                w.core_held = true;
                w.acting = false;
                w.app(Event::Passkey);
            });
        }
        if self.tab == Place::Agents {
            go("A asks for it back", &|w| {
                let actions = w.app(Event::Ask { holder: true });
                let refused = actions.has(Actions::NOT_HELD) || actions.has(Actions::WAITING);
                if !refused {
                    assert_eq!(w.life, Life::Asked, "an ask neither refused nor waiting");
                    w.waiting = Some(Call::Ask);
                }
            });
        }
        go("B asks for it", &|w| {
            let before = w.clone();
            let actions = w.app(Event::Ask { holder: false });
            assert!(actions.has(Actions::NOT_HELD), "B's ask wasn't refused");
            assert_eq!(*w, before, "B's ask changed the tab");
        });
        if self.life != Life::Free {
            go("the user hands it back", &|w| _ = w.app(Event::HandBack));
        }
        if self.life == Life::Asked {
            go("the user keeps it", &|w| _ = w.app(Event::Keep));
        }
        if self.timer {
            go("the handoff's 15 minutes pass", &|w| {
                w.timer = false;
                let handed_off = w.life == Life::HandedOff;
                w.app(Event::Timer);
                assert!(
                    !handed_off || w.waiting.is_none(),
                    "the 15 minutes passed and the handoff still waits"
                );
            });
        }
        go("the user stops A", &|w| {
            w.app(Event::Stop { holder: true });
            // bc_agent_disconnected ends A's sessions.
            w.acting = false;
        });
        go("A goes", &|w| {
            w.app(Event::Gone { holder: true });
            w.acting = false;
        });
        for (name, event) in [
            ("the user stops B", Event::Stop { holder: false }),
            ("B goes", Event::Gone { holder: false }),
        ] {
            go(name, &|w| {
                let before = w.clone();
                w.app(event);
                assert_eq!(*w, before, "{event:?} for B changed A's tab");
            });
        }
        if self.tab == Place::Agents {
            go("the tab closes", &|w| {
                (w.tab, w.unheard, w.acting) = (Place::Closed, true, false);
            });
            go("the user moves it to their Space", &|w| {
                (w.tab, w.unheard) = (Place::Users, true);
                if CORE_LETS_GO {
                    w.acting = false;
                }
            });
        }
        if self.tab != Place::Agents {
            // The app hears once from space_changed and once more from tab_closed, in either
            // order: the second is the same event again.
            go("the app hears where it went", &|w| {
                w.unheard = false;
                w.app(match w.tab {
                    Place::Closed => Event::Closed,
                    _ => Event::Moved,
                });
            });
        }
        next
    }

    /// What must hold in every world.
    fn check(&self) {
        assert_eq!(
            self.waiting.is_some(),
            self.life.waits(),
            "a call waits exactly while the app's hold says one does"
        );
        if self.waiting == Some(Call::HandOff) {
            assert_eq!(
                self.life,
                Life::HandedOff,
                "a handoff's call waits as a handoff"
            );
        }
        if self.unheard {
            return;
        }
        match self.tab {
            Place::Agents => assert_eq!(
                self.life != Life::Free,
                self.core_held,
                "the app and the core disagree on whether the user holds A's tab"
            ),
            Place::Closed | Place::Users => {
                assert_eq!(
                    self.life,
                    Life::Free,
                    "a hold outlived its tab in A's Space"
                );
            }
        }
        if self.tab == Place::Closed {
            assert!(!self.core_held, "the core holds a tab that closed");
        }
    }

    /// The world once things settle: the app hears, A's action finishes, the timer fires, and
    /// the user answers and hands back what they hold.
    fn settled(&self) -> World {
        let mut world = self.clone();
        if world.unheard {
            world = world.after("the app hears where it went");
        }
        if world.acting {
            world = world.after("A's action finishes");
        }
        if world.timer {
            world = world.after("the handoff's 15 minutes pass");
        }
        if world.life != Life::Free {
            world = world.after("the user hands it back");
        }
        world
    }

    fn after(&self, name: &str) -> World {
        let next = self.next().into_iter().find(|(each, _)| *each == name);
        next.unwrap_or_else(|| panic!("{name} can't happen in {self:?}"))
            .1
    }
}

#[test]
fn every_order_of_events_around_an_agents_tab_answers_each_call_once_and_keeps_both_copies_agreed()
{
    let mut seen = HashSet::from([World::start()]);
    let mut queue = VecDeque::from([World::start()]);
    let mut lives = HashSet::new();
    let mut owed = false;
    while let Some(world) = queue.pop_front() {
        world.check();
        lives.insert(world.life);
        let settled = world.settled();
        settled.check();
        assert_eq!(
            (settled.life, settled.waiting, settled.timer),
            (Life::Free, None, false),
            "from {world:?}, the tab didn't settle back to A with every call answered"
        );
        for (name, next) in world.next() {
            if name == "A's action finishes" && (world.core_held || world.tab != Place::Agents) {
                owed = true;
            }
            if seen.insert(next.clone()) {
                queue.push_back(next);
            }
        }
    }
    let all = [Life::Free, Life::Held, Life::HandedOff, Life::Asked];
    assert!(
        all.iter().all(|life| lives.contains(life)),
        "reached only {lives:?}"
    );
    // #2601's order: A's action started, then a hand_off or a move, then the action finishes on
    // the user's tab. The walk reaches it until the core lets go there (CORE_LETS_GO).
    assert_eq!(
        owed, !CORE_LETS_GO,
        "#2601's order and CORE_LETS_GO disagree"
    );
}
