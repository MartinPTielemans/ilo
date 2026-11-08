//! A page in another language, translated on the device (#671). Arc's URL bar offers to translate
//! a site that isn't in the user's language, and once it's translated, Use Original puts it back.
//! Ours: the offer is a quiet mark after the host, as the password key is, with no card on load;
//! its press opens a card under the URL field, where a page's question shows, which asks, says
//! how it goes and offers Use Original. The command bar has Translate Page and Use Original.
//!
//! The owner's call (2026-10-29): the words never leave the computer. The core says the language
//! each page declares (tab_language) and gathers and shows the page's words; the platform's
//! translator, Apple's Translation on macOS 26 and later, turns them into the user's language with
//! the models the system keeps. Where there's none, as before macOS 26 and on Linux and Windows for
//! now, nothing's offered. A language whose model isn't on the Mac yet says so, and the card opens
//! System Settings, where it's added.
//!
//! Agents get no path: they read a page in its own words, and an agent's Space offers none, so the
//! page an agent reads is never the one the user had translated.

use std::{cell::Cell, rc::Rc};

use gpui::{
    Bounds, Context, FontWeight, Hsla, KeyBinding, MouseButton, Pixels, SharedString, actions,
    canvas, div, prelude::*, px, svg,
};

use super::{
    KEY_CONTEXT, NAV_HOVERED, NAV_RADIUS, SECONDARY, Shell, TITLE,
    access::Accessible,
    commands::Command,
    header::TipSide,
    permission::{BESIDE_CLOSE, BUTTONS_TOP, Look, card_button_as, card_close, card_listener},
};
use crate::tokens::shell::header::{KEY_GAP, KEY_GLYPH, KEY_REACH};
use crate::tokens::shell::permission::{BUTTON_GAP, DETAIL};

/// What the computer is called where it's said that the page stays on it.
#[cfg(target_os = "macos")]
const DEVICE: &str = "this Mac";
#[cfg(not(target_os = "macos"))]
const DEVICE: &str = "this computer";

const GLYPH: &str = "icons/languages.svg";

actions!(translate, [CloseCard]);

/// How a page's translation goes, once asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Translation {
    /// The page's words are with the translator.
    Translating,
    /// The page shows them in the user's language.
    Translated,
    /// The translator has no model on the computer for the page's language yet.
    Missing,
    /// It didn't translate, as with a language the translator can't do.
    Failed,
}

/// Escape while the UI has the keyboard closes the card, and otherwise falls through, as the DRM
/// note's does.
pub(super) fn escape_binding() -> KeyBinding {
    KeyBinding::new("escape", CloseCard, Some(KEY_CONTEXT))
}

pub(super) fn bind(cx: &mut gpui::App) {
    cx.bind_keys([escape_binding()]);
    cx.on_action(|_: &CloseCard, cx| {
        let shell = super::commands::key_shell(cx);
        let closed = shell.and_then(|shell| {
            shell
                .update(cx, |shell, cx| {
                    shell.close_translate_card(cx) || shell.close_portrait_card(cx)
                })
                .ok()
        });
        if !closed.unwrap_or(false) {
            cx.propagate();
        }
    });
}

/// A language's own part of its tag, "pt" of "pt-BR", which says whether two tags are the same
/// language to read.
fn primary(language: &str) -> String {
    let primary = language.split(['-', '_']).next().unwrap_or_default();
    primary.to_ascii_lowercase()
}

impl Shell {
    /// The core says the language `tab`'s page declares, or with an empty one that a new page
    /// came, which shows none of a translation the last one had.
    pub fn set_page_language(&mut self, tab: u64, language: &str, cx: &mut Context<Self>) {
        let changed = match language.is_empty() {
            true => self.page_languages.remove(&tab).is_some(),
            false => {
                self.page_languages
                    .insert(tab, language.to_owned().into())
                    .as_deref()
                    != Some(language)
            }
        };
        let forgot = self.translations.remove(&tab).is_some();
        if self.translate_card == Some(tab) {
            self.translate_card = None;
        }
        if (changed || forgot) && tab == self.field_tab() {
            cx.notify();
        }
    }

