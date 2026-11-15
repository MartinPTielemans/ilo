//! An overlay's life, from hidden to shown and back, as one state and the events that move it
//! (#2584). An overlay is a window kept hidden and shown again over a browser window, and four bugs
//! in one week were a hidden window's state gone stale, each an order of events no one wrote down:
//! the pointer's capture after a menu (#2516), the drawing device after an adapter restart (#2536),
//! the scale it was told while hidden (#2541), and the keyboard Windows gave a hidden one (#2545).
//! The host keeps one `Life` an overlay and carries out the `Actions` each event returns: Windows'
//! (platform/windows/host.cc) through `browser_overlay_step`.

/// Where an overlay is in its life.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Life {
    /// The UI has it hidden, and it's off screen.
    Hidden = 0,
    /// The UI shows it, but it's off screen with its minimized browser window, until that comes
    /// back.
    Placed = 1,
    /// On screen, without the keyboard.
    Shown = 2,
    /// On screen, with the keyboard.
    Key = 3,
    /// The UI has hidden it, and it fades out on screen.
    Hiding = 4,
}

/// What happened to an overlay, from the UI, the user or the OS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// The UI shows it, with the keyboard if `keyboard`. Over a browser window that's `minimized`
    /// it would stand alone over nothing. `later`, it goes on screen at a later `Show`, once the UI
    /// has drawn it as it is now. Again while shown, it's placed again.
    Show {
        keyboard: bool,
        minimized: bool,
        later: bool,
    },
    /// The UI hides it, fading out if `fade`.
    Hide { fade: bool },
    /// Its fade out ended.
    Faded,
    /// A press went to it, which takes the pointer's capture until its release.
    Pressed,
    /// A press went to another of the app's windows while it was on screen.
    ClickedOutside,
    /// Its browser window's scale changed.
    ScaleChanged,
    /// The drawing device was lost, and a new one made.
    DeviceLost,
    /// Its browser window was minimized, and the OS took it off screen with it. Only one on screen
    /// hears it.
    Minimized,
    /// Its browser window came back, and the OS puts it back on screen. Only one it took off hears
    /// it.
    Restored,
    /// It became the active window, by the user or by the OS.
    Activated,
    /// It stopped being the active window.
    Deactivated,
}

/// What the host does for an event, in this order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Actions(pub u16);

