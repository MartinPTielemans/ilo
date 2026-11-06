//! The shadows of the overlays and the Site Controls on Linux, which the Mac's host draws in a
//! window under each (mac/host.mm). GPUI's Linux platform gives a window no shadow of its own, so
//! each has a window of GPUI's directly under it that draws only the shadow, which the pointer goes
//! through, as ilo's menu draws its panels' in its own window. On X11 with no compositor there's
//! none, as the X server would draw it opaque.
//!
//! shortcut: an arrow casts none, as GPUI shadows only rounded rectangles: the shadow is the
//! card's, through the corners rounded the most.

use std::{
    cell::RefCell,
    ffi::{c_int, c_void},
};

use gpui::{
    App, AppContext as _, Bounds, BoxShadow, Context, IntoElement, ParentElement as _, Pixels,
    Render, RootLook, Styled as _, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, div, hsla, point, px, size,
};

use crate::motion::{self, Motion};

/// What an overlay's level draws, in the terms of host_set_overlay_elevation.
#[derive(Clone, Copy)]
struct Elevation {
    outline_opacity: f32,
    outline_radius: f32,
    shadow_opacity: f32,
    shadow_radius: f32,
    shadow_drop: f32,
}

impl Elevation {
    /// How far its window reaches past the card above, on the sides and below, in whole points.
    /// GPUI draws a shadow out to three of its radii.
    fn margins(self) -> (f32, f32, f32) {
        let reach = (3. * self.shadow_radius).max(3. * self.outline_radius);
        let above = (reach - self.shadow_drop).max(3. * self.outline_radius);
        (
            above.ceil(),
            reach.ceil(),
            (reach + self.shadow_drop).ceil(),
        )
    }

    /// The drop shadow, and over it the outline, as elevation.rs gives GPUI's surfaces.
    fn shadows(self) -> Vec<BoxShadow> {
        let shadow = |opacity: f32, radius: f32, drop: f32| BoxShadow {
            color: hsla(0., 0., 0., opacity),
            offset: point(px(0.), px(drop)),
            blur_radius: px(radius),
            spread_radius: px(0.),
            inset: false,
        };
        vec![
            shadow(self.shadow_opacity, self.shadow_radius, self.shadow_drop),
            shadow(self.outline_opacity, self.outline_radius, 0.),
        ]
    }
}

struct Slot {
    overlay: c_int,
    elevation: Elevation,
    /// The overlay's view it's under, once the UI added it.
    over: *mut c_void,
    /// Its own window, once open.
    window: Option<(WindowHandle<Shadow>, *mut c_void)>,
    /// The overlay's outline when it isn't a rounded rectangle, from its top left.
    corners: Option<Vec<(f32, f32, f32)>>,
    /// The card's corners' radius, as it showed last.
    radius: f32,
    /// Where the overlay's top left is in the shadow's window, as it showed last.
    inset: gpui::Point<Pixels>,
    shown: bool,
}

thread_local! {
    static SLOTS: RefCell<Vec<Slot>> = const { RefCell::new(Vec::new()) };
}

fn with<R>(overlay: c_int, f: impl FnOnce(&mut Slot) -> R) -> Option<R> {
    SLOTS.with_borrow_mut(|all| all.iter_mut().find(|slot| slot.overlay == overlay).map(f))
}

/// The view of a shadow's window: the card, inside the window by the level's margins, with no
/// fill.
pub(crate) struct Shadow(c_int);

impl Render for Shadow {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full();
        let Some((elevation, radius)) = with(self.0, |slot| (slot.elevation, slot.radius)) else {
            return root;
        };
        let (above, sides, below) = elevation.margins();
        let size = window.viewport_size();
        root.child(
            div()
                .absolute()
                .left(px(sides))
                .top(px(above))
                .w(size.width - px(2. * sides))
                .h(size.height - px(above + below))
                .rounded(px(radius))
                .shadow(elevation.shadows()),
        )
    }
}

/// Gives `overlay` (one of the host's overlays, or the Site Controls) its level's shadow, and
/// opens its window once the update under way is over.
#[unsafe(no_mangle)]
pub extern "C" fn host_set_overlay_elevation(
    overlay: c_int,
    outline_opacity: f32,
    outline_radius: f64,
    shadow_opacity: f32,
    shadow_radius: f64,
    shadow_drop: f64,
) {
    let elevation = Elevation {
        outline_opacity,
        outline_radius: outline_radius as f32,
        shadow_opacity,
        shadow_radius: shadow_radius as f32,
        shadow_drop: shadow_drop as f32,
    };
    let opened = with(overlay, |slot| {
        slot.elevation = elevation;
        slot.window.is_some()
    });
    if opened.is_none() {
        SLOTS.with_borrow_mut(|all| {
            all.push(Slot {
                overlay,
                elevation,
                over: std::ptr::null_mut(),
                window: None,
                corners: None,
                radius: 0.,
                inset: point(px(0.), px(0.)),
                shown: false,
            })
        });
        crate::update_app_later(move |cx| open(overlay, cx));
    }
}

