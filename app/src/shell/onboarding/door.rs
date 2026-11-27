//! Other apps' door (first-run-v9 §8): under the two ways, a cord from each meets the knot, and one
//! drops from it to the card an app gets in the sidebar as it connects, with one line under it.
//! With nothing on, all of it is faint and the card is its outline, so it reads as the card to come.
//! A way turned on draws its cord, a bead runs on through the knot, and the card arrives reading a
//! page, then done; with the card there, a second way draws its cord and bead only.

use std::time::Instant;

use gpui::{
    AnyElement, Bounds, Context, Corners, FontWeight, PathBuilder, Pixels, Point, Rgba, Window,
    canvas, div, point, prelude::*, px, size,
};

use super::card::{self, Inks};
use super::{Shell, Step, arrive, cubic_bezier, lerp};
use crate::tokens::shell::onboarding::card as t;
use crate::tokens::shell::task::RING_TRACK;
use crate::tokens::shell::task::motion::RING_TURN;

/// The segments a cord is drawn in.
const SEGMENTS: usize = 32;

/// A cord or the card turned on or off, and when; `at` is None for one as it was when the step
/// came, which doesn't move.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Turn {
    on: bool,
    at: Option<Instant>,
}

/// The door's motion: each way's cord, the command's then MCP's, and the card.
#[derive(Default)]
pub(super) struct Door {
    cords: [Option<Turn>; 2],
    card: Option<Turn>,
}

impl Door {
    /// Follows the ways `on` as of `now`: what turned since the last frame moves from then.
    fn follow(&mut self, on: [bool; 2], now: Instant) {
        let follow = |kept: &mut Option<Turn>, on: bool| match kept {
            Some(turn) if turn.on == on => {}
            Some(turn) => *turn = Turn { on, at: Some(now) },
            None => *kept = Some(Turn { on, at: None }),
        };
        let [command, mcp] = &mut self.cords;
        follow(command, on[0]);
        follow(mcp, on[1]);
        follow(&mut self.card, on[0] || on[1]);
    }
}

/// What a frame of the door draws.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Scene {
    /// Each way's cord over its base: how much of it is drawn, and at what of the text's ink.
    cords: [(f32, f32); 2],
    /// The cord down to the card, the same.
    drop: (f32, f32),
    /// Each way's bead, how far along its run to the card, while it runs.
    beads: [Option<f32>; 2],
    /// The knot's ink.
    knot: f32,
    /// The card: 0 its outline, 1 arrived.
    card: f32,
    /// How far it has still to rise.
    rise: f32,
    /// The ring's turn while it reads.
    turned: f32,
    /// It has read its page, so the ring is a check.
    pub(super) read: bool,
    /// Something is still moving, so the card asks for another frame.
    pub(super) moving: bool,
}

/// The door as `door` has it at `now`; `still` (Reduce Motion) at rest at once.
fn scene(door: &Door, now: Instant, still: bool) -> Scene {
    let since = |turn: Option<Turn>| {
        let turn = turn.unwrap_or(Turn {
            on: false,
            at: None,
        });
        let secs = match (still, turn.at) {
            (false, Some(at)) => now.saturating_duration_since(at).as_secs_f32(),
            _ => f32::INFINITY,
        };
        (turn.on, secs)
    };
    let unit = |x: f32| x.clamp(0., 1.);
    let [bead_start, bead_length] = t::DOOR_BEAD;
    let [drop_start, drop_length] = t::DOOR_DROP;
    let [arrival, fade, rising, rise] = t::DOOR_ARRIVE;
    let cord = |(on, secs): (bool, f32)| match on {
        true => (drawing(secs / t::DOOR_DRAW), 1.),
        false => (1., 1. - unit(secs / t::DOOR_FALL)),
    };
    let bead = |(on, secs): (bool, f32)| {
        let along = (secs - bead_start) / bead_length;
        (on && (0. ..1.).contains(&along)).then_some(along)
    };
    let ways = [since(door.cords[0]), since(door.cords[1])];
    let (on, secs) = since(door.card);
    let arrived = secs - arrival;
    let moving = (ways.iter()).any(|&(on, secs)| match on {
        true => secs < bead_start + bead_length,
        false => secs < t::DOOR_FALL,
    }) || match on {
        true => arrived < t::DOOR_READ,
        false => secs < fade.max(t::DOOR_FALL).max(t::DOOR_KNOT_FADE),
    };
    Scene {
        cords: ways.map(cord),
        drop: match on {
            true => (unit((secs - drop_start) / drop_length), 1.),
            false => (1., 1. - unit(secs / t::DOOR_FALL)),
        },
        beads: ways.map(bead),
        knot: match on {
            true => lerp(
                t::DOOR_OFF,
                1.,
                unit((secs - t::DOOR_DRAW) / t::DOOR_KNOT_FADE),
            ),
            false => lerp(1., t::DOOR_OFF, unit(secs / t::DOOR_KNOT_FADE)),
        },
        card: match on {
            true => unit(arrived / fade),
            false => 1. - unit(secs / fade),
        },
        rise: match on {
            true => rise * (1. - arrive(arrived / rising)),
            false => 0.,
        },
        turned: match secs.is_finite() {
            true => (secs / RING_TURN).fract(),
            false => 0.,
        },
        read: on && arrived >= t::DOOR_READ,
        moving,
    }
}

