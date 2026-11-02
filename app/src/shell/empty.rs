//! The card of a window that shows no page, which is a fresh ⌘N window, a Space with no tab shown
//! and what's left after the last Today tab closes (docs/ux-spec.md › Window › Card, no page).
//! Nothing native covers it, since the core lays out no page view with no tab shown
//! (`Window::ShowNothing`), so GPUI draws ilo's mark, the Space's name and one quiet line on it and
//! takes its clicks. On the first run it shows the welcome instead, until a page shows.

use std::time::Instant;

use gpui::{
    Context, FontWeight, MouseButton, Pixels, Point, Stateful, Window, div, prelude::*, px, rgba,
};

use super::{
    Command, SECONDARY, Shell,
    display_face::{cap_top, display_text},
    fade_at,
    keys::user_keys,
};
use crate::palette::Palette;
use crate::tokens::text;

/// The mark and the line fade in over this long on the scroll edge's curve, as the hover card's
/// opacity does, from nothing, so a window that shows a page a frame later never shows them.
pub(super) const FADE: f32 = 0.1;
use crate::tokens::shell::empty::WORDS_SIZE;
use crate::tokens::shell::empty::KEYS_SIZE;
use crate::tokens::shell::empty::KEYS_HEIGHT;
use crate::tokens::shell::empty::KEYS_INSET;
use crate::tokens::shell::empty::KEYS_RADIUS;
use crate::tokens::shell::empty::KEYS_FILL;
use crate::tokens::shell::empty::KEYS_GAP;
/// ilo's mark, the app icon's lowercase i (branding/ilo.icon) drawn from its 1024 pt canvas at a
/// sixteenth, so 39 tall: a 184.4 dot 81.9 over a 133.1 × 358.4 stem with round ends.
const MARK_SCALE: f32 = 1. / 16.;
const MARK_DOT: f32 = 184.4 * MARK_SCALE;
const MARK_DOT_GAP: f32 = 81.9 * MARK_SCALE;
const MARK_STEM_WIDTH: f32 = 133.1 * MARK_SCALE;
const MARK_STEM_HEIGHT: f32 = 358.4 * MARK_SCALE;
use crate::tokens::shell::empty::MARK_DOT_FILL;
use crate::tokens::shell::empty::MARK_STEM_FILL;
use crate::tokens::shell::empty::MARK_GAP;
use crate::tokens::shell::empty::NAME_SIZE;
use crate::tokens::shell::empty::NAME_LINE;
use crate::tokens::shell::empty::NAME_GAP;
use crate::tokens::shell::empty::NAME;
use crate::tokens::shell::empty::WELCOME_MARK;
use crate::tokens::shell::empty::WELCOME_SIZE;
use crate::tokens::shell::empty::WELCOME_LINE;
use crate::tokens::shell::empty::TAGLINE_SIZE;
use crate::tokens::shell::empty::TAGLINE_LINE;
use crate::tokens::shell::empty::TAGLINE;
use crate::tokens::shell::empty::BUTTON_GAP;
use crate::tokens::shell::empty::BUTTON_HEIGHT;
use crate::tokens::shell::empty::BUTTON_INSET;
use crate::tokens::shell::empty::BUTTON_RADIUS;
use crate::tokens::shell::empty::BUTTON_FILL;
use crate::tokens::shell::empty::BUTTON_TEXT;
use crate::tokens::shell::empty::BUTTON_KEYS;
use crate::tokens::shell::empty::BUTTON_KEYS_HEIGHT;
use crate::tokens::shell::empty::BUTTON_KEYS_INSET;
use crate::tokens::shell::empty::BUTTON_KEYS_RADIUS;
use crate::tokens::shell::empty::HINT_BOTTOM;

impl Shell {
    /// Shows the first run's welcome on the empty card until a page shows: the launch had no
    /// Spaces to bring back and nothing to open.
    pub fn set_welcome(&mut self, cx: &mut Context<Self>) {
        self.welcome = true;
        cx.notify();
    }

    /// Starts the fade as the card comes to show no page, and ends it as a page shows, which ends
    /// the welcome for good.
    pub(super) fn step_empty(&mut self, window: &mut Window) {
        if self.selected != 0 {
            self.empty_since = None;
            self.empty_settled = false;
            self.welcome = false;
            self.in_hint = false;
            return;
        }
        let since = *self.empty_since.get_or_insert_with(Instant::now);
        if since.elapsed().as_secs_f32() < FADE {
            window.request_animation_frame();
        } else if !std::mem::replace(&mut self.empty_settled, true) {
            // check.sh waits for this line before it captures the empty card.
            log!("browser: empty card shown");
            if self.welcome {
                log!("browser: welcome shown");
            }
        }
    }

    /// How much of the line shows `now`: none with a page shown, else along its fade.
    pub(super) fn empty_opacity(&self, now: Instant) -> f32 {
        let Some(since) = self.empty_since.filter(|_| self.selected == 0) else {
            return 0.;
        };
        fade_at(0., 1., now.saturating_duration_since(since).as_secs_f32(), FADE)
    }

    /// Whether `at` is on the card while it shows no page.
    pub(super) fn empty_card_hit(&self, at: Point<Pixels>) -> bool {
        self.selected == 0 && self.page_frame(self.viewport).0.contains(&at)
    }

