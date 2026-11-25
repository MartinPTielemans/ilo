//! The broom before Clear on the New Tab line (docs/ux-spec.md › Sidebar › Clear divider): what
//! the browser says of it, when it's drawn and its forms. The shell draws the state it's told and
//! passes the user's click on: it learns nothing of a sort, gathers nothing for one and keeps no
//! time for it, and whether the glyph shows is read from that state and what the sidebar knows of
//! the pointer and the keys already.

use std::f32::consts::TAU;

use gpui::{
    AnyElement, App, Context, Div, Hsla, MouseButton, Radians, Role, Transformation,
    accesskit::Live, div, prelude::*, px, svg,
};

use super::{
    CLEAR_HIT_OUT, CLEAR_HOVERED, Shell, access::Accessible, cast::TURN, sidebar_keys::NEW_TAB,
};
use crate::palette::{Palette, TEXT_PRIMARY};
use crate::product::PRODUCT;
use crate::tokens::shell::{BROOM_SIZE, DIVIDER_GAP};
use crate::tokens::text;
use crate::tooltip::{Anchor, Tip};

#[cfg(test)]
mod tests;

#[cfg(test)]
thread_local! {
    /// How often the New Tab line with tabs today was built, and the broom it had the last time:
    /// its state and how much of it was drawn, or None with no glyph.
    pub(super) static DRAWN: std::cell::Cell<(u32, Option<(Broom, f32)>)> =
        const { std::cell::Cell::new((0, None)) };
}

/// What the broom shows, as the browser answers for the Space the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Broom {
    /// A click sorts the Space's Today tabs into folders.
    Idle,
    /// A sort runs: a ring turns in the glyph's place, and a click does nothing.
    Sorting,
    /// The last sort changed nothing. Its word stands in Clear's place for as long as the browser
    /// answers this, and a click sorts again.
    Failed,
    /// Too few Today tabs are outside a folder, or another Space's sort runs: no glyph is drawn
    /// and no room kept for one, and there is nothing to click.
    NothingToSort,
}

impl Broom {
    /// Whether a click starts a sort, as the menu item and the bar entry read it too.
    pub fn sorts(self) -> bool {
        matches!(self, Broom::Idle | Broom::Failed)
    }
}

/// The broom's name, in its tooltip and to a screen reader, and a screen reader's while it sorts.
pub(super) const NAME: &str = "Sort Tabs into Folders";
const SORTING: &str = "Sorting tabs";
/// What stands in Clear's place after a sort that changed nothing. Short: the line has room for
/// about 13 characters at the sidebar's usual width.
const FAILED: &str = "Couldn't sort";

/// What a screen reader is told of a sort that changed nothing.
fn failed_said() -> String {
    format!("{PRODUCT} couldn't sort today's tabs")
}

/// The frame the broom's glyph is drawn in is kept under this, for its tooltip.
pub(super) const FRAME: &str = "broom";

impl Shell {
    /// How much of the broom is drawn in `state`, from none to all. Where a click would sort, it
    /// comes with the pointer over the sidebar, anywhere in it, so it's there before the pointer
    /// reaches the line, and goes with it, as the URL field's button does and on that fade; at
    /// once with motion reduced; and it's there while the keys are on the New Tab line, and for
    /// an assistive app, whose cursor is no pointer. A sort that runs and one that failed show
    /// whatever the pointer does, and with nothing to sort there is no glyph. Nothing is asked or
    /// started to tell: this reads what the browser said of the broom and what the sidebar keeps
    /// of the pointer and the keys.
    pub(super) fn broom_shown(&self, state: Broom, cx: &App) -> f32 {
        match state {
            Broom::Sorting | Broom::Failed => 1.,
            Broom::NothingToSort => 0.,
            Broom::Idle if self.keyed == Some(NEW_TAB) => 1.,
            Broom::Idle if crate::motion::still(cx) => f32::from(u8::from(self.tools_live())),
            Broom::Idle => self.tools_opacity(),
        }
    }

