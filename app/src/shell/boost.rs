//! A site's Boost (#695, crate::boosts): the card under the URL field where it's made, edited and
//! zapped, which New Boost and Edit Boost open, as do Site Controls' Boost tile and "…" menu.
//! Arc's Boost opened an editor of its own; ours is the card, and the CSS is a file the user's own
//! editor opens, whose each save shows at once (crate::boosts::watch). Zap, Arc's, hides what the
//! user clicks on the page, now and on each of the site's pages from then on, until Done or
//! escape.
//!
//! A private window shows its profile's Boosts but makes none, as it keeps nothing, and an agent's
//! Space offers none: a Boost is the user's own view of their pages (principle 3, decision 8).

use gpui::{Context, FontWeight, KeyBinding, actions, div, prelude::*, px};

use super::{
    KEY_CONTEXT, Shell, TITLE,
    commands::Command,
    permission::{BESIDE_CLOSE, BUTTONS_TOP, Look, card_button_as, card_close, card_listener},
};
use crate::boosts;
use crate::tokens::shell::permission::{BUTTON_GAP, DETAIL};

actions!(boost, [Escape]);

/// Escape while the UI has the keyboard ends Zap, or else closes the card, and otherwise falls
/// through, as the DRM note's does.
pub(super) fn escape_binding() -> KeyBinding {
    KeyBinding::new("escape", Escape, Some(KEY_CONTEXT))
}

pub(super) fn bind(cx: &mut gpui::App) {
    cx.bind_keys([escape_binding()]);
    cx.on_action(|_: &Escape, cx| {
        let shell = super::commands::key_shell(cx);
        let done =
            shell.and_then(|shell| shell.update(cx, |shell, cx| shell.escape_boost(cx)).ok());
        if !done.unwrap_or(false) {
            cx.propagate();
        }
    });
}

impl Shell {
    /// The profile and site of `tab`'s page where it can have a Boost: an http(s) page in a
    /// regular window, outside an agent's Space.
    pub(super) fn boost_site(&self, tab: u64) -> Option<(u64, String)> {
        let agents = !self.browser.space_agent(self.space).is_empty();
        if tab == 0 || self.private || agents {
            return None;
        }
        let profile = self.shown_space()?.profile;
        Some((profile, boosts::site_of(&self.url(tab))?))
    }

    /// The site of the page shown and its Boost, if it has one, reading its profile's Boosts the
    /// first time. None with Settings' switch off, where nothing of Boosts is offered.
    pub(super) fn shown_boost(
        &self,
        cx: &mut gpui::App,
    ) -> Option<(u64, String, Option<boosts::Boost>)> {
        let (profile, site) = self.boost_site(self.field_tab())?;
        if !boosts::with(|boosts| boosts.loaded(profile)) {
            self.browser.load_boosts(profile, cx);
        }
        let (off, boost) = boosts::with(|boosts| (boosts.all_off(), boosts.get(profile, &site)));
        (!off).then_some((profile, site, boost))
    }

    /// Whether `tab`'s page can have a Boost, and whether its site's is on, none without one, for
    /// its Site Controls.
    pub fn boost_state(&self, tab: u64) -> (bool, Option<bool>) {
        let Some((profile, site)) = self.boost_site(tab) else {
            return (false, None);
        };
        boosts::with(|boosts| match boosts.all_off() {
            true => (false, None),
            false => (true, boosts.get(profile, &site).map(|boost| boost.on)),
        })
    }

    /// Site Controls' switch: the Boost of the site of `tab`'s page on or off there.
    pub fn set_tab_boost_on(&mut self, tab: u64, on: bool, cx: &mut Context<Self>) {
        if let Some((profile, site)) = self.boost_site(tab) {
            self.set_boost_on(profile, &site, on, cx);
        }
    }

    /// Whether the command bar offers `command`, one of Boosts'. It reads only what's read
    /// already, as the bar asks while it draws.
    pub(super) fn offers_boost(&self, command: Command) -> bool {
        let (all_off, boost) = boosts::with(|boosts| {
            let site = self.boost_site(self.field_tab());
            let boost = site.map(|(profile, site)| boosts.get(profile, &site).is_some());
            (boosts.all_off(), boost)
        });
        match command {
            _ if all_off => false,
            Command::NewBoost => boost == Some(false),
            Command::EditBoost => boost == Some(true),
            Command::ViewBoosts => !self.private && self.browser.space_agent(self.space).is_empty(),
            _ => false,
        }
    }

    /// New Boost and Edit Boost: the card for the site of the page shown.
    pub(super) fn open_boost(&mut self, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        if self.shown_boost(cx).is_none() {
            return;
        }
        // check.sh reads this line.
        log!("browser: boost card {tab} opened");
        self.boost_card = Some(tab);
        cx.notify();
    }

    /// Closes the card, as its × does, ending Zap, and says whether there was one.
    pub(super) fn close_boost_card(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(tab) = self.boost_card.take() else {
            return false;
        };
        if self.zapping == Some(tab) {
            self.stop_zap(cx);
        }
        cx.notify();
        true
    }

    /// Escape: Zap ends on the page shown, or else its card closes. Says whether either did.
    pub fn escape_boost(&mut self, cx: &mut Context<Self>) -> bool {
        let tab = self.field_tab();
        match self.zapping == Some(tab) && tab != 0 {
            true => {
                self.stop_zap(cx);
                true
            }
            false if self.boost_card == Some(tab) => self.close_boost_card(cx),
            false => false,
        }
    }

