//! The Site Controls and Capture of platform/host.h on Linux, which float over a browser window
//! with its keyboard until the user leaves them, as their windows do on the Mac. GPUI shows each as
//! an overlay of that window (overlays.rs); the core lends it the window's keyboard while it has it.
//!
//! The Site Controls cast their shadow from a window under them (shadows.rs), which scales and
//! fades with the frames they draw of their motion.
//!
//! The pointer over Capture is ilo's camera, as on the Mac, which GPUI shows in the crosshair's
//! place over its window (`gpui_linux::set_overlay_cursor`); the crosshair where it can't.
//!
//! shortcut: no blur under the Site Controls.

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
};

use gpui::{App, Bounds, CursorStyle, DevicePixels, Pixels, SvgSize, point, px, size};

use crate::{bc, overlays, shadows};

/// The Site Controls' slot in the host's overlays, for their elevation (platform/host.h).
const SITE_CONTROLS: std::ffi::c_int = 6;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    SiteControls,
    Capture,
}

struct Popover {
    kind: Kind,
    view: *mut c_void,
    dismissed: extern "C" fn(),
    shown: Option<Shown>,
}

#[derive(Clone, Copy)]
struct Shown {
    /// The UI's view of the browser window they're over, the core's id of it, and its content's
    /// size as they showed, which Capture's picture of the page fits.
    ui_view: *mut c_void,
    window: bc::bc_id,
    content: gpui::Size<Pixels>,
    bounds: Bounds<Pixels>,
    /// They take no more clicks or keys as they leave (host_release_site_controls).
    released: bool,
}

thread_local! {
    static POPOVERS: RefCell<Vec<Popover>> = const { RefCell::new(Vec::new()) };
    /// The pointer over Capture is the camera, not the arrow, as it is till its menu shows.
    static CAMERA: Cell<bool> = const { Cell::new(true) };
}

fn add(kind: Kind, view: *mut c_void, dismissed: extern "C" fn()) {
    POPOVERS.with_borrow_mut(|all| {
        all.retain(|each| each.kind != kind);
        all.push(Popover {
            kind,
            view,
            dismissed,
            shown: None,
        });
    });
}

fn with<R>(kind: Kind, f: impl FnOnce(&mut Popover) -> R) -> Option<R> {
    POPOVERS.with_borrow_mut(|all| all.iter_mut().find(|each| each.kind == kind).map(f))
}

fn name(kind: Kind) -> &'static str {
    match kind {
        Kind::SiteControls => "site controls",
        Kind::Capture => "capture",
    }
}

/// Shows `kind` over `ui_view`'s window at `left`, `top`, `width` by `height` in its content, with
/// the keyboard.
fn show(kind: Kind, ui_view: *mut c_void, left: f64, top: f64, width: f64, height: f64) -> bool {
    if with(kind, |p| {
        p.shown.is_some_and(|shown| shown.ui_view != ui_view)
    }) == Some(true)
    {
        hide(kind);
    }
    let Some(content) = gpui_linux::embedder_size(ui_view) else {
        return false;
    };
    let bounds = Bounds::new(
        point(px(left as f32), px(top as f32)),
        size(px(width as f32), px(height as f32)),
    );
    let window = crate::window_with_view(ui_view);
    let Some((view, had)) = with(kind, |p| {
        let had = p.shown.is_some_and(|shown| !shown.released);
        p.shown = Some(Shown {
            ui_view,
            window,
            content,
            bounds,
            released: false,
        });
        (p.view, had)
    }) else {
        return false;
    };
    if !had {
        overlays::lend_keyboard(window, view, true);
    }
    gpui_linux::set_overlay_pointer(view, None);
    if kind == Kind::SiteControls {
        // It comes in from nothing, as their frames say (host_site_controls_drew).
        if !had {
            shadows::drew(SITE_CONTROLS, 1., 0., point(px(0.), px(0.)));
        }
        shadows::show(SITE_CONTROLS, ui_view, bounds, 0.);
    }
    crate::overlays::show_over(view, ui_view, bounds, px(0.), true, left_popover);
    true
}

/// They take no more clicks or keys: the pointer goes through them, and the window has its
/// keyboard back.
fn release(kind: Kind) -> bool {
    let Some(Some((view, window))) = with(kind, |p| {
        let shown = p.shown.as_mut().filter(|shown| !shown.released)?;
        shown.released = true;
        Some((p.view, shown.window))
    }) else {
        return false;
    };
    gpui_linux::set_overlay_pointer(view, Some(&[]));
    gpui_linux::set_overlay_keyboard(view, false, left_popover);
    overlays::lend_keyboard(window, view, false);
    true
}