fn open(overlay: c_int, cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(1.), px(1.)),
        ))),
        kind: WindowKind::Floating,
        show: false,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        // GPUI holds a window that isn't the key one to 30 fps, and this one never is.
        inactive_frame_interval: None,
        ..Default::default()
    };
    let Ok(window) = cx.open_window(options, |_, cx| cx.new(|_| Shadow(overlay))) else {
        eprintln!("browser: couldn't open the shadow window of overlay {overlay}");
        return;
    };
    let Ok(view) = window.update(cx, |_, window, _| {
        let view = crate::ns_view(window);
        motion::attach_to(window, view);
        view
    }) else {
        return;
    };
    gpui_linux::set_overlay_pointer(view, Some(&[]));
    if let Some(over) = with(overlay, |slot| {
        slot.window = Some((window, view));
        slot.over
    }) && !over.is_null()
    {
        gpui_linux::set_overlay_under(view, over);
    }
}

/// `over` is `overlay`'s view, which its shadow goes under.
pub(crate) fn attach(overlay: c_int, over: *mut c_void) {
    if let Some(Some((_, view))) = with(overlay, |slot| {
        slot.over = over;
        slot.window
    }) {
        gpui_linux::set_overlay_under(view, over);
    }
}

/// `overlay`'s outline isn't a rounded rectangle: `corners`, from its top left.
pub(crate) fn set_shape(overlay: c_int, corners: &[(f32, f32, f32)]) {
    with(overlay, |slot| slot.corners = Some(corners.to_vec()));
}

/// `overlay` shows, or moves, at `bounds` over `ui_view`, its corners rounded by `radius` unless
/// it has an outline of its own. Called before the overlay shows: sway draws Xwayland's windows
/// over the browser window in the order they're mapped, whatever X's stacking says.
pub(crate) fn show(overlay: c_int, ui_view: *mut c_void, bounds: Bounds<Pixels>, radius: f32) {
    if let Some((view, framed)) = place(overlay, bounds, radius) {
        crate::overlays::show_over(view, ui_view, framed, px(0.), false, |_| {});
    }
}

/// `overlay`, which shows, moves to `bounds`, as its height changes.
pub(crate) fn moved(overlay: c_int, bounds: Bounds<Pixels>, radius: f32) {
    if let Some((view, framed)) = place(overlay, bounds, radius) {
        gpui_linux::move_overlay(view, framed, px(0.));
    }
}

/// Where the window of `overlay`'s shadow goes for it at `bounds`, and its view.
fn place(
    overlay: c_int,
    bounds: Bounds<Pixels>,
    radius: f32,
) -> Option<(*mut c_void, Bounds<Pixels>)> {
    let (window, view, changed, framed) = with(overlay, |slot| {
        let (window, view) = slot.window?;
        let (card, radius) = match &slot.corners {
            Some(corners) => card_of(corners, bounds),
            None => (bounds, radius),
        };
        let changed = slot.radius != radius;
        slot.radius = radius;
        slot.shown = true;
        let (above, sides, below) = slot.elevation.margins();
        let framed = Bounds::from_corners(
            card.origin - point(px(sides), px(above)),
            card.bottom_right() + point(px(sides), px(below)),
        );
        slot.inset = bounds.origin - framed.origin;
        Some((window, view, changed, framed))
    })??;
    // It draws the card's corners from what it holds, which a new size alone doesn't redraw.
    if changed {
        crate::update_app_later(move |cx| {
            window.update(cx, |_, window, _| window.refresh()).ok();
        });
    }
    Some((view, framed))
}

/// The shadow of `overlay` moves as `motion` says, its origin in the overlay, as it grows in or
/// shrinks away. It scales with the card, as GPUI draws a shadow under all of the card, which the
/// card must cover, where the Mac's shadow is cut out of it.
pub(crate) fn follow(overlay: c_int, mut motion: Motion) {
    if let Some(Some((view, inset))) = with(overlay, |slot| Some((slot.window?.1, slot.inset))) {
        motion.origin += inset;
        motion::start(view, motion);
    }
}

/// `overlay` drew a frame of its own motion at `scale` about `origin`, in it, and `opacity`, which
/// its shadow takes, about the same point.
pub(crate) fn drew(overlay: c_int, scale: f32, opacity: f32, origin: gpui::Point<Pixels>) {
    if let Some(Some((view, inset))) = with(overlay, |slot| Some((slot.window?.1, slot.inset))) {
        let origin = origin + inset;
        motion::hold(
            view,
            RootLook {
                scale,
                origin,
                opacity,
                moving: false,
            },
        );
    }
}

/// `overlay` hides.
pub(crate) fn hide(overlay: c_int) {
    if let Some(Some((_, view))) = with(overlay, |slot| {
        let shown = std::mem::take(&mut slot.shown);
        slot.window.filter(|_| shown)
    }) {
        gpui_linux::hide_overlay(view, || {});
    }
}

/// The card of an outline through `corners` at `bounds`: the box of the corners rounded the most,
/// as an arrow's are rounded less, and their radius.
fn card_of(corners: &[(f32, f32, f32)], bounds: Bounds<Pixels>) -> (Bounds<Pixels>, f32) {
    let radius = corners
        .iter()
        .map(|&(_, _, radius)| radius)
        .fold(0., f32::max);
    let card = corners.iter().filter(|&&(_, _, each)| each == radius);
    let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y, _) in card {
        (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x), bottom.max(y));
    }
    let origin = bounds.origin;
    let card = Bounds::from_corners(
        origin + point(px(left), px(top)),
        origin + point(px(right), px(bottom)),
    );
    (card, radius)
}
