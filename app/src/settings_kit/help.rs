//! ⓘ (#4092 › Controls: more explanation): after a heading or a label, one or two sentences that
//! show while the pointer is on it or it has the keyboard, on a plate under it. Longer help is an
//! inline "ⓘ How …?" link that opens a dialog (`form::Form` with only words).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{App, FocusHandle, SharedString, Window, div, prelude::*, px, rgba, svg};

use crate::settings_rows::{Look, detail_type, named_button, ring_round};
use crate::tokens::settings::kit::{HELP_GLYPH, HELP_PADDING, HELP_TOP, HELP_WIDTH};
use crate::tokens::shell::ICON_INK;

/// Which ⓘ says its words: the one under the pointer, else the one with the keyboard. A view keeps
/// one, and calls `frame` as it renders, where it can see the keyboard.
#[derive(Default, Clone)]
pub struct Helps(Rc<HelpState>);

#[derive(Default)]
struct HelpState {
    hovered: RefCell<Option<SharedString>>,
    focused: RefCell<Option<SharedString>>,
    handles: RefCell<HashMap<SharedString, FocusHandle>>,
}

impl Helps {
    /// Notes which ⓘ has the keyboard now; a view calls it before it draws its rows.
    pub fn frame(&self, window: &Window) {
        let focused = (self.0.handles.borrow().iter())
            .find(|(_, handle)| handle.is_focused(window))
            .map(|(id, _)| id.clone());
        *self.0.focused.borrow_mut() = focused;
    }

    /// The pointer came on to `id`'s ⓘ, or left it.
    pub fn hover(&self, id: &SharedString, on: bool) {
        let mut hovered = self.0.hovered.borrow_mut();
        match on {
            true => *hovered = Some(id.clone()),
            false if hovered.as_ref() == Some(id) => *hovered = None,
            false => {}
        }
    }

    /// The ⓘ whose words show: the pointer's, else the keyboard's.
    pub fn shown(&self) -> Option<SharedString> {
        (self.0.hovered.borrow().clone()).or_else(|| self.0.focused.borrow().clone())
    }

    fn handle(&self, id: &SharedString, cx: &mut App) -> FocusHandle {
        (self.0.handles.borrow_mut())
            .entry(id.clone())
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }
}

/// ⓘ, `id` among the view's, which shows `words` under it while the pointer is on it or it has
/// the keyboard. An assistive app reads it as a button named by its words.
pub fn icon(
    look: &Look,
    helps: &Helps,
    id: impl Into<SharedString>,
    words: impl Into<SharedString>,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let (id, words): (SharedString, SharedString) = (id.into(), words.into());
    let shown = helps.shown().as_ref() == Some(&id);
    let handle = helps.handle(&id, cx);
    let fill = rgba(match look.light() {
        true => crate::menu::LIGHT_FILL,
        false => crate::menu::DARK_FILL,
    });
    let plate = shown.then(|| {
        detail_type()
            .absolute()
            .top(px(HELP_GLYPH + HELP_TOP))
            .left_0()
            .w(px(HELP_WIDTH))
            .p(px(HELP_PADDING))
            .rounded(px(crate::tokens::settings::CONTROL_RADIUS))
            .bg(fill)
            .when_some(look.edge, |plate, edge| plate.border_1().border_color(edge))
            .text_color(look.ink)
            .font_weight(gpui::FontWeight::NORMAL)
            .child(words.clone())
    });
    let hovering = helps.clone();
    let hovered = id.clone();
    div()
        .id(SharedString::from(format!("help-{id}")))
        .relative()
        .flex_none()
        .size(px(HELP_GLYPH))
        .track_focus(&handle)
        .tab_index(0)
        .focus_from_keys_only()
        .rounded_full()
        .focus_visible(ring_round(look.ink))
        .map(|help| named_button(help, words))
        .on_hover(move |on, window, _| {
            hovering.hover(&hovered, *on);
            window.refresh();
        })
        .child(
            svg()
                .path("icons/info.svg")
                .size(px(HELP_GLYPH))
                .text_color(look.ink.opacity(ICON_INK)),
        )
        // Painted after the rows under it, so they don't cover it.
        .children(plate.map(gpui::deferred))
}

/// The inline "ⓘ How …?" link: ⓘ and `words`, in the rows' grey, which a click or Space or Return
/// gives its caller, to open the dialog with the longer help.
pub fn how(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    words: impl Into<SharedString>,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    let words: SharedString = words.into();
    let ink = look.ink;
    detail_type()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(crate::tokens::settings::kit::HELP_GAP))
        .text_color(look.detail)
        .hover(move |link| link.text_color(ink))
        .when(a11y, |link| named_button(link, words.clone()))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(crate::settings_rows::ring(look.ink))
        .child(
            svg()
                .flex_none()
                .path("icons/info.svg")
                .size(px(HELP_GLYPH))
                .text_color(look.detail),
        )
        .child(words)
}