/// DRAW, the curve a cord drawing takes.
fn drawing(x: f32) -> f32 {
    let [x1, y1, x2, y2] = t::DRAW;
    cubic_bezier(x1, y1, x2, y2, x.clamp(0., 1.))
}

/// The point `at` of the way along the curve from `a` to `d` over `b` and `c`.
fn bezier(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32), at: f32) -> (f32, f32) {
    let rest = 1. - at;
    let weights = [
        rest * rest * rest,
        3. * rest * rest * at,
        3. * rest * at * at,
        at * at * at,
    ];
    let points = [a, b, c, d];
    let sum = |axis: fn(&(f32, f32)) -> f32| {
        (points.iter().map(axis).zip(weights))
            .map(|(p, w)| p * w)
            .sum()
    };
    (sum(|p| p.0), sum(|p| p.1))
}

/// The door's cords on the card, `grown` lower while the ways are taller: each way's from its
/// card's bottom centre into the knot's top, and the one down from the knot to the card.
pub(super) fn cords(grown: f32) -> ([Vec<(f32, f32)>; 2], [(f32, f32); 2]) {
    let [top, width, height, gap, _] = t::WAYS;
    let [centre, knot] = t::DOOR_KNOT;
    let [below, above, inside] = t::DOOR_BEND;
    let x = t::WIDTH / 2.;
    let bottom = top + height + grown;
    let end = centre + grown - knot / 2. + inside;
    let left = (t::WIDTH - 2. * width - gap) / 2. + width / 2.;
    let cord = |from: f32| -> Vec<(f32, f32)> {
        (0..=SEGMENTS)
            .map(|each| {
                let at = each as f32 / SEGMENTS as f32;
                bezier(
                    (from, bottom),
                    (from, bottom + below),
                    (x, end - above),
                    (x, end),
                    at,
                )
            })
            .collect()
    };
    let down = [
        (x, centre + grown + knot / 2. - inside),
        (x, t::DOOR_CARD[0] + grown),
    ];
    ([cord(left), cord(left + width + gap)], down)
}

/// The part of the line through `points` that is `drawn` of its length.
fn part(points: &[(f32, f32)], drawn: f32) -> Vec<(f32, f32)> {
    let length = |a: (f32, f32), b: (f32, f32)| (b.0 - a.0).hypot(b.1 - a.1);
    let total: f32 = points.windows(2).map(|pair| length(pair[0], pair[1])).sum();
    let mut left = total * drawn.clamp(0., 1.);
    let mut kept = points.first().copied().into_iter().collect::<Vec<_>>();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let step = length(a, b);
        if left >= step {
            kept.push(b);
            left -= step;
            continue;
        }
        if step > 0. {
            let at = left / step;
            kept.push((lerp(a.0, b.0, at), lerp(a.1, b.1, at)));
        }
        break;
    }
    kept
}

