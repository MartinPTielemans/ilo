//! The keyboard's ring, one for every control in the app (#4108): `shell.keyed-ring` wide, in the
//! ink the control is drawn in at `shell.keyed`'s strength, while the control has the keyboard
//! from a key. A click shows none. Lifted from Settings' rows (settings_rows `ring` and
//! `ring_round`, which re-export it), the copy that's only a style and reads only the ring tokens.
//!
//! It's a style, so it never requests focus or tracks it: GPUI's `focus_visible` shows it while
//! the element has the keyboard, and focus stays the surface's act.
//!
//! How to compose it: give the control an id, a `tab_index` (and `focus_from_keys_only` if a
//! click mustn't take the keyboard), its corner radius, then `.focus_visible(ring(ink))`, or
//! `ring_round(ink)` where the control's own plate fills it and would cover a ring inside.

use gpui::{BoxShadow, Rgba, StyleRefinement, point, prelude::*, px};

use crate::tokens::shell::{KEYED, KEYED_RING};

/// The ring inside the control's plate, as Focus Sidebar's is.
pub fn ring(ink: Rgba) -> impl FnOnce(StyleRefinement) -> StyleRefinement {
    drawn(ink, true)
}

/// The same ring round a control whose own plate fills it, as a switch's pill does: just outside
/// its edge, as Site Controls ring a round control.
pub fn ring_round(ink: Rgba) -> impl FnOnce(StyleRefinement) -> StyleRefinement {
    drawn(ink, false)
}

fn drawn(ink: Rgba, inset: bool) -> impl FnOnce(StyleRefinement) -> StyleRefinement {
    let color = Rgba { a: KEYED, ..ink };
    move |style| {
        style.shadow(vec![BoxShadow {
            color: color.into(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(KEYED_RING),
            inset,
        }])
    }
}
