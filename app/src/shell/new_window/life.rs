//! A new window's life from ⌘N or ⇧⌘N to its command bar, as one state and the events that move it
//! (#2584). It waits for the window's first frame, then, from a full-screen window, for its full
//! screen and the desktop macOS moves it to. AppKit, the shell and macOS each say their part in no
//! fixed order: AppKit can take the window full screen as it opens (#2482), the shell can hear it
//! from the window's bounds or from the host, and the window can leave full screen on the way.
//! The shell keeps one `Life` a window and carries out the `Actions` each event returns.

/// Where a new window is on its way to its command bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Life {
    /// Not drawn on screen yet: GPUI draws a window some frames after it's put on screen, and a
    /// bar opened before would open over nothing.
    Unseen,
    /// Not drawn on screen yet, and from a full-screen window, so it goes full screen first.
    UnseenToGo,
    /// Drawn, and to go full screen, as the shell doesn't have it yet.
    ToGo,
    /// Asked for full screen, once, until the shell has it.
    Entering,
    /// Waiting for the window's desktop, out of full screen.
    Moving,
    /// Waiting for the window's desktop, full screen: macOS moves a window to a desktop of its own
    /// as its full screen begins, and a bar opened before is left on the one it came from.
    MovingThere,
    /// Its command bar has opened.
    Open,
}

/// What happened, from the window, the shell or the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Event {
    /// A frame is drawn with the window visible: whether the shell has it full screen, from the
    /// window's bounds or the host, and whether it's on the macOS desktop shown.
    Frame { full_screen: bool, on_space: bool },
    /// The host said the window went full screen or left it, which can come twice between frames.
    Heard { full_screen: bool },
    /// The answer to the ask for full screen: whether the window was already, as AppKit takes a
    /// window opened from a full-screen one there itself, and asking again would take it out.
    Asked { already: bool },
}

/// What the shell does for an event, in this order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Actions(u8);

impl Actions {
    pub const NONE: Self = Self(0);
    /// Logs the time from the key to the first frame, which check.sh reads.
    pub const FIRST_FRAME: Self = Self(1 << 0);
    /// Asks the window to go full screen, and steps `Asked` with the answer.
    pub const ASK: Self = Self(1 << 1);
    /// Takes the full screen the window had: the shell's layout and its note follow.
    pub const TAKE: Self = Self(1 << 2);
    /// Logs that it went full screen, which check.sh reads.
    pub const WENT: Self = Self(1 << 3);
    /// Opens ⌘T's command bar.
    pub const OPEN_BAR: Self = Self(1 << 4);
    /// Draws another frame, which steps `Frame` then.
    pub const NEXT_FRAME: Self = Self(1 << 5);
    /// Steps `Frame` again in this one, as what it reads may have changed.
    pub const AGAIN: Self = Self(1 << 6);

    pub fn has(self, action: Self) -> bool {
        self.0 & action.0 == action.0 && action.0 != 0
    }
}