    /// How the translation of `tab`'s page went. A page that went first has forgotten it.
    pub fn set_translation(&mut self, tab: u64, translation: Translation, cx: &mut Context<Self>) {
        if self.translations.get(&tab) != Some(&Translation::Translating) {
            return;
        }
        self.translations.insert(tab, translation);
        // A card that asked has its answer; one that went wrong needs the user, so it opens.
        match translation {
            Translation::Translated => {
                if self.translate_card == Some(tab) {
                    self.translate_card = None;
                }
            }
            Translation::Missing | Translation::Failed => self.translate_card = Some(tab),
            Translation::Translating => {}
        }
        if tab == self.field_tab() {
            cx.notify();
        }
    }

    /// The language `tab`'s page is in and the one it would be translated to, where it's offered:
    /// a page that declares a language the user doesn't read, with a translator on the computer,
    /// outside an agent's Space.
    fn translatable(&self, tab: u64) -> Option<(SharedString, SharedString)> {
        let from = self.page_languages.get(&tab)?;
        let to = self.browser.translation_target()?;
        let agents = !self.browser.space_agent(self.space).is_empty();
        let known = !from.is_empty() && from.as_ref() != "und";
        (tab != 0 && known && !agents && primary(from) != primary(&to)).then(|| (from.clone(), to))
    }

    /// Translate Page: the page shown, in the user's language.
    pub(super) fn translate_page(&mut self, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        let Some((from, to)) = self.translatable(tab) else {
            return;
        };
        if self.translations.get(&tab) == Some(&Translation::Translating) {
            return;
        }
        self.translations.insert(tab, Translation::Translating);
        self.browser.translate(tab, &from, &to, cx);
        cx.notify();
    }

    /// Use Original: the page's own words back, or a translation under way stopped.
    pub(super) fn use_original(&mut self, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        if self.translations.remove(&tab).is_none() {
            return;
        }
        if self.translate_card == Some(tab) {
            self.translate_card = None;
        }
        self.browser.use_original(tab, cx);
        cx.notify();
    }

    /// Whether the command bar offers `command`, one of the two translation's.
    pub(super) fn offers_translation(&self, command: Command) -> bool {
        let tab = self.field_tab();
        let shown = self.translations.get(&tab);
        match command {
            Command::TranslatePage => {
                self.translatable(tab).is_some()
                    && !matches!(
                        shown,
                        Some(Translation::Translated | Translation::Translating)
                    )
            }
            Command::UseOriginal => {
                matches!(
                    shown,
                    Some(Translation::Translated | Translation::Translating)
                )
            }
            _ => false,
        }
    }

    fn toggle_translate_card(&mut self, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        // check.sh reads this line.
        log!("browser: translate mark {tab} clicked");
        self.translate_card = match self.translate_card == Some(tab) {
            true => None,
            false => Some(tab),
        };
        cx.notify();
    }

    /// Closes the card, as its × and escape do, and says whether there was one.
    pub(super) fn close_translate_card(&mut self, cx: &mut Context<Self>) -> bool {
        if self.translate_card.take().is_none() {
            return false;
        }
        cx.notify();
        true
    }

