//! The update banner at the sidebar's foot (#678, Arc's): once the system's updater has an update
//! downloaded and waiting for a restart, or on the Mac one Sparkle won't download until the user
//! says, a pill over the footer says so, quietly, in "+ New Tab"'s ink, its glyph in the rows'
//! icon column and its words on their title column. Under the pointer it opens upward into the
//! version and the button that moves it on: Restart to Update, which says Restarting… until the
//! quit goes ahead, or Show Update, which opens Sparkle's window. It's the window's, not a tab's,
//! so it sits with the players, under them, and opens over them. It never pops up, has no toast and
//! no badge, and the restart that installs the update takes it away. While the updater's checks are
//! off, which makes every update a manual one, it offers under its button to turn them on, as
//! Arc's does after a manual update.

use web_time::Instant;

use gpui::{Context, MouseButton, SharedString, div, prelude::*, px, svg};

use super::{HOVERED, PRESSED, Shell, access::Accessible, permission::card_listener, plate};
use crate::dialog::{MOVE, ease_in_out};
use crate::tokens::settings::{CONTROL_HEIGHT, CONTROL_RADIUS};
use crate::tokens::shell::{
    FAVICON_DRAWN, FAVICON_SIZE, FAVICON_X, NEW_TAB, NEW_TAB_PLUS, PLATE_INSET, PLATE_RADIUS,
    SECONDARY, TITLE_GAP, UNSELECTED_TITLE, UPDATE_BANNER, UPDATE_BANNER_GAP, UPDATE_BANNER_GLYPH,
    UPDATE_BANNER_OPEN,
};
use crate::tokens::text;
use crate::updates::{self, Action, Stage};

/// What waits for the user: an update Sparkle won't download by itself, or one ready to restart
/// into, with its version if the updater names it.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Waiting {
    Available(Option<SharedString>),
    Ready(Option<SharedString>),
}

impl Waiting {
    pub(super) fn of(stage: Stage) -> Option<Waiting> {
        match stage {
            Stage::Available(version) => Some(Waiting::Available(version)),
            Stage::Ready(version) => Some(Waiting::Ready(version)),
            _ => None,
        }
    }

    fn version(&self) -> &Option<SharedString> {
        match self {
            Waiting::Available(version) | Waiting::Ready(version) => version,
        }
    }
}

/// The open banner's words, under its title: its first line's gap over the detail, the detail's
/// line, and the gaps around the button.
const OPEN_TOP: f32 = 8.;
const HEAD: f32 = 20.;
const DETAIL_TOP: f32 = 2.;
const DETAIL_LINE: f32 = 14.;
const BUTTON_TOP: f32 = 8.;
const BUTTON_INSET: f32 = 10.;
/// Update Automatically's line under the button, while the updater's checks are off.
const OFFER_TOP: f32 = 6.;
const OFFER: f32 = 16.;

/// The offer's words, as check.sh and an assistive app name it.
const UPDATE_AUTOMATICALLY: &str = "Update Automatically";

