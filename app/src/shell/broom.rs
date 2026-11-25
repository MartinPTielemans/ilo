//! The broom after Clear on the New Tab line (docs/ux-spec.md › Sidebar › Clear divider): what the
//! browser says of it, and its forms. The shell draws the state it's told and passes the user's
//! click on: it learns nothing of a sort, gathers nothing for one and keeps no time for it.

use std::f32::consts::TAU;

use gpui::{
    AnyElement, Context, Div, Hsla, MouseButton, Radians, Role, Transformation, accesskit::Live,
    div, prelude::*, px, svg,
};

use super::{CLEAR_HIT_OUT, CLEAR_HOVERED, Shell, access::Accessible, cast::TURN};
use crate::palette::{Palette, TEXT_PRIMARY};
use crate::product::PRODUCT;
use crate::tokens::shell::{BROOM_SIZE, BROOM_UNAVAILABLE, DIVIDER_GAP};
use crate::tokens::text;
use crate::tooltip::{Anchor, Tip};

#[cfg(test)]
mod tests;

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
    /// Too few Today tabs are outside a folder: the glyph keeps its place, so Clear doesn't shift
    /// as tabs come and go, and it takes no click.
    NothingToSort,
}

impl Broom {
    /// Whether a click starts a sort.
    fn sorts(self) -> bool {
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
    /// The broom on a line that has Clear, live: its glyph, or the ring while it sorts.
    pub(super) fn broom_button(
        &self,
        state: Broom,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let glyph = match state {
            Broom::Sorting => {
                let turned = self.breath.turned(TURN, self.clock.now());
                working_ring(&self.palette, BROOM_SIZE, turned).into_any_element()
            }
            _ => glyph(glyph_ink(&self.palette, state, self.broom_hovered)),
        };
        let name = match state {
            Broom::Sorting => SORTING,
            _ => NAME,
        };
        broom_box(glyph, Some(self.button_frame(FRAME).into_any_element()))
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
            // never wraps and the glyph after it keeps its place.
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

/// The broom where it doesn't change: on the page of a Space beside the one shown, whose Clear
/// then stands where it will once the Space is the one shown.
pub(super) fn still_broom(palette: &Palette) -> Div {
    broom_box(glyph(glyph_ink(palette, Broom::Idle, false)), None)
}

/// The broom's place on the line: its glyph's box the line's gap after Clear, ending where the
/// titles do. It takes clicks from where Clear stops taking them to as far past its glyph.
fn broom_box(glyph: AnyElement, frame: Option<AnyElement>) -> Div {
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
                .children(frame.map(|frame| div().absolute().inset_0().child(frame))),
        )
}

fn glyph(ink: Hsla) -> AnyElement {
    svg()
        .path("icons/broom.svg")
        .size(px(BROOM_SIZE))
        .text_color(ink)
        .into_any_element()
}

/// The glyph's ink: the New Tab label's, Clear's lit one under the pointer where a click would
/// sort, and a part of its own where there's nothing to sort, which needn't read.
fn glyph_ink(palette: &Palette, state: Broom, hovered: bool) -> Hsla {
    match state {
        Broom::NothingToSort => palette
            .glyph(palette.secondary())
            .opacity(BROOM_UNAVAILABLE),
        _ if hovered && state.sorts() => palette.glyph(CLEAR_HOVERED),
        _ => palette.glyph(palette.secondary()),
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
