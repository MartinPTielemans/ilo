//! The players at the sidebar's foot (#700), as Arc's mini player and media tray: one for each tab
//! whose page has media the browser can control while the window doesn't show it, playing or
//! paused, with Picture in Picture while the page can float and nothing of it does, as Arc's Back
//! to PiP, play or pause, the next track when the page has one, and × to put it away until the page
//! plays again. A click elsewhere on it shows its tab. The title is cut with an ellipsis
//! where Arc's scrolls, as a marquee would draw every frame.

use gpui::{Context, Div, MouseButton, Stateful, div, prelude::*, px, svg};

use super::{
    CLOSE_HOVERED, FAVICON_X, FOOTER_HEIGHT, HOVERED, MediaAction, NAV_RADIUS, PLATE_INSET,
    SECONDARY, Shell, Space, TILE, TILE_RADIUS, TITLE, permission::card_listener,
};
use crate::tokens::shell::{
    MEDIA_ARTIST, MEDIA_BUTTON, MEDIA_GAP, MEDIA_GLYPH, MEDIA_HEIGHT, MEDIA_TITLE,
};

/// The most players stacked, so the list keeps its room (ours: Arc's tray isn't measured).
const MOST: usize = 3;

impl Shell {
    /// The tabs with a player, oldest first, as many as stack.
    pub(super) fn media_players(&self) -> Vec<u64> {
        let mut tabs: Vec<u64> = (self.tabs.iter())
            .filter(|(tab, each)| {
                each.page.media
                    && !self.media_dismissed.contains(tab)
                    && !self.on_screen(**tab)
                    && self.lists(**tab)
            })
            .map(|(tab, _)| *tab)
            .collect();
        tabs.sort_unstable();
        tabs.truncate(MOST);
        tabs
    }

    /// Whether `tab` is a favorite or in one of the window's Spaces: every window hears every
    /// tab, a private window's included, which no other window may show.
    pub(super) fn lists(&self, tab: u64) -> bool {
        let in_space = |space: &Space| space.pinned.contains(&tab) || space.today.contains(&tab);
        self.all_favorites.contains(&tab) || self.spaces.iter().any(in_space)
    }

    /// Where the pages above the footer end, over the players and Restart to Update.
    pub(super) fn list_bottom(&self) -> f32 {
        let players = self.media_players().len() as f32 * (MEDIA_HEIGHT + MEDIA_GAP);
        FOOTER_HEIGHT + self.update_sign_room() + players
    }

    /// × on `tab`'s player: it goes until the page plays again.
    pub(super) fn dismiss_media(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.media_dismissed.insert(tab) {
            cx.notify();
        }
    }

    /// The players, bottom up from over the footer.
    pub(super) fn media_player_cards(
        &self,
        cx: &mut Context<Self>,
    ) -> Vec<impl IntoElement + use<>> {
        let tabs = self.media_players();
        if *self.media_logged.borrow() != tabs {
            let list: Vec<String> = tabs.iter().map(u64::to_string).collect();
            // check.sh waits for this line.
            log!("browser: media players {}", list.join(" "));
            *self.media_logged.borrow_mut() = tabs.clone();
        }
        // So a player back where the last one was is logged as drawn again.
        if tabs.is_empty() {
            self.media_drawn.set(None);
        }
        (tabs.iter().enumerate())
            .map(|(index, &tab)| self.media_player(tab, index, cx))
            .collect()
    }