    /// The mark after the host for the page the field shows, while it's offered or translated:
    /// quiet as the key until the page is translated, then in the accent, fainter while it
    /// translates.
    pub(super) fn translate_mark(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let tab = self.field_tab();
        let translation = self.translations.get(&tab).copied();
        if translation.is_none() && self.translatable(tab).is_none() {
            // Drawn again, it's logged again.
            self.translate_mark_drawn.set(None);
            return None;
        }
        let color: Hsla = match translation {
            Some(Translation::Translated) => self.palette.accent.into(),
            Some(Translation::Translating) => {
                let mut faint: Hsla = self.palette.accent.into();
                faint.a *= 0.5;
                faint
            }
            _ => {
                let mut quiet = self.palette.text(SECONDARY);
                quiet.a *= 0.6;
                quiet
            }
        };
        let words = match translation {
            Some(Translation::Translated) => "Translated",
            _ => "Translate",
        };
        let drawn = self.translate_mark_drawn.clone();
        let mark = div()
            .id("translate")
            .flex_none()
            .ml(px(KEY_GAP - KEY_REACH))
            .p(px(KEY_REACH))
            .when(self.a11y, |button| button.button_named(words));
        // It lights under the pointer and names itself as the field's other buttons do, so it
        // reads as one (#2112).
        Some(
            self.with_tooltip(mark, Command::TranslatePage, TipSide::Under, cx)
                .rounded(px(NAV_RADIUS))
                .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.toggle_translate_card(cx);
                    }),
                )
                .relative()
                .child(svg().path(GLYPH).size(px(KEY_GLYPH)).text_color(color))
                .child(div().absolute().inset_0().child(mark_frame(drawn))),
        )
    }

    /// The card of the page the field shows, while it's open.
    pub(super) fn translate_card(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let tab = self.field_tab();
        if self.translate_card != Some(tab) || tab == 0 {
            self.translate_card_drawn.set(None);
            return None;
        }
        let translation = self.translations.get(&tab).copied();
        let from = self.page_languages.get(&tab)?;
        let language = self.browser.language_name(from);
        let to = self.browser.translation_target().unwrap_or_default();
        let target = self.browser.language_name(&to);
        let palette = &self.palette;
        let button = |id: &'static str, label: &'static str, look: Look, command: Command| {
            card_button_as(palette, id, label, look)
                .on_click(cx.listener(move |shell, _, _, cx| shell.run(command, "card", cx)))
        };
        let not_now = card_button_as(palette, "translate-not-now", "Not Now", Look::Quiet)
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.close_translate_card(cx);
            }));
        let (title, detail, buttons): (String, String, Vec<gpui::AnyElement>) = match translation {
            None => (
                "Translate this page?".into(),
                format!("From {language} to {target}, on {DEVICE}."),
                vec![
                    not_now.into_any_element(),
                    button(
                        "translate-page",
                        "Translate",
                        Look::Primary,
                        Command::TranslatePage,
                    )
                    .into_any_element(),
                ],
            ),
            Some(Translation::Translating) => (
                format!("Translating from {language}…"),
                format!("The page doesn't leave {DEVICE}."),
                vec![
                    button(
                        "translate-original",
                        "Use Original",
                        Look::Quiet,
                        Command::UseOriginal,
                    )
                    .into_any_element(),
                ],
            ),
            Some(Translation::Translated) => (
                format!("Translated from {language}"),
                format!("To {target}, on {DEVICE}."),
                vec![
                    button(
                        "translate-original",
                        "Use Original",
                        Look::Quiet,
                        Command::UseOriginal,
                    )
                    .into_any_element(),
                ],
            ),
            Some(Translation::Missing) => (
                format!("{language} isn't on {DEVICE} yet"),
                "Add it in Translation Languages, then translate again.".into(),
                vec![
                    card_button_as(palette, "translate-settings", "Settings…", Look::Quiet)
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.browser.translation_settings(cx);
                        }))
                        .into_any_element(),
                    button(
                        "translate-again",
                        "Translate",
                        Look::Primary,
                        Command::TranslatePage,
                    )
                    .into_any_element(),
                ],
            ),
            Some(Translation::Failed) => (
                "This page didn't translate".into(),
                format!("{DEVICE_UPPER} can't translate it from {language} to {target}."),
                vec![not_now.into_any_element()],
            ),
        };
        let close =
            card_close(palette, "translate-close").on_click(cx.listener(|shell, _, _, cx| {
                shell.close_translate_card(cx);
            }));
        let listener = card_listener("translate card", &self.translate_card_drawn, tab);
        Some(
            self.plate("translate-card")
                .child(listener)
                .child(
                    div()
                        .pr(px(BESIDE_CLOSE))
                        .text_color(palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(div().text_color(palette.on_overlay(DETAIL)).child(detail))
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .flex()
                        .gap(px(BUTTON_GAP))
                        .children(buttons),
                )
                .child(close),
        )
    }
}

/// As DEVICE, starting a sentence.
#[cfg(target_os = "macos")]
const DEVICE_UPPER: &str = "This Mac";
#[cfg(not(target_os = "macos"))]
const DEVICE_UPPER: &str = "This computer";

/// Logs where the mark is, for check.sh to press it, when that changes.
fn mark_frame(drawn: Rc<Cell<Option<Bounds<Pixels>>>>) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            if drawn.replace(Some(bounds)) != Some(bounds) {
                let at = bounds.center();
                // check.sh reads this line.
                log!(
                    "browser: translate mark drawn at {:.0} {:.0}",
                    f32::from(at.x),
                    f32::from(at.y)
                );
            }
        },
        |_, _, _, _| {},
    )
    .size_full()
}

#[cfg(test)]
mod tests;
