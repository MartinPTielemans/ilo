//! The link row (#4092 › Controls: a page of Chromium's): a row named for the page, ↗ at its end,
//! which opens the page in the browser through `Host::open`. It replaces the foot's "Chromium
//! settings" button.

use gpui::{SharedString, prelude::*, px, svg};

use crate::settings_rows::{Look, frame, named_button, ring, text};
use crate::tokens::settings::kit::LINK_GLYPH;
use crate::tokens::shell::ICON_INK;

/// The row: `name` and its `line`, ↗ at its end, the whole row lit under the pointer. ⇥ reaches
/// it, and a click or Space or Return is its caller's.
pub fn draw(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    name: impl Into<SharedString>,
    line: Option<SharedString>,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    let name: SharedString = name.into();
    let hovered = look.hovered;
    let arrow = svg()
        .flex_none()
        .ml(px(crate::tokens::settings::END_GAP))
        .path("icons/arrow-up-right.svg")
        .size(px(LINK_GLYPH))
        .text_color(look.ink.opacity(ICON_INK));
    frame()
        .id(id)
        .relative()
        .when(a11y, |row| named_button(row, name.clone()))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
        .hover(move |row| row.bg(hovered))
        .child(text(look, name, line))
        .child(arrow)
}
