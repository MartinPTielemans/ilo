//! The players at the sidebar's foot (#700), as Arc 1.171's stack: a card for each tab whose page
//! has media the browser can control while the window doesn't show it, playing or paused. At rest
//! only the newest shows, its favicon (back to its tab), play or pause and the next track, with up
//! to two older ones peeking out behind it. Under the pointer every card grows, the oldest on top,
//! and gains a title row with Picture in Picture while the page can float and nothing of it does,
//! as Arc's Back to PiP, and × to put it away until the page plays again. A click elsewhere on a
//! card shows its tab. The title is cut with an ellipsis where Arc's scrolls, as a marquee would
//! draw every frame. Previous and Arc's volume drawer wait for the core.
//!
//! Ours, as Arc's card has none (#2100): while the stack is open and still, the newest card whose
//! page has a video shows it live above its title row. The core shows the page's own video there,
//! by its compositor surface as Picture in Picture does, in a window over the band this view draws
//! black, so nothing's copied and nothing runs while no card shows it. GPUI can't draw over that
//! window, so the band holds only the video, and the video leaves while the stack moves.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Bounds, Context, Div, MouseButton, Pixels, Stateful, Task, Window, black, canvas,
    div, prelude::*, px, svg,
};

use super::{
    CLOSE_HOVERED, FOOTER_HEIGHT, MediaAction, NAV_RADIUS, PLATE_INSET, SECONDARY, Shell, Space,
    TILE, TILE_RADIUS, TITLE, new_space::ease_out, permission::card_listener,
};
use crate::tokens::shell::media::motion::{FOLD, OPEN, WAIT};
use crate::tokens::shell::{
    MEDIA_BUTTON, MEDIA_CONTROL, MEDIA_GAP, MEDIA_GLYPH, MEDIA_OPEN, MEDIA_OPEN_GAP, MEDIA_PEEK,
    MEDIA_PEEK_FADE, MEDIA_PEEK_INSET, MEDIA_REST, MEDIA_TITLE, MEDIA_VIDEO_ASPECT,
};

/// The most cards in the stack: the newest and two peeking (Arc 1.171).
const MOST: usize = 3;

/// A next track the page has no handler for: Arc's button stays, disabled.
const DISABLED: f32 = 0.25;

/// The stack's state beyond what the pages say.
#[derive(Default)]
pub(super) struct MediaStack {
    /// The tabs with a player, in the order their players came, the newest last.
    order: RefCell<Vec<u64>>,
    /// Whether the pointer is on the stack, and whether it has opened.
    hovered: bool,
    open: bool,
    /// The open's motion: how open it was as it last turned, and when.
    from: f32,
    turned: Option<Instant>,
    /// The pointer's rest before it opens.
    wait: Option<Task<()>>,
    /// A card put away, folding: its tab, its place from the front, and when.
    leaving: Option<(u64, usize, Instant)>,
    /// The last state logged, for check.sh.
    logged: Option<bool>,
    /// Where the open card's video band was drawn, fully open, and what the core was last asked
    /// to show: the tab, and where.
    video_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    video_shown: Option<(u64, Bounds<Pixels>)>,
}

impl Shell {
    /// The tabs with a player, newest first, as many as stack.
    pub(super) fn media_players(&self) -> Vec<u64> {
        let mut playing: Vec<u64> = (self.tabs.iter())
            .filter(|(tab, each)| {
                each.page.media
                    && !self.media_dismissed.contains(tab)
                    && !self.on_screen(**tab)
                    && self.lists(**tab)
            })
            .map(|(tab, _)| *tab)
            .collect();
        playing.sort_unstable();
        // The newest is the one whose player came last, not the newest tab.
        let mut order = self.media_stack.order.borrow_mut();
        order.retain(|tab| playing.contains(tab));
        for tab in playing {
            if !order.contains(&tab) {
                order.push(tab);
            }
        }
        order.iter().rev().take(MOST).copied().collect()
    }

