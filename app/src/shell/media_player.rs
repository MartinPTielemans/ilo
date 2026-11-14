//! The players at the sidebar's foot (#700), as Arc 1.171's stack: a card for each tab whose page
//! has media the browser can control while the window doesn't show it, playing or paused. At rest
//! only the newest shows, its favicon (back to its tab), the previous track, play or pause, the
//! next track and the tab's volume, evenly spaced, with up to two older ones peeking out behind it.
//! Under the pointer every card grows, the oldest on top, and gains a title row with Picture in
//! Picture while the page can float and nothing of it does, as Arc's Back to PiP, and × to put it
//! away until the page plays again. A click elsewhere on a card shows its tab. The volume button
//! opens a drawer over the controls with a slider for the tab's volume, which closes with the
//! stack. The title is cut with an ellipsis where Arc's scrolls, as a marquee would draw every
//! frame.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use web_time::Instant;

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, Div, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Stateful, Task, Window,
    accesskit::{Action, Role},
    canvas, div,
    prelude::*,
    px, relative, svg,
};

use super::{
    CLOSE_HOVERED, MediaAction, NAV_RADIUS, PLATE_INSET, SECONDARY, Shell, Space, TILE,
    TILE_RADIUS, TITLE,
    access::Accessible,
    new_space::ease_out,
    permission::{card_listener, part_listener},
};
use crate::tokens::shell::media::motion::{DRAWER, FOLD, OPEN, WAIT};
use crate::tokens::shell::{
    MEDIA_BUTTON, MEDIA_CONTROL, MEDIA_DRAWER, MEDIA_GAP, MEDIA_GLYPH, MEDIA_OPEN, MEDIA_OPEN_GAP,
    MEDIA_PEEK, MEDIA_PEEK_FADE, MEDIA_PEEK_INSET, MEDIA_REST, MEDIA_SLIDER_KNOB,
    MEDIA_SLIDER_TRACK, MEDIA_TITLE,
};

/// The most cards in the stack: the newest and two peeking (Arc 1.171).
const MOST: usize = 3;