    /// The broom on a line that has Clear, live and `shown` of the way there: its glyph, or the
    /// ring while it sorts.
    pub(super) fn broom_button(
        &self,
        state: Broom,
        shown: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let glyph = match state {
            Broom::Sorting => {
                let turned = self.breath.turned(TURN, self.clock.now());
                working_ring(&self.palette, BROOM_SIZE, turned).into_any_element()
            }
            _ => glyph(glyph_ink(&self.palette, self.broom_hovered)),
        };
        let name = match state {
            Broom::Sorting => SORTING,
            _ => NAME,
        };
        broom_box(glyph, self.button_frame(FRAME).into_any_element())
            .opacity(shown)
            .id("broom")
            .when(self.a11y, |broom| {
                broom
                    .accessibility_id("broom")
                    .button_named(name)
                    .when(!state.sorts(), |broom| {
                        broom
                            .a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
                    })
            })
            .block_mouse_except_scroll()
            .on_hover(cx.listener(|shell, hovered: &bool, _, cx| shell.hover_broom(*hovered, cx)))
            // Not the line's press, which would open the command bar, whatever the broom shows.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| {
                    cx.stop_propagation();
                    shell.browser.tooltip(None, cx)
                }),
            )
            .on_click(cx.listener(|shell, _, _, cx| {
                cx.stop_propagation();
                // What the browser says now, which a sort begun since the draw has changed.
                if shell.browser.broom().is_some_and(Broom::sorts) {
                    shell.browser.start_broom(cx)
                }
            }))
    }

    /// The pointer has come onto the broom or left it: it lights up as Clear does where a click
    /// would sort, and after a moment says its name, hung under its glyph.
    fn hover_broom(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.broom_hovered = hovered;
        let busy = self.dragging().is_some() || self.menu_row.is_some();
        let frame = self.button_frames.borrow().get(FRAME).copied();
        let tip = frame
            .filter(|_| hovered && !busy)
            .map(|frame| (Tip::named(NAME.into()), Anchor::under(frame, self.viewport)));
        self.hover_tip(tip, cx);
        cx.notify();
    }

    /// The word in Clear's place after a sort that changed nothing, which a screen reader is told
    /// once, as it comes. It takes the presses Clear would, so one meant for Clear isn't the
    /// line's.
    pub(super) fn sort_failed(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let said = failed_said();
        div()
            .id("sort-failed")
            .when(self.a11y, |word| {
                // The Mac's AccessKit announces a live region's value, and Windows' and Linux's
                // its name, so it has both (#3236).
                word.role(Role::Status)
                    .aria_label(said.clone())
                    .aria_value(said)
                    .a11y_synthetic_children(|builder| builder.parent_node().set_live(Live::Polite))
            })
            // After the hairline, which has given way by then, the word does: the label before it
            // never wraps and the glyph before the word is never squeezed.
            .min_w_0()
            .ml(px(DIVIDER_GAP))
            .h_full()
            .flex()
            .items_center()
            .text_size(px(text::SMALL))
            .text_color(self.palette.text(self.palette.tertiary()))
            .child(div().truncate().child(FAILED))
            .block_mouse_except_scroll()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
    }
}

/// The broom's place on the line: its glyph's box the line's gap before Clear, where the hairline
/// ends as far before it. It takes clicks from as far before its glyph to where Clear starts
/// taking them.
fn broom_box(glyph: AnyElement, frame: AnyElement) -> Div {
    let out = DIVIDER_GAP - CLEAR_HIT_OUT;
    div()
        .flex_none()
        .ml(px(CLEAR_HIT_OUT))
        .mr(px(-out))
        .px(px(out))
        .h_full()
        .flex()
        .items_center()
        .child(
            div()
                .relative()
                .flex_none()
                .size(px(BROOM_SIZE))
                .child(glyph)
                // In a box of its own over the glyph's: left where it falls, GPUI lays it out
                // after the glyph.
                .child(div().absolute().inset_0().child(frame)),
        )
}

fn glyph(ink: Hsla) -> AnyElement {
    svg()
        .path("icons/broom.svg")
        .size(px(BROOM_SIZE))
        .text_color(ink)
        .into_any_element()
}

/// The glyph's ink: the New Tab label's, and Clear's lit one while `lit`, under the pointer.
fn glyph_ink(palette: &Palette, lit: bool) -> Hsla {
    match lit {
        true => palette.glyph(CLEAR_HOVERED),
        false => palette.glyph(palette.secondary()),
    }
}

/// The ring that turns in a glyph's place while the product is at work on something the user
/// asked of it, `turned` of a turn round: one sign for it, wherever it's drawn.
pub(super) fn working_ring(palette: &Palette, size: f32, turned: f32) -> gpui::Svg {
    svg()
        .path("icons/working.svg")
        .flex_none()
        .size(px(size))
        .text_color(palette.text(TEXT_PRIMARY))
        .with_transformation(Transformation::rotate(Radians(TAU * turned)))
}
