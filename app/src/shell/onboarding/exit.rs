//! First run's exit (first-run v9 §10): on Start Browsing the card's content fades, its knot with
//! it, then the card grows plain into the browser window where the host grows first run's window,
//! or else fades away, and only at the exit's end does the browser window come
//! (`Shell::onboarding_layer`).

use super::{Onboarding, arrive, ease_in_out, lerp, t};
use crate::tokens::shell::onboarding::exit as x;

#[cfg(test)]
thread_local! {
    /// What each frame of the layer drew, in order: the card's plate, its fill and the shape
    /// the host's glass has on that frame, none on a frame that drew no card.
    pub(super) static DRAWN: std::cell::RefCell<Vec<Option<(Plate, gpui::Rgba, (f32, f32, f32))>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The card's frame and corners: x, y, width, height, radius.
pub(super) type Plate = (f32, f32, f32, f32, f32);

/// What one frame of the exit draws, in its window's frame.
pub(super) struct Exit {
    /// The whole card's opacity.
    pub(super) card: f32,
    /// The step's content's, under the card's.
    pub(super) content: f32,
    /// Where the card's own frame, the tokens' size, sits centred in the window: its top left.
    /// The content stays there as it fades.
    pub(super) offset: (f32, f32),
    /// How far the card's fill has crossed to the window's under a page, 0 to 1.
    pub(super) tint: f32,
    pub(super) plate: Plate,
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
        match self.grown(still) {
            true => x::CONTENT + x::GROW,
            false => x::CROSS_FADE,
        }
    }

    /// Whether the exit grows the card to its window: the host said it grows the window, and
    /// Reduce Motion is off.
    pub(super) fn grown(&self, still: bool) -> bool {
        !still && self.grows == Some(true)
    }
}

/// The plate of a card drawn in `shape`, its width, height and radius, centred in a window `size`
/// across: the exit's card in the shape its glass was given on the frame before.
pub(super) fn behind(shape: (f32, f32, f32), size: (f32, f32)) -> Plate {
    let (w, h, radius) = shape;
    ((size.0 - w) / 2., (size.1 - h) / 2., w, h, radius)
}

/// The exit's frame `at` seconds in, in a window `size` across on this frame, as the host may
/// have grown it since the last. The card stays centred in it throughout, and where it's
/// `grown` it grows from its tokens' size to the window's.
pub(super) fn frame(at: f32, grown: bool, size: (f32, f32)) -> Exit {
    let (width, height) = size;
    let offset = ((width - t::WIDTH) / 2., (height - t::HEIGHT) / 2.);
    if !grown {
        return Exit {
            card: 1. - (at / x::CROSS_FADE).min(1.),
            content: 1.,
            offset,
            tint: 0.,
            plate: (offset.0, offset.1, t::WIDTH, t::HEIGHT, t::RADIUS),
        };
    }
    let content = 1. - (at / x::CONTENT).min(1.);
    let since = (at - x::CONTENT).max(0.);
    let grown = arrive(since / x::GROW);
    let [from_radius, to_radius] = x::CORNERS;
    let (w, h) = (lerp(t::WIDTH, width, grown), lerp(t::HEIGHT, height, grown));
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
