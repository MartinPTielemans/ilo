//! The hero card (#4092 › Controls: a feature page's switch): on top of a feature's page, its
//! title, one line, the switch or button that turns it on, and a drawing at the right, drawn in
//! code by the page.

use gpui::{AnyElement, SharedString, div, prelude::*, px};

use crate::settings_rows::{Look, detail_type};
use crate::tokens::radius;
use crate::tokens::settings::kit::{
    HERO_DRAWING, HERO_GAP, HERO_HEIGHT, HERO_PADDING, HERO_TITLE, HERO_TITLE_LINE,
};
use crate::tokens::settings::{BUTTONS_TOP, DETAIL_TOP};

/// The card: `title` and `line` at its left with `control` under them, and `drawing` in its own
/// room at the right, on a group's fill.
pub fn draw(
    look: &Look,
    title: impl Into<SharedString>,
    line: impl Into<SharedString>,
    control: AnyElement,
    drawing: AnyElement,
) -> gpui::Div {
    let words = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(HERO_TITLE))
                .line_height(px(HERO_TITLE_LINE))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title.into()),
        )
        .child(
            detail_type()
                .mt(px(DETAIL_TOP))
                .text_color(look.detail)
                .child(line.into()),
        )
        .child(div().mt(px(BUTTONS_TOP)).flex().child(control));
    div()
        .min_h(px(HERO_HEIGHT))
        .p(px(HERO_PADDING))
        .flex()
        .items_center()
        .gap(px(HERO_GAP))
        .rounded(px(radius::ROW))
        .bg(look.surface)
        .when_some(look.edge, |card, edge| card.border_1().border_color(edge))
        .text_color(look.ink)
        .child(words)
        .child(div().flex_none().w(px(HERO_DRAWING)).child(drawing))
}