fn hide(kind: Kind) -> bool {
    let Some(Some((view, shown))) = with(kind, |p| p.shown.take().map(|shown| (p.view, shown)))
    else {
        return false;
    };
    gpui_linux::hide_overlay(view, move || {
        if !shown.released {
            overlays::lend_keyboard(shown.window, view, false);
        }
    });
    if kind == Kind::SiteControls {
        shadows::hide(SITE_CONTROLS);
    }
    // check.sh reads this line.
    browser::log!("browser: {} hidden", name(kind));
    true
}

/// The user left `kind`: a press outside it or the keyboard going to another window. Site Controls
/// let go and the UI moves them out; Capture goes at once. The UI hears after the event.
fn dismiss(kind: Kind) {
    let gone = match kind {
        Kind::SiteControls => release(kind),
        Kind::Capture => hide(kind),
    };
    if gone && let Some(dismissed) = with(kind, |p| p.dismissed) {
        dismissed();
    }
}

fn left_popover(view: *mut c_void) {
    // To ilo's menu over them, such as a tile's, which gives the keyboard back as it closes.
    if crate::menu_window::over(view) {
        gpui_linux::set_overlay_keyboard(view, true, left_popover);
        return;
    }
    let kind = POPOVERS.with_borrow(|all| {
        (all.iter())
            .find(|each| each.view == view && each.shown.is_some())
            .map(|each| each.kind)
    });
    if let Some(kind) = kind {
        dismiss(kind);
    }
}

/// `ui_view`'s window's content may have changed size: Capture's picture of the page no longer
/// fits, so it goes, as the user leaving it. The UI hears once its update is over.
pub(crate) fn resized(ui_view: *mut c_void) {
    let shown = with(Kind::Capture, |p| p.shown).flatten();
    if let Some(shown) = shown
        && shown.ui_view == ui_view
        && gpui_linux::embedder_size(ui_view).is_some_and(|now| now != shown.content)
        && hide(Kind::Capture)
        && let Some(dismissed) = with(Kind::Capture, |p| p.dismissed)
    {
        overlays::later(dismissed);
    }
}

/// The views of those that show and still take clicks and keys, for the driver.
pub(crate) fn shown() -> Vec<*mut c_void> {
    POPOVERS.with_borrow(|all| {
        (all.iter())
            .filter(|p| p.shown.is_some_and(|shown| !shown.released))
            .map(|p| p.view)
            .collect()
    })
}

/// The browser window the popover of `view` shows over, by its UI's view, and where in its
/// content, for ilo's menu to show over it (overlays::placed).
pub(crate) fn placed(view: *mut c_void) -> Option<(*mut c_void, Bounds<Pixels>)> {
    POPOVERS.with_borrow(|all| {
        (all.iter())
            .find(|p| p.view == view)
            .and_then(|p| p.shown.map(|shown| (shown.ui_view, shown.bounds)))
    })
}

