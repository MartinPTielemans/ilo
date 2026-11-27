//! The checkbox drop-down (#4092 › Controls: several of a set): a button whose label sums up the
//! set ("Everything", or the picks' words joined), with ∨ at its end, opening a menu of rows that
//! each tick or untick one member and leave the menu open.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, Pixels, Point, SharedString, div, prelude::*, px, svg};

use super::{Choice, Host};
use crate::settings_rows::{Look, named_popup, ring};
use crate::shell::MenuEntry;
use crate::tokens::settings::kit::DROP_CHEVRON;
use crate::tokens::settings::{
    CHEVRONS_ALPHA, CHEVRONS_END, CHEVRONS_GAP, CONTROL_HEIGHT, CONTROL_RADIUS, POPUP_PADDING,
};

/// The button's words for the members `on`: `all` with every one on, `none` with none, else their
/// words in the choices' order, joined by commas.
pub fn summary<T: PartialEq>(
    choices: &[Choice<T>],
    on: &[T],
    all: &str,
    none: &str,
) -> SharedString {
    let picked: Vec<&str> = (choices.iter())
        .filter(|choice| on.contains(&choice.value))
        .map(|choice| choice.label.as_ref())
        .collect();
    match picked.len() {
        0 => none.to_owned().into(),
        count if count == choices.len() => all.to_owned().into(),
        _ => picked.join(", ").into(),
    }
}

/// The menu: a row a member, ticked while it's on, which `toggle` flips by its index and answers
/// whether it's on now; the menu stays open.
pub fn items<T: PartialEq>(
    choices: &[Choice<T>],
    on: &[T],
    toggle: Rc<dyn Fn(usize, &mut App) -> bool>,
) -> Vec<MenuEntry> {
    (choices.iter().enumerate())
        .map(|(index, choice)| {
            let toggle = toggle.clone();
            let entry = MenuEntry {
                checked: on.contains(&choice.value),
                ..MenuEntry::named(choice.label.clone(), true)
            };
            let entry = entry.stays(move |cx| toggle(index, cx));
            match choice.icon {
                Some(icon) => entry.icon(icon),
                None => entry,
            }
        })
        .collect()
}

/// Opens the menu of `choices` at `at` through `host`, the members `on` ticked. Each row chosen
/// flips its one member: `set` has that member and whether it's on now, and nothing else changes.
/// `closed` is called once the menu closes.
pub fn open<T: PartialEq + Clone + 'static>(
    host: &dyn Host,
    at: Point<Pixels>,
    choices: Vec<Choice<T>>,
    on: Vec<T>,
    set: Rc<dyn Fn(T, bool, &mut App)>,
    closed: impl FnOnce(&mut App) + 'static,
    cx: &mut App,
) {
    let ticked: Vec<bool> = (choices.iter())
        .map(|choice| on.contains(&choice.value))
        .collect();
    let ticked = Rc::new(RefCell::new(ticked));
    let values: Vec<T> = choices.iter().map(|choice| choice.value.clone()).collect();
    let toggle: Rc<dyn Fn(usize, &mut App) -> bool> = Rc::new(move |index, cx| {
        let now = {
            let mut ticked = ticked.borrow_mut();
            let Some(tick) = ticked.get_mut(index) else {
                return false;
            };
            *tick = !*tick;
            *tick
        };
        set(values[index].clone(), now, cx);
        now
    });
    let items = items(&choices, &on, toggle);
    host.menu(at, items, Box::new(move |_, cx: &mut App| closed(cx)), cx);
}

/// The button: its `summary` and ∨, lit as a pop-up is, pressed while its menu is `open`. ⇥
/// reaches it, a press or Space or Return calls `press`, and an assistive app reads it by its
/// row's `spoken` name with the summary its value.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    spoken: impl Into<SharedString>,
    summary: SharedString,
    width: f32,
    open: bool,
    a11y: bool,
    press: super::Press,
) -> gpui::Stateful<gpui::Div> {
    let chevron = svg()
        .flex_none()
        .path("icons/chevron-down.svg")
        .size(px(DROP_CHEVRON))
        .text_color(look.ink.opacity(CHEVRONS_ALPHA));
    let (hovered, pressed) = (look.hovered, look.pressed);
    let clicked = press.clone();
    div()
        .id(id)
        .relative()
        .flex_none()
        .max_w(px(width))
        .h(px(CONTROL_HEIGHT))
        .pl(px(POPUP_PADDING))
        .pr(px(CHEVRONS_END))
        .flex()
        .items_center()
        .gap(px(CHEVRONS_GAP))
        .rounded(px(CONTROL_RADIUS))
        .bg(if open { pressed } else { look.control })
        .when(!open, |drop| drop.hover(move |drop| drop.bg(hovered)))
        .when(a11y, |control| {
            named_popup(control, spoken, summary.clone())
        })
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
        .child(div().min_w_0().truncate().child(summary))
        .child(chevron)
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
