//! Settings › Tabs & Sidebar › Sidebar position (#2407): two drawings of the window, the sidebar
//! on the left in one and on the right in the other, each with the window's buttons where that
//! side puts them on this OS. A click on one picks it, and so do ← and → while the pair has the
//! keyboard; an assistive app reads them as a pair of options with one chosen.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, BoxShadow, FontWeight, KeyDownEvent, Rgba, Window,
    accesskit::{Action, Role, Toggled},
    div, point,
    prelude::*,
    px, rgb, svg,
};

use crate::settings_rows::{Look, drawn, ring};
use crate::tokens::settings::{
    SIDE_BAR, SIDE_BAR_FIRST, SIDE_BUTTON, SIDE_BUTTON_GAP, SIDE_BUTTONS, SIDE_GAP, SIDE_GLYPH,
    SIDE_GLYPH_GAP, SIDE_HEADING, SIDE_HEIGHT, SIDE_INSET, SIDE_LABEL_GAP, SIDE_MAC_CLOSE,
    SIDE_MAC_MINIMIZE, SIDE_MAC_ZOOM, SIDE_OTHER, SIDE_PAGE, SIDE_PAGE_LIGHT, SIDE_PAGE_PADDING,
    SIDE_PAGE_RADIUS, SIDE_PAGE_RING, SIDE_RADIUS, SIDE_RING, SIDE_RING_GAP, SIDE_ROOM,
    SIDE_ROWS_GAP, SIDE_SIDEBAR, SIDE_SIDEBAR_PADDING, SIDE_TEXT, SIDE_WIDTH, SIDE_WINDOW,
    SIDE_WINDOW_LIGHT,
};
use crate::tokens::shell::{WINDOW_BUTTON_CLOSE, WINDOW_BUTTON_MINIMIZE, WINDOW_BUTTON_ZOOM};
use crate::tokens::text::SMALL;

/// The window's buttons as this OS has them, which each drawing shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Buttons {
    /// AppKit's three: close, minimize, full screen; mirrored as a unit on the right.
    Mac,
    /// The caption group ilo draws: minimize, maximize, close in that order on either side, close
    /// at the corner on the right.
    Windows,
    /// The three ilo draws on its frameless window, mirrored as a unit on the right.
    Linux,
}

impl Buttons {
    /// This OS's.
    pub const HERE: Buttons = if cfg!(target_os = "macos") {
        Buttons::Mac
    } else if cfg!(target_os = "windows") {
        Buttons::Windows
    } else {
        Buttons::Linux
    };
}

/// A side's name, as the drawing's label and an assistive app say it.
pub fn title(right: bool) -> &'static str {
    if right { "Right" } else { "Left" }
}

/// The side a key moves the choice to, if it's ← or → alone and that side isn't chosen.
pub fn keyed(event: &KeyDownEvent, right: bool) -> Option<bool> {
    let keys = &event.keystroke;
    if keys.modifiers.modified() {
        return None;
    }
    match keys.key.as_str() {
        "left" if right => Some(false),
        "right" if !right => Some(true),
        _ => None,
    }
}

/// The pair, `right` chosen, on a row's plate in `look`, its drawings in the window's `base` ink
/// (the paper in dark, the ink in light). `pick` is called with the side picked, by a click or a
/// key, only when it isn't the one chosen. `logged` is Settings' (settings_rows::drawn), so
/// check.sh can click each drawing.
pub fn picker(
    look: &Look,
    base: Rgba,
    dark: bool,
    right: bool,
    buttons: Buttons,
    a11y: bool,
    logged: Rc<RefCell<Vec<String>>>,
    pick: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let pick = Rc::new(pick);
    let keyed_pick = pick.clone();
    let options = [false, true].map(|side| {
        let chosen = side == right;
        let pick = pick.clone();
        let label = div()
            .text_size(px(SMALL))
            .text_color(if chosen { look.ink } else { look.detail })
            .when(chosen, |label| label.font_weight(FontWeight::MEDIUM))
            .child(title(side));
        div()
            .id(("sidebar-side", usize::from(side)))
            .when(a11y, |option| {
                let toggled = if chosen {
                    Toggled::True
                } else {
                    Toggled::False
                };
                option
                    .role(Role::RadioButton)
                    .aria_label(title(side))
                    .aria_toggled(toggled)
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().add_action(Action::Click)
                    })
            })
            .flex()
            .flex_col()
            .items_center()
            .gap(px(SIDE_LABEL_GAP))
            .child(
                window(look, base, dark, side, chosen, buttons)
                    .relative()
                    .child(drawn(
                        format!("sidebar position option {}", title(side)),
                        logged.clone(),
                    )),
            )
            .child(label)
            .on_click(move |_, window, cx| {
                if !chosen {
                    pick(side, window, cx)
                }
            })
            .into_any_element()
    });
    div()
        .id("sidebar-position")
        .when(a11y, |pair| {
            pair.role(Role::RadioGroup).aria_label("Sidebar position")
        })
        .tab_index(0)
        .focus_from_keys_only()
        .rounded(px(SIDE_RADIUS + SIDE_ROOM))
        .focus_visible(ring(look.ink))
        .p(px(SIDE_ROOM))
        .flex()
        .gap(px(SIDE_GAP))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if let Some(side) = keyed(event, right) {
                cx.stop_propagation();
                keyed_pick(side, window, cx);
            }
        })
        .children(options)
}

