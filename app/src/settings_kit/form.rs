//! The dialog form and the popover (#4092 › Controls: a form, and adding to a list). A dialog form
//! is a card over the dimmed window, as an alert is: its title, its fields, then Cancel and the
//! primary, which stays dim and takes no click or ↩ until what's typed is valid. A popover is the
//! short form: one field and Add, hanging under the "+" row that opened it. Both are drawn in
//! Settings' own window, which has no page under it, and Escape or Cancel closes them changing
//! nothing.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Entity, Focusable, Pixels, Point, SharedString, Window, div, prelude::*, px,
    rgb, rgba,
};

use super::Press;
use crate::settings_rows::{Look, button, named_dimmed, primary};
use crate::shell::{CancelName, KeepName};
use crate::text_input::TextInput;
use crate::tokens::dialog::{
    BUTTON_INSET, BUTTONS_TOP, DETAIL_LINE, DETAIL_TOP, DIM, DIM_OPACITY, RADIUS, TEXT_INSET,
    TITLE_LINE, WIDTH,
};
use crate::tokens::settings::kit::{DIMMED, FORM_GAP, FORM_TOP, POPOVER_PADDING, POPOVER_WIDTH};
use crate::tokens::settings::{CONTROL_GAP, CONTROL_HEIGHT, CONTROL_RADIUS, FIELD_PADDING};

/// Where a form shows: over the middle of the window, dimmed, or under the row that opened it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    Dialog,
    Under(Point<Pixels>),
}

/// A form: its title and words, its fields, its primary's words, what makes it valid, and what's
/// done with what was typed once the primary is chosen.
pub struct Form {
    pub title: SharedString,
    /// Under the title, if anything, such as how to set something up.
    pub body: Option<SharedString>,
    pub fields: Vec<Entity<TextInput>>,
    /// The primary's words, none for a form that only tells, which has Cancel's alone.
    pub primary: Option<SharedString>,
    pub cancel: SharedString,
    pub valid: Rc<dyn Fn(&[String]) -> bool>,
    pub done: Rc<dyn Fn(Vec<String>, &mut App)>,
    pub placement: Placement,
}

impl Form {
    /// A popover under `at`: `field` and the primary `add`, valid once the field holds anything
    /// but spaces, unless `valid` says more.
    pub fn popover(
        at: Point<Pixels>,
        field: Entity<TextInput>,
        add: impl Into<SharedString>,
        done: impl Fn(String, &mut App) + 'static,
    ) -> Form {
        Form {
            title: SharedString::default(),
            body: None,
            fields: vec![field],
            primary: Some(add.into()),
            cancel: "Cancel".into(),
            valid: Rc::new(|texts: &[String]| texts.iter().all(|text| !text.trim().is_empty())),
            done: Rc::new(move |texts: Vec<String>, cx: &mut App| {
                done(texts.into_iter().next().unwrap_or_default(), cx)
            }),
            placement: Placement::Under(at),
        }
    }

    /// What its fields hold.
    pub fn texts(&self, cx: &App) -> Vec<String> {
        (self.fields.iter())
            .map(|field| field.read(cx).text().to_owned())
            .collect()
    }

    /// Whether the primary can be chosen: there is one, and what's typed is valid.
    pub fn ready(&self, cx: &App) -> bool {
        self.primary.is_some() && (self.valid)(&self.texts(cx))
    }

    /// The primary, chosen: while ready, what was typed goes to `done`, once, and it answers that
    /// the form closes; else nothing happens and it stays.
    pub fn submit(&self, cx: &mut App) -> bool {
        if !self.ready(cx) {
            return false;
        }
        let texts = self.texts(cx);
        (self.done)(texts, cx);
        true
    }

    /// Gives its first field the keyboard, as it opens.
    pub fn focus_first(&self, window: &mut Window, cx: &mut App) {
        if let Some(field) = self.fields.first() {
            let handle = field.focus_handle(cx);
            window.focus(&handle, cx);
        }
    }
}

/// A field among a form's, on a control's plate, as Settings' fields are.
fn field(look: &Look, input: &Entity<TextInput>) -> gpui::Div {
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(CONTROL_HEIGHT))
        .px(px(FIELD_PADDING))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.control)
        .child(input.clone())
}

