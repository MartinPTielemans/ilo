//! The motion the Mac's host gives the windows floating over a browser window with Core Animation
//! (mac/host.mm), on Linux: GPUI paints all each window draws at the scale and opacity of the
//! moment (`Window::set_root_look`), as no compositor scales or fades another app's window for it.
//! A window moves once it's attached.

use std::{
    cell::RefCell,
    ffi::c_void,
    time::{Duration, Instant},
};

use gpui::{Pixels, Point, RootLook, Window, point, px};

/// How a motion's value goes from its start to its end over its time: a CSS cubic Bézier's
/// control points, as Core Animation's timing functions are.
#[derive(Clone, Copy)]
pub(crate) struct Curve(pub f32, pub f32, pub f32, pub f32);

/// Core Animation's default, an animation's with no timing function.
pub(crate) const LINEAR: Curve = Curve(0., 0., 1., 1.);
pub(crate) const EASE_IN: Curve = Curve(0.42, 0., 1., 1.);
pub(crate) const EASE_OUT: Curve = Curve(0., 0., 0.58, 1.);

/// One value's way from `from` to `to`.
#[derive(Clone, Copy)]
pub(crate) struct Track {
    pub from: f32,
    pub to: f32,
    pub seconds: f32,
    pub curve: Curve,
}

impl Track {
    /// Where it is at once.
    pub(crate) fn at(value: f32) -> Self {
        Track {
            from: value,
            to: value,
            seconds: 0.,
            curve: LINEAR,
        }
    }

    fn value(self, elapsed: f32) -> f32 {
        if elapsed >= self.seconds {
            return self.to;
        }
        let Curve(x1, y1, x2, y2) = self.curve;
        let progress = browser::shell::cubic_bezier(x1, y1, x2, y2, elapsed / self.seconds);
        self.from + (self.to - self.from) * progress
    }
}

/// A window's scale about `origin`, in it, and its opacity, each on its way.
#[derive(Clone, Copy)]
pub(crate) struct Motion {
    pub scale: Track,
    pub origin: Point<Pixels>,
    pub opacity: Track,
}

impl Motion {
    /// Its opacity alone on its way, at full size.
    pub(crate) fn fade(opacity: Track) -> Self {
        Motion {
            scale: Track::at(1.),
            origin: point(px(0.), px(0.)),
            opacity,
        }
    }

    fn look(self, elapsed: f32) -> RootLook {
        RootLook {
            scale: self.scale.value(elapsed),
            origin: self.origin,
            opacity: self.opacity.value(elapsed),
            moving: [self.scale, self.opacity]
                .iter()
                .any(|track| track.from != track.to && elapsed < track.seconds),
        }
    }
}

thread_local! {
    /// The motion each window's view was last given, and when it started.
    static MOVING: RefCell<Vec<(usize, Motion, Instant)>> = const { RefCell::new(Vec::new()) };
}

/// How the window of `view` shows now, after any motion: as it draws if it never had one.
fn now(view: usize) -> RootLook {
    let look = MOVING.with_borrow(|moving| {
        (moving.iter().find(|(each, ..)| *each == view))
            .map(|(_, motion, started)| motion.look(started.elapsed().as_secs_f32()))
    });
    look.unwrap_or(RootLook {
        scale: 1.,
        origin: point(px(0.), px(0.)),
        opacity: 1.,
        moving: false,
    })
}

/// The window of `view` moves as it's told from now on. Each frame it draws reads its motion
/// then, so one started as it shows is in its first frame.
pub(crate) fn attach(view: *mut c_void) {
    crate::update_app_later(move |cx| {
        crate::overlays::update_window(view, cx, |window, _| attach_to(window, view));
    });
}

/// `window`, whose view is `view`, moves as it's told from now on (`attach`).
pub(crate) fn attach_to(window: &mut Window, view: *mut c_void) {
    let view = view as usize;
    window.set_root_look(Some(Box::new(move || now(view))));
}

/// The opacity the window of `view` shows at now, after any motion: 1 if it never had one.
pub(crate) fn opacity(view: *mut c_void) -> f32 {
    now(view as usize).opacity
}

/// Starts `motion` in the window of `view`, from now, in place of the one it had. Where what it
/// draws doesn't blend with what's under it, it shows at full opacity throughout, as the X server
/// with no compositor would blend it with black.
pub(crate) fn start(view: *mut c_void, mut motion: Motion) {
    if !gpui_linux::overlays_blend() {
        motion.opacity = Track::at(1.);
    }
    MOVING.with_borrow_mut(|moving| {
        moving.retain(|(each, ..)| *each != view as usize);
        moving.push((view as usize, motion, Instant::now()));
    });
    crate::update_app_later(move |cx| {
        crate::overlays::update_window(view, cx, |window, _| window.refresh());
    });
}

/// Shows the window of `view` as `look` says, from now on.
pub(crate) fn hold(view: *mut c_void, look: RootLook) {
    let motion = Motion {
        scale: Track::at(look.scale),
        origin: look.origin,
        opacity: Track::at(look.opacity),
    };
    start(view, motion);
}

/// Runs `then` once `seconds` have gone by, as a motion ends.
pub(crate) fn after(seconds: f32, then: impl FnOnce() + 'static) {
    if let Some(cx) = crate::CX.with_borrow(Clone::clone) {
        let timer = cx
            .background_executor()
            .timer(Duration::from_secs_f32(seconds));
        cx.foreground_executor()
            .spawn(async move {
                timer.await;
                then();
            })
            .detach();
    }
}