/// `ui_view`'s window is closing: what shows over it goes.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    for kind in [Kind::SiteControls, Kind::Capture] {
        if with(kind, |p| {
            p.shown.is_some_and(|shown| shown.ui_view == ui_view)
        }) == Some(true)
        {
            hide(kind);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_site_controls(view: *mut c_void, dismissed: extern "C" fn()) {
    add(Kind::SiteControls, view, dismissed);
    shadows::attach(SITE_CONTROLS, view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_site_controls_shape(corners: *const f64, count: usize) {
    let Some(view) = with(Kind::SiteControls, |p| p.view) else {
        return;
    };
    if corners.is_null() {
        return;
    }
    // SAFETY: the caller passes `count` corners of three numbers each.
    let numbers = unsafe { std::slice::from_raw_parts(corners, count * 3) };
    let corners: Vec<gpui_linux::Corner> = (numbers.chunks_exact(3))
        .map(|corner| (corner[0] as f32, corner[1] as f32, corner[2] as f32))
        .collect();
    gpui_linux::set_overlay_outline(view, &corners);
    shadows::set_shape(SITE_CONTROLS, &corners);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_site_controls(
    ui_view: *mut c_void,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) {
    if show(Kind::SiteControls, ui_view, left, top, width, height) {
        // check.sh reads these lines.
        browser::log!("browser: site controls at {left} {top} {width} {height}");
        browser::log!("browser: site controls shown");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_site_controls_height(height: f64) {
    let moved = with(Kind::SiteControls, |p| {
        let shown = p.shown.as_mut()?;
        shown.bounds.size.height = px(height as f32);
        Some((p.view, shown.bounds))
    });
    if let Some(Some((view, bounds))) = moved {
        gpui_linux::move_overlay(view, bounds, px(0.));
        shadows::moved(SITE_CONTROLS, bounds, 0.);
        browser::log!("browser: site controls height {height}");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_site_controls_drew(scale: f64, opacity: f64, x: f64, y: f64) {
    if with(Kind::SiteControls, |p| p.shown.is_some()) == Some(true) {
        let origin = point(px(x as f32), px(y as f32));
        shadows::drew(SITE_CONTROLS, scale as f32, opacity as f32, origin);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_release_site_controls() {
    if release(Kind::SiteControls) {
        browser::log!("browser: site controls released");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_site_controls() {
    hide(Kind::SiteControls);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_capture(view: *mut c_void, dismissed: extern "C" fn()) {
    add(Kind::Capture, view, dismissed);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_capture(
    ui_view: *mut c_void,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) {
    if show(Kind::Capture, ui_view, left, top, width, height) {
        camera();
        // check.sh reads this line.
        browser::log!("browser: capture shown at {left} {top} {width} {height}");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_capture_camera(camera: bool) {
    CAMERA.set(camera);
    self::camera();
}

/// Capture's window shows the pointer as the camera, or the arrow, as `CAMERA` says.
fn camera() {
    let Some(view) = with(Kind::Capture, |p| p.view) else {
        return;
    };
    let style = CAMERA.get().then_some(CursorStyle::Crosshair);
    crate::update_app_later(move |cx| {
        overlays::update_window(view, cx, |window, cx| {
            let picture = camera_picture(window.scale_factor(), cx);
            gpui_linux::set_overlay_cursor(view, CursorStyle::Crosshair, picture);
            window.set_cursor_override(style, cx);
        });
    });
}

// Lucide's camera (https://github.com/lucide-icons/lucide, commit
// 7faf87628a1ad6175975bc595344337ce14a7db4), ISC License; see NOTICE. Drawn as the Mac's is
// (mac/host.mm): a light line a point wide, over a dark fill, with a dark edge either side so it
// shows on any page.
const CAMERA_BODY: &str = "M13.997 4a2 2 0 0 1 1.76 1.05l.486.9A2 2 0 0 0 18.003 7H20a2 2 0 0 1 2 2v9\
    a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h1.997a2 2 0 0 0 1.759-1.048l.489-.904\
    A2 2 0 0 1 10.004 4z";

/// The camera pointer, 24 points square at `scale`, with its hot spot in the middle as the Mac's.
fn camera_picture(scale: f32, cx: &App) -> Option<gpui_linux::CursorPicture> {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" \
         viewBox=\"0 0 24 24\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\
         <path d=\"{CAMERA_BODY}\" fill=\"rgb(30,41,48)\" stroke=\"rgba(0,0,0,0.6)\" \
         stroke-width=\"3\"/>\
         <circle cx=\"12\" cy=\"13\" r=\"3\" fill=\"none\" stroke=\"rgba(0,0,0,0.6)\" \
         stroke-width=\"3.5\"/>\
         <path d=\"{CAMERA_BODY}\" fill=\"none\" stroke=\"rgb(215,215,215)\" stroke-width=\"1\"/>\
         <circle cx=\"12\" cy=\"13\" r=\"3\" fill=\"none\" stroke=\"white\" \
         stroke-width=\"1.5\"/></svg>"
    );
    let renderer = cx.svg_renderer();
    let parsed = renderer.parse_svg(svg.as_bytes()).ok()?;
    let side = (24. * scale).round().max(1.) as u32;
    let pixels = DevicePixels(side as i32);
    let drawn = (renderer.render_parsed(&parsed, SvgSize::ExactSize(size(pixels, pixels)))).ok()?;
    let mut rgba = drawn.as_bytes(0)?.to_vec();
    // GPUI's are BGRA.
    rgba.chunks_exact_mut(4).for_each(|pixel| pixel.swap(0, 2));
    Some(gpui_linux::CursorPicture {
        width: side,
        height: side,
        rgba,
        hot: (side / 2, side / 2),
        scale,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_capture() {
    hide(Kind::Capture);
}
