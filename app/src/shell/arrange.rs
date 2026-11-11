//! Arrange (#2347): the user moves what's on a site's pages, and the Boost keeps it
//! (crate::boosts::arrange). The card's Arrange starts it on the page shown; the core runs it and
//! hands each change back to be saved. The card lists the changes, each with a way back, and Site
//! Controls turns on an arrangement that changed outside ilo.
//!
//! As Zap, it's the user's alone: offered only where a Boost is, never in an agent's Space or a
//! private window, and no agent tool reaches it, its list or its file (principle 3).

use gpui::{Context, FontWeight, div, prelude::*, px, svg};

use super::{
    NAV_RADIUS, Shell, TITLE, URL_FIELD,
    permission::{Look, card_button_as},
};
use crate::boosts::{self, Arranged};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::theme::packed;
use crate::tokens::shell::boost::{
    LIST_TOP, MARK_LINE, MARK_RADIUS, MISFIT, PUT_BACK, PUT_BACK_GLYPH, ROW_GAP, ROW_TOP,
};
use crate::tokens::shell::permission::{BUTTON_GAP, DETAIL};

impl Shell {
    /// The card's Arrange: the mode on the page shown, in place of the card and of Zap.
    pub(super) fn start_arrange(&mut self, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        if self.shown_boost(cx).is_none() || self.arranging == Some(tab) {
            return;
        }
        self.close_boost_card(cx);
        self.stop_arrange(cx);
        // check.sh reads this line.
        log!("browser: arrange {tab} started");
        // The marks in the page are the Space's, as the card's own controls are.
        let palette = &self.palette;
        let look = boosts::arrange::Look {
            accent: packed(palette.accent),
            label: packed(palette.accent_label),
            ink: packed(palette.ink),
            line: MARK_LINE,
            radius: MARK_RADIUS,
        };
        if self.browser.arrange(tab, Some(&look), cx) {
            self.arranging = Some(tab);
        }
        cx.notify();
    }

    pub(super) fn stop_arrange(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.arranging.take() {
            self.browser.arrange(tab, None, cx);
            cx.notify();
        }
    }

    /// Whether Arrange is on on `tab`'s page.
    pub fn arranging(&self, tab: u64) -> bool {
        tab != 0 && self.arranging == Some(tab)
    }

    /// The core hands over `site`'s arrangement after a change on `tab`'s page, which it has
    /// placed and recorded: the app keeps it in the site's Boost.
    pub fn arrange_saved(&mut self, tab: u64, site: &str, bytes: &str, cx: &mut Context<Self>) {
        let Some((profile, shown)) = self.boost_site(tab).filter(|(_, shown)| shown == site) else {
            return;
        };
        self.read_profile_boosts(profile, cx);
        let changes = boosts::with(|boosts| boosts.save_arrangement(profile, &shown, bytes));
        self.browser.show_boosts(changes, cx);
        self.boosts_changed(cx);
    }

    /// Arrange ended on `tab`'s page, as Done or a new page ends it.
    pub fn arrange_ended(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.arranging == Some(tab) {
            self.arranging = None;
            cx.notify();
        }
    }

    /// Which of `site`'s changes didn't find their element as `tab`'s page loaded, by index.
    pub fn arrangement_fits(
        &mut self,
        tab: u64,
        site: &str,
        misfits: Vec<usize>,
        cx: &mut Context<Self>,
    ) {
        let Some((profile, shown)) = self.boost_site(tab).filter(|(_, shown)| shown == site) else {
            return;
        };
        boosts::with(|boosts| boosts.set_misfits(profile, &shown, misfits));
        self.boosts_changed(cx);
    }

    /// ↺ and ×: the change at `index` goes from the site's arrangement, and its thing goes back.
    fn put_back_arranged(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((profile, site, _)) = self.shown_boost(cx) else {
            return;
        };
        let Some(left) = boosts::with(|boosts| boosts.without(profile, &site, index)) else {
            return;
        };
        // The core records the user's edit before the file changes, or it would read the new
        // bytes as changed outside ilo. The last change back takes the arrangement away instead.
        if !left.is_empty() && !self.browser.keep_arrangement(profile, &site, &left) {
            return;
        }
        let changes = boosts::with(|boosts| boosts.put_back(profile, &site, index, &left));
        self.browser.show_boosts(changes, cx);
        self.boosts_changed(cx);
    }

