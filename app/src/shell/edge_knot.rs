//! The edge knot: with the sidebar hidden, a small tab at the window's edge on the sidebar's side,
//! only while a thread needs the user, that brings the sidebar out when clicked
//! (window-and-threads.md › Sidebar hidden). GPUI can't draw over the page's native view, so it's a
//! window of its own, the browser window's child, as the reveal's panel is. It's drawn from the
//! threads' own state alone and draws nothing more than that one needs the user: no words, no
//! count. Its tooltip and description say only what the thread's line in the sidebar already
//! shows. Nothing logs whether it shows.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Bounds, BoxShadow, Context, MouseButton, MouseDownEvent, Rgba, SharedString,
    Window, div, point, prelude::*, px, rgba, size,
};

use super::Shell;
use super::access::Accessible as _;
use crate::agents::task::Dot;
use crate::palette::{Palette, Tone, over};
use crate::tokens::floating::{FILL, RIM};
use crate::tokens::palette::INK_HOVER_MIX;
use crate::tokens::shell::edge_knot::{
    HEIGHT, MARGIN, MARK, MARK_NUDGE, RADIUS, SHADOW, SHADOW_BLUR, SHADOW_Y, TOP, WIDTH,
};
use crate::tooltip::{Anchor, Tip};

/// What a screen reader calls it (words table, XK1).
pub const EDGE_KNOT_NAME: &str = "A thread needs you";

/// How long after it comes a press on it does nothing, so a click meant for the page's edge as it
/// appears by itself, as a thread starts to need the user or the sidebar brought out goes away,
/// isn't taken for one on it.
pub const PRESS_GUARD: Duration = Duration::from_millis(500);

impl Shell {
    /// Notes which threads of this window's Spaces need the user, in the order they came to, after
    /// the threads' cards change. A thread that waits is in every Space's list
    /// (`task_cards::lists_in`), so any Space's counts.
    pub(super) fn note_waiting(&mut self) {
        let homes: Vec<u64> = self.spaces.iter().map(|space| space.id).collect();
        let waits: Vec<SharedString> = (self.task_cards.iter())
            .filter(|card| card.dot == Dot::Waiting && homes.contains(&card.home))
            .map(|card| card.agent.clone())
            .collect();
        self.waiting_order.retain(|agent| waits.contains(agent));
        for agent in waits {
            if !self.waiting_order.contains(&agent) {
                self.waiting_order.push(agent);
            }
        }
    }

    /// What the knot's tooltip says and is described as after its name: the title of the thread
    /// that came to need the user first, as its line in the sidebar draws it, never more of its
    /// words. The second line that line grows while it waits joins it, after " · ", once that line
    /// is in (the words table's N1, the one-line thread's).
    pub fn edge_knot_said(&self) -> Option<SharedString> {
        let first = self.waiting_order.first()?;
        let card = (self.task_cards.iter()).find(|card| &card.agent == first)?;
        Some(card.title.clone())
    }

    /// Where the knot goes, its top down the window's content, or None while it doesn't show: with
    /// the sidebar hidden and still, its panel in, the Library shut, no page full screen, in a
    /// window that has a sidebar to bring out and isn't private, and a thread of any Space needing
    /// the user. A fixed place, under where the window's controls show on hover.
    pub(super) fn edge_knot_top(&self) -> Option<f32> {
        let shows = self.sidebar_hidden
            && self.library.is_none()
            && self.rim_shown() == 0.
            && !self.reveal_shown()
            && !self.page_fullscreen
            && !self.private
            && !self.waiting_order.is_empty();
        shows.then_some(TOP)
    }

    /// Shows the knot's window where it goes, or hides it, as the frame drawn now has it. Only a
    /// change reaches the host, so a still window asks nothing of it.
    pub(super) fn place_edge_knot(&mut self, cx: &mut Context<Self>) {
        // The host finds the window's edge as it shows it, so a resize or the sidebar changing
        // sides shows it again.
        let at =
            (self.edge_knot_top()).map(|top| (top, f32::from(self.viewport.width), self.right));
        if at == self.edge_knot_at {
            return;
        }
        self.edge_knot_since = match (at, self.edge_knot_at) {
            (None, _) => None,
            (Some(_), None) => Some(self.clock.now()),
            (Some(_), Some(_)) => self.edge_knot_since,
        };
        self.edge_knot_at = at;
        match at {
            Some((top, ..)) => self.browser.show_edge_knot(top, WIDTH, HEIGHT, cx),
            None => self.browser.hide_edge_knot(cx),
        }
    }