    /// Whether `tab` is a favorite or in one of the window's Spaces: every window hears every
    /// tab, a private window's included, which no other window may show.
    pub(super) fn lists(&self, tab: u64) -> bool {
        let in_space = |space: &Space| space.pinned.contains(&tab) || space.today.contains(&tab);
        self.all_favorites.contains(&tab) || self.spaces.iter().any(in_space)
    }

    /// Where the pages above the footer end, over Restart to Update, the players and the tasks'
    /// plates.
    pub(super) fn list_bottom(&self) -> f32 {
        self.media_top() + self.foot_height()
    }

    /// The room the stack takes at rest, which the list ends over: its open grows over the list,
    /// so the rows never move under the pointer.
    pub(super) fn media_room(&self) -> f32 {
        match self.media_players().len() {
            0 => 0.,
            n => MEDIA_REST + (n - 1) as f32 * MEDIA_PEEK + MEDIA_GAP,
        }
    }

    /// The tab whose card shows its video, open: the newest whose page has one. Only one, as the
    /// core shows one video at a time, and none in the reveal, a window of its own over the page,
    /// which a video in the browser window's couldn't show over.
    fn media_video(&self) -> Option<u64> {
        if self.sidebar_hidden {
            return None;
        }
        (self.media_players().into_iter()).find(|tab| self.page(*tab).media_video)
    }

    /// Whether the stack is open, or opening.
    pub(super) fn media_open(&self) -> bool {
        self.media_stack.open
    }

    /// × on `tab`'s player: it goes until the page plays again, folding away as the cards above
    /// close up.
    pub(super) fn dismiss_media(&mut self, tab: u64, cx: &mut Context<Self>) {
        let place = self.media_players().iter().position(|each| *each == tab);
        if !self.media_dismissed.insert(tab) {
            return;
        }
        let now = cx.background_executor().now();
        self.media_stack.leaving = place
            .filter(|_| !crate::motion::still(cx))
            .map(|place| (tab, place, now));
        if self.media_players().is_empty() {
            self.media_hover(false, cx);
        }
        cx.notify();
    }

