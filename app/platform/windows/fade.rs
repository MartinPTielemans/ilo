//! The fades the Mac's host gives the tooltip and the link target with Core Animation, on Windows:
//! GPUI paints all the window draws at the opacity of the moment (`Window::set_root_look`), as
//! Linux's floats do (linux/motion.rs). The host starts each (host.cc, FadeIn and FadeOut).

use std::{cell::RefCell, ffi::c_void, time::Instant};

use gpui::{AnyWindowHandle, RootLook, Window, point, px};

/// A window's opacity on its way from `from` to `to`, easing out as the Mac's does.
#[derive(Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    seconds: f32,
    started: Instant,
}

impl Fade {
    /// Its opacity now, and whether it goes on past now.
    fn now(self) -> (f32, bool) {
        let elapsed = self.started.elapsed().as_secs_f32();
        if elapsed >= self.seconds {
            return (self.to, false);
        }
        let progress = browser::shell::cubic_bezier(0., 0., 0.58, 1., elapsed / self.seconds);
        (self.from + (self.to - self.from) * progress, true)
    }
}

thread_local! {
    /// Each window that fades, by its HWND, and its last fade.
    static FADING: RefCell<Vec<(usize, AnyWindowHandle, Option<Fade>)>> =
        const { RefCell::new(Vec::new()) };
}

/// The opacity the window `hwnd` shows at now, 1 if it never faded, and whether it's fading.
fn now(hwnd: usize) -> (f32, bool) {
    let fade = FADING.with_borrow(|fading| {
        (fading.iter().find(|(each, ..)| *each == hwnd)).and_then(|(.., fade)| *fade)
    });
    fade.map_or((1., false), Fade::now)
}

/// `window`, whose HWND is `hwnd`, fades as the host tells it from now on.
pub(crate) fn attach(window: &mut Window, hwnd: *mut c_void) {
    let hwnd = hwnd as usize;
    let handle = window.window_handle();
    FADING.with_borrow_mut(|fading| fading.push((hwnd, handle, None)));
    window.set_root_look(Some(Box::new(move || {
        let (opacity, moving) = now(hwnd);
        RootLook { scale: 1., origin: point(px(0.), px(0.)), opacity, moving }
    })));
}

/// Fades the window `hwnd` from `from`, or from where it shows now if `from` is below 0, to `to`
/// over `seconds`, for the host.
#[unsafe(no_mangle)]
pub extern "C" fn browser_fade(hwnd: *mut c_void, from: f64, to: f64, seconds: f64) {
    let hwnd = hwnd as usize;
    let from = if from < 0. { now(hwnd).0 } else { from as f32 };
    let fade = Fade { from, to: to as f32, seconds: seconds as f32, started: Instant::now() };
    let handle = FADING.with_borrow_mut(|fading| {
        let (.., handle, last) = fading.iter_mut().find(|(each, ..)| *each == hwnd)?;
        *last = Some(fade);
        Some(*handle)
    });
    if let Some(handle) = handle {
        crate::update_app_later(move |cx| {
            handle.update(cx, |_, window, _| window.refresh()).ok();
        });
    }
}
