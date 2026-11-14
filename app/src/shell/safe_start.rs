//! The safe-start card at the sidebar's foot (#2345, docs/ux-spec.md › Safe start). After the
//! browser crashed at launch twice in a row, it started with extensions and Boosts not loaded
//! (crate::safe_start), and this card, over the footer and under the update banner, says so in
//! every window until the launch ends: what happened, what's off, that pages aren't ad-blocked
//! meanwhile if Settings blocks ads, Turn Back On, which restarts into a normal launch with the
//! windows and tabs, and Manage Extensions…, for the extension that may be the cause. It has no ×:
//! the hold lasts the launch whatever the user does, so the card does too. Agents get none of it.

use gpui::{
    Context, Font, FontWeight, MouseButton, SharedString, TextRun, WindowTextSystem, div,
    prelude::*, px, svg,
};

use super::{FOOTER_HEIGHT, PRESSED, Shell, access::Accessible, permission::card_listener};
use crate::tokens::settings::{CONTROL_HEIGHT, CONTROL_RADIUS};
use crate::tokens::shell::{
    FAVICON_DRAWN, FAVICON_SIZE, FAVICON_X, PLATE_INSET, PLATE_RADIUS, SAFE_START_DETAIL_LINE,
    SAFE_START_PAD, SAFE_START_TITLE_LINE, SECONDARY, TITLE_GAP, UNSELECTED_TITLE,
    UPDATE_BANNER_GAP,
};
use crate::tokens::text;

/// Its words, as check.sh and an assistive app name them.
pub(super) const TITLE: &str = concat!(env!("APP_NAME"), " closed unexpectedly twice");
const OFF: &str = "Extensions and Boosts are off for now.";
const OFF_UNBLOCKED: &str = "Extensions and Boosts are off for now, so ads aren’t blocked.";
pub(super) const TURN_BACK_ON: &str = "Turn Back On";
pub(super) const MANAGE: &str = "Manage Extensions…";

/// The gaps between the title, the detail, the button and the link, as the update banner's open
/// state has them.
const DETAIL_TOP: f32 = 2.;
const BUTTON_TOP: f32 = 8.;
const BUTTON_INSET: f32 = 10.;
const LINK_TOP: f32 = 6.;
const LINK: f32 = 16.;

/// The card's words, wrapped in the sidebar's width: its height follows their lines, as the foot's
/// list ends over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Shape {
    width: f32,
    ad_blocking: bool,
    title_lines: usize,
    detail_lines: usize,
}

impl Shape {
    fn height(&self) -> f32 {
        SAFE_START_PAD
            + self.title_lines as f32 * SAFE_START_TITLE_LINE
            + DETAIL_TOP
            + self.detail_lines as f32 * SAFE_START_DETAIL_LINE
            + BUTTON_TOP
            + CONTROL_HEIGHT
            + LINK_TOP
            + LINK
            + SAFE_START_PAD
    }
}

/// Where the words start in the card: the rows' title column.
fn words_x() -> f32 {
    FAVICON_X - PLATE_INSET + FAVICON_SIZE + TITLE_GAP
}

fn detail(ad_blocking: bool) -> &'static str {
    match ad_blocking {
        true => OFF_UNBLOCKED,
        false => OFF,
    }
}

