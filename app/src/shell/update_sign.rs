//! Restart to Update at the sidebar's foot (#1416, #1418): once the system's updater has an update
//! downloaded and waiting for a restart, one row over the footer says so, in "+ New Tab"'s quiet
//! ink, and a click restarts into it as About's button does. It's the window's, not a tab's, so it
//! sits with the players, under them, where their coming and going never moves it. It never pops
//! up, never draws over the page and has no badge: it's there while an update waits, and the
//! restart that installs the update takes it away.

use gpui::{Context, MouseButton, div, prelude::*, px, svg};

use super::{
    FAVICON_DRAWN, FOOTER_HEIGHT, HOVERED, Shell, access::Accessible, permission::card_listener,
    plate, row_box,
};
use crate::tokens::shell::{NEW_TAB, NEW_TAB_PLUS, ROW_PITCH};
use crate::tokens::text;
use crate::updates::{self, Action};

impl Shell {
    /// How much of the foot the sign takes: a row while an update waits, else nothing.
    pub(super) fn update_sign_room(&self) -> f32 {
        if self.update_waiting { ROW_PITCH } else { 0. }
    }

    /// The updater said something new: whether an update waits now.
    pub(super) fn update_changed(&mut self, cx: &mut Context<Self>) {
        let waiting = updates::ready(cx);
        if self.update_waiting != waiting {
            self.update_waiting = waiting;
            cx.notify();
        }
    }

    /// The sign, while an update waits for a restart.
    pub(super) fn update_sign(&self) -> Option<impl IntoElement + use<>> {
        if !self.update_waiting {
            self.update_drawn.set(None);
            return None;
        }
        let glyph = svg()
            .path("icons/circle-fading-arrow-up.svg")
            .size(px(FAVICON_DRAWN))
            .text_color(self.palette.glyph(NEW_TAB_PLUS));
        let label = div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(text::BODY))
            .text_color(self.palette.text(NEW_TAB))
            .child(updates::RESTART);
        let hovered = self.palette.plate(HOVERED);
        let pressed = self.palette.selected(0.);
        Some(
            row_box()
                .id("update-sign")
                .when(self.a11y, |sign| sign.button_named(updates::RESTART))
                .absolute()
                .left_0()
                .w(px(self.width()))
                .bottom(px(FOOTER_HEIGHT))
                .group("update-sign")
                // check.sh clicks it where it's drawn.
                .child(card_listener("update sign", &self.update_drawn, 0))
                .child(
                    plate(glyph, label)
                        .id("update-sign-plate")
                        .group_hover("update-sign", |style| style.bg(hovered))
                        .group_active("update-sign", |style| style.bg(pressed)),
                )
                .block_mouse_except_scroll()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| {
                    log!("browser: command restart-to-update via sidebar");
                    updates::act(Action::Restart, cx);
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{App, AppContext, Entity, TestAppContext};

    use crate::shell::{Folders, Shell, recorder::Recorder};
    use crate::updates::{self, Host, Stage, Updater};

    /// The updater, recording what the sign asks of it.
    #[derive(Default)]
    struct Asked {
        asked: RefCell<Vec<&'static str>>,
    }

    impl Host for Asked {
        fn check(&self, _: &mut App) {
            self.asked.borrow_mut().push("check");
        }

        fn restart(&self, _: &mut App) {
            self.asked.borrow_mut().push("restart");
        }
    }

    fn shell(cx: &mut TestAppContext) -> Entity<Shell> {
        let shell = cx.new(|cx| Shell::new(Rc::new(Recorder::default()), cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), vec![1, 2], Vec::new(), Folders::default(), cx);
            shell.set_selected(1, cx);
        });
        shell
    }

    /// The sign shows only once an update waits for a restart, whatever came before, takes a row
    /// from the list while it does, and asks the updater to restart into it.
    #[gpui::test]
    fn the_sign_shows_only_while_an_update_waits(cx: &mut TestAppContext) {
        let updater = Rc::new(Asked::default());
        let asked = updater.clone();
        cx.update(|cx| updates::start(Updater::Own, asked, cx));
        let shell = shell(cx);
        let bare = shell.update(cx, |shell, _| shell.list_bottom());
        for stage in [
            Stage::Checking,
            Stage::Current,
            Stage::Found(Some("0.1.2".into())),
            Stage::Downloading(Some("0.1.2".into())),
            Stage::Failed { found: true },
        ] {
            cx.update(|cx| updates::changed(stage.clone(), cx));
            let room = shell.update(cx, |shell, _| shell.update_sign_room());
            assert_eq!(room, 0., "a sign at {stage:?}");
            assert!(shell.update(cx, |shell, _| shell.update_sign().is_none()));
        }
        cx.update(|cx| updates::changed(Stage::Ready(Some("0.1.2".into())), cx));
        let (room, bottom) =
            shell.update(cx, |shell, _| (shell.update_sign_room(), shell.list_bottom()));
        assert!(room > 0. && bottom == bare + room, "no room for the sign");
        assert!(shell.update(cx, |shell, _| shell.update_sign().is_some()));
        cx.update(|cx| updates::act(updates::Action::Restart, cx));
        assert_eq!(*updater.asked.borrow(), ["restart"]);
    }
}
