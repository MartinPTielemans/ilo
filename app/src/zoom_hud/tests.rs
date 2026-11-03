//! How long the zoom HUD stays, on a host that records what the HUD asks of its window.

use std::cell::RefCell;

use gpui::TestAppContext;

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Host for Recorder {
    fn show(&self, _: &WeakEntity<Shell>) {
        self.0.borrow_mut().push("show".into());
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }
}

fn open_hud(cx: &mut TestAppContext) -> Rc<Recorder> {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| open(host.clone(), cx))
        .expect("no HUD window");
    host
}

fn zoom(cx: &mut TestAppContext, tab: u64, percent: i32) {
    cx.update(|cx| show(tab, percent, WeakEntity::new_invalid(), cx));
    cx.run_until_parked();
}

/// Moves the clock on by `seconds` and returns what the HUD asked of the host meanwhile.
fn after(cx: &mut TestAppContext, host: &Recorder, seconds: f32) -> Vec<String> {
    cx.executor()
        .advance_clock(Duration::from_secs_f32(seconds));
    cx.run_until_parked();
    host.0.take()
}

#[gpui::test]
fn each_zoom_starts_its_time_again(cx: &mut TestAppContext) {
    let host = open_hud(cx);
    zoom(cx, 1, 90);
    assert_eq!(host.0.take(), ["show"]);
    assert!(after(cx, &host, 4.).is_empty());
    zoom(cx, 1, 80);
    assert_eq!(host.0.take(), ["show"]);
    // The first would have left at 5 s.
    assert!(after(cx, &host, 4.9).is_empty());
    assert!(after(cx, &host, 0.1).is_empty());
    assert_eq!(after(cx, &host, LEAVE), ["hide"]);
}

#[gpui::test]
fn another_tab_takes_it_at_once(cx: &mut TestAppContext) {
    let host = open_hud(cx);
    zoom(cx, 1, 110);
    host.0.take();
    cx.update(|cx| tab_shown(1, cx));
    assert!(after(cx, &host, 1.).is_empty());
    cx.update(|cx| tab_shown(2, cx));
    assert_eq!(after(cx, &host, LEAVE), ["hide"]);
}

#[gpui::test]
fn a_change_dips_to_hide_the_old_percent(_: &mut TestAppContext) {
    assert_eq!(
        Pose::arriving(0.),
        Pose {
            scale: FROM,
            opacity: 0.
        }
    );
    assert_eq!(
        Pose::arriving(1.),
        Pose {
            scale: 1.,
            opacity: 1.
        }
    );
    let settled = Pose::arriving(1.);
    let dipped = settled.changing(DIP);
    assert!(
        (dipped.scale - DIPPED).abs() < 1e-3 && dipped.text_opacity() < 1e-3,
        "{dipped:?}"
    );
    assert_eq!(settled.changing(DIP + BACK), settled);
    let gone = settled.leaving(LEAVE);
    assert!(
        (gone.scale - FROM).abs() < 1e-3 && gone.opacity < 1e-3,
        "{gone:?}"
    );
}