/// The form drawn over the whole window: `cancel` on Cancel, Escape or a press outside a popover,
/// `submit` on the primary or ↩. `placed` logs where its controls are, for check.sh.
pub fn draw(
    form: &Form,
    look: &Look,
    cancel: Press,
    submit: Press,
    placed: &dyn Fn(String) -> gpui::Canvas<()>,
    cx: &App,
) -> AnyElement {
    let ready = form.ready(cx);
    let fill = rgba(match look.light() {
        true => crate::menu::LIGHT_FILL,
        false => crate::menu::DARK_FILL,
    });
    let cancel_button = {
        let cancel = cancel.clone();
        button(look, "kit-form-cancel", form.cancel.clone())
            .relative()
            .child(placed("form cancel".into()))
            .on_click(move |_, window, cx| cancel(window, cx))
    };
    let primary_button = form.primary.clone().map(|words| {
        let submit = submit.clone();
        let (hovered, pressed) = (look.primary_hovered, look.primary_pressed);
        primary(look, "kit-form-primary", words.clone())
            .relative()
            .child(placed("form primary".into()))
            .map(|button| named_dimmed(button, words, ready))
            .when(!ready, |button| {
                // Dim, and as it is under the pointer and pressed: it takes no click yet.
                button
                    .opacity(DIMMED)
                    .hover(move |button| button.bg(look.primary))
                    .active(move |button| button.bg(look.primary))
            })
            .when(ready, move |button| {
                button
                    .hover(move |button| button.bg(hovered))
                    .active(move |button| button.bg(pressed))
                    .on_click(move |_, window, cx| submit(window, cx))
            })
    });
    let keys = {
        let (cancel, submit) = (cancel.clone(), submit.clone());
        move |card: gpui::Stateful<gpui::Div>| {
            card.key_context("Naming")
                .on_action(move |_: &KeepName, window, cx| submit(window, cx))
                .on_action(move |_: &CancelName, window, cx| cancel(window, cx))
        }
    };
    let card = match form.placement {
        Placement::Dialog => {
            let title = div()
                .line_height(px(TITLE_LINE))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(form.title.clone());
            let body = form.body.clone().map(|body| {
                div()
                    .mt(px(DETAIL_TOP))
                    .line_height(px(DETAIL_LINE))
                    .text_color(look.detail)
                    .child(body)
            });
            let fields = (!form.fields.is_empty()).then(|| {
                div()
                    .mt(px(FORM_TOP))
                    .flex()
                    .flex_col()
                    .gap(px(FORM_GAP))
                    .children(form.fields.iter().map(|input| field(look, input)))
            });
            let buttons = div()
                .mt(px(BUTTONS_TOP))
                .mr(px(BUTTON_INSET - TEXT_INSET))
                .flex()
                .justify_end()
                .gap(px(CONTROL_GAP))
                .child(cancel_button)
                .children(primary_button);
            keys(div().id("kit-form"))
                .w(px(WIDTH))
                .p(px(TEXT_INSET))
                .pb(px(BUTTON_INSET))
                .rounded(px(RADIUS))
                .bg(fill)
                .text_color(look.ink)
                .when_some(look.edge, |card, edge| card.border_1().border_color(edge))
                .occlude()
                .child(title)
                .children(body)
                .children(fields)
                .child(buttons)
        }
        Placement::Under(at) => {
            let row = div()
                .flex()
                .items_center()
                .gap(px(CONTROL_GAP))
                .children(form.fields.iter().map(|input| field(look, input)))
                .children(primary_button);
            keys(div().id("kit-popover"))
                .absolute()
                .left(at.x)
                .top(at.y)
                .w(px(POPOVER_WIDTH))
                .p(px(POPOVER_PADDING))
                .rounded(px(RADIUS / 2.))
                .bg(fill)
                .text_color(look.ink)
                .when_some(look.edge, |card, edge| card.border_1().border_color(edge))
                .occlude()
                .child(row)
        }
    };
    let dims = form.placement == Placement::Dialog;
    div()
        .id("kit-form-cover")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .when(dims, |cover| {
            cover
                .bg(rgb(DIM).opacity(DIM_OPACITY))
                .flex()
                .items_center()
                .justify_center()
        })
        .occlude()
        // A press outside a popover closes it, as a menu's does; a dialog form waits for Cancel.
        .when(!dims, |cover| {
            cover.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                cancel(window, cx)
            })
        })
        .child(card.on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()))
        .into_any_element()
}