/// A previous or next track the page has no handler for: Arc's button stays, disabled.
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
    /// The tab whose volume drawer shows, open or closing, whether it's open, and its motion as
    /// the open's.
    drawer: Option<u64>,
    drawer_open: bool,
    drawer_from: f32,
    drawer_turned: Option<Instant>,
    /// The volume the slider was moved to, shown until the drawer turns so it never jumps back
    /// before the core says so, and whether the pointer holds it.
    slid: Option<f32>,
    sliding: bool,
    /// Where the slider's track was drawn.
    pub(super) track: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The drawer last logged, for check.sh.
    drawer_logged: Option<u64>,
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
            // A slider dragged off the stack keeps it open until it's let go.
            if !stack.sliding {
                self.turn_media(false, cx);
            }
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
        if !open {
            self.turn_drawer(None, cx);
        }
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

    /// Adjust Tab Volume on `tab`'s card: its drawer opens, with the stack, or closes.
    pub(super) fn toggle_volume(&mut self, tab: u64, cx: &mut Context<Self>) {
        let stack = &self.media_stack;
        let open = !(stack.drawer == Some(tab) && stack.drawer_open);
        if open {
            self.turn_media(true, cx);
        }
        self.turn_drawer(open.then_some(tab), cx);
    }

    /// Opens `tab`'s volume drawer, or with none closes the one open, from where it is.
    fn turn_drawer(&mut self, tab: Option<u64>, cx: &mut Context<Self>) {
        let stack = &self.media_stack;
        if stack.drawer_open == tab.is_some() && (tab.is_none() || stack.drawer == tab) {
            return;
        }
        let now = cx.background_executor().now();
        // Another card's drawer opens from closed, as the one open closes at once.
        let from = match tab {
            Some(tab) if stack.drawer != Some(tab) => 0.,
            _ => self.drawer_progress(now),
        };
        let stack = &mut self.media_stack;
        (stack.slid, stack.sliding) = (None, false);
        stack.drawer = tab.or(stack.drawer);
        stack.drawer_open = tab.is_some();
        match crate::motion::still(cx) {
            true => (stack.drawer_from, stack.drawer_turned) = (tab.is_some() as u8 as f32, None),
            false => (stack.drawer_from, stack.drawer_turned) = (from, Some(now)),
        }
        cx.notify();
    }

    /// How open the volume drawer is at `now`, from 0 closed to 1.
    fn drawer_progress(&self, now: Instant) -> f32 {
        let stack = &self.media_stack;
        let to = stack.drawer_open as u8 as f32;
        let Some(turned) = stack.drawer_turned else {
            return to;
        };
        let t = now.saturating_duration_since(turned).as_secs_f32() / DRAWER;
        stack.drawer_from + (to - stack.drawer_from) * ease_out(t)
    }

    /// How far `tab`'s volume drawer is out at `now`.
    fn drawer_out(&self, tab: u64, now: Instant) -> f32 {
        match self.media_stack.drawer == Some(tab) {
            true => self.drawer_progress(now),
            false => 0.,
        }
    }

    /// The volume `tab`'s card shows: where its slider was moved, else the page's.
    fn media_volume(&self, tab: u64) -> f32 {
        let stack = &self.media_stack;
        match (stack.drawer == Some(tab), stack.slid) {
            (true, Some(slid)) => slid,
            _ => self.page(tab).volume.0,
        }
    }

    /// The slider was pressed or dragged to `x`: the tab's volume follows it, a hundredth a step.
    fn slide_volume(&mut self, tab: u64, x: f32, cx: &mut Context<Self>) {
        let Some(track) = self.media_stack.track.get() else {
            return;
        };
        let width = f32::from(track.size.width).max(1.);
        let at = (x - f32::from(track.left())) / width;
        self.set_media_volume(tab, at, cx);
    }

    fn set_media_volume(&mut self, tab: u64, volume: f32, cx: &mut Context<Self>) {
        let volume = (volume.clamp(0., 1.) * 100.).round() / 100.;
        if self.media_volume(tab) == volume && self.media_stack.slid.is_some() {
            return;
        }
        self.media_stack.slid = Some(volume);
        self.browser.set_volume(tab, volume, cx);
        cx.notify();
    }

    /// The slider let go: a stack the pointer left while it was held closes now.
    fn release_volume(&mut self, cx: &mut Context<Self>) {
        let stack = &mut self.media_stack;
        if !std::mem::take(&mut stack.sliding) {
            return;
        }
        if let Some(volume) = stack.slid {
            // check.sh waits for this line.
            log!("browser: command media-volume {volume:.2} via player");
        }
        if !stack.hovered {
            self.turn_media(false, cx);
        }
        cx.notify();
    }

    /// Steps the stack's motion for a frame, and once it's still, logs how it rests.
    pub(super) fn step_media_stack(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        if self.media_players().is_empty() && self.media_stack.leaving.is_none() {
            // Gone with no pointer leaving it: the next stack starts at rest.
            let stack = &mut self.media_stack;
            (stack.hovered, stack.open, stack.turned) = (false, false, None);
            (stack.wait, stack.logged) = (None, None);
            (stack.drawer, stack.drawer_open, stack.drawer_turned) = (None, false, None);
            (stack.slid, stack.sliding, stack.drawer_logged) = (None, false, None);
            return;
        }
        let stack = &mut self.media_stack;
        let moving = stack
            .turned
            .is_some_and(|turned| now.saturating_duration_since(turned).as_secs_f32() < OPEN);
        if !moving {
            stack.turned = None;
        }
        let drawing = (stack.drawer_turned)
            .is_some_and(|turned| now.saturating_duration_since(turned).as_secs_f32() < DRAWER);
        if !drawing {
            stack.drawer_turned = None;
            if !stack.drawer_open {
                stack.drawer = None;
            }
        }
        let shown = stack.drawer.filter(|_| stack.drawer_open);
        if !drawing && stack.drawer_logged != shown {
            stack.drawer_logged = shown;
            // check.sh waits for this line before it drags the slider.
            match shown {
                Some(tab) => log!("browser: media volume drawer {tab} open"),
                None => log!("browser: media volume drawer closed"),
            }
        }
        let moving = moving || drawing;
        let folding = stack
            .leaving
            .is_some_and(|(_, _, at)| now.saturating_duration_since(at).as_secs_f32() < FOLD);
        if !folding {
            stack.leaving = None;
        }
        if moving || folding {
            window.request_animation_frame();
            return;
        }
        let open = stack.open;
        if stack.logged != Some(open) {
            stack.logged = Some(open);
            // check.sh waits for this line before it clicks what the open stack shows.
            log!(
                "browser: media stack {}",
                if open { "open" } else { "rest" }
            );
        }
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
        let height = MEDIA_REST + (MEDIA_OPEN - MEDIA_REST) * p;
        let mut placed = Vec::new();
        let (mut open_bottom, mut top) = (0f32, 0f32);
        for (k, &(tab, left)) in cards.iter().enumerate() {
            let rest_k = k.min(MOST - 1) as f32;
            let lerp = |rest: f32, open: f32| rest + (open - rest) * p;
            let bottom = lerp(rest_k * MEDIA_PEEK, open_bottom);
            let inset = lerp(PLATE_INSET + rest_k * MEDIA_PEEK_INSET, PLATE_INSET);
            let alpha = lerp(1. - rest_k * MEDIA_PEEK_FADE, 1.) * left;
            // Its volume drawer grows it up from its bottom, which stays put.
            let drawer = self.drawer_out(tab, now);
            let h = (height + MEDIA_DRAWER * drawer) * left;
            open_bottom += (h + MEDIA_OPEN_GAP) * left.max(0.);
            top = top.max(bottom + h);
            placed.push((k, tab, bottom, inset, alpha, h, drawer));
        }
        // Back to front, so the newest is over the rest.
        let cards: Vec<AnyElement> = (placed.into_iter().rev())
            .map(|(k, tab, bottom, inset, alpha, h, drawer)| {
                let front = k == 0;
                self.media_card(tab, front, p, drawer, cx)
                    .absolute()
                    .left(px(inset))
                    .w(px(width - 2. * inset))
                    .bottom(px(bottom))
                    .h(px(h))
                    .opacity(alpha)
                    .into_any_element()
            })
            .collect();
        let bottom = self.over_safe_start() + self.update_banner_room();
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

    /// `tab`'s card, the front one or one behind it, with the stack `p` open and its volume drawer
    /// `drawer` out.
    fn media_card(
        &self,
        tab: u64,
        front: bool,
        p: f32,
        drawer: f32,
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
            .button_named("Back to Tab")
            .child(self.tab_icon(tab));
        let play = self
            .media_control("media-play", tab, live)
            .button_named(match action {
                MediaAction::Pause => "Pause",
                _ => "Play",
            })
            .child(self.media_glyph(glyph, TITLE));
        // Previous and next without a handler stay, disabled, with no plate and no click.
        let skip = |id, glyph, name, action, acts: bool| {
            let alpha = if acts { SECONDARY } else { DISABLED };
            let control = self
                .media_control(id, tab, live && acts)
                .button_named(name)
                .child(self.media_glyph(glyph, alpha));
            match live && acts {
                true => control.on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-{} via player", label(action));
                    shell.browser.media(tab, action, cx);
                })),
                false => control,
            }
        };
        let previous = skip(
            "media-previous",
            "icons/skip-back.svg",
            "Previous Track",
            MediaAction::Previous,
            page.media_previous,
        );
        let next = skip(
            "media-next",
            "icons/skip-forward.svg",
            "Next Track",
            MediaAction::Next,
            page.media_next,
        );
        let volume = self.media_volume(tab);
        let adjust = self
            .media_control("media-volume", tab, live)
            .button_named("Adjust Tab Volume")
            .child(self.media_glyph(volume_glyph(volume), SECONDARY));
        let (back, play, adjust) = match live {
            false => (back, play, adjust),
            true => (
                back.on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-show via player");
                    shell.select(tab, cx);
                })),
                play.on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-{} via player", label(action));
                    shell.browser.media(tab, action, cx);
                })),
                adjust.on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command media-volume via player");
                    shell.toggle_volume(tab, cx);
                })),
            ),
        };
        // Evenly spaced, each in a fifth of the card.
        let slot = |control: Stateful<Div>| div().flex_1().flex().justify_center().child(control);
        let controls = div()
            .flex_none()
            .opacity(if front { 1. } else { p })
            .h(px(MEDIA_REST))
            .flex()
            .items_center()
            .child(slot(back))
            .child(slot(previous))
            .child(slot(play))
            .child(slot(next))
            .child(slot(adjust));
        let drawer_row = (drawer > 0.).then(|| {
            div()
                .flex_none()
                .h(px(MEDIA_DRAWER * drawer))
                .overflow_hidden()
                .opacity(drawer)
                .flex()
                .items_center()
                .px(px(MEDIA_CONTROL / 2. - 2.))
                .child(self.media_slider(tab, volume, cx))
        });
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
            .child(title_row)
            .children(drawer_row)
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

    /// The volume drawer's slider at `volume`: a press puts the knob under the pointer, which
    /// drags it anywhere in the window until it's let go.
    fn media_slider(&self, tab: u64, volume: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let sliding = self.media_stack.sliding;
        let shell = cx.weak_entity();
        let drag = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                if !sliding {
                    return;
                }
                let moved = shell.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    // Only the drag's moves, as the side panel's edge takes them.
                    if phase == DispatchPhase::Capture
                        && event.pressed_button == Some(MouseButton::Left)
                    {
                        let x = f32::from(event.position.x);
                        moved
                            .update(cx, |shell, cx| shell.slide_volume(tab, x, cx))
                            .ok();
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        shell.update(cx, |shell, cx| shell.release_volume(cx)).ok();
                    }
                });
            },
        )
        .absolute()
        .size_full();
        let knob = MEDIA_SLIDER_KNOB;
        // The knob's centre runs the track's length, which check.sh reads from the log.
        let track = div()
            .relative()
            .flex_1()
            .h(px(MEDIA_SLIDER_TRACK))
            .rounded_full()
            .bg(self.palette.text(DISABLED))
            .child(part_listener(
                "media volume".into(),
                &self.media_stack.track,
                tab,
            ))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(relative(volume))
                    .rounded_full()
                    .bg(self.palette.text(SECONDARY)),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(volume))
                    .top(px((MEDIA_SLIDER_TRACK - knob) / 2.))
                    .ml(px(-knob / 2.))
                    .size(px(knob))
                    .rounded_full()
                    .bg(self.palette.text(TITLE)),
            )
            .child(drag);
        let (up, down) = (cx.weak_entity(), cx.weak_entity());
        div()
            .id(("media-volume-slider", tab))
            .flex_1()
            .h(px(MEDIA_DRAWER))
            .px(px(knob / 2.))
            .flex()
            .items_center()
            .role(Role::Slider)
            .aria_label("Tab Volume")
            .aria_min_numeric_value(0.)
            .aria_max_numeric_value(100.)
            .aria_numeric_value_step(10.)
            .aria_numeric_value((volume * 100.).round() as f64)
            .on_a11y_action(Action::Increment, move |_, _, cx| {
                (up.update(cx, |shell, cx| {
                    let volume = shell.media_volume(tab) + 0.1;
                    shell.set_media_volume(tab, volume, cx)
                }))
                .ok();
            })
            .on_a11y_action(Action::Decrement, move |_, _, cx| {
                (down.update(cx, |shell, cx| {
                    let volume = shell.media_volume(tab) - 0.1;
                    shell.set_media_volume(tab, volume, cx)
                }))
                .ok();
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    // Not the card's click, which would show the tab.
                    cx.stop_propagation();
                    shell.media_stack.sliding = true;
                    shell.slide_volume(tab, f32::from(event.position.x), cx);
                }),
            )
            // A release over the slider, which can come before the frame that listens in the whole
            // window, as a click's does.
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| shell.release_volume(cx)),
            )
            .child(track)
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
fn label(action: MediaAction) -> &'static str {
    match action {
        MediaAction::Play => "play",
        MediaAction::Pause => "pause",
        MediaAction::Previous => "previous",
        MediaAction::Next => "next",
        MediaAction::PictureInPicture => "picture-in-picture",
    }
}

