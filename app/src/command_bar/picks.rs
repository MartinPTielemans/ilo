//! The user's tabs picked in the bar's composer for the thread it starts (`crate::tab_picks`):
//! the add menu's row for them, their chips over the field, and how the bar's height follows.

use gpui::{App, Context, Window, canvas, prelude::*};

use super::{CHIP, CommandBar, DIM, Surface};
use crate::palette::Palette;
use crate::tab_picks::{self, Offer, Offered};

impl CommandBar {
    /// The add menu's row for the user's tabs, where the window lists any: none over a private
    /// window, which starts no thread.
    pub(super) fn tabs_row(&self, cx: &mut Context<Self>) -> Option<crate::plus::Row> {
        if !self.host.lists_tabs_to_hand(cx) {
            return None;
        }
        let picked = self.ask.as_ref().map(|ask| ask.picks.tabs());
        let (host, picked, flips) = (self.host.clone(), picked?, cx.weak_entity());
        Some(tab_picks::row(
            // The add menu opened: the tabs the thread may be handed, asked now and here alone,
            // with which of them are picked.
            move |cx| (host.tabs_to_hand(cx), picked),
            move |space, offered, cx| {
                let picked = flips.update(cx, |bar, cx| bar.flip_pick(space, offered, cx));
                picked.unwrap_or(false)
            },
        ))
    }

    /// The user's click on a tab's row of the menu. Whether it is picked now.
    fn flip_pick(&mut self, space: u64, offered: Offered, cx: &mut Context<Self>) -> bool {
        let Some(ask) = &mut self.ask else {
            return false;
        };
        let picked = ask.picks.flip(space, offered);
        self.picks_changed(cx);
        picked
    }

    /// The chips changed. The window's tabs are watched while there is one, and not otherwise:
    /// a tab closed, moved to another Space or gone to a page of the browser's own loses its
    /// chip, and one that stays shows as its tab is now.
    fn picks_changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
        let Some(ask) = &mut self.ask else { return };
        if ask.picks.is_empty() {
            ask.picks_watch = None;
            self.picks_height.set(0.);
            return self.fit_picks();
        }
        if ask.picks_watch.is_none() {
            let bar = cx.weak_entity();
            let kept = move |listed: Offer, cx: &mut App| {
                bar.update(cx, |bar, cx| {
                    if (bar.ask.as_mut()).is_some_and(|ask| ask.picks.keep(&listed)) {
                        bar.picks_changed(cx);
                    }
                })
                .ok();
            };
            ask.picks_watch = self.host.tabs_listed(Box::new(kept), cx);
        }
    }

    /// The bar's own window is as tall as what it draws, its chips' rows with it.
    fn fit_picks(&self) {
        if self.surface == Surface::Window && self.ask.is_some() {
            self.host.set_height(self.drawn_height());
        }
    }

    /// The user's press on the ✕ of `tab`'s chip.
    pub(super) fn unpick(&mut self, tab: u64, cx: &mut Context<Self>) {
        if (self.ask.as_mut()).is_some_and(|ask| ask.picks.unpick(tab)) {
            self.picks_changed(cx);
        }
    }

    /// ⌫ in the empty field takes the last chip. Whether it did.
    pub(super) fn unpick_last(&mut self, cx: &mut Context<Self>) -> bool {
        let field = self.input.read(cx);
        if field.composing() || !field.text().is_empty() {
            return false;
        }
        if !(self.ask.as_mut()).is_some_and(|ask| ask.picks.unpick_last()) {
            return false;
        }
        self.picks_changed(cx);
        true
    }

    /// Whether a chip is held, with which the composer starts its thread without words.
    pub(super) fn picked(&self) -> bool {
        (self.ask.as_ref()).is_some_and(|ask| !ask.picks.is_empty())
    }

    /// The chips over the composer's field, with a pick: each ✕ unpicks its tab. How tall they
    /// drew, wrapping, is the bar's to count (`drawn_height`), once laid out.
    pub(super) fn picked_chips(
        &self,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let ask = self.ask.as_ref().filter(|ask| !ask.picks.is_empty())?;
        let look = tab_picks::Look {
            fill: palette.plate(CHIP),
            ink: palette.ink.into(),
            dim: palette.on_overlay(DIM),
            a11y: self.a11y,
        };
        let bar = cx.weak_entity();
        let unpick = move |tab, _: &mut Window, cx: &mut App| {
            bar.update(cx, |bar, cx| bar.unpick(tab, cx)).ok();
        };
        let (height, bar) = (self.picks_height.clone(), cx.weak_entity());
        let measured = canvas(
            move |bounds, _, cx| {
                let tall = f32::from(bounds.size.height);
                if height.replace(tall) != tall {
                    let bar = bar.clone();
                    cx.defer(move |cx| {
                        bar.update(cx, |bar, cx| {
                            bar.fit_picks();
                            cx.notify();
                        })
                        .ok();
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        Some(
            tab_picks::chips(&ask.picks, &look, unpick)
                .relative()
                .child(measured),
        )
    }
}