    /// ilo's mark over the Space's name and the card's one line, the next step and its key,
    /// centred, while no page is shown, or the welcome on the first run. A click anywhere on the
    /// card does what that key does: ⌘T's command bar, centred on the window.
    pub(super) fn empty_card(&self, cx: &mut Context<Self>) -> Option<Stateful<gpui::Div>> {
        if self.selected != 0 {
            return None;
        }
        let (frame, _) = self.page_frame(self.viewport);
        let sign = match self.welcome {
            true => welcome(&self.palette, cx),
            false => self.sign(cx),
        };
        Some(
            div()
                .id("empty-card")
                .absolute()
                .left(frame.left())
                .top(frame.top())
                .w(frame.size.width)
                .h(frame.size.height)
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _, _, cx| shell.pressed_bar_target(cx)),
                )
                // On release, so a press that drags the window by its background isn't a click.
                .on_click(cx.listener(|shell, _, _, cx| {
                    shell.clicked_bar_target(Command::NewTab, "card", cx)
                }))
                .child(sign.opacity(self.empty_opacity(Instant::now())))
                .children(self.in_hint.then(|| self.hint(cx))),
        )
    }

    /// The onboarding's last word, at the card's foot under the command bar it leaves up: what the
    /// bar is, and its key.
    fn hint(&self, cx: &mut Context<Self>) -> gpui::Div {
        let keys = user_keys(cx).label(Command::NewTab).map(|keys| {
            div()
                .h(px(KEYS_HEIGHT))
                .px(px(KEYS_INSET))
                .flex()
                .items_center()
                .rounded(px(KEYS_RADIUS))
                .bg(self.palette.plate(rgba(KEYS_FILL).a))
                .text_size(px(KEYS_SIZE))
                .child(keys)
        });
        div()
            .absolute()
            .bottom(px(HINT_BOTTOM))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .items_center()
            .gap(px(KEYS_GAP))
            .text_size(px(WORDS_SIZE))
            .text_color(self.palette.text(SECONDARY))
            .child("Type a site, a search or a command.")
            .when_some(keys, |hint, keys| hint.child(keys).child("brings it back from anywhere."))
    }

    /// The mark, the Space's name in the display face, and the line.
    fn sign(&self, cx: &mut Context<Self>) -> gpui::Div {
        let keys = user_keys(cx).label(Command::NewTab).map(|keys| {
            div()
                .h(px(KEYS_HEIGHT))
                .px(px(KEYS_INSET))
                .flex()
                .items_center()
                .rounded(px(KEYS_RADIUS))
                .bg(self.palette.plate(rgba(KEYS_FILL).a))
                .text_size(px(KEYS_SIZE))
                .child(keys)
        });
        let line = div()
            .flex()
            .items_center()
            .gap(px(KEYS_GAP))
            .text_color(self.palette.text(SECONDARY))
            .child(div().text_size(px(WORDS_SIZE)).child("New Tab"))
            .children(keys);
        let sign = div().flex().flex_col().items_center().child(mark(1., &self.palette));
        match self.shown_space().map(|space| space.name.clone()).filter(|name| !name.is_empty()) {
            // The name's caps, not its line, start the mark's gap below the mark.
            Some(name) => sign
                .child(
                    display_text(name, NAME_SIZE, NAME_LINE, cx)
                        .mt(px(MARK_GAP - cap_top(NAME_SIZE, NAME_LINE)))
                        .text_color(self.palette.text(rgba(NAME).a)),
                )
                .child(line.mt(px(NAME_GAP))),
            None => sign.child(line.mt(px(MARK_GAP))),
        }
    }
}

/// ilo's mark, the app icon's i, at `scale` of its size on the card. Its stem takes the window's
/// ink, as the icon's does on its light tile.
fn mark(scale: f32, palette: &Palette) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(MARK_DOT_GAP * scale))
        .child(div().size(px(MARK_DOT * scale)).rounded_full().bg(rgba(MARK_DOT_FILL)))
        .child(
            div()
                .w(px(MARK_STEM_WIDTH * scale))
                .h(px(MARK_STEM_HEIGHT * scale))
                .rounded_full()
                .bg(palette.plate(rgba(MARK_STEM_FILL).a)),
        )
}

/// The first run's welcome: a bigger mark, "Welcome to ilo" and "a good tool." in the display
/// face, and a button that opens a new tab, as a click anywhere on the card does.
fn welcome(palette: &Palette, cx: &mut Context<Shell>) -> gpui::Div {
    let keys = user_keys(cx).label(Command::NewTab).map(|keys| {
        div()
            .h(px(BUTTON_KEYS_HEIGHT))
            .px(px(BUTTON_KEYS_INSET))
            .flex()
            .items_center()
            .rounded(px(BUTTON_KEYS_RADIUS))
            .bg(rgba(BUTTON_KEYS))
            .text_size(px(text::SMALL))
            .child(keys)
    });
    let button = div()
        .mt(px(BUTTON_GAP))
        .h(px(BUTTON_HEIGHT))
        .px(px(BUTTON_INSET))
        .flex()
        .items_center()
        .gap(px(KEYS_GAP))
        .rounded(px(BUTTON_RADIUS))
        .bg(rgba(BUTTON_FILL))
        .text_color(rgba(BUTTON_TEXT))
        .text_size(px(text::BODY))
        .font_weight(FontWeight::MEDIUM)
        .child("Open a new tab")
        .children(keys);
    div()
        .flex()
        .flex_col()
        .items_center()
        .child(mark(WELCOME_MARK, palette))
        .child(
            display_text("Welcome to ilo", WELCOME_SIZE, WELCOME_LINE, cx)
                .mt(px(MARK_GAP - cap_top(WELCOME_SIZE, WELCOME_LINE)))
                .text_color(palette.text(rgba(NAME).a)),
        )
        .child(
            display_text("a good tool.", TAGLINE_SIZE, TAGLINE_LINE, cx)
                .text_color(palette.text(rgba(TAGLINE).a)),
        )
        .child(button)
}

#[cfg(test)]
mod tests;