    /// The pointer came onto the stack or left it: it opens once the pointer rests there, and
    /// closes at once.
    pub(super) fn media_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let stack = &mut self.media_stack;
        if stack.hovered == hovered {
            return;
        }
        stack.hovered = hovered;
        if !hovered {
            stack.wait = None;
            self.turn_media(false, cx);
            return;
        }
        if stack.open {
            return;
        }
        stack.wait = Some(cx.spawn(async move |shell, cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(WAIT))
                .await;
            shell
                .update(cx, |shell, cx| {
                    shell.media_stack.wait = None;
                    if shell.media_stack.hovered {
                        shell.turn_media(true, cx);
                    }
                })
                .ok();
        }));
    }

    /// Opens or closes the stack from where it is.
    fn turn_media(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.media_stack.open == open {
            return;
        }
        let now = cx.background_executor().now();
        let from = self.media_progress(now);
        let stack = &mut self.media_stack;
        stack.open = open;
        stack.wait = None;
        match crate::motion::still(cx) {
            // Under Reduce Motion it's where it ends from its first frame.
            true => (stack.from, stack.turned) = (open as u8 as f32, None),
            false => (stack.from, stack.turned) = (from, Some(now)),
        }
        cx.notify();
    }

    /// How open the stack is at `now`, from 0 at rest to 1.
    fn media_progress(&self, now: Instant) -> f32 {
        let stack = &self.media_stack;
        let to = stack.open as u8 as f32;
        let Some(turned) = stack.turned else {
            return to;
        };
        let t = now.saturating_duration_since(turned).as_secs_f32() / OPEN;
        stack.from + (to - stack.from) * ease_out(t)
    }

    /// Steps the stack's motion for a frame, and once it's still, logs how it rests.
    pub(super) fn step_media_stack(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        if self.media_players().is_empty() && self.media_stack.leaving.is_none() {
            // Gone with no pointer leaving it: the next stack starts at rest.
            let stack = &mut self.media_stack;
            (stack.hovered, stack.open, stack.turned) = (false, false, None);
            (stack.wait, stack.logged) = (None, None);
            self.place_media_video(false, cx);
            return;
        }
        let stack = &mut self.media_stack;
        let moving = stack
            .turned
            .is_some_and(|turned| now.saturating_duration_since(turned).as_secs_f32() < OPEN);
        if !moving {
            stack.turned = None;
        }
        let folding = stack
            .leaving
            .is_some_and(|(_, _, at)| now.saturating_duration_since(at).as_secs_f32() < FOLD);
        if !folding {
            stack.leaving = None;
        }
        let open = stack.open;
        self.place_media_video(open && !moving && !folding, cx);
        if moving || folding {
            window.request_animation_frame();
            return;
        }
        let stack = &mut self.media_stack;
        if stack.logged != Some(open) {
            stack.logged = Some(open);
            // check.sh waits for this line before it clicks what the open stack shows.
            log!(
                "browser: media stack {}",
                if open { "open" } else { "rest" }
            );
        }
    }

    /// Tells the core where the open card's video goes while the stack is `settled` open, and
    /// that it's gone as the stack starts to close, the card goes or the band moves. The video is
    /// a native view GPUI can't fade or draw over, so it shows only once the stack is still.
    fn place_media_video(&mut self, settled: bool, cx: &mut Context<Self>) {
        let drawn = self.media_stack.video_drawn.get();
        let want = (settled.then(|| self.media_video()).flatten()).zip(drawn);
        let shown = self.media_stack.video_shown;
        if want == shown {
            return;
        }
        if let Some((tab, _)) = shown.filter(|(tab, _)| want.is_none_or(|(to, _)| to != *tab)) {
            // check.sh waits for this line.
            log!("browser: card video {tab} none");
            self.browser.show_video(tab, None, 0., cx);
        }
        if let Some((tab, at)) = want {
            let (x, y) = (f32::from(at.origin.x), f32::from(at.origin.y));
            let (w, h) = (f32::from(at.size.width), f32::from(at.size.height));
            // check.sh waits for this line, and captures the band from it.
            log!("browser: card video {tab} at {x} {y} {w} {h}");
            self.browser
                .show_video(tab, Some(at), TILE_RADIUS - MEDIA_OPEN_GAP, cx);
        }
        self.media_stack.video_shown = want;
    }

    /// The stack, over the footer and Restart to Update.
    pub(super) fn media_player_cards(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
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
        let now = cx.background_executor().now();
        let p = self.media_progress(now);
        // The cards front to back, with one put away folding in its place: (tab, how much of it
        // is left).
        let mut cards: Vec<(u64, f32)> = tabs.iter().map(|tab| (*tab, 1.)).collect();
        if let Some((tab, place, at)) = self.media_stack.leaving {
            let t = now.saturating_duration_since(at).as_secs_f32() / FOLD;
            if t < 1. {
                cards.insert(place.min(cards.len()), (tab, 1. - ease_out(t)));
            }
        }
        if cards.is_empty() {
            return None;
        }
        let width = self.width();
        let video = self.media_video();
        if video.is_none() {
            self.media_stack.video_drawn.set(None);
        }
        let band = media_video_band(width - 2. * PLATE_INSET);
        let mut placed = Vec::new();
        let (mut open_bottom, mut top) = (0f32, 0f32);
        for (k, &(tab, left)) in cards.iter().enumerate() {
            let rest_k = k.min(MOST - 1) as f32;
            let lerp = |rest: f32, open: f32| rest + (open - rest) * p;
            let bottom = lerp(rest_k * MEDIA_PEEK, open_bottom);
            let inset = lerp(PLATE_INSET + rest_k * MEDIA_PEEK_INSET, PLATE_INSET);
            let alpha = lerp(1. - rest_k * MEDIA_PEEK_FADE, 1.) * left;
            let open = match video == Some(tab) {
                true => MEDIA_OPEN + MEDIA_OPEN_GAP + band,
                false => MEDIA_OPEN,
            };
            let h = (MEDIA_REST + (open - MEDIA_REST) * p) * left;
            open_bottom += (h + MEDIA_OPEN_GAP) * left.max(0.);
            top = top.max(bottom + h);
            placed.push((k, tab, bottom, inset, alpha, h));
        }
        // Back to front, so the newest is over the rest.
        let cards: Vec<AnyElement> = (placed.into_iter().rev())
            .map(|(k, tab, bottom, inset, alpha, h)| {
                let front = k == 0;
                let band = (video == Some(tab)).then_some(band);
                self.media_card(tab, front, band, p, cx)
                    .absolute()
                    .left(px(inset))
                    .w(px(width - 2. * inset))
                    .bottom(px(bottom))
                    .h(px(h))
                    .opacity(alpha)
                    .into_any_element()
            })
            .collect();
        let bottom = FOOTER_HEIGHT + self.update_banner_room();
        Some(
            div()
                .id("media-stack")
                .absolute()
                .left_0()
                .w(px(width))
                .bottom(px(bottom))
                .h(px(top))
                // Over the list's last rows while it's open, which take none of its pointer.
                .occlude()
                .on_hover(
                    cx.listener(|shell, hovered: &bool, _, cx| shell.media_hover(*hovered, cx)),
                )
                .children(cards)
                .into_any_element(),
        )
    }

    /// `tab`'s card, the front one or one behind it, with the stack `p` open, and with its video
    /// in a `band` this tall over the title row if it shows it.
    fn media_card(
        &self,
        tab: u64,
        front: bool,
        band: Option<f32>,
        p: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let page = self.page(tab);
        let title = match page.media_title.is_empty() {
            true => self.title(tab),
            false => page.media_title.clone(),
        };
        let artist = (!page.media_artist.is_empty()).then(|| {
            div()
                .flex_none()
                .text_color(self.palette.text(SECONDARY))
                .child(format!(" · {}", page.media_artist))
        });
        let float = page.media_picture_in_picture.then(|| {
            let glyph = "icons/picture-in-picture-2.svg";
            self.media_button("media-pip", tab, glyph)
                .on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-picture-in-picture via player");
                    shell.browser.media(tab, MediaAction::PictureInPicture, cx);
                }))
        });
        let close = self
            .media_button("media-close", tab, "icons/x.svg")
            .on_click(cx.listener(move |shell, _, _, cx| {
                log!("browser: command media-dismiss via player");
                shell.dismiss_media(tab, cx);
            }));
        // The black the video's fitted in, which the core draws the video over once the stack is
        // still and open, and which shows until the page's first frame comes.
        let band = band.map(|height| {
            let drawn = self.media_stack.video_drawn.clone();
            let listener = canvas(
                move |bounds, window, _| {
                    let at = (p >= 1.).then_some(bounds);
                    if drawn.replace(at) != at {
                        // The shell tells the core where it's drawn at its next frame.
                        window.on_next_frame(|window, _| window.refresh());
                    }
                },
                |_, _, _, _| {},
            )
            .size_full();
            div()
                .flex_none()
                .mt(px(MEDIA_OPEN_GAP))
                .mx(px(MEDIA_OPEN_GAP))
                .h(px(height))
                .rounded(px(TILE_RADIUS - MEDIA_OPEN_GAP))
                .bg(black())
                .opacity(p)
                .child(listener)
        });
        let title_row = div()
            .flex_none()
            .h(px(MEDIA_OPEN - MEDIA_REST))
            .flex()
            .items_center()
            .pt(px(MEDIA_OPEN_GAP))
            .pl(px(MEDIA_CONTROL / 2. - 2.))
            .pr(px((MEDIA_REST - MEDIA_BUTTON) / 2.))
            .gap(px(2.))
            .opacity(p)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(MEDIA_TITLE))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(self.palette.text(TITLE))
                            .child(title),
                    )
                    .children(artist),
            )
            .children(float)
            .child(close);
        let (glyph, action) = match page.media_playing {
            true => ("icons/pause.svg", MediaAction::Pause),
            false => ("icons/play.svg", MediaAction::Play),
        };
        // A card peeking out behind takes a press whole, so its controls act only once it shows.
        let live = front || self.media_stack.open;
        let back = self
            .media_control("media-back", tab, live)
            .child(self.tab_icon(tab));
        let play = self
            .media_control("media-play", tab, live)
            .child(self.media_glyph(glyph, TITLE));
        let next_glyph = match page.media_next {
            true => SECONDARY,
            false => DISABLED,
        };
        let next = self
            .media_control("media-next", tab, live && page.media_next)
            .child(self.media_glyph("icons/skip-forward.svg", next_glyph));
        let (back, play, next) = match live {
            false => (back, play, next),
            true => (
                back.on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-show via player");
                    shell.select(tab, cx);
                })),
                play.on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-{} via player", label(action));
                    shell.browser.media(tab, action, cx);
                })),
                match page.media_next {
                    true => next.on_click(cx.listener(move |shell, _, _, cx| {
                        log!("browser: command media-next via player");
                        shell.browser.media(tab, MediaAction::Next, cx);
                    })),
                    false => next,
                },
            ),
        };
        // Evenly spaced, each in a third of the card.
        let slot = |control: Stateful<Div>| div().flex_1().flex().justify_center().child(control);
        let controls = div()
            .flex_none()
            .opacity(if front { 1. } else { p })
            .h(px(MEDIA_REST))
            .flex()
            .items_center()
            .child(slot(back))
            .child(slot(play))
            .child(slot(next));
        // check.sh finds the front card's buttons from where it's drawn.
        let listener = front.then(|| card_listener("media player", &self.media_drawn, tab));
        let fill = self.window_fill_at(1.).blend(self.palette.plate(TILE));
        let card = div()
            .id(("media-player", tab))
            .rounded(px(TILE_RADIUS))
            .overflow_hidden()
            .bg(fill)
            .flex()
            .flex_col()
            .justify_end()
            .children(listener)
            .children(band)
            .child(title_row)
            .child(controls);
        match live {
            true => card
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-show via player");
                    shell.select(tab, cx);
                })),
            // A press on a card peeking out behind opens the stack, and does nothing else: its
            // edge is too thin to aim at.
            false => card.on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| {
                    cx.stop_propagation();
                    shell.turn_media(true, cx);
                }),
            ),
        }
    }

    /// A control on the card's lower row: a plate under the pointer while it can act, as Arc's
    /// disabled next track has none.
    fn media_control(&self, id: &'static str, tab: u64, acts: bool) -> Stateful<Div> {
        let control = div()
            .id((id, tab))
            .flex_none()
            .size(px(MEDIA_CONTROL))
            .rounded(px(NAV_RADIUS))
            .flex()
            .items_center()
            .justify_center();
        match acts {
            true => control
                .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
                // Not the card's click, which would show the tab.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
            false => control,
        }
    }

    fn media_glyph(&self, glyph: &'static str, alpha: f32) -> impl IntoElement + use<> {
        svg()
            .path(glyph)
            .size(px(MEDIA_GLYPH))
            .text_color(self.palette.text(alpha))
    }

    /// A button on the card's title row, which a task's plate at the foot has for its ✕ too.
    pub(super) fn media_button(
        &self,
        id: &'static str,
        tab: u64,
        glyph: &'static str,
    ) -> Stateful<Div> {
        div()
            .id((id, tab))
            .flex_none()
            .size(px(MEDIA_BUTTON))
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
            .flex()
            .items_center()
            .justify_center()
            .child(self.media_glyph(glyph, SECONDARY))
            // Not the player's click, which would show the tab.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}

/// How the log names `action`.
/// The height of the open card's video band, in a card `width` wide.
fn media_video_band(width: f32) -> f32 {
    (width - 2. * MEDIA_OPEN_GAP) / MEDIA_VIDEO_ASPECT
}

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
    use std::time::Duration;

    use gpui::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

    use crate::shell::{Folders, Page, Shell, recorder::Recorder};
    use crate::tokens::shell::media::motion::{OPEN, WAIT};
    use crate::tokens::shell::{
        MEDIA_BUTTON, MEDIA_GAP, MEDIA_OPEN, MEDIA_OPEN_GAP, MEDIA_PEEK, MEDIA_REST,
        MEDIA_VIDEO_ASPECT, TILE_RADIUS,
    };

    fn space(shell: &mut Shell, tabs: Vec<u64>, cx: &mut gpui::Context<Shell>) {
        let space = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), tabs, Vec::new(), Folders::default(), cx);
        shell.set_selected(1, cx);
    }

    /// One Space with tabs 1 to 5, tab 1 shown.
    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| space(shell, vec![1, 2, 3, 4, 5], cx));
        (shell, host)
    }

    /// The same in a window, drawn.
    fn drawn(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| space(shell, vec![1, 2, 3, 4, 5], cx));
        (shell, cx, host)
    }

    fn playing(shell: &mut Shell, tab: u64, media_playing: bool, cx: &mut gpui::Context<Shell>) {
        let page = Page {
            media: true,
            media_playing,
            media_picture_in_picture: true,
            media_video: true,
            ..shell.page(tab)
        };
        shell.set_page(tab, page, cx);
    }

    /// The newest player is in front, by when it came rather than by tab, and two more peek
    /// behind it; the list ends over them at rest.
    #[gpui::test]
    fn the_newest_player_is_in_front_and_two_peek(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        shell.update(cx, |shell, cx| {
            let bare = shell.list_bottom();
            for tab in [4, 2, 3, 5] {
                playing(shell, tab, true, cx);
            }
            assert_eq!(shell.media_players(), [5, 3, 2]);
            let room = MEDIA_REST + 2. * MEDIA_PEEK + MEDIA_GAP;
            assert_eq!(shell.list_bottom(), bare + room);
            // The front one put away, the one it hid comes back behind.
            shell.dismiss_media(5, cx);
            assert_eq!(shell.media_players(), [3, 2, 4]);
        });
    }

    /// A tab playing out of sight has a player, the one shown none; a paused one keeps it, × puts
    /// it away until the page plays again, and the list ends over the stack.
    #[gpui::test]
    fn a_tab_playing_out_of_sight_has_a_player_until_put_away(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        shell.update(cx, |shell, cx| {
            let bare = shell.list_bottom();
            playing(shell, 1, true, cx);
            playing(shell, 2, true, cx);
            assert_eq!(shell.media_players(), [2]);
            assert_eq!(shell.list_bottom(), bare + MEDIA_REST + MEDIA_GAP);
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

    /// The stack opens once the pointer has rested on it, not as it passes, closes as it
    /// leaves, and the list stays where it is.
    #[gpui::test]
    fn the_stack_opens_once_the_pointer_rests_and_closes_at_once(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        let bottom = shell.update(cx, |shell, cx| {
            playing(shell, 2, true, cx);
            playing(shell, 3, true, cx);
            shell.media_hover(true, cx);
            shell.list_bottom()
        });
        let wait = Duration::from_secs_f32(WAIT);
        cx.executor().advance_clock(wait / 2);
        shell.update(cx, |shell, cx| {
            assert!(!shell.media_open());
            // Passing through: it never opens.
            shell.media_hover(false, cx);
        });
        cx.executor().advance_clock(wait);
        shell.update(cx, |shell, cx| {
            assert!(!shell.media_open());
            shell.media_hover(true, cx);
        });
        cx.executor().advance_clock(wait + Duration::from_millis(1));
        shell.update(cx, |shell, cx| {
            assert!(shell.media_open());
            assert_eq!(shell.list_bottom(), bottom);
            shell.media_hover(false, cx);
            assert!(!shell.media_open());
        });
    }

    /// The open grows over `OPEN` from where it is, and under Reduce Motion is open at once.
    #[gpui::test]
    fn the_open_eases_and_reduce_motion_keeps_it_still(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        shell.update(cx, |shell, cx| {
            playing(shell, 2, true, cx);
            shell.turn_media(true, cx);
        });
        let now = |cx: &mut TestAppContext| cx.executor().now();
        let at = now(cx);
        let half = shell.read_with(cx, |shell, _| shell.media_progress(at));
        assert_eq!(half, 0.);
        cx.executor()
            .advance_clock(Duration::from_secs_f32(OPEN / 2.));
        let at = now(cx);
        let half = shell.read_with(cx, |shell, _| shell.media_progress(at));
        assert!(half > 0.5 && half < 1., "{half}");
        cx.executor().advance_clock(Duration::from_secs_f32(OPEN));
        let at = now(cx);
        assert_eq!(shell.read_with(cx, |shell, _| shell.media_progress(at)), 1.);
        cx.update(|cx| cx.set_reduce_motion(true));
        shell.update(cx, |shell, cx| shell.turn_media(false, cx));
        let at = now(cx);
        assert_eq!(shell.read_with(cx, |shell, _| shell.media_progress(at)), 0.);
    }

    /// A press on a card peeking out behind opens the stack and does nothing else.
    #[gpui::test]
    fn a_press_on_a_peeking_card_opens_the_stack_only(cx: &mut TestAppContext) {
        let (shell, cx, host) = drawn(cx);
        shell.update(cx, |shell, cx| {
            playing(shell, 2, true, cx);
            playing(shell, 3, true, cx);
        });
        cx.run_until_parked();
        let front = shell
            .read_with(cx, |shell, _| shell.media_drawn.get())
            .expect("no player");
        host.take();
        let edge = point(front.center().x, front.top() - px(MEDIA_PEEK / 2.));
        cx.simulate_click(edge, Modifiers::none());
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.media_open()));
        let calls = host.take();
        assert!(
            !calls
                .iter()
                .any(|call| call.starts_with("select") || call.starts_with("media ")),
            "{calls:?}"
        );
    }

    /// Open, the front card's title row has Picture in Picture while the page can float, which
    /// floats it, and ×, which puts the card away as the stack stays open; a click elsewhere on
    /// a card shows its tab.
    #[gpui::test]
    fn the_open_card_floats_the_page_and_puts_itself_away(cx: &mut TestAppContext) {
        let (shell, cx, host) = drawn(cx);
        cx.update(|_, cx| cx.set_reduce_motion(true));
        shell.update(cx, |shell, cx| {
            playing(shell, 2, true, cx);
            playing(shell, 3, true, cx);
            shell.media_hover(true, cx);
            shell.turn_media(true, cx);
        });
        cx.run_until_parked();
        let front = shell
            .read_with(cx, |shell, _| shell.media_drawn.get())
            .expect("no player");
        // Its page has a video, which it shows over its title row.
        let band = shell
            .read_with(cx, |shell, _| shell.media_stack.video_drawn.get())
            .expect("no video");
        let height = MEDIA_OPEN + MEDIA_OPEN_GAP + f32::from(band.size.height);
        assert_eq!(f32::from(front.size.height), height);
        // From the right, 4.5 in: ×, then Picture in Picture 2 before it.
        let y = front.bottom() - px(MEDIA_REST + (MEDIA_OPEN - MEDIA_REST - MEDIA_OPEN_GAP) / 2.);
        let close = front.right() - px((MEDIA_REST - MEDIA_BUTTON) / 2. + MEDIA_BUTTON / 2.);
        let float = close - px(MEDIA_BUTTON + 2.);
        host.take();
        cx.simulate_click(point(float, y), Modifiers::none());
        cx.run_until_parked();
        let calls = host.take();
        assert!(
            calls.contains(&"media 3 PictureInPicture".to_owned()),
            "{calls:?}"
        );
        cx.simulate_click(point(close, y), Modifiers::none());
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.media_players(), [2]);
            assert!(shell.media_open());
        });
        let front = shell
            .read_with(cx, |shell, _| shell.media_drawn.get())
            .expect("no player");
        cx.simulate_click(point(front.center().x, y), Modifiers::none());
        cx.run_until_parked();
        let calls = host.take();
        assert!(calls.contains(&"select 2".to_owned()), "{calls:?}");
    }

    /// Open and still, the newest card whose page has a video shows it, over its title row and
    /// fitted to the card, and the core is told where; as the stack closes, or in the reveal, it's
    /// told it's gone. A page with only sound has no band.
    #[gpui::test]
    fn the_open_card_shows_the_newest_video_while_the_stack_is_open(cx: &mut TestAppContext) {
        let (shell, cx, host) = drawn(cx);
        cx.update(|_, cx| cx.set_reduce_motion(true));
        shell.update(cx, |shell, cx| {
            for tab in [2, 3, 4] {
                playing(shell, tab, true, cx);
            }
            let sound = Page {
                media_video: false,
                ..shell.page(4)
            };
            shell.set_page(4, sound, cx);
        });
        cx.run_until_parked();
        // At rest, nothing.
        let calls = host.take();
        assert!(!calls.iter().any(|call| call.starts_with("show video")));
        shell.update(cx, |shell, cx| {
            shell.media_hover(true, cx);
            shell.turn_media(true, cx);
        });
        cx.run_until_parked();
        // The frame after the band's drawn, which the app's window asks for.
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let band = shell
            .read_with(cx, |shell, _| shell.media_stack.video_drawn.get())
            .expect("no video");
        let (w, h) = (f32::from(band.size.width), f32::from(band.size.height));
        assert!((w / h - MEDIA_VIDEO_ASPECT).abs() < 0.01, "{w}x{h}");
        let shown = format!(
            "show video 3 at {} {} {w} {h} round {}",
            f32::from(band.origin.x),
            f32::from(band.origin.y),
            TILE_RADIUS - MEDIA_OPEN_GAP
        );
        let calls = host.take();
        let videos: Vec<&String> = (calls.iter())
            .filter(|call| call.starts_with("show video"))
            .collect();
        assert_eq!(videos, [&shown], "{calls:?}");
        shell.update(cx, |shell, cx| shell.media_hover(false, cx));
        cx.run_until_parked();
        assert!(host.take().contains(&"show video 3 none".to_owned()));
        // Open in the reveal, nothing.
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            shell.media_hover(true, cx);
            shell.turn_media(true, cx);
        });
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let calls = host.take();
        assert!(!calls.iter().any(|call| call.starts_with("show video")));
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
            shell.set_tabs(
                1,
                Vec::new(),
                vec![1, 2, 3],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_tabs(5, vec![6], Vec::new(), Vec::new(), Folders::default(), cx);
            shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_favorites(vec![(7, 1)], cx);
            for tab in [6, 7, 10] {
                playing(shell, tab, true, cx);
            }
        };
        shell.update(cx, |shell, cx| {
            tabs(shell, cx);
            assert_eq!(shell.media_players(), [7, 6]);
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
        assert!(
            shell
                .read_with(cx, |shell, _| shell.media_drawn.get())
                .is_none()
        );
        shell.update(cx, |shell, cx| playing(shell, 6, true, cx));
        cx.run_until_parked();
        assert!(
            shell
                .read_with(cx, |shell, _| shell.media_drawn.get())
                .is_some()
        );
    }
}
