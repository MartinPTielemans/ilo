//! Past the last Space, fingers pull its page left and bring a circle out from under the page's
//! edge, with a glow, as a ring round it closes. Once it closes the circle readies a new Space and
//! the trackpad clicks, as it does again if the fingers go back; letting go readied makes the
//! Space, as New Space… does (Arc 1.170 on video; docs/ux-spec.md › Spaces).

use std::f32::consts::PI;
use web_time::Instant;

use gpui::{
    Bounds, BoxShadow, Context, Hsla, PathBuilder, Window,
    accesskit::{Live, Role},
    canvas, div, fill, hsla, linear_color_stop, linear_gradient, point,
    prelude::*,
    px, size,
};

use super::{Command, Shell, Swipe};

/// How far fingers pull the last Space's page past its place to ready a new Space, and by how far
/// the ring has closed (Arc on video: the fingers 333 pt across).
const NEW_SPACE_READY: f32 = 22.7;
const RING_FULL: f32 = 22.;
use crate::tokens::shell::new_space::CIRCLE;
use crate::tokens::shell::new_space::CUT;
use crate::tokens::shell::new_space::RING;
/// The circle's centre at rest, right of the page's edge, how much faster than the page it comes
/// out, and how far left of the edge it stops, while the page goes on.
const CIRCLE_START: f32 = 20.;
const CIRCLE_SPEED: f32 = 3.1;
const CIRCLE_INSET: f32 = 27.;
const TRAVEL: f32 = CIRCLE_START + CIRCLE_INSET;
use crate::tokens::shell::new_space::PLUS;
use crate::tokens::shell::new_space::PLUS_STROKE;
use crate::tokens::shell::new_space::WHITE_TINT;
/// Readying, the white closes in on the colour to a dot over 60 ms, which shrinks into a + over
/// 70 ms more, as the circle pops to 1.13× and back over 120 ms from 40 ms in.
const CLOSE: f32 = 0.06;
use crate::tokens::shell::new_space::DOT;
const MORPH: f32 = 0.07;
const POP_AT: f32 = 0.04;
const POP: f32 = 0.12;
const POP_SCALE: f32 = 1.13;
/// Going back, the colour grows out from the middle over 120 ms.
const UNREADY: f32 = 0.12;
/// Let go readied, the circle goes back under the edge over 100 ms.
const EXIT: f32 = 0.1;
use crate::tokens::shell::new_space::GLOW;
use crate::tokens::shell::new_space::GLOW_STRIP;
use crate::tokens::shell::new_space::GLOW_WIDTH;
use crate::tokens::shell::new_space::SHADOW;

/// The circle going back under the page's edge after a pull made `space`, and where the fingers
/// left the page they pulled.
pub(super) struct CircleExit {
    space: u64,
    accent: (Hsla, Hsla),
    start: Instant,
    held: f32,
}

/// What the circle shows.
#[derive(Clone, Copy)]
enum Look {
    /// The ring `way` of the way round.
    Pulling(f32),
    /// This long since it readied.
    Readying(f32),
    /// This long since it stopped being ready, going back to the ring `way` of the way round.
    Unreadying(f32, f32),
}

impl Shell {
    /// Whether `swipe` has pulled the last Space's page far enough to ready a new Space.
    fn pull_ready(&self, swipe: &Swipe) -> bool {
        let beside = self.new_space_beside(swipe.from);
        beside && !swipe.library && self.swiped_to(swipe) <= -NEW_SPACE_READY
    }

    /// Whether a new Space comes after `space`: the last, while no form is naming one. A private
    /// window makes none.
    pub(super) fn new_space_beside(&self, space: u64) -> bool {
        let last = self.spaces.last().is_some_and(|last| last.id == space);
        last && !self.one_space() && self.form.is_none()
    }

    /// Readies a new Space as the fingers' pull reaches it and lets it go as they go back, with a
    /// detent each way.
    pub(super) fn ready_new_space(&mut self, swipe: &mut Swipe, cx: &mut Context<Self>) {
        let ready = self.pull_ready(swipe);
        if ready == swipe.ready {
            return;
        }
        swipe.ready = ready;
        swipe.turned = Some(Instant::now());
        // check.sh waits for these lines.
        log!(
            "browser: new space {}",
            if ready { "ready" } else { "unready" }
        );
        self.browser.detent(cx);
    }

    /// Makes the Space the fingers readied and shows it at once, with the form for naming it, as
    /// the circle goes back.
    pub(super) fn make_pulled_space(&mut self, held: f32, cx: &mut Context<Self>) {
        let accent = self.circle_accent(self.space);
        self.run(Command::NewSpace, "swipe", cx);
        if let Some(form) = &self.form {
            let (space, start) = (form.space, crate::motion::start(cx));
            self.circle_exit = Some(CircleExit {
                space,
                accent,
                start,
                held,
            });
        }
    }