    /// Turn On, for an arrangement the core keeps off: the user keeps it as it is now.
    pub fn turn_on_arrangement(&mut self, tab: u64, cx: &mut Context<Self>) {
        let Some((profile, site)) = self.boost_site(tab) else {
            return;
        };
        let arrangement =
            boosts::with(|boosts| boosts.get(profile, &site)).and_then(|boost| boost.arrangement);
        if !arrangement.is_some_and(|arrangement| turns_on(&arrangement)) {
            return;
        }
        let Some(bytes) = boosts::with(|boosts| boosts.arrangement(profile, &site)) else {
            return;
        };
        log!("browser: arrangement for {site} turned on");
        if self.browser.keep_arrangement(profile, &site, &bytes) {
            boosts::with(|boosts| boosts.set_arranged(profile, &site, Arranged::On));
        }
        self.boosts_changed(cx);
    }

    /// What Site Controls says of the arrangement of `tab`'s site, if it has one: after the Boost's
    /// state, "1 change no longer fits", and why it's off where Turn On can turn it on, which its
    /// row says. One that can't be read is the card's to say.
    pub fn arrangement_note(&self, tab: u64) -> (Option<String>, Option<&'static str>) {
        let Some((profile, site)) = self.boost_site(tab) else {
            return (None, None);
        };
        let arrangement =
            boosts::with(|boosts| boosts.get(profile, &site)).and_then(|boost| boost.arrangement);
        let Some(arrangement) = arrangement else {
            return (None, None);
        };
        let misfits = match arrangement.misfits.len() {
            0 => None,
            1 => Some("1 change no longer fits".to_owned()),
            count => Some(format!("{count} changes no longer fit")),
        };
        let off = off_because(&arrangement).filter(|_| turns_on(&arrangement));
        (misfits, off)
    }

    /// The card's list of what Arrange moved on the site, under its words.
    pub(super) fn arranged_list(
        &self,
        arrangement: &boosts::Arrangement,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = &self.palette;
        let small = px(crate::tokens::text::SMALL);
        let line = |text: String| {
            div()
                .truncate()
                .text_size(small)
                .text_color(palette.on_overlay(DETAIL))
                .child(text)
        };
        let off = off_because(arrangement);
        let changes = arrangement.changes.clone().unwrap_or_default();
        let rows = changes.iter().enumerate().map(|(index, change)| {
            let fits = !arrangement.misfits.contains(&index);
            let (what, glyph) = match fits {
                true => (change.describe(), "icons/undo-2.svg"),
                false => ("No longer fits".into(), "icons/x.svg"),
            };
            // Off, the file is the user's to turn on as it is, so it isn't edited here.
            let button = (arrangement.state == Some(Arranged::On)).then(|| {
                div()
                    .id(("arranged-put-back", index))
                    .flex_none()
                    .size(px(PUT_BACK))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(NAV_RADIUS))
                    .bg(palette.plate(URL_FIELD))
                    .hover(|style| style.bg(palette.state(URL_FIELD, STATE_HOVERED)))
                    .active(|style| style.bg(palette.state(URL_FIELD, STATE_PRESSED)))
                    .child(
                        svg()
                            .path(glyph)
                            .size(px(PUT_BACK_GLYPH))
                            .text_color(palette.text(TITLE)),
                    )
                    .on_click(
                        cx.listener(move |shell, _, _, cx| shell.put_back_arranged(index, cx)),
                    )
            });
            div()
                .flex()
                .items_center()
                .gap(px(ROW_GAP))
                .pt(px(ROW_TOP))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_color(palette.text(TITLE))
                                .when(!fits, |name| name.opacity(MISFIT))
                                .child(change.title()),
                        )
                        .child(line(what)),
                )
                .children(button)
        });
        let turn_on = turns_on(arrangement).then(|| {
            div().mt(px(BUTTON_GAP)).flex().child(
                card_button_as(palette, "arranged-turn-on", "Turn On", Look::Quiet).on_click(
                    cx.listener(|shell, _, _, cx| shell.turn_on_arrangement(shell.field_tab(), cx)),
                ),
            )
        });
        div()
            .mt(px(LIST_TOP))
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(small)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.on_overlay(DETAIL))
                    .child("Arranged"),
            )
            .children(off.map(|off| line(off.into())))
            .children(rows)
            .children(turn_on)
    }
}

/// Why the core keeps `arrangement` off, if it does, in the words the card and Site Controls say.
fn off_because(arrangement: &boosts::Arrangement) -> Option<&'static str> {
    match (&arrangement.changes, arrangement.state) {
        (None, _) | (_, Some(Arranged::Refused)) => Some("Can't be read, off"),
        (_, Some(Arranged::Changed)) => Some("Changed outside ilo, off"),
        (_, Some(Arranged::New)) => Some("New on this computer, off"),
        _ => None,
    }
}

/// Whether Turn On can turn `arrangement` on: one the core keeps off for where it came from, not
/// for what's in it.
fn turns_on(arrangement: &boosts::Arrangement) -> bool {
    arrangement.changes.is_some()
        && matches!(arrangement.state, Some(Arranged::New | Arranged::Changed))
}
