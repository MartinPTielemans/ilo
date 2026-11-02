//! The agent's pointer over the page (ours, #365): as an agent clicks or types in a page the window
//! shows, in its own Space, its pointer glides to the element with its Space's name beside it, and
//! a Signal ring goes round the element with the ref the agent named it by. The window over the
//! page draws it (`Shell::over_page`), which the mouse goes through, so the user's own pointer and
//! clicks are never in its way, and it goes a moment after the agent's last action.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, Context, Pixels, Point, SharedString, Task, Window, div, point,
    prelude::*, px, rgb, svg,
};

use super::{Shell, panes::pane_frames};
use crate::tokens::shell::agent_pointer::{
    COLOR, LABEL_HEIGHT, LABEL_INSET, LABEL_SIZE, POINTER, RING, RING_OUTSET, RING_RADIUS,
    TAG_HEIGHT, TAG_INSET, TAG_RADIUS, TAG_SIZE, TEXT,
};

/// How long the pointer takes to glide to the element, and the ring to come in once it's there,
/// in seconds.
const GLIDE: f32 = 0.26;
const RING_IN: f32 = 0.12;
/// How long it stays after it's there, and how long it then takes to fade.
const HOLD: f32 = 1.6;
const FADE: f32 = 0.2;
/// Where the pointer comes from when it wasn't over the page: up and left of the element.
const ARRIVAL: f32 = 36.;
/// Lucide's mouse-pointer-2 has its tip this far into its box, across and down.
const TIP: f32 = 4. / 24.;

/// The agent's pointer, at an element in a tab's page.
pub(super) struct AgentPointer {
    tab: u64,
    tag: SharedString,
    /// Where it glides from and the element's box, in points from the page's top left.
    from: Point<Pixels>,
    element: Bounds<Pixels>,
    since: Instant,
    /// Wakes the shell as it starts to fade, and lets it go once it has.
    _going: Task<()>,
}

impl AgentPointer {
    /// Where its tip is `t` seconds in, from the page's top left.
    fn tip(&self, t: f32, still: bool) -> Point<Pixels> {
        let to = self.element.center();
        let way = if still { 1. } else { ease_out(t / GLIDE) };
        self.from + (to - self.from) * way
    }
}

