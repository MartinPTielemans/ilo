//! The Dock icon's dot in the color of the key window's Space (docs/ux-spec.md › Dock dot). The
//! Dock takes 10–45 ms of the main thread to swap the icon, more than a whole Space switch may
//! take, so the swap waits until the switch is over and a run of switches has stopped.

use std::{rc::Rc, time::Duration};

use gpui::{App, Global, Task};

/// What swaps the icon: platform/mac/dock.mm in the browser.
pub trait Host {
    /// Gives the dot `color`, as 0xRRGGBB, or the icon's own green.
    fn show(&self, color: Option<u32>);
}

/// How long the dot waits after the last change: past a Space switch's slide, which has settled
/// by 300 ms, and long enough to take quick switches as one.
pub const QUIET: Duration = Duration::from_millis(400);

pub use crate::tokens::dock_dot::GREEN;

/// Settings › General's switch for the dot, where the host draws one: the Dock icon's on the Mac
/// and the taskbar button's on Windows (#2681). Linux's host draws none, so it has no switch.
pub struct Switch {
    pub name: &'static str,
    pub line: &'static str,
}

pub const SWITCH: Option<Switch> = if cfg!(target_os = "macos") {
    Some(Switch {
        name: "Dock icon takes the Space's color",
        line: concat!(
            "Its dot shows the Space in front. Off, the icon stays ",
            env!("APP_NAME"),
            " green."
        ),
    })
} else if cfg!(target_os = "windows") {
    Some(Switch {
        name: "Taskbar button takes the Space's color",
        line: concat!(
            "Its dot shows the Space in front. Off, the dot stays ",
            env!("APP_NAME"),
            " green."
        ),
    })
} else {
    None
};

struct DockDot {
    host: Rc<dyn Host>,
    wait: Option<Task<()>>,
    /// The color last asked for, which the dot takes again once it follows again.
    last: Option<u32>,
    /// Settings › General's Dock icon switch is off: the icon stays green and nothing swaps.
    plain: bool,
}

impl Global for DockDot {}

/// Starts following, with `host` to swap the icon. Without it, `follow` does nothing, as in the
/// preview.
pub fn open(host: Rc<dyn Host>, cx: &mut App) {
    cx.set_global(DockDot {
        host,
        wait: None,
        last: None,
        plain: false,
    });
}

/// Settings › General's Dock icon switch: off, the icon goes back to green at once and stays it,
/// sparing each Space switch the swap; on, it takes the key window's Space's color again.
pub fn set_plain(plain: bool, cx: &mut App) {
    let Some(dot) = cx.try_global::<DockDot>() else {
        return;
    };
    if dot.plain == plain {
        return;
    }
    let (host, last) = (dot.host.clone(), dot.last);
    let dot = cx.global_mut::<DockDot>();
    dot.plain = plain;
    dot.wait = None;
    host.show(if plain { None } else { last });
}