    /// Whether the knot's window shows, for its window to draw only then.
    pub fn edge_knot_shown(&self) -> bool {
        self.edge_knot_at.is_some()
    }

    /// What the knot's drawing reads, so its window draws again only as that changes.
    pub fn edge_knot_look(&self) -> (bool, bool, crate::palette::Palette, Option<SharedString>) {
        (self.right, self.a11y, self.palette, self.edge_knot_said())
    }

    /// What the knot's window shows: the tab, flush with the window's edge, its other corners
    /// rounded, in the floating fill with its rim inside, its shadow in the room round it, and the
    /// knot still in it until the knot's own drawing is in. The window is the tab and that room on
    /// each side but the edge's.
    pub fn render_edge_knot(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Dark in either look, as the board draws it in a light window: it lies on the site's page,
        // not on the window's own colours, so it stands out from a page of any colour the same way.
        let fill = rgba(FILL);
        let rim = rgba(RIM);
        let palette = Palette {
            ink: Tone::Dark.ink(),
            ..self.palette
        };
        let ink = Rgba::from(palette.text(1.));
        let hovered = Rgba {
            a: fill.a,
            ..over(fill, ink, INK_HOVER_MIX)
        };
        let shadow = BoxShadow {
            color: rgba(SHADOW).into(),
            offset: point(px(0.), px(SHADOW_Y)),
            blur_radius: px(crate::elevation::sigma(SHADOW_BLUR)),
            spread_radius: px(0.),
            inset: false,
        };
        let said = self.edge_knot_said();
        // Where the tab is in the browser window's content, for the tooltip beside it.
        let x = match self.right {
            true => f32::from(self.viewport.width) - WIDTH,
            false => 0.,
        };
        let frame = Bounds::new(point(px(x), px(TOP)), size(px(WIDTH), px(HEIGHT)));
        let tab = div()
            .id("edge-knot")
            .when(self.a11y, |tab| {
                (tab.button_named(EDGE_KNOT_NAME))
                    .when_some(said.clone(), |tab, said| tab.aria_description(said))
            })
            .when_some(said, |tab, said| {
                tab.on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                    let tip = hovered.then(|| {
                        let anchor = Anchor::beside(frame, shell.viewport, shell.right);
                        (Tip::named(said.clone()), anchor)
                    });
                    shell.hover_tip(tip, cx);
                }))
            })
            .absolute()
            .top(px(MARGIN))
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .bg(fill)
            .hover(move |tab| tab.bg(hovered))
            .border(px(1.))
            .border_color(rim)
            .shadow(vec![shadow])
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    shell.edge_knot_pressed(cx);
                }),
            )
            .child(crate::product::mark(MARK, &palette));
        // Square and rimless where it meets the window's edge, and the knot as far past the middle
        // the other way.
        let tab = match self.right {
            true => (tab.right_0().border_r(px(0.)).rounded_l(px(RADIUS))).pr(px(2. * MARK_NUDGE)),
            false => (tab.left_0().border_l(px(0.)).rounded_r(px(RADIUS))).pl(px(2. * MARK_NUDGE)),
        };
        div().size_full().child(tab).into_any_element()
    }

    /// A press on the knot brings the sidebar out over the page, where the thread waiting is in
    /// whichever Space it shows, and does nothing more: unless it came by itself less than
    /// `PRESS_GUARD` ago. The keyboard goes on to that thread's line once the ring goes into the
    /// sidebar brought out (the one-line thread's slices).
    pub(super) fn edge_knot_pressed(&mut self, cx: &mut Context<Self>) {
        let now: Instant = self.clock.now();
        let Some(since) = self.edge_knot_since else {
            return;
        };
        if now.duration_since(since) < PRESS_GUARD {
            return;
        }
        crate::tooltip::dismiss(cx);
        self.set_revealed(true, cx);
    }
}