/// The volume button's glyph: a speaker with full waves, one wave under half, or none at 0, as
/// Arc's has 3 waves, 1 or none.
fn volume_glyph(volume: f32) -> &'static str {
    match volume {
        volume if volume <= 0. => "icons/volume.svg",
        volume if volume < 0.5 => "icons/volume-1.svg",
        _ => "icons/volume-2.svg",
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::Duration;

    use gpui::{
        AppContext, Bounds, Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels,
        PlatformInput, Point, TestAppContext, VisualTestContext, point, px,
    };

    use crate::shell::{Folders, Page, Shell, recorder::Recorder};
    use crate::tokens::shell::media::motion::{OPEN, WAIT};
    use crate::tokens::shell::{
        MEDIA_BUTTON, MEDIA_DRAWER, MEDIA_GAP, MEDIA_OPEN, MEDIA_OPEN_GAP, MEDIA_PEEK, MEDIA_REST,
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
        assert_eq!(f32::from(front.size.height), MEDIA_OPEN);
        // From the right, 4.5 in: ×, then Picture in Picture 2 before it.
        let y = front.top() + px(MEDIA_OPEN_GAP + (MEDIA_OPEN - MEDIA_REST - MEDIA_OPEN_GAP) / 2.);
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

    /// The front card's drawn bounds.
    fn front(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Bounds<Pixels> {
        (shell.read_with(cx, |shell, _| shell.media_drawn.get())).expect("no player")
    }

    /// The middle of the front card's control `slot`, of the five from the left.
    fn control(shell: &Entity<Shell>, cx: &mut VisualTestContext, slot: usize) -> Point<Pixels> {
        let card = front(shell, cx);
        let fifth = card.size.width / 5.;
        let x = card.left() + fifth * slot as f32 + fifth / 2.;
        point(x, card.bottom() - px(MEDIA_REST / 2.))
    }

    /// Tab 2 playing out of sight, with the stack open and still.
    fn open_player(
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let (shell, cx, host) = drawn(cx);
        cx.update(|_, cx| cx.set_reduce_motion(true));
        shell.update(cx, |shell, cx| {
            playing(shell, 2, true, cx);
            shell.media_hover(true, cx);
            shell.turn_media(true, cx);
        });
        cx.run_until_parked();
        host.take();
        (shell, cx, host)
    }

    /// Previous is the second of five controls, as Arc's: disabled with no plate while the page
    /// has no handler for it, and with one, it skips back.
    #[gpui::test]
    fn previous_skips_back_once_the_page_has_a_handler(cx: &mut TestAppContext) {
        let (shell, cx, host) = open_player(cx);
        let previous = control(&shell, cx, 1);
        cx.simulate_click(previous, Modifiers::none());
        cx.run_until_parked();
        let calls = host.take();
        assert!(
            !calls.iter().any(|call| call.starts_with("media ")),
            "{calls:?}"
        );
        shell.update(cx, |shell, cx| {
            let page = Page {
                media_previous: true,
                ..shell.page(2)
            };
            shell.set_page(2, page, cx);
        });
        cx.run_until_parked();
        cx.simulate_click(previous, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["media 2 Previous"]);
    }

    /// Adjust Tab Volume, the last control, opens a drawer that grows the card 33 with its bottom
    /// put, the controls staying under the pointer. A press on its slider sets the volume where it
    /// is, a drag follows the pointer, and the glyph shows the volume; the drawer closes with the
    /// stack.
    #[gpui::test]
    fn the_volume_drawer_grows_the_card_and_its_slider_sets_the_volume(cx: &mut TestAppContext) {
        let (shell, cx, host) = open_player(cx);
        let before = front(&shell, cx);
        let adjust = control(&shell, cx, 4);
        cx.simulate_click(adjust, Modifiers::none());
        cx.run_until_parked();
        let card = front(&shell, cx);
        assert_eq!(f32::from(card.size.height), MEDIA_OPEN + MEDIA_DRAWER);
        assert_eq!(card.bottom(), before.bottom());
        assert_eq!(control(&shell, cx, 4), adjust);
        let track =
            (shell.read_with(cx, |shell, _| shell.media_stack.track.get())).expect("no slider");
        let at = |part: f32| point(track.left() + track.size.width * part, track.center().y);
        // A click lets go of the slider, its release coming before the next frame, as the
        // driver's does.
        cx.update(|window, cx| {
            let (position, modifiers) = (at(0.5), Modifiers::none());
            let (button, click_count) = (MouseButton::Left, 1);
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button,
                    position,
                    modifiers,
                    click_count,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button,
                    position,
                    modifiers,
                    click_count,
                }),
                cx,
            );
        });
        cx.run_until_parked();
        assert_eq!(host.take(), ["volume 2 0.50"]);
        assert!(!shell.read_with(cx, |shell, _| shell.media_stack.sliding));
        let left = MouseButton::Left;
        cx.simulate_mouse_down(at(0.25), left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["volume 2 0.25"]);
        // Dragged past the track's start, it stops at 0.
        let off = point(card.left() + px(2.), track.top() - px(6.));
        cx.simulate_mouse_move(off, left, Modifiers::none());
        cx.simulate_mouse_up(off, left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["volume 2 0.00"]);
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.media_volume(2), 0.);
            assert!(shell.media_open(), "the drag closed the stack");
        });
        shell.update(cx, |shell, cx| shell.media_hover(false, cx));
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| assert_eq!(shell.media_stack.drawer, None));
        assert_eq!(f32::from(front(&shell, cx).size.height), MEDIA_REST);
    }

    /// The drawer eases out from the card over `DRAWER`.
    #[gpui::test]
    fn the_drawer_eases_out(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        shell.update(cx, |shell, cx| {
            playing(shell, 2, true, cx);
            shell.toggle_volume(2, cx);
            assert!(shell.media_open());
        });
        let at = cx.executor().now();
        assert_eq!(shell.read_with(cx, |shell, _| shell.drawer_out(2, at)), 0.);
        let half = Duration::from_secs_f32(super::DRAWER / 2.);
        cx.executor().advance_clock(half);
        let at = cx.executor().now();
        let out = shell.read_with(cx, |shell, _| shell.drawer_out(2, at));
        assert!(out > 0.5 && out < 1., "{out}");
        shell.update(cx, |shell, cx| shell.toggle_volume(2, cx));
        cx.executor().advance_clock(half * 3);
        let at = cx.executor().now();
        assert_eq!(shell.read_with(cx, |shell, _| shell.drawer_out(2, at)), 0.);
    }

    /// The glyph has full waves, one under half, and none at 0, as Arc's 3, 1 or none.
    #[test]
    fn the_volume_glyph_shows_how_loud() {
        let glyphs = [1., 0.5, 0.49, 0.01, 0.].map(super::volume_glyph);
        let waves = ["volume-2", "volume-2", "volume-1", "volume-1", "volume."];
        for (glyph, waves) in glyphs.iter().zip(waves) {
            assert!(glyph.contains(waves), "{glyph}");
        }
    }
}