/// Strokes the line through `points`, `width` wide, at `origin`.
fn stroke(
    origin: Point<Pixels>,
    points: &[(f32, f32)],
    width: f32,
    ink: Rgba,
    window: &mut Window,
) {
    if points.len() < 2 || ink.a <= 0. {
        return;
    }
    let at = |(x, y): (f32, f32)| point(origin.x + px(x), origin.y + px(y));
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(at(points[0]));
    for &each in &points[1..] {
        path.line_to(at(each));
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, ink);
    }
}

impl Shell {
    /// The door under the ways, `grown` lower while they're taller, with the ways `on` and the
    /// first program found, `first`, on its card.
    pub(super) fn other_apps_door(
        &mut self,
        on: [bool; 2],
        first: &'static str,
        grown: f32,
        inks: Inks,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let still = crate::motion::still(cx);
        let Some(onboarding) = self.onboarding.as_mut() else {
            return div().into_any_element();
        };
        let now = onboarding.clock.now();
        onboarding.door.follow(on, now);
        let scene = scene(&onboarding.door, now, still);
        if scene.moving {
            window.request_animation_frame();
        }
        let [base, faint, width, bead] = t::DOOR_CORD;
        let (ways, down) = cords(grown);
        let painted = scene.clone();
        let cords = canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let origin = bounds.origin;
                for way in &ways {
                    stroke(origin, way, base, inks.text(faint), window);
                }
                stroke(origin, &down, base, inks.text(faint), window);
                for (way, (drawn, ink)) in ways.iter().zip(painted.cords) {
                    stroke(origin, &part(way, drawn), width, inks.text(ink), window);
                }
                let (drawn, ink) = painted.drop;
                stroke(origin, &part(&down, drawn), width, inks.text(ink), window);
                for (way, along) in ways.iter().zip(painted.beads) {
                    let Some(along) = along else { continue };
                    let run: Vec<(f32, f32)> = way.iter().chain(&down).copied().collect();
                    let Some(&(x, y)) = part(&run, along).last() else {
                        continue;
                    };
                    let corner = point(origin.x + px(x - bead / 2.), origin.y + px(y - bead / 2.));
                    let dot = Bounds::new(corner, size(px(bead), px(bead)));
                    window.paint_quad(
                        gpui::fill(dot, inks.text(1.)).corner_radii(Corners::all(px(bead / 2.))),
                    );
                }
            },
        )
        .absolute()
        .inset_0();
        let [centre, knot] = t::DOOR_KNOT;
        let knot = card::at((t::WIDTH - knot) / 2., centre + grown - knot / 2.)
            .size(px(knot))
            .child(card::whole_knot(knot, inks.text(scene.knot)))
            .children(self.place(Step::OtherApps, "door knot"));
        let [top, wide, high, corners, side, gap] = t::DOOR_CARD;
        let [off, outline] = t::DOOR_OUTLINE;
        let [name_size, name_line, line_size, line_line] = t::DOOR_NAME;
        let [ring, ring_stroke] = t::DOOR_RING;
        let arrived = scene.card > 0. && on.contains(&true);
        let mark = match scene.read {
            true => card::check_disc(inks).into_any_element(),
            false => crate::shell::ring::ring(
                ring,
                ring_stroke,
                if arrived { scene.turned } else { 0. },
                inks.text(RING_TRACK).into(),
                inks.text(1.).into(),
            )
            .into_any_element(),
        };
        let line = arrived.then(|| match scene.read {
            true => "Island Ferries · €14 · 08:40",
            false => "Reading Island Ferries",
        });
        let fill = inks.group();
        let sidebar_card = card::at((t::WIDTH - wide) / 2., top + grown)
            .w(px(wide))
            .h(px(high))
            .rounded(px(corners))
            .bg(Rgba {
                a: fill.a * scene.card,
                ..fill
            })
            .border_1()
            .border_color(inks.text(lerp(off, outline, scene.card)))
            .child(
                div()
                    .relative()
                    .top(px(scene.rise))
                    .size_full()
                    .px(px(side))
                    .flex()
                    .items_center()
                    .gap(px(gap))
                    .opacity(lerp(t::DOOR_GHOST, 1., scene.card))
                    .child(mark)
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(name_size))
                                    .line_height(px(name_line))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(inks.text(1.))
                                    .child(first),
                            )
                            .children(line.map(|line| {
                                div()
                                    .text_size(px(line_size))
                                    .line_height(px(line_line))
                                    .text_color(inks.text_2())
                                    .whitespace_nowrap()
                                    .child(line)
                            })),
                    ),
            )
            .children(self.place(Step::OtherApps, "door"));
        let said = card::at(0., t::DOOR_LINE + grown)
            .w(px(t::WIDTH))
            .flex()
            .justify_center()
            .text_size(px(line_size))
            .line_height(px(line_line))
            .text_color(inks.text(t::TEXT_3))
            .child(
                div()
                    .relative()
                    .child("An app appears in your sidebar when it opens a page.")
                    .children(self.place(Step::OtherApps, "door line")),
            );
        // The door draws on the card's own coordinates, as v9 places it.
        let column = (t::WIDTH - t::CENTRED_COLUMN) / 2.;
        card::at(-column, -t::BODY_TOP)
            .w(px(t::WIDTH))
            .h(px(t::HEIGHT))
            .child(cords)
            .child(knot)
            .child(sidebar_card)
            .child(said)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each way's cord leaves its card's bottom centre and ends inside the knot's top; the one
    /// down leaves inside its bottom and ends at the card.
    #[test]
    fn the_cords_run_from_each_way_through_the_knot_to_the_card() {
        let ([command, mcp], [from, to]) = cords(0.);
        let near =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01;
        assert!(near(command[0], (214., 248.)));
        assert!(near(mcp[0], (566., 248.)));
        assert!(near(*command.last().unwrap(), (390., 274.)));
        assert!(near(*mcp.last().unwrap(), (390., 274.)));
        assert!(near(from, (390., 310.)));
        assert!(near(to, (390., 328.)));
        // Taller ways move it all down with them.
        let ([command, _], [_, to]) = cords(t::WAY_GROW);
        assert!(near(command[0], (214., 248. + t::WAY_GROW)));
        assert!(near(to, (390., 328. + t::WAY_GROW)));
    }

    /// A way turned on draws its cord, runs its bead, brings the card and then reads its page, and
    /// stops asking for frames; turned off, it falls back and the card is its outline again.
    #[test]
    fn a_way_on_brings_the_card_and_then_rests() {
        let start = Instant::now();
        let at = |secs: f32| start + std::time::Duration::from_secs_f32(secs);
        let mut door = Door::default();
        door.follow([false, false], start);
        let rest = scene(&door, start, false);
        assert_eq!(
            (rest.card, rest.knot, rest.moving),
            (0., t::DOOR_OFF, false)
        );
        door.follow([true, false], start);
        let drawing = scene(&door, at(t::DOOR_DRAW / 2.), false);
        assert!(drawing.cords[0].0 > 0. && drawing.cords[0].0 < 1. && drawing.moving);
        assert_eq!(drawing.card, 0.);
        let running = scene(&door, at(t::DOOR_BEAD[0] + 0.1), false);
        assert!(running.beads[0].is_some() && running.beads[1].is_none());
        let reading = scene(&door, at(t::DOOR_ARRIVE[0] + 0.5), false);
        assert_eq!((reading.card, reading.read), (1., false));
        let read = scene(&door, at(t::DOOR_ARRIVE[0] + t::DOOR_READ + 0.1), false);
        assert!(read.read && !read.moving && read.knot == 1.);
        // A second way draws its cord and bead only: the card stays read.
        door.follow([true, true], at(5.));
        let second = scene(&door, at(5.1), false);
        assert!(second.read && second.card == 1. && second.cords[1].0 < 1.);
        door.follow([false, false], at(10.));
        let gone = scene(&door, at(11.), false);
        assert_eq!(
            (gone.card, gone.cords[0].1, gone.read, gone.moving),
            (0., 0., false, false)
        );
    }

    /// With Reduce Motion, a way on is at rest at once, its card read.
    #[test]
    fn still_rests_at_once() {
        let now = Instant::now();
        let mut door = Door::default();
        door.follow([false, false], now);
        door.follow([false, true], now);
        let scene = scene(&door, now, true);
        assert!(scene.read && !scene.moving && scene.beads == [None, None]);
        assert_eq!(scene.cords[1], (1., 1.));
    }
}
