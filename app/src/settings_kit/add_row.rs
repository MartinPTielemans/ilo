//! The "+ Label" row (#4092 › Controls: adding to a list): the last row in the list's card, the
//! plus and its words, such as "Add Site". A short form opens as a popover under it, one field and
//! Add; a longer one as a dialog (`form`). It replaces Add Site…'s field in the list.

use gpui::{SharedString, div, prelude::*, px, svg};

use crate::settings_rows::{Look, frame, named_button, ring};
use crate::tokens::settings::kit::{ADD_GAP, ADD_GLYPH};

/// The row: the plus, then `label`, in the rows' ink, lit under the pointer as a list's row. ⇥
/// reaches it, and a click or Space or Return is its caller's, which opens the popover or dialog.
pub fn draw(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    let label: SharedString = label.into();
    let hovered = look.hovered;
    let plus = svg()
        .flex_none()
        .path("icons/plus.svg")
        .size(px(ADD_GLYPH))
        .text_color(look.ink);
    frame()
        .id(id)
        .relative()
        .gap(px(ADD_GAP))
        .when(a11y, |row| named_button(row, label.clone()))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
        .hover(move |row| row.bg(hovered))
        // Last in its card, so its lit plate keeps to the card's lower corners.
        .rounded_b(px(crate::tokens::radius::ROW))
        .child(plus)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(label),
        )
}
