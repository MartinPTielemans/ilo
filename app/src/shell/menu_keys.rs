//! Off the Mac, the footer's menu control with the keyboard (#4030), as Chrome's F10 rings its menu
//! button. Focus ilo Menu rings the control, the sidebar holding the keyboard. Return, Space or Down
//! opens ilo's menu with its first item highlighted, as a menu opened from the keyboard is, and
//! Escape or Focus ilo Menu again gives the keyboard back: to the page if it had it, else to the window.
//! Whatever ends the sidebar's ring ends this one. On the Mac the menu bar holds ilo's menu and the
//! system has its own key for it. It moves only the user's view, so agents get no path, as with
//! Focus Sidebar.

use gpui::{Context, canvas, div, prelude::*, px};

use super::{NAV_RADIUS, Shell};
use crate::tokens::shell::{KEYED, KEYED_RING};

impl Shell {
    /// Whether the window draws the menu control for Focus ilo Menu to ring: off the Mac, with the
    /// sidebar there or able to show, as Focus Sidebar shows it.
    pub(super) fn menu_reaches(&self) -> bool {
        self.menu_control
            && self.library.is_none()
            && !self.page_fullscreen
            && self.little().is_none()
            && self.onboarding.is_none()
    }

    /// Focus ilo Menu: rings the menu control, showing the sidebar if it's hidden, or, with the ring on
    /// it already, gives the keyboard back. `from_page` is whether the page had the keyboard.
    pub(super) fn focus_menu(&mut self, from_page: bool, cx: &mut Context<Self>) {
        if self.keyed_menu.is_some() {
            return self.give_back_menu_keys("focus-menu", cx);
        }
        if !self.menu_reaches() {
            return;
        }
        if !self.dock(cx) && self.sidebar_hidden {
            self.toggle_sidebar(cx);
        }
        self.leave_keys("menu", cx);
        self.leave_strip("menu", cx);
        self.take_keyboard(cx);
        self.focus_keys = true;
        self.keyed_menu = Some(from_page);
        // check.sh waits for this line.
        log!("browser: menu keyed");
        cx.notify();
    }

    /// Return, Space or Down with the ring on the menu control: its menu, as a click opens it,
    /// with its first item highlighted. Says whether the ring was there.
    pub(super) fn key_open_menu(&mut self, cx: &mut Context<Self>) -> bool {
        if self.leave_menu_keys("open", cx).is_none() {
            return false;
        }
        self.app_menu(cx);
        // After the host's own deferred show, where overlays open for a shortcut.
        cx.defer(crate::menu::highlight_first);
        true
    }

    /// Escape or Focus ilo Menu with the ring on the menu control: the page gets the keyboard back if
    /// it had it, else the window keeps it.
    pub(super) fn give_back_menu_keys(&mut self, why: &str, cx: &mut Context<Self>) {
        if self.leave_menu_keys(why, cx) == Some(true) {
            self.browser.focus_page(cx);
        }
    }

    /// Takes the ring off the menu control, for `why`, saying whether the page had the keyboard
    /// before it, if it was there.
    pub(super) fn leave_menu_keys(&mut self, why: &str, cx: &mut Context<Self>) -> Option<bool> {
        let from_page = self.keyed_menu.take();
        self.menu_ring_drawn.set(false);
        if from_page.is_some() {
            // check.sh waits for this line.
            log!("browser: menu keys left by {why}");
            cx.notify();
        }
        from_page
    }

    /// The ring over the menu control while the keys are on it, as the strip's stops have theirs.
    pub(super) fn menu_ring(&self) -> Option<impl IntoElement + use<>> {
        let drawn = self.menu_ring_drawn.clone();
        // The checks capture the window once this says the ring is on screen.
        let frame = canvas(
            move |bounds, _, _| {
                if !drawn.replace(true) {
                    let (origin, size) = (bounds.origin, bounds.size);
                    log!(
                        "browser: menu ring at {} {} {} {}",
                        f32::from(origin.x),
                        f32::from(origin.y),
                        f32::from(size.width),
                        f32::from(size.height),
                    );
                }
            },
            |_, _, _, _| {},
        );
        self.keyed_menu.is_some().then(|| {
            div()
                .absolute()
                .inset_0()
                .rounded(px(NAV_RADIUS))
                .border(px(KEYED_RING))
                .border_color(self.palette.plate(KEYED))
                .child(frame.absolute().size_full())
        })
    }
}