impl Shell {
    /// The agent at work in `tab` is about to click or type in the element it named `tag`, whose
    /// box is `element`, in points from the top left of the tab's page. Only a page the window
    /// shows in the agent's Space has the pointer.
    pub fn agent_pointing(
        &mut self,
        tab: u64,
        tag: SharedString,
        element: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.agent_page_shows(tab) {
            return;
        }
        // From where it is if it's still over the page, else from up and left of the element.
        let still = cx.reduce_motion();
        let from = (self.agent_pointer.as_ref())
            .filter(|pointer| pointer.tab == tab && self.agent_pointer_shown())
            .map(|pointer| pointer.tip(pointer.since.elapsed().as_secs_f32(), still))
            .unwrap_or_else(|| element.center() - point(px(ARRIVAL), px(ARRIVAL)));
        // check.sh reads this line.
        log!(
            "browser: agent pointer {tab} {tag} at {} {} {} {}",
            f32::from(element.origin.x),
            f32::from(element.origin.y),
            f32::from(element.size.width),
            f32::from(element.size.height)
        );
        let going = cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(Duration::from_secs_f32(GLIDE + HOLD)).await;
            if shell.update(cx, |_, cx| cx.notify()).is_err() {
                return;
            }
            cx.background_executor().timer(Duration::from_secs_f32(FADE)).await;
            shell
                .update(cx, |shell, cx| {
                    shell.agent_pointer = None;
                    log!("browser: agent pointer gone");
                    cx.notify();
                })
                .ok();
        });
        let since = Instant::now();
        self.agent_pointer = Some(AgentPointer { tab, tag, from, element, since, _going: going });
        cx.notify();
    }

    /// Whether the agent's pointer is over the page the window shows.
    pub(super) fn agent_pointer_shown(&self) -> bool {
        self.agent_pointer.as_ref().is_some_and(|pointer| {
            self.agent_page_shows(pointer.tab)
                && pointer.since.elapsed().as_secs_f32() < GLIDE + HOLD + FADE
        })
    }

    /// What the window over the page draws: Add Split View's targets while a tab or a link is
    /// dragged over it, and the agent's pointer.
    pub fn over_page(&self, window: &mut Window, cx: &App) -> AnyElement {
        div()
            .size_full()
            .relative()
            .child(self.drop_targets(window))
            .children(self.agent_pointer_drawn(window, cx))
            .into_any_element()
    }

    /// The pane `tab`'s page is in, in the window.
    fn pane_of(&self, tab: u64) -> Option<Bounds<Pixels>> {
        if let Some((watched, place)) = self.task_page_place() {
            return (watched == tab).then_some(place);
        }
        let (frame, _) = self.page_frame(self.viewport);
        let panes = self.panes(self.selected);
        let index = panes.iter().position(|&pane| pane == tab)?;
        let (stacked, shares) = (self.stacked(self.selected), self.shares(self.selected));
        pane_frames(frame, panes.len(), stacked, &shares).get(index).copied()
    }

    fn agent_pointer_drawn(&self, window: &mut Window, cx: &App) -> Option<AnyElement> {
        if !self.agent_pointer_shown() {
            return None;
        }
        let pointer = self.agent_pointer.as_ref()?;
        let pane = self.pane_of(pointer.tab)?;
        let name = self.agent_page().map(|(space, _)| space.agent.clone()).unwrap_or_default();
        let t = pointer.since.elapsed().as_secs_f32();
        let still = cx.reduce_motion();
        // Frames only while it moves: the glide, the ring coming in and the fade.
        if t < GLIDE + RING_IN || t > GLIDE + HOLD {
            window.request_animation_frame();
        }
        let ring_in = if still { 1. } else { ((t - GLIDE) / RING_IN).clamp(0., 1.) };
        let shown = 1. - ((t - GLIDE - HOLD) / FADE).clamp(0., 1.);
        let tip = pointer.tip(t, still);
        let ring = pointer.element.dilate(px(RING_OUTSET));
        // The tag sits on the ring's top left, or under the ring where the page's top cuts it.
        let tag_top = match ring.top() - px(TAG_HEIGHT) < px(0.) {
            true => ring.bottom() + px(2.),
            false => ring.top() - px(TAG_HEIGHT) - px(2.),
        };
        let chip = |text: SharedString, size: f32, height: f32, inset: f32, radius: f32| {
            div()
                .absolute()
                .h(px(height))
                .px(px(inset))
                .flex()
                .items_center()
                .rounded(px(radius))
                .bg(rgb(COLOR))
                .text_size(px(size))
                .text_color(rgb(TEXT))
                .font_weight(gpui::FontWeight::MEDIUM)
                .whitespace_nowrap()
                .child(text)
        };
        let pointer_box = px(POINTER);
        let page = div()
            .absolute()
            .left(pane.left())
            .top(pane.top())
            .w(pane.size.width)
            .h(pane.size.height)
            .overflow_hidden()
            .opacity(shown)
            .child(
                div()
                    .absolute()
                    .left(ring.left())
                    .top(ring.top())
                    .w(ring.size.width)
                    .h(ring.size.height)
                    .rounded(px(RING_RADIUS))
                    .border(px(RING))
                    .border_color(rgb(COLOR))
                    .opacity(ring_in),
            )
            .child(
                chip(pointer.tag.clone(), TAG_SIZE, TAG_HEIGHT, TAG_INSET, TAG_RADIUS)
                    .left(ring.left())
                    .top(tag_top)
                    .opacity(ring_in),
            )
            .child(
                svg()
                    .absolute()
                    .left(tip.x - pointer_box * TIP)
                    .top(tip.y - pointer_box * TIP)
                    .size(pointer_box)
                    .path("icons/mouse-pointer-2.svg")
                    .text_color(rgb(COLOR)),
            )
            .when(!name.is_empty(), |page| {
                page.child(
                    chip(name, LABEL_SIZE, LABEL_HEIGHT, LABEL_INSET, LABEL_HEIGHT / 2.)
                        .left(tip.x + pointer_box * 0.75)
                        .top(tip.y + pointer_box * 0.75),
                )
            });
        Some(page.into_any_element())
    }
}

fn ease_out(progress: f32) -> f32 {
    1. - (1. - progress.clamp(0., 1.)).powi(3)
}