/// The key window shows a Space in `color`, or one without a color. Asked again for the same
/// color, the host checks again too, which catches an icon style change it wasn't told of.
pub fn follow(color: Option<u32>, cx: &mut App) {
    if !cx.has_global::<DockDot>() {
        return;
    }
    let color = color.filter(|&color| color != GREEN);
    let dot = cx.global_mut::<DockDot>();
    dot.last = color;
    if dot.plain {
        return;
    }
    // Replacing the wait drops the one before, so only the last change of a run swaps.
    let wait = cx.spawn(async move |cx| {
        cx.background_executor().timer(QUIET).await;
        cx.update(|cx| {
            let dot = cx.global_mut::<DockDot>();
            dot.wait = None;
            dot.host.clone()
        })
        .show(color);
    });
    cx.global_mut::<DockDot>().wait = Some(wait);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use gpui::TestAppContext;

    use super::*;

    #[derive(Default)]
    struct Recorder(RefCell<Vec<Option<u32>>>);

    impl Host for Recorder {
        fn show(&self, color: Option<u32>) {
            self.0.borrow_mut().push(color);
        }
    }

    fn opened(cx: &mut TestAppContext) -> Rc<Recorder> {
        let host = Rc::new(Recorder::default());
        cx.update(|cx| open(host.clone(), cx));
        host
    }

    fn after(cx: &mut TestAppContext, host: &Recorder, time: Duration) -> Vec<Option<u32>> {
        cx.executor().advance_clock(time);
        cx.run_until_parked();
        host.0.take()
    }

    #[gpui::test]
    fn it_swaps_once_the_switch_is_over(cx: &mut TestAppContext) {
        let host = opened(cx);
        cx.update(|cx| follow(Some(0x8655BF), cx));
        assert!(after(cx, &host, QUIET - Duration::from_millis(1)).is_empty());
        assert_eq!(after(cx, &host, Duration::from_millis(1)), [Some(0x8655BF)]);
    }

    #[gpui::test]
    fn quick_switches_swap_once_for_the_last_space(cx: &mut TestAppContext) {
        let host = opened(cx);
        for color in [Some(0x8655BF), None, Some(0xE0475B)] {
            cx.update(|cx| follow(color, cx));
            assert!(after(cx, &host, QUIET / 2).is_empty());
        }
        assert_eq!(after(cx, &host, QUIET / 2), [Some(0xE0475B)]);
        assert!(after(cx, &host, QUIET * 2).is_empty());
    }

    /// With the switch off the icon goes green at once and no Space's color swaps it; on again,
    /// it takes the last Space's color.
    #[gpui::test]
    fn off_the_icon_stays_green_and_nothing_swaps(cx: &mut TestAppContext) {
        let host = opened(cx);
        cx.update(|cx| follow(Some(0x8655BF), cx));
        assert_eq!(after(cx, &host, QUIET), [Some(0x8655BF)]);
        cx.update(|cx| set_plain(true, cx));
        assert_eq!(host.0.take(), [None]);
        cx.update(|cx| follow(Some(0xE0475B), cx));
        assert!(
            after(cx, &host, QUIET * 2).is_empty(),
            "a switch swapped the plain icon"
        );
        cx.update(|cx| set_plain(false, cx));
        assert_eq!(host.0.take(), [Some(0xE0475B)]);
    }

    /// Where the host colours a dot, Settings offers the way back to green, named for where the
    /// dot is; Linux's host draws none, so nothing needs turning off there.
    #[gpui::test]
    fn the_switch_is_where_the_dot_is_and_turns_it_green(cx: &mut TestAppContext) {
        let Some(switch) = SWITCH else {
            assert!(
                cfg!(target_os = "linux"),
                "a system that draws the dot has no switch"
            );
            return;
        };
        let place = if cfg!(target_os = "windows") {
            "taskbar dot"
        } else {
            "dock dot"
        };
        let row = crate::settings_search::Entry {
            section: crate::settings_column::Section::General,
            under: None,
            name: switch.name.into(),
            line: Some(switch.line.into()),
            keys: None,
        };
        assert_eq!(
            crate::settings_search::find(place, &[row]),
            [0],
            "Settings' search for {place:?} didn't find {:?}",
            switch.name
        );
        let host = opened(cx);
        cx.update(|cx| follow(Some(0x8655BF), cx));
        assert_eq!(after(cx, &host, QUIET), [Some(0x8655BF)]);
        cx.update(|cx| set_plain(true, cx));
        assert_eq!(
            host.0.take(),
            [None],
            "the switch off didn't turn the dot green"
        );
    }

    #[gpui::test]
    fn a_green_space_is_the_icons_own(cx: &mut TestAppContext) {
        let host = opened(cx);
        cx.update(|cx| follow(Some(GREEN), cx));
        assert_eq!(after(cx, &host, QUIET), [None]);
    }
}