    /// `space` shows. Returns whether a pull made it, which then shows where it is rather than
    /// sliding in. Another Space shown ends the circle going back.
    pub(super) fn pulled_space_shown(&mut self, space: u64) -> bool {
        self.circle_exit.take_if(|exit| exit.space != space);
        self.circle_exit.is_some()
    }

    /// Where the page a pull made a Space from stays, as the fingers left it, until that Space
    /// shows, while the circle goes back.
    pub(super) fn held_page(&self) -> Option<f32> {
        let exit = self.circle_exit.as_ref()?;
        let going = exit.start.elapsed().as_secs_f32() < EXIT;
        (exit.space != self.space && going).then_some(exit.held)
    }

    /// Asks for frames while the circle readies or goes back.
    pub(super) fn step_new_space(&mut self, window: &mut Window) {
        let turned = self.swipe.as_ref().and_then(|swipe| swipe.turned);
        let turning = (CLOSE + MORPH).max(POP_AT + POP).max(UNREADY);
        if turned.is_some_and(|at| at.elapsed().as_secs_f32() < turning) {
            window.request_animation_frame();
        }
        if let Some(exit) = &self.circle_exit {
            if exit.start.elapsed().as_secs_f32() < EXIT {
                window.request_animation_frame();
            } else if exit.space == self.space {
                self.circle_exit = None;
            }
        }
    }

    /// The last Space and how far past its place its page is, while fingers or its settle have
    /// it there.
    fn pulled(&self) -> Option<(u64, f32)> {
        if self.form().is_some() || self.form_going.is_some() {
            return None;
        }
        let (space, x) = *self.pages().first()?;
        let last = self.spaces.last().is_some_and(|last| last.id == space);
        (last && x < 0.).then_some((space, -x))
    }

    /// The circle's colour, `space`'s accent, and its label's on it.
    fn circle_accent(&self, space: u64) -> (Hsla, Hsla) {
        let palette = self.palette_of(space);
        (palette.accent.into(), palette.accent_label.into())
    }

    /// The circle halfway down the page's edge and its glow, while the last Space's page is pulled
    /// and as the circle goes back after making a Space.
    pub(super) fn new_space_circle(&self, cx: &gpui::App) -> Option<impl IntoElement + use<>> {
        let (travel, accent, look) = match &self.circle_exit {
            Some(exit) => {
                let t = exit.start.elapsed().as_secs_f32() / EXIT;
                if t >= 1. {
                    return None;
                }
                (
                    TRAVEL * (1. - smoothstep(t)),
                    exit.accent,
                    Look::Readying(1.),
                )
            }
            None => {
                let (space, pulled) = self.pulled()?;
                let way = (pulled / RING_FULL).min(1.);
                let swipe = self.swipe.as_ref();
                let since = swipe.and_then(|swipe| swipe.turned);
                let since = since.map(|at| at.elapsed().as_secs_f32());
                let look = match (swipe.is_some_and(|swipe| swipe.ready), since) {
                    (true, Some(t)) => Look::Readying(t),
                    (false, Some(t)) if t < UNREADY => Look::Unreadying(t, way),
                    _ => Look::Pulling(way),
                };
                (
                    (CIRCLE_SPEED * pulled).min(TRAVEL),
                    self.circle_accent(space),
                    look,
                )
            }
        };
        let (accent, label) = accent;
        let white = label.blend(accent.opacity(WHITE_TINT));
        let popped = match look {
            Look::Readying(t) => (PI * ((t - POP_AT) / POP).clamp(0., 1.)).sin(),
            _ => 0.,
        };
        let scale = crate::motion::scale(1. + (POP_SCALE - 1.) * popped, cx);
        let across = CIRCLE * scale;
        let disc = div()
            .relative()
            .size(px(across))
            .rounded_full()
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., SHADOW),
                offset: point(px(0.), px(1.)),
                blur_radius: px(4.),
                spread_radius: px(0.),
                inset: false,
            }]);
        let inner = CIRCLE - 2. * (RING + CUT);
        let disc = match look {
            Look::Pulling(way) => disc
                .bg(accent)
                .child(ring(way, white, Some(self.window_fill())))
                .children(plus(across, PLUS, PLUS_STROKE, white)),
            // The colour grows back out under the white, which thins to the ring.
            Look::Unreadying(t, way) => {
                let grown = CIRCLE * smoothstep(t / UNREADY);
                let cut = (grown >= inner).then(|| self.window_fill());
                disc.bg(white)
                    .child(dot(across, grown, accent))
                    .child(ring(way, white, cut))
                    .children(plus(across, PLUS, PLUS_STROKE, white))
            }
            // The white closes in on the colour, and the dot left shrinks into a +.
            Look::Readying(t) if t < CLOSE => {
                let closed = inner + (DOT - inner) * ease_out(t / CLOSE);
                disc.bg(white).child(dot(across, closed * scale, accent))
            }
            Look::Readying(t) => {
                let shrunk = DOT + (PLUS_STROKE - DOT) * ease_out((t - CLOSE) / MORPH);
                disc.bg(white)
                    .child(dot(across, shrunk * scale, accent))
                    .children(plus(across, PLUS * scale, PLUS_STROKE * scale, accent))
            }
        };
        // VoiceOver says when letting go makes the Space, as the trackpad's click does, while the
        // fingers pull, and nothing once it's made.
        let said = match look {
            _ if self.circle_exit.is_some() => None,
            Look::Readying(_) => Some("Let go to make it"),
            _ => Some("Pull further to make one"),
        };
        let disc =
            disc.id("new-space-circle")
                .when_some(said.filter(|_| self.a11y), |disc, said| {
                    disc.role(Role::Status)
                        .aria_label("New Space")
                        .aria_value(said)
                        .a11y_synthetic_children(|builder| {
                            builder.parent_node().set_live(Live::Polite)
                        })
                });
        let width = self.width();
        let centre = width + CIRCLE_START - travel;
        Some(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(width))
                .overflow_hidden()
                .child(glow(width, travel / TRAVEL))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(centre - across / 2.))
                        .w(px(across))
                        .flex()
                        .items_center()
                        .child(disc),
                ),
        )
    }
}