/// One drawing: the window's material with the sidebar on `right` or not, its buttons at its top,
/// and the page's card on the other side.
fn window(
    look: &Look,
    base: Rgba,
    dark: bool,
    right: bool,
    chosen: bool,
    buttons: Buttons,
) -> gpui::Div {
    let bar = |height: f32, width: f32, alpha: f32| {
        div()
            .flex_none()
            .h(px(height))
            .w(gpui::relative(width))
            .rounded_full()
            .bg(base.opacity(alpha))
    };
    let head = div()
        .flex_none()
        .h(px(SIDE_BUTTONS))
        .flex()
        .items_center()
        .when(right, |head| head.justify_end())
        .children(window_buttons(base, right, buttons));
    let sidebar = div()
        .flex_none()
        .w(px((SIDE_WIDTH * SIDE_SIDEBAR).round()))
        .flex()
        .flex_col()
        .gap(px(SIDE_ROWS_GAP))
        .py(px(SIDE_SIDEBAR_PADDING))
        .px(px(SIDE_INSET))
        .child(head)
        .children(
            [0.8, 0.64, 0.72, 0.56, 0.68]
                .iter()
                .enumerate()
                .map(|(index, &width)| {
                    let alpha = if index == 0 { SIDE_BAR_FIRST } else { SIDE_BAR };
                    bar(4., width, alpha)
                }),
        );
    let page_fill = rgb(if dark { SIDE_PAGE } else { SIDE_PAGE_LIGHT });
    let page = div()
        .flex_1()
        .my(px(SIDE_INSET))
        .map(|page| match right {
            true => page.ml(px(SIDE_INSET)),
            false => page.mr(px(SIDE_INSET)),
        })
        .rounded(px(SIDE_PAGE_RADIUS))
        .bg(page_fill)
        .shadow(vec![spread(base.opacity(SIDE_PAGE_RING), 0.5)])
        .flex()
        .flex_col()
        .gap(px(SIDE_ROWS_GAP))
        .p(px(SIDE_PAGE_PADDING))
        .child(bar(5., 0.55, SIDE_HEADING))
        .children(
            [0.9, 0.82, 0.88, 0.6]
                .iter()
                .map(|&width| bar(3., width, SIDE_TEXT)),
        );
    // GPUI paints a later shadow over an earlier one, so the plate's gap goes over the accent.
    let ring = match chosen {
        true => vec![
            spread(look.accent, SIDE_RING_GAP + SIDE_RING),
            spread(look.surface, SIDE_RING_GAP),
        ],
        false => vec![spread(base.opacity(SIDE_OTHER), 1.)],
    };
    div()
        .flex_none()
        .w(px(SIDE_WIDTH))
        .h(px(SIDE_HEIGHT))
        .rounded(px(SIDE_RADIUS))
        .overflow_hidden()
        .bg(rgb(if dark { SIDE_WINDOW } else { SIDE_WINDOW_LIGHT }))
        .shadow(ring)
        .flex()
        .map(|window| match right {
            true => window.child(page).child(sidebar),
            false => window.child(sidebar).child(page),
        })
}

/// The window's buttons in a drawing, close at the corner on either side.
fn window_buttons(base: Rgba, right: bool, buttons: Buttons) -> Vec<AnyElement> {
    let dot = |color: u32| {
        div()
            .flex_none()
            .size(px(SIDE_BUTTON))
            .rounded_full()
            .bg(rgb(color))
    };
    let (colors, gap) = match buttons {
        Buttons::Mac => (
            [SIDE_MAC_CLOSE, SIDE_MAC_MINIMIZE, SIDE_MAC_ZOOM],
            SIDE_BUTTON_GAP,
        ),
        Buttons::Linux => (
            [
                WINDOW_BUTTON_CLOSE,
                WINDOW_BUTTON_MINIMIZE,
                WINDOW_BUTTON_ZOOM,
            ],
            SIDE_BUTTON_GAP,
        ),
        Buttons::Windows => {
            let glyphs = [
                "icons/window-windows-minimize.svg",
                "icons/window-windows-maximize.svg",
                "icons/window-windows-close.svg",
            ];
            let row = div()
                .flex()
                .gap(px(SIDE_GLYPH_GAP))
                .children(glyphs.map(|glyph| {
                    svg()
                        .flex_none()
                        .path(glyph)
                        .size(px(SIDE_BUTTON))
                        .text_color(base.opacity(SIDE_GLYPH))
                }));
            return vec![row.into_any_element()];
        }
    };
    let mut order = colors.to_vec();
    if right {
        order.reverse();
    }
    vec![
        div()
            .flex()
            .gap(px(gap))
            .children(order.into_iter().map(dot))
            .into_any_element(),
    ]
}

/// A ring `width` out from an element's edge, in `color`.
fn spread(color: Rgba, width: f32) -> BoxShadow {
    BoxShadow {
        color: color.into(),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(width),
        inset: false,
    }
}

#[cfg(test)]
mod tests;