impl std::ops::BitOr for Actions {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A window ⌘N opened, from a full-screen window if `full_screen`, and `drawn` if its first frame
/// came on screen with it, as on the Mac (#2141).
pub(crate) fn start(full_screen: bool, drawn: bool) -> Life {
    match (full_screen, drawn) {
        (false, false) => Life::Unseen,
        (true, false) => Life::UnseenToGo,
        (false, true) => Life::Moving,
        (true, true) => Life::ToGo,
    }
}

/// The life after `event`, and what the shell does for it.
pub(crate) fn step(life: Life, event: Event) -> (Life, Actions) {
    use Actions as A;
    use Event::*;
    use Life::*;
    match (life, event) {
        (Unseen, Frame { .. }) => (Moving, A::FIRST_FRAME | A::AGAIN),
        (UnseenToGo, Frame { .. }) => (ToGo, A::FIRST_FRAME | A::AGAIN),
        (Unseen | UnseenToGo, _) => (life, A::NONE),

        // The shell already has it, which it can have heard from the bounds before it drew, so
        // it never asks (#2482).
        (
            ToGo | Entering,
            Frame {
                full_screen: true, ..
            },
        ) => (MovingThere, A::AGAIN),
        (
            ToGo,
            Frame {
                full_screen: false, ..
            },
        ) => (Entering, A::ASK),
        (ToGo, _) => (ToGo, A::NONE),

        // AppKit took it there before the window could say so, and the shell takes it from here.
        (Entering, Asked { already: true }) => (MovingThere, A::TAKE | A::AGAIN),
        (Entering, Asked { already: false }) => (Entering, A::NONE),
        // The host says so, then a frame reads it.
        (Entering, Heard { full_screen: true }) => (Entering, A::NEXT_FRAME),
        // It went full screen and left before a frame read it: the bar opens out of full screen,
        // where the window is.
        (Entering, Heard { full_screen: false }) => (Moving, A::NEXT_FRAME),
        (
            Entering,
            Frame {
                full_screen: false, ..
            },
        ) => (Entering, A::NONE),

        // It left full screen on the way, and the bar opens where the window went.
        (
            MovingThere,
            Frame {
                full_screen: false, ..
            }
            | Heard { full_screen: false },
        ) => (Moving, A::NEXT_FRAME),
        (
            Moving | MovingThere,
            Frame {
                on_space: false, ..
            },
        ) => (life, A::NEXT_FRAME),
        (Moving, Frame { on_space: true, .. }) => (Open, A::OPEN_BAR),
        (
            MovingThere,
            Frame {
                full_screen: true,
                on_space: true,
            },
        ) => (Open, A::WENT | A::OPEN_BAR),
        (Moving | MovingThere, Heard { .. }) => (life, A::NEXT_FRAME),
        (Moving | MovingThere, Asked { .. }) => (life, A::NONE),

        (Open, _) => (Open, A::NONE),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, VecDeque};

    use super::*;

    /// A new window, the shell over it and macOS around it, as the shell sees them.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct World {
        life: Life,
        /// Whether the window ⌘N came from was full screen.
        from_full: bool,
        /// Whether GPUI has the window visible, which it draws only once it is.
        visible: bool,
        /// Whether the window is full screen or on its way, as AppKit has it at once.
        full: bool,
        /// Whether it was asked to go full screen and hasn't got there.
        pending: bool,
        /// Whether the shell has it full screen.
        heard: bool,
        /// The changes of full screen the host hasn't told the shell yet, which it tells each of,
        /// in order, once macOS's transition is over: the next is the opposite of the last told.
        owed: u8,
        told: bool,
        /// How many times AppKit or the user changed it, kept small so the worlds are few.
        changes: u8,
        /// Whether it's on the macOS desktop shown.
        on_space: bool,
        /// Whether AppKit or the user took it out of full screen.
        left: bool,
        frames_logged: u8,
        asks: u8,
        asked_while_heard: bool,
        went: u8,
        bars: u8,
        /// What held as the bar opened: drawn, on the desktop shown, full screen, the shell had it.
        opened: Option<(bool, bool, bool, bool)>,
    }

    impl World {
        /// Every way a window can open: from full screen or not, drawn as it came on screen or
        /// not, and taken full screen by AppKit as it opened or not.
        fn starts() -> Vec<World> {
            let mut starts = Vec::new();
            for from_full in [false, true] {
                for drawn in [false, true] {
                    for taken in [false, true] {
                        if taken && !from_full {
                            continue;
                        }
                        starts.push(World {
                            life: start(from_full, drawn),
                            from_full,
                            visible: drawn,
                            full: taken,
                            pending: false,
                            heard: false,
                            owed: taken as u8,
                            told: false,
                            changes: taken as u8,
                            on_space: !taken,
                            left: false,
                            frames_logged: drawn as u8,
                            asks: 0,
                            asked_while_heard: false,
                            went: 0,
                            bars: 0,
                            opened: None,
                        });
                    }
                }
            }
            starts
        }

        /// Steps `event` and carries out what it returns, as `step_opening` and `opening_heard`
        /// do.
        fn deliver(&mut self, event: Event) -> Actions {
            let (life, mut actions) = step(self.life, event);
            self.life = life;
            if actions.has(Actions::ASK) {
                self.asks += 1;
                self.asked_while_heard |= self.heard;
                let already = self.full;
                if !already {
                    self.pending = true;
                }
                let (life, more) = step(self.life, Event::Asked { already });
                assert!(!more.has(Actions::ASK), "it asked again in its answer");
                self.life = life;
                actions = actions | more;
            }
            if actions.has(Actions::FIRST_FRAME) {
                self.frames_logged += 1;
            }
            if actions.has(Actions::TAKE) {
                self.heard = true;
                self.deliver(Event::Heard { full_screen: true });
            }
            if actions.has(Actions::WENT) {
                self.went += 1;
            }
            if actions.has(Actions::OPEN_BAR) {
                self.bars += 1;
                let drawn = self.frames_logged > 0;
                self.opened = Some((drawn, self.on_space, self.full, self.heard));
            }
            if let Event::Heard { .. } = event {
                assert!(
                    (actions.0 & !Actions::NEXT_FRAME.0) == 0,
                    "{event:?} in {:?} did more than ask for a frame: {actions:?}",
                    self.life
                );
            }
            actions
        }

        /// A frame with the window visible, which the shell steps until it says no more.
        fn frame(&mut self) {
            for _ in 0..8 {
                let actions = self.deliver(Event::Frame {
                    full_screen: self.heard,
                    on_space: self.on_space,
                });
                if !actions.has(Actions::AGAIN) {
                    return;
                }
            }
            panic!("a frame steps without end: {self:?}");
        }

        fn goes_full(&mut self) {
            self.full = true;
            self.pending = false;
            self.on_space = false;
            self.owed += 1;
            self.changes += 1;
        }