impl Actions {
    pub const NONE: Self = Self(0);
    /// A press held in another window lets go of the pointer, which this one takes with the
    /// keyboard: its release may never reach that window, as the menu's hook takes it (#2516).
    pub const TAKE_POINTER: Self = Self(1 << 0);
    /// Moved and sized over its browser window.
    pub const PLACE: Self = Self(1 << 1);
    /// Told its browser window's scale, which one off screen may not have heard (#2541).
    pub const TELL_SCALE: Self = Self(1 << 2);
    /// Put on screen, which draws it.
    pub const ON_SCREEN: Self = Self(1 << 3);
    /// Given the keyboard.
    pub const TAKE_KEYBOARD: Self = Self(1 << 4);
    /// Lets go of the pointer if it holds it, so its moves don't go where nothing shows.
    pub const LET_GO: Self = Self(1 << 5);
    /// Taken off screen.
    pub const OFF_SCREEN: Self = Self(1 << 6);
    /// Gives the keyboard, if it has it, back to what had it before (#2545).
    pub const GIVE_BACK: Self = Self(1 << 7);
    /// Stays off screen as the OS puts it back with its browser window (#1346).
    pub const KEEP_OFF: Self = Self(1 << 8);
    /// Draws again, on the new device (#2536).
    pub const DRAW: Self = Self(1 << 9);
    /// Tells the UI the user left it, or that it can't show, which hides it.
    pub const DISMISS: Self = Self(1 << 10);

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

/// The life after `event`, and what the host does for it.
pub fn step(life: Life, event: Event) -> (Life, Actions) {
    use Actions as A;
    use Event::*;
    use Life::*;
    let on_screen = matches!(life, Shown | Key | Hiding);
    match (life, event) {
        // One the OS took off with its window comes back with it.
        (
            Placed,
            Show {
                minimized: true, ..
            },
        ) => (Placed, A::NONE),
        // Alone on screen it would stand where the window was, as the menu and the command bar
        // never have (#1346).
        (
            Hidden | Shown | Key | Hiding,
            Show {
                minimized: true, ..
            },
        ) => (
            Hidden,
            A::LET_GO | A::GIVE_BACK | A::OFF_SCREEN | A::DISMISS,
        ),
        (Hidden | Placed, Show { later: true, .. }) => (Placed, A::NONE),
        (Shown | Key | Hiding, Show { later: true, .. }) => (life, A::NONE),
        (Key, Show { .. }) => (Key, A::PLACE | A::TELL_SCALE),
        (_, Show { keyboard: true, .. }) => (
            Key,
            A::TAKE_POINTER | A::PLACE | A::TELL_SCALE | A::ON_SCREEN | A::TAKE_KEYBOARD,
        ),
        (
            _,
            Show {
                keyboard: false, ..
            },
        ) => (Shown, A::PLACE | A::TELL_SCALE | A::ON_SCREEN),

        (Hidden | Placed, Hide { .. }) => (Hidden, A::NONE),
        (Hiding, Hide { fade: true }) => (Hiding, A::NONE),
        (Shown | Key, Hide { fade: true }) => (Hiding, A::LET_GO | A::GIVE_BACK),
        (Shown | Key | Hiding, Hide { fade: false }) | (Hiding, Faded) => {
            (Hidden, A::LET_GO | A::GIVE_BACK | A::OFF_SCREEN)
        }
        (_, Faded) => (life, A::NONE),

        // The OS sends a press only to a window on screen, which GPUI captures the pointer for.
        (_, Pressed) => (life, A::NONE),
        (Shown | Key, ClickedOutside) => (life, A::DISMISS),
        (_, ClickedOutside) => (life, A::NONE),

        // One off screen is told as it's shown.
        (_, ScaleChanged) if on_screen => (life, A::TELL_SCALE),
        (_, ScaleChanged) => (life, A::NONE),
        // One off screen draws as it's shown.
        (_, DeviceLost) if on_screen => (life, A::DRAW),
        (_, DeviceLost) => (life, A::NONE),

        // Off screen with its window, it holds nothing a press gave it.
        (Shown | Key, Minimized) => (Placed, A::LET_GO),
        // Faded out off screen.
        (Hiding, Minimized) => (Hidden, A::LET_GO),
        (_, Minimized) => (life, A::NONE),
        (Placed, Restored) => (Shown, A::TELL_SCALE),
        // The UI hid it meanwhile.
        (Hidden, Restored) => (Hidden, A::KEEP_OFF),
        (_, Restored) => (life, A::NONE),

        // With the keyboard it takes the pointer, however it came to have it.
        (Shown, Activated) => (Key, A::TAKE_POINTER),
        (Key, Activated) => (Key, A::NONE),
        // The UI doesn't show it: what Windows made active after Win+D (#2545).
        (Hidden | Placed | Hiding, Activated) => (life, A::GIVE_BACK),
        // Whether the user left it is the host's to say: a menu over it or its own browser window
        // taking the keyboard isn't the user leaving it.
        (Key, Deactivated) => (Shown, A::NONE),
        (_, Deactivated) => (life, A::NONE),
    }
}

/// `Show`'s and `Hide`'s flags, as `browser_overlay_step` takes them.
pub const KEYBOARD: u8 = 1 << 0;
pub const MINIMIZED: u8 = 1 << 1;
/// `Show`'s `later`, or `Hide`'s `fade`.
pub const LATER: u8 = 1 << 2;

/// `step` for the Windows host (platform/windows/host.cc): `life` and `event` as `Life` and the
/// order of `Event` number them, with `flags` for `Show` and `Hide`, to the next life in the low
/// byte and the actions above it.
#[unsafe(no_mangle)]
pub extern "C" fn browser_overlay_step(life: u8, event: u8, flags: u8) -> u32 {
    let life = match life {
        1 => Life::Placed,
        2 => Life::Shown,
        3 => Life::Key,
        4 => Life::Hiding,
        _ => Life::Hidden,
    };
    let event = match event {
        0 => Event::Show {
            keyboard: flags & KEYBOARD != 0,
            minimized: flags & MINIMIZED != 0,
            later: flags & LATER != 0,
        },
        1 => Event::Hide {
            fade: flags & LATER != 0,
        },
        2 => Event::Faded,
        3 => Event::Pressed,
        4 => Event::ClickedOutside,
        5 => Event::ScaleChanged,
        6 => Event::DeviceLost,
        7 => Event::Minimized,
        8 => Event::Restored,
        9 => Event::Activated,
        _ => Event::Deactivated,
    };
    let (life, actions) = step(life, event);
    life as u32 | (actions.0 as u32) << 8
}

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, VecDeque};

    use super::*;

    /// Who has the keyboard or the pointer's capture.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Holder {
        Nobody,
        Browser,
        OtherApp,
        Overlay(usize),
    }

    const OVERLAYS: usize = 2;

    /// A browser window, two overlays over it and the OS around them, as the host sees it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct World {
        life: [Life; OVERLAYS],
        on_screen: [bool; OVERLAYS],
        /// Whether each has the scale its browser window has now.
        scaled: [bool; OVERLAYS],
        /// Whether each last drew on the device there is now.
        drawn: [bool; OVERLAYS],
        /// Those the OS took off screen with the minimized window, which it puts back.
        taken_off: [bool; OVERLAYS],
        minimized: bool,
        keyboard: Holder,
        capture: Holder,
    }

    impl World {
        fn new() -> Self {
            World {
                life: [Life::Hidden; OVERLAYS],
                on_screen: [false; OVERLAYS],
                scaled: [true; OVERLAYS],
                drawn: [true; OVERLAYS],
                taken_off: [false; OVERLAYS],
                minimized: false,
                keyboard: Holder::Browser,
                capture: Holder::Nobody,
            }
        }

        /// Steps overlay `i` with `event` and carries out what it returns, as the host does.
        fn deliver(&mut self, i: usize, event: Event, depth: usize) {
            let (life, actions) = step(self.life[i], event);
            self.life[i] = life;
            if actions.has(Actions::TAKE_POINTER)
                && !matches!(self.capture, Holder::Overlay(at) if at == i)
            {
                self.capture = Holder::Nobody;
            }
            if actions.has(Actions::TELL_SCALE) {
                self.scaled[i] = true;
            }
            if actions.has(Actions::ON_SCREEN) {
                self.on_screen[i] = true;
                self.drawn[i] = true;
            }
            if actions.has(Actions::DRAW) {
                self.drawn[i] = true;
            }
            if actions.has(Actions::TAKE_KEYBOARD) {
                self.give_keyboard(Holder::Overlay(i), depth);
            }
            if actions.has(Actions::LET_GO) && self.capture == Holder::Overlay(i) {
                self.capture = Holder::Nobody;
            }
            if actions.has(Actions::OFF_SCREEN) {
                self.on_screen[i] = false;
            }
            if actions.has(Actions::GIVE_BACK) && self.keyboard == Holder::Overlay(i) {
                // To its browser window, unless that's minimized.
                if !self.minimized {
                    self.give_keyboard(Holder::Browser, depth);
                }
            }
        }

        /// The OS moves the keyboard to `to`, telling the overlay that had it and the one that
        /// has it, whose answers may move it again.
        fn give_keyboard(&mut self, to: Holder, depth: usize) {
            let from = self.keyboard;
            if from == to {
                return;
            }
            assert!(depth < 8, "the keyboard moves without end: {self:?}");
            self.keyboard = to;
            if let Holder::Overlay(i) = from {
                self.deliver(i, Event::Deactivated, depth + 1);
            }
            if let Holder::Overlay(i) = to
                && self.keyboard == to
            {
                self.deliver(i, Event::Activated, depth + 1);
            }
        }

        /// Every next world, by any one thing the UI, the user or the OS can do now.
        fn next(&self) -> Vec<(String, World)> {
            let mut next = Vec::new();
            let mut add = |what: String, change: &dyn Fn(&mut World)| {
                let mut world = *self;
                change(&mut world);
                next.push((what, world));
            };
            for i in 0..OVERLAYS {
                for keyboard in [false, true] {
                    for later in [false, true] {
                        add(
                            format!("ui shows {i}, keyboard {keyboard}, later {later}"),
                            &|w| {
                                let minimized = w.minimized;
                                // Without the keyboard while its window is minimized, which is never
                                // in front.
                                let keyboard = keyboard && !minimized;
                                w.deliver(
                                    i,
                                    Event::Show {
                                        keyboard,
                                        minimized,
                                        later,
                                    },
                                    0,
                                )
                            },
                        );
                    }
                }
                for fade in [false, true] {
                    add(format!("ui hides {i}, fade {fade}"), &|w| {
                        w.deliver(i, Event::Hide { fade }, 0)
                    });
                }
                add(format!("{i}'s fade ends"), &|w| {
                    w.deliver(i, Event::Faded, 0)
                });
                if self.on_screen[i] {
                    add(format!("press on {i}"), &|w| {
                        w.capture = Holder::Overlay(i);
                        w.deliver(i, Event::Pressed, 0);
                        w.give_keyboard(Holder::Overlay(i), 0);
                        let others: Vec<_> = (0..OVERLAYS)
                            .filter(|&o| o != i && w.on_screen[o])
                            .collect();
                        for other in others {
                            w.deliver(other, Event::ClickedOutside, 0);
                        }
                    });
                }
                // Windows activates a window the browser window owns as that comes back, hidden
                // or not (#2545), and the user one on screen.
                if !self.minimized {
                    add(format!("the OS activates {i}"), &|w| {
                        w.give_keyboard(Holder::Overlay(i), 0)
                    });
                }
            }
            if !self.minimized {
                add("press in the browser window".into(), &|w| {
                    w.capture = Holder::Browser;
                    let shown: Vec<_> = (0..OVERLAYS).filter(|&i| w.on_screen[i]).collect();
                    for i in shown {
                        w.deliver(i, Event::ClickedOutside, 0);
                    }
                    w.give_keyboard(Holder::Browser, 0);
                });
            }
            add("release".into(), &|w| w.capture = Holder::Nobody);
            add("another app takes the keyboard".into(), &|w| {
                w.give_keyboard(Holder::OtherApp, 0)
            });
            add("scale changes".into(), &|w| {
                // Told no window: the OS tells one on screen, but the model doesn't count on it.
                w.scaled = [false; OVERLAYS];
                for i in 0..OVERLAYS {
                    w.deliver(i, Event::ScaleChanged, 0);
                }
            });
            add("device lost".into(), &|w| {
                w.drawn = [false; OVERLAYS];
                for i in 0..OVERLAYS {
                    w.deliver(i, Event::DeviceLost, 0);
                }
            });
            if self.minimized {
                add("restored".into(), &|w| {
                    w.minimized = false;
                    // Windows tells only those it took off, and puts back those it isn't told to
                    // keep off.
                    let taken_off: Vec<_> = (0..OVERLAYS).filter(|&i| w.taken_off[i]).collect();
                    for i in taken_off {
                        let (_, actions) = step(w.life[i], Event::Restored);
                        w.taken_off[i] = false;
                        w.deliver(i, Event::Restored, 0);
                        if !actions.has(Actions::KEEP_OFF) {
                            w.on_screen[i] = true;
                            w.drawn[i] = true;
                        }
                    }
                    w.give_keyboard(Holder::Browser, 0);
                });
            } else {
                add("minimized".into(), &|w| {
                    w.minimized = true;
                    w.give_keyboard(Holder::OtherApp, 0);
                    // Windows takes off and tells only those on screen.
                    for i in 0..OVERLAYS {
                        w.taken_off[i] = w.on_screen[i];
                        w.on_screen[i] = false;
                    }
                    let taken_off: Vec<_> = (0..OVERLAYS).filter(|&i| w.taken_off[i]).collect();
                    for i in taken_off {
                        w.deliver(i, Event::Minimized, 0);
                    }
                });
            }
            next
        }

        /// What an overlay's life promises, or the first promise broken.
        fn broken(&self) -> Option<String> {
            let keys = (0..OVERLAYS).filter(|&i| self.life[i] == Life::Key).count();
            if keys > 1 {
                return Some(format!("{keys} overlays have the keyboard"));
            }
            for i in 0..OVERLAYS {
                let life = self.life[i];
                let shows = matches!(life, Life::Shown | Life::Key | Life::Hiding);
                let off = matches!(life, Life::Hidden | Life::Placed);
                if off && self.keyboard == Holder::Overlay(i) {
                    return Some(format!("{i} is {life:?} and has the keyboard"));
                }
                if off && self.capture == Holder::Overlay(i) {
                    return Some(format!("{i} is {life:?} and has the pointer's capture"));
                }
                if (life == Life::Key) != (self.keyboard == Holder::Overlay(i)) {
                    return Some(format!(
                        "{i} is {life:?}, the keyboard is {:?}'s",
                        self.keyboard
                    ));
                }
                if life == Life::Key
                    && !matches!(self.capture, Holder::Nobody)
                    && self.capture != Holder::Overlay(i)
                {
                    return Some(format!(
                        "{i} has the keyboard, the capture is {:?}'s",
                        self.capture
                    ));
                }
                if self.on_screen[i] != shows {
                    return Some(format!(
                        "{i} is {life:?} and on screen {}",
                        self.on_screen[i]
                    ));
                }
                if shows && !self.scaled[i] {
                    return Some(format!(
                        "{i} is {life:?} at a scale its window doesn't have"
                    ));
                }
                if shows && !self.drawn[i] {
                    return Some(format!("{i} is {life:?}, drawn on a device that's gone"));
                }
            }
            None
        }
    }

    /// Every world reachable from launch, by any order of events of any length, keeps every
    /// promise, and the path to the first that doesn't is the failure.
    #[test]
    fn every_order_of_events_keeps_an_overlays_promises() {
        let start = World::new();
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([(start, Vec::<String>::new())]);
        while let Some((world, path)) = queue.pop_front() {
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
            Life::Hidden,
            Life::Placed,
            Life::Shown,
            Life::Key,
            Life::Hiding,
        ] {
            assert!(
                seen.iter().any(|w| w.life[0] == life),
                "{life:?} is never reached"
            );
        }
    }

    /// The four bugs of #2563, as the orders that made them.
    #[test]
    fn a_hidden_window_keeps_nothing_it_held() {
        // #2516: a press held in the browser window, and the menu shown with the keyboard.
        let mut world = World::new();
        world.capture = Holder::Browser;
        world.deliver(
            0,
            Event::Show {
                keyboard: true,
                minimized: false,
                later: false,
            },
            0,
        );
        assert_eq!(
            world.capture,
            Holder::Nobody,
            "the browser window kept the pointer"
        );

        // #2541: the scale changed while it was hidden.
        let mut world = World::new();
        world.scaled = [false; OVERLAYS];
        world.deliver(
            0,
            Event::Show {
                keyboard: false,
                minimized: false,
                later: false,
            },
            0,
        );
        assert!(
            world.scaled[0],
            "shown at the scale it was told while hidden"
        );

        // #2545: Win+D and back, and Windows activates the hidden command bar.
        let mut world = World::new();
        world.give_keyboard(Holder::Overlay(0), 0);
        assert_eq!(
            world.keyboard,
            Holder::Browser,
            "the hidden bar kept the keyboard"
        );

        // #2536: the device lost under the open bar.
        let mut world = World::new();
        world.deliver(
            0,
            Event::Show {
                keyboard: true,
                minimized: false,
                later: false,
            },
            0,
        );
        world.drawn[0] = false;
        world.deliver(0, Event::DeviceLost, 0);
        assert!(
            world.drawn[0],
            "the open bar showed nothing after the device was lost"
        );
    }

    #[test]
    fn the_host_reads_what_step_returns() {
        let packed = browser_overlay_step(Life::Hidden as u8, 0, KEYBOARD);
        let (life, actions) = step(
            Life::Hidden,
            Event::Show {
                keyboard: true,
                minimized: false,
                later: false,
            },
        );
        assert_eq!(packed & 0xff, life as u32);
        assert_eq!(packed >> 8, actions.0 as u32);
        assert_eq!(
            browser_overlay_step(Life::Key as u8, 10, 0) & 0xff,
            Life::Shown as u32
        );
    }
}
