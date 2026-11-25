//! The window's material under a floating window's plate (#4090), which the plate's translucent
//! fill lies on. The Mac's host puts it under the plate in its shape, as the plate is drawn now:
//! scaled and faded with it. Where the host has no material, nothing is told and the fill is
//! opaque (`Float::over_material`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gpui::{Bounds, IntoElement, Pixels, Point, Styled, Window, WindowId, canvas, point, size};

/// A plate as the host is told of it: where it's drawn in its window and its corner radius, both
/// scaled as it's drawn, how opaque it's drawn, and whether its look is light.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Plate {
    pub bounds: Bounds<Pixels>,
    pub radius: f32,
    pub opacity: f32,
    pub light: bool,
}

/// What tells the host of a window's plate, answering whether its material now lies under it.
pub type Tell = fn(&Window, Plate) -> bool;

thread_local! {
    static HOST: Cell<Option<Tell>> = const { Cell::new(None) };
    /// The scale what's painting now is drawn at, about its origin (`shell::scaled`).
    static SCALE: Cell<Option<(f32, Point<Pixels>)>> = const { Cell::new(None) };
    /// The last plate each window's host put its material under, so a frame that moves nothing
    /// tells it nothing. The toast and the zoom HUD each have one window for the app's life.
    static TOLD: RefCell<HashMap<WindowId, Plate>> = RefCell::default();
}

/// Has the host told of each plate from now on. The preview has none.
pub fn set_host(tell: Tell) {
    HOST.set(Some(tell));
}

/// Runs `paint` with what it paints drawn at `factor` its size about `origin`.
pub(crate) fn scaled<R>(factor: f32, origin: Point<Pixels>, paint: impl FnOnce() -> R) -> R {
    let was = SCALE.replace(Some((factor, origin)));
    let painted = paint();
    SCALE.set(was);
    painted
}

/// The material under the plate it fills, with the plate's corner `radius`, as opaque as
/// `opacity`, in a `light` or dark look.
pub fn under(radius: f32, opacity: f32, light: bool) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let Some(tell) = HOST.get() else {
                return;
            };
            let plate = match SCALE.get() {
                Some((factor, origin)) => Plate {
                    bounds: scaled_about(bounds, factor, origin),
                    radius: radius * factor,
                    opacity,
                    light,
                },
                None => Plate {
                    bounds,
                    radius,
                    opacity,
                    light,
                },
            };
            // A plate the host couldn't put its material under yet, as its view isn't in its window
            // or has no size, isn't told: the frame GPUI draws once the window has them tells it
            // again.
            let window_id = window.window_handle().window_id();
            let moved = TOLD.with_borrow(|told| told.get(&window_id) != Some(&plate));
            if moved && tell(window, plate) {
                TOLD.with_borrow_mut(|told| told.insert(window_id, plate));
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn scaled_about(bounds: Bounds<Pixels>, factor: f32, origin: Point<Pixels>) -> Bounds<Pixels> {
    let at = |edge: Pixels, about: Pixels| about + (edge - about) * factor;
    Bounds::new(
        point(at(bounds.origin.x, origin.x), at(bounds.origin.y, origin.y)),
        size(bounds.size.width * factor, bounds.size.height * factor),
    )
}

#[cfg(test)]
mod tests;