    /// Whether Zap is on on `tab`'s page, for the app's escape typed in the page.
    pub fn zapping(&self, tab: u64) -> bool {
        tab != 0 && self.zapping == Some(tab)
    }

    fn start_zap(&mut self, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        if self.shown_boost(cx).is_none() || self.zapping == Some(tab) {
            return;
        }
        // One page at a time, as Zap is the pointer's.
        self.stop_zap(cx);
        self.zapping = Some(tab);
        self.browser.zap(tab, true, cx);
        cx.notify();
    }

    fn stop_zap(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.zapping.take() {
            self.browser.zap(tab, false, cx);
            cx.notify();
        }
    }

    /// The core says what the user clicked on `tab`'s page with Zap on, as a selector, or none for
    /// the page itself: its Boost hides it from now on, and at once.
    pub fn boost_zapped(&mut self, tab: u64, selector: Option<&str>, cx: &mut Context<Self>) {
        let Some(selector) = selector.filter(|_| self.zapping == Some(tab)) else {
            return;
        };
        let Some((profile, site)) = self.boost_site(tab) else {
            return;
        };
        let changes = boosts::with(|boosts| boosts.zap(profile, &site, selector));
        self.browser.show_boosts(changes, cx);
        cx.notify();
    }

    /// Zap ended on `tab`'s page, as a new page there does.
    pub fn boost_zap_ended(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.zapping == Some(tab) {
            self.zapping = None;
            cx.notify();
        }
    }

    fn undo_zap(&mut self, cx: &mut Context<Self>) {
        let Some((profile, site, _)) = self.shown_boost(cx) else {
            return;
        };
        let changes = boosts::with(|boosts| boosts.undo_zap(profile, &site));
        self.browser.show_boosts(changes, cx);
        cx.notify();
    }

    /// Edit CSS…: the site's file in the user's editor, started if it has none.
    fn edit_boost_css(&mut self, cx: &mut Context<Self>) {
        let Some((profile, site, _)) = self.shown_boost(cx) else {
            return;
        };
        self.edit_boost_file(profile, &site, cx);
    }

    pub(super) fn edit_boost_file(&mut self, profile: u64, site: &str, cx: &mut Context<Self>) {
        let Some(path) = boosts::with(|boosts| boosts.file(profile, site)) else {
            return;
        };
        log!("browser: boost edit {site}");
        let changes = boosts::with(|boosts| boosts.reload(profile));
        self.browser.show_boosts(changes, cx);
        self.browser.edit_file(&path, cx);
        cx.notify();
    }

    /// The Boosts changed outside the window, as a file saved in the user's editor: what shows
    /// them draws again.
    pub fn boosts_changed(&mut self, cx: &mut Context<Self>) {
        if self.library.is_some() && self.section == super::library::Section::Boosts {
            self.list_boosts(cx);
        }
        cx.notify();
    }

    /// The card of the page the field shows, while it's open: a new Boost, the Boost, or Zap's.
    pub(super) fn boost_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let tab = self.field_tab();
        let shown = (self.boost_card == Some(tab))
            .then(|| self.shown_boost(cx))
            .flatten();
        let Some((_, site, boost)) = shown else {
            self.boost_card_drawn.set(None);
            return None;
        };
        let palette = &self.palette;
        let quiet =
            |id: &'static str, label: &'static str| card_button_as(palette, id, label, Look::Quiet);
        let edit = quiet("boost-edit", "Edit CSS…")
            .on_click(cx.listener(|shell, _, _, cx| shell.edit_boost_css(cx)));
        let zap = card_button_as(palette, "boost-zap", "Zap", Look::Primary)
            .on_click(cx.listener(|shell, _, _, cx| shell.start_zap(cx)));
        let zaps = boost.as_ref().map_or(0, |boost| boost.summary.zaps);
        let (title, detail, buttons): (String, String, Vec<gpui::AnyElement>) =
            match (self.zapping == Some(tab), boost) {
                (true, _) => (
                    "Click what to hide".into(),
                    "Each click hides it now and from now on. Esc when you're done.".into(),
                    (zaps > 0)
                        .then(|| {
                            quiet("boost-undo", "Undo")
                                .on_click(cx.listener(|shell, _, _, cx| shell.undo_zap(cx)))
                                .into_any_element()
                        })
                        .into_iter()
                        .chain(
                            [card_button_as(palette, "boost-done", "Done", Look::Primary)
                                .on_click(cx.listener(|shell, _, _, cx| shell.stop_zap(cx)))
                                .into_any_element()],
                        )
                        .collect(),
                ),
                (false, None) => (
                    site,
                    "A new Boost. Zap what you don't want to see, or style it with your own CSS."
                        .into(),
                    vec![edit.into_any_element(), zap.into_any_element()],
                ),
                (false, Some(boost)) => (
                    site,
                    match boost.on {
                        true => boost.summary.describe(),
                        false => format!("{} Off on this site.", boost.summary.describe()),
                    },
                    vec![edit.into_any_element(), zap.into_any_element()],
                ),
            };
        let close = card_close(palette, "boost-close").on_click(cx.listener(|shell, _, _, cx| {
            shell.close_boost_card(cx);
        }));
        let listener = card_listener("boost card", &self.boost_card_drawn, tab);
        Some(
            self.plate("boost-card")
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

#[cfg(test)]
mod tests;