    fn media_player(
        &self,
        tab: u64,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = self.page(tab);
        let title = match page.media_title.is_empty() {
            true => self.title(tab),
            false => page.media_title.clone(),
        };
        let artist = (!page.media_artist.is_empty()).then(|| {
            div()
                .truncate()
                .text_size(px(MEDIA_ARTIST))
                .text_color(self.palette.text(SECONDARY))
                .child(page.media_artist.clone())
        });
        let (glyph, action) = match page.media_playing {
            true => ("icons/pause.svg", MediaAction::Pause),
            false => ("icons/play.svg", MediaAction::Play),
        };
        let float = page.media_picture_in_picture.then(|| {
            let glyph = "icons/picture-in-picture-2.svg";
            self.media_button("media-pip", tab, glyph).on_click(cx.listener(
                move |shell, _, _, cx| {
                    log!("browser: command media-picture-in-picture via player");
                    shell.browser.media(tab, MediaAction::PictureInPicture, cx);
                },
            ))
        });
        let play = self.media_button("media-play", tab, glyph).on_click(cx.listener(
            move |shell, _, _, cx| {
                log!("browser: command media-{} via player", label(action));
                shell.browser.media(tab, action, cx);
            },
        ));
        let next = page.media_next.then(|| {
            self.media_button("media-next", tab, "icons/skip-forward.svg").on_click(
                cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-next via player");
                    shell.browser.media(tab, MediaAction::Next, cx);
                }),
            )
        });
        let close = self.media_button("media-close", tab, "icons/x.svg").on_click(cx.listener(
            move |shell, _, _, cx| {
                log!("browser: command media-dismiss via player");
                shell.dismiss_media(tab, cx);
            },
        ));
        // Over Restart to Update, while an update waits.
        let bottom =
            FOOTER_HEIGHT + self.update_sign_room() + index as f32 * (MEDIA_HEIGHT + MEDIA_GAP);
        // check.sh finds the bottom one's buttons from where it's drawn.
        let listener = (index == 0).then(|| card_listener("media player", &self.media_drawn, tab));
        div()
            .id(("media-player", tab))
            .absolute()
            .left(px(PLATE_INSET))
            .w(px(self.width() - 2. * PLATE_INSET))
            .bottom(px(bottom))
            .h(px(MEDIA_HEIGHT))
            .rounded(px(TILE_RADIUS))
            .bg(self.palette.plate(TILE))
            .hover(|style| style.bg(self.palette.plate(HOVERED)))
            .flex()
            .items_center()
            .gap(px(MEDIA_GAP))
            .pl(px(FAVICON_X - PLATE_INSET))
            .pr(px((MEDIA_HEIGHT - MEDIA_BUTTON) / 2.))
            .children(listener)
            .child(self.tab_icon(tab))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(MEDIA_TITLE))
                            .text_color(self.palette.text(TITLE))
                            .child(title),
                    )
                    .children(artist),
            )
            .children(float)
            .child(play)
            .children(next)
            .child(close)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                log!("browser: command media-show via player");
                shell.select(tab, cx);
            }))
    }

    fn media_button(&self, id: &'static str, tab: u64, glyph: &'static str) -> Stateful<Div> {
        div()
            .id((id, tab))
            .flex_none()
            .size(px(MEDIA_BUTTON))
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
            .flex()
            .items_center()
            .justify_center()
            .child(svg().path(glyph).size(px(MEDIA_GLYPH)).text_color(self.palette.text(SECONDARY)))
            // Not the player's click, which would show the tab.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}

/// How the log names `action`.
fn label(action: MediaAction) -> &'static str {
    match action {
        MediaAction::Play => "play",
        MediaAction::Pause => "pause",
        MediaAction::Next => "next",
        MediaAction::PictureInPicture => "picture-in-picture",
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{AppContext, Entity, Modifiers, TestAppContext, point, px};

    use crate::shell::{Folders, Page, Shell, recorder::Recorder};
    use crate::tokens::shell::{MEDIA_BUTTON, MEDIA_GAP, MEDIA_HEIGHT};

    /// One Space with tabs 1, 2 and 3, tab 1 shown.
    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), vec![1, 2, 3], Vec::new(), Folders::default(), cx);
            shell.set_selected(1, cx);
        });
        (shell, host)
    }

    fn playing(shell: &mut Shell, tab: u64, media_playing: bool, cx: &mut gpui::Context<Shell>) {
        let page = Page { media: true, media_playing, ..shell.page(tab) };
        shell.set_page(tab, page, cx);
    }

    /// A tab playing out of sight has a player, the one shown none; a paused one keeps it, × puts
    /// it away until the page plays again, and the list ends over the players.
    #[gpui::test]
    fn a_tab_playing_out_of_sight_has_a_player_until_put_away(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        shell.update(cx, |shell, cx| {
            let bare = shell.list_bottom();
            playing(shell, 1, true, cx);
            playing(shell, 2, true, cx);
            assert_eq!(shell.media_players(), [2]);
            assert!(shell.list_bottom() > bare);
            playing(shell, 2, false, cx);
            assert_eq!(shell.media_players(), [2]);
            shell.dismiss_media(2, cx);
            assert!(shell.media_players().is_empty());
            assert_eq!(shell.list_bottom(), bare);
            playing(shell, 2, true, cx);
            assert_eq!(shell.media_players(), [2]);
            shell.set_selected(2, cx);
            assert_eq!(shell.media_players(), [1]);
        });
    }

    /// Every window hears every tab, so a player is for the tabs this window lists: another
    /// Space's or a favorite's, as Arc's shows in each window, but never a private window's in a
    /// window of the profiles, nor theirs in the private window.
    #[gpui::test]
    fn a_player_is_for_the_tabs_its_window_lists(cx: &mut TestAppContext) {
        let (shell, host) = shell(cx);
        let private = cx.new(|cx| Shell::private_window(host, 9, "Private".into(), 1, cx));
        let tabs = |shell: &mut Shell, cx: &mut gpui::Context<Shell>| {
            let other = (5, "Work".into(), None, 1, "".into(), "".into(), "".into());
            let home = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![home, other], cx);
            shell.set_tabs(1, Vec::new(), vec![1, 2, 3], Vec::new(), Folders::default(), cx);
            shell.set_tabs(5, vec![6], Vec::new(), Vec::new(), Folders::default(), cx);
            shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_favorites(vec![(7, 1)], cx);
            for tab in [6, 7, 10] {
                playing(shell, tab, true, cx);
            }
        };
        shell.update(cx, |shell, cx| {
            tabs(shell, cx);
            assert_eq!(shell.media_players(), [6, 7]);
        });
        private.update(cx, |shell, cx| {
            tabs(shell, cx);
            shell.set_selected(1, cx);
            assert_eq!(shell.media_players(), [10]);
        });
    }

    /// Music in another Space draws its player as it starts, though its row isn't drawn.
    #[gpui::test]
    fn another_spaces_music_draws_its_player_as_it_starts(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        shell.update(cx, |shell, cx| {
            let home = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
            let other = (5, "Work".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![home, other], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), vec![1], Vec::new(), Folders::default(), cx);
            shell.set_tabs(5, Vec::new(), vec![6], Vec::new(), Folders::default(), cx);
            shell.set_selected(1, cx);
        });
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.media_drawn.get()).is_none());
        shell.update(cx, |shell, cx| playing(shell, 6, true, cx));
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.media_drawn.get()).is_some());
    }

    /// A page that can float has Picture in Picture before play, as Arc's Back to PiP, which asks
    /// the core to float it; once it floats the button goes, and a click there shows the tab.
    #[gpui::test]
    fn picture_in_picture_floats_the_page_until_it_floats(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let floats = |shell: &mut Shell, can: bool, cx: &mut gpui::Context<Shell>| {
            let page = Page { media: true, media_picture_in_picture: can, ..shell.page(2) };
            shell.set_page(2, page, cx);
        };
        shell.update(cx, |shell, cx| {
            let space = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), vec![1, 2], Vec::new(), Folders::default(), cx);
            shell.set_selected(1, cx);
            floats(shell, true, cx);
        });
        cx.run_until_parked();
        // From the right, 8 in and 30 apart: ×, play, then Picture in Picture.
        let plate = shell.read_with(cx, |shell, _| shell.media_drawn.get()).expect("no player");
        let x = plate.right() - px((MEDIA_HEIGHT - MEDIA_BUTTON) / 2. + MEDIA_BUTTON / 2.)
            - px(2. * (MEDIA_BUTTON + MEDIA_GAP));
        let button = point(x, plate.center().y);
        host.take();
        cx.simulate_click(button, Modifiers::none());
        cx.run_until_parked();
        let calls = host.take();
        assert!(calls.contains(&"media 2 PictureInPicture".to_owned()), "{calls:?}");
        shell.update(cx, |shell, cx| floats(shell, false, cx));
        cx.run_until_parked();
        cx.simulate_click(button, Modifiers::none());
        cx.run_until_parked();
        let calls = host.take();
        assert!(!calls.iter().any(|call| call.starts_with("media ")), "{calls:?}");
        assert!(calls.contains(&"select 2".to_owned()), "{calls:?}");
    }
}
