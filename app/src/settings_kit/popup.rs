//! The pop-up (#4092 › Controls: one of a list): a button whose label is the value, with ⌃⌄ at
//! its end, opening a menu over it with a ✓ on the value. A choice may carry an icon, on the button
//! and in the menu. It replaces Settings' segmented bars.

use std::rc::Rc;

use gpui::{App, Pixels, Point, SharedString, div, prelude::*, px, svg};

use super::{Choice, Host};
use crate::settings_rows::{Look, named_popup, ring};
use crate::shell::MenuEntry;
use crate::tokens::settings::{
    CHEVRONS, CHEVRONS_ALPHA, CHEVRONS_END, CHEVRONS_GAP, CONTROL_HEIGHT, CONTROL_RADIUS,
    FOLDER_GLYPH, POPUP_PADDING,
};
use crate::tokens::shell::ICON_INK;

/// The button's words: the chosen value's, or none if it's none of `choices`.
pub fn label<T: PartialEq>(choices: &[Choice<T>], chosen: &T) -> SharedString {
    (choices.iter())
        .find(|choice| choice.value == *chosen)
        .map(|choice| choice.label.clone())
        .unwrap_or_default()
}

/// The chosen value's icon, which the button shows before its words.
pub fn icon<T: PartialEq>(choices: &[Choice<T>], chosen: &T) -> Option<&'static str> {
    (choices.iter())
        .find(|choice| choice.value == *chosen)
        .and_then(|choice| choice.icon)
}

/// The menu: a row a choice, the chosen one with its ✓ and opening over the button's words.
pub fn items<T: PartialEq>(choices: &[Choice<T>], chosen: &T) -> Vec<MenuEntry> {
    (choices.iter())
        .map(|choice| {
            let on = choice.value == *chosen;
            let entry = MenuEntry {
                checked: on,
                over: on,
                ..MenuEntry::named(choice.label.clone(), true)
            };
            match choice.icon {
                Some(icon) => entry.icon(icon),
                None => entry,
            }
        })
        .collect()
}

/// Opens the menu of `choices` at `at` through `host`. Once it closes, `set` has the value picked,
/// only when it isn't `chosen` already, so a pick sets exactly its one setting; `closed` is called
/// however it closed.
pub fn open<T: PartialEq + Clone + 'static>(
    host: &dyn Host,
    at: Point<Pixels>,
    choices: Vec<Choice<T>>,
    chosen: T,
    set: impl FnOnce(T, &mut App) + 'static,
    closed: impl FnOnce(&mut App) + 'static,
    cx: &mut App,
) {
    let items = items(&choices, &chosen);
    let picked = crate::menu::picked(move |picked, cx| {
        let value = picked
            .and_then(|at| choices.get(at))
            .map(|c| c.value.clone());
        if let Some(value) = value.filter(|value| *value != chosen) {
            set(value, cx);
        }
        closed(cx);
    });
    host.menu(at, items, picked, cx);
}

/// The button, `width` wide: `icon` and `label`, ⌃⌄ at its end, lit under the pointer and pressed
/// while its menu is `open`. ⇥ reaches it, a press or Space or Return calls `press`, and an
/// assistive app, while one asks (`a11y`), reads it by its row's `spoken` name with `label` its
/// value.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    spoken: impl Into<SharedString>,
    label: SharedString,
    icon: Option<&'static str>,
    width: f32,
    open: bool,
    a11y: bool,
    press: super::Press,
) -> gpui::Stateful<gpui::Div> {
    let chevrons = svg()
        .flex_none()
        .path("icons/chevrons-up-down.svg")
        .size(px(CHEVRONS))
        .text_color(look.ink.opacity(CHEVRONS_ALPHA));
    let icon = icon.map(|icon| {
        svg()
            .flex_none()
            .path(icon)
            .size(px(FOLDER_GLYPH))
            .text_color(look.ink.opacity(ICON_INK))
    });
    let words = div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(CHEVRONS_GAP))
        .children(icon)
        .child(div().min_w_0().truncate().child(label.clone()));
    let (hovered, pressed) = (look.hovered, look.pressed);
    let clicked = press.clone();
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(CONTROL_HEIGHT))
        .pl(px(POPUP_PADDING))
        .pr(px(CHEVRONS_END))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(CHEVRONS_GAP))
        .rounded(px(CONTROL_RADIUS))
        .bg(if open { pressed } else { look.control })
        .when(!open, |popup| popup.hover(move |popup| popup.bg(hovered)))
        .when(a11y, |control| named_popup(control, spoken, label))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
        .child(words)
        .child(chevrons)
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            press(window, cx)
        })
        .on_click(move |event, window, cx| {
            if event.is_keyboard() {
                clicked(window, cx)
            }
        })
}

/// Where a control's menu opens: under it, at its start, as `at` keeps it once it has drawn. Put it
/// in the control, which is relative.
pub fn under(at: Rc<std::cell::Cell<Point<Pixels>>>) -> gpui::Canvas<()> {
    gpui::canvas(
        |_, _, _| {},
        move |bounds, _, _, _| {
            at.set(gpui::point(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height,
            ))
        },
    )
    .absolute()
    .inset_0()
}
