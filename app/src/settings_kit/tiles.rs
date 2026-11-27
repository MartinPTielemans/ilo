//! Drawn tiles (#4092 › Controls: a look or a place): a row of small drawings, made in code by
//! the page, each with its words under it, the pick ringed. ← and → move the pick, as a row of
//! radio buttons does.

use std::rc::Rc;

use gpui::{AnyElement, App, SharedString, accesskit::Role, div, prelude::*, px};

use super::Choice;
use crate::settings_rows::{Look, detail_type};
use crate::tokens::settings::kit::{
    TILE_GAP, TILE_HEIGHT, TILE_LABEL_TOP, TILE_RADIUS, TILE_RING, TILE_RING_GAP, TILE_WIDTH,
};

/// Where ← or → (`key`) moves the pick from `at` among `count` tiles: to the one beside, none at
/// either end, and none for another key.
pub fn moved(key: &str, at: usize, count: usize) -> Option<usize> {
    match key {
        "left" if at > 0 => Some(at - 1),
        "right" if at + 1 < count => Some(at + 1),
        _ => None,
    }
}

/// The row: a tile a choice, its drawing from `draw` with the choice's words under it, the one
/// `chosen` ringed in the ink. A click on one, or ← or → on the row, calls `pick` with its value,
/// only when it isn't the one chosen. An assistive app reads them as radio buttons.
pub fn draw<T: PartialEq + Clone + 'static>(
    look: &Look,
    id: impl Into<SharedString>,
    choices: &[Choice<T>],
    chosen: &T,
    a11y: bool,
    drawing: impl Fn(&Choice<T>) -> AnyElement,
    pick: Rc<dyn Fn(T, &mut App)>,
) -> gpui::Div {
    let id: SharedString = id.into();
    let values: Rc<Vec<T>> = Rc::new(choices.iter().map(|choice| choice.value.clone()).collect());
    let at = values.iter().position(|value| value == chosen).unwrap_or(0);
    let tiles = choices.iter().enumerate().map(|(index, choice)| {
        let on = choice.value == *chosen;
        let (pick, keys) = (pick.clone(), pick.clone());
        let value = choice.value.clone();
        let values = values.clone();
        let ring = div()
            .p(px(TILE_RING_GAP))
            .rounded(px(TILE_RADIUS + TILE_RING_GAP + TILE_RING))
            .border(px(TILE_RING))
            .border_color(match on {
                true => look.ink,
                false => gpui::transparent_black().into(),
            })
            .child(
                div()
                    .w(px(TILE_WIDTH))
                    .h(px(TILE_HEIGHT))
                    .rounded(px(TILE_RADIUS))
                    .overflow_hidden()
                    .child(drawing(choice)),
            );
        div()
            .id((id.clone(), index))
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .when(a11y, |tile| {
                use gpui::accesskit::Toggled;
                let toggled = if on { Toggled::True } else { Toggled::False };
                tile.role(Role::RadioButton)
                    .aria_label(choice.label.clone())
                    .aria_toggled(toggled)
            })
            .tab_index(0)
            .focus_from_keys_only()
            .focus_visible(crate::settings_rows::ring(look.ink))
            .on_key_down(move |event, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let key = event.keystroke.key.as_str();
                if let Some(to) = moved(key, at, values.len()) {
                    cx.stop_propagation();
                    keys(values[to].clone(), cx);
                }
            })
            .on_click(move |_, _, cx| {
                if !on {
                    pick(value.clone(), cx)
                }
            })
            .child(ring)
            .child(
                detail_type()
                    .mt(px(TILE_LABEL_TOP))
                    .text_color(if on { look.ink } else { look.detail })
                    .child(choice.label.clone()),
            )
    });
    let tiles: Vec<_> = tiles.collect();
    div().flex().gap(px(TILE_GAP)).children(tiles)
}