/// The white ring over the disc's edge, from 12 o'clock anticlockwise `way` of the way round with
/// round ends, and inside it a line of `cut`, the sidebar's colour.
fn ring(way: f32, white: Hsla, cut: Option<Hsla>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            if way <= 0. {
                return;
            }
            let scale = f32::from(bounds.size.width) / CIRCLE;
            let centre = bounds.center();
            let mut arc = |radius: f32, stroke: f32, color: Hsla| {
                let radius = px(radius * scale);
                let at = |way: f32| {
                    let angle = 2. * PI * way;
                    centre + point(-radius * angle.sin(), -radius * angle.cos())
                };
                let mut path = PathBuilder::stroke(px(stroke * scale));
                path.move_to(at(0.));
                // Half the way round or less at a time, so a full ring closes.
                let mut done = 0.;
                while done < way {
                    done += (way - done).min(0.5);
                    path.arc_to(point(radius, radius), px(0.), false, false, at(done));
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
                if way < 1. {
                    let end = px(stroke * scale);
                    for at in [at(0.), at(way)] {
                        let bounds = Bounds::centered_at(at, size(end, end));
                        window.paint_quad(fill(bounds, color).corner_radii(end / 2.));
                    }
                }
            };
            if let Some(cut) = cut {
                arc((CIRCLE - RING - CUT) / 2., RING + CUT, cut);
            }
            arc((CIRCLE - RING) / 2., RING, white);
        },
    )
    .absolute()
    .size_full()
}

/// A dot `across` wide, centred on a disc `disc` wide.
fn dot(disc: f32, across: f32, color: Hsla) -> impl IntoElement {
    div()
        .absolute()
        .left(px((disc - across) / 2.))
        .top(px((disc - across) / 2.))
        .size(px(across))
        .rounded_full()
        .bg(color)
}

/// A + `arms` across with arms `stroke` thick and round ends, centred on a disc `across` wide.
fn plus(across: f32, arms: f32, stroke: f32, color: Hsla) -> [impl IntoElement; 2] {
    let bar = |bar: gpui::Size<f32>| {
        div()
            .absolute()
            .left(px((across - bar.width) / 2.))
            .top(px((across - bar.height) / 2.))
            .w(px(bar.width))
            .h(px(bar.height))
            .rounded(px(stroke / 2.))
            .bg(color)
    };
    [bar(size(arms, stroke)), bar(size(stroke, arms))]
}

/// The glow that comes out with the circle, `out` of the way: white, brightest on the page's edge
/// (`edge` from the left) halfway down, fading to nothing GLOW_WIDTH left of it and at the top and
/// bottom. It slides out from under the edge as the circle does. A cone, drawn in strips across
/// as GPUI has no radial gradient.
fn glow(edge: f32, out: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let half = f32::from(bounds.size.height) / 2.;
            let middle = f32::from(bounds.origin.y) + half;
            let tip = f32::from(bounds.origin.x) + edge + GLOW_WIDTH * (1. - out);
            let mut from_middle = 0.;
            while from_middle < half {
                let down = (from_middle + GLOW_STRIP / 2.) / half;
                let reach = GLOW_WIDTH * (1. - down * down).max(0.).sqrt();
                let brightest = GLOW * (1. - down).max(0.);
                let shade = linear_gradient(
                    90.,
                    linear_color_stop(hsla(0., 0., 1., 0.), 0.),
                    linear_color_stop(hsla(0., 0., 1., brightest), 1.),
                );
                for top in [middle - from_middle - GLOW_STRIP, middle + from_middle] {
                    let strip = Bounds::new(
                        point(px(tip - reach), px(top)),
                        size(px(reach), px(GLOW_STRIP)),
                    );
                    window.paint_quad(fill(strip, shade));
                }
                from_middle += GLOW_STRIP;
            }
        },
    )
    .absolute()
    .size_full()
}

pub(super) fn ease_out(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
