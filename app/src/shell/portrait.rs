//! Portrait Mode's card (#673). As Arc's small panel does, Capture in Portrait Mode asks what to
//! put the page on, and Capture copies the picture (capture/portrait.rs). The card hangs under the
//! URL field, where a page's question hangs.
//!
//! The background chosen is kept, and ⌥ on Site Controls' Capture Full Page captures on it
//! without asking, as Arc's ⌥-click skips the panel. A Little ilo window captures without asking
//! too, as it has no URL field for the card to hang from.

use gpui::{Context, FontWeight, div, prelude::*, px, rgb, svg};

use super::{
    SECONDARY, Shell, TITLE, URL_FIELD,
    access::Accessible,
    permission::{
        BESIDE_CLOSE, BUTTONS_TOP, Look, card_button_as, card_close, card_listener, part_listener,
    },
};
use crate::capture::portrait::Background;
use crate::tokens::capture::portrait::{SWATCH, SWATCH_GAP, SWATCH_RING};
use crate::tokens::shell::permission::{BUTTON_GAP, DETAIL};
use crate::tokens::theme_picker::SWATCHES as COLOURS;

/// How many backgrounds the card can offer: the desktop's picture, white and the theme picker's
/// colours.
pub(super) const SWATCHES: usize = COLOURS.len() + 2;

/// The background Portrait Mode last captured on, which the next starts from, kept in the profile
/// directory (settings.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortraitBackground(pub Background);

impl gpui::Global for PortraitBackground {}

impl Shell {
    /// The backgrounds the card offers. The desktop's picture is offered only where the system gives
    /// one, as the Mac and Windows do.
    fn portrait_backgrounds(&self) -> impl Iterator<Item = Background> + use<> {
        let desktop = self.browser.desktop_picture_offered();
        (desktop.then_some(Background::Desktop).into_iter())
            .chain([Background::White])
            .chain(COLOURS.iter().map(|&colour| Background::Colour(colour)))
    }

    /// The background the card starts on: the one kept, or Arc's first, the desktop's picture,
    /// where it's offered, and otherwise white.
    fn portrait_background(&self, cx: &gpui::App) -> Background {
        let kept = cx.try_global::<PortraitBackground>().map(|kept| kept.0);
        let first = self.portrait_backgrounds().next();
        kept.filter(|kept| self.portrait_backgrounds().any(|each| each == *kept))
            .or(first)
            .unwrap_or(Background::White)
    }

    /// Capture in Portrait Mode: the card that asks, or closes it. In a Little ilo window it
    /// captures at once.
    pub(super) fn capture_in_portrait_mode(&mut self, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        if tab == 0 {
            return;
        }
        if self.little.is_some() {
            return self.capture_portrait(self.portrait_background(cx), cx);
        }
        if self.portrait_card.take().is_none() {
            self.portrait_card = Some((tab, self.portrait_background(cx)));
            // The card hangs from the URL field, so a hidden sidebar comes out with it.
            if self.can_reveal() {
                self.set_revealed(true, cx);
            }
        }
        cx.notify();
    }

    /// Portrait Mode on the background kept, without asking.
    pub(super) fn capture_portrait_now(&mut self, cx: &mut Context<Self>) {
        self.capture_portrait(self.portrait_background(cx), cx);
    }

    /// Captures the page the URL field shows on `background`, which the next capture starts from.
    fn capture_portrait(&mut self, background: Background, cx: &mut Context<Self>) {
        self.portrait_card = None;
        let tab = self.field_tab();
        let Some((frame, radius)) = self.pane_frame(tab) else {
            return;
        };
        cx.set_global(PortraitBackground(background));
        self.browser
            .capture_portrait(tab, frame, radius, background, cx);
        cx.notify();
    }

    /// Closes the card, and says whether it was open.
    pub(super) fn close_portrait_card(&mut self, cx: &mut Context<Self>) -> bool {
        if self.portrait_card.take().is_none() {
            return false;
        }
        cx.notify();
        true
    }

    /// The swatch for `background`, the `index`th, ringed while it's the one chosen.
    fn portrait_swatch(
        &self,
        index: usize,
        background: Background,
        chosen: bool,
        tab: u64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = &self.palette;
        let name = background.name();
        let (fill, label) = match background {
            Background::Colour(colour) => {
                (rgb(colour).into(), crate::theme_picker::swatch_name(colour))
            }
            Background::White => (gpui::white(), "White"),
            Background::Desktop => (palette.plate(URL_FIELD), "Desktop Picture"),
        };
        let swatch = div()
            .id(gpui::SharedString::from(format!("portrait-{name}")))
            .relative()
            .size(px(SWATCH))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(fill)
            .border(px(if chosen { SWATCH_RING } else { 1. }))
            .border_color(match chosen {
                true => palette.accent.into(),
                false => palette.on_overlay(SECONDARY),
            })
            .button_named(label)
            .child(part_listener(
                format!("portrait swatch {name}"),
                &self.portrait_swatches_drawn[index],
                tab,
            ))
            .on_click(cx.listener(move |shell, _, _, cx| {
                if let Some((tab, _)) = shell.portrait_card {
                    // check.sh reads this line.
                    log!("browser: portrait chose {}", background.name());
                    shell.portrait_card = Some((tab, background));
                    cx.notify();
                }
            }));
        match background {
            Background::Desktop => swatch.child(
                svg()
                    .path("icons/image.svg")
                    .size(px(SWATCH - 6.))
                    .text_color(palette.text(TITLE)),
            ),
            _ => swatch,
        }
    }

    pub(super) fn portrait_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let tab = self.field_tab();
        let Some((_, chosen)) = self
            .portrait_card
            .filter(|(open, _)| *open == tab && tab != 0)
        else {
            self.portrait_card_drawn.set(None);
            return None;
        };
        let swatches: Vec<_> = (self.portrait_backgrounds().enumerate())
            .map(|(index, each)| self.portrait_swatch(index, each, each == chosen, tab, cx))
            .collect();
        let palette = &self.palette;
        let not_now = card_button_as(palette, "portrait-not-now", "Not Now", Look::Quiet).on_click(
            cx.listener(|shell, _, _, cx| {
                shell.close_portrait_card(cx);
            }),
        );
        let capture = card_button_as(palette, "portrait-capture", "Capture", Look::Primary)
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.capture_portrait(chosen, cx);
            }));
        let close =
            card_close(palette, "portrait-close").on_click(cx.listener(|shell, _, _, cx| {
                shell.close_portrait_card(cx);
            }));
        let listener = card_listener("portrait card", &self.portrait_card_drawn, tab);
        Some(
            self.plate("portrait-card")
                .child(listener)
                .child(
                    div()
                        .pr(px(BESIDE_CLOSE))
                        .text_color(palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        .child("Capture in Portrait Mode"),
                )
                .child(
                    div()
                        .text_color(palette.on_overlay(DETAIL))
                        .child("The page on a background, copied to share."),
                )
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .flex()
                        .flex_wrap()
                        .gap(px(SWATCH_GAP))
                        .children(swatches),
                )
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .flex()
                        .gap(px(BUTTON_GAP))
                        .child(not_now)
                        .child(capture),
                )
                .child(close),
        )
    }
}

#[cfg(test)]
mod tests;
