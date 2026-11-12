//! A site's Boost (#695, crate::boosts): the card under the URL field where it's made, edited and
//! zapped, which New Boost and Edit Boost open, as do Site Controls' Boost tile and "…" menu.
//! Arc's Boost opened an editor of its own; ours is the card, and the CSS is a file the user's own
//! editor opens, whose each save shows at once (crate::boosts::watch). Zap, Arc's, hides what the
//! user clicks on the page, now and on each of the site's pages from then on, until Done or
//! escape.
//!
//! A private window shows its profile's Boosts but makes none, as it keeps nothing, and an agent's
//! Space offers none: a Boost is the user's own view of their pages (principle 3, decision 8).

use std::rc::Rc;

use gpui::{Context, FontWeight, KeyBinding, actions, div, point, prelude::*, px};

use super::{
    KEY_CONTEXT, Shell, TITLE,
    commands::Command,
    permission::{BESIDE_CLOSE, BUTTONS_TOP, Look, card_button_as, card_close, card_listener},
};
use crate::tokens::shell::permission::{BUTTON_GAP, DETAIL};
use crate::tokens::shell::{URL_FIELD_HEIGHT, URL_FIELD_TOP};
use crate::{
    boost_editor::{self, Request},
    boosts,
    palette::{Palette, Tone},
    theme_picker::At,
};

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
        self.read_profile_boosts(profile, cx);
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

    /// Reads `profile`'s Boosts the first time, so what's offered knows the ones it has.
    fn read_profile_boosts(&self, profile: u64, cx: &mut gpui::App) {
        if !boosts::with(|boosts| boosts.loaded(profile)) {
            self.browser.load_boosts(profile, cx);
        }
    }

    /// Reads the Boosts of the profile of `tab`'s page the first time, before a menu offers its
    /// Boost.
    pub(super) fn read_boosts_of(&self, tab: u64, cx: &mut gpui::App) {
        if let Some((profile, _)) = self.boost_site(tab) {
            self.read_profile_boosts(profile, cx);
        }
    }

    /// The sidebar's menus' item for the Boost of `tab`'s site: New Boost… without one, Edit
    /// Boost… with one, and none where its page can't have one or Settings' switch is off.
    pub(super) fn boost_entry(&self, tab: u64) -> Option<&'static str> {
        let (profile, site) = self.boost_site(tab)?;
        boosts::with(|boosts| match boosts.all_off() {
            true => None,
            false if boosts.get(profile, &site).is_some() => Some("Edit Boost…"),
            false => Some("New Boost…"),
        })
    }

    /// The card for the site of `tab`'s page, from its row's menu: a tab not shown is shown first,
    /// and its card opens once it is, as the card hangs under the URL field.
    pub(super) fn open_boost_for(&mut self, tab: u64, cx: &mut Context<Self>) {
        if tab == self.field_tab() {
            return self.open_boost(cx);
        }
        self.boost_pending = Some(tab);
        self.browser.select(tab, cx);
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

    /// Customize…: the editor without code, beside the sidebar, its arrow at the URL field, in
    /// place of the card.
    fn customize_boost(&mut self, cx: &mut Context<Self>) {
        let Some((profile, site, boost)) = self.shown_boost(cx) else {
            return;
        };
        self.close_boost_card(cx);
        let style = boosts::with(|boosts| boosts.style(profile, &site));
        // A new Boost is on as soon as it has edits.
        let on = boost.is_none_or(|boost| boost.on);
        let shell = cx.weak_entity();
        let editing = site.clone();
        let requested: boost_editor::Requested = Rc::new(move |request, cx| {
            shell
                .update(cx, |shell, cx| {
                    shell.boost_requested(profile, &editing, request, cx)
                })
                .ok();
        });
        let anchor = point(self.page_edge_x(), URL_FIELD_TOP + URL_FIELD_HEIGHT / 2.);
        let height = f32::from(self.viewport.height);
        // The editor is a dark sheet over a dark or a light window, as the theme picker is.
        let theme = self.spaces.iter().find(|each| each.id == self.space);
        let palette = Palette::of(theme.and_then(|each| each.theme).as_ref(), Tone::Dark);
        self.browser.hover_card(crate::hover_card::Hover::Close, cx);
        let window = self.browser.window();
        // The page's favicon, a one-colour one in the ink, as the sidebar draws it on the dark
        // sheet.
        let icon = (self.tabs.get(&self.field_tab()))
            .and_then(|each| each.favicon.clone())
            .map(
                |favicon| match super::tile_theme::ink_copy(&favicon, Tone::Dark) {
                    Some(copy) => gpui::ImageSource::from(copy),
                    None => gpui::ImageSource::from(favicon),
                },
            );
        boost_editor::show(
            window,
            site,
            icon,
            style,
            on,
            At::Row(anchor),
            self.right,
            height,
            palette,
            requested,
            cx,
        );
    }

    /// What the editor asks of `site`'s Boost.
    fn boost_requested(
        &mut self,
        profile: u64,
        site: &str,
        request: Request,
        cx: &mut Context<Self>,
    ) {
        match request {
            Request::Style(style, kept) => {
                let changes = boosts::with(|boosts| boosts.set_style(profile, site, &style, kept));
                self.browser.show_boosts(changes, cx);
                if kept {
                    self.boosts_changed(cx);
                }
            }
            Request::On(on) => self.set_boost_on(profile, site, on, cx),
            Request::EditCss => self.edit_boost_file(profile, site, cx),
            Request::EditScript => self.edit_boost_script_file(profile, site, cx),
            Request::Zap => {
                // Zap's own card says what to do and ends it.
                self.boost_card = Some(self.field_tab());
                self.start_zap(cx);
            }
            Request::Delete => self.delete_boost(profile, site, cx),
        }
    }

    /// Closes the card, as its × does, ending Zap, and says whether there was one.
    pub(super) fn close_boost_card(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(tab) = self.boost_card.take() else {
            return false;
        };
        // check.sh reads this line.
        log!("browser: boost card {tab} closed");
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

    /// Edit Script…, from the editor's footer or the Library row: the site's script in the user's
    /// editor, started if it has none. It runs only once the user turns it on in the browser
    /// (#2297). The card has no button for it: a script is the rarest and riskiest of the tools,
    /// so it's one step in, under Customize….
    pub(super) fn edit_boost_script_file(
        &mut self,
        profile: u64,
        site: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = boosts::with(|boosts| boosts.script_file(profile, site)) else {
            return;
        };
        log!("browser: boost edit script {site}");
        let changes = boosts::with(|boosts| boosts.reload(profile));
        self.browser.show_boosts(changes, cx);
        self.browser.edit_file(&path, cx);
        cx.notify();
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
        // Settings' switch takes Window › View Boosts… away, and gives it back.
        self.update_menus(cx);
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
        let customize = quiet("boost-customize", "Customize…")
            .on_click(cx.listener(|shell, _, _, cx| shell.customize_boost(cx)));
        let zaps = boost.as_ref().map_or(0, |boost| boost.summary.zaps);
        // Customize… has a line of its own under the other two, as three don't fit the card, so
        // Zap's row stays where it is as Zap starts and Done comes up under the pointer.
        let customize =
            (self.zapping != Some(tab)).then(|| div().mt(px(BUTTON_GAP)).flex().child(customize));
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
                .children(customize)
                .child(close),
        )
    }
}

#[cfg(test)]
mod tests;
