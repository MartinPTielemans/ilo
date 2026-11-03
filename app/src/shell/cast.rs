//! A tab's Cast dialog, as a card under the URL field where a page's question hangs: Chrome's
//! header, such as "Cast tab", over the devices its media router finds on the network, each its
//! name over what it's doing. A click on one casts the tab to it, or stops it casting, as on
//! Chrome's; × closes the card, and so does casting starting, as Chrome's dialog does. Chrome's
//! hangs from its toolbar's Cast button, which our layout hasn't. It opens from View › Cast…, the
//! command bar, the page menu's Cast… and a page asking to cast, and shows over its tab alone.

use gpui::{Context, FontWeight, SharedString, div, prelude::*, px, rgba, svg};

use super::{
    NAV, NAV_RADIUS, SECONDARY, Shell, TITLE,
    permission::{BESIDE_CLOSE, card_close, card_listener},
};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::tokens::shell::cast::{GLYPH, NAME, ROW, ROW_GAP, STATUS};
use crate::tokens::shell::permission::DETAIL;

/// A tab's Cast dialog, as the core reports it.
#[derive(Clone, PartialEq, Debug)]
pub struct Cast {
    /// Chrome's, such as "Cast tab".
    pub header: SharedString,
    pub devices: Vec<CastDevice>,
}

/// A device Chrome found to cast to.
#[derive(Clone, PartialEq, Debug)]
pub struct CastDevice {
    /// Chrome's, for `Browser::cast_to`.
    pub id: SharedString,
    pub name: SharedString,
    pub state: CastState,
    /// Chrome's words for what it's doing or what went wrong, or "".
    pub status: SharedString,
}

/// What a device is doing, as Chrome's dialog shows it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CastState {
    Available,
    Connecting,
    Connected,
    Disconnecting,
    Unavailable,
}

impl Shell {
    /// The core reports `tab`'s Cast dialog, or that it closed (None). It shows in the window
    /// showing the tab as it opens.
    pub fn set_cast(&mut self, tab: u64, cast: Option<Cast>, cx: &mut Context<Self>) {
        match cast {
            Some(cast) if tab == self.selected || self.cast_tab() == Some(tab) => {
                self.cast = Some((tab, cast));
                // It needs the sidebar, so a hidden one comes out with it.
                if self.can_reveal() {
                    self.set_revealed(true, cx);
                }
            }
            None if self.cast_tab() == Some(tab) => self.cast = None,
            _ => return,
        }
        cx.notify();
    }

    fn cast_tab(&self) -> Option<u64> {
        self.cast.as_ref().map(|(tab, _)| *tab)
    }

    /// View › Cast…: opens the shown tab's Cast dialog, or closes it.
    pub(super) fn cast(&mut self, cx: &mut Context<Self>) {
        if self.cast.take().is_some() {
            self.browser.cast_close(cx);
            cx.notify();
        } else if self.selected != 0 {
            self.browser.cast(self.selected, cx);
        }
    }

    /// Another tab shown closes the card, as the dialog is the tab's.
    pub(super) fn cast_for_shown(&mut self, cx: &mut Context<Self>) {
        if self.cast.as_ref().is_some_and(|(tab, _)| *tab != self.selected) {
            self.cast = None;
            self.browser.cast_close(cx);
        }
    }

    /// The Cast dialog of the tab shown, under the URL field, if no question is there.
    pub(super) fn cast_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let Some((tab, cast)) = self.cast.as_ref().filter(|(tab, _)| *tab == self.selected) else {
            self.cast_drawn.set(None);
            return None;
        };
        if self.question().is_some() {
            return None;
        }
        let tab = *tab;
        let close = card_close(&self.palette, "cast-close").on_click(cx.listener(|shell, _, _, cx| {
            log!("browser: command cast-close via card");
            shell.cast = None;
            shell.browser.cast_close(cx);
            cx.notify();
        }));
        // Chrome's words while it looks, as no device answered yet.
        let looking = cast.devices.is_empty().then(|| {
            div().mt(px(ROW_GAP)).text_color(rgba(DETAIL)).child("Looking for devices…")
        });
        let devices = cast.devices.iter().enumerate().map(|(index, device)| {
            self.cast_row(tab, index, device, cx)
        });
        Some(
            self.plate("cast")
                .child(card_listener("cast", &self.cast_drawn, tab))
                .child(
                    div()
                        .pr(px(BESIDE_CLOSE))
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(cast.header.clone()),
                )
                .children(looking)
                .children(devices)
                .child(close),
        )
    }

    fn cast_row(
        &self,
        tab: u64,
        index: usize,
        device: &CastDevice,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let status = (!device.status.is_empty()).then(|| {
            div()
                .truncate()
                .text_size(px(STATUS))
                .text_color(self.palette.text(SECONDARY))
                .child(device.status.clone())
        });
        let busy = matches!(device.state, CastState::Connecting | CastState::Disconnecting);
        let row = div()
            .id(("cast-device", index))
            .mt(px(ROW_GAP))
            .h(px(ROW))
            .flex()
            .items_center()
            .gap(px((ROW - GLYPH) / 2.))
            .px(px((ROW - GLYPH) / 2.))
            .rounded(px(NAV_RADIUS))
            .child(
                svg()
                    .path("icons/cast.svg")
                    .flex_none()
                    .size(px(GLYPH))
                    .text_color(self.palette.glyph(NAV)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(NAME))
                            .text_color(self.palette.text(TITLE))
                            .child(device.name.clone()),
                    )
                    .children(status),
            );
        // One Chrome can't cast to now, or that's on its way, takes no click, as on Chrome's.
        if device.state == CastState::Unavailable || busy {
            return row.opacity(0.5).into_any_element();
        }
        let id = device.id.clone();
        row.hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
            .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
            .on_click(cx.listener(move |shell, _, _, cx| {
                log!("browser: command cast-to {id} via card");
                shell.browser.cast_to(tab, id.clone(), cx);
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{AppContext, Entity, TestAppContext};

    use super::{Cast, CastDevice, CastState};
    use crate::shell::{Command, Folders, Shell, recorder::Recorder};

    /// One Space with tabs 1 and 2, tab 1 shown.
    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), vec![1, 2], Vec::new(), Folders::default(), cx);
            shell.set_selected(1, cx);
        });
        host.take();
        (shell, host)
    }

    fn tv() -> Cast {
        let device = CastDevice {
            id: "cast:1".into(),
            name: "Living Room".into(),
            state: CastState::Available,
            status: "".into(),
        };
        Cast { header: "Cast tab".into(), devices: vec![device] }
    }

    /// View › Cast… asks the core for the tab shown's dialog, and again closes it; the core's
    /// answer shows only over the tab it's for, and another tab shown closes it.
    #[gpui::test]
    fn the_cast_card_is_the_shown_tab_s_until_it_closes(cx: &mut TestAppContext) {
        let (shell, host) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.run(Command::Cast, "ui", cx);
            assert_eq!(host.take(), ["cast 1"]);
            shell.set_cast(2, Some(tv()), cx);
            assert!(shell.cast.is_none(), "another tab's dialog took the card");
            shell.set_cast(1, Some(tv()), cx);
            assert_eq!(shell.cast.as_ref().map(|(tab, _)| *tab), Some(1));
            shell.run(Command::Cast, "ui", cx);
            assert_eq!(host.take(), ["cast close"]);
            assert!(shell.cast.is_none());
            shell.set_cast(1, Some(tv()), cx);
            shell.set_selected(2, cx);
            assert!(host.take().contains(&"cast close".to_owned()));
            assert!(shell.cast.is_none());
        });
    }
}
