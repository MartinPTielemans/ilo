//! Point (#2352, browser::point) between the core and the app: what's under the pointer and the
//! element clicked come back to the window of their tab, and Point ending without the app asking
//! goes to the Agents, whose ask goes unanswered. The core's calls are made from the Engine.

use browser::point;
use gpui::{Bounds, point as at, px, size};

use super::*;

/// The element under the pointer in `tab`'s Point, or nothing there (a box 0 wide).
pub(crate) unsafe extern "C" fn tab_point_hover(
    _: *mut c_void,
    tab: bc::bc_id,
    element: *const bc::bc_arrange_element,
    rect: bc::bc_rect,
) {
    let text = |ptr: *const c_char| match ptr.is_null() {
        true => String::new(),
        false => unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    };
    let hover = (!element.is_null() && rect.width > 0.).then(|| {
        let element = unsafe { &*element };
        let label = point::label(&text(element.kind), &text(element.name));
        let frame = Bounds::new(
            at(px(rect.x as f32), px(rect.y as f32)),
            size(px(rect.width as f32), px(rect.height as f32)),
        );
        (label.into(), frame)
    });
    // After the update under way, as the core may report from inside one.
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.point_hover(tab, hover, cx));
        }
    });
}

/// The user clicked an element in `tab`'s Point: all that goes to the agent if they send it.
pub(crate) unsafe extern "C" fn tab_point_picked(
    _: *mut c_void,
    tab: bc::bc_id,
    element: *const bc::bc_point_element,
) {
    if element.is_null() {
        return;
    }
    // Copied now, as it lasts only for the call.
    let element = unsafe { point::from_core(&*element) };
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.point_picked(tab, element, cx));
        }
    });
}

/// Point ended on `tab` without the app asking: a new page, the tab moved or closed.
pub(crate) unsafe extern "C" fn tab_point_ended(_: *mut c_void, tab: bc::bc_id) {
    log!("browser: point ended in {tab}");
    agents_core::update(move |agents, cx| agents.point_ended(tab, cx));
}