        fn taken_out(&mut self) {
            self.full = false;
            self.pending = false;
            self.on_space = false;
            self.left = true;
            self.owed += 1;
            self.changes += 1;
        }

        /// The host tells the shell the next change it owes.
        fn tell(&mut self) {
            self.owed -= 1;
            self.told = !self.told;
            self.heard = self.told;
            self.deliver(Event::Heard {
                full_screen: self.told,
            });
        }

        /// Every next world, by any one thing GPUI, AppKit, macOS, the shell or the user can do
        /// now.
        fn next(&self) -> Vec<(&'static str, World)> {
            let mut next = Vec::new();
            let mut add = |what: &'static str, change: &dyn Fn(&mut World)| {
                let mut world = *self;
                change(&mut world);
                next.push((what, world));
            };
            if !self.visible {
                add("GPUI has it visible", &|w| w.visible = true);
            } else {
                add("a frame", &|w| w.frame());
            }
            if self.changes < 3 && !self.full {
                add("AppKit or the user takes it full screen", &|w| {
                    w.goes_full()
                });
            } else if self.changes < 3 {
                add("AppKit or the user takes it out", &|w| w.taken_out());
            }
            if !self.on_space {
                add("macOS shows its desktop", &|w| w.on_space = true);
            }
            if self.full && !self.heard {
                // The bounds say it's full screen before the host does, and step nothing.
                add("the shell hears from the bounds", &|w| w.heard = true);
            }
            if self.owed > 0 {
                add("the host tells the shell", &|w| w.tell());
            }
            next
        }

        /// What a new window promises, or the first promise broken.
        fn broken(&self) -> Option<String> {
            if self.bars > 1 {
                return Some(format!("its bar opened {} times", self.bars));
            }
            if self.asks > 1 {
                return Some(format!("it asked for full screen {} times", self.asks));
            }
            if self.asked_while_heard {
                return Some("it asked for a full screen the shell had, which takes it out".into());
            }
            if self.frames_logged > 1 {
                return Some(format!(
                    "its first frame was logged {} times",
                    self.frames_logged
                ));
            }
            if self.went > 1 {
                return Some(format!("it went full screen {} times", self.went));
            }
            if self.went == 1 && !self.from_full {
                return Some("it went full screen, not from a full-screen window".into());
            }
            let Some((drawn, on_space, full, heard)) = self.opened else {
                return None;
            };
            if !drawn {
                return Some("its bar opened before its first frame".into());
            }
            if !on_space {
                return Some("its bar opened before the window's desktop showed".into());
            }
            if self.from_full && !self.left && !(full && heard) {
                return Some(format!(
                    "from full screen, its bar opened with full screen {full}, the shell's {heard}"
                ));
            }
            if self.from_full && !self.left && self.went != 1 {
                return Some("from full screen, it didn't say it went full screen".into());
            }
            if self.went == 1 && !heard {
                return Some("it went full screen without the shell having it".into());
            }
            None
        }

        /// The world once nothing else happens: GPUI shows the window, the asked full screen
        /// comes, the host tells the shell what it owes, macOS shows its desktop, and frames are
        /// drawn.
        fn settled(mut self) -> World {
            for _ in 0..16 {
                if self.bars > 0 {
                    break;
                }
                if !self.visible {
                    self.visible = true;
                } else if self.pending {
                    self.goes_full();
                } else if self.owed > 0 {
                    self.tell();
                } else if !self.on_space {
                    self.on_space = true;
                } else {
                    self.frame();
                }
            }
            self
        }
    }

    /// Every world reachable from every way a window opens, by any order of events of any
    /// length, keeps every promise, and its bar opens once things settle. The path to the first
    /// that doesn't is the failure.
    #[test]
    fn every_order_of_events_opens_a_new_windows_bar_once_where_it_should() {
        let mut seen = HashSet::new();
        let mut lives = HashSet::new();
        let mut queue = VecDeque::new();
        for start in World::starts() {
            seen.insert(start);
            queue.push_back((start, vec!["opens"]));
        }
        while let Some((world, path)) = queue.pop_front() {
            lives.insert(world.life);
            let settled = world.settled();
            if let Some(broken) = settled.broken() {
                panic!("{broken}, as it settled after: {}", path.join(", "));
            }
            assert!(
                settled.bars == 1,
                "its bar never opened, after: {}, then settling: {settled:?}",
                path.join(", ")
            );
            for (what, next) in world.next() {
                if seen.insert(next) {
                    let mut path = path.clone();
                    path.push(what);
                    if let Some(broken) = next.broken() {
                        panic!("{broken}, after: {}", path.join(", "));
                    }
                    queue.push_back((next, path));
                }
            }
        }
        // Each life is reached, so none is left untested.
        for life in [
            Life::Unseen,
            Life::UnseenToGo,
            Life::ToGo,
            Life::Entering,
            Life::Moving,
            Life::MovingThere,
            Life::Open,
        ] {
            assert!(lives.contains(&life), "no order of events reaches {life:?}");
        }
    }
}