impl Shell {
    /// The banner's state, as check.sh reads it: collapsed, open or restarting, or none.
    pub(super) fn update_banner_state(&self) -> Option<&'static str> {
        self.update_banner.as_ref()?;
        Some(match (self.update_restarting, self.update_open) {
            (true, _) => "restarting",
            (false, true) => "open",
            (false, false) => "collapsed",
        })
    }

    /// How much of the foot the banner takes: its pill and the gaps around it while one shows,
    /// else nothing. Opening, it goes over the list rather than moving it.
    pub(super) fn update_banner_room(&self) -> f32 {
        match self.update_banner {
            Some(_) => UPDATE_BANNER + 2. * UPDATE_BANNER_GAP,
            None => 0.,
        }
    }

    /// How tall the banner is drawn: at rest its pill or its open height, and between them while
    /// it opens or closes.
    pub(super) fn update_banner_height(&self) -> f32 {
        let rest = self.update_banner_rest();
        match self.update_motion {
            Some((from, start)) => {
                let t = (start.elapsed().as_secs_f32() / MOVE).min(1.);
                from + (rest - from) * ease_in_out(t)
            }
            None => rest,
        }
    }

    fn update_banner_rest(&self) -> f32 {
        match self.update_open || self.update_restarting {
            true if self.offers_automatic_updates() => UPDATE_BANNER_OPEN + OFFER_TOP + OFFER,
            true => UPDATE_BANNER_OPEN,
            false => UPDATE_BANNER,
        }
    }

    /// Whether the open banner offers Update Automatically: while the checks are off, and not
    /// once a restart is under way.
    fn offers_automatic_updates(&self) -> bool {
        self.update_offer && !self.update_restarting
    }

    /// Draws the banner's opening or closing on.
    pub(super) fn step_update_banner(&mut self, window: &mut gpui::Window) {
        let Some((_, start)) = self.update_motion else {
            return;
        };
        if start.elapsed().as_secs_f32() < MOVE {
            window.request_animation_frame();
        } else {
            self.update_motion = None;
        }
    }

    /// Opens or closes from where it's drawn now, in the dialog's timing, or at once under Reduce
    /// Motion.
    fn move_update_banner(&mut self, change: impl FnOnce(&mut Self), cx: &mut Context<Self>) {
        let (from, rest) = (self.update_banner_height(), self.update_banner_rest());
        change(self);
        if self.update_banner.is_some() && self.update_banner_rest() != rest {
            let start: Instant = crate::motion::start(cx);
            self.update_motion = Some((from, start));
        }
        self.say_update_banner();
        cx.notify();
    }

    /// check.sh waits for these lines.
    fn say_update_banner(&mut self) {
        let state = self.update_banner_state();
        if state != self.update_said {
            self.update_said = state;
            log!("browser: update banner {}", state.unwrap_or("gone"));
        }
    }

    /// The updater said something new: what waits now, and whether a restart is under way. A
    /// restart a page stopped leaves the banner open on it, to try again.
    pub(super) fn update_changed(&mut self, cx: &mut Context<Self>) {
        let waiting = Waiting::of(updates::stage(cx));
        let ready = matches!(waiting, Some(Waiting::Ready(_)));
        let restarting = ready && updates::restarting(cx);
        let stopped = self.update_restarting && !restarting && ready;
        let offer = updates::automatic(cx) == Some(false);
        if self.update_banner == waiting
            && self.update_restarting == restarting
            && self.update_offer == offer
        {
            return;
        }
        let appears = self.update_banner.is_none();
        self.move_update_banner(
            |shell| {
                shell.update_banner = waiting;
                shell.update_restarting = restarting;
                shell.update_offer = offer;
                shell.update_open = (shell.update_open || stopped) && shell.update_banner.is_some();
            },
            cx,
        );
        // It comes collapsed where it rests, with nothing to move from.
        if appears {
            self.update_motion = None;
        }
    }

    /// The pointer came onto the banner, or left it.
    pub(super) fn update_banner_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.update_banner.is_none() || self.update_open == hovered {
            return;
        }
        self.move_update_banner(|shell| shell.update_open = hovered, cx);
    }

    /// A click on the banner: the restart into a ready update, or Sparkle's window on one it won't
    /// download, as About's button does. Nothing while a restart is under way.
    pub(super) fn click_update_banner(&mut self, cx: &mut Context<Self>) {
        match self.update_banner {
            Some(Waiting::Ready(_)) if !self.update_restarting => {
                log!("browser: command restart-to-update via sidebar");
                updates::act(Action::Restart, cx);
            }
            Some(Waiting::Available(_)) => {
                log!("browser: command show-update via sidebar");
                updates::act(Action::Show, cx);
            }
            _ => {}
        }
    }

    /// A click on Update Automatically: the updater's checks come on, as About's switch turns
    /// them, and the offer goes.
    pub(super) fn click_update_offer(&mut self, cx: &mut Context<Self>) {
        if !self.offers_automatic_updates() {
            return;
        }
        log!("browser: command update-automatically via sidebar");
        updates::set_automatic(true, cx);
        self.update_changed(cx);
    }

    /// The banner open, as under the pointer, for the preview.
    pub fn preview_update_banner_open(&mut self, cx: &mut Context<Self>) {
        self.update_banner_hovered(true, cx);
        self.update_motion = None;
    }

    /// The banner, while an update waits.
    pub(super) fn update_banner(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let Some(waiting) = self.update_banner.clone() else {
            self.update_drawn.set(None);
            return None;
        };
        let height = self.update_banner_height();
        let open = height > UPDATE_BANNER;
        let app = env!("APP_NAME");
        let (rest_words, title, detail, button) = match (&waiting, self.update_restarting) {
            (Waiting::Ready(version), restarting) => (
                "Update ready",
                match version {
                    Some(version) => format!("{app} {version} is ready"),
                    None => "The update is ready".into(),
                },
                if restarting {
                    concat!(env!("APP_NAME"), " reopens with your tabs.")
                } else {
                    "Your tabs come back."
                },
                if restarting {
                    "Restarting…"
                } else {
                    updates::RESTART
                },
            ),
            (Waiting::Available(version), _) => (
                "Update available",
                match version {
                    Some(version) => format!("{app} {version} is available"),
                    None => "An update is available".into(),
                },
                "See what’s new before it installs.",
                updates::SHOW,
            ),
        };
        let glyph = |size: f32, alpha: f32| {
            svg()
                .path("icons/circle-fading-arrow-up.svg")
                .size(px(size))
                .text_color(self.palette.glyph(alpha))
        };
        // At rest: the glyph, the words on the title column and the version at the end, in small
        // type and + New Tab's inks.
        let collapsed = plate(
            glyph(UPDATE_BANNER_GLYPH, NEW_TAB_PLUS),
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .pr(px(FAVICON_X - PLATE_INSET))
                .text_size(px(text::SMALL))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(self.palette.text(NEW_TAB))
                        .child(rest_words),
                )
                .children(waiting.version().clone().map(|version| {
                    div()
                        .flex_none()
                        .text_color(self.palette.text(NEW_TAB_PLUS))
                        .child(version)
                })),
        )
        .h(px(UPDATE_BANNER));
        // Open: the version as a title on the same columns, a line under it, and the button.
        let title_x = FAVICON_X - PLATE_INSET + FAVICON_SIZE + TITLE_GAP;
        let accent = self.palette.accent;
        let opened = div()
            .absolute()
            .top(px(OPEN_TOP))
            .left_0()
            .right_0()
            .child(
                plate(
                    glyph(FAVICON_DRAWN, UNSELECTED_TITLE),
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(text::BODY))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(self.palette.text(1.))
                        .child(title),
                )
                .h(px(HEAD)),
            )
            .child(
                div()
                    .mt(px(DETAIL_TOP))
                    .pl(px(title_x))
                    .pr(px(BUTTON_INSET))
                    .h(px(DETAIL_LINE))
                    .truncate()
                    .text_size(px(text::SMALL))
                    .text_color(self.palette.text(SECONDARY))
                    .child(detail),
            )
            .child(
                div()
                    .id("update-banner-button")
                    .mt(px(BUTTON_TOP))
                    .mx(px(BUTTON_INSET))
                    .h(px(CONTROL_HEIGHT))
                    .rounded(px(CONTROL_RADIUS))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(text::BODY))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .map(|button_box| match self.update_restarting {
                        true => button_box
                            .bg(self.palette.plate(HOVERED))
                            .text_color(self.palette.text(SECONDARY)),
                        false => button_box
                            .bg(accent)
                            .text_color(self.palette.accent_label)
                            .hover(|style| style.bg(self.palette.accent_hovered())),
                    })
                    .child(button),
            )
            .when(self.offers_automatic_updates(), |opened| {
                opened.child(
                    div()
                        .id("update-banner-offer")
                        .when(self.a11y, |offer| offer.button_named(UPDATE_AUTOMATICALLY))
                        .mt(px(OFFER_TOP))
                        .mx(px(BUTTON_INSET))
                        .h(px(OFFER))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(text::SMALL))
                        .text_color(self.palette.text(SECONDARY))
                        .hover(|style| style.text_color(self.palette.text(1.)))
                        // check.sh clicks it where it's drawn.
                        .child(card_listener("update offer", &self.update_offer_drawn, 0))
                        .child(UPDATE_AUTOMATICALLY)
                        .on_click(cx.listener(|shell, _, _, cx| {
                            cx.stop_propagation();
                            shell.click_update_offer(cx);
                        })),
                )
            });
        let named = if open { button } else { rest_words };
        Some(
            div()
                .id("update-banner")
                .when(self.a11y, |banner| banner.button_named(named))
                .absolute()
                .left(px(PLATE_INSET))
                .w(px(self.width() - 2. * PLATE_INSET))
                .bottom(px(self.over_safe_start() + UPDATE_BANNER_GAP))
                .h(px(height))
                .rounded(px(PLATE_RADIUS))
                .overflow_hidden()
                // Opaque, so the rows it opens over don't show through, under the plate's ink.
                .bg(self.window_fill_at(1.))
                // Rounded too, as a clip keeps a child's square corners.
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded(px(PLATE_RADIUS))
                        .bg(self.palette.plate(if open { HOVERED } else { PRESSED })),
                )
                // check.sh moves onto it and clicks it where it's drawn.
                .child(card_listener("update banner", &self.update_drawn, 0))
                .child(match open {
                    true => opened.into_any_element(),
                    false => collapsed.into_any_element(),
                })
                .block_mouse_except_scroll()
                .on_hover(cx.listener(|shell, hovered: &bool, _, cx| {
                    shell.update_banner_hovered(*hovered, cx)
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|shell, _, _, cx| shell.click_update_banner(cx))),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{App, AppContext, Entity, TestAppContext};

    use super::{OFFER, OFFER_TOP};
    use crate::shell::{Folders, Shell, recorder::Recorder};
    use crate::tokens::shell::{UPDATE_BANNER, UPDATE_BANNER_OPEN};
    use crate::updates::{self, Host, Stage, Updater};

    /// The updater, recording what the banner asks of it.
    #[derive(Default)]
    struct Asked {
        asked: RefCell<Vec<&'static str>>,
    }

    impl Host for Asked {
        fn check(&self, _: &mut App) {
            self.asked.borrow_mut().push("check");
        }

        fn show(&self, _: &mut App) {
            self.asked.borrow_mut().push("show");
        }

        fn restart(&self, _: &mut App) {
            self.asked.borrow_mut().push("restart");
        }

        fn follow(&self, _: updates::Track, _: &mut App) {}

        fn check_automatically(&self, on: bool, _: &mut App) {
            self.asked
                .borrow_mut()
                .push(if on { "automatic on" } else { "automatic off" });
        }
    }

    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Asked>) {
        let updater = Rc::new(Asked::default());
        let asked = updater.clone();
        cx.update(|cx| updates::start(Updater::Own, asked, cx));
        let shell = cx.new(|cx| Shell::new(Rc::new(Recorder::default()), cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![1, 2],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(1, cx);
        });
        (shell, updater)
    }

    fn says(stage: Stage, cx: &mut TestAppContext) {
        cx.update(|cx| updates::changed(stage, cx));
    }

    fn state(shell: &Entity<Shell>, cx: &mut TestAppContext) -> Option<&'static str> {
        shell.update(cx, |shell, _| shell.update_banner_state())
    }

    /// Nothing shows while the updater checks, finds an update it downloads by itself, downloads
    /// it or fails. A ready update, or one Sparkle waits on the user for, shows collapsed, its
    /// pill's room taken from the list.
    #[gpui::test]
    fn a_waiting_update_shows_collapsed_and_takes_its_room_from_the_list(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        let bare = shell.update(cx, |shell, _| shell.list_bottom());
        for stage in [
            Stage::Checking,
            Stage::Current,
            Stage::Found(Some("0.1.5".into())),
            Stage::Downloading(Some("0.1.5".into())),
            Stage::Failed { found: true },
        ] {
            says(stage.clone(), cx);
            assert_eq!(state(&shell, cx), None, "a banner at {stage:?}");
            assert_eq!(shell.update(cx, |shell, _| shell.update_banner_room()), 0.);
        }
        for stage in [
            Stage::Available(Some("0.1.5".into())),
            Stage::Ready(Some("0.1.5".into())),
        ] {
            says(stage.clone(), cx);
            assert_eq!(state(&shell, cx), Some("collapsed"), "at {stage:?}");
            let (room, bottom, height) = shell.update(cx, |shell, _| {
                (
                    shell.update_banner_room(),
                    shell.list_bottom(),
                    shell.update_banner_height(),
                )
            });
            assert!(
                room > UPDATE_BANNER && bottom == bare + room,
                "no room at {stage:?}"
            );
            assert_eq!(height, UPDATE_BANNER, "not collapsed at {stage:?}");
        }
    }

    /// Under the pointer it opens to its full height, over the list, which keeps its room, and
    /// closes as the pointer leaves.
    #[gpui::test]
    fn the_banner_opens_under_the_pointer_and_closes_as_it_leaves(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        says(Stage::Ready(Some("0.1.5".into())), cx);
        let room = shell.update(cx, |shell, _| shell.update_banner_room());
        shell.update(cx, |shell, cx| shell.update_banner_hovered(true, cx));
        assert_eq!(state(&shell, cx), Some("open"));
        assert_eq!(
            shell.update(cx, |shell, _| shell.update_banner_room()),
            room
        );
        shell.update(cx, |shell, _| shell.update_motion = None);
        assert_eq!(
            shell.update(cx, |shell, _| shell.update_banner_height()),
            UPDATE_BANNER_OPEN
        );
        shell.update(cx, |shell, cx| shell.update_banner_hovered(false, cx));
        assert_eq!(state(&shell, cx), Some("collapsed"));
    }

    /// A click on a ready update restarts into it, and the banner says Restarting… open, as the
    /// pointer leaves too, until the quit goes ahead.
    #[gpui::test]
    fn a_click_restarts_into_a_ready_update_and_says_so(cx: &mut TestAppContext) {
        let (shell, updater) = shell(cx);
        says(Stage::Ready(Some("0.1.5".into())), cx);
        shell.update(cx, |shell, cx| shell.update_banner_hovered(true, cx));
        shell.update(cx, |shell, cx| shell.click_update_banner(cx));
        assert_eq!(*updater.asked.borrow(), ["restart"]);
        assert_eq!(state(&shell, cx), Some("restarting"));
        shell.update(cx, |shell, cx| shell.update_banner_hovered(false, cx));
        assert_eq!(state(&shell, cx), Some("restarting"));
        // A second click while it restarts asks nothing more.
        shell.update(cx, |shell, cx| shell.click_update_banner(cx));
        assert_eq!(*updater.asked.borrow(), ["restart"]);
    }

    /// A page that asks to stay stops the restart: the banner is open again, offering it.
    #[gpui::test]
    fn a_restart_a_page_stops_leaves_the_banner_open(cx: &mut TestAppContext) {
        let (shell, updater) = shell(cx);
        says(Stage::Ready(Some("0.1.5".into())), cx);
        shell.update(cx, |shell, cx| shell.click_update_banner(cx));
        assert_eq!(state(&shell, cx), Some("restarting"));
        cx.update(updates::restart_stopped);
        assert_eq!(state(&shell, cx), Some("open"));
        shell.update(cx, |shell, cx| shell.click_update_banner(cx));
        assert_eq!(*updater.asked.borrow(), ["restart", "restart"]);
    }

    /// One Sparkle won't download by itself opens Sparkle's window on a click, and goes once the
    /// user is done with that window.
    #[gpui::test]
    fn a_click_on_an_update_sparkle_waits_on_opens_its_window(cx: &mut TestAppContext) {
        let (shell, updater) = shell(cx);
        says(Stage::Available(Some("0.1.5".into())), cx);
        shell.update(cx, |shell, cx| shell.click_update_banner(cx));
        assert_eq!(*updater.asked.borrow(), ["show"]);
        assert_eq!(state(&shell, cx), Some("collapsed"));
        says(Stage::Idle, cx);
        assert_eq!(state(&shell, cx), None);
        assert_eq!(shell.update(cx, |shell, _| shell.update_banner_room()), 0.);
    }

    /// The open banner's height once its motion is done.
    fn open_height(shell: &Entity<Shell>, cx: &mut TestAppContext) -> f32 {
        shell.update(cx, |shell, _| {
            shell.update_motion = None;
            shell.update_banner_height()
        })
    }

    /// With the updater's checks off, every update is a manual one: the open banner offers Update
    /// Automatically under its button, which turns them on and goes, the banner staying open in
    /// its plain form. Settings' switch turns them off again.
    #[gpui::test]
    fn with_checks_off_the_open_banner_offers_to_turn_them_on(cx: &mut TestAppContext) {
        let (shell, updater) = shell(cx);
        cx.update(|cx| updates::checks(Some(false), cx));
        for stage in [
            Stage::Available(Some("0.1.5".into())),
            Stage::Ready(Some("0.1.5".into())),
        ] {
            says(stage, cx);
            shell.update(cx, |shell, cx| shell.update_banner_hovered(true, cx));
            assert_eq!(
                open_height(&shell, cx),
                UPDATE_BANNER_OPEN + OFFER_TOP + OFFER
            );
            shell.update(cx, |shell, cx| shell.update_banner_hovered(false, cx));
            assert_eq!(open_height(&shell, cx), UPDATE_BANNER, "the offer opens it");
        }
        shell.update(cx, |shell, cx| shell.update_banner_hovered(true, cx));
        shell.update(cx, |shell, cx| shell.click_update_offer(cx));
        assert_eq!(*updater.asked.borrow(), ["automatic on"]);
        assert_eq!(cx.update(|cx| updates::automatic(cx)), Some(true));
        assert_eq!(state(&shell, cx), Some("open"));
        assert_eq!(open_height(&shell, cx), UPDATE_BANNER_OPEN);
        // Gone, a second click asks nothing.
        shell.update(cx, |shell, cx| shell.click_update_offer(cx));
        assert_eq!(*updater.asked.borrow(), ["automatic on"]);
        // Settings' switch, which reads and sets these, turns it off again, and the offer comes
        // back.
        cx.update(|cx| updates::set_automatic(false, cx));
        assert_eq!(*updater.asked.borrow(), ["automatic on", "automatic off"]);
        assert_eq!(cx.update(|cx| updates::automatic(cx)), Some(false));
        assert_eq!(
            open_height(&shell, cx),
            UPDATE_BANNER_OPEN + OFFER_TOP + OFFER
        );
    }

    /// Turned off in Settings while an update waits, the banner offers it at once, and turned on
    /// there, the offer goes.
    #[gpui::test]
    fn the_offer_follows_settings_switch(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        cx.update(|cx| updates::checks(Some(true), cx));
        says(Stage::Ready(Some("0.1.5".into())), cx);
        shell.update(cx, |shell, cx| shell.update_banner_hovered(true, cx));
        assert_eq!(
            open_height(&shell, cx),
            UPDATE_BANNER_OPEN,
            "an offer with checks on"
        );
        cx.update(|cx| updates::set_automatic(false, cx));
        assert_eq!(
            open_height(&shell, cx),
            UPDATE_BANNER_OPEN + OFFER_TOP + OFFER
        );
        cx.update(|cx| updates::set_automatic(true, cx));
        assert_eq!(open_height(&shell, cx), UPDATE_BANNER_OPEN);
    }

    /// Where the user can't turn the checks off, as in a Flatpak, and once a restart is under way,
    /// there's no offer.
    #[gpui::test]
    fn no_offer_without_a_switch_or_while_restarting(cx: &mut TestAppContext) {
        let (shell, updater) = shell(cx);
        says(Stage::Ready(Some("0.1.5".into())), cx);
        shell.update(cx, |shell, cx| shell.update_banner_hovered(true, cx));
        assert_eq!(
            open_height(&shell, cx),
            UPDATE_BANNER_OPEN,
            "an offer with no switch"
        );
        cx.update(|cx| updates::checks(Some(false), cx));
        cx.update(|cx| updates::changed(Stage::Ready(Some("0.1.5".into())), cx));
        shell.update(cx, |shell, cx| shell.update_changed(cx));
        assert_eq!(
            open_height(&shell, cx),
            UPDATE_BANNER_OPEN + OFFER_TOP + OFFER
        );
        shell.update(cx, |shell, cx| shell.click_update_banner(cx));
        assert_eq!(state(&shell, cx), Some("restarting"));
        assert_eq!(
            open_height(&shell, cx),
            UPDATE_BANNER_OPEN,
            "an offer while restarting"
        );
        shell.update(cx, |shell, cx| shell.click_update_offer(cx));
        assert_eq!(*updater.asked.borrow(), ["restart"]);
    }
}
