//! A Bluetooth device a page pairs with asks the user, in the page's card in place of Chrome's
//! dialogs, which are tab-modal ones our windows don't host (#1701): for its PIN, in a field with
//! Pair and ×, or whether to pair, and whether the passkey it shows is the one here. Chrome asks
//! only on Linux and Windows; the Mac pairs through the system's own prompt. The words are
//! Chrome's. Pair, or ↩ in the field, answers, once the PIN has a digit, as Chrome's OK needs; ×,
//! or Esc, cancels, and the page's connect fails. The core keeps each tab's prompt while another
//! tab is shown, and ends it with its page. An agent's tab never gets one: pairing is the user's.

use gpui::{Context, Entity, Focusable, FontWeight, SharedString, Window, div, prelude::*, px};

use super::{
    CancelName, FORM_PLACEHOLDER, FORM_PLATE, FORM_TEXT, KeepName, NAV_RADIUS, Shell, TITLE,
    permission::{BESIDE_CLOSE, BUTTONS_TOP, card_button, card_close, card_listener},
};
use crate::text_input::TextInput;
use crate::tokens::shell::permission::{BUTTON_HEIGHT, DETAIL};

/// What a device asks, as the core reports it.
#[derive(Clone, PartialEq, Debug)]
pub struct Pair {
    /// The device, as Chrome names it.
    pub device: SharedString,
    pub kind: PairKind,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PairKind {
    /// Its PIN, typed.
    Pin,
    /// Whether to pair.
    Confirm,
    /// Whether the passkey it shows is this one.
    Passkey(SharedString),
}

impl Pair {
    /// Chrome's title for its dialog.
    fn title(&self) -> &'static str {
        match self.kind {
            PairKind::Pin => "Device Credentials",
            PairKind::Confirm => "Pair Confirmation",
            PairKind::Passkey(_) => "Confirm passkey",
        }
    }

    /// Chrome's words under it.
    fn asks(&self) -> String {
        let device = &self.device;
        match &self.kind {
            PairKind::Pin => format!("Enter Bluetooth Passkey for device {device}"),
            PairKind::Confirm => {
                format!("Bluetooth device {device} would like permission to pair.")
            }
            PairKind::Passkey(passkey) => format!(
                "Please confirm that {passkey} is the passkey displayed on Bluetooth device \
                 {device}."
            ),
        }
    }
}

/// Whether `pin` can pair, as Chrome's OK: it has a digit.
fn pin_takes(pin: &str) -> bool {
    pin.chars().any(|c| c.is_ascii_digit())
}

/// A tab's prompt as its card shows it, with its PIN as typed so far.
pub(super) struct Pairing {
    pair: Pair,
    pin: Entity<TextInput>,
}

impl Shell {
    /// The core reports what a device `tab`'s page pairs with asks, or that it ended (None).
    pub fn set_pair(&mut self, tab: u64, pair: Option<Pair>, cx: &mut Context<Self>) {
        match pair {
            Some(pair) => {
                let placeholder = self.palette.text(FORM_PLACEHOLDER);
                let pin = cx.new(|cx| {
                    TextInput::new("PIN", cx)
                        .placeholder_color(placeholder)
                        .spoken("Bluetooth PIN")
                });
                let asks_pin = pair.kind == PairKind::Pin;
                self.pairs.insert(tab, Pairing { pair, pin });
                if tab == self.selected {
                    // The card needs the sidebar, so a hidden one comes out with it.
                    if self.can_reveal() {
                        self.set_revealed(true, cx);
                    }
                    // The PIN field takes the keyboard, as Chrome's dialog's does.
                    if asks_pin {
                        self.browser.focus_ui(cx);
                        self.pair_wants_keyboard = true;
                    }
                }
            }
            None if self.pairs.remove(&tab).is_none() => return,
            None => {}
        }
        cx.notify();
    }

    /// The prompt of the tab shown, unless its page's question or chooser has the card.
    pub(super) fn pairing(&self) -> Option<&Pairing> {
        self.pairs
            .get(&self.selected)
            .filter(|_| self.question().is_none() && self.chooser().is_none())
    }

    /// Gives the PIN field the keyboard, once it's drawn.
    pub(super) fn focus_pair(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.pair_wants_keyboard) {
            return;
        }
        if let Some(pairing) = self.pairing() {
            window.focus(&pairing.pin.focus_handle(cx), cx);
            // check.sh waits for this line, which says the field has the keyboard.
            log!("browser: pair pin field focused");
        }
    }

    fn answer_pair(&mut self, tab: u64, cx: &mut Context<Self>) {
        let Some(pairing) = self.pairs.get(&tab) else {
            return;
        };
        let pin = match pairing.pair.kind {
            PairKind::Pin => {
                let pin = pairing.pin.read(cx).committed();
                if !pin_takes(&pin) {
                    return;
                }
                Some(SharedString::from(pin))
            }
            _ => None,
        };
        self.pairs.remove(&tab);
        log!("browser: command pair via card");
        self.browser.pair(tab, pin, cx);
        cx.notify();
    }

    fn cancel_pair(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.pairs.remove(&tab).is_some() {
            log!("browser: command pair-close via card");
            self.browser.cancel_pair(tab, cx);
            cx.notify();
        }
    }

    /// The tab shown's prompt, as a card under the URL field.
    pub(super) fn pair_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let Some(pairing) = self.pairing() else {
            // Drawn again, it's logged again.
            self.pair_drawn.set(None);
            return None;
        };
        let tab = self.selected;
        let pair = &pairing.pair;
        let field = (pair.kind == PairKind::Pin).then(|| {
            div()
                .mt(px(BUTTONS_TOP))
                .h(px(BUTTON_HEIGHT))
                .px(px(BUTTON_HEIGHT / 3.))
                .flex()
                .items_center()
                .rounded(px(NAV_RADIUS))
                .bg(self.palette.plate(FORM_PLATE))
                .text_color(self.palette.text(FORM_TEXT))
                .key_context("Naming")
                .on_action(
                    cx.listener(move |shell, _: &KeepName, _, cx| shell.answer_pair(tab, cx)),
                )
                .on_action(
                    cx.listener(move |shell, _: &CancelName, _, cx| shell.cancel_pair(tab, cx)),
                )
                .child(pairing.pin.clone())
        });
        let takes = match pair.kind {
            PairKind::Pin => pin_takes(pairing.pin.read(cx).text()),
            _ => true,
        };
        let button = card_button(&self.palette, "pair", "Pair")
            .when(!takes, |button| button.opacity(0.5))
            .on_click(cx.listener(move |shell, _, _, cx| shell.answer_pair(tab, cx)));
        let close = card_close(&self.palette, "pair-close")
            .on_click(cx.listener(move |shell, _, _, cx| shell.cancel_pair(tab, cx)));
        Some(
            self.plate("pair")
                .child(card_listener("pair", &self.pair_drawn, tab))
                .child(
                    div()
                        .pr(px(BESIDE_CLOSE))
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(pair.title()),
                )
                .child(
                    div()
                        .text_color(self.palette.on_overlay(DETAIL))
                        .child(pair.asks()),
                )
                .children(field)
                .child(div().mt(px(BUTTONS_TOP)).flex().child(button))
                .child(close),
        )
    }
}

#[cfg(test)]
mod tests;
