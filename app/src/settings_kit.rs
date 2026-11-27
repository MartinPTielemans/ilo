//! Settings' control kit (#4092 › Controls): one form for each kind of choice, built once here and
//! taken by every page. A pop-up picks one of a list, a checkbox drop-down several of a set, drawn
//! tiles a look or a place, a ⋯ menu acts on one row, a "+ Label" row adds to a list through a
//! popover or a dialog form, a ↗ row opens a page of Chromium's, ⓘ says more, and a hero card heads
//! a feature's page. What each decides is plain code the tests drive; what it asks of the window,
//! a menu, an alert or a page opened, goes through `Host`. The look is the rows' (`Look`): only the
//! kind of control is the reference's.

use std::rc::Rc;

use gpui::{App, Pixels, Point, Rgba, SharedString, div, prelude::*};

use crate::menu;
use crate::settings_rows::Look;
use crate::shell::MenuEntry;

pub mod add_row;
pub mod checks;
pub mod form;
pub mod help;
pub mod hero;
pub mod link_row;
pub mod popup;
pub mod row_menu;
pub mod tiles;

#[cfg(test)]
mod tests;

/// What the kit asks of the window its controls are in.
pub trait Host {
    /// Shows `items` in ilo's menu with its top left `at`, in the window's content, and calls
    /// `chosen` once it closes.
    fn menu(&self, at: Point<Pixels>, items: Vec<MenuEntry>, chosen: menu::Chosen, cx: &mut App);
    /// Asks `alert`, and calls `confirmed` only if the user chooses its action.
    fn alert(&self, alert: Alert, confirmed: Box<dyn FnOnce(&mut App)>, cx: &mut App);
    /// Opens `url`, such as a page of Chromium's settings, in the browser.
    fn open(&self, url: &str, cx: &mut App);
}

/// What an alert asks before something that can't be taken back: its message, the line under it,
/// and the action's button, in red, beside Cancel.
#[derive(Clone, Debug, PartialEq)]
pub struct Alert {
    pub message: SharedString,
    pub detail: SharedString,
    pub action: SharedString,
}

impl Alert {
    pub fn new(
        message: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        action: impl Into<SharedString>,
    ) -> Self {
        Alert {
            message: message.into(),
            detail: detail.into(),
            action: action.into(),
        }
    }
}

/// One of a pop-up's, a checkbox drop-down's or a row of tiles' choices: the value it sets, its
/// words, and the icon before them, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice<T> {
    pub value: T,
    pub label: SharedString,
    pub icon: Option<&'static str>,
}

impl<T> Choice<T> {
    pub fn new(value: T, label: impl Into<SharedString>) -> Self {
        Choice {
            value,
            label: label.into(),
            icon: None,
        }
    }

    /// With `icon` before its words, in the menu and on the button.
    pub fn icon(self, icon: &'static str) -> Self {
        Choice {
            icon: Some(icon),
            ..self
        }
    }
}

/// What opens a control's menu, as a pop-up button opens on a press and on Space or Return.
pub type Press = Rc<dyn Fn(&mut gpui::Window, &mut App)>;

/// The red a destroying or disconnecting action's words are in, the menu's own, as dark or light
/// as the look: the only red in Settings, and never a fill.
pub fn red(look: &Look) -> Rgba {
    gpui::rgba(match look.light() {
        true => crate::tokens::menu::LIGHT_DESTRUCTIVE,
        false => crate::tokens::menu::DARK_DESTRUCTIVE,
    })
}

/// A push button that destroys, such as Delete All: a button's plate, its words in `red`.
pub fn red_button(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    crate::settings_rows::stop_button(look, id, title, a11y)
        .relative()
        .text_color(red(look))
}

/// A control's frame that logs where it's drawn, for check.sh's `click_placed`: `what` names it,
/// and the log line is the rows' (`settings_rows::drawn`).
pub fn placed(what: String, logged: &crate::settings_rows::Drawn) -> gpui::Canvas<()> {
    crate::settings_rows::drawn(what, logged.clone())
}

/// A row's text with `after` beside its name, as ⓘ follows a label: the name, then `after`, and
/// the line under them.
pub fn text_with(
    look: &Look,
    name: impl Into<SharedString>,
    after: gpui::AnyElement,
    line: Option<SharedString>,
) -> gpui::Div {
    use crate::tokens::settings::kit::HELP_GAP;
    use crate::tokens::settings::{DETAIL_TOP, LINE_ROW_HEIGHT, PADDING};
    let lined = line.is_some();
    let head = div()
        .flex()
        .items_center()
        .gap(gpui::px(HELP_GAP))
        .min_w_0()
        .child(
            div()
                .truncate()
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(name.into()),
        )
        .child(after);
    div()
        .flex_1()
        .min_w_0()
        .when(lined, |text| {
            text.min_h(gpui::px(LINE_ROW_HEIGHT - 2. * PADDING))
                .flex()
                .flex_col()
                .justify_center()
        })
        .child(head)
        .children(line.map(|line| {
            crate::settings_rows::detail_type()
                .mt(gpui::px(DETAIL_TOP))
                .text_color(look.detail)
                .child(line)
        }))
}
