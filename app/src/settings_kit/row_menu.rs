//! The ⋯ menu on a list's row (#4092 › Controls: per-item actions): what can be done to the row,
//! the destroying or disconnecting ones last, after a line, in red. One that can't be taken back
//! asks in an alert first. It replaces the lone Remove and Revoke buttons on list rows.

use std::rc::Rc;

use gpui::{App, Pixels, Point, SharedString, prelude::*};

use super::{Alert, Host};
use crate::settings_rows::{Look, icon_button, named_popup};
use crate::shell::MenuEntry;

/// One of the menu's actions: its words, whether it destroys or disconnects, the alert it asks
/// first, if any, and what it does.
#[derive(Clone)]
pub struct Action {
    pub label: SharedString,
    pub destructive: bool,
    pub ask: Option<Alert>,
    pub run: Rc<dyn Fn(&mut App)>,
}

impl Action {
    pub fn new(label: impl Into<SharedString>, run: impl Fn(&mut App) + 'static) -> Self {
        Action {
            label: label.into(),
            destructive: false,
            ask: None,
            run: Rc::new(run),
        }
    }

    /// One that destroys or disconnects, last in red, asking `ask` first where it can't be taken
    /// back.
    pub fn destructive(self, ask: Option<Alert>) -> Self {
        Action {
            destructive: true,
            ask,
            ..self
        }
    }
}

/// The menu's rows, the plain actions first, then a line and the destructive ones, with the index
/// of the action each row stands for, none for the line.
pub fn items(actions: &[Action]) -> (Vec<MenuEntry>, Vec<Option<usize>>) {
    let plain = (actions.iter().enumerate()).filter(|(_, action)| !action.destructive);
    let red: Vec<_> = (actions.iter().enumerate())
        .filter(|(_, action)| action.destructive)
        .collect();
    let mut entries = Vec::new();
    let mut stands = Vec::new();
    for (index, action) in plain {
        entries.push(MenuEntry::named(action.label.clone(), true));
        stands.push(Some(index));
    }
    if !red.is_empty() && !entries.is_empty() {
        entries.push(MenuEntry::line());
        stands.push(None);
    }
    for (index, action) in red {
        entries.push(MenuEntry {
            destructive: true,
            ..MenuEntry::named(action.label.clone(), true)
        });
        stands.push(Some(index));
    }
    (entries, stands)
}

/// Does `action`, asking its alert through `host` first if it has one, and doing it only on yes.
pub fn choose(host: &Rc<dyn Host>, action: &Action, cx: &mut App) {
    let run = action.run.clone();
    match &action.ask {
        Some(alert) => host.alert(alert.clone(), Box::new(move |cx| run(cx)), cx),
        None => run(cx),
    }
}

/// Opens the menu of `actions` at `at` through `host`; once it closes, the action chosen is done
/// as `choose` does it, and `closed` is called however it closed.
pub fn open(
    host: Rc<dyn Host>,
    at: Point<Pixels>,
    actions: Vec<Action>,
    closed: impl FnOnce(&mut App) + 'static,
    cx: &mut App,
) {
    let (entries, stands) = items(&actions);
    let asking = host.clone();
    let picked = crate::menu::picked(move |picked, cx| {
        closed(cx);
        let action = picked
            .and_then(|at| stands.get(at).copied().flatten())
            .and_then(|index| actions.get(index));
        if let Some(action) = action {
            choose(&asking, action, cx);
        }
    });
    host.menu(at, entries, picked, cx);
}

/// The ⋯ button at a row's end, a control's square, pressed while its menu is `open`. ⇥ reaches
/// it, a press or Space or Return calls `press`, and an assistive app reads it by `spoken`, the
/// row's name and "options", as a menu button.
pub fn draw(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    spoken: impl Into<SharedString>,
    open: bool,
    a11y: bool,
    press: super::Press,
) -> gpui::Stateful<gpui::Div> {
    let clicked = press.clone();
    let spoken: SharedString = spoken.into();
    icon_button(look, id, "icons/ellipsis.svg")
        .relative()
        .when(open, |button| button.bg(look.pressed))
        .when(a11y, |button| named_popup(button, spoken, ""))
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
