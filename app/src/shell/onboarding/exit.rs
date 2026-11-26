//! First run's exit (first-run v9 §10): on Start Browsing the card's content fades, then the card
//! grows into the browser window where the host grows first run's window, or else fades away, and
//! only at the exit's end does the browser window come (`Shell::onboarding_layer`).

use super::{Onboarding, arrive, ease_in_out, lerp, t};
use crate::tokens::shell::empty::MARK_SIZE;
use crate::tokens::shell::onboarding::exit as x;

/// What one frame of the exit draws, in its window's frame.
pub(super) struct Exit {
    /// The whole card's opacity.
    pub(super) card: f32,
    /// The step's content's, under the card's.
    pub(super) content: f32,
    /// Where the card's own frame, the tokens' size, sits centred in the window: its top left.
    /// The content stays there as it fades.
    pub(super) offset: (f32, f32),
    /// How far the card's fill has crossed to the window's tint, 0 to 1.
    pub(super) tint: f32,
    /// The card's frame and corners: x, y, width, height, radius.
    pub(super) plate: (f32, f32, f32, f32, f32),
    /// The last step's knot, drawn over the fading content so it stays as the rest goes: its
    /// size and where it is. None in the cross-fade, where the knot fades with the card.
    pub(super) knot: Option<(f32, f32, f32)>,
}

impl Onboarding {
    /// Seconds into the exit, if it has started. Its clock waits for the first frame the window
    /// reports visible, so none of it plays unseen.
    pub(super) fn exit_clock(&mut self, visible: bool) -> Option<f32> {
        let at = self.ending?;
        if !visible {
            self.ending = Some(self.clock.now());
            return Some(0.);
        }
        Some(self.secs_since(at))
    }

    /// The exit's whole length: the content's fade and the grow where the host grew the window,
    /// else the cross-fade.
    pub(super) fn exit_length(&self, still: bool) -> f32 {
        match !still && self.grows == Some(true) {
            true => x::CONTENT + x::GROW,
            false => x::CROSS_FADE,
        }
    }
}

/// The exit's frame `at` seconds in, in a window `size` across on this frame, as the host may
/// have grown it since the last. The card stays centred in it throughout, and grows from its
/// tokens' size to the window's. `landing` is where the empty page's mark will be in the browser
/// window, which a grown window's frame is.
pub(super) fn frame(
    at: f32,
    still: bool,
    grows: Option<bool>,
    size: (f32, f32),
    landing: Option<(f32, f32)>,
) -> Exit {
    let (width, height) = size;
    let offset = ((width - t::WIDTH) / 2., (height - t::HEIGHT) / 2.);
    if still || grows != Some(true) {
        return Exit {
            card: 1. - (at / x::CROSS_FADE).min(1.),
            content: 1.,
            offset,
            tint: 0.,
            plate: (offset.0, offset.1, t::WIDTH, t::HEIGHT, t::RADIUS),
            knot: None,
        };
    }
    let [knot, knot_top] = t::DONE_KNOT;
    let knot_at = (offset.0 + (t::WIDTH - knot) / 2., offset.1 + knot_top);
    let content = 1. - (at / x::CONTENT).min(1.);
    let since = (at - x::CONTENT).max(0.);
    let grown = arrive(since / x::GROW);
    let [from_radius, to_radius] = x::CORNERS;
    let (w, h) = (lerp(t::WIDTH, width, grown), lerp(t::HEIGHT, height, grown));
    let to = landing.unwrap_or(((width - MARK_SIZE) / 2., (height - MARK_SIZE) / 2.));
    let travelled = arrive(since / x::KNOT);
    Exit {
        card: 1.,
        content,
        offset,
        tint: ease_in_out(since / x::MATERIAL),
        plate: (
            (width - w) / 2.,
            (height - h) / 2.,
            w,
            h,
            lerp(from_radius, to_radius, grown),
        ),
        knot: Some((
            lerp(knot, MARK_SIZE, travelled),
            lerp(knot_at.0, to.0, travelled),
            lerp(knot_at.1, to.1, travelled),
        )),
    }
}

/// `from` crossed `at` of the way to `to`, each channel and its alpha.
pub(super) fn crossed(from: gpui::Rgba, to: gpui::Rgba, at: f32) -> gpui::Rgba {
    gpui::Rgba {
        r: lerp(from.r, to.r, at),
        g: lerp(from.g, to.g, at),
        b: lerp(from.b, to.b, at),
        a: lerp(from.a, to.a, at),
    }
}