/// How many lines `text` takes wrapped at `wrap`.
fn lines(text: &str, size: f32, weight: FontWeight, wrap: f32, system: &WindowTextSystem) -> usize {
    let run = TextRun {
        len: text.len(),
        font: Font {
            weight,
            ..gpui::font(crate::face::UI)
        },
        // Measured only, never drawn.
        color: Default::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let text: SharedString = text.to_string().into();
    system
        .shape_text(text, px(size), &[run], Some(px(wrap)), None)
        .map_or(1, |shaped| {
            (shaped.iter())
                .map(|line| line.wrap_boundaries().len() + 1)
                .sum()
        })
}

impl Shell {
    /// Whether this launch is a safe start, so the card shows.
    pub(super) fn safe_start_shown(&self) -> bool {
        self.browser.safe_start()
    }

    /// How much of the foot the card takes, with its gaps, while it shows, else nothing.
    pub(super) fn safe_start_room(&self) -> f32 {
        match (self.safe_start_shown(), self.safe_start) {
            (true, Some(shape)) => shape.height() + UPDATE_BANNER_GAP,
            _ => 0.,
        }
    }

    /// Where what stands on the footer starts: over the card, while it shows.
    pub(super) fn over_safe_start(&self) -> f32 {
        FOOTER_HEIGHT + self.safe_start_room()
    }

    /// Wraps the card's words in the sidebar's width, again only when the width or Settings' ad
    /// blocking changed: before anything in the frame asks where the foot's things stand.
    pub(super) fn shape_safe_start(&mut self, cx: &mut Context<Self>) {
        if !self.safe_start_shown() {
            return;
        }
        let width = self.width();
        let ad_blocking = self.browser.ad_blocking();
        if (self.safe_start)
            .is_some_and(|shape| shape.width == width && shape.ad_blocking == ad_blocking)
        {
            return;
        }
        let system = (self.text_system.take())
            .unwrap_or_else(|| WindowTextSystem::new(cx.text_system().clone()));
        let wrap = width - 2. * PLATE_INSET - words_x() - BUTTON_INSET;
        let shape = Shape {
            width,
            ad_blocking,
            title_lines: lines(TITLE, text::BODY, FontWeight::MEDIUM, wrap, &system),
            detail_lines: lines(
                detail(ad_blocking),
                text::SMALL,
                FontWeight::NORMAL,
                wrap,
                &system,
            ),
        };
        self.text_system = Some(system);
        if self.safe_start.is_none() {
            // check.sh waits for this line.
            log!("browser: safe start card shown");
        }
        self.safe_start = Some(shape);
    }

    /// Turn Back On: the browser restarts with its windows and tabs, into a normal launch.
    pub(super) fn turn_back_on(&mut self, cx: &mut Context<Self>) {
        log!("browser: command turn-back-on via sidebar");
        self.browser.restart(cx);
    }

    fn manage_from_safe_start(&mut self, cx: &mut Context<Self>) {
        log!("browser: command manage-extensions via safe start");
        self.browser.manage_extensions(None, cx);
    }

    /// The card, while this launch is a safe start.
    pub(super) fn safe_start_card(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let shape = self.safe_start.filter(|_| self.safe_start_shown())?;
        let words = words_x();
        let glyph = svg()
            .path("icons/circle-alert.svg")
            .size(px(FAVICON_DRAWN))
            .text_color(self.palette.glyph(UNSELECTED_TITLE));
        let head = div()
            .relative()
            .pl(px(words))
            .pr(px(BUTTON_INSET))
            .child(
                // In the rows' icon column, on the title's first line.
                div()
                    .absolute()
                    .left(px(FAVICON_X - PLATE_INSET))
                    .top_0()
                    .size(px(FAVICON_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph),
            )
            .child(
                div()
                    .text_size(px(text::BODY))
                    .line_height(px(SAFE_START_TITLE_LINE))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(self.palette.text(1.))
                    .child(TITLE),
            )
            .child(
                div()
                    .mt(px(DETAIL_TOP))
                    .text_size(px(text::SMALL))
                    .line_height(px(SAFE_START_DETAIL_LINE))
                    .text_color(self.palette.text(SECONDARY))
                    .child(detail(shape.ad_blocking)),
            );
        let button = div()
            .id("safe-start-button")
            .when(self.a11y, |button| button.button_named(TURN_BACK_ON))
            .relative()
            .mt(px(BUTTON_TOP))
            .mx(px(BUTTON_INSET))
            .h(px(CONTROL_HEIGHT))
            .rounded(px(CONTROL_RADIUS))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(text::BODY))
            .font_weight(FontWeight::MEDIUM)
            .bg(self.palette.accent)
            .text_color(self.palette.accent_label)
            .hover(|style| style.bg(self.palette.accent_hovered()))
            // check.sh clicks it where it's drawn.
            .child(card_listener(
                "safe start button",
                &self.safe_start_button_drawn,
                0,
            ))
            .child(TURN_BACK_ON)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|shell, _, _, cx| shell.turn_back_on(cx)));
        let link = div()
            .id("safe-start-manage")
            .when(self.a11y, |link| link.button_named(MANAGE))
            .relative()
            .mt(px(LINK_TOP))
            .mx(px(BUTTON_INSET))
            .h(px(LINK))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(text::SMALL))
            .text_color(self.palette.text(SECONDARY))
            .hover(|style| style.text_color(self.palette.text(1.)))
            // check.sh clicks it where it's drawn.
            .child(card_listener(
                "safe start manage",
                &self.safe_start_manage_drawn,
                0,
            ))
            .child(MANAGE)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|shell, _, _, cx| shell.manage_from_safe_start(cx)));
        Some(
            div()
                .id("safe-start")
                .absolute()
                .left(px(PLATE_INSET))
                .w(px(self.width() - 2. * PLATE_INSET))
                .bottom(px(FOOTER_HEIGHT + UPDATE_BANNER_GAP))
                .h(px(shape.height()))
                .pt(px(SAFE_START_PAD))
                .rounded(px(PLATE_RADIUS))
                .overflow_hidden()
                .bg(self.window_fill_at(1.))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .rounded(px(PLATE_RADIUS))
                        .bg(self.palette.plate(PRESSED)),
                )
                // check.sh reads where it's drawn.
                .child(card_listener("safe start card", &self.safe_start_drawn, 0))
                .child(head)
                .child(button)
                .child(link)
                .block_mouse_except_scroll()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        )
    }
}

#[cfg(test)]
mod tests;
